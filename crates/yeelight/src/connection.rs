use crate::capabilities::Capabilities;
use crate::color::{hs_to_rgb, rgb_to_hs, Rgb};
use crate::error::Error;
use crate::props::LightState;
use crate::protocol::{encode_command, parse_incoming, Command, Incoming};
use crate::scheduler::{channel_of, Channel, PollAction, Scheduler};
use socket2::TcpKeepalive;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, error::TryRecvError};
use tokio::time::{timeout, Instant};
use tracing::debug;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnavailableReason {
    Timeout,
    Refused,
    Network,
    Closed,
    StuckQuota,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorUpdate {
    Hsv { hue: u16, sat: u8 },
    Rgb(u32),
}

#[derive(Clone, Debug)]
pub enum DeviceCommand {
    SetPower(bool),
    SetBrightness(u8),
    SetColor(ColorUpdate),
    SetCapabilities(Capabilities),
    Relocate(SocketAddr),
    Retry,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Online,
    Unavailable(UnavailableReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailReason {
    Rejected(String),
    Unsupported,
    Timeout,
}

#[derive(Clone, Debug)]
pub enum DeviceEvent {
    Connection(ConnectionState),
    State(LightState),
    Failed(FailReason),
}

#[derive(Clone, Debug)]
pub struct LinkConfig {
    pub connect_timeout: Duration,
    pub command_timeout: Duration,
    pub min_slider_interval: Duration,
    pub max_commands_per_minute: usize,
    pub stuck_after: Duration,
    pub quota_pause: Duration,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for LinkConfig {
    fn default() -> Self {
        Self {
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

enum ServeEnd {
    Shutdown,
    Relocate(SocketAddr),
    Stuck,
    Lost(UnavailableReason),
}

struct Inflight {
    id: u32,
    channel: Option<Channel>,
    command: Command,
    started: Instant,
}

/// Keep one TCP session alive, reconnecting with backoff until shut down.
pub async fn run_device(
    mut addr: SocketAddr,
    mut caps: Capabilities,
    mut commands: mpsc::Receiver<DeviceCommand>,
    events: mpsc::Sender<DeviceEvent>,
    limits: LinkConfig,
) {
    let mut backoff = limits.initial_backoff;
    let mut scheduler = Scheduler::new(limits.min_slider_interval, limits.max_commands_per_minute);
    scheduler.stuck_after = limits.stuck_after;
    scheduler.quota_pause = limits.quota_pause;
    let mut next_id = 1u32;

    loop {
        let _ = events
            .send(DeviceEvent::Connection(ConnectionState::Connecting))
            .await;
        match connect_and_serve(
            addr,
            &mut caps,
            &mut commands,
            &events,
            &limits,
            &mut scheduler,
            &mut next_id,
        )
        .await
        {
            ServeEnd::Shutdown => break,
            ServeEnd::Relocate(new_addr) => {
                addr = new_addr;
                backoff = limits.initial_backoff;
            }
            ServeEnd::Stuck => {
                let _ = events
                    .send(DeviceEvent::Connection(ConnectionState::Unavailable(
                        UnavailableReason::StuckQuota,
                    )))
                    .await;
                if !wait_for_retry(&mut commands, &mut addr, &mut caps, &mut scheduler, &events)
                    .await
                {
                    break;
                }
                backoff = limits.initial_backoff;
            }
            ServeEnd::Lost(reason) => {
                let _ = events
                    .send(DeviceEvent::Connection(ConnectionState::Unavailable(
                        reason,
                    )))
                    .await;
                if !wait_backoff(
                    &mut commands,
                    &mut addr,
                    &mut caps,
                    &mut scheduler,
                    &events,
                    backoff,
                )
                .await
                {
                    break;
                }
                backoff = (backoff * 2).min(limits.max_backoff);
            }
        }
    }
}

async fn wait_backoff(
    commands: &mut mpsc::Receiver<DeviceCommand>,
    addr: &mut SocketAddr,
    caps: &mut Capabilities,
    scheduler: &mut Scheduler,
    events: &mpsc::Sender<DeviceEvent>,
    backoff: Duration,
) -> bool {
    let sleep = tokio::time::sleep(backoff);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            _ = &mut sleep => return true,
            cmd = commands.recv() => {
                let immediate = matches!(cmd, Some(DeviceCommand::Retry | DeviceCommand::Relocate(_)));
                if !dispatch_waiting(cmd, addr, caps, scheduler, events).await {
                    return false;
                }
                if immediate {
                    return true;
                }
            }
        }
    }
}

async fn wait_for_retry(
    commands: &mut mpsc::Receiver<DeviceCommand>,
    addr: &mut SocketAddr,
    caps: &mut Capabilities,
    scheduler: &mut Scheduler,
    events: &mpsc::Sender<DeviceEvent>,
) -> bool {
    loop {
        let cmd = commands.recv().await;
        let resume = matches!(cmd, Some(DeviceCommand::Retry | DeviceCommand::Relocate(_)));
        let relocate = matches!(cmd, Some(DeviceCommand::Relocate(_)));
        if !dispatch_waiting(cmd, addr, caps, scheduler, events).await {
            return false;
        }
        if resume || relocate {
            return true;
        }
    }
}

/// Returns false when the device task should stop.
async fn dispatch_waiting(
    cmd: Option<DeviceCommand>,
    addr: &mut SocketAddr,
    caps: &mut Capabilities,
    scheduler: &mut Scheduler,
    events: &mpsc::Sender<DeviceEvent>,
) -> bool {
    match cmd {
        None | Some(DeviceCommand::Shutdown) => false,
        Some(DeviceCommand::Relocate(new_addr)) => {
            *addr = new_addr;
            true
        }
        Some(DeviceCommand::Retry) => true,
        Some(other) => {
            let _ = apply_user_command(other, caps, scheduler, events).await;
            true
        }
    }
}

async fn connect_and_serve(
    addr: SocketAddr,
    caps: &mut Capabilities,
    commands: &mut mpsc::Receiver<DeviceCommand>,
    events: &mpsc::Sender<DeviceEvent>,
    limits: &LinkConfig,
    scheduler: &mut Scheduler,
    next_id: &mut u32,
) -> ServeEnd {
    let stream = match timeout(limits.connect_timeout, TcpStream::connect(addr)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) if err.kind() == ErrorKind::ConnectionRefused => {
            return ServeEnd::Lost(UnavailableReason::Refused);
        }
        Ok(Err(err)) => {
            debug!("connect to {addr} failed: {err}");
            return ServeEnd::Lost(UnavailableReason::Network);
        }
        Err(_) => return ServeEnd::Lost(UnavailableReason::Timeout),
    };
    tune(&stream);
    debug!("connected to {addr}");

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut inflight: Option<Inflight> = None;
    let mut online = false;

    if let Err(end) = send_command(
        &mut writer,
        &mut inflight,
        next_id,
        Command::sync(),
        None,
        scheduler,
        true,
    )
    .await
    {
        return end;
    }

    loop {
        loop {
            match commands.try_recv() {
                Ok(cmd) => match on_command(cmd, caps, scheduler, events).await {
                    CommandEffect::Continue => {}
                    CommandEffect::Stop => return ServeEnd::Shutdown,
                    CommandEffect::Relocate(new_addr) => return ServeEnd::Relocate(new_addr),
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return ServeEnd::Shutdown,
            }
        }

        if inflight.is_none() {
            if let PollAction::Send(channel, command) = scheduler.poll(Instant::now()) {
                if let Err(end) = send_command(
                    &mut writer,
                    &mut inflight,
                    next_id,
                    command,
                    Some(channel),
                    scheduler,
                    false,
                )
                .await
                {
                    return end;
                }
                continue;
            }
        }
        let wait = if let Some(current) = inflight.as_ref() {
            let deadline = current.started + limits.command_timeout;
            let now = Instant::now();
            if now >= deadline {
                debug!("command to {addr} timed out");
                return ServeEnd::Lost(UnavailableReason::Timeout);
            }
            deadline.saturating_duration_since(now)
        } else {
            match scheduler.poll(Instant::now()) {
                PollAction::Wait(duration) => duration,
                PollAction::Send(_, _) | PollAction::Idle => Duration::from_secs(3600),
            }
        };

        tokio::select! {
            read = reader.read_line(&mut line) => {
                match read {
                    Ok(0) => return ServeEnd::Lost(UnavailableReason::Closed),
                    Ok(_) => {
                        let text = std::mem::take(&mut line);
                        match handle_line(&text, &mut inflight, scheduler, events, &mut online).await {
                            LineEffect::Continue => {}
                            LineEffect::Stuck => return ServeEnd::Stuck,
                        }
                    }
                    Err(err) => {
                        debug!("read from {addr} failed: {err}");
                        return ServeEnd::Lost(UnavailableReason::Network);
                    }
                }
            }
            cmd = commands.recv() => {
                match cmd {
                    None => return ServeEnd::Shutdown,
                    Some(cmd) => match on_command(cmd, caps, scheduler, events).await {
                        CommandEffect::Continue => {}
                        CommandEffect::Stop => return ServeEnd::Shutdown,
                        CommandEffect::Relocate(new_addr) => return ServeEnd::Relocate(new_addr),
                    },
                }
            }
            _ = tokio::time::sleep(wait) => {}
        }
    }
}

enum CommandEffect {
    Continue,
    Stop,
    Relocate(SocketAddr),
}

async fn on_command(
    cmd: DeviceCommand,
    caps: &mut Capabilities,
    scheduler: &mut Scheduler,
    events: &mpsc::Sender<DeviceEvent>,
) -> CommandEffect {
    match cmd {
        DeviceCommand::Shutdown => CommandEffect::Stop,
        DeviceCommand::Relocate(addr) => CommandEffect::Relocate(addr),
        DeviceCommand::Retry => CommandEffect::Continue,
        DeviceCommand::SetCapabilities(next) => {
            *caps = next;
            CommandEffect::Continue
        }
        other => {
            let _ = apply_user_command(other, caps, scheduler, events).await;
            CommandEffect::Continue
        }
    }
}

async fn apply_user_command(
    cmd: DeviceCommand,
    caps: &Capabilities,
    scheduler: &mut Scheduler,
    events: &mpsc::Sender<DeviceEvent>,
) -> bool {
    let built = match cmd {
        DeviceCommand::SetPower(on) if caps.power => Some((Channel::Power, Command::set_power(on))),
        DeviceCommand::SetBrightness(value) if caps.brightness => {
            Some((Channel::Brightness, Command::set_bright(value)))
        }
        DeviceCommand::SetColor(update) => color_command(update, caps)
            .map(|command| (channel_of(&command).unwrap_or(Channel::Color), command)),
        DeviceCommand::SetPower(_) | DeviceCommand::SetBrightness(_) => None,
        _ => return true,
    };
    let Some((channel, command)) = built else {
        let _ = events
            .send(DeviceEvent::Failed(FailReason::Unsupported))
            .await;
        return false;
    };
    scheduler.push(channel, command, Instant::now());
    true
}

fn color_command(update: ColorUpdate, caps: &Capabilities) -> Option<Command> {
    match update {
        ColorUpdate::Hsv { hue, sat } if caps.hsv => Some(Command::set_hsv(hue, sat)),
        ColorUpdate::Hsv { hue, sat } if caps.rgb => {
            Some(Command::set_rgb(hs_to_rgb(hue, sat).to_u32()))
        }
        ColorUpdate::Rgb(rgb) if caps.rgb => Some(Command::set_rgb(rgb)),
        ColorUpdate::Rgb(rgb) if caps.hsv => {
            let (hue, sat) = rgb_to_hs(Rgb::from_u32(rgb));
            Some(Command::set_hsv(hue, sat))
        }
        _ => None,
    }
}

enum LineEffect {
    Continue,
    Stuck,
}

async fn handle_line(
    line: &str,
    inflight: &mut Option<Inflight>,
    scheduler: &mut Scheduler,
    events: &mpsc::Sender<DeviceEvent>,
    online: &mut bool,
) -> LineEffect {
    let incoming = match parse_incoming(line) {
        Ok(incoming) => incoming,
        Err(err) => {
            debug!("ignored bulb message: {err}");
            return LineEffect::Continue;
        }
    };
    match incoming {
        Incoming::Notification(state) => {
            let _ = events.send(DeviceEvent::State(state)).await;
            LineEffect::Continue
        }
        Incoming::Result { id, result } => {
            let Some(current) = inflight.take() else {
                return LineEffect::Continue;
            };
            if current.id != id {
                debug!("result id {id} did not match in-flight {}", current.id);
                *inflight = Some(current);
                return LineEffect::Continue;
            }
            let is_sync = matches!(current.command, Command::GetProp { .. });
            match result {
                Ok(values) => {
                    scheduler.on_success();
                    if is_sync {
                        if let Some(names) = current.command.prop_names() {
                            let mut state = LightState::default();
                            state.apply_values(names, &values);
                            let _ = events.send(DeviceEvent::State(state)).await;
                        }
                    }
                    announce_online(events, online).await;
                    LineEffect::Continue
                }
                Err(device_error) => {
                    if device_error.is_quota() {
                        let now = Instant::now();
                        scheduler.on_quota_exceeded(now);
                        if let Some(channel) = current.channel {
                            scheduler.requeue(channel, current.command, now);
                        }
                        if scheduler.is_stuck(now) {
                            return LineEffect::Stuck;
                        }
                        announce_online(events, online).await;
                        return LineEffect::Continue;
                    }
                    let _ = events
                        .send(DeviceEvent::Failed(FailReason::Rejected(
                            device_error.message,
                        )))
                        .await;
                    announce_online(events, online).await;
                    LineEffect::Continue
                }
            }
        }
    }
}

async fn announce_online(events: &mpsc::Sender<DeviceEvent>, online: &mut bool) {
    if !*online {
        let _ = events
            .send(DeviceEvent::Connection(ConnectionState::Online))
            .await;
        *online = true;
    }
}

async fn send_command(
    writer: &mut OwnedWriteHalf,
    inflight: &mut Option<Inflight>,
    next_id: &mut u32,
    command: Command,
    channel: Option<Channel>,
    scheduler: &mut Scheduler,
    count_quota: bool,
) -> Result<(), ServeEnd> {
    let id = *next_id;
    *next_id = next_id.wrapping_add(1).max(1);
    let bytes = encode_command(id, &command);
    debug!("send {bytes}");
    if writer.write_all(bytes.as_bytes()).await.is_err() {
        return Err(ServeEnd::Lost(UnavailableReason::Network));
    }
    if count_quota {
        scheduler.note_sent(Instant::now());
    }
    *inflight = Some(Inflight {
        id,
        channel,
        command,
        started: Instant::now(),
    });
    Ok(())
}

fn tune(stream: &TcpStream) {
    let _ = stream.set_nodelay(true);
    let socket = socket2::SockRef::from(stream);
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(30))
        .with_interval(Duration::from_secs(10));
    let _ = socket.set_keepalive(true);
    let _ = socket.set_tcp_keepalive(&keepalive);
}

/// Open a short-lived connection and exchange one command. Used by the control example.
pub async fn transact(addr: SocketAddr, command: Command) -> Result<String, Error> {
    let mut stream = timeout(Duration::from_secs(3), TcpStream::connect(addr))
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(Error::Io)?;
    tune(&stream);
    stream
        .write_all(encode_command(1, &command).as_bytes())
        .await?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let read = timeout(Duration::from_secs(5), reader.read_line(&mut line))
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(Error::Io)?;
    if read == 0 {
        return Err(Error::Closed);
    }
    Ok(line)
}
