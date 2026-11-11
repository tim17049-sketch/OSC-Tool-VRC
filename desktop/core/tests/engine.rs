//! End to end: a fake watch connects over WebSocket, a fake VRChat listens on
//! UDP, and the engine in between has to produce the right OSC messages.

use futures_util::SinkExt;
use osctool_core::{Config, Engine};
use rosc::{OscPacket, OscType};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio_tungstenite::tungstenite::Message;

fn free_tcp_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Collect OSC messages until `done` says we have seen enough.
async fn collect(
    vrchat: &UdpSocket,
    mut done: impl FnMut(&[(String, Vec<OscType>)]) -> bool,
) -> Vec<(String, Vec<OscType>)> {
    let mut seen = Vec::new();
    let mut buf = [0u8; 2048];
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    while !done(&seen) {
        let n = tokio::time::timeout_at(deadline, vrchat.recv(&mut buf))
            .await
            .expect("timed out waiting for OSC")
            .unwrap();
        if let Ok((_, OscPacket::Message(m))) = rosc::decoder::decode_udp(&buf[..n]) {
            seen.push((m.addr, m.args));
        }
    }
    seen
}

fn has(seen: &[(String, Vec<OscType>)], addr: &str, arg: &OscType) -> bool {
    seen.iter().any(|(a, args)| a == addr && args.first() == Some(arg))
}

fn chatbox_texts(seen: &[(String, Vec<OscType>)]) -> Vec<String> {
    seen.iter()
        .filter(|(a, _)| a == "/chatbox/input")
        .filter_map(|(_, args)| match args.first() {
            Some(OscType::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn heart_rate_reaches_chatbox_and_avatar() {
    let vrchat = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let hr_port = free_tcp_port();
    let config = Config {
        osc_port: vrchat.local_addr().unwrap().port(),
        hr_port,
        template: "HR {hr}\n🎵 {song}".into(),
        interval_ms: 1500,
        ..Config::default()
    };
    let engine = Engine::start(config).await.unwrap();

    // The watch connects, says hello and sends a reading.
    let url = format!("ws://127.0.0.1:{hr_port}/");
    let mut watch = None;
    for _ in 0..50 {
        if let Ok((ws, _)) = tokio_tungstenite::connect_async(&url).await {
            watch = Some(ws);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut watch = watch.expect("receiver did not come up");
    watch
        .send(Message::text(r#"{"type":"hello","device":"Test Watch"}"#))
        .await
        .unwrap();
    watch
        .send(Message::text(r#"{"type":"hr","bpm":87}"#))
        .await
        .unwrap();

    let seen = collect(&vrchat, |seen| {
        has(seen, "/avatar/parameters/HR", &OscType::Int(87))
            && has(seen, "/avatar/parameters/isHRBeat", &OscType::Bool(true))
            && chatbox_texts(seen).iter().any(|t| t == "HR 87")
    })
    .await;
    assert!(has(&seen, "/avatar/parameters/isHRConnected", &OscType::Bool(true)));
    assert!(has(&seen, "/avatar/parameters/onesHR", &OscType::Int(7)));
    assert!(has(&seen, "/avatar/parameters/tensHR", &OscType::Int(8)));

    let snapshot = engine.snapshot();
    assert_eq!(snapshot.hr.bpm, Some(87));
    assert_eq!(snapshot.hr.device.as_deref(), Some("Test Watch"));
    assert!(snapshot.network.hr_listening);

    // A typed message replaces the template ...
    engine.send_message("hello world");
    collect(&vrchat, |seen| chatbox_texts(seen).iter().any(|t| t == "hello world")).await;

    // ... and losing the watch drops the heart rate line and the avatar flag.
    engine.send_message("");
    drop(watch);
    let seen = collect(&vrchat, |seen| {
        has(seen, "/avatar/parameters/isHRConnected", &OscType::Bool(false))
            && chatbox_texts(seen).iter().any(|t| t.is_empty())
    })
    .await;
    assert!(has(&seen, "/avatar/parameters/isHRActive", &OscType::Bool(false)));
    assert_eq!(engine.snapshot().hr.bpm, None);
}

/// The watch app drops the connection when its pings go unanswered.
#[tokio::test(flavor = "multi_thread")]
async fn receiver_answers_pings() {
    use futures_util::StreamExt;

    let vrchat = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let hr_port = free_tcp_port();
    let config = Config {
        osc_port: vrchat.local_addr().unwrap().port(),
        hr_port,
        chatbox_enabled: false,
        ..Config::default()
    };
    let _engine = Engine::start(config).await.unwrap();

    let url = format!("ws://127.0.0.1:{hr_port}/");
    let mut watch = None;
    for _ in 0..50 {
        if let Ok((ws, _)) = tokio_tungstenite::connect_async(&url).await {
            watch = Some(ws);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut watch = watch.expect("receiver did not come up");
    watch.send(Message::Ping("abc".into())).await.unwrap();

    let pong = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match watch.next().await {
                Some(Ok(Message::Pong(payload))) => break payload,
                Some(Ok(_)) => continue,
                other => panic!("connection ended: {other:?}"),
            }
        }
    })
    .await
    .expect("no pong within 2 s");
    assert_eq!(&pong[..], b"abc");
}

#[tokio::test(flavor = "multi_thread")]
async fn chatbox_respects_the_rate_limit() {
    let vrchat = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let config = Config {
        osc_port: vrchat.local_addr().unwrap().port(),
        hr_port: free_tcp_port(),
        template: "static".into(),
        // Too fast on purpose; the engine has to clamp it.
        interval_ms: 10,
        hr_avatar_params: false,
        ..Config::default()
    };
    let engine = Engine::start(config).await.unwrap();
    assert_eq!(engine.config().interval_ms, 1500);

    let mut stamps = Vec::new();
    let mut buf = [0u8; 2048];
    while stamps.len() < 3 {
        vrchat.recv(&mut buf).await.unwrap();
        stamps.push(tokio::time::Instant::now());
        // Hammer the engine; none of this may speed it up.
        engine.send_message("spam");
        engine.send_message("");
    }
    for pair in stamps.windows(2) {
        assert!(pair[1] - pair[0] >= Duration::from_millis(1450));
    }
}
