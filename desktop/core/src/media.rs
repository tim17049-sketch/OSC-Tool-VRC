use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Media {
    pub title: String,
    pub artist: String,
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
    if status != Status::Playing {
        return None;
    }
    let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let title = props.Title().ok()?.to_string();
    if title.trim().is_empty() {
        return None;
    }
    let artist = props.Artist().map(|a| a.to_string()).unwrap_or_default();
    Some(Media { title, artist })
}

#[cfg(not(windows))]
pub fn now_playing() -> Option<Media> {
    None
}
