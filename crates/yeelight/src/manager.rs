use crate::capabilities::Capabilities;
use crate::connection::{
    run_device, ColorUpdate, ConnectionState, DeviceCommand, DeviceEvent, FailReason, LinkConfig,
};
use crate::discovery::{Discovery, DiscoveryConfig};
use crate::discovery_msg::DISCOVERY_PORT;
use crate::lan::is_lan_ipv4;
use crate::props::{DeviceId, LightState};
use crate::DiscoveryInfo;
use async_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, warn};

#[derive(Clone, Debug)]
pub struct ManagerConfig {
    pub searches_per_scan: u32,
    pub search_gap: Duration,
    pub listen_adverts: bool,
    pub discovery_port: u16,
    pub extra_search_targets: Vec<SocketAddr>,
    pub connect_timeout: Duration,
    pub command_timeout: Duration,
    pub min_slider_interval: Duration,
    pub max_commands_per_minute: usize,
    pub stuck_after: Duration,
    pub quota_pause: Duration,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            searches_per_scan: 3,
            search_gap: Duration::from_secs(1),
            listen_adverts: true,
            discovery_port: DISCOVERY_PORT,
            extra_search_targets: Vec::new(),
            connect_timeout: Duration::from_secs(3),
            command_timeout: Duration::from_secs(5),
            min_slider_interval: Duration::from_millis(250),
            max_commands_per_minute: 50,
            stuck_after: Duration::from_secs(120),
            quota_pause: Duration::from_secs(15),
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
        }
    }
}

impl ManagerConfig {
    fn link(&self) -> LinkConfig {
        LinkConfig {
            connect_timeout: self.connect_timeout,
            command_timeout: self.command_timeout,
            min_slider_interval: self.min_slider_interval,
            max_commands_per_minute: self.max_commands_per_minute,
            stuck_after: self.stuck_after,
            quota_pause: self.quota_pause,
            initial_backoff: self.initial_backoff,
            max_backoff: self.max_backoff,
        }
    }

    fn discovery(&self) -> DiscoveryConfig {
        DiscoveryConfig {
            listen_adverts: self.listen_adverts,
            port: self.discovery_port,
            extra_targets: self.extra_search_targets.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum BackendCommand {
    Scan,
    Track {
        id: DeviceId,
        addr: SocketAddr,
        support: Vec<String>,
        model: String,
    },
    SetPower {
        id: DeviceId,
        on: bool,
    },
    SetBrightness {
        id: DeviceId,
        brightness: u8,
    },
    SetColor {
        id: DeviceId,
        color: ColorUpdate,
    },
    Forget {
        id: DeviceId,
    },
    Shutdown,
}

#[derive(Clone, Debug)]
pub enum BackendEvent {
    DeviceSeen(DiscoveryInfo),
    ConnectionChanged {
        id: DeviceId,
        state: ConnectionState,
    },
    StateChanged {
        id: DeviceId,
        state: LightState,
    },
    CommandFailed {
        id: DeviceId,
        reason: FailReason,
    },
    ScanFinished,
}

struct Running {
    tx: mpsc::Sender<DeviceCommand>,
    addr: SocketAddr,
}

/// Runs until `Shutdown` or until the command channel closes.
pub async fn run(
    commands: Receiver<BackendCommand>,
    events: Sender<BackendEvent>,
    config: ManagerConfig,
) {
    let (packet_tx, mut packet_rx) = mpsc::channel(64);
    let (device_tx, mut device_rx) = mpsc::channel::<(DeviceId, DeviceEvent)>(128);
    let discovery = match Discovery::start(packet_tx, config.discovery()).await {
        Ok(discovery) => Some(discovery),
        Err(err) => {
            warn!("device discovery is unavailable: {err}");
            None
        }
    };

    let mut devices: HashMap<DeviceId, Running> = HashMap::new();
    let mut scanning = false;
    let (scan_done_tx, mut scan_done_rx) = mpsc::channel::<()>(2);

    loop {
        tokio::select! {
            cmd = commands.recv() => {
                match cmd {
                    Err(_) | Ok(BackendCommand::Shutdown) => break,
                    Ok(BackendCommand::Scan) => {
                        for running in devices.values() {
                            let _ = running.tx.try_send(DeviceCommand::Retry);
                        }
                        if scanning {
                            continue;
                        }
                        let Some(discovery) = discovery.clone() else {
                            let _ = events.send(BackendEvent::ScanFinished).await;
                            continue;
                        };
                        scanning = true;
                        let done = scan_done_tx.clone();
                        let times = config.searches_per_scan.max(1);
                        let gap = config.search_gap;
                        tokio::spawn(async move {
                            for index in 0..times {
                                if discovery.search().await.is_err() {
                                    break;
                                }
                                if index + 1 != times {
                                    tokio::time::sleep(gap).await;
                                }
                            }
                            tokio::time::sleep(Duration::from_millis(400)).await;
                            let _ = done.send(()).await;
                        });
                    }
                    Ok(BackendCommand::Track { id, addr, support, model }) => {
                        let caps = Capabilities::from_support(&support, &model);
                        ensure_device(&mut devices, id, addr, caps, &device_tx, &config);
                    }
                    Ok(BackendCommand::SetPower { id, on }) => {
                        send_device(&devices, &id, DeviceCommand::SetPower(on));
                    }
                    Ok(BackendCommand::SetBrightness { id, brightness }) => {
                        send_device(&devices, &id, DeviceCommand::SetBrightness(brightness));
                    }
                    Ok(BackendCommand::SetColor { id, color }) => {
                        send_device(&devices, &id, DeviceCommand::SetColor(color));
                    }
                    Ok(BackendCommand::Forget { id }) => {
                        if let Some(running) = devices.remove(&id) {
                            let _ = running.tx.try_send(DeviceCommand::Shutdown);
                        }
                    }
                }
            }
            Some(()) = scan_done_rx.recv() => {
                scanning = false;
                let _ = events.send(BackendEvent::ScanFinished).await;
            }
            Some(packet) = packet_rx.recv() => {
                let info = packet.info;
                let caps = Capabilities::from_support(&info.support, &info.model);
                let _ = events.send(BackendEvent::DeviceSeen(info.clone())).await;
                let _ = events.send(BackendEvent::StateChanged {
                    id: info.id.clone(),
                    state: state_from_discovery(&info),
                }).await;
                ensure_device(
                    &mut devices,
                    info.id.clone(),
                    info.location,
                    caps.clone(),
                    &device_tx,
                    &config,
                );
                send_device(&devices, &info.id, DeviceCommand::SetCapabilities(caps));
            }
            Some((id, event)) = device_rx.recv() => {
                let mapped = match event {
                    DeviceEvent::Connection(state) => BackendEvent::ConnectionChanged { id, state },
                    DeviceEvent::State(state) => BackendEvent::StateChanged { id, state },
                    DeviceEvent::Failed(reason) => BackendEvent::CommandFailed { id, reason },
                };
                if events.send(mapped).await.is_err() {
                    break;
                }
            }
        }
    }

    if let Some(discovery) = discovery {
        discovery.shutdown().await;
    }
    for (_, running) in devices {
        let _ = running.tx.try_send(DeviceCommand::Shutdown);
    }
}

fn send_device(devices: &HashMap<DeviceId, Running>, id: &DeviceId, command: DeviceCommand) {
    if let Some(running) = devices.get(id) {
        if running.tx.try_send(command).is_err() {
            debug!("command for {id} was dropped because the device task is busy");
        }
    }
}

fn ensure_device(
    devices: &mut HashMap<DeviceId, Running>,
    id: DeviceId,
    addr: SocketAddr,
    caps: Capabilities,
    device_tx: &mpsc::Sender<(DeviceId, DeviceEvent)>,
    config: &ManagerConfig,
) {
    if !addr_is_lan(addr) {
        warn!("refusing to connect to {addr}, which is outside the local network");
        return;
    }
    if let Some(existing) = devices.get_mut(&id) {
        if existing.addr != addr {
            existing.addr = addr;
            let _ = existing.tx.try_send(DeviceCommand::Relocate(addr));
        }
        let _ = existing.tx.try_send(DeviceCommand::SetCapabilities(caps));
        return;
    }

    let (tx, rx) = mpsc::channel(32);
    let bridge = device_tx.clone();
    let ident = id.clone();
    let limits = config.link();
    tokio::spawn(async move {
        let (event_tx, mut event_rx) = mpsc::channel(32);
        tokio::spawn(async move {
            run_device(addr, caps, rx, event_tx, limits).await;
        });
        while let Some(event) = event_rx.recv().await {
            if bridge.send((ident.clone(), event)).await.is_err() {
                break;
            }
        }
    });
    devices.insert(id, Running { tx, addr });
}

fn addr_is_lan(addr: SocketAddr) -> bool {
    match addr {
        SocketAddr::V4(addr) => is_lan_ipv4(*addr.ip()),
        SocketAddr::V6(_) => false,
    }
}

fn state_from_discovery(info: &DiscoveryInfo) -> LightState {
    LightState {
        power: info.power,
        bright: info.bright,
        color_mode: info.color_mode,
        ct: info.ct,
        rgb: info.rgb,
        hue: info.hue,
        sat: info.sat,
        name: if info.name.is_empty() {
            None
        } else {
            Some(info.name.clone())
        },
        flowing: None,
    }
}
