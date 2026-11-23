//! PODxt Live tone actions for Device check.
//!
//! The family crate stays unchanged. Controller changes and a saved-tone
//! program change go through its public session, on the one write operation
//! that crate already names. Trying a change always puts the original values
//! back. Leaving a change on has its own gate and stays locked. Nothing here
//! stores a tone. A test build returns before a device is opened.

use std::time::Duration;

use line6_devices::family::podxt::{self, models};
use line6_devices::gate::{GatePolicy, Operation};
use line6_devices::identity::Identity;
use line6_devices::session::{Link, Request, Session, Wait};

use crate::hardware_gates::{self, Gate};

include!("podxt_live_packs.rs");

const PODXT_LIVE_MEMBER: u16 = 0x000a;
const WINDOW: Duration = Duration::from_secs(2);
const APPLY: Duration = Duration::from_millis(200);
const CC_GAP: Duration = Duration::from_millis(20);
const STORED_WINDOW: Duration = Duration::from_secs(3);
const STORED_GAP: Duration = Duration::from_millis(100);
const CHANNEL_LOAD: Duration = Duration::from_millis(400);
const LIVE_STATE_CCS: [u8; 5] = [7, 4, 17, 105, 21];

const SWITCH: u8 = 1;
const INVERTED: u8 = 2;
const ROUTING: u8 = 4;

struct Spec {
    name: &'static str,
    cc: u8,
    flags: u8,
}

const fn spec(name: &'static str, cc: u8, flags: u8) -> Spec {
    Spec { name, cc, flags }
}

const SPECS: [Spec; 55] = [
    spec("drive", 13, 0),
    spec("bass", 14, 0),
    spec("mid", 15, 0),
    spec("treble", 16, 0),
    spec("presence", 21, 0),
    spec("volume", 17, 0),
    spec("stomp-p2", 79, 0),
    spec("stomp-p3", 80, 0),
    spec("stomp-p4", 81, 0),
    spec("stomp-p5", 82, 0),
    spec("stomp-p6", 83, 0),
    spec("mod-p2", 52, 0),
    spec("mod-p3", 53, 0),
    spec("mod-p4", 54, 0),
    spec("mod-p5", 55, 0),
    spec("mod-mix", 56, 0),
    spec("delay-p2", 33, 0),
    spec("delay-p3", 35, 0),
    spec("delay-p4", 85, 0),
    spec("delay-p5", 86, 0),
    spec("delay-mix", 34, 0),
    spec("comp-threshold", 9, 0),
    spec("comp-gain", 5, 0),
    spec("gate-decay", 24, 0),
    spec("reverb-decay", 38, 0),
    spec("reverb-predelay", 40, 0),
    spec("reverb-tone", 39, 0),
    spec("reverb-mix", 18, 0),
    spec("eq-freq1", 20, 0),
    spec("eq-gain1", 114, 0),
    spec("eq-freq2", 42, 0),
    spec("eq-gain2", 116, 0),
    spec("eq-freq3", 60, 0),
    spec("eq-gain3", 117, 0),
    spec("eq-freq4", 77, 0),
    spec("eq-gain4", 119, 0),
    spec("room", 76, 0),
    spec("bypass-volume", 105, 0),
    spec("vol-pedal", 7, 0),
    spec("vol-pedal-min", 46, 0),
    spec("wah-position", 4, 0),
    spec("amp", 111, INVERTED),
    spec("stomp", 25, SWITCH),
    spec("comp", 26, SWITCH),
    spec("gate", 22, SWITCH),
    spec("mod", 50, SWITCH),
    spec("delay", 28, SWITCH),
    spec("reverb", 36, SWITCH),
    spec("wah", 43, SWITCH),
    spec("eq", 63, SWITCH),
    spec("amp-switch-mode", 84, SWITCH),
    spec("mod-prepost", 57, SWITCH | ROUTING),
    spec("delay-prepost", 87, SWITCH | ROUTING),
    spec("reverb-prepost", 41, SWITCH | ROUTING),
    spec("vol-pedal-prepost", 47, SWITCH | ROUTING),
];

/// One PODxt Live knob, switch or routing control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control(u8);

impl Control {
    pub const MID: Self = Self(2);
    pub const TREBLE: Self = Self(3);
    pub const EQ_GAIN1: Self = Self(29);
    pub const AMP: Self = Self(41);
    pub const STOMP: Self = Self(42);
    pub const MOD_ENABLE: Self = Self(45);
    pub const DELAY_ENABLE: Self = Self(46);
    pub const REVERB_ENABLE: Self = Self(47);
    pub const MOD_PREPOST: Self = Self(51);

    fn spec(self) -> &'static Spec {
        &SPECS[usize::from(self.0)]
    }

    pub fn name(self) -> &'static str {
        self.spec().name
    }

    pub fn cc(self) -> u8 {
        self.spec().cc
    }

    pub fn is_switch(self) -> bool {
        self.spec().flags & (SWITCH | INVERTED) != 0
    }

    pub fn is_routing(self) -> bool {
        self.spec().flags & ROUTING != 0
    }

    pub fn is_on(self, stored: u8) -> bool {
        if self.spec().flags & INVERTED != 0 {
            stored <= 63
        } else {
            stored > 63
        }
    }

    pub fn switch_value(self, on: bool) -> u8 {
        let inverted = self.spec().flags & INVERTED != 0;
        if on == inverted { 0 } else { 127 }
    }
}

/// What a basic control should become.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    On,
    Off,
    Toggle,
    /// Knob steps from where it is now. Bounces off the end so it always changes.
    Offset(i8),
}

/// Extra PODxt Live values: models, timing, mic and the selectors around them.
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

    fn cc(self) -> u8 {
        match self {
            Self::AmpModel => 12,
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

    fn low_cc(self) -> Option<u8> {
        match self {
            Self::ModSpeed => Some(61),
            Self::DelayTime => Some(62),
            Self::Tempo => Some(90),
            _ => None,
        }
    }

    pub fn parse(self, text: &str) -> Result<u16, String> {
        let number = text.parse::<u16>().ok();
        let value = match self {
            Self::Tempo => parse_tempo(text),
            Self::PedalAssign => number.and_then(|v| [0, 64, 127].get(usize::from(v)).copied()),
            Self::GateThreshold => text
                .parse::<i16>()
                .ok()
                .filter(|db| (-96..=0).contains(db))
                .map(|db| (-db) as u16),
            _ => number.filter(|v| self.valid_target(*v)),
        };
        value.ok_or_else(|| format!("{}={text} is not a value this check can send", self.name()))
    }

    pub fn valid_target(self, value: u16) -> bool {
        match self {
            Self::GateThreshold => value <= 96,
            Self::Mic => value < 4,
            Self::ModNote | Self::DelayNote | Self::TweakParam => value <= 13,
            Self::PedalAssign => matches!(value, 0 | 64 | 127),
            Self::ModSpeed | Self::DelayTime => value <= 16383,
            Self::Tempo => (300..=2400).contains(&value),
            _ => self
                .model_list()
                .is_some_and(|list| usize::from(value) < list.len()),
        }
    }

    fn check_model(self, index: u16, packs: podxt::Packs, role: &str) -> Result<(), String> {
        let Some(list) = self.model_list() else {
            return Ok(());
        };
        let id = *list.get(usize::from(index)).ok_or_else(|| {
            format!(
                "{}: {role} model index {index} is outside the list. Nothing was changed.",
                self.name()
            )
        })?;
        if let Some(bit) = required_pack(id).filter(|bit| packs.0 & bit == 0) {
            let pack = podxt::Packs::ALL
                .iter()
                .find(|(known, _)| *known == bit)
                .map_or("an unknown pack", |(_, name)| *name);
            return Err(format!(
                "{}: the {role} model needs {pack}, which is not installed. Nothing was changed.",
                self.name()
            ));
        }
        Ok(())
    }

    fn describe(self, value: u16) -> String {
        match self {
            Self::Mic => format!("mic {value}"),
            Self::Tempo => format!("{}.{} BPM", value / 10, value % 10),
            _ => match self
                .model_list()
                .and_then(|list| list.get(usize::from(value)))
            {
                Some(&id) => models::name(id).unwrap_or("unknown model").to_owned(),
                None => value.to_string(),
            },
        }
    }
}

fn required_pack(id: u32) -> Option<u8> {
    PACKS
        .iter()
        .find(|(known, _)| *known == id)
        .map(|(_, bit)| *bit)
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
    Some(whole * 10 + tenths)
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

    pub fn read(self, tone: &podxt::EditBuffer) -> u16 {
        let high = u16::from(tone.cc(self.cc()));
        match self.low_cc() {
            Some(low) => (high << 7) | u16::from(tone.cc(low)),
            None => high,
        }
    }

    pub fn describe(self, value: u16) -> String {
        match self {
            Self::Basic(control) if control.is_routing() => if control.is_on(value as u8) {
                "post"
            } else {
                "pre"
            }
            .into(),
            Self::Basic(control) if control.is_switch() => if control.is_on(value as u8) {
                "on"
            } else {
                "off"
            }
            .into(),
            Self::Basic(_) => value.to_string(),
            Self::Additional(setting) => setting.describe(value),
        }
    }

    fn matches(self, stored: u16, expected: u16) -> bool {
        match self {
            Self::Basic(control) if control.is_switch() => {
                control.is_on(stored as u8) == control.is_on(expected as u8)
            }
            Self::Additional(Setting::PedalAssign) => {
                pedal_bucket(stored) == pedal_bucket(expected)
            }
            _ => stored == expected,
        }
    }

    fn messages(self, value: u16) -> Vec<(u8, u8)> {
        let high = if self.low_cc().is_some() {
            (value >> 7) as u8
        } else {
            value as u8
        };
        let mut out = vec![(self.cc(), high)];
        if let Some(low) = self.low_cc() {
            out.push((low, (value & 0x7f) as u8));
        }
        out
    }

    fn order(self, value: u16) -> u8 {
        match self {
            Self::Additional(Setting::AmpModel) => 0,
            Self::Additional(setting) if setting.model_list().is_some() => 1,
            Self::Additional(Setting::Tempo) => 2,
            Self::Additional(Setting::ModNote | Setting::DelayNote) if value == 0 => 3,
            Self::Additional(Setting::ModNote | Setting::DelayNote) => 5,
            _ => 4,
        }
    }

    fn undo_companions(self) -> &'static [LiveControl] {
        const fn basic(index: u8) -> LiveControl {
            LiveControl::Basic(Control(index))
        }
        const AMP: [LiveControl; 7] = [
            basic(0),
            basic(1),
            basic(2),
            basic(3),
            basic(4),
            basic(5),
            basic(41),
        ];
        const CAB: [LiveControl; 2] = [LiveControl::Additional(Setting::Mic), basic(36)];
        const MOD_NOTE: [LiveControl; 1] = [LiveControl::Additional(Setting::ModNote)];
        const DELAY_NOTE: [LiveControl; 1] = [LiveControl::Additional(Setting::DelayNote)];
        match self {
            Self::Additional(Setting::AmpModel) => &AMP,
            Self::Additional(Setting::CabModel) => &CAB,
            Self::Additional(Setting::ModSpeed | Setting::ModNote) => &MOD_NOTE,
            Self::Additional(Setting::DelayTime | Setting::DelayNote) => &DELAY_NOTE,
            _ => &[],
        }
    }
}

fn pedal_bucket(value: u16) -> u16 {
    match value {
        0..=63 => 0,
        64..=126 => 1,
        _ => 2,
    }
}

/// One requested live change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveTarget {
    Basic(Control, Target),
    Additional(Setting, u16),
}

impl LiveTarget {
    fn control(self) -> LiveControl {
        match self {
            Self::Basic(control, _) => LiveControl::Basic(control),
            Self::Additional(setting, _) => LiveControl::Additional(setting),
        }
    }

    fn validate(self) -> Result<(), String> {
        if let Self::Additional(setting, value) = self
            && !setting.valid_target(value)
        {
            return Err(format!(
                "{}: {value} is not a value this check can send.",
                setting.name()
            ));
        }
        Ok(())
    }

    fn resolve(self, original: u16) -> u16 {
        match self {
            Self::Additional(_, value) => value,
            Self::Basic(control, target) => {
                let original = original as u8;
                u16::from(match target {
                    Target::On => control.switch_value(true),
                    Target::Off => control.switch_value(false),
                    Target::Toggle if control.is_switch() => {
                        control.switch_value(!control.is_on(original))
                    }
                    Target::Toggle => 127 - original,
                    Target::Offset(step) => offset_knob(original, step),
                })
            }
        }
    }
}

fn offset_knob(original: u8, step: i8) -> u8 {
    let moved = i16::from(original) + i16::from(step);
    let bounced = if (0..=127).contains(&moved) {
        moved
    } else {
        i16::from(original) - i16::from(step)
    };
    bounced.clamp(0, 127) as u8
}

/// The Device check try: EQ, routing, and a few knobs and switches. Not the stomp.
pub fn device_check_targets() -> Vec<LiveTarget> {
    vec![
        LiveTarget::Basic(Control::MOD_ENABLE, Target::Toggle),
        LiveTarget::Basic(Control::DELAY_ENABLE, Target::Toggle),
        LiveTarget::Basic(Control::REVERB_ENABLE, Target::Toggle),
        LiveTarget::Basic(Control::TREBLE, Target::Offset(20)),
        LiveTarget::Basic(Control::MID, Target::Offset(-20)),
        LiveTarget::Basic(Control::EQ_GAIN1, Target::Offset(10)),
        LiveTarget::Basic(Control::MOD_PREPOST, Target::Toggle),
    ]
}

#[derive(Clone, Copy, Debug)]
enum ToneRequest {
    Controller { cc: u8, value: u8 },
    Program { index: u8 },
}

impl Request for ToneRequest {
    fn bytes(self) -> Vec<u8> {
        match self {
            Self::Controller { cc, value } => vec![0xb0, cc, value],
            Self::Program { index } => vec![0xc0, index],
        }
    }

    fn path(self) -> line6_devices::session::Path {
        line6_devices::session::Path::PodxtMidi
    }

    fn operation(self) -> Operation {
        // The family crate has one write operation. The app gates decide
        // which of these messages a session may send.
        let _ = self;
        Operation::PodxtEditBufferWrite
    }
}

#[cfg(all(not(test), target_os = "linux"))]
struct TonePolicy {
    gate: Gate,
}

#[cfg(all(not(test), target_os = "linux"))]
impl GatePolicy for TonePolicy {
    fn allows(&self, operation: Operation) -> bool {
        if hardware_gates::is_locked(self.gate) {
            return false;
        }
        match self.gate {
            Gate::DeviceCheckLiveTest | Gate::KeepLiveEdit => matches!(
                operation,
                Operation::PodxtIdentity
                    | Operation::PodxtRead
                    | Operation::PodxtPackQuery
                    | Operation::PodxtEditBufferWrite
            ),
            Gate::DeviceCheckChannel => matches!(
                operation,
                Operation::PodxtIdentity
                    | Operation::PodxtRead
                    | Operation::PodxtLibraryRead
                    | Operation::PodxtEditBufferWrite
            ),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Change {
    control: LiveControl,
    original: u16,
    target: u16,
}

struct EditAttempt {
    before: podxt::EditBuffer,
    changes: Vec<Change>,
    sent: usize,
    attempted: usize,
    after: Result<podxt::EditBuffer, String>,
}

#[derive(Debug)]
pub struct LiveEditTest {
    pub before: podxt::EditBuffer,
    sent: usize,
    changes: Vec<Change>,
    after: Result<podxt::EditBuffer, String>,
    restored: Result<podxt::EditBuffer, String>,
}

impl LiveEditTest {
    pub fn stuck(&self) -> bool {
        self.sent == self.changes.len()
            && self.after.as_ref().is_ok_and(|after| {
                self.changes
                    .iter()
                    .all(|change| change_applied(change, after))
            })
    }

    pub fn undone(&self) -> bool {
        self.restored.as_ref().is_ok_and(|restored| {
            self.changes.iter().all(|change| {
                change
                    .control
                    .matches(change.control.read(restored), change.original)
            })
        })
    }
}

fn change_applied(change: &Change, after: &podxt::EditBuffer) -> bool {
    change
        .control
        .matches(change.control.read(after), change.target)
}

#[derive(Debug)]
pub enum LiveApply {
    Applied {
        before: podxt::EditBuffer,
        after: podxt::EditBuffer,
    },
    UndoAttempted(LiveEditTest),
}

/// Linux builds where the tone try or the saved-tone switch is open.
pub fn offered() -> bool {
    cfg!(target_os = "linux")
        && (!hardware_gates::is_locked(Gate::DeviceCheckLiveTest)
            || !hardware_gates::is_locked(Gate::DeviceCheckChannel))
}

/// Channel text such as `1A` or `32D`.
pub fn parse_channel(text: &str) -> Result<podxt::Location, String> {
    let text = text.trim().to_ascii_uppercase();
    let Some((number, letter)) = text.split_at_checked(text.len().saturating_sub(1)) else {
        return Err("Type a channel such as 1A or 32D.".into());
    };
    let letter = letter.as_bytes().first().copied().unwrap_or(0);
    let number = number.parse::<u16>().ok();
    let index = match (number, letter) {
        (Some(number), b'A'..=b'D') if (1..=32).contains(&number) => {
            (number - 1) * 4 + u16::from(letter - b'A')
        }
        _ => return Err("Type a channel such as 1A or 32D.".into()),
    };
    podxt::Location::new(podxt::BankKind::Tones, index)
        .ok_or_else(|| "Type a channel such as 1A or 32D.".into())
}

pub fn run_try(progress: impl FnMut(&str)) -> Result<String, String> {
    hardware_gates::run_if_unlocked(Gate::DeviceCheckLiveTest, || run_try_supported(progress))
}

fn run_try_supported(progress: impl FnMut(&str)) -> Result<String, String> {
    #[cfg(any(test, not(target_os = "linux")))]
    {
        let _ = progress;
        Err("this build does not open a device for a tone check; nothing was sent".into())
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        try_live(progress)
    }
}

pub fn run_channel(text: &str, progress: impl FnMut(&str)) -> Result<String, String> {
    let location = parse_channel(text)?;
    hardware_gates::run_if_unlocked(Gate::DeviceCheckChannel, || {
        run_channel_supported(location, progress)
    })
}

fn run_channel_supported(
    location: podxt::Location,
    progress: impl FnMut(&str),
) -> Result<String, String> {
    #[cfg(any(test, not(target_os = "linux")))]
    {
        let _ = (location, progress);
        Err("this build does not open a device for a tone check; nothing was sent".into())
    }
    #[cfg(all(not(test), target_os = "linux"))]
    {
        switch_live(location, progress)
    }
}

/// Change the active tone, read it back, then put the original values back.
pub fn live_settings_test<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<LiveEditTest, String> {
    hardware_gates::run_if_unlocked(Gate::DeviceCheckLiveTest, || test_open(session, targets))
}

/// Leave a verified edit in the active tone. This gate stays locked, so the
/// edit is not sent.
pub fn apply_live_settings<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<LiveApply, String> {
    hardware_gates::run_if_unlocked(Gate::KeepLiveEdit, || keep_verified(session, targets)).map_err(
        |error| {
            if hardware_gates::is_locked(Gate::KeepLiveEdit) {
                "Leaving a change on the PODxt stays off. Nothing was sent.".into()
            } else {
                error
            }
        },
    )
}

#[derive(Debug)]
pub struct ChannelSwitch {
    pub target: podxt::Location,
    pub after: Result<podxt::EditBuffer, String>,
    pub target_stored: podxt::EditBuffer,
}

impl ChannelSwitch {
    pub fn switched(&self) -> bool {
        self.after
            .as_ref()
            .is_ok_and(|after| same_tone(after, &self.target_stored))
    }
}

/// Select a saved tone. Refuses when the playing tone has unsaved edits.
/// `back` switches to the original channel again afterwards.
pub fn switch_channel<L: Link>(
    session: &mut Session<L>,
    target: podxt::Location,
    original: Option<podxt::Location>,
    back: bool,
    progress: impl FnMut(&str),
) -> Result<ChannelSwitch, String> {
    hardware_gates::run_if_unlocked(Gate::DeviceCheckChannel, || {
        switch_open(session, target, original, back, progress)
    })
}

fn test_open<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<LiveEditTest, String> {
    let attempt = edit_settings(session, targets)?;
    Ok(undo_attempt(session, attempt))
}

fn keep_verified<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<LiveApply, String> {
    let attempt = edit_settings(session, targets)?;
    let confirmed = attempt.sent == attempt.changes.len()
        && attempt.after.as_ref().is_ok_and(|after| {
            attempt
                .changes
                .iter()
                .all(|change| change_applied(change, after))
        });
    if confirmed {
        Ok(LiveApply::Applied {
            before: attempt.before,
            after: attempt.after.expect("confirmed"),
        })
    } else {
        Ok(LiveApply::UndoAttempted(undo_attempt(session, attempt)))
    }
}

fn edit_settings<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<EditAttempt, String> {
    let (before, changes) = prepare_changes(session, targets)?;
    send_changes(session, before, changes)
}

fn prepare_changes<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<(podxt::EditBuffer, Vec<Change>), String> {
    if targets.is_empty() {
        return Err("No changes requested.".into());
    }
    for (i, target) in targets.iter().enumerate() {
        target.validate()?;
        if targets[..i]
            .iter()
            .any(|other| other.control() == target.control())
        {
            return Err(format!("{} is listed twice.", target.control().name()));
        }
    }
    let (_, identity) = identify(session)?;
    if identity.member != PODXT_LIVE_MEMBER {
        return Err("This check needs the PODxt Live. Nothing was changed.".into());
    }
    let before = read_edit_buffer(session)?;
    let mut changes: Vec<Change> = targets
        .iter()
        .map(|target| {
            let control = target.control();
            let original = control.read(&before);
            Change {
                control,
                original,
                target: target.resolve(original),
            }
        })
        .collect();
    if changes.iter().any(|change| {
        change.control == LiveControl::Additional(Setting::Mic)
            && !Setting::Mic.valid_target(change.original)
    }) {
        return Err(
            "The original mic is outside the range that can be put back. Nothing was changed."
                .into(),
        );
    }
    if let [change] = changes.as_slice()
        && matches!(
            change.control,
            LiveControl::Additional(Setting::Mic) | LiveControl::Basic(Control(36))
        )
        && change.original == change.target
    {
        return Err(format!(
            "{} is already {}. Choose a different value.",
            change.control.name(),
            change.control.describe(change.target)
        ));
    }
    check_packs(session, &changes)?;
    changes.sort_by_key(|change| change.control.order(change.target));
    Ok((before, changes))
}

fn check_packs<L: Link>(session: &mut Session<L>, changes: &[Change]) -> Result<(), String> {
    let has_model = changes.iter().any(|change| {
        matches!(change.control, LiveControl::Additional(setting) if setting.model_list().is_some())
    });
    if !has_model {
        return Ok(());
    }
    let packs = query_packs(session)?;
    for change in changes {
        if let LiveControl::Additional(setting) = change.control {
            setting.check_model(change.target, packs, "requested")?;
            setting.check_model(change.original, packs, "original")?;
        }
    }
    Ok(())
}

fn send_changes<L: Link>(
    session: &mut Session<L>,
    before: podxt::EditBuffer,
    changes: Vec<Change>,
) -> Result<EditAttempt, String> {
    let mut sent = 0;
    let mut attempted = 0;
    let mut sent_bytes = Vec::new();
    let mut send_error = None;
    'changes: for change in &changes {
        attempted += 1;
        for (cc, value) in change.control.messages(change.target) {
            if let Err(error) = session.send(ToneRequest::Controller { cc, value }) {
                send_error = Some(error);
                break 'changes;
            }
            sent_bytes.push((cc, value));
            session.link.pause(CC_GAP);
        }
        sent += 1;
    }
    let after = match send_error {
        Some(error) => Err(error),
        None => read_after(session, &sent_bytes),
    };
    Ok(EditAttempt {
        before,
        changes,
        sent,
        attempted,
        after,
    })
}

fn undo_attempt<L: Link>(session: &mut Session<L>, attempt: EditAttempt) -> LiveEditTest {
    let EditAttempt {
        before,
        changes,
        sent,
        attempted,
        after,
    } = attempt;
    let mut undo: Vec<_> = changes[..attempted]
        .iter()
        .rev()
        .map(|change| change.control)
        .collect();
    undo.sort_by_key(|control| control.order(control.read(&before)));
    let sent_back = send_originals(session, &before, &undo);
    let mut restored = read_after(session, &sent_back);
    if let Ok(readback) = &restored {
        let companions = companions_to_repair(&before, readback, &undo);
        if !companions.is_empty() {
            let sent_back = send_originals(session, &before, &companions);
            restored = read_after(session, &sent_back);
        }
    }
    LiveEditTest {
        before,
        sent,
        changes,
        after,
        restored,
    }
}

fn companions_to_repair(
    before: &podxt::EditBuffer,
    readback: &podxt::EditBuffer,
    undo: &[LiveControl],
) -> Vec<LiveControl> {
    let mut companions = Vec::new();
    for control in undo {
        for companion in control.undo_companions() {
            if companion.read(readback) != companion.read(before) && !companions.contains(companion)
            {
                companions.push(*companion);
            }
        }
    }
    companions.sort_by_key(|control| control.order(control.read(before)));
    companions
}

fn send_originals<L: Link>(
    session: &mut Session<L>,
    before: &podxt::EditBuffer,
    controls: &[LiveControl],
) -> Vec<(u8, u8)> {
    let mut sent = Vec::new();
    for control in controls {
        for (cc, value) in control.messages(control.read(before)) {
            if session.send(ToneRequest::Controller { cc, value }).is_ok() {
                sent.push((cc, value));
            }
            session.link.pause(CC_GAP);
        }
    }
    sent
}

fn read_after<L: Link>(
    session: &mut Session<L>,
    sent: &[(u8, u8)],
) -> Result<podxt::EditBuffer, String> {
    let _ = sent;
    session.link.pause(APPLY);
    session.listen(Duration::from_millis(50))?;
    read_edit_buffer(session)
}

fn switch_open<L: Link>(
    session: &mut Session<L>,
    target: podxt::Location,
    original: Option<podxt::Location>,
    back: bool,
    mut progress: impl FnMut(&str),
) -> Result<ChannelSwitch, String> {
    if target.kind() != podxt::BankKind::Tones {
        return Err("Only tone channels can be selected.".into());
    }
    let (_, identity) = identify(session)?;
    if identity.member != PODXT_LIVE_MEMBER {
        return Err("This check needs the PODxt Live. Nothing was switched.".into());
    }
    let before = read_edit_buffer(session)?;
    let original = match original {
        Some(location) => {
            confirm_original(session, location, &before)?;
            location
        }
        None => find_channel(session, &before, &mut progress)?.ok_or(
            "The current tone matches no single saved tone, so it was left alone. Nothing was switched.",
        )?,
    };
    if original == target {
        return Err(format!(
            "The PODxt is already on channel {}. Nothing was switched.",
            target.label()
        ));
    }
    progress(&format!("Reading saved tone {}.", target.label()));
    let target_stored = read_stored(session, target)?;
    session.send(ToneRequest::Program {
        index: target.index() as u8,
    })?;
    session.link.pause(CHANNEL_LOAD);
    let _ = session.listen(Duration::from_millis(50))?;
    let after = read_edit_buffer(session);
    if back {
        let _ = session.send(ToneRequest::Program {
            index: original.index() as u8,
        });
        session.link.pause(CHANNEL_LOAD);
        let _ = session.listen(Duration::from_millis(50))?;
        let _ = read_edit_buffer(session)?;
    }
    Ok(ChannelSwitch {
        target,
        after,
        target_stored,
    })
}

fn confirm_original<L: Link>(
    session: &mut Session<L>,
    location: podxt::Location,
    before: &podxt::EditBuffer,
) -> Result<(), String> {
    if location.kind() != podxt::BankKind::Tones {
        return Err("Only tone channels can be selected.".into());
    }
    if !same_tone(&read_stored(session, location)?, before) {
        return Err(format!(
            "The current tone does not match saved tone {}, so it was left alone. Nothing was switched.",
            location.label()
        ));
    }
    Ok(())
}

fn find_channel<L: Link>(
    session: &mut Session<L>,
    playing: &podxt::EditBuffer,
    progress: &mut impl FnMut(&str),
) -> Result<Option<podxt::Location>, String> {
    let mut found = Vec::new();
    let total = podxt::BankKind::Tones.size();
    for location in podxt::Location::all(&[podxt::BankKind::Tones]) {
        progress(&format!(
            "Reading saved tone {} ({} of {total}).",
            location.label(),
            location.index() + 1
        ));
        if same_tone(&read_stored(session, location)?, playing) {
            found.push(location);
        }
    }
    Ok(if found.len() == 1 {
        Some(found[0])
    } else {
        None
    })
}

fn same_tone(a: &podxt::EditBuffer, b: &podxt::EditBuffer) -> bool {
    a.differences(b)
        .iter()
        .all(|(cc, ..)| LIVE_STATE_CCS.contains(cc))
}

fn identify<L: Link>(session: &mut Session<L>) -> Result<(Vec<u8>, Identity), String> {
    session.listen(Duration::from_millis(200))?;
    session.send(podxt::PodxtRequest::Identity)?;
    let Wait::Found(reply) =
        session.wait_for(WINDOW, |message| Identity::parse(message).is_some(), false)?
    else {
        return Err("No identity reply from the PODxt. Nothing else was sent.".into());
    };
    let identity = Identity::parse(&reply).expect("checked above");
    if !identity.is_line6() || identity.family != podxt::IDENTITY_FAMILY {
        return Err("The reply is not a PODxt. Nothing else was sent.".into());
    }
    Ok((reply, identity))
}

fn read_edit_buffer<L: Link>(session: &mut Session<L>) -> Result<podxt::EditBuffer, String> {
    session.send(podxt::PodxtRequest::EditBuffer)?;
    match session.wait_for(WINDOW, is_dump, false)? {
        Wait::Found(dump) => podxt::EditBuffer::parse(dump),
        _ => Err("No tone reply arrived.".into()),
    }
}

fn read_stored<L: Link>(
    session: &mut Session<L>,
    location: podxt::Location,
) -> Result<podxt::EditBuffer, String> {
    session.send(podxt::PodxtRequest::Stored(location))?;
    let result = match session.wait_for(STORED_WINDOW, is_dump, false)? {
        Wait::Found(dump) => podxt::EditBuffer::parse(dump),
        _ => Err(format!("No saved tone arrived for {}.", location.label())),
    };
    session.listen(STORED_GAP)?;
    result
}

fn query_packs<L: Link>(session: &mut Session<L>) -> Result<podxt::Packs, String> {
    session.send(podxt::PodxtRequest::InstalledPacks)?;
    match session.wait_for(WINDOW, is_packs, false)? {
        Wait::Found(reply) => Ok(podxt::Packs(reply[7])),
        _ => Err("The model packs were not read. Nothing was changed.".into()),
    }
}

fn is_dump(message: &[u8]) -> bool {
    message.len() == podxt::EDIT_BUFFER_LEN && message[..6] == [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x74]
}

fn is_packs(message: &[u8]) -> bool {
    message.len() == 9 && message[..6] == [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0e]
}

fn test_summary(test: &LiveEditTest) -> String {
    let name = test.before.name();
    if test.undone() && test.stuck() {
        format!("Tone {name}. The changes were accepted and then put back. Nothing was saved.")
    } else if test.undone() {
        format!(
            "Tone {name}. Some changes did not show on the read-back. What was sent was put back. Nothing was saved."
        )
    } else {
        format!(
            "Tone {name}. The original tone could not be confirmed. Check the PODxt before you play. Nothing was saved."
        )
    }
}

fn channel_summary(result: &ChannelSwitch) -> String {
    if result.switched() {
        let name = result
            .after
            .as_ref()
            .map(|tone| tone.name())
            .unwrap_or_else(|_| "unread".into());
        format!(
            "The PODxt is now on channel {}, tone {name}. Nothing was saved.",
            result.target.label()
        )
    } else {
        format!(
            "The PODxt did not switch to channel {}. Nothing was saved.",
            result.target.label()
        )
    }
}

#[cfg(all(not(test), target_os = "linux"))]
fn open_podxt() -> Result<line6_devices::transport::rawmidi::RawMidiLink, String> {
    use line6_devices::session::Path;
    use line6_devices::transport::rawmidi::{self, RawMidiLink};
    RawMidiLink::open(rawmidi::PODXT_LIVE_CARD, Path::PodxtMidi)
}

#[cfg(all(not(test), target_os = "linux"))]
fn try_live(mut progress: impl FnMut(&str)) -> Result<String, String> {
    use line6_devices::session::{Quiet, Session};
    progress("Opening the PODxt Live.");
    let mut link = open_podxt()?;
    let mut log = Quiet;
    let policy = TonePolicy {
        gate: Gate::DeviceCheckLiveTest,
    };
    let mut session = Session::new(&mut link, &policy, &mut log);
    progress("Reading the current tone, then changing it and putting it back.");
    let test = live_settings_test(&mut session, &device_check_targets())?;
    Ok(test_summary(&test))
}

#[cfg(all(not(test), target_os = "linux"))]
fn switch_live(
    location: podxt::Location,
    mut progress: impl FnMut(&str),
) -> Result<String, String> {
    use line6_devices::session::{Quiet, Session};
    progress("Opening the PODxt Live.");
    let mut link = open_podxt()?;
    let mut log = Quiet;
    let policy = TonePolicy {
        gate: Gate::DeviceCheckChannel,
    };
    let mut session = Session::new(&mut link, &policy, &mut log);
    let result = switch_channel(&mut session, location, None, false, &mut progress)?;
    Ok(channel_summary(&result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use line6_devices::session::{Path, Quiet, Session};
    use std::collections::VecDeque;

    struct OpenPolicy;

    impl GatePolicy for OpenPolicy {
        fn allows(&self, operation: Operation) -> bool {
            matches!(
                operation,
                Operation::PodxtIdentity
                    | Operation::PodxtRead
                    | Operation::PodxtLibraryRead
                    | Operation::PodxtPackQuery
                    | Operation::PodxtEditBufferWrite
            )
        }
    }

    struct Fake {
        clock: Duration,
        script: VecDeque<(Vec<u8>, Vec<Vec<u8>>)>,
        inbox: VecDeque<Vec<u8>>,
        writes: Vec<Vec<u8>>,
    }

    impl Fake {
        fn new(script: Vec<(Vec<u8>, Vec<Vec<u8>>)>) -> Self {
            Self {
                clock: Duration::ZERO,
                script: script.into(),
                inbox: VecDeque::new(),
                writes: Vec::new(),
            }
        }
    }

    impl Link for Fake {
        fn path(&self) -> Path {
            Path::PodxtMidi
        }
        fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.writes.push(bytes.to_vec());
            let Some((expected, replies)) = self.script.pop_front() else {
                return Err(format!("unexpected {}", line6_devices::midi::hex(bytes)));
            };
            if bytes != expected.as_slice() {
                return Err(format!(
                    "got {} wanted {}",
                    line6_devices::midi::hex(bytes),
                    line6_devices::midi::hex(&expected)
                ));
            }
            self.inbox.extend(replies);
            Ok(())
        }
        fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
            match self.inbox.pop_front() {
                Some(packet) => {
                    self.clock += Duration::from_millis(4);
                    Ok(Some(packet))
                }
                None => {
                    self.clock += wait;
                    Ok(None)
                }
            }
        }
        fn elapsed(&self) -> Duration {
            self.clock
        }
        fn pause(&mut self, duration: Duration) {
            self.clock += duration;
        }
    }

    fn identity() -> Vec<u8> {
        vec![
            0xf0, 0x7e, 0x7f, 0x06, 0x02, 0x00, 0x01, 0x0c, 0x03, 0x00, 0x0a, 0x00, 0x03, 0x00,
            0x01, 0x00, 0xf7,
        ]
    }

    fn tone(name: &str, ccs: &[(u8, u8)]) -> Vec<u8> {
        let mut raw = vec![0u8; podxt::EDIT_BUFFER_LEN];
        raw[..6].copy_from_slice(&[0xf0, 0x00, 0x01, 0x0c, 0x03, 0x74]);
        raw[podxt::EDIT_BUFFER_LEN - 1] = 0xf7;
        let bytes = name.as_bytes();
        raw[7..7 + bytes.len()].copy_from_slice(bytes);
        for &(cc, value) in ccs {
            raw[39 + usize::from(cc)] = value;
        }
        raw
    }

    fn script(items: Vec<(Vec<u8>, Vec<u8>)>) -> Fake {
        Fake::new(
            items
                .into_iter()
                .map(|(request, reply)| {
                    let replies = if reply.is_empty() {
                        Vec::new()
                    } else {
                        vec![reply]
                    };
                    (request, replies)
                })
                .collect(),
        )
    }

    #[test]
    fn the_try_does_not_touch_the_stomp_and_puts_values_back() {
        let before = tone(
            "Run",
            &[
                (Control::MID.cc(), 50),
                (Control::TREBLE.cc(), 40),
                (Control::EQ_GAIN1.cc(), 60),
                (Control::MOD_ENABLE.cc(), 0),
                (Control::DELAY_ENABLE.cc(), 127),
                (Control::REVERB_ENABLE.cc(), 0),
                (Control::MOD_PREPOST.cc(), 0),
                (Control::STOMP.cc(), 127),
            ],
        );
        let mut after = before.clone();
        let targets = device_check_targets();
        assert!(
            targets
                .iter()
                .all(|target| target.control().cc() != Control::STOMP.cc())
        );
        let mut requests = vec![
            (podxt::PodxtRequest::Identity.bytes(), identity()),
            (podxt::PodxtRequest::EditBuffer.bytes(), before.clone()),
        ];
        let parsed = podxt::EditBuffer::parse(before.clone()).unwrap();
        let mut changes: Vec<_> = targets
            .iter()
            .map(|target| {
                let original = target.control().read(&parsed);
                (target.control(), target.resolve(original))
            })
            .collect();
        changes.sort_by_key(|(control, value)| control.order(*value));
        for (control, value) in &changes {
            for (cc, byte) in control.messages(*value) {
                requests.push((vec![0xb0, cc, byte], Vec::new()));
                after[39 + usize::from(cc)] = byte;
            }
        }
        requests.push((podxt::PodxtRequest::EditBuffer.bytes(), after));
        for (control, _) in changes.iter().rev() {
            for (cc, byte) in control.messages(control.read(&parsed)) {
                requests.push((vec![0xb0, cc, byte], Vec::new()));
            }
        }
        requests.push((podxt::PodxtRequest::EditBuffer.bytes(), before.clone()));
        let mut log = Quiet;
        let mut session = Session::new(script(requests), &OpenPolicy, &mut log);
        let test = live_settings_test(&mut session, &targets).unwrap();
        assert!(test.stuck() && test.undone(), "{}", test_summary(&test));
        assert_eq!(test.restored.unwrap().cc(Control::STOMP.cc()), 127);
        assert!(session.link.script.is_empty());
    }

    #[test]
    fn keeping_a_change_sends_nothing() {
        let mut log = Quiet;
        let mut session = Session::new(Fake::new(vec![]), &OpenPolicy, &mut log);
        let error = apply_live_settings(
            &mut session,
            &[LiveTarget::Basic(Control::TREBLE, Target::Offset(1))],
        )
        .unwrap_err();
        assert!(error.contains("stays off"), "{error}");
        assert!(session.link.writes.is_empty());
    }

    #[test]
    fn tempo_mic_and_models_use_the_edit_bytes() {
        assert_eq!(Setting::Tempo.parse("113.5").unwrap(), 1135);
        assert_eq!(
            LiveControl::Additional(Setting::Tempo).messages(1135),
            vec![(89, 8), (90, 111)]
        );
        assert_eq!(Setting::Mic.parse("2").unwrap(), 2);
        assert_eq!(
            LiveControl::Additional(Setting::Mic).messages(2),
            vec![(70, 2)]
        );
        let index = models::AMP_MODELS
            .iter()
            .position(|id| *id == 0x00020020)
            .unwrap() as u16;
        assert_eq!(
            LiveControl::Additional(Setting::AmpModel).messages(index),
            vec![(12, index as u8)]
        );
        assert!(LiveControl::Basic(Control::MOD_PREPOST).describe(127) == "post");
    }

    #[test]
    fn a_missing_pack_blocks_the_model_before_any_change() {
        let before = tone("Run", &[(12, 0)]);
        let packs = vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0e, 0x00, 0x00, 0xf7];
        let bass = models::AMP_MODELS
            .iter()
            .position(|id| *id == 0x00030001)
            .unwrap() as u16;
        let requests = vec![
            (podxt::PodxtRequest::Identity.bytes(), identity()),
            (podxt::PodxtRequest::EditBuffer.bytes(), before),
            (podxt::PodxtRequest::InstalledPacks.bytes(), packs),
        ];
        let mut log = Quiet;
        let mut session = Session::new(script(requests), &OpenPolicy, &mut log);
        let error = live_settings_test(
            &mut session,
            &[LiveTarget::Additional(Setting::AmpModel, bass)],
        )
        .unwrap_err();
        assert!(error.contains("Bass Expansion"), "{error}");
        assert!(session.link.script.is_empty());
        assert!(session.link.writes.iter().all(|bytes| bytes[0] != 0xb0));
    }

    #[test]
    fn a_saved_tone_is_selected_only_when_the_current_tone_matches() {
        let here = tone("Here", &[(16, 10)]);
        let there = tone("There", &[(16, 40)]);
        let original = parse_channel("1A").unwrap();
        let target = parse_channel("1B").unwrap();
        let mut requests = vec![
            (podxt::PodxtRequest::Identity.bytes(), identity()),
            (podxt::PodxtRequest::EditBuffer.bytes(), here.clone()),
            (podxt::PodxtRequest::Stored(original).bytes(), here),
            (podxt::PodxtRequest::Stored(target).bytes(), there.clone()),
            (vec![0xc0, target.index() as u8], Vec::new()),
            (podxt::PodxtRequest::EditBuffer.bytes(), there),
        ];
        let mut log = Quiet;
        let mut session = Session::new(script(requests.clone()), &OpenPolicy, &mut log);
        let switched = switch_channel(&mut session, target, Some(original), false, |_| {}).unwrap();
        assert!(switched.switched(), "{}", channel_summary(&switched));
        assert!(session.link.script.is_empty());

        requests[2].1 = tone("Other", &[(16, 11)]);
        let mut log = Quiet;
        let mut session = Session::new(script(requests), &OpenPolicy, &mut log);
        let error =
            switch_channel(&mut session, target, Some(original), false, |_| {}).unwrap_err();
        assert!(error.contains("left alone"), "{error}");
        assert!(session.link.writes.iter().all(|bytes| bytes[0] != 0xc0));
    }

    #[test]
    fn channel_text_and_a_test_build_send_nothing() {
        assert_eq!(parse_channel(" 32d ").unwrap().label(), "32D");
        assert!(parse_channel("33A").is_err());
        assert!(parse_channel("1E").is_err());
        let error = run_try(|_| {}).unwrap_err();
        assert!(error.contains("nothing was sent"), "{error}");
        let error = run_channel("1A", |_| {}).unwrap_err();
        assert!(error.contains("nothing was sent"), "{error}");
        assert!(hardware_gates::is_locked(Gate::KeepLiveEdit));
        assert!(!hardware_gates::is_locked(Gate::DeviceCheckLiveTest));
        assert!(!hardware_gates::is_locked(Gate::DeviceCheckChannel));
        assert!(hardware_gates::is_locked(Gate::EditBufferWrite));
    }
}
