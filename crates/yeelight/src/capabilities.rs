use crate::props::DeviceId;

/// What the controller may offer for one bulb.
///
/// Derived from the discovery `support` list. `model` only fills in a color
/// temperature range hint and a friendly fallback name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub power: bool,
    pub brightness: bool,
    pub rgb: bool,
    pub hsv: bool,
    pub color_temperature: bool,
    pub background: bool,
    pub ct_min: u32,
    pub ct_max: u32,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            power: false,
            brightness: false,
            rgb: false,
            hsv: false,
            color_temperature: false,
            background: false,
            ct_min: 1700,
            ct_max: 6500,
        }
    }
}

impl Capabilities {
    pub fn from_support(support: &[String], model: &str) -> Self {
        let has = |name: &str| support.iter().any(|method| method == name);
        let (ct_min, ct_max) = ct_range(model);
        Self {
            power: has("set_power"),
            brightness: has("set_bright"),
            rgb: has("set_rgb"),
            hsv: has("set_hsv"),
            color_temperature: has("set_ct_abx"),
            background: has("bg_set_power"),
            ct_min,
            ct_max,
        }
    }

    pub fn has_color(&self) -> bool {
        self.rgb || self.hsv
    }
}

fn ct_range(model: &str) -> (u32, u32) {
    let model = model.to_ascii_lowercase();
    if model.contains("ceiling") || model.contains("lamp") || model.starts_with("ct") {
        (2700, 6500)
    } else {
        (1700, 6500)
    }
}

pub fn model_label(model: &str) -> &'static str {
    let model = model.to_ascii_lowercase();
    if model.contains("stripe") || model.contains("strip") {
        "Light Strip"
    } else if model.contains("ceiling") {
        "Ceiling Light"
    } else if model.contains("lamp") || model == "bslamp" {
        "Desk Lamp"
    } else if model.contains("color") {
        "Color Bulb"
    } else if model.contains("mono") || model.starts_with("ct") {
        "White Bulb"
    } else {
        "Light"
    }
}

pub fn fallback_name(model: &str, id: &DeviceId) -> String {
    format!("{} {}", model_label(model), id.suffix())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn support(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn color_bulb_exposes_color_and_temperature() {
        let caps = Capabilities::from_support(
            &support(&[
                "set_power",
                "set_bright",
                "set_rgb",
                "set_hsv",
                "set_ct_abx",
            ]),
            "color",
        );
        assert!(caps.power && caps.brightness && caps.has_color());
        assert!(caps.hsv && caps.rgb && caps.color_temperature);
        assert!(!caps.background);
        assert_eq!((caps.ct_min, caps.ct_max), (1700, 6500));
    }

    #[test]
    fn mono_hides_color() {
        let caps = Capabilities::from_support(&support(&["set_power", "set_bright"]), "mono");
        assert!(caps.power && caps.brightness);
        assert!(!caps.has_color());
        assert!(!caps.color_temperature);
    }

    #[test]
    fn background_and_names() {
        let caps =
            Capabilities::from_support(&support(&["set_power", "bg_set_power"]), "ceiling10");
        assert!(caps.background);
        assert_eq!(model_label("ceiling10"), "Ceiling Light");
        assert_eq!(model_label("mystery"), "Light");
        let name = fallback_name("color4", &DeviceId::new("0x000000000015243f"));
        assert_eq!(name, "Color Bulb 243f");
    }
}
