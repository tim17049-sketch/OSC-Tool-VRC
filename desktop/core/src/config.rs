use serde::{Deserialize, Serialize};
use std::path::Path;

/// VRChat throttles the chatbox when it is updated too often; below this
/// interval the chatbox gets muted for a while.
pub const MIN_INTERVAL_MS: u64 = 1500;

pub const DEFAULT_TEMPLATE: &str = "❤ {hr} bpm {hr_trend}\n🎵 {song}\n🕐 {time}";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Where VRChat listens for OSC.
    pub osc_host: String,
    pub osc_port: u16,

    /// Send the template to the chatbox continuously.
    pub chatbox_enabled: bool,
    pub template: String,
    /// Time between two chatbox updates.
    pub interval_ms: u64,
    /// Play VRChat's notification sound for manually sent messages.
    pub notify_sound: bool,
    /// How long a manually sent message replaces the template.
    pub message_seconds: u64,
    pub time_24h: bool,

    /// Port the watch connects to (WebSocket).
    pub hr_port: u16,
    /// Send heart rate as avatar parameters (HR, HRPercent, isHRBeat, ...).
    pub hr_avatar_params: bool,
    /// Range mapped to HRPercent 0..1.
    pub hr_min: u16,
    pub hr_max: u16,
    /// Seconds without a sample until the heart rate counts as lost.
    pub hr_timeout_s: u64,

    /// "auto", "de" or "en". Only used by the UI and for the date format.
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            osc_host: "127.0.0.1".into(),
            osc_port: 9000,
            chatbox_enabled: true,
            template: DEFAULT_TEMPLATE.into(),
            interval_ms: 2000,
            notify_sound: false,
            message_seconds: 10,
            time_24h: true,
            hr_port: 9555,
            hr_avatar_params: true,
            hr_min: 0,
            hr_max: 255,
            hr_timeout_s: 10,
            language: "auto".into(),
        }
    }
}

impl Config {
    /// Clamp values a hand-edited file or a buggy UI could get wrong.
    pub fn sanitized(mut self) -> Self {
        self.interval_ms = self.interval_ms.clamp(MIN_INTERVAL_MS, 60_000);
        self.message_seconds = self.message_seconds.clamp(2, 120);
        self.hr_timeout_s = self.hr_timeout_s.clamp(3, 120);
        if self.osc_host.trim().is_empty() {
            self.osc_host = "127.0.0.1".into();
        }
        if self.osc_port == 0 {
            self.osc_port = 9000;
        }
        if self.hr_port == 0 {
            self.hr_port = 9555;
        }
        if self.hr_max <= self.hr_min {
            self.hr_min = 0;
            self.hr_max = 255;
        }
        self
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }
}
