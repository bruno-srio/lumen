//! Virtual Yeelight bulbs that speak the real LAN protocol.
//!
//! The simulator answers unicast `M-SEARCH` on its discovery port and, when
//! asked, also joins the Yeelight multicast group. Each bulb accepts the same
//! JSON commands as hardware, including the four-connection and per-minute
//! command limits.

use serde_json::Value;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, watch, Mutex};
use yeelight::{
    encode_error, encode_notification, encode_ok, encode_result_strings, format_notify,
    format_search_response, parse_client_message, ColorMode, DeviceId, DiscoveryInfo, LightState,
    DISCOVERY_PORT, MULTICAST_ADDR,
};

const MAX_CONNECTIONS: usize = 4;
const COMMANDS_PER_MINUTE: usize = 60;

#[derive(Clone, Debug)]
pub struct BulbSpec {
    pub id: DeviceId,
    pub model: String,
    pub fw_ver: String,
    pub support: Vec<String>,
    pub name: String,
    pub tcp_port: u16,
    pub power: bool,
    pub bright: u8,
    pub color_mode: u8,
    pub ct: u32,
    pub rgb: u32,
    pub hue: u16,
    pub sat: u8,
}

impl BulbSpec {
    pub fn color(id: &str, name: &str) -> Self {
        Self::new(
            id,
            "color",
            name,
            &[
                "get_prop",
                "set_default",
                "set_power",
                "toggle",
                "set_bright",
                "set_rgb",
                "set_hsv",
                "set_ct_abx",
                "set_name",
                "start_cf",
                "stop_cf",
                "cron_add",
                "cron_del",
                "cron_get",
            ],
        )
    }

    pub fn mono(id: &str, name: &str) -> Self {
        Self::new(
            id,
            "mono",
            name,
            &["get_prop", "set_power", "toggle", "set_bright", "set_name"],
        )
    }

    pub fn color_temperature(id: &str, name: &str) -> Self {
        Self::new(
            id,
            "ct_bulb",
            name,
            &[
                "get_prop",
                "set_power",
                "toggle",
                "set_bright",
                "set_ct_abx",
                "set_name",
                "start_cf",
                "stop_cf",
                "cron_add",
                "cron_del",
                "cron_get",
            ],
        )
    }

    fn new(id: &str, model: &str, name: &str, support: &[&str]) -> Self {
        Self {
            id: DeviceId::new(id),
            model: model.to_string(),
            fw_ver: "18".to_string(),
            support: support.iter().map(|method| (*method).to_string()).collect(),
            name: name.to_string(),
            tcp_port: 0,
            power: true,
            bright: 80,
            color_mode: if model == "color" { 1 } else { 2 },
            ct: 4000,
            rgb: 16_711_680,
            hue: 0,
            sat: if model == "color" { 100 } else { 0 },
        }
    }
}

pub fn default_bulbs() -> Vec<BulbSpec> {
    vec![
        BulbSpec::color("0x000000000000a001", "Desk Lamp"),
        BulbSpec::mono("0x000000000000a002", "Hall Light"),
        BulbSpec::color_temperature("0x000000000000a003", "Bedroom"),
    ]
}

#[derive(Clone, Debug)]
pub struct SimConfig {
    pub discovery_port: u16,
    pub multicast: bool,
    pub bulbs: Vec<BulbSpec>,
    pub stuck_quota: bool,
    pub latency: Duration,
    pub drop_after: Option<u64>,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            discovery_port: DISCOVERY_PORT,
            multicast: true,
            bulbs: default_bulbs(),
            stuck_quota: false,
            latency: Duration::ZERO,
            drop_after: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulbSnapshot {
    pub power: bool,
    pub bright: u8,
    pub color_mode: u8,
    pub ct: u32,
    pub rgb: u32,
    pub hue: u16,
    pub sat: u8,
    pub name: String,
    pub flowing: bool,
    pub delayoff: u32,
    pub tcp_addr: SocketAddr,
    pub applied_commands: u64,
}

#[derive(Clone)]
pub struct SimBulb {
    inner: Arc<BulbInner>,
}

struct BulbInner {
    spec: BulbSpec,
    state: std::sync::Mutex<LiveState>,
    notes: Mutex<Vec<mpsc::UnboundedSender<String>>>,
    accepting: watch::Sender<bool>,
    kick: watch::Sender<u64>,
    rebind: watch::Sender<u64>,
    tcp_addr: watch::Sender<SocketAddr>,
    connections: AtomicUsize,
    applied: AtomicU64,
    received: AtomicU64,
    stuck_quota: AtomicBool,
    latency: Duration,
    drop_after: Option<u64>,
}

#[derive(Clone)]
struct LiveState {
    power: bool,
    bright: u8,
    color_mode: u8,
    ct: u32,
    rgb: u32,
    hue: u16,
    sat: u8,
    name: String,
    flowing: bool,
    delayoff: u32,
}

pub struct SimNetwork {
    discovery_addr: SocketAddr,
    bulbs: Vec<SimBulb>,
    shutdown: watch::Sender<bool>,
}

impl SimNetwork {
    pub async fn start(config: SimConfig) -> std::io::Result<Self> {
        let (shutdown, shutdown_rx) = watch::channel(false);
        let mut bulbs = Vec::new();
        for spec in config.bulbs.clone() {
            bulbs.push(spawn_bulb(spec, &config, shutdown_rx.clone()).await?);
        }
        let discovery = bind_discovery(config.discovery_port, config.multicast).await?;
        let discovery_addr = discovery.local_addr()?;
        let bulbs_for_discovery = bulbs.clone();
        let mut shutdown_discovery = shutdown_rx;
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                tokio::select! {
                    changed = shutdown_discovery.changed() => {
                        if changed.is_err() || *shutdown_discovery.borrow() {
                            break;
                        }
                    }
                    received = discovery.recv_from(&mut buf) => {
                        let Ok((len, peer)) = received else { continue };
                        let text = String::from_utf8_lossy(&buf[..len]).to_string();
                        if !text.starts_with("M-SEARCH") {
                            continue;
                        }
                        reply_search(&discovery, peer, &bulbs_for_discovery).await;
                    }
                }
            }
        });
        if config.multicast {
            announce(&bulbs).await;
        }
        Ok(Self {
            discovery_addr,
            bulbs,
            shutdown,
        })
    }

    pub fn discovery_addr(&self) -> SocketAddr {
        self.discovery_addr
    }

    pub fn bulbs(&self) -> &[SimBulb] {
        &self.bulbs
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown.send(true);
        for bulb in &self.bulbs {
            bulb.inner.kick.send_modify(|n| *n += 1);
        }
    }
}

impl SimBulb {
    pub fn id(&self) -> &DeviceId {
        &self.inner.spec.id
    }

    pub fn model(&self) -> &str {
        &self.inner.spec.model
    }

    pub fn tcp_addr(&self) -> SocketAddr {
        *self.inner.tcp_addr.borrow()
    }

    pub async fn snapshot(&self) -> BulbSnapshot {
        let state = self.inner.state.lock().expect("bulb state").clone();
        BulbSnapshot {
            power: state.power,
            bright: state.bright,
            color_mode: state.color_mode,
            ct: state.ct,
            rgb: state.rgb,
            hue: state.hue,
            sat: state.sat,
            name: state.name,
            flowing: state.flowing,
            delayoff: state.delayoff,
            tcp_addr: self.tcp_addr(),
            applied_commands: self.inner.applied.load(Ordering::Relaxed),
        }
    }

    pub async fn set_accepting(&self, accepting: bool) {
        let _ = self.inner.accepting.send(accepting);
        if !accepting {
            self.inner.kick.send_modify(|value| *value += 1);
        }
    }

    pub async fn rebind(&self) {
        let mut watch = self.inner.tcp_addr.subscribe();
        let previous = *watch.borrow();
        self.inner.rebind.send_modify(|value| *value += 1);
        while *watch.borrow() == previous {
            if watch.changed().await.is_err() {
                break;
            }
        }
    }

    pub fn set_stuck_quota(&self, stuck: bool) {
        self.inner.stuck_quota.store(stuck, Ordering::Relaxed);
    }
}

async fn spawn_bulb(
    spec: BulbSpec,
    config: &SimConfig,
    mut shutdown: watch::Receiver<bool>,
) -> std::io::Result<SimBulb> {
    let initial_port = spec.tcp_port;
    let listener =
        TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, initial_port))).await?;
    let addr = listener.local_addr()?;
    let (accepting, mut accepting_rx) = watch::channel(true);
    let (kick, kick_rx) = watch::channel(0u64);
    let (rebind, mut rebind_rx) = watch::channel(0u64);
    let (tcp_addr, _) = watch::channel(addr);
    let inner = Arc::new(BulbInner {
        state: std::sync::Mutex::new(LiveState {
            power: spec.power,
            bright: spec.bright,
            color_mode: spec.color_mode,
            ct: spec.ct,
            rgb: spec.rgb,
            hue: spec.hue,
            sat: spec.sat,
            name: spec.name.clone(),
            flowing: false,
            delayoff: 0,
        }),
        spec,
        notes: Mutex::new(Vec::new()),
        accepting,
        kick,
        rebind,
        tcp_addr,
        connections: AtomicUsize::new(0),
        applied: AtomicU64::new(0),
        received: AtomicU64::new(0),
        stuck_quota: AtomicBool::new(config.stuck_quota),
        latency: config.latency,
        drop_after: config.drop_after,
    });
    let bulb = SimBulb {
        inner: inner.clone(),
    };
    let task_bulb = bulb.clone();
    tokio::spawn(async move {
        let mut listener = listener;
        loop {
            if *shutdown.borrow() {
                break;
            }
            if !*accepting_rx.borrow() {
                tokio::select! {
                    changed = accepting_rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                    }
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            break;
                        }
                    }
                }
                continue;
            }
            let wake = {
                tokio::select! {
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            AcceptWake::Stop
                        } else {
                            AcceptWake::Again
                        }
                    }
                    changed = accepting_rx.changed() => {
                        if changed.is_err() {
                            AcceptWake::Stop
                        } else {
                            AcceptWake::Pause
                        }
                    }
                    changed = rebind_rx.changed() => {
                        if changed.is_err() { AcceptWake::Stop } else { AcceptWake::Rebind }
                    }
                    accepted = listener.accept() => AcceptWake::Stream(accepted.ok().map(|(stream, _)| stream)),
                }
            };
            match wake {
                AcceptWake::Stop => break,
                AcceptWake::Again => {}
                AcceptWake::Pause => {
                    if *accepting_rx.borrow() {
                        continue;
                    }
                    task_bulb.inner.kick.send_modify(|value| *value += 1);
                    let port = task_bulb.tcp_addr().port();
                    drop(listener);
                    while !*accepting_rx.borrow() && !*shutdown.borrow() {
                        if accepting_rx.changed().await.is_err() {
                            return;
                        }
                    }
                    if *shutdown.borrow() {
                        break;
                    }
                    listener = bind_tcp(port).await;
                }
                AcceptWake::Rebind => {
                    task_bulb.inner.kick.send_modify(|value| *value += 1);
                    drop(listener);
                    listener = bind_tcp(0).await;
                    if let Ok(addr) = listener.local_addr() {
                        let _ = task_bulb.inner.tcp_addr.send(addr);
                    }
                }
                AcceptWake::Stream(None) => {}
                AcceptWake::Stream(Some(stream)) => {
                    if task_bulb.inner.connections.fetch_add(1, Ordering::Relaxed)
                        >= MAX_CONNECTIONS
                    {
                        task_bulb.inner.connections.fetch_sub(1, Ordering::Relaxed);
                        continue;
                    }
                    let inner = task_bulb.inner.clone();
                    let kick = kick_rx.clone();
                    tokio::spawn(async move {
                        handle_connection(stream, inner, kick).await;
                    });
                }
            }
        }
    });
    Ok(bulb)
}

enum AcceptWake {
    Stop,
    Again,
    Pause,
    Rebind,
    Stream(Option<TcpStream>),
}

async fn handle_connection(
    stream: TcpStream,
    inner: Arc<BulbInner>,
    mut kick: watch::Receiver<u64>,
) {
    let _guard = ConnectionGuard(inner.clone());
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let (note_tx, mut note_rx) = mpsc::unbounded_channel();
    inner.notes.lock().await.push(note_tx);
    let mut line = String::new();
    let mut recent: Vec<tokio::time::Instant> = Vec::new();
    loop {
        tokio::select! {
            changed = kick.changed() => {
                if changed.is_err() { break; }
                break;
            }
            note = note_rx.recv() => {
                let Some(note) = note else { break };
                if writer.write_all(note.as_bytes()).await.is_err() { break; }
            }
            read = reader.read_line(&mut line) => {
                match read {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let text = std::mem::take(&mut line);
                        if inner.latency > Duration::ZERO {
                            tokio::time::sleep(inner.latency).await;
                        }
                        let count = inner.received.fetch_add(1, Ordering::Relaxed) + 1;
                        if inner.drop_after.is_some_and(|limit| count > limit) {
                            break;
                        }
                        let now = tokio::time::Instant::now();
                        recent.retain(|sent| now.saturating_duration_since(*sent) < Duration::from_secs(60));
                        let over_quota = recent.len() >= COMMANDS_PER_MINUTE;
                        let reply = answer(&inner, &text, over_quota).await;
                        if !over_quota && !inner.stuck_quota.load(Ordering::Relaxed) {
                            recent.push(now);
                        }
                        if writer.write_all(reply.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
    inner
        .notes
        .lock()
        .await
        .retain(|sender| !sender.is_closed());
}

struct ConnectionGuard(Arc<BulbInner>);
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.connections.fetch_sub(1, Ordering::Relaxed);
    }
}

async fn answer(inner: &BulbInner, text: &str, over_quota: bool) -> String {
    let Ok(message) = parse_client_message(text) else {
        return encode_error(0, -1, "invalid command");
    };
    if inner.stuck_quota.load(Ordering::Relaxed) || over_quota {
        return encode_error(message.id, -1, "client quota exceeded");
    }
    let outcome = {
        let mut state = inner.state.lock().expect("bulb state");
        apply_command(
            &message.method,
            &message.params,
            &mut state,
            &inner.spec.support,
        )
    };
    match outcome {
        CommandOutcome::Ok => {
            inner.applied.fetch_add(1, Ordering::Relaxed);
            encode_ok(message.id)
        }
        CommandOutcome::Props(values) => encode_result_strings(message.id, &values),
        CommandOutcome::Changed(changed) => {
            inner.applied.fetch_add(1, Ordering::Relaxed);
            let notification = encode_notification(&changed);
            let mut notes = inner.notes.lock().await;
            notes.retain(|sender| sender.send(notification.clone()).is_ok());
            encode_ok(message.id)
        }
        CommandOutcome::Error(reason) => encode_error(message.id, -1, reason),
    }
}

enum CommandOutcome {
    Ok,
    Props(Vec<String>),
    Changed(LightState),
    Error(&'static str),
}

fn apply_command(
    method: &str,
    params: &[Value],
    state: &mut LiveState,
    support: &[String],
) -> CommandOutcome {
    if method != "get_prop" && !support.iter().any(|item| item == method) {
        return CommandOutcome::Error("unsupported method");
    }
    match method {
        "get_prop" => {
            let values = params
                .iter()
                .map(|value| value.as_str().unwrap_or_default())
                .map(|name| property(state, name))
                .collect();
            CommandOutcome::Props(values)
        }
        "set_power" | "toggle" => {
            let on = if method == "toggle" {
                !state.power
            } else {
                matches!(arg_str(params, 0), Some("on"))
            };
            state.power = on;
            CommandOutcome::Changed(LightState {
                power: Some(on),
                ..LightState::default()
            })
        }
        "set_bright" => {
            if !state.power {
                return CommandOutcome::Error("device is off");
            }
            let Some(value) = arg_u32(params, 0) else {
                return CommandOutcome::Error("invalid params");
            };
            state.bright = (value as u8).clamp(1, 100);
            CommandOutcome::Changed(LightState {
                bright: Some(state.bright),
                ..LightState::default()
            })
        }
        "set_rgb" => {
            if !state.power {
                return CommandOutcome::Error("device is off");
            }
            let Some(rgb) = arg_u32(params, 0) else {
                return CommandOutcome::Error("invalid params");
            };
            state.rgb = rgb & 0x00ff_ffff;
            state.color_mode = ColorMode::Rgb as u8;
            let (hue, sat) = yeelight::rgb_to_hs(yeelight::Rgb::from_u32(state.rgb));
            state.hue = hue;
            state.sat = sat;
            CommandOutcome::Changed(color_state(state))
        }
        "set_hsv" => {
            if !state.power {
                return CommandOutcome::Error("device is off");
            }
            let (Some(hue), Some(sat)) = (arg_u32(params, 0), arg_u32(params, 1)) else {
                return CommandOutcome::Error("invalid params");
            };
            state.hue = (hue as u16) % 360;
            state.sat = (sat as u8).min(100);
            state.color_mode = ColorMode::Hsv as u8;
            state.rgb = yeelight::hs_to_rgb(state.hue, state.sat).to_u32();
            CommandOutcome::Changed(color_state(state))
        }
        "set_ct_abx" => {
            if !state.power {
                return CommandOutcome::Error("device is off");
            }
            let Some(ct) = arg_u32(params, 0) else {
                return CommandOutcome::Error("invalid params");
            };
            state.ct = ct.clamp(1700, 6500);
            state.color_mode = ColorMode::ColorTemperature as u8;
            CommandOutcome::Changed(LightState {
                ct: Some(state.ct),
                color_mode: Some(ColorMode::ColorTemperature),
                ..LightState::default()
            })
        }
        "set_name" => {
            let Some(name) = arg_str(params, 0) else {
                return CommandOutcome::Error("invalid params");
            };
            state.name = name.to_string();
            CommandOutcome::Changed(LightState {
                name: Some(state.name.clone()),
                ..LightState::default()
            })
        }
        "start_cf" => {
            if !state.power {
                return CommandOutcome::Error("device is off");
            }
            state.flowing = true;
            CommandOutcome::Changed(LightState {
                flowing: Some(true),
                ..LightState::default()
            })
        }
        "stop_cf" => {
            state.flowing = false;
            CommandOutcome::Changed(LightState {
                flowing: Some(false),
                ..LightState::default()
            })
        }
        "cron_add" => {
            let minutes = arg_u32(params, 1).unwrap_or(0);
            state.delayoff = minutes;
            CommandOutcome::Changed(LightState {
                delayoff: Some(minutes),
                ..LightState::default()
            })
        }
        "cron_del" => {
            state.delayoff = 0;
            CommandOutcome::Changed(LightState {
                delayoff: Some(0),
                ..LightState::default()
            })
        }
        "cron_get" => CommandOutcome::Props(vec![state.delayoff.to_string()]),
        "set_default" => CommandOutcome::Ok,
        _ => CommandOutcome::Error("unsupported method"),
    }
}

fn color_state(state: &LiveState) -> LightState {
    LightState {
        color_mode: ColorMode::from_code(state.color_mode),
        rgb: Some(state.rgb),
        hue: Some(state.hue),
        sat: Some(state.sat),
        ..LightState::default()
    }
}

fn property(state: &LiveState, name: &str) -> String {
    match name {
        "power" => if state.power { "on" } else { "off" }.to_string(),
        "bright" => state.bright.to_string(),
        "ct" => state.ct.to_string(),
        "rgb" => state.rgb.to_string(),
        "hue" => state.hue.to_string(),
        "sat" => state.sat.to_string(),
        "color_mode" => state.color_mode.to_string(),
        "name" => state.name.clone(),
        "flowing" => if state.flowing { "1" } else { "0" }.to_string(),
        "delayoff" => state.delayoff.to_string(),
        _ => String::new(),
    }
}

fn arg_str(params: &[Value], index: usize) -> Option<&str> {
    params.get(index).and_then(Value::as_str)
}

fn arg_u32(params: &[Value], index: usize) -> Option<u32> {
    match params.get(index)? {
        Value::Number(number) => number.as_u64().map(|value| value as u32),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

async fn bind_tcp(port: u16) -> TcpListener {
    loop {
        if let Ok(listener) =
            TcpListener::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))).await
        {
            return listener;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn bind_discovery(port: u16, multicast: bool) -> std::io::Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_reuse_address(true)?;
    let _ = socket.set_reuse_port(true);
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)).into())?;
    if multicast {
        socket.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)?;
    }
    UdpSocket::from_std(socket.into())
}

async fn reply_search(socket: &UdpSocket, peer: SocketAddr, bulbs: &[SimBulb]) {
    let source_ip = route_ip(peer);
    for bulb in bulbs {
        let message = format_search_response(&discovery_info(bulb, source_ip));
        let _ = socket.send_to(message.as_bytes(), peer).await;
    }
}

async fn announce(bulbs: &[SimBulb]) {
    let Ok(socket) = bind_discovery(0, true).await else {
        return;
    };
    let target = SocketAddr::from((MULTICAST_ADDR, DISCOVERY_PORT));
    let source_ip = route_ip(target);
    for bulb in bulbs {
        let message = format_notify(&discovery_info(bulb, source_ip));
        let _ = socket.send_to(message.as_bytes(), target).await;
    }
}

fn discovery_info(bulb: &SimBulb, host: Ipv4Addr) -> DiscoveryInfo {
    let state = bulb.inner.state.lock().expect("bulb state").clone();
    let port = bulb.tcp_addr().port();
    DiscoveryInfo {
        id: bulb.inner.spec.id.clone(),
        location: SocketAddr::from((host, port)),
        model: bulb.inner.spec.model.clone(),
        fw_ver: bulb.inner.spec.fw_ver.clone(),
        support: bulb.inner.spec.support.clone(),
        power: Some(state.power),
        bright: Some(state.bright),
        color_mode: ColorMode::from_code(state.color_mode),
        ct: Some(state.ct),
        rgb: Some(state.rgb),
        hue: Some(state.hue),
        sat: Some(state.sat),
        name: state.name,
        max_age: Some(3600),
    }
}

fn route_ip(peer: SocketAddr) -> Ipv4Addr {
    if let Ok(socket) = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
        if socket.connect(peer).is_ok() {
            if let Ok(SocketAddr::V4(addr)) = socket.local_addr() {
                if !addr.ip().is_unspecified() {
                    return *addr.ip();
                }
            }
        }
    }
    Ipv4Addr::LOCALHOST
}
