/// 8-bit RGB color. The bulb's brightness is a separate property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_u32(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xff) as u8,
            g: ((value >> 8) & 0xff) as u8,
            b: (value & 0xff) as u8,
        }
    }

    pub fn to_u32(self) -> u32 {
        (u32::from(self.r) << 16) | (u32::from(self.g) << 8) | u32::from(self.b)
    }
}

/// Hue is 0–359 and saturation is 0–100, matching `set_hsv`.
pub fn rgb_to_hs(rgb: Rgb) -> (u16, u8) {
    let r = f32::from(rgb.r) / 255.0;
    let g = f32::from(rgb.g) / 255.0;
    let b = f32::from(rgb.b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    let sat = if max == 0.0 { 0.0 } else { delta / max };
    ((hue.round() as u16) % 360, (sat * 100.0).round() as u8)
}

pub fn hs_to_rgb(hue: u16, sat: u8) -> Rgb {
    let h = f32::from(hue % 360);
    let s = f32::from(sat.min(100)) / 100.0;
    let v = 1.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h as u16) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Rgb {
        r: ((r + m) * 255.0).round() as u8,
        g: ((g + m) * 255.0).round() as u8,
        b: ((b + m) * 255.0).round() as u8,
    }
}

/// Approximate a color temperature in kelvin as sRGB, for the card indicator.
pub fn ct_to_rgb(kelvin: u32) -> Rgb {
    let temp = (kelvin.clamp(1000, 40000) as f64) / 100.0;
    let r = if temp <= 66.0 {
        255.0
    } else {
        329.698727446 * (temp - 60.0).powf(-0.1332047592)
    };
    let g = if temp <= 66.0 {
        99.4708025861 * temp.ln() - 161.1195681661
    } else {
        288.1221695283 * (temp - 60.0).powf(-0.0755148492)
    };
    let b = if temp >= 66.0 {
        255.0
    } else if temp <= 19.0 {
        0.0
    } else {
        138.5177312231 * (temp - 10.0).ln() - 305.0447927307
    };
    Rgb {
        r: r.clamp(0.0, 255.0).round() as u8,
        g: g.clamp(0.0, 255.0).round() as u8,
        b: b.clamp(0.0, 255.0).round() as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_round_trip_u32() {
        let rgb = Rgb::from_u32(0x00ff_8800);
        assert_eq!(rgb, Rgb::new(255, 136, 0));
        assert_eq!(rgb.to_u32(), 0x00ff_8800);
    }

    #[test]
    fn primary_hues() {
        assert_eq!(rgb_to_hs(Rgb::new(255, 0, 0)), (0, 100));
        assert_eq!(rgb_to_hs(Rgb::new(0, 255, 0)), (120, 100));
        assert_eq!(rgb_to_hs(Rgb::new(0, 0, 255)), (240, 100));
        assert_eq!(rgb_to_hs(Rgb::new(255, 255, 255)).1, 0);
    }

    #[test]
    fn hs_to_rgb_primaries() {
        assert_eq!(hs_to_rgb(0, 100), Rgb::new(255, 0, 0));
        assert_eq!(hs_to_rgb(120, 100), Rgb::new(0, 255, 0));
        assert_eq!(hs_to_rgb(240, 100), Rgb::new(0, 0, 255));
        assert_eq!(hs_to_rgb(0, 0), Rgb::new(255, 255, 255));
    }

    #[test]
    fn warm_white_is_not_blue() {
        let warm = ct_to_rgb(2700);
        assert!(warm.r > warm.b);
        let cool = ct_to_rgb(6500);
        assert!(cool.b > warm.b);
    }
}
