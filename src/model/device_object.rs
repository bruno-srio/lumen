use gettextrs::gettext;
use glib::subclass::prelude::*;
use glib::{prelude::*, Properties};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};
use yeelight::{Capabilities, ConnectionState, DeviceId, LightState, UnavailableReason};

const HOLD: Duration = Duration::from_secs(1);

mod imp {
    use super::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::DeviceObject)]
    pub struct DeviceObject {
        #[property(get, set, name = "device-id")]
        pub device_id: RefCell<String>,
        #[property(get, set)]
        pub name: RefCell<String>,
        #[property(get, set, name = "custom-name")]
        pub custom_name: RefCell<String>,
        #[property(get, set, name = "bulb-name")]
        pub bulb_name: RefCell<String>,
        #[property(get, set)]
        pub model: RefCell<String>,
        #[property(get, set)]
        pub firmware: RefCell<String>,
        #[property(get, set)]
        pub address: RefCell<String>,
        #[property(get, set)]
        pub port: Cell<u32>,
        #[property(get, set)]
        pub support: RefCell<String>,
        #[property(get, set, name = "status-text")]
        pub status_text: RefCell<String>,
        #[property(get, set, name = "unavailable-reason")]
        pub unavailable_reason: RefCell<String>,
        #[property(get, set)]
        pub power: Cell<bool>,
        #[property(get, set)]
        pub brightness: Cell<u32>,
        #[property(get, set)]
        pub hue: Cell<u32>,
        #[property(get, set)]
        pub saturation: Cell<u32>,
        #[property(get, set)]
        pub rgb: Cell<u32>,
        #[property(get, set, name = "has-power")]
        pub has_power: Cell<bool>,
        #[property(get, set, name = "has-brightness")]
        pub has_brightness: Cell<bool>,
        #[property(get, set, name = "has-color")]
        pub has_color: Cell<bool>,
        #[property(get, set, name = "has-temperature")]
        pub has_temperature: Cell<bool>,
        #[property(get, set)]
        pub online: Cell<bool>,
        #[property(get, set)]
        pub connecting: Cell<bool>,
        #[property(get, set)]
        pub seen: Cell<bool>,
        #[property(get, set, name = "power-enabled")]
        pub power_enabled: Cell<bool>,
        #[property(get, set, name = "brightness-enabled")]
        pub brightness_enabled: Cell<bool>,
        #[property(get, set, name = "color-enabled")]
        pub color_enabled: Cell<bool>,

        pub confirmed_power: Cell<bool>,
        pub confirmed_bright: Cell<u32>,
        pub confirmed_hue: Cell<u32>,
        pub confirmed_sat: Cell<u32>,
        pub bright_hold: Cell<Option<Instant>>,
        pub color_hold: Cell<Option<Instant>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceObject {
        const NAME: &'static str = "LumenDeviceObject";
        type Type = super::DeviceObject;
    }

    impl ObjectImpl for DeviceObject {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }
        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }
        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }
}

glib::wrapper! {
    pub struct DeviceObject(ObjectSubclass<imp::DeviceObject>);
}

impl DeviceObject {
    pub fn new(id: &str) -> Self {
        glib::Object::builder().property("device-id", id).build()
    }

    pub fn assign_custom_name(&self, name: Option<String>) {
        let cleaned = name.unwrap_or_default().trim().to_string();
        self.set_custom_name(cleaned);
        self.recompute_name();
    }

    pub fn set_capabilities(&self, caps: &Capabilities) {
        self.set_has_power(caps.power);
        self.set_has_brightness(caps.brightness);
        self.set_has_color(caps.has_color());
        self.set_has_temperature(caps.color_temperature);
        self.refresh_status();
    }

    pub fn set_connection(&self, state: &ConnectionState) {
        match state {
            ConnectionState::Connecting => {
                self.set_connecting(true);
                self.set_online(false);
                self.set_unavailable_reason(String::new());
            }
            ConnectionState::Online => {
                self.set_connecting(false);
                self.set_online(true);
                self.set_unavailable_reason(String::new());
            }
            ConnectionState::Unavailable(reason) => {
                self.set_connecting(false);
                self.set_online(false);
                self.set_unavailable_reason(reason_text(*reason));
            }
        }
        self.refresh_status();
    }

    pub fn apply_state(&self, state: &LightState) {
        let imp = self.imp();
        if let Some(power) = state.power {
            imp.confirmed_power.set(power);
            self.set_power(power);
        }
        if let Some(bright) = state.bright {
            imp.confirmed_bright.set(u32::from(bright));
            if hold_expired(&imp.bright_hold) {
                self.set_brightness(u32::from(bright));
            }
        }
        if let Some(hue) = state.hue {
            imp.confirmed_hue.set(u32::from(hue));
            if hold_expired(&imp.color_hold) {
                self.set_hue(u32::from(hue));
            }
        }
        if let Some(sat) = state.sat {
            imp.confirmed_sat.set(u32::from(sat));
            if hold_expired(&imp.color_hold) {
                self.set_saturation(u32::from(sat));
            }
        }
        if let Some(rgb) = state.rgb {
            self.set_rgb(rgb);
        }
        if let Some(name) = &state.name {
            if self.bulb_name() != *name {
                self.set_bulb_name(name.clone());
                self.recompute_name();
            }
        }
        self.refresh_status();
    }

    pub fn user_power(&self, on: bool) {
        self.set_power(on);
        self.refresh_status();
    }

    pub fn user_brightness(&self, value: u8) {
        self.imp().bright_hold.set(Some(Instant::now() + HOLD));
        self.set_brightness(u32::from(value.clamp(1, 100)));
        self.refresh_status();
    }

    pub fn user_color(&self, hue: u16, sat: u8) {
        self.imp().color_hold.set(Some(Instant::now() + HOLD));
        self.set_hue(u32::from(hue % 360));
        self.set_saturation(u32::from(sat.min(100)));
        let rgb = yeelight::hs_to_rgb(hue, sat).to_u32();
        self.set_rgb(rgb);
        self.refresh_status();
    }

    pub fn revert(&self) {
        let imp = self.imp();
        imp.bright_hold.set(None);
        imp.color_hold.set(None);
        self.set_power(imp.confirmed_power.get());
        self.set_brightness(imp.confirmed_bright.get());
        self.set_hue(imp.confirmed_hue.get());
        self.set_saturation(imp.confirmed_sat.get());
        self.refresh_status();
    }

    pub fn recompute_name(&self) {
        let custom = self.custom_name();
        let bulb = self.bulb_name();
        let name = if !custom.trim().is_empty() {
            custom
        } else if !bulb.trim().is_empty() {
            bulb
        } else {
            yeelight::fallback_name(&self.model(), &DeviceId::new(self.device_id()))
        };
        if self.name() != name {
            self.set_name(name);
        }
    }

    pub fn refresh_status(&self) {
        let text = if self.connecting() && !self.online() {
            gettext("Connecting…")
        } else if !self.online() {
            let reason = self.unavailable_reason();
            if reason.is_empty() {
                gettext("Unavailable")
            } else {
                reason
            }
        } else if !self.power() {
            gettext("Off")
        } else if self.has_brightness() {
            format!("{} · {}%", gettext("On"), self.brightness())
        } else {
            gettext("On")
        };
        if self.status_text() != text {
            self.set_status_text(text);
        }
        let online = self.online();
        self.set_power_enabled(online && self.has_power());
        self.set_brightness_enabled(online && self.power() && self.has_brightness());
        self.set_color_enabled(online && self.power() && self.has_color());
    }

    pub fn features_label(&self) -> String {
        let mut parts = Vec::new();
        if self.has_power() {
            parts.push(gettext("Power"));
        }
        if self.has_brightness() {
            parts.push(gettext("Brightness"));
        }
        if self.has_color() {
            parts.push(gettext("Color"));
        }
        if self.has_temperature() {
            parts.push(gettext("Color temperature"));
        }
        if parts.is_empty() {
            gettext("No controls reported")
        } else {
            parts.join(", ")
        }
    }
}

fn hold_expired(hold: &Cell<Option<Instant>>) -> bool {
    match hold.get() {
        Some(until) if Instant::now() < until => false,
        _ => {
            hold.set(None);
            true
        }
    }
}

fn reason_text(reason: UnavailableReason) -> String {
    match reason {
        UnavailableReason::Timeout => gettext("The light did not respond."),
        UnavailableReason::Refused => {
            gettext("The light refused the connection. Enable LAN Control in the Yeelight app.")
        }
        UnavailableReason::Network => gettext("Network error."),
        UnavailableReason::Closed => gettext("The light closed the connection."),
        UnavailableReason::StuckQuota => {
            gettext("Not responding — try turning the light off and on at the switch.")
        }
    }
}
