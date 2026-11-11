//! Heart rate receiver: a tiny WebSocket server the watch app connects to.
//!
//! Protocol (JSON text frames, watch -> PC):
//!   {"type":"hello","device":"Galaxy Watch7","app":"0.1.0"}
//!   {"type":"hr","bpm":72}
//!   {"type":"hr","bpm":0}        sensor has no skin contact
//! A frame containing just a number ("72") is accepted as well, which makes
//! it easy to feed the tool from scripts or other devices.

use futures_util::{SinkExt, StreamExt};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use serde::Deserialize;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

/// mDNS service type the watch looks for in the local network.
pub const SERVICE_TYPE: &str = "_osctoolhr._tcp.local.";

const HISTORY: Duration = Duration::from_secs(120);
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, PartialEq)]
pub enum HrEvent {
    Connected,
    Hello { device: Option<String> },
    Sample(u16),
    /// Watch is connected but the sensor reports no reading (not worn).
    NoContact,
    Disconnected,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum WatchMsg {
    Hello { device: Option<String> },
    Hr { bpm: f64 },
}

pub fn parse_message(text: &str) -> Option<HrEvent> {
    let text = text.trim();
    let bpm = if let Ok(n) = text.parse::<f64>() {
        n
    } else {
        match serde_json::from_str::<WatchMsg>(text).ok()? {
            WatchMsg::Hello { device } => {
                let device = device
                    .map(|d| d.chars().filter(|c| !c.is_control()).take(40).collect::<String>())
                    .filter(|d| !d.trim().is_empty());
                return Some(HrEvent::Hello { device });
            }
            WatchMsg::Hr { bpm } => bpm,
        }
    };
    if !bpm.is_finite() {
        return None;
    }
    let bpm = bpm.round();
    if bpm == 0.0 {
        Some(HrEvent::NoContact)
    } else if (20.0..=255.0).contains(&bpm) {
        Some(HrEvent::Sample(bpm as u16))
    } else {
        None
    }
}

/// Everything known about the current heart rate.
#[derive(Default)]
pub struct HrState {
    pub bpm: Option<u16>,
    pub last_sample: Option<Instant>,
    pub connections: usize,
    pub device: Option<String>,
    recent: VecDeque<(Instant, u16)>,
    pub min: Option<u16>,
    pub max: Option<u16>,
    sum: u64,
    count: u64,
}

impl HrState {
    pub fn push(&mut self, bpm: u16, now: Instant) {
        self.bpm = Some(bpm);
        self.last_sample = Some(now);
        self.recent.push_back((now, bpm));
        while self
            .recent
            .front()
            .is_some_and(|(t, _)| now.duration_since(*t) > HISTORY)
        {
            self.recent.pop_front();
        }
        self.min = Some(self.min.map_or(bpm, |m| m.min(bpm)));
        self.max = Some(self.max.map_or(bpm, |m| m.max(bpm)));
        self.sum += bpm as u64;
        self.count += 1;
    }

    pub fn avg(&self) -> Option<u16> {
        (self.count > 0).then(|| (self.sum as f64 / self.count as f64).round() as u16)
    }

    /// +1 rising, -1 falling, 0 steady: the last 5 seconds compared with the
    /// 10 seconds before them. `None` until there is enough data.
    pub fn trend(&self, now: Instant) -> Option<i8> {
        self.bpm?;
        let mean = |from: u64, to: u64| -> Option<f64> {
            let (mut sum, mut n) = (0.0, 0u32);
            for (t, bpm) in &self.recent {
                let age = now.duration_since(*t);
                if age >= Duration::from_secs(from) && age < Duration::from_secs(to) {
                    sum += *bpm as f64;
                    n += 1;
                }
            }
            (n > 0).then(|| sum / n as f64)
        };
        let diff = mean(0, 5)? - mean(5, 15)?;
        Some(if diff >= 2.5 {
            1
        } else if diff <= -2.5 {
            -1
        } else {
            0
        })
    }
}

/// Accept watch connections until the future is dropped. Dropping it also
/// closes every open connection.
pub async fn serve<F>(listener: TcpListener, host_name: String, on_event: F)
where
    F: Fn(HrEvent) + Send + Sync + 'static,
{
    let on_event: Arc<dyn Fn(HrEvent) + Send + Sync> = Arc::new(on_event);
    let mut connections = JoinSet::new();
    loop {
        while connections.try_join_next().is_some() {}
        match listener.accept().await {
            Ok((stream, _)) => {
                let _ = stream.set_nodelay(true);
                connections.spawn(handle(stream, host_name.clone(), on_event.clone()));
            }
            Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
        }
    }
}

struct ConnectionGuard(Arc<dyn Fn(HrEvent) + Send + Sync>);

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        (self.0)(HrEvent::Disconnected);
    }
}

async fn handle(stream: TcpStream, host_name: String, on_event: Arc<dyn Fn(HrEvent) + Send + Sync>) {
    // Heart rate frames are tiny; refuse anything big.
    let config = WebSocketConfig::default()
        .max_message_size(Some(4096))
        .max_frame_size(Some(4096));
    let Ok(ws) = tokio_tungstenite::accept_async_with_config(stream, Some(config)).await else {
        return;
    };
    let (mut tx, mut rx) = ws.split();

    on_event(HrEvent::Connected);
    let _guard = ConnectionGuard(on_event.clone());

    let welcome = serde_json::json!({ "type": "welcome", "name": host_name }).to_string();
    if tx.send(Message::text(welcome)).await.is_err() {
        return;
    }

    loop {
        match tokio::time::timeout(IDLE_TIMEOUT, rx.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                if let Some(event) = parse_message(text.as_str()) {
                    on_event(event);
                }
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(Some(Err(_))) | Ok(None) | Err(_) => break,
            // Pings are answered by the WebSocket layer on the next read.
            Ok(Some(Ok(_))) => {}
        }
    }
}

/// Announces the receiver via mDNS so the watch finds the PC on its own.
pub struct Advertiser {
    daemon: ServiceDaemon,
}

impl Advertiser {
    pub fn start(port: u16, host_name: &str) -> Option<Self> {
        let label: String = host_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
            .collect();
        let label = if label.is_empty() { "pc".to_string() } else { label };
        let daemon = ServiceDaemon::new().ok()?;
        // IPv4 only: the watch connects to whatever address it resolves, and
        // link-local IPv6 addresses are no use to it.
        let _ = daemon.disable_interface(mdns_sd::IfKind::IPv6);
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &label,
            &format!("{label}.local."),
            "",
            port,
            &[("v", "1")][..],
        )
        .ok()?
        .enable_addr_auto();
        daemon.register(info).ok()?;
        Some(Self { daemon })
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

/// IPv4 addresses of this PC in the local network, for typing into the watch
/// when automatic discovery does not work.
pub fn local_addresses() -> Vec<String> {
    let mut out: Vec<String> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            std::net::IpAddr::V4(ip) if !ip.is_link_local() => Some(ip.to_string()),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_and_plain_numbers() {
        assert_eq!(parse_message(r#"{"type":"hr","bpm":72}"#), Some(HrEvent::Sample(72)));
        assert_eq!(parse_message(r#"{"type":"hr","bpm":71.6,"ts":1}"#), Some(HrEvent::Sample(72)));
        assert_eq!(parse_message(" 88 "), Some(HrEvent::Sample(88)));
        assert_eq!(parse_message(r#"{"type":"hr","bpm":0}"#), Some(HrEvent::NoContact));
        assert_eq!(
            parse_message(r#"{"type":"hello","device":"Galaxy Watch7"}"#),
            Some(HrEvent::Hello { device: Some("Galaxy Watch7".into()) })
        );
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_message("999"), None);
        assert_eq!(parse_message("-5"), None);
        assert_eq!(parse_message("NaN"), None);
        assert_eq!(parse_message(r#"{"type":"nope"}"#), None);
        assert_eq!(parse_message("hello"), None);
    }

    #[test]
    fn trend_follows_recent_samples() {
        let start = Instant::now();
        let mut state = HrState::default();
        for s in 0..20u64 {
            let bpm = if s < 15 { 70 } else { 90 };
            state.push(bpm, start + Duration::from_secs(s));
        }
        let now = start + Duration::from_secs(20);
        assert_eq!(state.trend(now), Some(1));
        assert_eq!(state.min, Some(70));
        assert_eq!(state.max, Some(90));
        assert_eq!(state.avg(), Some(75));
    }

    #[test]
    fn trend_needs_history() {
        let now = Instant::now();
        let mut state = HrState::default();
        assert_eq!(state.trend(now), None);
        state.push(80, now);
        assert_eq!(state.trend(now), None);
    }
}
