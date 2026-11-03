use crate::discovery_msg::{search_request, validate_discovery, DISCOVERY_PORT, MULTICAST_ADDR};
use crate::error::Error;
use crate::lan::is_lan_ipv4;
use crate::DiscoveryInfo;
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, warn};

#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    pub listen_adverts: bool,
    pub port: u16,
    pub extra_targets: Vec<SocketAddr>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            listen_adverts: true,
            port: DISCOVERY_PORT,
            extra_targets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiscoveryPacket {
    pub info: DiscoveryInfo,
    pub source: Ipv4Addr,
}

#[derive(Clone)]
pub struct Discovery {
    search_tx: mpsc::Sender<SearchCmd>,
}

enum SearchCmd {
    Search,
    Probe(SocketAddr),
    Shutdown,
}

impl Discovery {
    pub async fn start(
        events: mpsc::Sender<DiscoveryPacket>,
        config: DiscoveryConfig,
    ) -> Result<Self, Error> {
        let search = bind_search_socket()?;
        if config.listen_adverts {
            match bind_advert_socket(config.port) {
                Ok(socket) => {
                    let events = events.clone();
                    tokio::spawn(async move {
                        read_loop(socket, events).await;
                    });
                }
                Err(err) => {
                    warn!(
                        "not listening for Yeelight advertisements on UDP {}: {err}",
                        config.port
                    );
                }
            }
        }

        let (search_tx, mut search_rx) = mpsc::channel::<SearchCmd>(8);
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                tokio::select! {
                    cmd = search_rx.recv() => {
                        match cmd {
                            Some(SearchCmd::Search) => send_searches(&search, &config).await,
                            Some(SearchCmd::Probe(target)) => {
                                let _ = search.send_to(search_request().as_bytes(), target).await;
                            }
                            Some(SearchCmd::Shutdown) | None => break,
                        }
                    }
                    received = search.recv_from(&mut buf) => {
                        if let Ok((len, source)) = received {
                            dispatch(&events, &buf[..len], source).await;
                        }
                    }
                }
            }
        });

        Ok(Self { search_tx })
    }

    pub async fn probe(&self, target: SocketAddr) -> Result<(), Error> {
        self.search_tx
            .send(SearchCmd::Probe(target))
            .await
            .map_err(|_| Error::Closed)
    }

    pub async fn search(&self) -> Result<(), Error> {
        self.search_tx
            .send(SearchCmd::Search)
            .await
            .map_err(|_| Error::Closed)
    }

    pub async fn shutdown(&self) {
        let _ = self.search_tx.send(SearchCmd::Shutdown).await;
    }
}

async fn read_loop(socket: UdpSocket, events: mpsc::Sender<DiscoveryPacket>) {
    let mut buf = [0u8; 2048];
    while !events.is_closed() {
        match socket.recv_from(&mut buf).await {
            Ok((len, source)) => dispatch(&events, &buf[..len], source).await,
            Err(err) => {
                debug!("discovery socket read failed: {err}");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn dispatch(events: &mpsc::Sender<DiscoveryPacket>, bytes: &[u8], source: SocketAddr) {
    let SocketAddr::V4(source) = source else {
        return;
    };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return;
    };
    if text.starts_with("M-SEARCH") {
        return;
    }
    match validate_discovery(text, *source.ip()) {
        Ok(info) => {
            let _ = events
                .send(DiscoveryPacket {
                    info,
                    source: *source.ip(),
                })
                .await;
        }
        Err(Error::Rejected(reason)) => debug!("ignored discovery packet from {source}: {reason}"),
        Err(err) => debug!("ignored unreadable discovery packet from {source}: {err}"),
    }
}

async fn send_searches(socket: &UdpSocket, config: &DiscoveryConfig) {
    let payload = search_request().as_bytes();
    let multicast = SocketAddr::from((MULTICAST_ADDR, config.port));
    let interfaces = lan_ipv4_interfaces();
    if interfaces.is_empty() {
        if let Err(err) = socket.send_to(payload, multicast).await {
            debug!("multicast search failed: {err}");
        }
    } else {
        for ip in &interfaces {
            if let Err(err) = socket2::SockRef::from(socket).set_multicast_if_v4(ip) {
                debug!("could not select interface {ip}: {err}");
                continue;
            }
            if let Err(err) = socket.send_to(payload, multicast).await {
                debug!("multicast search via {ip} failed: {err}");
            }
        }
    }
    let localhost = SocketAddr::from((Ipv4Addr::LOCALHOST, config.port));
    if let Err(err) = socket.send_to(payload, localhost).await {
        debug!("loopback search failed: {err}");
    }
    for target in &config.extra_targets {
        if let Err(err) = socket.send_to(payload, *target).await {
            debug!("search to {target} failed: {err}");
        }
    }
}

fn bind_search_socket() -> Result<UdpSocket, Error> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_nonblocking(true)?;
    socket.set_multicast_loop_v4(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into())?;
    Ok(UdpSocket::from_std(socket.into())?)
}

fn bind_advert_socket(port: u16) -> Result<UdpSocket, Error> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    let _ = socket.set_reuse_port(true);
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
    socket.join_multicast_v4(&MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED)?;
    Ok(UdpSocket::from_std(socket.into())?)
}

/// Up, non-loopback IPv4 addresses suitable for multicast search.
pub fn lan_ipv4_interfaces() -> Vec<Ipv4Addr> {
    let mut found = Vec::new();
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return found;
    };
    for interface in interfaces {
        if interface.is_loopback() {
            continue;
        }
        let if_addrs::IfAddr::V4(addr) = interface.addr else {
            continue;
        };
        if is_lan_ipv4(addr.ip) {
            found.push(addr.ip);
        }
    }
    found.sort();
    found.dedup();
    found
}
