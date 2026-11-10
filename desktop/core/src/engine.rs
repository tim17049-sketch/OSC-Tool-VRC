use crate::config::{Config, ParamKind, MIN_INTERVAL_MS};
use crate::hr::{self, Advertiser, DeviceInfo, HrEvent, HrState};
use crate::lyrics::{self, Lines};
use crate::media::{self, Media};
use crate::osc::{self, clamp_chatbox, PARAM_PREFIX};
use crate::scanner::{self, Rng};
use crate::stats::{self, Load, Stats};
use crate::template::{self, Values};
use rosc::OscType;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{broadcast, watch, Notify};
use tokio::task::JoinHandle;

/// State of the whole tool as shown by the UI.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub hr: HrSnapshot,
    pub chatbox: ChatboxSnapshot,
    pub media: Option<Media>,
    /// Synced lyrics for the current song, while the template uses
    /// `{lyric}`: "loading", "found", "missing" or "error".
    pub lyrics: Option<&'static str>,
    pub cpu: f32,
    pub ram: f32,
    pub network: NetworkSnapshot,
    pub receiver: ReceiverSnapshot,
    pub scanner: ScannerSnapshot,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReceiverSnapshot {
    pub enabled: bool,
    pub port: u16,
    pub listening: bool,
    pub error: Option<String>,
    /// Messages received since the start.
    pub total: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    /// Local time, "HH:MM:SS".
    pub time: String,
    pub name: String,
    pub percent: u8,
    pub verdict: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ScannerSnapshot {
    pub scanning: bool,
    /// Newest first.
    pub results: Vec<ScanResult>,
}

/// Latest value of every OSC address VRChat sent, for the monitor.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MonitorRow {
    pub address: String,
    pub value: String,
    pub count: u64,
    pub age_ms: u64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HrSnapshot {
    pub bpm: Option<u16>,
    /// A watch (or another sender) is connected.
    pub connected: bool,
    pub device: Option<DeviceInfo>,
    pub trend: Option<i8>,
    pub min: Option<u16>,
    pub max: Option<u16>,
    pub avg: Option<u16>,
    pub sample_age_ms: Option<u64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatboxSnapshot {
    /// What the chatbox shows (or would show while sending is switched off).
    pub preview: String,
    pub enabled: bool,
    pub sent_count: u64,
    pub last_sent_ms_ago: Option<u64>,
    /// Seconds a manually sent message stays up, if one is showing.
    pub message_remaining_s: Option<u64>,
    /// The message stays until it is removed.
    pub message_kept: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSnapshot {
    pub host_name: String,
    pub addresses: Vec<String>,
    pub hr_port: u16,
    pub hr_listening: bool,
    /// The watch can find this PC by itself.
    pub discoverable: bool,
    pub hr_error: Option<String>,
}

struct Message {
    text: String,
    started: Instant,
    /// `None`: as long as the settings say.
    duration: Option<Duration>,
    /// Not sent yet; goes out as soon as VRChat's rate limit allows.
    fresh: bool,
    sound_pending: bool,
}

impl Message {
    /// Remaining time, `None` if it stays until removed.
    fn remaining(&self, cfg: &Config) -> Option<Duration> {
        let duration = match self.duration {
            Some(d) => d,
            None if cfg.message_keep => return None,
            None => Duration::from_secs(cfg.message_seconds),
        };
        Some(duration.saturating_sub(self.started.elapsed()))
    }
}

#[derive(Default)]
struct ChatState {
    last_text: String,
    last_sent: Option<Instant>,
    sent_count: u64,
    error: Option<String>,
    message: Option<Message>,
}

#[derive(Default)]
struct ServerState {
    listening: bool,
    discoverable: bool,
    error: Option<String>,
}

#[derive(Default)]
struct EnvState {
    media: Option<Media>,
    /// When `media` was read; its position moves on from there.
    media_at: Option<Instant>,
    load: Load,
    addresses: Vec<String>,
}

type TrackKey = (String, String);

#[derive(Default)]
struct LyricsState {
    key: Option<TrackKey>,
    status: Option<&'static str>,
    lines: Option<Arc<Lines>>,
    /// Finished lookups; `None` = no synced lyrics exist.
    cache: HashMap<TrackKey, Option<Arc<Lines>>>,
    loading: Option<TrackKey>,
    failed: Option<(TrackKey, Instant)>,
}

struct MonitorEntry {
    value: String,
    count: u64,
    last: Instant,
}

#[derive(Default)]
struct ReceiverState {
    listening: bool,
    error: Option<String>,
    total: u64,
    entries: HashMap<String, MonitorEntry>,
}

#[derive(Default)]
struct ScanState {
    busy_until: Option<Instant>,
    results: VecDeque<ScanResult>,
    /// Last value of the trigger parameter, to react to off -> on only.
    trigger_on: bool,
}

/// Lyrics are shown this much ahead, since the chatbox lags a little.
const LYRIC_LEAD_MS: u64 = 600;
const MONITOR_MAX: usize = 512;

struct Inner {
    cfg: RwLock<Config>,
    hr: Mutex<HrState>,
    chat: Mutex<ChatState>,
    env: Mutex<EnvState>,
    server: Mutex<ServerState>,
    lyrics: Mutex<LyricsState>,
    receiver: Mutex<ReceiverState>,
    scan: Mutex<ScanState>,
    socket: UdpSocket,
    /// Runtime the engine lives on; lets non-async callers start work.
    rt: tokio::runtime::Handle,
    host_name: String,
    wake_chat: Notify,
    restart_server: Notify,
    /// Turns true on shutdown; tells connected watches to stop.
    quitting: watch::Sender<bool>,
    restart_receiver: Notify,
    events: broadcast::Sender<Snapshot>,
}

/// Runs everything: the heart rate receiver, the chatbox loop and the avatar
/// parameter output. Dropping the engine stops all of it.
pub struct Engine {
    inner: Arc<Inner>,
    tasks: Vec<JoinHandle<()>>,
}

impl Engine {
    /// Must be called from within a tokio runtime.
    pub async fn start(cfg: Config) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(("0.0.0.0", 0)).await?;
        let (events, _) = broadcast::channel(32);
        let inner = Arc::new(Inner {
            cfg: RwLock::new(cfg.sanitized()),
            hr: Mutex::default(),
            chat: Mutex::default(),
            env: Mutex::new(EnvState {
                addresses: hr::local_addresses(),
                ..Default::default()
            }),
            server: Mutex::default(),
            lyrics: Mutex::default(),
            receiver: Mutex::default(),
            scan: Mutex::default(),
            socket,
            rt: tokio::runtime::Handle::current(),
            host_name: stats::host_name(),
            wake_chat: Notify::new(),
            restart_server: Notify::new(),
            quitting: watch::Sender::new(false),
            restart_receiver: Notify::new(),
            events,
        });
        let tasks = vec![
            tokio::spawn(inner.clone().run_server()),
            tokio::spawn(inner.clone().run_chatbox()),
            tokio::spawn(inner.clone().run_beat()),
            tokio::spawn(inner.clone().run_watchdog()),
            tokio::spawn(inner.clone().run_environment()),
            tokio::spawn(inner.clone().run_receiver()),
        ];
        Ok(Self { inner, tasks })
    }

    pub fn config(&self) -> Config {
        self.inner.cfg()
    }

    pub fn set_config(&self, cfg: Config) -> Config {
        let cfg = cfg.sanitized();
        let (port_changed, receiver_changed) = {
            let mut current = self.inner.cfg.write().unwrap();
            let changed = (
                current.hr_port != cfg.hr_port,
                current.osc_receive != cfg.osc_receive
                    || current.osc_receive_port != cfg.osc_receive_port,
            );
            *current = cfg.clone();
            changed
        };
        if port_changed {
            self.inner.restart_server.notify_one();
        }
        if receiver_changed {
            self.inner.restart_receiver.notify_one();
        }
        self.inner.wake_chat.notify_one();
        self.inner.emit();
        cfg
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.snapshot()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Snapshot> {
        self.inner.events.subscribe()
    }

    /// Show `text` in the chatbox for a while instead of the template. An
    /// empty text ends a message that is currently showing.
    pub fn send_message(&self, text: &str) {
        self.inner.show_message(text, None, true);
    }

    /// Run the furry scanner on `name` (or the configured name). Returns
    /// false while a scan is still running.
    pub fn scan(&self, name: Option<String>) -> bool {
        self.inner.scan(name)
    }

    /// Set an avatar parameter once.
    pub fn send_param(&self, name: &str, kind: ParamKind, value: f64) -> Result<(), String> {
        let name = osc::param_name(name).ok_or("invalid parameter name")?;
        if !value.is_finite() {
            return Err("invalid value".into());
        }
        let value = match kind {
            ParamKind::Bool => OscType::Bool(value != 0.0),
            ParamKind::Int => OscType::Int(value.round().clamp(-2_147_483_648.0, 2_147_483_647.0) as i32),
            ParamKind::Float => OscType::Float(value.clamp(-1.0, 1.0) as f32),
        };
        let inner = self.inner.clone();
        self.inner
            .rt
            .spawn(async move { inner.send(osc::avatar_param(&name, value)).await });
        Ok(())
    }

    /// Everything VRChat sent so far, latest value per address.
    pub fn monitor(&self) -> Vec<MonitorRow> {
        let receiver = self.inner.receiver.lock().unwrap();
        let mut rows: Vec<MonitorRow> = receiver
            .entries
            .iter()
            .map(|(address, e)| MonitorRow {
                address: address.clone(),
                value: e.value.clone(),
                count: e.count,
                age_ms: e.last.elapsed().as_millis() as u64,
            })
            .collect();
        rows.sort_by(|a, b| a.address.cmp(&b.address));
        rows
    }

    pub fn clear_monitor(&self) {
        self.inner.receiver.lock().unwrap().entries.clear();
    }

    /// Toggle the "typing" dots above the avatar.
    pub fn set_typing(&self, typing: bool) {
        let inner = self.inner.clone();
        self.inner
            .rt
            .spawn(async move { inner.send(osc::chatbox_typing(typing)).await });
    }

    /// Clear the chatbox and tell the avatar the heart rate is gone. Call
    /// before the app exits.
    pub async fn shutdown(&self) {
        // Say goodbye to the watch so it stops measuring, and give the close
        // frames a moment to leave before the process ends.
        self.inner.quitting.send_replace(true);
        let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
        while self.inner.hr.lock().unwrap().connections > 0 && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let cfg = self.inner.cfg();
        let showing = !self.inner.chat.lock().unwrap().last_text.is_empty();
        if showing {
            self.inner.send(osc::chatbox_input("", false)).await;
        }
        if cfg.hr_avatar_params {
            for name in ["isHRConnected", "isHRActive", "isHRBeat"] {
                self.inner
                    .send(osc::avatar_param(name, OscType::Bool(false)))
                    .await;
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Inner {
    fn cfg(&self) -> Config {
        self.cfg.read().unwrap().clone()
    }

    fn emit(&self) {
        // Nobody listening is fine.
        let _ = self.events.send(self.snapshot());
    }

    async fn send(&self, packet: Vec<u8>) {
        let (host, port) = {
            let cfg = self.cfg.read().unwrap();
            (cfg.osc_host.clone(), cfg.osc_port)
        };
        let result = self.socket.send_to(&packet, (host.as_str(), port)).await;
        let mut chat = self.chat.lock().unwrap();
        chat.error = result.err().map(|e| e.to_string());
    }

    // ---- heart rate -------------------------------------------------------

    async fn run_server(self: Arc<Self>) {
        loop {
            let port = self.cfg.read().unwrap().hr_port;
            let restart = self.restart_server.notified();
            tokio::pin!(restart);

            match TcpListener::bind(("0.0.0.0", port)).await {
                Ok(listener) => {
                    let advertiser = Advertiser::start(port, &self.host_name);
                    {
                        let mut server = self.server.lock().unwrap();
                        server.listening = true;
                        server.discoverable = advertiser.is_some();
                        server.error = None;
                    }
                    self.emit();
                    let me = self.clone();
                    let serve = hr::serve(listener, self.host_name.clone(), self.quitting.subscribe(), move |event| {
                        me.on_hr_event(event)
                    });
                    tokio::select! {
                        _ = serve => {}
                        _ = &mut restart => {}
                    }
                    drop(advertiser);
                }
                Err(e) => {
                    {
                        let mut server = self.server.lock().unwrap();
                        server.listening = false;
                        server.discoverable = false;
                        server.error = Some(e.to_string());
                    }
                    self.emit();
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                        _ = &mut restart => {}
                    }
                }
            }
            let mut server = self.server.lock().unwrap();
            server.listening = false;
            server.discoverable = false;
        }
    }

    /// Called by the receiver for every event of every connection.
    fn on_hr_event(self: &Arc<Self>, event: HrEvent) {
        {
            let mut hr = self.hr.lock().unwrap();
            match event {
                HrEvent::Connected => hr.connections += 1,
                HrEvent::Hello(info) => hr.device = Some(info),
                HrEvent::Status { battery, charging } => {
                    let device = hr.device.get_or_insert_with(DeviceInfo::default);
                    device.battery = battery.or(device.battery);
                    device.charging = charging.or(device.charging);
                }
                HrEvent::Sample(bpm) => hr.push(bpm, Instant::now()),
                HrEvent::NoContact => hr.bpm = None,
                HrEvent::Disconnected => {
                    hr.connections = hr.connections.saturating_sub(1);
                    if hr.connections == 0 {
                        hr.bpm = None;
                        hr.device = None;
                    }
                }
            }
        }
        let me = self.clone();
        self.rt.spawn(async move { me.send_hr_params().await });
        self.emit();
    }

    /// Avatar parameters in the layout established by HRtoVRChat_OSC, which
    /// most heart rate prefabs understand.
    async fn send_hr_params(&self) {
        let cfg = self.cfg();
        if !cfg.hr_avatar_params {
            return;
        }
        let (bpm, connected) = {
            let hr = self.hr.lock().unwrap();
            (hr.bpm, hr.connections > 0)
        };
        let value = bpm.unwrap_or(0);
        let range = (cfg.hr_max - cfg.hr_min) as f32;
        let percent = ((value.saturating_sub(cfg.hr_min)) as f32 / range).clamp(0.0, 1.0);
        let params = [
            ("isHRConnected", OscType::Bool(connected)),
            ("isHRActive", OscType::Bool(bpm.is_some())),
            ("HR", OscType::Int(value as i32)),
            ("onesHR", OscType::Int((value % 10) as i32)),
            ("tensHR", OscType::Int((value / 10 % 10) as i32)),
            ("hundredsHR", OscType::Int((value / 100 % 10) as i32)),
            ("HRPercent", OscType::Float(percent)),
            ("FullHRPercent", OscType::Float(percent * 2.0 - 1.0)),
        ];
        for (name, value) in params {
            self.send(osc::avatar_param(name, value)).await;
        }
    }

    /// Flash `isHRBeat` once per heartbeat.
    async fn run_beat(self: Arc<Self>) {
        loop {
            let bpm = {
                let enabled = self.cfg.read().unwrap().hr_avatar_params;
                let hr = self.hr.lock().unwrap();
                hr.bpm.filter(|_| enabled)
            };
            let Some(bpm) = bpm else {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            };
            let period = Duration::from_millis(60_000 / bpm.max(20) as u64);
            let flash = Duration::from_millis(120).min(period / 2);
            self.send(osc::avatar_param("isHRBeat", OscType::Bool(true))).await;
            tokio::time::sleep(flash).await;
            self.send(osc::avatar_param("isHRBeat", OscType::Bool(false))).await;
            tokio::time::sleep(period - flash).await;
        }
    }

    /// Notice when the watch stops sending and keep the UI up to date.
    async fn run_watchdog(self: Arc<Self>) {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let timeout = Duration::from_secs(self.cfg.read().unwrap().hr_timeout_s);
            let lost = {
                let mut hr = self.hr.lock().unwrap();
                let stale = hr.bpm.is_some()
                    && hr.last_sample.is_some_and(|t| t.elapsed() > timeout);
                if stale {
                    hr.bpm = None;
                }
                stale
            };
            if lost {
                self.send_hr_params().await;
                self.wake_chat.notify_one();
            }
            self.emit();
        }
    }

    // ---- chatbox ----------------------------------------------------------

    fn values(&self, cfg: &Config) -> Values {
        let mut v = Values::new();
        let now = Instant::now();
        let beat_frame = self.chat.lock().unwrap().sent_count % 2 == 0;
        {
            let hr = self.hr.lock().unwrap();
            let active = hr.bpm.is_some();
            v.insert("hr", hr.bpm.map(|b| b.to_string()));
            v.insert(
                "hr_trend",
                // Without a trend yet, show nothing rather than drop the line.
                active.then(|| match hr.trend(now) {
                    Some(1) => "↑".to_string(),
                    Some(-1) => "↓".to_string(),
                    Some(_) => "→".to_string(),
                    None => String::new(),
                }),
            );
            v.insert(
                "heart",
                active.then(|| if beat_frame { "❤" } else { "♡" }.to_string()),
            );
            v.insert("hr_min", hr.min.filter(|_| active).map(|b| b.to_string()));
            v.insert("hr_max", hr.max.filter(|_| active).map(|b| b.to_string()));
            v.insert("hr_avg", hr.avg().filter(|_| active).map(|b| b.to_string()));
        }
        {
            let hr = self.hr.lock().unwrap();
            let device = hr.device.as_ref().filter(|_| hr.connections > 0);
            v.insert("watch", device.and_then(|d| d.name.clone()));
            v.insert(
                "watch_battery",
                device.and_then(|d| d.battery).map(|b| format!("{b}%")),
            );
        }
        let media = self.media_now().filter(|m| m.playing || cfg.show_paused);
        let nonempty = |s: &String| !s.trim().is_empty();
        v.insert("song", media.as_ref().map(Media::display));
        v.insert("title", media.as_ref().map(|m| m.title.clone()));
        v.insert("artist", media.as_ref().map(|m| m.artist.clone()).filter(nonempty));
        v.insert("album", media.as_ref().map(|m| m.album.clone()).filter(nonempty));
        v.insert(
            "play",
            media.as_ref().map(|m| if m.playing { "▶" } else { "⏸" }.to_string()),
        );
        let timeline = media.as_ref().and_then(|m| Some((m.position_ms?, m.duration_ms?)));
        v.insert("pos", timeline.map(|(p, _)| media::fmt_time(p)));
        v.insert("dur", timeline.map(|(_, d)| media::fmt_time(d)));
        v.insert("bar", timeline.map(|(p, d)| media::progress_bar(p, d, 10)));
        v.insert("lyric", self.current_lyric(media.as_ref()));
        {
            let env = self.env.lock().unwrap();
            v.insert("cpu", Some(format!("{:.0}%", env.load.cpu)));
            v.insert("ram", Some(format!("{:.0}%", env.load.ram)));
        }
        let local = chrono::Local::now();
        let time = if cfg.time_24h { "%H:%M" } else { "%-I:%M %p" };
        let date = if cfg.language == "en" { "%Y-%m-%d" } else { "%d.%m.%Y" };
        v.insert("time", Some(local.format(time).to_string()));
        v.insert("date", Some(local.format(date).to_string()));
        v
    }

    fn render_template(&self, cfg: &Config) -> String {
        clamp_chatbox(&template::render(&cfg.template, &self.values(cfg)))
    }

    /// What is playing, with the position moved on to now.
    fn media_now(&self) -> Option<Media> {
        let env = self.env.lock().unwrap();
        let mut media = env.media.clone()?;
        if media.playing {
            if let (Some(pos), Some(at)) = (media.position_ms.as_mut(), env.media_at) {
                *pos += at.elapsed().as_millis() as u64;
                if let Some(dur) = media.duration_ms {
                    *pos = (*pos).min(dur);
                }
            }
        }
        Some(media)
    }

    fn current_lyric(&self, media: Option<&Media>) -> Option<String> {
        let media = media.filter(|m| m.playing)?;
        let pos = media.position_ms?;
        let lyrics = self.lyrics.lock().unwrap();
        if lyrics.key.as_ref() != Some(&media.key()) {
            return None;
        }
        let line = lyrics::line_at(lyrics.lines.as_ref()?, pos + LYRIC_LEAD_MS)?;
        Some(line.chars().take(70).collect())
    }

    /// Replace the template with `text` for `duration` (`None`: as the
    /// settings say). An empty text removes the current message.
    fn show_message(&self, text: &str, duration: Option<Duration>, sound: bool) {
        let text = clamp_chatbox(text.trim());
        {
            let mut chat = self.chat.lock().unwrap();
            chat.message = (!text.is_empty()).then(|| Message {
                text,
                started: Instant::now(),
                duration,
                fresh: true,
                sound_pending: sound,
            });
        }
        self.wake_chat.notify_one();
        self.emit();
    }

    /// The manually sent message, as long as it is still showing.
    fn active_message(&self, cfg: &Config) -> Option<String> {
        let mut chat = self.chat.lock().unwrap();
        let expired = chat
            .message
            .as_ref()
            .is_some_and(|m| m.remaining(cfg).is_some_and(|r| r.is_zero()));
        if expired {
            chat.message = None;
        }
        chat.message.as_ref().map(|m| m.text.clone())
    }

    async fn run_chatbox(self: Arc<Self>) {
        loop {
            let cfg = self.cfg();
            let message = self.active_message(&cfg);
            // A new message goes out as soon as VRChat allows, not only at
            // the next regular update.
            let fresh = self.chat.lock().unwrap().message.as_ref().is_some_and(|m| m.fresh);
            let interval = Duration::from_millis(if fresh {
                MIN_INTERVAL_MS
            } else {
                cfg.interval_ms.max(MIN_INTERVAL_MS)
            });

            let wanted = match &message {
                Some(text) => text.clone(),
                None if cfg.chatbox_enabled => self.render_template(&cfg),
                None => String::new(),
            };

            let (last_sent, showing) = {
                let chat = self.chat.lock().unwrap();
                (chat.last_sent, !chat.last_text.is_empty())
            };
            let wait = last_sent
                .map(|t| interval.saturating_sub(t.elapsed()))
                .unwrap_or_default();

            // VRChat hides the chatbox after a few seconds, so the text is
            // re-sent every interval. An empty text is sent once, to clear.
            let has_work = !wanted.is_empty() || showing;
            if has_work && wait.is_zero() {
                let notify = {
                    let mut chat = self.chat.lock().unwrap();
                    let pending = chat
                        .message
                        .as_mut()
                        .map(|m| {
                            m.fresh = false;
                            std::mem::take(&mut m.sound_pending)
                        })
                        .unwrap_or(false);
                    pending && cfg.notify_sound
                };
                self.send(osc::chatbox_input(&wanted, notify)).await;
                {
                    let mut chat = self.chat.lock().unwrap();
                    chat.last_text = wanted;
                    chat.last_sent = Some(Instant::now());
                    chat.sent_count += 1;
                }
                self.emit();
                continue;
            }

            let nap = if has_work { wait } else { Duration::from_millis(500) };
            tokio::select! {
                _ = tokio::time::sleep(nap) => {}
                _ = self.wake_chat.notified() => {}
            }
        }
    }

    // ---- environment ------------------------------------------------------

    async fn run_environment(self: Arc<Self>) {
        let mut stats = Stats::new();
        let mut round = 0u32;
        loop {
            let load = stats.sample();
            let media = tokio::task::spawn_blocking(media::now_playing)
                .await
                .unwrap_or(None);
            self.update_lyrics(media.as_ref());
            {
                let mut env = self.env.lock().unwrap();
                env.load = load;
                env.media = media;
                env.media_at = Some(Instant::now());
                if round % 5 == 0 {
                    env.addresses = hr::local_addresses();
                }
            }
            round = round.wrapping_add(1);
            // Often enough for lyrics to stay in sync after seeking.
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// Look up lyrics for a new song, but only while the template wants them.
    fn update_lyrics(self: &Arc<Self>, media: Option<&Media>) {
        let wanted = self.cfg.read().unwrap().template.contains("{lyric}");
        let mut lyrics = self.lyrics.lock().unwrap();
        let Some(media) = media.filter(|_| wanted) else {
            lyrics.key = None;
            lyrics.lines = None;
            lyrics.status = None;
            return;
        };
        let key = media.key();
        if lyrics.key.as_ref() == Some(&key) && lyrics.status != Some("error") {
            return;
        }
        lyrics.key = Some(key.clone());
        lyrics.lines = None;
        if let Some(cached) = lyrics.cache.get(&key).cloned() {
            lyrics.status = Some(if cached.is_some() { "found" } else { "missing" });
            lyrics.lines = cached;
            return;
        }
        // After a network error, try again a minute later.
        if let Some((failed, at)) = &lyrics.failed {
            if *failed == key && at.elapsed() < Duration::from_secs(60) {
                lyrics.status = Some("error");
                return;
            }
        }
        lyrics.status = Some("loading");
        if lyrics.loading.is_some() {
            return; // checked again once the running lookup is done
        }
        lyrics.loading = Some(key.clone());
        let me = self.clone();
        let duration = media.duration_ms;
        self.rt.spawn(async move {
            let (artist, title) = key.clone();
            let result = tokio::task::spawn_blocking(move || lyrics::fetch(&artist, &title, duration))
                .await
                .unwrap_or_else(|e| Err(e.to_string()));
            let mut lyrics = me.lyrics.lock().unwrap();
            lyrics.loading = None;
            match result {
                Ok(found) => {
                    if lyrics.cache.len() >= 64 {
                        lyrics.cache.clear();
                    }
                    let found = found.map(Arc::new);
                    lyrics.cache.insert(key.clone(), found.clone());
                    if lyrics.key.as_ref() == Some(&key) {
                        lyrics.status = Some(if found.is_some() { "found" } else { "missing" });
                        lyrics.lines = found;
                    } else {
                        // The song changed meanwhile; look up the new one.
                        lyrics.key = None;
                    }
                }
                Err(_) => {
                    lyrics.failed = Some((key.clone(), Instant::now()));
                    if lyrics.key.as_ref() == Some(&key) {
                        lyrics.status = Some("error");
                    } else {
                        lyrics.key = None;
                    }
                }
            }
        });
    }

    // ---- furry scanner ----------------------------------------------------

    fn scan(self: &Arc<Self>, name: Option<String>) -> bool {
        let cfg = self.cfg();
        let en = cfg.english();
        let name: String = name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .or_else(|| Some(cfg.scan_name.clone()).filter(|n| !n.is_empty()))
            .unwrap_or_else(|| if en { "You" } else { "Du" }.to_string())
            .chars()
            .filter(|c| !c.is_control())
            .take(20)
            .collect();
        {
            let mut scan = self.scan.lock().unwrap();
            if scan.busy_until.is_some_and(|t| Instant::now() < t) {
                return false;
            }
            scan.busy_until = Some(Instant::now() + Duration::from_secs(4));
        }
        let mut rng = Rng::from_time();
        self.show_message(&scanner::scanning_text(&mut rng, &name, en), Some(Duration::from_secs(5)), false);

        let me = self.clone();
        self.rt.spawn(async move {
            tokio::time::sleep(Duration::from_millis(2600)).await;
            let verdict = scanner::roll(&mut rng, en);
            let shown = Duration::from_secs(me.cfg().message_seconds.max(8));
            {
                let mut scan = me.scan.lock().unwrap();
                scan.results.push_front(ScanResult {
                    time: chrono::Local::now().format("%H:%M:%S").to_string(),
                    name: name.clone(),
                    percent: verdict.percent,
                    verdict: verdict.verdict.clone(),
                });
                scan.results.truncate(15);
                scan.busy_until = None;
            }
            me.show_message(&scanner::result_text(&name, &verdict), Some(shown), true);
        });
        true
    }

    // ---- receiving from VRChat --------------------------------------------

    async fn run_receiver(self: Arc<Self>) {
        loop {
            let restart = self.restart_receiver.notified();
            tokio::pin!(restart);
            let (enabled, port) = {
                let cfg = self.cfg.read().unwrap();
                (cfg.osc_receive, cfg.osc_receive_port)
            };
            if !enabled {
                {
                    let mut receiver = self.receiver.lock().unwrap();
                    receiver.listening = false;
                    receiver.error = None;
                }
                self.emit();
                restart.await;
                continue;
            }
            match UdpSocket::bind(("0.0.0.0", port)).await {
                Ok(socket) => {
                    {
                        let mut receiver = self.receiver.lock().unwrap();
                        receiver.listening = true;
                        receiver.error = None;
                    }
                    self.emit();
                    let mut buf = vec![0u8; 65_536];
                    loop {
                        tokio::select! {
                            received = socket.recv_from(&mut buf) => {
                                if let Ok((n, _)) = received {
                                    self.on_osc(&buf[..n]);
                                }
                            }
                            _ = &mut restart => break,
                        }
                    }
                }
                Err(e) => {
                    {
                        let mut receiver = self.receiver.lock().unwrap();
                        receiver.listening = false;
                        receiver.error = Some(e.to_string());
                    }
                    self.emit();
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                        _ = &mut restart => {}
                    }
                }
            }
            self.receiver.lock().unwrap().listening = false;
        }
    }

    fn on_osc(self: &Arc<Self>, packet: &[u8]) {
        let messages = osc::decode(packet);
        if messages.is_empty() {
            return;
        }
        let (trigger, scan_enabled) = {
            let cfg = self.cfg.read().unwrap();
            (format!("{PARAM_PREFIX}{}", cfg.scan_param), cfg.scan_param_enabled)
        };
        let mut start_scan = false;
        {
            let now = Instant::now();
            let mut receiver = self.receiver.lock().unwrap();
            for m in &messages {
                receiver.total += 1;
                if !receiver.entries.contains_key(&m.addr) && receiver.entries.len() >= MONITOR_MAX {
                    let oldest = receiver
                        .entries
                        .iter()
                        .min_by_key(|(_, e)| e.last)
                        .map(|(a, _)| a.clone());
                    if let Some(oldest) = oldest {
                        receiver.entries.remove(&oldest);
                    }
                }
                let entry = receiver.entries.entry(m.addr.clone()).or_insert(MonitorEntry {
                    value: String::new(),
                    count: 0,
                    last: now,
                });
                entry.value = osc::format_args(&m.args);
                entry.count += 1;
                entry.last = now;

                if m.addr == trigger {
                    let on = osc::truthy(m.args.first()).unwrap_or(false);
                    let mut scan = self.scan.lock().unwrap();
                    if on && !scan.trigger_on && scan_enabled {
                        start_scan = true;
                    }
                    scan.trigger_on = on;
                }
            }
        }
        if start_scan {
            self.scan(None);
        }
    }

    // ---- snapshot ---------------------------------------------------------

    fn snapshot(&self) -> Snapshot {
        let cfg = self.cfg();
        let now = Instant::now();
        let message = self.active_message(&cfg);
        let preview = message.clone().unwrap_or_else(|| self.render_template(&cfg));

        let hr = {
            let hr = self.hr.lock().unwrap();
            HrSnapshot {
                bpm: hr.bpm,
                connected: hr.connections > 0,
                device: hr.device.clone(),
                trend: hr.trend(now),
                min: hr.min,
                max: hr.max,
                avg: hr.avg(),
                sample_age_ms: hr.last_sample.map(|t| t.elapsed().as_millis() as u64),
            }
        };
        let chatbox = {
            let chat = self.chat.lock().unwrap();
            ChatboxSnapshot {
                preview,
                enabled: cfg.chatbox_enabled,
                sent_count: chat.sent_count,
                last_sent_ms_ago: chat.last_sent.map(|t| t.elapsed().as_millis() as u64),
                message_remaining_s: chat
                    .message
                    .as_ref()
                    .map(|m| m.remaining(&cfg).map_or(0, |r| r.as_secs())),
                message_kept: chat
                    .message
                    .as_ref()
                    .is_some_and(|m| m.remaining(&cfg).is_none()),
                error: chat.error.clone(),
            }
        };
        let media = self.media_now();
        let (load, addresses) = {
            let env = self.env.lock().unwrap();
            (env.load, env.addresses.clone())
        };
        let lyrics = self.lyrics.lock().unwrap().status;
        let receiver = {
            let r = self.receiver.lock().unwrap();
            ReceiverSnapshot {
                enabled: cfg.osc_receive,
                port: cfg.osc_receive_port,
                listening: r.listening,
                error: r.error.clone(),
                total: r.total,
            }
        };
        let scanner = {
            let scan = self.scan.lock().unwrap();
            ScannerSnapshot {
                scanning: scan.busy_until.is_some_and(|t| now < t),
                results: scan.results.iter().cloned().collect(),
            }
        };
        let network = {
            let server = self.server.lock().unwrap();
            NetworkSnapshot {
                host_name: self.host_name.clone(),
                addresses,
                hr_port: cfg.hr_port,
                hr_listening: server.listening,
                discoverable: server.discoverable,
                hr_error: server.error.clone(),
            }
        };
        Snapshot {
            hr,
            chatbox,
            media,
            lyrics,
            cpu: load.cpu,
            ram: load.ram,
            network,
            receiver,
            scanner,
        }
    }
}
