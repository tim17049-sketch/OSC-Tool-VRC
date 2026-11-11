use crate::config::{Config, MIN_INTERVAL_MS};
use crate::hr::{self, Advertiser, HrEvent, HrState};
use crate::media::{self, Media};
use crate::osc::{self, clamp_chatbox};
use crate::stats::{self, Load, Stats};
use crate::template::{self, Values};
use crate::vr::{Batteries, Battery, SteamVr};
use rosc::OscType;
use serde::Serialize;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{broadcast, Notify};
use tokio::task::JoinHandle;

/// State of the whole tool as shown by the UI.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub hr: HrSnapshot,
    pub chatbox: ChatboxSnapshot,
    pub media: Option<Media>,
    pub cpu: f32,
    pub ram: f32,
    pub cpu_temp: Option<f32>,
    pub gpu: Option<f32>,
    pub gpu_temp: Option<f32>,
    /// Battery levels reported by SteamVR.
    pub vr: Batteries,
    pub network: NetworkSnapshot,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HrSnapshot {
    pub bpm: Option<u16>,
    /// A watch (or another sender) is connected.
    pub connected: bool,
    pub device: Option<String>,
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
    until: Instant,
    sound_pending: bool,
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
    load: Load,
    vr: Batteries,
    addresses: Vec<String>,
}

struct Inner {
    cfg: RwLock<Config>,
    hr: Mutex<HrState>,
    chat: Mutex<ChatState>,
    env: Mutex<EnvState>,
    server: Mutex<ServerState>,
    socket: UdpSocket,
    /// Runtime the engine lives on; lets non-async callers start work.
    rt: tokio::runtime::Handle,
    host_name: String,
    wake_chat: Notify,
    restart_server: Notify,
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
            socket,
            rt: tokio::runtime::Handle::current(),
            host_name: stats::host_name(),
            wake_chat: Notify::new(),
            restart_server: Notify::new(),
            events,
        });
        let tasks = vec![
            tokio::spawn(inner.clone().run_server()),
            tokio::spawn(inner.clone().run_chatbox()),
            tokio::spawn(inner.clone().run_beat()),
            tokio::spawn(inner.clone().run_watchdog()),
            tokio::spawn(inner.clone().run_environment()),
        ];
        Ok(Self { inner, tasks })
    }

    pub fn config(&self) -> Config {
        self.inner.cfg()
    }

    pub fn set_config(&self, cfg: Config) -> Config {
        let cfg = cfg.sanitized();
        let port_changed = {
            let mut current = self.inner.cfg.write().unwrap();
            let changed = current.hr_port != cfg.hr_port;
            *current = cfg.clone();
            changed
        };
        if port_changed {
            self.inner.restart_server.notify_one();
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
        let text = clamp_chatbox(text.trim());
        let seconds = self.inner.cfg().message_seconds;
        {
            let mut chat = self.inner.chat.lock().unwrap();
            chat.message = (!text.is_empty()).then(|| Message {
                text,
                until: Instant::now() + Duration::from_secs(seconds),
                sound_pending: true,
            });
        }
        self.inner.wake_chat.notify_one();
        self.inner.emit();
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
                    let serve = hr::serve(listener, self.host_name.clone(), move |event| {
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
                HrEvent::Hello { device } => hr.device = device,
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
            let env = self.env.lock().unwrap();
            v.insert("song", env.media.as_ref().map(Media::display));
            v.insert("title", env.media.as_ref().map(|m| m.title.clone()));
            v.insert(
                "artist",
                env.media
                    .as_ref()
                    .map(|m| m.artist.clone())
                    .filter(|a| !a.trim().is_empty()),
            );
            v.insert("cpu", Some(format!("{:.0}%", env.load.cpu)));
            v.insert("ram", Some(format!("{:.0}%", env.load.ram)));
            v.insert("cpu_temp", env.load.cpu_temp.map(|t| format!("{t:.0}°C")));
            v.insert("gpu", env.load.gpu.map(|g| format!("{g:.0}%")));
            v.insert("gpu_temp", env.load.gpu_temp.map(|t| format!("{t:.0}°C")));
            v.insert("hmd_bat", env.vr.hmd.as_ref().map(Battery::display));
            v.insert("left_bat", env.vr.left.as_ref().map(Battery::display));
            v.insert("right_bat", env.vr.right.as_ref().map(Battery::display));
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

    /// The manually sent message, as long as it is still showing.
    fn active_message(&self) -> Option<String> {
        let mut chat = self.chat.lock().unwrap();
        if chat.message.as_ref().is_some_and(|m| Instant::now() >= m.until) {
            chat.message = None;
        }
        chat.message.as_ref().map(|m| m.text.clone())
    }

    async fn run_chatbox(self: Arc<Self>) {
        loop {
            let cfg = self.cfg();
            let interval = Duration::from_millis(cfg.interval_ms.max(MIN_INTERVAL_MS));

            let message = self.active_message();
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
                        .map(|m| std::mem::take(&mut m.sound_pending))
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
                tokio::time::sleep(interval).await;
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
        let mut steamvr = Some(SteamVr::default());
        let mut round = 0u32;
        loop {
            let load = stats.sample();
            // SteamVR and the media controls may block for a moment.
            let mut vr = steamvr.take().unwrap_or_default();
            let (vr, batteries, media) = tokio::task::spawn_blocking(move || {
                let batteries = vr.sample();
                (vr, batteries, media::now_playing())
            })
            .await
            .unwrap_or_default();
            steamvr = Some(vr);
            {
                let mut env = self.env.lock().unwrap();
                env.load = load;
                env.vr = batteries;
                env.media = media;
                if round % 5 == 0 {
                    env.addresses = hr::local_addresses();
                }
            }
            round = round.wrapping_add(1);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    // ---- snapshot ---------------------------------------------------------

    fn snapshot(&self) -> Snapshot {
        let cfg = self.cfg();
        let now = Instant::now();
        let message = self.active_message();
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
                    .map(|m| m.until.saturating_duration_since(now).as_secs()),
                error: chat.error.clone(),
            }
        };
        let (media, load, vr, addresses) = {
            let env = self.env.lock().unwrap();
            (env.media.clone(), env.load, env.vr, env.addresses.clone())
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
            cpu: load.cpu,
            ram: load.ram,
            cpu_temp: load.cpu_temp,
            gpu: load.gpu,
            gpu_temp: load.gpu_temp,
            vr,
            network,
        }
    }
}
