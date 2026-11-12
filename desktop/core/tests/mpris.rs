//! Linux music detection against a fake MPRIS player on a real session bus.
//! Runs only where a session bus exists, e.g. `dbus-run-session cargo test`.
#![cfg(target_os = "linux")]

use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

struct FakePlayer;

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl FakePlayer {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        "Playing".into()
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let mut m = HashMap::new();
        let v = |v: Value<'static>| OwnedValue::try_from(v).unwrap();
        m.insert("xesam:title".into(), v(Value::from("Midnight City")));
        m.insert("xesam:artist".into(), v(Value::from(vec!["M83"])));
        m.insert("xesam:album".into(), v(Value::from("Hurry Up, We're Dreaming")));
        m.insert("mpris:length".into(), v(Value::from(244_000_000i64)));
        m
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        61_500_000
    }
}

#[test]
fn reads_playing_track_from_mpris() {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("no session bus, skipped");
        return;
    }
    let _player = zbus::blocking::connection::Builder::session()
        .unwrap()
        .name("org.mpris.MediaPlayer2.fake")
        .unwrap()
        .serve_at("/org/mpris/MediaPlayer2", FakePlayer)
        .unwrap()
        .build()
        .unwrap();

    let media = osctool_core::media::now_playing().expect("player not found");
    assert_eq!(media.title, "Midnight City");
    assert_eq!(media.artist, "M83");
    assert_eq!(media.album, "Hurry Up, We're Dreaming");
    assert!(media.playing);
    assert_eq!(media.duration_ms, Some(244_000));
    assert_eq!(media.position_ms, Some(61_500));
}
