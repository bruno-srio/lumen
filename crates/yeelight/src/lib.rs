//! Yeelight local-network protocol and a small async client.
//!
//! The bulb protocol is not encrypted and has no authentication. This crate
//! only connects to loopback, link-local, and private IPv4 addresses.

mod capabilities;
mod color;
mod connection;
mod discovery;
mod discovery_msg;
mod effects;
mod error;
mod lan;
mod manager;
mod props;
mod protocol;
mod scheduler;

pub use capabilities::{fallback_name, model_label, Capabilities};
pub use color::{ct_to_rgb, hs_to_rgb, rgb_to_hs, Rgb};
pub use connection::{
    transact, ColorUpdate, ConnectionState, DeviceCommand, DeviceEvent, EffectPreset, FailReason,
    LinkConfig, UnavailableReason,
};
pub use discovery::{lan_ipv4_interfaces, Discovery, DiscoveryConfig, DiscoveryPacket};
pub use discovery_msg::{
    format_notify, format_search_response, parse_discovery, search_request, validate_discovery,
    DiscoveryInfo, DISCOVERY_PORT, MULTICAST_ADDR, SEARCH_REQUEST,
};
pub use effects::flow;
pub use error::{DeviceError, Error};
pub use lan::is_lan_ipv4;
pub use manager::{run as run_manager, BackendCommand, BackendEvent, ManagerConfig};
pub use props::{ColorMode, DeviceId, LightState, SYNC_PROPS};
pub use protocol::{
    encode_command, encode_error, encode_notification, encode_ok, encode_result_strings,
    parse_client_message, parse_incoming, ClientMessage, Command, Effect, Incoming, LineBuffer,
};
