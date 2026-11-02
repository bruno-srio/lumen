use crate::error::Error;
use crate::lan::is_lan_ipv4;
use crate::props::{parse_power, ColorMode, DeviceId};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

pub const DISCOVERY_PORT: u16 = 1982;
pub const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);

pub const SEARCH_REQUEST: &str = "\
M-SEARCH * HTTP/1.1\r\n\
HOST: 239.255.255.250:1982\r\n\
MAN: \"ssdp:discover\"\r\n\
ST: wifi_bulb\r\n\
\r\n";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryInfo {
    pub id: DeviceId,
    pub location: SocketAddr,
    pub model: String,
    pub fw_ver: String,
    pub support: Vec<String>,
    pub power: Option<bool>,
    pub bright: Option<u8>,
    pub color_mode: Option<ColorMode>,
    pub ct: Option<u32>,
    pub rgb: Option<u32>,
    pub hue: Option<u16>,
    pub sat: Option<u8>,
    pub name: String,
    pub max_age: Option<u32>,
}

pub fn search_request() -> &'static str {
    SEARCH_REQUEST
}

pub fn parse_discovery(message: &str) -> Result<DiscoveryInfo, Error> {
    let mut lines = message.split(['\n']);
    let start = lines
        .next()
        .unwrap_or_default()
        .trim()
        .trim_end_matches('\r');
    let kind_ok = start == "HTTP/1.1 200 OK" || start.starts_with("NOTIFY * HTTP/1.1");
    if !kind_ok {
        return Err(Error::Parse(format!(
            "unexpected discovery start line: {start}"
        )));
    }

    let mut headers = HashMap::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }

    let id = headers
        .get("id")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| Error::Parse("discovery message has no id".into()))?;
    let location = headers
        .get("location")
        .ok_or_else(|| Error::Parse("discovery message has no location".into()))?;
    let location = parse_location(location)?;

    let support = headers
        .get("support")
        .map(|value| {
            value
                .split_whitespace()
                .filter(|method| !method.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    Ok(DiscoveryInfo {
        id: DeviceId::new(id),
        location,
        model: headers
            .get("model")
            .cloned()
            .unwrap_or_else(|| "unknown".into()),
        fw_ver: headers.get("fw_ver").cloned().unwrap_or_default(),
        support,
        power: headers.get("power").and_then(|value| parse_power(value)),
        bright: headers
            .get("bright")
            .and_then(|value| value.parse::<u8>().ok())
            .map(|value| value.clamp(1, 100)),
        color_mode: headers
            .get("color_mode")
            .and_then(|value| value.parse::<u8>().ok())
            .and_then(ColorMode::from_code),
        ct: headers.get("ct").and_then(|value| value.parse().ok()),
        rgb: headers.get("rgb").and_then(|value| value.parse().ok()),
        hue: headers.get("hue").and_then(|value| value.parse().ok()),
        sat: headers.get("sat").and_then(|value| value.parse().ok()),
        name: headers.get("name").cloned().unwrap_or_default(),
        max_age: headers
            .get("cache-control")
            .and_then(|value| parse_max_age(value)),
    })
}

/// Accept a discovery datagram only when its location matches the sender and
/// both are on a local network.
pub fn validate_discovery(message: &str, source: Ipv4Addr) -> Result<DiscoveryInfo, Error> {
    let info = parse_discovery(message)?;
    let std::net::SocketAddr::V4(location) = info.location else {
        return Err(Error::Rejected("location is not IPv4"));
    };
    if *location.ip() != source {
        return Err(Error::Rejected(
            "location address does not match the sender",
        ));
    }
    if !is_lan_ipv4(source) {
        return Err(Error::Rejected("sender is not on the local network"));
    }
    Ok(info)
}

pub fn format_search_response(info: &DiscoveryInfo) -> String {
    format_message("HTTP/1.1 200 OK", info, false)
}

pub fn format_notify(info: &DiscoveryInfo) -> String {
    format_message("NOTIFY * HTTP/1.1", info, true)
}

fn format_message(start: &str, info: &DiscoveryInfo, notify: bool) -> String {
    let max_age = info.max_age.unwrap_or(3600);
    let mut out = format!("{start}\r\nCache-Control: max-age={max_age}\r\n");
    if notify {
        out.push_str("NTS: ssdp:alive\r\n");
    }
    out.push_str(&format!("Location: yeelight://{}\r\n", info.location));
    out.push_str(&format!("id: {}\r\n", info.id));
    out.push_str(&format!("model: {}\r\n", info.model));
    out.push_str(&format!("fw_ver: {}\r\n", info.fw_ver));
    out.push_str(&format!("support: {}\r\n", info.support.join(" ")));
    if let Some(power) = info.power {
        out.push_str(&format!("power: {}\r\n", if power { "on" } else { "off" }));
    }
    if let Some(bright) = info.bright {
        out.push_str(&format!("bright: {bright}\r\n"));
    }
    if let Some(mode) = info.color_mode {
        out.push_str(&format!("color_mode: {}\r\n", mode as u8));
    }
    if let Some(ct) = info.ct {
        out.push_str(&format!("ct: {ct}\r\n"));
    }
    if let Some(rgb) = info.rgb {
        out.push_str(&format!("rgb: {rgb}\r\n"));
    }
    if let Some(hue) = info.hue {
        out.push_str(&format!("hue: {hue}\r\n"));
    }
    if let Some(sat) = info.sat {
        out.push_str(&format!("sat: {sat}\r\n"));
    }
    out.push_str(&format!("name: {}\r\n", info.name));
    out.push_str("\r\n");
    out
}

fn parse_location(value: &str) -> Result<SocketAddr, Error> {
    let rest = value
        .trim()
        .strip_prefix("yeelight://")
        .ok_or_else(|| Error::Parse(format!("bad location: {value}")))?;
    rest.parse()
        .map_err(|_| Error::Parse(format!("bad location address: {value}")))
}

fn parse_max_age(value: &str) -> Option<u32> {
    value.split('=').nth(1)?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = "\
HTTP/1.1 200 OK\r\n\
Cache-Control: max-age=3600\r\n\
Date:\r\n\
Ext:\r\n\
Location: yeelight://192.168.1.239:55443\r\n\
Server: POSIX UPnP/1.0 YGLC/1\r\n\
id: 0x000000000015243f\r\n\
model: color\r\n\
fw_ver: 18\r\n\
support: get_prop set_default set_power toggle set_bright start_cf stop_cf set_scene cron_add cron_get cron_del set_ct_abx set_rgb\r\n\
power: on\r\n\
bright: 100\r\n\
color_mode: 2\r\n\
ct: 4000\r\n\
rgb: 16711680\r\n\
hue: 100\r\n\
sat: 35\r\n\
name: my_bulb\r\n\
\r\n";

    const NOTIFY: &str = "\
NOTIFY * HTTP/1.1\r\n\
Host: 239.255.255.250:1982\r\n\
Cache-Control: max-age=3600\r\n\
Location: yeelight://192.168.1.239:55443\r\n\
NTS: ssdp:alive\r\n\
id: 0x000000000015243f\r\n\
model: color\r\n\
fw_ver: 18\r\n\
support: get_prop set_power set_bright set_rgb\r\n\
power: on\r\n\
bright: 100\r\n\
color_mode: 2\r\n\
ct: 4000\r\n\
rgb: 16711680\r\n\
hue: 100\r\n\
sat: 35\r\n\
name: my_bulb\r\n\
\r\n";

    #[test]
    fn parses_search_response_from_the_spec() {
        let info = parse_discovery(RESPONSE).unwrap();
        assert_eq!(info.id.as_str(), "0x000000000015243f");
        assert_eq!(info.location.to_string(), "192.168.1.239:55443");
        assert_eq!(info.model, "color");
        assert_eq!(info.fw_ver, "18");
        assert!(info.support.iter().any(|m| m == "set_rgb"));
        assert!(info.support.iter().any(|m| m == "set_ct_abx"));
        assert_eq!(info.power, Some(true));
        assert_eq!(info.bright, Some(100));
        assert_eq!(info.color_mode, Some(ColorMode::ColorTemperature));
        assert_eq!(info.ct, Some(4000));
        assert_eq!(info.rgb, Some(16_711_680));
        assert_eq!(info.hue, Some(100));
        assert_eq!(info.sat, Some(35));
        assert_eq!(info.name, "my_bulb");
        assert_eq!(info.max_age, Some(3600));
    }

    #[test]
    fn parses_advertisement_and_case_insensitive_headers() {
        let info = parse_discovery(NOTIFY).unwrap();
        assert_eq!(info.id.as_str(), "0x000000000015243f");
        let mixed = RESPONSE
            .replace("Location:", "lOcAtIoN:")
            .replace("id:", "ID:");
        let info = parse_discovery(&mixed).unwrap();
        assert_eq!(info.location.port(), 55443);
    }

    #[test]
    fn rejects_wrong_start_line_and_public_senders() {
        assert!(parse_discovery("M-SEARCH * HTTP/1.1\r\n\r\n").is_err());
        assert!(parse_discovery("HTTP/1.1 200 OK\r\n\r\n").is_err());
        let err = validate_discovery(RESPONSE, Ipv4Addr::new(8, 8, 8, 8)).unwrap_err();
        assert!(matches!(err, Error::Rejected(_)));
        let err = validate_discovery(RESPONSE, Ipv4Addr::new(192, 168, 1, 1)).unwrap_err();
        assert!(matches!(err, Error::Rejected(_)));
        let ok = validate_discovery(RESPONSE, Ipv4Addr::new(192, 168, 1, 239)).unwrap();
        assert_eq!(ok.name, "my_bulb");
    }

    #[test]
    fn search_request_matches_the_spec() {
        let request = search_request();
        assert!(request.starts_with("M-SEARCH * HTTP/1.1\r\n"));
        assert!(request.contains("MAN: \"ssdp:discover\"\r\n"));
        assert!(request.contains("ST: wifi_bulb\r\n"));
    }

    #[test]
    fn formatted_messages_parse_back() {
        let info = parse_discovery(RESPONSE).unwrap();
        let again = parse_discovery(&format_search_response(&info)).unwrap();
        assert_eq!(again.id, info.id);
        assert_eq!(again.support, info.support);
        let notify = parse_discovery(&format_notify(&info)).unwrap();
        assert_eq!(notify.location, info.location);
    }
}
