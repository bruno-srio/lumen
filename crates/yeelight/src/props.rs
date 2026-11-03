use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Stable Yeelight device id (`id` header). Compared case-insensitively.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(raw: impl AsRef<str>) -> Self {
        Self(raw.as_ref().trim().to_ascii_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Last four hex digits, used in the generated display name.
    pub fn suffix(&self) -> String {
        let hex = self.0.trim_start_matches("0x");
        let start = hex.len().saturating_sub(4);
        hex[start..].to_string()
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Rgb = 1,
    ColorTemperature = 2,
    Hsv = 3,
}

impl ColorMode {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Rgb),
            2 => Some(Self::ColorTemperature),
            3 => Some(Self::Hsv),
            _ => None,
        }
    }
}

/// Subset of bulb properties the controller displays and edits.
///
/// `None` means "not present in this update", not "unknown value".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LightState {
    pub power: Option<bool>,
    pub bright: Option<u8>,
    pub color_mode: Option<ColorMode>,
    pub ct: Option<u32>,
    pub rgb: Option<u32>,
    pub hue: Option<u16>,
    pub sat: Option<u8>,
    pub name: Option<String>,
    pub flowing: Option<bool>,
    pub delayoff: Option<u32>,
}

impl LightState {
    pub fn merge(&mut self, other: &LightState) {
        if other.power.is_some() {
            self.power = other.power;
        }
        if other.bright.is_some() {
            self.bright = other.bright;
        }
        if other.color_mode.is_some() {
            self.color_mode = other.color_mode;
        }
        if other.ct.is_some() {
            self.ct = other.ct;
        }
        if other.rgb.is_some() {
            self.rgb = other.rgb;
        }
        if other.hue.is_some() {
            self.hue = other.hue;
        }
        if other.sat.is_some() {
            self.sat = other.sat;
        }
        if other.name.is_some() {
            self.name.clone_from(&other.name);
        }
        if other.flowing.is_some() {
            self.flowing = other.flowing;
        }
        if other.delayoff.is_some() {
            self.delayoff = other.delayoff;
        }
    }

    pub fn apply_values(&mut self, names: &[String], values: &[Value]) {
        for (name, value) in names.iter().zip(values.iter()) {
            if let Some(raw) = value_to_raw(value) {
                self.apply_raw(name, &raw);
            }
        }
    }

    pub fn apply_map(&mut self, map: &serde_json::Map<String, Value>) {
        for (key, value) in map {
            if let Some(raw) = value_to_raw(value) {
                self.apply_raw(key, &raw);
            }
        }
    }

    fn apply_raw(&mut self, key: &str, raw: &str) {
        if raw.is_empty() {
            return;
        }
        match key {
            "power" => {
                if let Some(on) = parse_power(raw) {
                    self.power = Some(on);
                }
            }
            "bright" => {
                if let Ok(value) = raw.parse::<u8>() {
                    self.bright = Some(value.clamp(1, 100));
                }
            }
            "color_mode" => {
                if let Ok(code) = raw.parse::<u8>() {
                    self.color_mode = ColorMode::from_code(code);
                }
            }
            "ct" => {
                if let Ok(value) = raw.parse::<u32>() {
                    self.ct = Some(value);
                }
            }
            "rgb" => {
                if let Ok(value) = raw.parse::<u32>() {
                    self.rgb = Some(value & 0x00ff_ffff);
                }
            }
            "hue" => {
                if let Ok(value) = raw.parse::<u16>() {
                    self.hue = Some(value % 360);
                }
            }
            "sat" => {
                if let Ok(value) = raw.parse::<u8>() {
                    self.sat = Some(value.min(100));
                }
            }
            "name" => self.name = Some(raw.to_string()),
            "flowing" => match raw {
                "1" => self.flowing = Some(true),
                "0" => self.flowing = Some(false),
                _ => {}
            },
            "delayoff" => {
                if let Ok(value) = raw.parse::<u32>() {
                    self.delayoff = Some(value);
                }
            }
            _ => {}
        }
    }
}

pub fn parse_power(raw: &str) -> Option<bool> {
    match raw {
        "on" => Some(true),
        "off" => Some(false),
        _ => None,
    }
}

fn value_to_raw(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(true) => Some("1".to_string()),
        Value::Bool(false) => Some("0".to_string()),
        _ => None,
    }
}

pub const SYNC_PROPS: &[&str] = &[
    "power",
    "bright",
    "ct",
    "rgb",
    "hue",
    "sat",
    "color_mode",
    "name",
    "flowing",
    "delayoff",
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn id_normalizes_case_and_suffix() {
        let id = DeviceId::new("  0x000000000015243F ");
        assert_eq!(id.as_str(), "0x000000000015243f");
        assert_eq!(id.suffix(), "243f");
        assert_eq!(id, DeviceId::new("0x000000000015243f"));
    }

    #[test]
    fn get_prop_values_and_notification_map() {
        let mut state = LightState::default();
        let names: Vec<String> = ["power", "not_exist", "bright"]
            .into_iter()
            .map(str::to_string)
            .collect();
        state.apply_values(&names, &[json!("on"), json!(""), json!("100")]);
        assert_eq!(state.power, Some(true));
        assert_eq!(state.bright, Some(100));

        let mut map = serde_json::Map::new();
        map.insert("power".into(), json!("off"));
        map.insert("bright".into(), json!(40));
        map.insert("color_mode".into(), json!("1"));
        map.insert("rgb".into(), json!("16711680"));
        state.apply_map(&map);
        assert_eq!(state.power, Some(false));
        assert_eq!(state.bright, Some(40));
        assert_eq!(state.color_mode, Some(ColorMode::Rgb));
        assert_eq!(state.rgb, Some(16_711_680));
    }
}
