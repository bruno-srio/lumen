use std::net::{Ipv4Addr, SocketAddr};
use yeelight::{is_lan_ipv4, DISCOVERY_PORT};

/// What a group command is allowed to change on one member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FanoutMember {
    pub online: bool,
    pub power: bool,
    pub has_power: bool,
    pub has_brightness: bool,
    pub has_color: bool,
    pub has_temperature: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TintKind {
    Color,
    Temperature,
}

/// Accept `ip` or `ip:port` when the address is on the local network.
/// A bare address uses the Yeelight discovery port.
pub fn parse_probe_address(text: &str) -> Option<SocketAddr> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let addr = if let Ok(addr) = text.parse::<SocketAddr>() {
        addr
    } else if let Ok(ip) = text.parse::<Ipv4Addr>() {
        SocketAddr::from((ip, DISCOVERY_PORT))
    } else {
        return None;
    };
    match addr {
        SocketAddr::V4(addr) if is_lan_ipv4(*addr.ip()) => Some(SocketAddr::V4(addr)),
        _ => None,
    }
}

pub fn receives_power(member: &FanoutMember) -> bool {
    member.online && member.has_power
}

pub fn receives_brightness(member: &FanoutMember) -> bool {
    member.online && member.power && member.has_brightness
}

pub fn receives_tint(member: &FanoutMember, kind: TintKind) -> bool {
    if !member.online || !member.power {
        return false;
    }
    match kind {
        TintKind::Color => member.has_color,
        TintKind::Temperature => member.has_temperature,
    }
}

/// Toggle All turns the lights off when any reachable one is on, and on otherwise.
pub fn toggle_all_target(members: &[FanoutMember]) -> bool {
    let any_on = members
        .iter()
        .any(|member| member.online && member.has_power && member.power);
    !any_on
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member() -> FanoutMember {
        FanoutMember {
            online: true,
            power: true,
            has_power: true,
            has_brightness: true,
            has_color: true,
            has_temperature: true,
        }
    }

    #[test]
    fn probe_address_accepts_a_lan_ip_and_an_explicit_port() {
        let plain = parse_probe_address("192.168.1.20").unwrap();
        assert_eq!(plain.ip().to_string(), "192.168.1.20");
        assert_eq!(plain.port(), DISCOVERY_PORT);
        let explicit = parse_probe_address("  10.0.0.4:1982 ").unwrap();
        assert_eq!(explicit.port(), 1982);
    }

    #[test]
    fn probe_address_rejects_empty_and_public() {
        assert!(parse_probe_address("").is_none());
        assert!(parse_probe_address("   ").is_none());
        assert!(parse_probe_address("not-an-ip").is_none());
        assert!(parse_probe_address("8.8.8.8").is_none());
        assert!(parse_probe_address("1.1.1.1:1982").is_none());
    }

    #[test]
    fn fanout_respects_power_and_capability() {
        let mut off = member();
        off.power = false;
        assert!(receives_power(&off));
        assert!(!receives_brightness(&off));
        assert!(!receives_tint(&off, TintKind::Color));

        let mut offline = member();
        offline.online = false;
        assert!(!receives_power(&offline));

        let mut mono = member();
        mono.has_color = false;
        mono.has_temperature = false;
        assert!(!receives_tint(&mono, TintKind::Color));
        assert!(!receives_tint(&mono, TintKind::Temperature));

        let mut white = member();
        white.has_color = false;
        assert!(!receives_tint(&white, TintKind::Color));
        assert!(receives_tint(&white, TintKind::Temperature));

        let mut dim = member();
        dim.has_brightness = false;
        assert!(!receives_brightness(&dim));
    }

    #[test]
    fn toggle_all_turns_off_when_anything_is_on() {
        let on = member();
        let mut off = member();
        off.power = false;
        assert!(!toggle_all_target(&[on, off]));
        assert!(toggle_all_target(&[off]));

        let mut unreachable = member();
        unreachable.online = false;
        unreachable.power = true;
        assert!(toggle_all_target(&[unreachable]));
    }
}
