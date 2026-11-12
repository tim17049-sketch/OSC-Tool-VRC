use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// false while paused.
    pub playing: bool,
    /// Playback position when this was read, if the player reports one.
    pub position_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

impl Media {
    /// "Artist – Title", or just the title when the player reports no artist.
    pub fn display(&self) -> String {
        if self.artist.trim().is_empty() {
            self.title.clone()
        } else {
            format!("{} – {}", self.artist, self.title)
        }
    }

    /// Identifies the track, e.g. to cache lyrics.
    pub fn key(&self) -> (String, String) {
        (self.artist.clone(), self.title.clone())
    }
}

/// "1:05", or "1:02:03" for long tracks.
pub fn fmt_time(ms: u64) -> String {
    let s = ms / 1000;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Progress bar such as "━━━━●─────".
pub fn progress_bar(pos_ms: u64, dur_ms: u64, width: usize) -> String {
    if dur_ms == 0 || width < 2 {
        return String::new();
    }
    let filled = ((pos_ms.min(dur_ms) as f64 / dur_ms as f64) * (width - 1) as f64).round() as usize;
    let mut bar = "━".repeat(filled);
    bar.push('●');
    bar.push_str(&"─".repeat(width - 1 - filled));
    bar
}

/// What is playing right now according to Windows' media controls (the same
/// source as the volume flyout). Works with Spotify, browsers, etc.
#[cfg(windows)]
pub fn now_playing() -> Option<Media> {
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };

    let manager = Manager::RequestAsync().ok()?.get().ok()?;
    let session = manager.GetCurrentSession().ok()?;
    let status = session.GetPlaybackInfo().ok()?.PlaybackStatus().ok()?;
    let playing = status == Status::Playing;
    if !playing && status != Status::Paused {
        return None;
    }
    let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let title = props.Title().ok()?.to_string();
    if title.trim().is_empty() {
        return None;
    }
    let artist = props.Artist().map(|a| a.to_string()).unwrap_or_default();
    let album = props.AlbumTitle().map(|a| a.to_string()).unwrap_or_default();

    // Times are in 100 ns units. Players only report the position now and
    // then, together with when they did; the rest is extrapolated.
    let (mut position_ms, mut duration_ms) = (None, None);
    if let Ok(timeline) = session.GetTimelineProperties() {
        let start = timeline.StartTime().map(|t| t.Duration).unwrap_or(0);
        let end = timeline.EndTime().map(|t| t.Duration).unwrap_or(0);
        if end > start {
            let duration = (end - start) / 10_000;
            let mut position = timeline.Position().map(|t| t.Duration - start).unwrap_or(0) / 10_000;
            if playing {
                if let Ok(updated) = timeline.LastUpdatedTime() {
                    // FILETIME epoch (1601) to Unix epoch (1970).
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as i64 / 100 + 116_444_736_000_000_000)
                        .unwrap_or(0);
                    let since = (now - updated.UniversalTime) / 10_000;
                    if (0..duration).contains(&since) {
                        position += since;
                    }
                }
            }
            duration_ms = Some(duration as u64);
            position_ms = Some(position.clamp(0, duration) as u64);
        }
    }
    Some(Media {
        title,
        artist,
        album,
        playing,
        position_ms,
        duration_ms,
    })
}

/// What is playing right now according to MPRIS, the D-Bus interface that
/// Spotify, Firefox, Chromium, VLC and most other Linux players offer.
#[cfg(target_os = "linux")]
pub fn now_playing() -> Option<Media> {
    linux::now_playing()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn now_playing() -> Option<Media> {
    None
}

#[cfg(target_os = "linux")]
mod linux {
    use super::Media;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use zbus::blocking::fdo::{DBusProxy, PropertiesProxy};
    use zbus::blocking::Connection;
    use zbus::names::InterfaceName;
    use zbus::zvariant::{OwnedValue, Value};

    const PREFIX: &str = "org.mpris.MediaPlayer2.";
    const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

    /// One session bus connection, opened on first use and again after errors.
    static BUS: Mutex<Option<Connection>> = Mutex::new(None);

    fn bus() -> Option<Connection> {
        let mut bus = BUS.lock().ok()?;
        if bus.is_none() {
            *bus = Connection::session().ok();
        }
        bus.clone()
    }

    pub fn now_playing() -> Option<Media> {
        let bus = bus()?;
        let names = match DBusProxy::new(&bus).and_then(|p| p.list_names().map_err(zbus::Error::from)) {
            Ok(names) => names,
            Err(_) => {
                if let Ok(mut b) = BUS.lock() {
                    *b = None;
                }
                return None;
            }
        };
        let mut paused = None;
        for name in names.iter().map(|n| n.as_str()) {
            // playerctld only mirrors the other players.
            if !name.starts_with(PREFIX) || name == "org.mpris.MediaPlayer2.playerctld" {
                continue;
            }
            match read_player(&bus, name) {
                Some(media) if media.playing => return Some(media),
                Some(media) if paused.is_none() => paused = Some(media),
                _ => {}
            }
        }
        paused
    }

    /// Variants inside the metadata dictionary come wrapped once more.
    fn inner<'a>(value: &'a Value<'a>) -> &'a Value<'a> {
        match value {
            Value::Value(v) => inner(v),
            other => other,
        }
    }

    fn text(value: Option<&OwnedValue>) -> Option<String> {
        match inner(value?) {
            Value::Str(s) => Some(s.to_string()),
            Value::Array(a) => {
                let parts: Vec<String> = a
                    .iter()
                    .filter_map(|v| match inner(v) {
                        Value::Str(s) => Some(s.to_string()),
                        _ => None,
                    })
                    .collect();
                Some(parts.join(", "))
            }
            _ => None,
        }
    }

    /// Microseconds as reported by MPRIS, converted to milliseconds.
    fn micros(value: Option<&OwnedValue>) -> Option<u64> {
        let us = match inner(value?) {
            Value::I64(v) => u64::try_from(*v).ok()?,
            Value::U64(v) => *v,
            Value::I32(v) => u64::try_from(*v).ok()?,
            Value::U32(v) => u64::from(*v),
            Value::F64(v) if *v >= 0.0 => *v as u64,
            _ => return None,
        };
        Some(us / 1000)
    }

    fn read_player(bus: &Connection, name: &str) -> Option<Media> {
        let props = PropertiesProxy::builder(bus)
            .destination(name)
            .ok()?
            .path("/org/mpris/MediaPlayer2")
            .ok()?
            .build()
            .ok()?;
        let all = props
            .get_all(InterfaceName::from_static_str_unchecked(PLAYER))
            .ok()?;
        let status = text(all.get("PlaybackStatus"))?;
        let playing = status == "Playing";
        if !playing && status != "Paused" {
            return None;
        }
        let metadata: HashMap<String, OwnedValue> = all
            .get("Metadata")
            .and_then(|m| m.try_clone().ok())
            .and_then(|m| HashMap::try_from(m).ok())
            .unwrap_or_default();
        let title = text(metadata.get("xesam:title")).filter(|t| !t.trim().is_empty())?;
        let duration_ms = micros(metadata.get("mpris:length")).filter(|d| *d > 0);
        // Some players leave Position out of GetAll; ask for it on its own.
        let position_ms = micros(all.get("Position")).or_else(|| {
            let value = props
                .get(InterfaceName::from_static_str_unchecked(PLAYER), "Position")
                .ok()?;
            micros(Some(&value))
        });
        Some(Media {
            title,
            artist: text(metadata.get("xesam:artist")).unwrap_or_default(),
            album: text(metadata.get("xesam:album")).unwrap_or_default(),
            playing,
            position_ms: duration_ms.and(position_ms).map(|p| p.min(duration_ms.unwrap_or(p))),
            duration_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_time() {
        assert_eq!(fmt_time(65_400), "1:05");
        assert_eq!(fmt_time(3_723_000), "1:02:03");
    }

    #[test]
    fn draws_progress() {
        assert_eq!(progress_bar(0, 100, 5), "●────");
        assert_eq!(progress_bar(50, 100, 5), "━━●──");
        assert_eq!(progress_bar(100, 100, 5), "━━━━●");
        assert_eq!(progress_bar(500, 100, 5), "━━━━●");
        assert_eq!(progress_bar(1, 0, 5), "");
    }
}
