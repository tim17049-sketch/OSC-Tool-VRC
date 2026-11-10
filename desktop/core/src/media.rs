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

#[cfg(not(windows))]
pub fn now_playing() -> Option<Media> {
    None
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
