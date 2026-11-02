use crate::error::{DeviceError, Error};
use crate::props::{LightState, SYNC_PROPS};
use serde_json::{json, Value};

pub const POWER_DURATION_MS: u32 = 200;
pub const SLIDER_DURATION_MS: u32 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Sudden,
    Smooth,
}

impl Effect {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sudden => "sudden",
            Self::Smooth => "smooth",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    GetProp {
        props: Vec<String>,
    },
    SetPower {
        on: bool,
        effect: Effect,
        duration_ms: u32,
    },
    SetBright {
        brightness: u8,
        effect: Effect,
        duration_ms: u32,
    },
    SetRgb {
        rgb: u32,
        effect: Effect,
        duration_ms: u32,
    },
    SetHsv {
        hue: u16,
        sat: u8,
        effect: Effect,
        duration_ms: u32,
    },
    SetCt {
        kelvin: u32,
        effect: Effect,
        duration_ms: u32,
    },
    SetName {
        name: String,
    },
}

impl Command {
    pub fn sync() -> Self {
        Self::GetProp {
            props: SYNC_PROPS.iter().map(|name| (*name).to_string()).collect(),
        }
    }

    pub fn set_power(on: bool) -> Self {
        Self::SetPower {
            on,
            effect: Effect::Smooth,
            duration_ms: POWER_DURATION_MS,
        }
    }

    pub fn set_bright(brightness: u8) -> Self {
        Self::SetBright {
            brightness: brightness.clamp(1, 100),
            effect: Effect::Smooth,
            duration_ms: SLIDER_DURATION_MS,
        }
    }

    pub fn set_rgb(rgb: u32) -> Self {
        Self::SetRgb {
            rgb: rgb & 0x00ff_ffff,
            effect: Effect::Smooth,
            duration_ms: SLIDER_DURATION_MS,
        }
    }

    pub fn set_hsv(hue: u16, sat: u8) -> Self {
        Self::SetHsv {
            hue: hue % 360,
            sat: sat.min(100),
            effect: Effect::Smooth,
            duration_ms: SLIDER_DURATION_MS,
        }
    }

    pub fn prop_names(&self) -> Option<&[String]> {
        match self {
            Self::GetProp { props } => Some(props),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Result {
        id: u32,
        result: Result<Vec<Value>, DeviceError>,
    },
    Notification(LightState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientMessage {
    pub id: u32,
    pub method: String,
    pub params: Vec<Value>,
}

pub fn encode_command(id: u32, command: &Command) -> String {
    let (method, params) = match command {
        Command::GetProp { props } => (
            "get_prop",
            Value::Array(props.iter().cloned().map(Value::String).collect()),
        ),
        Command::SetPower {
            on,
            effect,
            duration_ms,
        } => (
            "set_power",
            json!([if *on { "on" } else { "off" }, effect.as_str(), duration_ms]),
        ),
        Command::SetBright {
            brightness,
            effect,
            duration_ms,
        } => (
            "set_bright",
            json!([brightness, effect.as_str(), duration_ms]),
        ),
        Command::SetRgb {
            rgb,
            effect,
            duration_ms,
        } => ("set_rgb", json!([rgb, effect.as_str(), duration_ms])),
        Command::SetHsv {
            hue,
            sat,
            effect,
            duration_ms,
        } => ("set_hsv", json!([hue, sat, effect.as_str(), duration_ms])),
        Command::SetCt {
            kelvin,
            effect,
            duration_ms,
        } => ("set_ct_abx", json!([kelvin, effect.as_str(), duration_ms])),
        Command::SetName { name } => ("set_name", json!([name])),
    };
    format!(
        "{}\r\n",
        json!({"id": id, "method": method, "params": params})
    )
}

pub fn encode_ok(id: u32) -> String {
    format!("{{\"id\":{id},\"result\":[\"ok\"]}}\r\n")
}

pub fn encode_error(id: u32, code: i32, message: &str) -> String {
    format!(
        "{}\r\n",
        json!({"id": id, "error": {"code": code, "message": message}})
    )
}

pub fn encode_result_strings(id: u32, values: &[String]) -> String {
    format!(
        "{}\r\n",
        json!({"id": id, "result": values.iter().map(|v| Value::String(v.clone())).collect::<Vec<_>>()})
    )
}

pub fn encode_notification(state: &LightState) -> String {
    let mut params = serde_json::Map::new();
    if let Some(on) = state.power {
        params.insert("power".into(), json!(if on { "on" } else { "off" }));
    }
    if let Some(bright) = state.bright {
        params.insert("bright".into(), json!(bright.to_string()));
    }
    if let Some(mode) = state.color_mode {
        params.insert("color_mode".into(), json!((mode as u8).to_string()));
    }
    if let Some(ct) = state.ct {
        params.insert("ct".into(), json!(ct.to_string()));
    }
    if let Some(rgb) = state.rgb {
        params.insert("rgb".into(), json!(rgb.to_string()));
    }
    if let Some(hue) = state.hue {
        params.insert("hue".into(), json!(hue.to_string()));
    }
    if let Some(sat) = state.sat {
        params.insert("sat".into(), json!(sat.to_string()));
    }
    if let Some(name) = &state.name {
        params.insert("name".into(), json!(name));
    }
    if let Some(flowing) = state.flowing {
        params.insert("flowing".into(), json!(if flowing { "1" } else { "0" }));
    }
    format!("{}\r\n", json!({"method": "props", "params": params}))
}

pub fn parse_incoming(line: &str) -> Result<Incoming, Error> {
    let value: Value = serde_json::from_str(line.trim())
        .map_err(|err| Error::Parse(format!("invalid JSON: {err}")))?;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Parse("message is not an object".into()))?;

    if obj.get("method").and_then(Value::as_str) == Some("props") {
        let mut state = LightState::default();
        if let Some(params) = obj.get("params").and_then(Value::as_object) {
            state.apply_map(params);
        }
        return Ok(Incoming::Notification(state));
    }

    let id = obj
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::Parse("missing id".into()))? as u32;

    if let Some(error) = obj.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(-1) as i32;
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("error")
            .to_string();
        return Ok(Incoming::Result {
            id,
            result: Err(DeviceError { code, message }),
        });
    }

    if let Some(result) = obj.get("result").and_then(Value::as_array) {
        return Ok(Incoming::Result {
            id,
            result: Ok(result.clone()),
        });
    }

    Err(Error::Parse("unrecognized message".into()))
}

pub fn parse_client_message(line: &str) -> Result<ClientMessage, Error> {
    let value: Value = serde_json::from_str(line.trim())
        .map_err(|err| Error::Parse(format!("invalid JSON: {err}")))?;
    let obj = value
        .as_object()
        .ok_or_else(|| Error::Parse("command is not an object".into()))?;
    let id = obj
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::Parse("missing id".into()))? as u32;
    let method = obj
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Parse("missing method".into()))?
        .to_string();
    let params = obj
        .get("params")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(ClientMessage { id, method, params })
}

/// Accumulates socket bytes and emits one parsed message per line.
#[derive(Debug, Default)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn push(&mut self, data: &[u8]) -> Vec<Result<Incoming, Error>> {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buf.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&line);
            let text = text.trim_end_matches(['\r', '\n']);
            if text.is_empty() {
                continue;
            }
            out.push(parse_incoming(text));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spec_examples() {
        let power = encode_command(1, &Command::set_power(true));
        assert!(power.contains("\"method\":\"set_power\""));
        assert!(power.contains("\"on\""));
        assert!(power.ends_with("\r\n"));

        let rgb = encode_command(
            1,
            &Command::SetRgb {
                rgb: 255,
                effect: Effect::Smooth,
                duration_ms: 500,
            },
        );
        let parsed: Value = serde_json::from_str(rgb.trim()).unwrap();
        assert_eq!(parsed["params"][0], 255);
        assert_eq!(parsed["params"][1], "smooth");
        assert_eq!(parsed["params"][2], 500);

        let bright = encode_command(1, &Command::set_bright(50));
        assert!(bright.contains("\"set_bright\""));
    }

    #[test]
    fn parses_results_and_errors() {
        match parse_incoming(r#"{"id":1, "result":["ok"]}"#).unwrap() {
            Incoming::Result { id, result } => {
                assert_eq!(id, 1);
                assert_eq!(result.unwrap()[0], "ok");
            }
            other => panic!("{other:?}"),
        }
        match parse_incoming(r#"{"id":2, "error":{"code":-1, "message":"unsupported method"}}"#)
            .unwrap()
        {
            Incoming::Result { result, .. } => {
                let err = result.unwrap_err();
                assert_eq!(err.code, -1);
                assert!(err.message.contains("unsupported"));
            }
            other => panic!("{other:?}"),
        }
        match parse_incoming(r#"{"id":3, "result":["on","100"]}"#).unwrap() {
            Incoming::Result { result, .. } => {
                let values = result.unwrap();
                assert_eq!(values[0], "on");
                assert_eq!(values[1], "100");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_notification_with_string_values() {
        match parse_incoming(r#"{"method":"props","params":{"power":"on","bright":"10"}}"#).unwrap()
        {
            Incoming::Notification(state) => {
                assert_eq!(state.power, Some(true));
                assert_eq!(state.bright, Some(10));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn line_buffer_handles_partial_and_multiple_messages() {
        let mut buffer = LineBuffer::new();
        assert!(buffer.push(br#"{"id":1,"result":["ok"]}"#).is_empty());
        let first = buffer.push(b"\r\n{\"method\":\"props\",\"params\":{\"power\":\"off\"}}\r\n");
        assert_eq!(first.len(), 2);
        assert!(matches!(first[0], Ok(Incoming::Result { .. })));
        assert!(matches!(first[1], Ok(Incoming::Notification(_))));
    }

    #[test]
    fn malformed_json_is_an_error_not_a_panic() {
        assert!(parse_incoming("not json").is_err());
        assert!(parse_incoming("{").is_err());
        let mut buffer = LineBuffer::new();
        let parsed = buffer.push(b"garbage\r\n");
        assert!(parsed[0].is_err());
    }
}
