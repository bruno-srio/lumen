use crate::connection::EffectPreset;

/// `(count, stop action, flow expression)` for `start_cf`.
///
/// Stop action `1` keeps the last step. Color-only steps are rewritten as
/// color-temperature steps when the bulb has no RGB.
pub fn flow(preset: EffectPreset, temperature_only: bool) -> (i32, i32, String) {
    let steps = match preset {
        EffectPreset::Candle => {
            if temperature_only {
                vec![
                    (400, 2, 2000, 40),
                    (250, 2, 1700, 20),
                    (500, 2, 2200, 55),
                    (300, 2, 1900, 30),
                ]
            } else {
                vec![
                    (400, 1, 0xff6a00, 40),
                    (250, 1, 0xff4500, 18),
                    (500, 1, 0xff8c1a, 55),
                    (300, 1, 0xff5500, 28),
                ]
            }
        }
        EffectPreset::Sunrise => {
            if temperature_only {
                vec![(900_000, 2, 4500, 100)]
            } else {
                vec![(900_000, 2, 4500, 80), (60_000, 1, 0xffe0b0, 100)]
            }
        }
        EffectPreset::ColorLoop => {
            if temperature_only {
                vec![
                    (4000, 2, 2000, 70),
                    (4000, 2, 3500, 80),
                    (4000, 2, 5000, 90),
                    (4000, 2, 6500, 100),
                ]
            } else {
                vec![
                    (3000, 1, 0xff0000, 80),
                    (3000, 1, 0x00ff00, 80),
                    (3000, 1, 0x0000ff, 80),
                    (3000, 1, 0xff00ff, 80),
                ]
            }
        }
    };
    let expression = steps
        .into_iter()
        .map(|(duration, mode, value, bright)| {
            format!("{},{},{},{}", duration.max(50), mode, value, bright)
        })
        .collect::<Vec<_>>()
        .join(",");
    let count = match preset {
        EffectPreset::Sunrise => 1,
        EffectPreset::Candle | EffectPreset::ColorLoop => 0,
    };
    (count, 1, expression)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sunrise_is_a_single_long_step() {
        let (count, _action, expression) = flow(EffectPreset::Sunrise, true);
        assert_eq!(count, 1);
        assert!(expression.starts_with("900000,2,4500,100"));
    }

    #[test]
    fn color_loop_falls_back_to_temperature() {
        let (_count, _action, expression) = flow(EffectPreset::ColorLoop, true);
        assert!(expression.contains(",2,"));
        assert!(!expression.contains(",1,"));
    }
}
