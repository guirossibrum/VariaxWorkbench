//! Electric-model gain balancing recovered from native host orchestration.
#[derive(Clone, Copy, Debug)]
pub struct PickupLevel {
    pub model_offset_db: Option<f32>,
    pub level_db: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BalancedLevels {
    pub pickup_db: [f32; 2],
    pub trim_db: f32,
}
impl BalancedLevels {
    /// Position ordering preserves the original host's gain swap. This helper
    /// takes already-selected model offsets; Java model-combination selection
    /// and acoustic gain branches belong to the complete host pipeline.
    pub fn electric(
        first: PickupLevel,
        second: PickupLevel,
        combined_offset_db: Option<f32>,
        body_gain_db: f32,
        trim_db: f32,
        swap_positions: bool,
    ) -> Result<Self, String> {
        let mut offsets = [
            first.model_offset_db.unwrap_or(-1000.0),
            second.model_offset_db.unwrap_or(-1000.0),
        ];
        if let Some(combined) = combined_offset_db {
            offsets = [combined; 2];
        }
        if offsets
            .iter()
            .chain([first.level_db, second.level_db, body_gain_db, trim_db].iter())
            .any(|v| !v.is_finite())
        {
            return Err("nonfinite pickup levels".into());
        }
        let maximum = if offsets[1] <= offsets[0] {
            offsets[0]
        } else {
            offsets[1]
        };
        let mut pickup_db = [
            first.level_db + (offsets[0] - maximum),
            second.level_db + (offsets[1] - maximum),
        ];
        let mut adjusted_trim = ((maximum - 10.5) + trim_db) + body_gain_db;
        if adjusted_trim > 0.0 {
            pickup_db[0] += adjusted_trim;
            pickup_db[1] += adjusted_trim;
            adjusted_trim = 0.0;
        }
        if first.model_offset_db.is_some() && second.model_offset_db.is_some() && swap_positions {
            pickup_db.swap(0, 1);
        }
        Ok(Self {
            pickup_db,
            trim_db: adjusted_trim,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observed_electric_levels_and_trim_match_original_host() {
        for line in include_str!("../../tests/fixtures/native-pickup-levels-observed.txt").lines() {
            let f: Vec<_> = line.split_whitespace().collect();
            assert_eq!(f.len(), 11);
            let bits = |i| u32::from_str_radix(f[i], 16).unwrap();
            let value = |i| f32::from_bits(bits(i));
            let optional = |i| if f[i] == "none" { None } else { Some(value(i)) };
            let actual = BalancedLevels::electric(
                PickupLevel {
                    model_offset_db: optional(0),
                    level_db: value(2),
                },
                PickupLevel {
                    model_offset_db: optional(1),
                    level_db: value(3),
                },
                optional(4),
                value(5),
                value(6),
                f[7] == "1",
            )
            .unwrap();
            assert_eq!(actual.pickup_db[0].to_bits(), bits(8), "{line}");
            if f[9] != "unobserved" {
                assert_eq!(actual.pickup_db[1].to_bits(), bits(9), "{line}");
            }
            assert_eq!(actual.trim_db.to_bits(), bits(10), "{line}");
        }
    }
    #[test]
    fn nonfinite_levels_are_rejected() {
        let first = PickupLevel {
            model_offset_db: Some(0.),
            level_db: 0.,
        };
        let second = PickupLevel {
            model_offset_db: None,
            level_db: 0.,
        };
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(BalancedLevels::electric(first, second, Some(invalid), 0., 0., false).is_err());
            assert!(BalancedLevels::electric(first, second, None, invalid, 0., false).is_err());
            assert!(BalancedLevels::electric(first, second, None, 0., invalid, false).is_err());
        }
    }
}
