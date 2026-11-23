//! Additional PODxt Live edit-buffer encodings from Line 6 Edit 3.06.

use super::{Control, EditBuffer, Packs, PodxtRequest, Target, Value, models};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    AmpModel,
    CabModel,
    StompModel,
    ModModel,
    DelayModel,
    ReverbModel,
    WahModel,
    GateThreshold,
    Mic,
    ModNote,
    DelayNote,
    TweakParam,
    PedalAssign,
    ModSpeed,
    DelayTime,
    Tempo,
}

impl Setting {
    pub const ALL: [Self; 16] = [
        Self::AmpModel,
        Self::CabModel,
        Self::StompModel,
        Self::ModModel,
        Self::DelayModel,
        Self::ReverbModel,
        Self::WahModel,
        Self::GateThreshold,
        Self::Mic,
        Self::ModNote,
        Self::DelayNote,
        Self::TweakParam,
        Self::PedalAssign,
        Self::ModSpeed,
        Self::DelayTime,
        Self::Tempo,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::AmpModel => "amp-model",
            Self::CabModel => "cab-model",
            Self::StompModel => "stomp-model",
            Self::ModModel => "mod-model",
            Self::DelayModel => "delay-model",
            Self::ReverbModel => "reverb-model",
            Self::WahModel => "wah-model",
            Self::GateThreshold => "gate-threshold",
            Self::Mic => "mic",
            Self::ModNote => "mod-note",
            Self::DelayNote => "delay-note",
            Self::TweakParam => "tweak-param",
            Self::PedalAssign => "pedal-assign",
            Self::ModSpeed => "mod-speed",
            Self::DelayTime => "delay-time",
            Self::Tempo => "tempo",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|setting| setting.name() == name)
    }

    pub fn model_list(self) -> Option<&'static [u32]> {
        Some(match self {
            Self::AmpModel => &models::AMP_MODELS,
            Self::CabModel => &models::CAB_MODELS,
            Self::StompModel => &models::STOMP_MODELS,
            Self::ModModel => &models::MOD_MODELS,
            Self::DelayModel => &models::DELAY_MODELS,
            Self::ReverbModel => &models::REVERB_MODELS,
            Self::WahModel => &models::WAH_MODELS,
            _ => return None,
        })
    }

    // GetCCFromModelID uses the position in the family-specific list.
    pub fn model_index(self, id: u32) -> Option<u16> {
        let index = self.model_list()?.iter().position(|&model| model == id)?;
        Some(index as u16)
    }

    pub(super) fn cc(self) -> u8 {
        match self {
            Self::AmpModel => 12, // Line 6 Edit's amp selection with Load Amp Defaults off.
            Self::CabModel => 71,
            Self::StompModel => 75,
            Self::ModModel => 58,
            Self::DelayModel => 88,
            Self::ReverbModel => 37,
            Self::WahModel => 91,
            Self::GateThreshold => 23,
            Self::Mic => 70,
            Self::ModNote => 51,
            Self::DelayNote => 31,
            Self::TweakParam => 108,
            Self::PedalAssign => 65,
            Self::ModSpeed => 29,
            Self::DelayTime => 30,
            Self::Tempo => 89,
        }
    }

    pub(super) fn low_cc(self) -> Option<u8> {
        match self {
            Self::ModSpeed => Some(61),
            Self::DelayTime => Some(62),
            Self::Tempo => Some(90),
            _ => None,
        }
    }

    /// Encode a CLI value as it appears in the tone dump, combining pairs
    /// into one integer. Model choices are zero-based indices or hex IDs.
    pub fn parse(self, text: &str) -> Result<u16, String> {
        let number = || text.parse::<u16>().ok();
        let (encoded, expected) = match self {
            Self::GateThreshold => (
                text.parse::<i16>()
                    .ok()
                    .filter(|db| (-96..=0).contains(db))
                    .map(|db| (-db) as u16),
                "a whole number from -96 to 0 dB".to_owned(),
            ),
            Self::Mic => (
                number().filter(|v| *v <= 3),
                "a mic index from 0 to 3".into(),
            ),
            Self::ModNote | Self::DelayNote => (
                number().filter(|v| *v <= 13),
                "a NOTE index from 0 to 13 (0 is off)".into(),
            ),
            Self::TweakParam => (
                // PODxt Live manual, Appendix C: Tweak Parameter Destination 0..13.
                number().filter(|v| *v <= 13),
                "a tweak index from 0 to 13".into(),
            ),
            Self::PedalAssign => (
                number().and_then(|v| [0, 64, 127].get(usize::from(v)).copied()),
                "a pedal assignment from 0 to 2".into(),
            ),
            Self::ModSpeed | Self::DelayTime => (
                number().filter(|v| *v <= 16383),
                "a raw 14-bit value from 0 to 16383".into(),
            ),
            Self::Tempo => (
                parse_tempo(text),
                "30 to 240 BPM with at most one decimal place".into(),
            ),
            _ => {
                let max = self.model_list().expect("model setting").len() - 1;
                let value = match text.strip_prefix("0x") {
                    Some(hex) => u32::from_str_radix(hex, 16)
                        .ok()
                        .and_then(|id| self.model_index(id)),
                    None => number().filter(|index| usize::from(*index) <= max),
                };
                (
                    value,
                    format!("a model index from 0 to {max} or a hexadecimal model ID"),
                )
            }
        };
        encoded.ok_or_else(|| format!("{}={text}: use {expected}", self.name()))
    }

    fn valid_target(self, value: u16) -> bool {
        match self {
            Self::GateThreshold => value <= 96,
            Self::Mic => value <= 3,
            Self::ModNote | Self::DelayNote => value <= 13,
            Self::TweakParam => value <= 13,
            Self::PedalAssign => matches!(value, 0 | 64 | 127),
            Self::ModSpeed | Self::DelayTime => value <= 16383,
            Self::Tempo => (300..=2400).contains(&value),
            _ => self
                .model_list()
                .is_some_and(|list| usize::from(value) < list.len()),
        }
    }

    pub(super) fn check_model(self, index: u16, packs: Packs, role: &str) -> Result<(), String> {
        let Some(list) = self.model_list() else {
            return Ok(());
        };
        let id = *list.get(usize::from(index)).ok_or_else(|| {
            format!(
                "{}: {role} model index {index} is outside the PODxt Live list.",
                self.name()
            )
        })?;
        if let Some(required) = models::required_pack(id).filter(|bit| packs.0 & bit == 0) {
            let pack = Packs::ALL
                .iter()
                .find(|(bit, _)| *bit == required)
                .map_or("an unknown pack", |(_, name)| *name);
            return Err(format!(
                "{}: {role} model {} requires {pack}, which is inactive.",
                self.name(),
                self.describe(index)
            ));
        }
        Ok(())
    }

    fn describe(self, value: u16) -> String {
        match self {
            Self::GateThreshold => format!("{} dB", -i32::from(value)),
            Self::Tempo => format!("{}.{} BPM", value / 10, value % 10),
            Self::PedalAssign => format!("{} (raw {value})", pedal_assignment(value)),
            _ => match self
                .model_list()
                .and_then(|list| list.get(usize::from(value)))
            {
                Some(&id) => format!(
                    "{} (index {value}, ID {id:#010x})",
                    models::name(id).unwrap_or("unknown")
                ),
                None => value.to_string(),
            },
        }
    }
}

fn parse_tempo(text: &str) -> Option<u16> {
    let (whole, tenths) = match text.split_once('.') {
        Some((whole, fraction))
            if fraction.len() == 1 && fraction.as_bytes()[0].is_ascii_digit() =>
        {
            (whole, u16::from(fraction.as_bytes()[0] - b'0'))
        }
        Some(_) => return None,
        None => (text, 0),
    };
    let whole = whole
        .parse::<u16>()
        .ok()
        .filter(|n| (30..=240).contains(n))?;
    let value = whole * 10 + tenths;
    (value <= 2400).then_some(value)
}

fn pedal_assignment(value: u16) -> u16 {
    match value {
        0..=63 => 0,
        64..=126 => 1,
        _ => 2,
    }
}

/// A controller message derived from one checked additional setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingMessage {
    cc: u8,
    value: u8,
}

impl SettingMessage {
    pub(super) fn bytes(self) -> Vec<u8> {
        vec![0xb0, self.cc, self.value]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveControl {
    Basic(Control),
    Additional(Setting),
}

impl LiveControl {
    pub fn name(self) -> &'static str {
        match self {
            Self::Basic(control) => control.name(),
            Self::Additional(setting) => setting.name(),
        }
    }

    pub fn cc(self) -> u8 {
        match self {
            Self::Basic(control) => control.cc(),
            Self::Additional(setting) => setting.cc(),
        }
    }

    pub fn low_cc(self) -> Option<u8> {
        match self {
            Self::Basic(_) => None,
            Self::Additional(setting) => setting.low_cc(),
        }
    }

    pub fn read(self, tone: &EditBuffer) -> u16 {
        let high = u16::from(tone.cc(self.cc()));
        match self.low_cc() {
            Some(low) => (high << 7) | u16::from(tone.cc(low)),
            None => high,
        }
    }

    pub fn describe(self, value: u16) -> String {
        match self {
            Self::Basic(control) => control.describe(value as u8),
            Self::Additional(setting) => setting.describe(value),
        }
    }

    pub fn device_may_override(self) -> bool {
        matches!(self, Self::Basic(control) if control.device_may_override())
    }

    pub(super) fn matches(self, stored: u16, expected: u16) -> bool {
        match self {
            Self::Basic(control) if control.is_switch() => {
                control.is_on(stored as u8) == control.is_on(expected as u8)
            }
            Self::Additional(Setting::PedalAssign) => {
                pedal_assignment(stored) == pedal_assignment(expected)
            }
            _ => stored == expected,
        }
    }

    pub(super) fn messages(self, value: u16) -> impl Iterator<Item = (u8, u8, PodxtRequest)> {
        let high = if self.low_cc().is_some() {
            value >> 7
        } else {
            value
        } as u8;
        let low = self.low_cc().map(|cc| (cc, (value & 0x7f) as u8));
        [(self.cc(), high)]
            .into_iter()
            .chain(low)
            .map(move |(cc, value)| {
                let request = match self {
                    Self::Basic(control) => PodxtRequest::Set(control, Value(value)),
                    Self::Additional(_) => {
                        PodxtRequest::SetAdditional(SettingMessage { cc, value })
                    }
                };
                (cc, value, request)
            })
    }

    pub(super) fn undo_companions(self) -> &'static [Self] {
        use Control as C;
        use LiveControl::{Additional as A, Basic as B};
        use Setting as S;

        match self {
            A(S::AmpModel) => &[
                B(C::Drive),
                B(C::Bass),
                B(C::Mid),
                B(C::Treble),
                B(C::Presence),
                B(C::ChannelVolume),
                B(C::AmpEnable),
            ],
            A(S::CabModel) => &[A(S::Mic), B(C::Room)],
            A(S::StompModel) => &[
                B(C::StompEnable),
                B(C::StompParam2),
                B(C::StompParam3),
                B(C::StompParam4),
                B(C::StompParam5),
                B(C::StompParam6),
            ],
            A(S::ModModel) => &[
                B(C::ModEnable),
                B(C::ModPrePost),
                B(C::ModMix),
                A(S::ModSpeed),
                A(S::ModNote),
                B(C::ModParam2),
                B(C::ModParam3),
                B(C::ModParam4),
                B(C::ModParam5),
            ],
            A(S::DelayModel) => &[
                B(C::DelayEnable),
                B(C::DelayPrePost),
                B(C::DelayMix),
                A(S::DelayTime),
                A(S::DelayNote),
                B(C::DelayParam2),
                B(C::DelayParam3),
                B(C::DelayParam4),
                B(C::DelayParam5),
            ],
            A(S::ReverbModel) => &[
                B(C::ReverbEnable),
                B(C::ReverbPrePost),
                B(C::ReverbMix),
                B(C::ReverbDecay),
                B(C::ReverbPredelay),
                B(C::ReverbTone),
            ],
            A(S::WahModel) => &[B(C::WahEnable)],
            A(S::ModSpeed) => &[A(S::ModNote)],
            A(S::DelayTime) => &[A(S::DelayNote)],
            A(S::ModNote) => &[A(S::ModSpeed)],
            A(S::DelayNote) => &[A(S::DelayTime)],
            A(S::Tempo) => &[
                A(S::ModSpeed),
                A(S::DelayTime),
                A(S::ModNote),
                A(S::DelayNote),
            ],
            _ => &[],
        }
    }

    // Apply model and tempo changes before their dependent values. Turn
    // sync off before setting a manual rate, and restore sync after a rate.
    pub(super) fn order(self, value: u16) -> u8 {
        match self {
            Self::Additional(Setting::AmpModel) => 0,
            Self::Additional(setting) if setting.model_list().is_some() => 1,
            Self::Additional(Setting::Tempo) => 2,
            Self::Additional(Setting::ModNote | Setting::DelayNote) if value == 0 => 3,
            Self::Additional(Setting::ModNote | Setting::DelayNote) => 5,
            _ => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveTarget {
    Basic(Control, Target),
    Additional(Setting, u16),
}

impl LiveTarget {
    pub fn control(self) -> LiveControl {
        match self {
            Self::Basic(control, _) => LiveControl::Basic(control),
            Self::Additional(setting, _) => LiveControl::Additional(setting),
        }
    }

    pub(super) fn validate(self) -> Result<(), String> {
        match self {
            Self::Additional(setting, value) if !setting.valid_target(value) => {
                Err(format!("{}: invalid target {value}.", setting.name()))
            }
            _ => Ok(()),
        }
    }

    pub(super) fn resolve(self, original: u16) -> u16 {
        let (control, target) = match self {
            Self::Basic(control, target) => (control, target),
            Self::Additional(_, value) => return value,
        };
        let original = original as u8;
        u16::from(match target {
            Target::Value(value) => value.get(),
            Target::On => control.switch_value(true),
            Target::Off => control.switch_value(false),
            Target::Toggle if control.is_switch() => control.switch_value(!control.is_on(original)),
            Target::Toggle => 127 - original,
            Target::Offset(step) => {
                let moved = i16::from(original) + i16::from(step);
                let bounced = if (0..=127).contains(&moved) {
                    moved
                } else {
                    i16::from(original) - i16::from(step)
                };
                bounced.clamp(0, 127) as u8
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_id_is_encoded_as_its_index_on_each_model_controller() {
        let groups = [
            (Setting::AmpModel, 12, 107, 0x00020020, 22),
            (Setting::CabModel, 71, 47, 0x01020001, 1),
            (Setting::StompModel, 75, 31, 0x020a000a, 15),
            (Setting::ModModel, 58, 24, 0x02030005, 1),
            (Setting::DelayModel, 88, 14, 0x02020003, 1),
            (Setting::ReverbModel, 37, 15, 0x02040003, 1),
            (Setting::WahModel, 91, 8, 0x02060003, 1),
        ];
        for (setting, cc, len, example, example_index) in groups {
            assert_eq!(setting.model_index(example), Some(example_index));
            assert_eq!(setting.model_index(u32::MAX), None);
            let list = setting.model_list().unwrap();
            assert_eq!(list.len(), len);
            for (index, &id) in list.iter().enumerate() {
                let value = setting.model_index(id).unwrap();
                assert_eq!(usize::from(value), index);
                let mut messages = LiveControl::Additional(setting).messages(value);
                let (_, _, request) = messages.next().unwrap();
                use crate::session::Request;
                assert_eq!(request.bytes(), [0xb0, cc, index as u8]);
                assert!(messages.next().is_none());
            }
        }
    }

    #[test]
    fn raw_pairs_and_selectors_match_the_original_java_encoder() {
        use Setting::*;
        for (setting, input, expected) in [
            (GateThreshold, "-48", &[(23, 48)][..]),
            (Mic, "2", &[(70, 2)]),
            (ModNote, "4", &[(51, 4)]),
            (DelayNote, "8", &[(31, 8)]),
            (TweakParam, "9", &[(108, 9)]),
            (PedalAssign, "0", &[(65, 0)]),
            (PedalAssign, "1", &[(65, 64)]),
            (PedalAssign, "2", &[(65, 127)]),
            (ModSpeed, "4097", &[(29, 32), (61, 1)]),
            (DelayTime, "8193", &[(30, 64), (62, 1)]),
            (Tempo, "113.5", &[(89, 8), (90, 111)]),
        ] {
            let value = setting.parse(input).unwrap();
            assert!(setting.valid_target(value));
            assert_eq!(
                LiveControl::Additional(setting)
                    .messages(value)
                    .map(|(cc, value, _)| (cc, value))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
