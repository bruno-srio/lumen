use std::net::Ipv4Addr;

/// Addresses this controller is willing to exchange Yeelight traffic with.
///
/// The protocol has no authentication, so connections are limited to loopback
/// (the bundled simulator), link-local, and RFC 1918 private ranges.
pub fn is_lan_ipv4(ip: Ipv4Addr) -> bool {
    if ip.is_unspecified() || ip.is_broadcast() || ip.is_multicast() {
        return false;
    }
    let [a, b, _, _] = ip.octets();
    ip.is_loopback()
        || ip.is_link_local()
        || a == 10
        || (a == 192 && b == 168)
        || (a == 172 && (16..=31).contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_private_and_loopback() {
        assert!(is_lan_ipv4(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(is_lan_ipv4(Ipv4Addr::new(10, 1, 2, 3)));
        assert!(is_lan_ipv4(Ipv4Addr::new(192, 168, 1, 20)));
        assert!(is_lan_ipv4(Ipv4Addr::new(172, 16, 0, 1)));
        assert!(is_lan_ipv4(Ipv4Addr::new(172, 31, 255, 255)));
        assert!(is_lan_ipv4(Ipv4Addr::new(169, 254, 1, 1)));
    }

    #[test]
    fn rejects_public_and_special() {
        assert!(!is_lan_ipv4(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(!is_lan_ipv4(Ipv4Addr::new(1, 1, 1, 1)));
        assert!(!is_lan_ipv4(Ipv4Addr::new(172, 15, 0, 1)));
        assert!(!is_lan_ipv4(Ipv4Addr::new(172, 32, 0, 1)));
        assert!(!is_lan_ipv4(Ipv4Addr::new(0, 0, 0, 0)));
        assert!(!is_lan_ipv4(Ipv4Addr::new(255, 255, 255, 255)));
        assert!(!is_lan_ipv4(Ipv4Addr::new(239, 255, 255, 250)));
    }
}
