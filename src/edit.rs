//! Domains of explicit legacy Workbench UI edits, not a file-import filter.
//! Imported/unknown fields remain byte-preserving; see docs/parameter-domains.md.

pub const SYSTEM: u32 = 0x02010000;
pub const PICKUP1: u32 = 0x02010001;
pub const PICKUP2: u32 = 0x02010002;
pub const BODY: u32 = 0x02010003;
pub const WIRING: u32 = 0x02010004;
pub const TUNING: u32 = 0x02010005;

pub const RESISTANCES: &[f32] = &[25., 50., 100., 250., 470., 500., 1000.];
pub const CAPACITANCES: &[f32] = &[0.0011, 0.0022, 0.0047, 0.011, 0.022, 0.047, 0.1, -0.047];

#[derive(Clone, Copy, Debug)]
pub enum Domain {
    Float {
        min: f32,
        max: f32,
        unit: &'static str,
    },
    Integer {
        min: i32,
        max: i32,
    },
    Choices(&'static [f32]),
}

impl Domain {
    pub fn validate(self, value_type: u32, bits: u32) -> Result<(), String> {
        let valid = match self {
            Self::Integer { min, max } => value_type == 0 && (min..=max).contains(&(bits as i32)),
            Self::Float { min, max, .. } => {
                value_type == 1 && (min..=max).contains(&f32::from_bits(bits))
            }
            Self::Choices(choices) => value_type == 1 && choices.contains(&f32::from_bits(bits)),
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "value is outside the supported edit domain {self:?}"
            ))
        }
    }
}

pub fn domain(slot: u32, id: u32) -> Option<Domain> {
    use Domain::*;
    match (slot, id) {
        (PICKUP1 | PICKUP2, 0x200000) => Some(Float {
            min: 0.1,
            max: 9.5,
            unit: "in",
        }),
        (PICKUP1 | PICKUP2, 0x200001) => Some(Float {
            min: -45.,
            max: 45.,
            unit: "degrees",
        }),
        (PICKUP1 | PICKUP2, 0x200003) => Some(Float {
            min: -18.,
            max: 6.,
            unit: "dB",
        }),
        (PICKUP2, 0x200002 | 0x200004) | (WIRING, 0x200001 | 0x200005) => {
            Some(Integer { min: 0, max: 1 })
        }
        (WIRING, 0x200000 | 0x200002) => Some(Choices(RESISTANCES)),
        (WIRING, 0x200003) => Some(Choices(CAPACITANCES)),
        (WIRING, 0x200004) | (TUNING, 0x20000c..=0x200011) => Some(Float {
            min: 0.,
            max: 1.,
            unit: "fraction",
        }),
        (SYSTEM, 0x200000) => Some(Float {
            min: -20.,
            max: 6.,
            unit: "dB",
        }),
        (TUNING, 0x200000..=0x200005) => Some(Integer { min: -12, max: 12 }),
        (TUNING, 0x200006..=0x20000b) => Some(Float {
            min: -100.,
            max: 100.,
            unit: "cents",
        }),
        (TUNING, 0x200012) => Some(Float {
            min: 0.,
            max: 25.,
            unit: "stored spread",
        }),
        _ => None,
    }
}

pub fn is_acoustic(body: u32) -> bool {
    matches!(body, 0x03100012..=0x03100017 | 0x0310001a | 0x0310001b)
}

pub fn is_twelve_string(body: u32) -> bool {
    matches!(body, 0x0310000d | 0x03100013 | 0x03100015)
}

/// Original UI defaults for the legacy electric family, without the developer
/// ENABLE_ALL_12STR switch or the independently unsupported Tyler override.
pub fn availability(body: Option<u32>, slot: u32, id: Option<u32>) -> Result<(), String> {
    if let Some(body) = body {
        if is_acoustic(body) && matches!(slot, PICKUP1 | PICKUP2 | WIRING) {
            return Err(
                "virtual pickups and electrical controls are unavailable for acoustic bodies"
                    .into(),
            );
        }
        if body == 0x03100018 && slot == TUNING {
            return Err("alternate-tuning controls are unavailable for the sitar body".into());
        }
        if slot == TUNING
            && id.is_some_and(|id| (0x200006..=0x200012).contains(&id))
            && !is_twelve_string(body)
        {
            return Err(
                "twelve-string detune/mix/spread controls require a twelve-string body".into(),
            );
        }
    }
    Ok(())
}

/// The original UI catalog exposes 28 body and 32 pickup choices. Additional
/// native definition IDs are implementation resources, not selectable models.
pub fn model_allowed(slot: u32, id: u32) -> bool {
    match slot {
        BODY => (0x03100000..=0x0310001b).contains(&id),
        PICKUP1 | PICKUP2 => matches!(id, 0x03110000..=0x03110015 | 0x03118000..=0x03118009),
        _ => false,
    }
}
