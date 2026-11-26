//! PODxt family, reached through the device's own MIDI port.
//!
//! Requests and the edit-buffer layout come from Line 6 Edit 3.06
//! (`L6SharcTonePatchManager.RequestPatch`, `L6SharcToneMIDITranscoder.DecodePatch`),
//! confirmed by owner-run reads on 2026-11-22: name at bytes 7..23, the value
//! of MIDI controller `n` at byte `39 + n`, knobs `value / 127`, switches on
//! when `value > 63`, amp enable inverted (CC 111 set means amp off). Gate
//! threshold, compressor, delay time and tempo use other conversions and are
//! reported raw.

pub mod backup;
pub mod controls;
pub mod firmware_read;
pub mod models;
mod procedures;
mod store;
mod store_backup;
mod transfer;
pub mod transport;

mod live_edit;
mod live_read;
mod live_settings;
pub use live_edit::{
    EditorBuffer, EditorEdit, edit_editor_buffer, get_editor_buffer, playing_channel,
    recall_editor_channel,
};
pub use live_read::{
    ReadCommand, ReadUpdate, controller_notification, follow_action, follow_window, read_live,
    tuner_window,
};
pub use live_settings::{LiveControl, LiveTarget, Setting, SettingMessage};
pub use procedures::{check_live_identity, identify_podxt_live, read_stored_tone, restore_channel};
use procedures::{identify_podxt, read_edit_buffer, request_tone, send_control, wait_tone};
pub use store::{StoreControl, StoreResult, store_tone, store_tone_observing_wait};
pub use store_backup::{
    ConfirmedStore, PreparedStore, StoreSource, VerifiedToneBackup, prepare_tone_store,
    verify_tone_backup,
};
pub use transfer::{TonePayload, WholeToneSource, send_whole_tone};

use crate::gate::Operation;
use crate::identity::{self, Identity};
use crate::midi::ControllerTally;
use crate::session::{Link, Path, Request, Session, Wait};
use std::time::Duration;

pub const IDENTITY_FAMILY: u16 = 0x0003;
pub const PODXT_LIVE_MEMBER: u16 = 0x000a;
pub const PODXT_LIVE_VERSION: [u8; 4] = [0, 3, 0, 1];
/// Line 6 Edit waits 1 s for the edit buffer; this allows a little more.
const WINDOW: Duration = Duration::from_secs(2);
pub const EDIT_BUFFER_LEN: usize = 168;
pub const LISTEN_MAX: Duration = Duration::from_secs(30);

pub fn member_name(member: u16) -> &'static str {
    match member {
        PODXT_LIVE_MEMBER => "PODxt Live",
        _ => "PODxt family (unverified member)",
    }
}

/// The two factory Variax models selected for the readback test, from
/// Line 6 Edit's `VARIAX_FAMILY_0_MODEL` format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LinkedVariaxModel {
    FactoryTModel1 = 10,
    FactorySpank1 = 15,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodxtRequest {
    Identity,
    /// Edit-buffer (current tone) dump request, `03 75`.
    EditBuffer,
    /// Stored tone or setup dump request by memory location, `03 73 hi lo 00 00`
    /// (Line 6 Edit `RequestPatch(bank, index)`). Does not change the active tone.
    Stored(Location),
    /// Installed model-pack query, `03 0E 00` (Line 6 Edit
    /// `L6PODxtDevice.UpdateInstalledPacks`). Read-only.
    InstalledPacks,
    /// Workbench 1.75 `SetXTLiveVariaxMessagesEnable(false)`.
    ReleaseVariaxControl,
    /// Line 6 Edit 3.06 `WrapVariaxMessage` for the linked electric model.
    SetLinkedVariaxModel(LinkedVariaxModel),
    /// One live setting in the active tone: `B0 <cc> <value>`, exactly as
    /// Line 6 Edit `EncodeParamChange` sends a knob or switch change.
    /// Edit-buffer write (G6); never stored.
    Set(Control, Value),
    /// One half or the entirety of an additional PODxt Live setting.
    SetAdditional(SettingMessage),
    /// Select a stored tone as the playing channel: `C0 <index>` (Line 6
    /// Edit `ChangeDeviceProgram`, `m_pc_offset` 0 on the PODxt Live).
    Program(Location),
    /// Global value query, `03 57 <id>` (Line 6 Edit `QueryGlobalValue`).
    QueryGlobal(Global),
    /// Tuner on/off: CC 69, 127 or 0 (Line 6 Edit `SetTunerMode`).
    TunerMode(bool),
}

impl Request for PodxtRequest {
    fn bytes(self) -> Vec<u8> {
        match self {
            PodxtRequest::Identity => identity::REQUEST.to_vec(),
            PodxtRequest::EditBuffer => vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x75, 0xf7],
            PodxtRequest::InstalledPacks => vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0e, 0x00, 0xf7],
            PodxtRequest::ReleaseVariaxControl => {
                vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x23, 0x00, 0xf7]
            }
            PodxtRequest::SetLinkedVariaxModel(model) => {
                vec![0xf0, 0, 1, 0x0c, 3, 0x10, 0, 120, model as u8, 0xf7]
            }
            PodxtRequest::Set(control, value) => vec![0xb0, control.cc(), value.get()],
            PodxtRequest::SetAdditional(message) => message.bytes(),
            PodxtRequest::Program(location) => vec![0xc0, location.index() as u8],
            PodxtRequest::QueryGlobal(global) => {
                vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x57, global.id(), 0xf7]
            }
            PodxtRequest::TunerMode(on) => vec![0xb0, TUNER_CC, if on { 127 } else { 0 }],
            PodxtRequest::Stored(location) => {
                let address = location.address();
                vec![
                    0xf0,
                    0x00,
                    0x01,
                    0x0c,
                    0x03,
                    0x73,
                    (address >> 7) as u8,
                    (address & 0x7f) as u8,
                    0x00,
                    0x00,
                    0xf7,
                ]
            }
        }
    }

    fn path(self) -> Path {
        Path::PodxtMidi
    }

    fn operation(self) -> Operation {
        match self {
            PodxtRequest::Identity => Operation::PodxtIdentity,
            PodxtRequest::EditBuffer => Operation::PodxtRead,
            PodxtRequest::Stored(_) => Operation::PodxtLibraryRead,
            PodxtRequest::InstalledPacks => Operation::PodxtPackQuery,
            PodxtRequest::Set(..) => Operation::PodxtEditBufferWrite,
            PodxtRequest::SetAdditional(_) => Operation::PodxtAdditionalEditBufferWrite,
            PodxtRequest::Program(_) => Operation::PodxtChannelSwitch,
            PodxtRequest::ReleaseVariaxControl => Operation::PodxtVariaxControlRelease,
            PodxtRequest::SetLinkedVariaxModel(_) => Operation::PodxtLinkedVariaxModelEdit,
            PodxtRequest::QueryGlobal(_) => Operation::PodxtGlobalRead,
            PodxtRequest::TunerMode(_) => Operation::PodxtTunerMode,
        }
    }
}

/// Stomp enable controller (`CC_STOMP_ENABLE`).
pub const STOMP_ENABLE_CC: u8 = 25;
/// Time for the PODxt to apply controller changes before reading back.
const APPLY: Duration = Duration::from_millis(200);
/// Gap between consecutive controller messages.
const CC_GAP: Duration = Duration::from_millis(20);

/// How Line 6 Edit encodes a live control's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Knob: the 0..1 value scaled to 0..=127 (`Float2CC`).
    Knob,
    /// Switch: on = 127, off = 0.
    Switch,
    /// Switch sent as `127 - value` (amp enable: on = 0, off = 127).
    InvertedSwitch,
}

/// Live settings a front end may change (closed list). Every entry is one
/// controller message in Line 6 Edit 3.06 `EncodeParamChange` with the value
/// encoding of its [`Kind`], and is present on the PODxt Live (guitar). Not
/// here on purpose: the additional encodings in [`Setting`], bass-PODxt-only
/// controls, Variax-wrapped controls forwarded to the guitar, and the FX
/// loop switch (CC 107): the PODxt
/// Live has no effects loop and ignored it in an owner test (no echo).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    Drive,
    Bass,
    Mid,
    Treble,
    Presence,
    ChannelVolume,
    StompParam2,
    StompParam3,
    StompParam4,
    StompParam5,
    StompParam6,
    ModParam2,
    ModParam3,
    ModParam4,
    ModParam5,
    ModMix,
    DelayParam2,
    DelayParam3,
    DelayParam4,
    DelayParam5,
    DelayMix,
    CompThreshold,
    CompGain,
    GateDecay,
    ReverbDecay,
    ReverbPredelay,
    ReverbTone,
    ReverbMix,
    EqFreq1,
    EqGain1,
    EqFreq2,
    EqGain2,
    EqFreq3,
    EqGain3,
    EqFreq4,
    EqGain4,
    Room,
    BypassVolume,
    VolumePedal,
    VolumePedalMin,
    WahPosition,
    AmpEnable,
    StompEnable,
    CompEnable,
    GateEnable,
    ModEnable,
    DelayEnable,
    ReverbEnable,
    WahEnable,
    EqEnable,
    AmpSwitchMode,
    ModPrePost,
    DelayPrePost,
    ReverbPrePost,
    VolumePedalPrePost,
}

impl Control {
    pub const ALL: [Control; 55] = [
        Control::Drive,
        Control::Bass,
        Control::Mid,
        Control::Treble,
        Control::Presence,
        Control::ChannelVolume,
        Control::StompParam2,
        Control::StompParam3,
        Control::StompParam4,
        Control::StompParam5,
        Control::StompParam6,
        Control::ModParam2,
        Control::ModParam3,
        Control::ModParam4,
        Control::ModParam5,
        Control::ModMix,
        Control::DelayParam2,
        Control::DelayParam3,
        Control::DelayParam4,
        Control::DelayParam5,
        Control::DelayMix,
        Control::CompThreshold,
        Control::CompGain,
        Control::GateDecay,
        Control::ReverbDecay,
        Control::ReverbPredelay,
        Control::ReverbTone,
        Control::ReverbMix,
        Control::EqFreq1,
        Control::EqGain1,
        Control::EqFreq2,
        Control::EqGain2,
        Control::EqFreq3,
        Control::EqGain3,
        Control::EqFreq4,
        Control::EqGain4,
        Control::Room,
        Control::BypassVolume,
        Control::VolumePedal,
        Control::VolumePedalMin,
        Control::WahPosition,
        Control::AmpEnable,
        Control::StompEnable,
        Control::CompEnable,
        Control::GateEnable,
        Control::ModEnable,
        Control::DelayEnable,
        Control::ReverbEnable,
        Control::WahEnable,
        Control::EqEnable,
        Control::AmpSwitchMode,
        Control::ModPrePost,
        Control::DelayPrePost,
        Control::ReverbPrePost,
        Control::VolumePedalPrePost,
    ];

    /// Controller number (Line 6 Edit `CC_*` constants, PODxt Live mapping).
    pub fn cc(self) -> u8 {
        match self {
            Control::Drive => 13,
            Control::Bass => 14,
            Control::Mid => 15,
            Control::Treble => 16,
            Control::Presence => 21,
            Control::ChannelVolume => 17,
            Control::StompParam2 => 79,
            Control::StompParam3 => 80,
            Control::StompParam4 => 81,
            Control::StompParam5 => 82,
            Control::StompParam6 => 83,
            Control::ModParam2 => 52,
            Control::ModParam3 => 53,
            Control::ModParam4 => 54,
            Control::ModParam5 => 55,
            Control::ModMix => 56,
            Control::DelayParam2 => 33,
            Control::DelayParam3 => 35,
            Control::DelayParam4 => 85,
            Control::DelayParam5 => 86,
            Control::DelayMix => 34,
            Control::CompThreshold => 9,
            Control::CompGain => 5,
            Control::GateDecay => 24,
            Control::ReverbDecay => 38,
            Control::ReverbPredelay => 40,
            Control::ReverbTone => 39,
            Control::ReverbMix => 18,
            Control::EqFreq1 => 20,
            Control::EqGain1 => 114,
            Control::EqFreq2 => 42,
            Control::EqGain2 => 116,
            Control::EqFreq3 => 60,
            Control::EqGain3 => 117,
            Control::EqFreq4 => 77,
            Control::EqGain4 => 119,
            Control::Room => 76,
            Control::BypassVolume => 105,
            Control::VolumePedal => 7,
            Control::VolumePedalMin => 46,
            Control::WahPosition => 4,
            Control::AmpEnable => 111,
            Control::StompEnable => 25,
            Control::CompEnable => 26,
            Control::GateEnable => 22,
            Control::ModEnable => 50,
            Control::DelayEnable => 28,
            Control::ReverbEnable => 36,
            Control::WahEnable => 43,
            Control::EqEnable => 63,
            Control::AmpSwitchMode => 84,
            Control::ModPrePost => 57,
            Control::DelayPrePost => 87,
            Control::ReverbPrePost => 41,
            Control::VolumePedalPrePost => 47,
        }
    }

    pub fn kind(self) -> Kind {
        match self {
            Control::Drive => Kind::Knob,
            Control::Bass => Kind::Knob,
            Control::Mid => Kind::Knob,
            Control::Treble => Kind::Knob,
            Control::Presence => Kind::Knob,
            Control::ChannelVolume => Kind::Knob,
            Control::StompParam2 => Kind::Knob,
            Control::StompParam3 => Kind::Knob,
            Control::StompParam4 => Kind::Knob,
            Control::StompParam5 => Kind::Knob,
            Control::StompParam6 => Kind::Knob,
            Control::ModParam2 => Kind::Knob,
            Control::ModParam3 => Kind::Knob,
            Control::ModParam4 => Kind::Knob,
            Control::ModParam5 => Kind::Knob,
            Control::ModMix => Kind::Knob,
            Control::DelayParam2 => Kind::Knob,
            Control::DelayParam3 => Kind::Knob,
            Control::DelayParam4 => Kind::Knob,
            Control::DelayParam5 => Kind::Knob,
            Control::DelayMix => Kind::Knob,
            Control::CompThreshold => Kind::Knob,
            Control::CompGain => Kind::Knob,
            Control::GateDecay => Kind::Knob,
            Control::ReverbDecay => Kind::Knob,
            Control::ReverbPredelay => Kind::Knob,
            Control::ReverbTone => Kind::Knob,
            Control::ReverbMix => Kind::Knob,
            Control::EqFreq1 => Kind::Knob,
            Control::EqGain1 => Kind::Knob,
            Control::EqFreq2 => Kind::Knob,
            Control::EqGain2 => Kind::Knob,
            Control::EqFreq3 => Kind::Knob,
            Control::EqGain3 => Kind::Knob,
            Control::EqFreq4 => Kind::Knob,
            Control::EqGain4 => Kind::Knob,
            Control::Room => Kind::Knob,
            Control::BypassVolume => Kind::Knob,
            Control::VolumePedal => Kind::Knob,
            Control::VolumePedalMin => Kind::Knob,
            Control::WahPosition => Kind::Knob,
            Control::AmpEnable => Kind::InvertedSwitch,
            Control::StompEnable => Kind::Switch,
            Control::CompEnable => Kind::Switch,
            Control::GateEnable => Kind::Switch,
            Control::ModEnable => Kind::Switch,
            Control::DelayEnable => Kind::Switch,
            Control::ReverbEnable => Kind::Switch,
            Control::WahEnable => Kind::Switch,
            Control::EqEnable => Kind::Switch,
            Control::AmpSwitchMode => Kind::Switch,
            Control::ModPrePost => Kind::Switch,
            Control::DelayPrePost => Kind::Switch,
            Control::ReverbPrePost => Kind::Switch,
            Control::VolumePedalPrePost => Kind::Switch,
        }
    }

    pub fn is_switch(self) -> bool {
        self.kind() != Kind::Knob
    }

    pub fn name(self) -> &'static str {
        match self {
            Control::Drive => "drive",
            Control::Bass => "bass",
            Control::Mid => "mid",
            Control::Treble => "treble",
            Control::Presence => "presence",
            Control::ChannelVolume => "volume",
            Control::StompParam2 => "stomp-p2",
            Control::StompParam3 => "stomp-p3",
            Control::StompParam4 => "stomp-p4",
            Control::StompParam5 => "stomp-p5",
            Control::StompParam6 => "stomp-p6",
            Control::ModParam2 => "mod-p2",
            Control::ModParam3 => "mod-p3",
            Control::ModParam4 => "mod-p4",
            Control::ModParam5 => "mod-p5",
            Control::ModMix => "mod-mix",
            Control::DelayParam2 => "delay-p2",
            Control::DelayParam3 => "delay-p3",
            Control::DelayParam4 => "delay-p4",
            Control::DelayParam5 => "delay-p5",
            Control::DelayMix => "delay-mix",
            Control::CompThreshold => "comp-threshold",
            Control::CompGain => "comp-gain",
            Control::GateDecay => "gate-decay",
            Control::ReverbDecay => "reverb-decay",
            Control::ReverbPredelay => "reverb-predelay",
            Control::ReverbTone => "reverb-tone",
            Control::ReverbMix => "reverb-mix",
            Control::EqFreq1 => "eq-freq1",
            Control::EqGain1 => "eq-gain1",
            Control::EqFreq2 => "eq-freq2",
            Control::EqGain2 => "eq-gain2",
            Control::EqFreq3 => "eq-freq3",
            Control::EqGain3 => "eq-gain3",
            Control::EqFreq4 => "eq-freq4",
            Control::EqGain4 => "eq-gain4",
            Control::Room => "room",
            Control::BypassVolume => "bypass-volume",
            Control::VolumePedal => "vol-pedal",
            Control::VolumePedalMin => "vol-pedal-min",
            Control::WahPosition => "wah-position",
            Control::AmpEnable => "amp",
            Control::StompEnable => "stomp",
            Control::CompEnable => "comp",
            Control::GateEnable => "gate",
            Control::ModEnable => "mod",
            Control::DelayEnable => "delay",
            Control::ReverbEnable => "reverb",
            Control::WahEnable => "wah",
            Control::EqEnable => "eq",
            Control::AmpSwitchMode => "amp-switch-mode",
            Control::ModPrePost => "mod-prepost",
            Control::DelayPrePost => "delay-prepost",
            Control::ReverbPrePost => "reverb-prepost",
            Control::VolumePedalPrePost => "vol-pedal-prepost",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|control| control.name() == name)
    }

    /// The PODxt can overwrite this setting from its own controls. Owner
    /// tests 2026-11-22/23: the Channel Volume knob re-sends its position to
    /// whichever volume is active (channel volume with the amp on, bypass
    /// volume with the amp off), and the PODxt sent Presence = 0 by itself
    /// during large change sequences (cause not identified).
    pub fn device_may_override(self) -> bool {
        matches!(
            self,
            Control::ChannelVolume | Control::BypassVolume | Control::Presence
        )
    }

    /// Whether a stored value means "on" (switches only).
    pub fn is_on(self, stored: u8) -> bool {
        match self.kind() {
            Kind::InvertedSwitch => stored <= 63,
            _ => stored > 63,
        }
    }

    /// Raw value for "on" / "off".
    pub fn switch_value(self, on: bool) -> u8 {
        match (self.kind(), on) {
            (Kind::InvertedSwitch, true) | (Kind::Switch | Kind::Knob, false) => 0,
            _ => 127,
        }
    }

    pub fn is_routing(self) -> bool {
        matches!(
            self,
            Self::ModPrePost | Self::DelayPrePost | Self::ReverbPrePost | Self::VolumePedalPrePost
        )
    }

    /// A stored value as people read it: routing, on/off, or knob position.
    pub fn describe(self, stored: u8) -> String {
        if self.is_routing() {
            (if self.is_on(stored) { "post" } else { "pre" }).to_owned()
        } else if self.is_switch() {
            (if self.is_on(stored) { "on" } else { "off" }).to_owned()
        } else {
            format!("{stored} ({:.1}%)", f32::from(stored) / 127.0 * 100.0)
        }
    }
}

/// A 7-bit controller value. Only 0..=127 can be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value(u8);

impl Value {
    pub fn new(value: u8) -> Option<Self> {
        (value <= 127).then_some(Self(value))
    }

    pub fn switch(on: bool) -> Self {
        Self(if on { 127 } else { 0 })
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// What to set a control to; resolved against the tone read first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Value(Value),
    On,
    Off,
    Toggle,
    /// Knob moved by this many steps from where it is (bounced back off the
    /// end of the range, so it always changes).
    Offset(i8),
}

/// One requested change, resolved: `original` is what the tone held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub control: LiveControl,
    pub original: u16,
    pub target: u16,
}

/// A group of verified live edits left active in the PODxt Live edit buffer.
pub struct VerifiedLiveEdits {
    pub identity_reply: Vec<u8>,
    pub before: EditBuffer,
    pub after: EditBuffer,
    pub changes: Vec<Change>,
    /// Values received from the device besides the requested controller echoes.
    pub device_sent: Vec<(u8, u8)>,
}

/// Outcome of applying a group of live edits without storing the tone.
pub enum LiveApply {
    Applied(VerifiedLiveEdits),
    /// The forward change could not be confirmed. Inspect the readback and
    /// exact undo result before showing the active tone as restored.
    UndoAttempted(LiveEditTest),
}

struct EditAttempt {
    identity_reply: Vec<u8>,
    before: EditBuffer,
    changes: Vec<Change>,
    sent: usize,
    attempted: usize,
    after: Result<EditBuffer, String>,
    device_sent_after_changes: Vec<(u8, u8)>,
}

fn change_applied(change: &Change, after: &EditBuffer) -> bool {
    change
        .control
        .matches(change.control.read(after), change.target)
}

/// Result of a live-edit test.
pub struct LiveEditTest {
    pub identity_reply: Vec<u8>,
    pub before: EditBuffer,
    pub changes: Vec<Change>,
    /// How many changes were actually sent before any send error.
    pub sent: usize,
    pub after: Result<EditBuffer, String>,
    pub undo_sent: Result<(), String>,
    pub restored: Result<EditBuffer, String>,
    /// Controller messages the PODxt sent that were not echoes of what was
    /// just sent, as `(cc, value)`: after the changes and after the undo.
    pub device_sent_after_changes: Vec<(u8, u8)>,
    pub device_sent_after_undo: Vec<(u8, u8)>,
}

/// Controllers in `tally` that are not a plain echo of `sent` (cc, value).
fn device_originated(tally: &ControllerTally, sent: &[(u8, u8)]) -> Vec<(u8, u8)> {
    tally
        .controllers
        .iter()
        .flat_map(|((_, cc), t)| {
            let echoes = sent.iter().filter(|(s, _)| s == cc).count();
            let expected = sent.iter().rev().find(|(s, _)| s == cc).map(|(_, v)| *v);
            // Every distinct value seen (first/last/min/max) that is not what
            // was sent; for a controller that was not sent at all, its last.
            let mut values: Vec<u8> = match expected {
                None => vec![t.last],
                Some(_) if t.count <= echoes && expected == Some(t.last) => Vec::new(),
                Some(sent) => [t.first, t.last, t.min, t.max]
                    .into_iter()
                    .filter(|v| *v != sent)
                    .collect(),
            };
            values.sort_unstable();
            values.dedup();
            values.into_iter().map(move |v| (*cc, v))
        })
        .collect()
}

impl LiveEditTest {
    /// Each change, and whether the tone read afterwards shows it.
    pub fn applied(&self) -> Vec<(Change, bool)> {
        self.changes
            .iter()
            .map(|change| {
                let ok = self
                    .after
                    .as_ref()
                    .is_ok_and(|after| change_applied(change, after));
                (*change, ok)
            })
            .collect()
    }

    /// Every change reached the active tone.
    pub fn stuck(&self) -> bool {
        self.sent == self.changes.len() && self.applied().iter().all(|(_, ok)| *ok)
    }

    /// Controllers that changed although they were not asked to (for
    /// example a physical knob drifting): `(cc, before, after)`.
    pub fn unexpected(&self) -> Vec<(u8, u8, u8)> {
        let asked: Vec<u8> = self
            .changes
            .iter()
            .flat_map(|c| [c.control.cc()].into_iter().chain(c.control.low_cc()))
            .collect();
        match &self.after {
            Ok(after) => self
                .before
                .differences(after)
                .into_iter()
                .filter(|(cc, ..)| !asked.contains(cc))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Every controller that differs between the original tone and the tone
    /// after the undo, including side effects the device made on its own:
    /// `(cc, before, after undo)`.
    pub fn not_restored(&self) -> Vec<(u8, u8, u8)> {
        match &self.restored {
            Ok(restored) => self.before.differences(restored),
            Err(_) => Vec::new(),
        }
    }

    /// The full edit-buffer reply matches its snapshot before the test.
    pub fn exact_undo(&self) -> bool {
        self.undo_sent.is_ok()
            && self
                .restored
                .as_ref()
                .is_ok_and(|tone| self.before.raw == tone.raw)
    }

    /// Every changed control holds its original value again.
    pub fn undone(&self) -> bool {
        self.undo_sent.is_ok()
            && self.restored.as_ref().is_ok_and(|restored| {
                self.changes
                    .iter()
                    .all(|c| c.control.matches(c.control.read(restored), c.original))
            })
    }
}

/// G6 live-edit test for the previously verified single-controller controls.
pub fn live_edit_test<L: Link>(
    session: &mut Session<L>,
    targets: &[(Control, Target)],
) -> Result<LiveEditTest, String> {
    let targets: Vec<_> = targets
        .iter()
        .map(|&(control, target)| LiveTarget::Basic(control, target))
        .collect();
    live_settings_test(session, &targets)
}

/// Apply an explicitly selected group of G6 changes to the edit buffer,
/// read back, restore the original values, and read back again.
pub fn live_settings_test<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<LiveEditTest, String> {
    let attempt = edit_settings(session, targets)?;
    Ok(undo_edit_attempt(session, attempt))
}

/// Set active PODxt Live controls and retain the edit after readback confirms
/// every requested value. A failed or partial application attempts automatic
/// undo and returns the undo report. All hardware access remains subject to
/// the session policy. Keep permission is separate from permission to test
/// with undo. This operation never stores the tone in a channel.
pub fn apply_live_settings<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<LiveApply, String> {
    if !session.allows(Operation::PodxtKeepEditBufferChanges) {
        return Err("Keeping PODxt Live edit-buffer changes is closed in this build.".into());
    }
    match edit_settings(session, targets)? {
        EditAttempt {
            identity_reply,
            before,
            changes,
            sent,
            after: Ok(after),
            device_sent_after_changes,
            ..
        } if sent == changes.len()
            && changes.iter().all(|change| change_applied(change, &after)) =>
        {
            Ok(LiveApply::Applied(VerifiedLiveEdits {
                identity_reply,
                before,
                after,
                changes,
                device_sent: device_sent_after_changes,
            }))
        }
        attempt => Ok(LiveApply::UndoAttempted(undo_edit_attempt(
            session, attempt,
        ))),
    }
}

fn edit_settings<L: Link>(
    session: &mut Session<L>,
    targets: &[LiveTarget],
) -> Result<EditAttempt, String> {
    if targets.is_empty() {
        return Err("No changes requested.".into());
    }
    let additional = targets
        .iter()
        .any(|target| matches!(target, LiveTarget::Additional(..)));
    if additional && !session.allows(Operation::PodxtAdditionalEditBufferWrite) {
        return Err("Additional PODxt Live edits are closed in this build.".into());
    }
    if targets
        .iter()
        .any(|target| matches!(target, LiveTarget::Basic(..)))
        && !session.allows(Operation::PodxtEditBufferWrite)
    {
        return Err("PODxt edit-buffer writes are closed in this build.".into());
    }
    for (i, &target) in targets.iter().enumerate() {
        target.validate()?;
        let control = target.control();
        if targets[..i].iter().any(|other| other.control() == control) {
            return Err(format!("{} is listed twice.", control.name()));
        }
    }

    let identity_reply = identify_podxt_live(session)?;
    let before = read_edit_buffer(session)?;
    let mut changes: Vec<Change> = targets
        .iter()
        .map(|&target| {
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
            "The original mic index is outside the supported range for undo. No edits were sent."
                .into(),
        );
    }
    if let [change] = changes.as_slice()
        && matches!(
            change.control,
            LiveControl::Additional(Setting::Mic) | LiveControl::Basic(Control::Room)
        )
        && change.original == change.target
    {
        return Err(format!(
            "{} is already {}. Choose a different value to test a change.",
            change.control.name(),
            change.control.describe(change.target)
        ));
    }

    let has_model = changes.iter().any(
        |change| matches!(change.control, LiveControl::Additional(setting) if setting.model_list().is_some()),
    );
    if has_model {
        let packs = query_packs(session)?;
        for change in &changes {
            if let LiveControl::Additional(setting) = change.control {
                setting.check_model(change.target, packs, "requested")?;
                setting.check_model(change.original, packs, "original (needed for undo)")?;
            }
        }
    }
    changes.sort_by_key(|change| change.control.order(change.target));

    let mut sent = 0;
    let mut attempted = 0;
    let mut sent_forward = Vec::new();
    let mut send_error = None;
    for change in &changes {
        attempted += 1;
        if let Err(error) = send_control(session, change.control, change.target, |cc, value| {
            sent_forward.push((cc, value));
        }) {
            send_error = Some(error);
            break;
        }
        sent += 1;
    }
    let mut device_sent_after_changes = Vec::new();
    let after = match send_error {
        Some(error) => Err(error),
        None => check_live_settings(session, &sent_forward, &mut device_sent_after_changes),
    };

    Ok(EditAttempt {
        identity_reply,
        before,
        changes,
        sent,
        attempted,
        after,
        device_sent_after_changes,
    })
}

fn undo_edit_attempt<L: Link>(session: &mut Session<L>, attempt: EditAttempt) -> LiveEditTest {
    let EditAttempt {
        identity_reply,
        before,
        changes,
        sent,
        attempted,
        after,
        device_sent_after_changes,
    } = attempt;
    // A failed send can have reached the device. Undo even the partially
    // transmitted setting, with 14-bit values always sent high byte first.
    let mut undo: Vec<_> = changes[..attempted]
        .iter()
        .rev()
        .map(|change| change.control)
        .collect();
    undo.sort_by_key(|control| control.order(control.read(&before)));
    let (mut undo_sent, sent_back) = undo_settings(session, &before, &undo);
    let mut device_sent_after_undo = Vec::new();
    let mut restored = check_live_settings(session, &sent_back, &mut device_sent_after_undo);

    if let Ok(readback) = &restored {
        // A model or timing change can also affect settings in the same
        // block. Reapply only changed companions whose encoding is known.
        let mut companions = Vec::new();
        for control in undo {
            for &companion in control.undo_companions() {
                if companion.read(readback) != companion.read(&before)
                    && !companions.contains(&companion)
                {
                    companions.push(companion);
                }
            }
        }
        for (timing, note) in [
            (Setting::ModSpeed, Setting::ModNote),
            (Setting::DelayTime, Setting::DelayNote),
        ] {
            let note = LiveControl::Additional(note);
            if companions.contains(&LiveControl::Additional(timing))
                && note.read(&before) != 0
                && !companions.contains(&note)
            {
                companions.push(note);
            }
        }
        if !companions.is_empty() {
            companions.sort_by_key(|control| control.order(control.read(&before)));
            let (repair_sent, sent_back) = undo_settings(session, &before, &companions);
            undo_sent = undo_sent.and(repair_sent);
            restored = check_live_settings(session, &sent_back, &mut device_sent_after_undo);
        }
    }
    LiveEditTest {
        identity_reply,
        before,
        changes,
        sent,
        after,
        undo_sent,
        restored,
        device_sent_after_changes,
        device_sent_after_undo,
    }
}

fn undo_settings<L: Link>(
    session: &mut Session<L>,
    before: &EditBuffer,
    controls: &[LiveControl],
) -> (Result<(), String>, Vec<(u8, u8)>) {
    let mut status = Ok(());
    let mut sent = Vec::new();
    for &control in controls {
        for (cc, value, request) in control.messages(control.read(before)) {
            match session.send(request) {
                Ok(()) => sent.push((cc, value)),
                Err(error) => status = Err(error),
            }
            session.link.pause(CC_GAP);
        }
    }
    (status, sent)
}

fn check_live_settings<L: Link>(
    session: &mut Session<L>,
    sent: &[(u8, u8)],
    device_sent: &mut Vec<(u8, u8)>,
) -> Result<EditBuffer, String> {
    session.link.pause(APPLY);
    let tally = session.listen(Duration::from_millis(50))?;
    device_sent.extend(device_originated(&tally, sent));
    read_edit_buffer(session)
}

/// Tuner mode controller (Line 6 Edit `SetTunerMode`).
pub const TUNER_CC: u8 = 69;

/// Global values the PODxt answers to `03 57 <id>` (Line 6 Edit
/// `L6SharcToneDevice.GLOBAL_*`). Values are raw 16-bit; their meaning is
/// only decoded where Line 6 Edit decodes it (tuner note and deviation,
/// current channel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Global {
    Version,
    ClipStatus,
    UsbMix,
    UsbMode,
    MidiChannel,
    CurrentChannel,
    UiMode,
    TunerMode,
    TunerLevel,
    TunerReference,
    TunerNote,
    TunerDeviation,
    ProgramChangeScheme,
    MidiThru,
    Edited,
    FavoriteChannel,
    LcdContrast,
    OutputMode,
    FxSetup,
    PotsMode,
    InputSelect,
}

impl Global {
    pub const STATUS: [Global; 21] = [
        Global::Version,
        Global::ClipStatus,
        Global::UsbMix,
        Global::UsbMode,
        Global::MidiChannel,
        Global::CurrentChannel,
        Global::UiMode,
        Global::TunerMode,
        Global::TunerLevel,
        Global::TunerReference,
        Global::TunerNote,
        Global::TunerDeviation,
        Global::ProgramChangeScheme,
        Global::MidiThru,
        Global::Edited,
        Global::FavoriteChannel,
        Global::LcdContrast,
        Global::OutputMode,
        Global::FxSetup,
        Global::PotsMode,
        Global::InputSelect,
    ];

    pub fn id(self) -> u8 {
        match self {
            Global::Version => 0,
            Global::ClipStatus => 1,
            Global::UsbMix => 4,
            Global::UsbMode => 5,
            Global::MidiChannel => 16,
            Global::CurrentChannel => 17,
            Global::UiMode => 18,
            Global::TunerMode => 19,
            Global::TunerLevel => 20,
            Global::TunerReference => 21,
            Global::TunerNote => 22,
            Global::TunerDeviation => 23,
            Global::ProgramChangeScheme => 26,
            Global::MidiThru => 27,
            Global::Edited => 28,
            Global::FavoriteChannel => 29,
            Global::LcdContrast => 30,
            Global::OutputMode => 31,
            Global::FxSetup => 32,
            Global::PotsMode => 33,
            Global::InputSelect => 34,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Global::Version => "version",
            Global::ClipStatus => "clip status",
            Global::UsbMix => "USB mix",
            Global::UsbMode => "USB mode",
            Global::MidiChannel => "MIDI channel",
            Global::CurrentChannel => "current channel (patch number)",
            Global::UiMode => "UI mode",
            Global::TunerMode => "tuner mode",
            Global::TunerLevel => "tuner level",
            Global::TunerReference => "tuner reference",
            Global::TunerNote => "tuner note",
            Global::TunerDeviation => "tuner deviation",
            Global::ProgramChangeScheme => "program change scheme",
            Global::MidiThru => "MIDI thru",
            Global::Edited => "edited (unsaved changes)",
            Global::FavoriteChannel => "favorite channel",
            Global::LcdContrast => "LCD contrast",
            Global::OutputMode => "output mode",
            Global::FxSetup => "FX setup",
            Global::PotsMode => "pots (knob) mode",
            Global::InputSelect => "input select",
        }
    }
}

/// A raw global value as Line 6 Edit reads it: signed 16-bit.
pub fn signed(raw: u16) -> i32 {
    i32::from(raw as i16)
}

/// Tuner note name from the raw note value (Line 6 Edit `GetTunerNote` and
/// `L6TunerDisplay.NOTE_NAMES`); "-" when no note is detected.
pub fn tuner_note_name(raw: u16) -> &'static str {
    const NAMES: [&str; 13] = [
        "-", "A", "Bb", "B", "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab",
    ];
    let value = signed(raw);
    if value <= -3 {
        NAMES[0]
    } else {
        NAMES[((value + 2).rem_euclid(12) + 1) as usize]
    }
}

/// Tuner deviation in cents. Line 6 Edit's `GetTunerDeviation` returns
/// `signed / 100` and its display maps -0.5..+0.5 onto the whole needle
/// (`20 + 335 * (deviation + 0.5)`), i.e. half a semitone each way, so the
/// raw value is the deviation in cents.
pub fn tuner_cents(raw: u16) -> f32 {
    signed(raw) as f32
}

fn global_value(message: &[u8], id: u8) -> Option<u16> {
    (message.len() == 12 && message[..5] == [0xf0, 0x00, 0x01, 0x0c, 0x03] && message[6] == id)
        .then(|| {
            (u16::from(message[7]) << 12)
                | (u16::from(message[8]) << 8)
                | (u16::from(message[9]) << 4)
                | u16::from(message[10])
        })
}

/// One global value, or `None` when the PODxt does not answer in time.
pub fn read_global<L: Link>(
    session: &mut Session<L>,
    global: Global,
) -> Result<Option<u16>, String> {
    session.send(PodxtRequest::QueryGlobal(global))?;
    let id = global.id();
    Ok(
        match session.wait_for(GLOBAL_WINDOW, |m| global_value(m, id).is_some(), false)? {
            Wait::Found(reply) => global_value(&reply, id),
            _ => None,
        },
    )
}

/// Line 6 Edit waits 100 ms for a global value; this allows a little more.
const GLOBAL_WINDOW: Duration = Duration::from_millis(300);
/// Line 6 Edit polls the tuner every 250 ms.
pub const TUNER_POLL: Duration = Duration::from_millis(250);
pub const TUNER_MAX: Duration = Duration::from_secs(60);

/// Global values as read: `None` when the PODxt did not answer.
pub type GlobalValues = Vec<(Global, Option<u16>)>;

/// Identity, then every [`Global::STATUS`] value. Read-only.
pub fn read_status<L: Link>(session: &mut Session<L>) -> Result<(Vec<u8>, GlobalValues), String> {
    let (identity_reply, _) = identify_podxt(session)?;
    let mut values = Vec::new();
    for global in Global::STATUS {
        values.push((global, read_global(session, global)?));
    }
    Ok((identity_reply, values))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TunerReading {
    pub at: Duration,
    pub note: Option<u16>,
    pub deviation: Option<u16>,
}

pub struct TunerRun {
    pub identity_reply: Vec<u8>,
    pub readings: Vec<TunerReading>,
    /// Result of switching the tuner off again (always attempted).
    pub off_sent: Result<(), String>,
}

/// Tuner on (CC 69 = 127), poll note and deviation every 250 ms for
/// `window` (at most [`TUNER_MAX`]), then tuner off, which is sent even when
/// polling fails.
pub fn tuner<L: Link>(
    session: &mut Session<L>,
    window: Duration,
    mut on_reading: impl FnMut(&TunerReading),
) -> Result<TunerRun, String> {
    let identity_reply = identify_podxt_live(session)?;
    let window = window.min(TUNER_MAX);
    if let Err(error) = session.send(PodxtRequest::TunerMode(true)) {
        let _ = session.send(PodxtRequest::TunerMode(false));
        return Err(error);
    }
    let start = session.link.elapsed();
    let mut readings = Vec::new();
    let mut poll_error = None;
    while session.link.elapsed().saturating_sub(start) < window {
        let reading = (|| -> Result<TunerReading, String> {
            let note = read_global(session, Global::TunerNote)?;
            let deviation = read_global(session, Global::TunerDeviation)?;
            Ok(TunerReading {
                at: session.link.elapsed().saturating_sub(start),
                note,
                deviation,
            })
        })();
        match reading {
            Ok(reading) => {
                on_reading(&reading);
                readings.push(reading);
            }
            Err(error) => {
                poll_error = Some(error);
                break;
            }
        }
        session.link.pause(TUNER_POLL);
    }
    let off_sent = session.send(PodxtRequest::TunerMode(false));
    if let Some(error) = poll_error {
        return Err(match off_sent {
            Ok(()) => format!("{error} (tuner switched off)"),
            Err(off) => format!("{error}; switching the tuner off also failed: {off}"),
        });
    }
    Ok(TunerRun {
        identity_reply,
        readings,
        off_sent,
    })
}

/// Bytes that follow live physical state rather than the stored tone: volume
/// pedal (CC 7), wah position (CC 4), and the knob/device-driven channel
/// volume (CC 17), bypass volume (CC 105) and presence (CC 21).
pub const LIVE_STATE_CCS: [u8; 5] = [7, 4, 17, 105, 21];

impl EditBuffer {
    /// Same stored tone, ignoring [`LIVE_STATE_CCS`].
    pub fn same_tone(&self, other: &EditBuffer) -> bool {
        self.name() == other.name()
            && self
                .differences(other)
                .iter()
                .all(|(cc, ..)| LIVE_STATE_CCS.contains(cc))
    }
}

fn read_stored<L: Link>(
    session: &mut Session<L>,
    location: Location,
) -> Result<EditBuffer, String> {
    read_stored_tone(session, location)?
        .ok_or_else(|| format!("No dump for stored tone {}.", location.label()))
}

/// Which stored tone channel the playing tone is (exactly one match among
/// the 128 tones, read by location), or `None`.
pub fn find_channel<L: Link>(
    session: &mut Session<L>,
    playing: &EditBuffer,
) -> Result<Option<Location>, String> {
    let mut found = Vec::new();
    for location in Location::all(&[BankKind::Tones]) {
        if read_stored(session, location)?.same_tone(playing) {
            found.push(location);
        }
    }
    Ok(if found.len() == 1 {
        Some(found[0])
    } else {
        None
    })
}

/// Time for the PODxt to load a channel after a program change.
const CHANNEL_LOAD: Duration = Duration::from_millis(400);

pub struct ChannelSwitch {
    pub identity_reply: Vec<u8>,
    pub before: EditBuffer,
    /// The channel the playing tone was on (when `back` was requested).
    pub original: Option<Location>,
    pub target: Location,
    pub target_stored: EditBuffer,
    pub after: Result<EditBuffer, String>,
    /// Present when switching back was requested.
    pub back: Option<Result<EditBuffer, String>>,
}

impl ChannelSwitch {
    pub fn switched(&self) -> bool {
        self.after
            .as_ref()
            .is_ok_and(|after| after.same_tone(&self.target_stored))
    }

    pub fn returned(&self) -> bool {
        matches!(&self.back, Some(Ok(back)) if back.same_tone(&self.before))
    }
}

/// Program change to `target` (a stored tone), with checks. Refuses when
/// the playing tone has unsaved edits (switching would discard them): it
/// must match the stored tone of its channel, found by reading all 128 tones
/// or given as `original`. With `back`, switches back to the original
/// channel afterwards and checks the playing tone is the one from before.
pub fn switch_channel<L: Link>(
    session: &mut Session<L>,
    target: Location,
    original: Option<Location>,
    back: bool,
) -> Result<ChannelSwitch, String> {
    switch_channel_checked(session, target, original, back, |_, _| Ok(()))
}

/// Run a final check of the playing and destination tones before program change.
pub fn switch_channel_checked<L: Link>(
    session: &mut Session<L>,
    target: Location,
    original: Option<Location>,
    back: bool,
    before_switch: impl FnOnce(&EditBuffer, &EditBuffer) -> Result<(), String>,
) -> Result<ChannelSwitch, String> {
    if target.kind() != BankKind::Tones || original.is_some_and(|o| o.kind() != BankKind::Tones) {
        return Err("Only tone channels can be selected.".into());
    }
    let identity_reply = identify_podxt_live(session)?;
    let before = read_edit_buffer(session)?;
    let original = match original {
        Some(location) => {
            if !read_stored(session, location)?.same_tone(&before) {
                return Err(format!(
                    "The playing tone does not match stored tone {}: it has unsaved edits or is another channel. Nothing was switched.",
                    location.label()
                ));
            }
            Some(location)
        }
        None => match find_channel(session, &before)? {
            Some(location) => Some(location),
            None => {
                return Err("The playing tone matches no single stored tone (unsaved edits?). Switching would discard them. Nothing was switched.".into());
            }
        },
    };
    let target_stored = read_stored(session, target)?;
    before_switch(&before, &target_stored)?;
    session.send(PodxtRequest::Program(target))?;
    session.link.pause(CHANNEL_LOAD);
    session.listen(Duration::from_millis(50))?;
    let after = read_edit_buffer(session);
    let back = if back {
        let to = original.expect("set above");
        Some(session.send(PodxtRequest::Program(to)).and_then(|()| {
            session.link.pause(CHANNEL_LOAD);
            session.listen(Duration::from_millis(50))?;
            read_edit_buffer(session)
        }))
    } else {
        None
    };
    Ok(ChannelSwitch {
        identity_reply,
        before,
        original,
        target,
        target_stored,
        after,
        back,
    })
}

/// The first G6 test: toggle the stomp, check, undo.
pub fn stomp_test<L: Link>(session: &mut Session<L>) -> Result<LiveEditTest, String> {
    live_edit_test(session, &[(Control::StompEnable, Target::Toggle)])
}

/// Installed model packs: a bitmask from the device (`L6PODxtDevice.XTPACKS_*`).
/// A pack decides which amp/effect models the unit can run; stored tones may
/// still refer to models from packs that are not installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packs(pub u8);

impl Packs {
    pub const ALL: [(u8, &'static str); 4] = [
        (1, "Metal Shop"),
        (2, "Collector Classics"),
        (4, "FX Junkie"),
        (8, "Bass Expansion"),
    ];

    pub fn names(self) -> Vec<&'static str> {
        Self::ALL
            .iter()
            .filter(|(bit, _)| self.0 & bit != 0)
            .map(|(_, name)| *name)
            .collect()
    }
}

fn is_packs_reply(message: &[u8]) -> bool {
    message.len() == 9 && message[..6] == [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0e]
}

/// Identity, then the installed-pack query (reply: 9 bytes, mask at byte 7).
pub fn read_packs<L: Link>(session: &mut Session<L>) -> Result<(Vec<u8>, Packs), String> {
    let (identity_reply, _) = identify_podxt(session)?;
    Ok((identity_reply, query_packs(session)?))
}

fn query_packs<L: Link>(session: &mut Session<L>) -> Result<Packs, String> {
    session.send(PodxtRequest::InstalledPacks)?;
    match session.wait_for(WINDOW, is_packs_reply, false)? {
        Wait::Found(reply) => Ok(Packs(reply[7])),
        _ => Err("No installed-pack reply arrived.".into()),
    }
}

/// PODxt Live memory banks (Line 6 Edit `L6PODxtDevice` topology).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankKind {
    Tones,
    AmpSetups,
    FxSetups,
}

impl BankKind {
    pub const ALL: [BankKind; 3] = [BankKind::Tones, BankKind::AmpSetups, BankKind::FxSetups];

    pub fn size(self) -> u16 {
        match self {
            BankKind::Tones => 128,
            BankKind::AmpSetups => 107,
            BankKind::FxSetups => 64,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BankKind::Tones => "Tones",
            BankKind::AmpSetups => "Amp Setups",
            BankKind::FxSetups => "FX Setups",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            BankKind::Tones => "tones",
            BankKind::AmpSetups => "amp-setups",
            BankKind::FxSetups => "fx-setups",
        }
    }
}

/// One stored location. Only valid locations can be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    kind: BankKind,
    index: u16,
}

impl Location {
    pub fn new(kind: BankKind, index: u16) -> Option<Self> {
        (index < kind.size()).then_some(Self { kind, index })
    }

    pub fn all(kinds: &[BankKind]) -> impl Iterator<Item = Location> + '_ {
        kinds
            .iter()
            .flat_map(|&kind| (0..kind.size()).map(move |index| Location { kind, index }))
    }

    pub fn kind(self) -> BankKind {
        self.kind
    }

    pub fn index(self) -> u16 {
        self.index
    }

    /// Line 6 Edit `L6SharcTonePatchManager.TranslateAddr`.
    pub fn address(self) -> u16 {
        let (low_base, high_base) = match self.kind {
            BankKind::Tones => (0, 128),
            BankKind::AmpSetups => (64, 192),
            BankKind::FxSetups => (128, 256),
        };
        self.index + if self.index < 64 { low_base } else { high_base }
    }

    /// An unambiguous name for confirmations across the device's three banks.
    pub fn slot_name(self) -> String {
        match self.kind {
            BankKind::Tones => self.label(),
            BankKind::AmpSetups => format!("AMP {}", self.index + 1),
            BankKind::FxSetups => format!("FX {}", self.index + 1),
        }
    }

    /// Tones as the PODxt shows channels (`1A`..`32D`); setups as `#1`...
    /// A tone channel from its label, `1A`..`32D`.
    pub fn tone_from_label(label: &str) -> Option<Self> {
        let (bank, letter) = label.split_at(label.len().checked_sub(1)?);
        let bank: u16 = bank.parse().ok()?;
        let slot = "ABCD".find(letter.to_ascii_uppercase().as_str())? as u16;
        (1..=32).contains(&bank).then(|| Location {
            kind: BankKind::Tones,
            index: (bank - 1) * 4 + slot,
        })
    }

    pub fn label(self) -> String {
        match self.kind {
            BankKind::Tones => format!(
                "{}{}",
                self.index / 4 + 1,
                ['A', 'B', 'C', 'D'][usize::from(self.index % 4)]
            ),
            _ => format!("#{}", self.index + 1),
        }
    }
}

fn is_edit_buffer(message: &[u8]) -> bool {
    message.len() == EDIT_BUFFER_LEN && message[..6] == [0xf0, 0x00, 0x01, 0x0c, 0x03, 0x74]
}

pub struct CurrentTone {
    pub identity_reply: Vec<u8>,
    pub identity: Identity,
    pub tone: EditBuffer,
}

/// Send Workbench's standalone release after verifying the PODxt Live
/// identity. The owner checks the EDIT page for the effect on the display.
pub fn release_variax_control<L: Link>(session: &mut Session<L>) -> Result<Vec<u8>, String> {
    if !session.allows(Operation::PodxtVariaxControlRelease) {
        return Err("PODxt Live Variax control release is closed in this build.".into());
    }
    let reply = identify_podxt_live(session)?;
    session.send(PodxtRequest::ReleaseVariaxControl)?;
    Ok(reply)
}

/// Select one of the test's factory Variax models through the PODxt Live.
pub fn set_linked_variax_model<L: Link>(
    session: &mut Session<L>,
    model: LinkedVariaxModel,
) -> Result<(), String> {
    if !session.allows(Operation::PodxtLinkedVariaxModelEdit) {
        return Err("PODxt linked Variax model edits are closed in this build.".into());
    }
    identify_podxt_live(session)?;
    session.send(PodxtRequest::SetLinkedVariaxModel(model))
}

/// Identity, then the edit-buffer request only if the reply is a PODxt.
pub fn read_current<L: Link>(session: &mut Session<L>) -> Result<CurrentTone, String> {
    let (identity_reply, identity) = identify_podxt(session)?;
    let tone = read_edit_buffer(session)?;
    Ok(CurrentTone {
        identity_reply,
        identity,
        tone,
    })
}

/// Line 6 Edit waits up to three times 1 s for each stored dump.
const STORED_WINDOW: Duration = Duration::from_secs(3);
/// Line 6 Edit pauses 100 ms after each stored dump (and may see a short
/// `03 72` block-end message in that time).
const STORED_GAP: Duration = Duration::from_millis(100);

pub struct StoredTone {
    pub location: Location,
    pub tone: EditBuffer,
}

pub struct Library {
    pub identity_reply: Vec<u8>,
    /// Current tone before and after the sweep, to show it was not changed.
    pub before: EditBuffer,
    pub after: Result<EditBuffer, String>,
    pub entries: Vec<StoredTone>,
    /// Why the sweep stopped early, if it did.
    pub stopped: Option<String>,
}

/// Identity, current tone, then every stored location in `kinds` by address,
/// then the current tone again. Stops at the first location that does not
/// answer; the closing current-tone read still runs.
pub fn read_library<L: Link>(
    session: &mut Session<L>,
    kinds: &[BankKind],
    mut progress: impl FnMut(&StoredTone),
) -> Result<Library, String> {
    let (identity_reply, _) = identify_podxt(session)?;
    let before = read_edit_buffer(session)?;
    let mut entries = Vec::new();
    let mut stopped = None;
    for location in Location::all(kinds) {
        match request_tone(Some(location), |request, window| {
            wait_tone(session, request, window)
        })? {
            Some(tone) => {
                let stored = StoredTone { location, tone };
                progress(&stored);
                entries.push(stored);
                session.listen(STORED_GAP)?;
            }
            None => {
                stopped = Some(format!(
                    "{} {}: no 168-byte dump",
                    location.kind().name(),
                    location.label()
                ));
                break;
            }
        }
    }
    let after = read_edit_buffer(session);
    Ok(Library {
        identity_reply,
        before,
        after,
        entries,
        stopped,
    })
}

/// Identity, then listen only (at most [`LISTEN_MAX`]). The PODxt sends a
/// controller message when a pedal, knob or switch moves. Owner test
/// 2026-11-22: the pedal streams CC 7 (volume) or, in wah mode, CC 4 (wah
/// position) over 0..=127; the physical knobs also send their CC now and then
/// on their own (for example CC 15 mid, CC 17 channel volume).
pub fn listen<L: Link>(
    session: &mut Session<L>,
    window: Duration,
) -> Result<(Vec<u8>, ControllerTally), String> {
    let (identity_reply, _) = identify_podxt(session)?;
    let window = window.min(LISTEN_MAX);
    session.note(format!(
        "listening {} s: move the pedal now",
        window.as_secs()
    ));
    let tally = session.listen(window)?;
    session.note("listening finished");
    Ok((identity_reply, tally))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub index: u8,
    pub id: u32,
    pub name: &'static str,
}

/// A raw 168-byte edit-buffer dump with typed accessors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditBuffer {
    pub raw: Vec<u8>,
}

impl EditBuffer {
    pub fn parse(raw: Vec<u8>) -> Result<Self, String> {
        if !is_edit_buffer(&raw) || raw[EDIT_BUFFER_LEN - 1] != 0xf7 {
            return Err("not a 168-byte PODxt edit-buffer dump".into());
        }
        Ok(Self { raw })
    }

    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.raw[7..23])
            .trim_end_matches([' ', '\0'])
            .to_owned()
    }

    /// Stored value of MIDI controller `number` (0..=127).
    pub fn cc(&self, number: u8) -> u8 {
        self.raw[39 + usize::from(number)]
    }

    pub fn knob(&self, number: u8) -> f32 {
        f32::from(self.cc(number)) / 127.0
    }

    pub fn switch(&self, number: u8) -> bool {
        self.cc(number) > 63
    }

    fn model(&self, number: u8, list: &[u32]) -> Model {
        let index = self.cc(number);
        let id = list.get(usize::from(index)).copied().unwrap_or(list[0]);
        Model {
            index,
            id,
            name: models::name(id).unwrap_or("unknown"),
        }
    }

    pub fn amp(&self) -> Model {
        self.model(12, &models::AMP_MODELS)
    }
    pub fn amp_enabled(&self) -> bool {
        !self.switch(111)
    }
    pub fn cab(&self) -> Model {
        self.model(71, &models::CAB_MODELS)
    }
    pub fn stomp(&self) -> Model {
        self.model(75, &models::STOMP_MODELS)
    }
    pub fn modulation(&self) -> Model {
        self.model(58, &models::MOD_MODELS)
    }
    pub fn delay(&self) -> Model {
        self.model(88, &models::DELAY_MODELS)
    }
    pub fn reverb(&self) -> Model {
        self.model(37, &models::REVERB_MODELS)
    }
    pub fn wah(&self) -> Model {
        self.model(91, &models::WAH_MODELS)
    }
    /// Volume pedal level, CC 7 (`VOL_PEDAL_LEVEL`), 0.0..=1.0. Owner test
    /// 2026-11-22: this byte follows the physical pedal (toe 127, heel 0,
    /// about half 54), so each read returns the pedal position at that moment.
    pub fn volume_pedal_level(&self) -> f32 {
        self.knob(7)
    }
    /// Wah pedal position, CC 4 (`WAH_POSITION`), 0.0..=1.0. Owner test
    /// 2026-11-22 in wah mode: follows the pedal (heel 3, toe 127). In wah
    /// mode the pedal streams CC 4 instead of CC 7; CC 43 is the wah switch.
    pub fn wah_position(&self) -> f32 {
        self.knob(4)
    }

    /// Controllers whose stored value differs: `(controller, self, other)`.
    pub fn differences(&self, other: &EditBuffer) -> Vec<(u8, u8, u8)> {
        (0u8..=127)
            .filter(|&cc| 39 + usize::from(cc) < EDIT_BUFFER_LEN - 1)
            .filter(|&cc| self.cc(cc) != other.cc(cc))
            .map(|cc| (cc, self.cc(cc), other.cc(cc)))
            .collect()
    }

    /// Tempo in BPM: Line 6 Edit sends BPM x 10 as CC 89 (high 7 bits) and
    /// CC 90 (low 7 bits).
    pub fn tempo_bpm(&self) -> f32 {
        f32::from((u16::from(self.cc(89)) << 7) | u16::from(self.cc(90))) / 10.0
    }

    /// Human-readable summary.
    pub fn describe(&self) -> Vec<String> {
        let pct = |n| format!("{:.1}%", self.knob(n) * 100.0);
        let on = |n| if self.switch(n) { "on" } else { "off" };
        let show = |m: Model| format!("{} (index {}, id {:#x})", m.name, m.index, m.id);
        vec![
            format!("Tone name : {}", self.name()),
            format!(
                "Amp       : {}  amp {}",
                show(self.amp()),
                if self.amp_enabled() { "on" } else { "off" }
            ),
            format!(
                "            drive {}  bass {}  mid {}  treble {}  presence {}  channel volume {}",
                pct(13),
                pct(14),
                pct(15),
                pct(16),
                pct(21),
                pct(17)
            ),
            format!(
                "Cab       : {}  mic {}  room {}",
                show(self.cab()),
                self.cc(70),
                pct(76)
            ),
            format!(
                "Gate      : {}  threshold raw {}  decay {}",
                on(22),
                self.cc(23),
                pct(24)
            ),
            format!(
                "Comp      : {}  threshold raw {}  gain raw {}",
                on(26),
                self.cc(9),
                self.cc(5)
            ),
            format!("Stomp     : {}  {}", on(25), show(self.stomp())),
            format!(
                "Mod       : {}  {}  mix {}",
                on(50),
                show(self.modulation()),
                pct(56)
            ),
            format!(
                "Delay     : {}  {}  mix {}  time raw {}/{}",
                on(28),
                show(self.delay()),
                pct(34),
                self.cc(30),
                self.cc(62)
            ),
            format!(
                "Reverb    : {}  {}  mix {}  decay {}  tone {}",
                on(36),
                show(self.reverb()),
                pct(18),
                pct(38),
                pct(39)
            ),
            format!(
                "Wah       : {}  {}   EQ {}   FX loop {}",
                on(43),
                show(self.wah()),
                on(63),
                on(107)
            ),
            format!(
                "Pedals    : volume pedal level {}  wah position {}  pedal assign raw {}  volume pedal pre/post raw {}",
                pct(7),
                pct(4),
                self.cc(65),
                self.cc(47)
            ),
            format!(
                "Tempo     : {:.1} BPM (raw {}/{})",
                self.tempo_bpm(),
                self.cc(89),
                self.cc(90)
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::tests::{PODXT_LIVE, VARIAX_600};
    use crate::session::Quiet;
    use crate::session::testing::{AllowAll, Fake};

    fn live_identity_reply(version: [u8; 4]) -> Vec<u8> {
        let mut reply = PODXT_LIVE.to_vec();
        reply[12..16].copy_from_slice(&version);
        reply
    }

    pub(super) fn synthetic() -> Vec<u8> {
        let mut dump = vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x74, 0x0a];
        dump.extend(b"Test Tone       ");
        dump.resize(EDIT_BUFFER_LEN - 1, 0);
        dump.push(0xf7);
        dump
    }

    #[test]
    fn model_lists_have_the_line6_edit_lengths_and_names() {
        assert_eq!(
            [
                models::AMP_MODELS.len(),
                models::CAB_MODELS.len(),
                models::STOMP_MODELS.len(),
                models::MOD_MODELS.len(),
                models::DELAY_MODELS.len(),
                models::REVERB_MODELS.len(),
                models::WAH_MODELS.len()
            ],
            [107, 47, 31, 24, 14, 15, 8]
        );
        for list in [
            &models::AMP_MODELS[..],
            &models::CAB_MODELS,
            &models::WAH_MODELS,
        ] {
            assert!(list.iter().all(|id| models::name(*id).is_some()));
        }
    }

    #[test]
    fn controllers_decode_from_their_offsets() {
        let mut raw = synthetic();
        raw[39 + 12] = 22;
        raw[39 + 14] = 127;
        raw[39 + 36] = 127;
        let tone = EditBuffer::parse(raw).unwrap();
        assert_eq!(tone.name(), "Test Tone");
        assert_eq!(
            (tone.amp().name, tone.amp().id),
            ("Brit J-800", 0x0002_0020)
        );
        assert!(tone.amp_enabled() && tone.switch(36));
        assert_eq!(tone.knob(14), 1.0);
    }

    #[test]
    fn out_of_range_index_falls_back_and_amp_off_is_inverted() {
        let mut raw = synthetic();
        raw[39 + 12] = 127;
        raw[39 + 111] = 127;
        let tone = EditBuffer::parse(raw).unwrap();
        assert_eq!(tone.amp().id, models::AMP_MODELS[0]);
        assert!(!tone.amp_enabled());
    }

    #[test]
    fn pedal_bytes_decode_and_controllers_are_named() {
        let mut raw = synthetic();
        raw[39 + 7] = 64;
        raw[39 + 4] = 127;
        let tone = EditBuffer::parse(raw).unwrap();
        assert_eq!(tone.volume_pedal_level(), 64.0 / 127.0);
        assert_eq!(tone.wah_position(), 1.0);
        assert!(
            tone.describe()
                .iter()
                .any(|l| l.starts_with("Pedals    : volume pedal level 50.4%"))
        );
        assert_eq!(controls::name(7), Some("vol_pedal_level"));
        assert_eq!(controls::name(4), Some("wah_position"));
        assert_eq!(controls::CONTROLS.len(), 103);
    }

    #[test]
    fn pack_query_is_its_own_operation_and_decodes_the_mask() {
        assert_eq!(
            PodxtRequest::InstalledPacks.operation(),
            Operation::PodxtPackQuery
        );
        assert_eq!(
            PodxtRequest::InstalledPacks.bytes(),
            vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0e, 0x00, 0xf7]
        );
        assert_eq!(
            Packs(0b0011).names(),
            vec!["Metal Shop", "Collector Classics"]
        );
        let mut log = Quiet;
        let reply = vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0e, 0x00, 0x0f, 0xf7];
        let script = vec![
            (PodxtRequest::Identity, vec![PODXT_LIVE.to_vec()]),
            (PodxtRequest::InstalledPacks, vec![reply]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        assert_eq!(read_packs(&mut s).unwrap().1.names().len(), 4);
    }

    fn with(values: &[(u8, u8)]) -> Vec<u8> {
        let mut raw = synthetic();
        for &(cc, value) in values {
            raw[39 + usize::from(cc)] = value;
        }
        raw
    }

    fn set(control: Control, value: u8) -> PodxtRequest {
        PodxtRequest::Set(control, Value::new(value).unwrap())
    }

    #[test]
    fn set_requests_are_line6_edits_ccs_and_g6_operations() {
        assert_eq!(set(Control::StompEnable, 127).bytes(), vec![0xb0, 25, 127]);
        assert_eq!(set(Control::Drive, 64).bytes(), vec![0xb0, 13, 64]);
        assert_eq!(
            set(Control::ModEnable, 0).operation(),
            Operation::PodxtEditBufferWrite
        );
        assert!(Value::new(128).is_none());
        assert_eq!(Control::from_name("delay"), Some(Control::DelayEnable));
        assert!(Control::from_name("amp-model").is_none());
        assert_eq!(Control::from_name("amp"), Some(Control::AmpEnable));
        let mut ccs: Vec<u8> = Control::ALL.iter().map(|c| c.cc()).collect();
        ccs.sort_unstable();
        ccs.dedup();
        assert_eq!(ccs.len(), Control::ALL.len());
        let mut names: Vec<&str> = Control::ALL.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Control::ALL.len());
        // Bass-only and specially encoded controls stay out of the list.
        for cc in [
            107, 44, 45, 48, 49, 23, 70, 51, 31, 108, 65, 29, 61, 30, 62, 89, 90, 11, 12, 71, 75,
            58, 88, 37, 91, 96, 101, 120, 127,
        ] {
            assert!(Control::ALL.iter().all(|c| c.cc() != cc), "CC {cc}");
        }
        assert_eq!(
            (
                Control::AmpEnable.switch_value(true),
                Control::AmpEnable.switch_value(false)
            ),
            (0, 127)
        );
        assert!(Control::AmpEnable.is_on(0) && !Control::AmpEnable.is_on(127));
        assert_eq!(Control::EqGain2.cc(), 116);
        assert_eq!(Control::from_name("vol-pedal-min").unwrap().cc(), 46);
        for control in Control::ALL {
            assert!(controls::name(control.cc()).is_some(), "{control:?}");
        }
    }

    fn packs_reply(mask: u8) -> Vec<u8> {
        vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, mask, 0xf7]
    }

    fn setting_message(setting: Setting, value: u16, byte_index: usize) -> PodxtRequest {
        LiveControl::Additional(setting)
            .messages(value)
            .nth(byte_index)
            .unwrap()
            .2
    }

    #[test]
    fn each_mic_choice_is_written_read_back_and_restored() {
        for (target, label) in [
            (0, "57 On Axis (index 0)"),
            (1, "57 Off Axis (index 1)"),
            (2, "421 Dynamic (index 2)"),
            (3, "67 Condenser (index 3)"),
        ] {
            let original = u8::from(target == 0);
            let target = Setting::Mic.parse(&target.to_string()).unwrap();
            let before = with(&[(70, original), (71, 17), (76, 41)]);
            let script = vec![
                (
                    PodxtRequest::Identity,
                    vec![live_identity_reply([0, 3, 0, 1])],
                ),
                (PodxtRequest::EditBuffer, vec![before.clone()]),
                (setting_message(Setting::Mic, target, 0), vec![]),
                (
                    PodxtRequest::EditBuffer,
                    vec![with(&[(70, target as u8), (71, 17), (76, 41)])],
                ),
                (
                    setting_message(Setting::Mic, u16::from(original), 0),
                    vec![],
                ),
                (PodxtRequest::EditBuffer, vec![before]),
            ];
            let mut log = Quiet;
            let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
            let test = live_settings_test(
                &mut session,
                &[LiveTarget::Additional(Setting::Mic, target)],
            )
            .unwrap();
            assert_eq!(test.changes[0].control.describe(target), label);
            assert_eq!(session.sent[2], [0xb0, 70, target as u8]);
            assert_eq!(session.sent[4], [0xb0, 70, original]);
            assert!(test.stuck() && test.unexpected().is_empty() && test.exact_undo());
        }
    }

    #[test]
    fn room_testing_changes_cc_76_and_restores_the_full_tone() {
        let before = with(&[(70, 3), (71, 17), (76, 41)]);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (set(Control::Room, 61), vec![]),
            (
                PodxtRequest::EditBuffer,
                vec![with(&[(70, 3), (71, 17), (76, 61)])],
            ),
            (set(Control::Room, 41), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut log = Quiet;
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let test = live_edit_test(&mut session, &[(Control::Room, Target::Offset(20))]).unwrap();
        assert_eq!(session.sent[2], [0xb0, 76, 61]);
        assert_eq!(session.sent[4], [0xb0, 76, 41]);
        assert!(test.stuck() && test.unexpected().is_empty() && test.exact_undo());
    }

    #[test]
    fn a_mic_test_needs_a_known_original_and_a_different_target() {
        for (original, message) in [
            (1, "already 57 Off Axis"),
            (4, "outside the supported range"),
        ] {
            let script = vec![
                (
                    PodxtRequest::Identity,
                    vec![live_identity_reply([0, 3, 0, 1])],
                ),
                (PodxtRequest::EditBuffer, vec![with(&[(70, original)])]),
            ];
            let mut log = Quiet;
            let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
            assert!(matches!(
                live_settings_test(&mut session, &[LiveTarget::Additional(Setting::Mic, 1)]),
                Err(error) if error.contains(message)
            ));
            assert_eq!(session.sent.len(), 2);
        }
    }

    #[test]
    fn unverified_firmware_blocks_device_sends_after_identity() {
        type Send = fn(&mut Session<'_, Fake>) -> Result<(), String>;
        fn test_editor_buffer() -> EditorBuffer {
            EditorBuffer {
                channel: Location::tone_from_label("1B").unwrap(),
                tone: EditBuffer::parse(synthetic()).unwrap(),
                packs: Packs(0),
            }
        }

        let operations: [(&str, Send); 12] = [
            ("edit and undo", |session| stomp_test(session).map(|_| ())),
            ("keep edit", |session| {
                apply_live_settings(
                    session,
                    &[LiveTarget::Basic(Control::Drive, Target::Offset(1))],
                )
                .map(|_| ())
            }),
            ("additional edit", |session| {
                live_settings_test(session, &[LiveTarget::Additional(Setting::Tempo, 1200)])
                    .map(|_| ())
            }),
            ("editor edit buffer", |session| {
                edit_editor_buffer(
                    session,
                    &test_editor_buffer(),
                    EditorEdit::Controls(&[LiveTarget::Basic(Control::Drive, Target::Offset(1))]),
                    || false,
                )
                .map(|_| ())
            }),
            ("whole-tone send", |session| {
                let buffer = test_editor_buffer();
                let backup = VerifiedToneBackup {
                    slot: buffer.channel,
                    original: TonePayload::from_dump(&buffer.tone).unwrap(),
                };
                send_whole_tone(session, &buffer, WholeToneSource::Current, &backup, || {
                    false
                })
                .map(|_| ())
            }),
            ("store", |session| {
                let buffer = test_editor_buffer();
                let candidate = TonePayload::from_dump(&buffer.tone).unwrap();
                let prepared = PreparedStore {
                    backup: VerifiedToneBackup {
                        slot: buffer.channel,
                        original: candidate.clone(),
                    },
                    candidate,
                };
                let confirmed = prepared.confirm("STORE PODXT 1B").unwrap();
                store_tone(session, confirmed, &StoreControl::default()).map(|_| ())
            }),
            ("Variax control release", |session| {
                release_variax_control(session).map(|_| ())
            }),
            ("linked Variax model", |session| {
                set_linked_variax_model(session, LinkedVariaxModel::FactorySpank1)
            }),
            ("channel switch", |session| {
                let slot = Location::tone_from_label("1B").unwrap();
                switch_channel(session, slot, None, false).map(|_| ())
            }),
            ("editor channel recall", |session| {
                let slot = Location::tone_from_label("1B").unwrap();
                recall_editor_channel(session, slot, || false).map(|_| ())
            }),
            ("tuner", |session| {
                tuner(session, Duration::ZERO, |_| {}).map(|_| ())
            }),
            ("editor tuner", |session| {
                read_live(session, ReadCommand::Tuner, || false, |_| {})
            }),
        ];
        for (version, found) in [([0, 2, 0, 7], "2.07"), ([0, 3, 0, 2], "3.02")] {
            for (name, send) in operations {
                let link = Fake::new(
                    Path::PodxtMidi,
                    vec![(PodxtRequest::Identity, vec![live_identity_reply(version)])],
                );
                let mut log = Quiet;
                let mut session = Session::new(link, &AllowAll, &mut log);
                assert_eq!(
                    send(&mut session).unwrap_err(),
                    format!(
                        "Found firmware {found} (family 0x0003, member 0x000a). The verified target is PODxt Live firmware 3.01."
                    ),
                    "{name}",
                );
                assert_eq!(session.link.writes, [identity::REQUEST.to_vec()], "{name}");
                session.link.assert_finished();
            }
        }
    }

    #[test]
    fn linked_variax_model_changes_send_only_the_identified_podxt_the_wrapped_model() {
        for model in [
            LinkedVariaxModel::FactoryTModel1,
            LinkedVariaxModel::FactorySpank1,
        ] {
            let mut log = Quiet;
            let request = PodxtRequest::SetLinkedVariaxModel(model);
            let script = vec![
                (
                    PodxtRequest::Identity,
                    vec![live_identity_reply([0, 3, 0, 1])],
                ),
                (request, vec![]),
            ];
            let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
            set_linked_variax_model(&mut session, model).unwrap();
            assert_eq!(
                session.link.writes,
                [
                    identity::REQUEST.to_vec(),
                    vec![0xf0, 0, 1, 0x0c, 3, 0x10, 0, 120, model as u8, 0xf7]
                ]
            );
        }
        let mut log = Quiet;
        let link = Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]);
        let mut session = Session::new(link, &crate::gate::AllClosed, &mut log);
        assert!(set_linked_variax_model(&mut session, LinkedVariaxModel::FactorySpank1).is_err());
        assert!(session.link.writes.is_empty());
    }

    #[test]
    fn variax_control_release_requires_permission_and_podxt_live_identity() {
        let mut log = Quiet;
        let link = Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]);
        let mut session = Session::new(link, &crate::gate::AllClosed, &mut log);
        assert!(release_variax_control(&mut session).is_err());
        assert!(session.link.writes.is_empty());

        let mut other = live_identity_reply([0, 3, 0, 1]);
        other[10] = 0x09;
        let script = vec![(PodxtRequest::Identity, vec![other])];
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        assert!(release_variax_control(&mut session).is_err());
        assert_eq!(session.link.writes, [identity::REQUEST.to_vec()]);
    }

    #[test]
    fn variax_control_release_sends_only_identity_and_the_original_workbench_message() {
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::ReleaseVariaxControl, vec![]),
        ];
        let mut log = Quiet;
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        assert_eq!(
            release_variax_control(&mut session).unwrap(),
            live_identity_reply([0, 3, 0, 1])
        );
        assert_eq!(
            session.link.writes,
            [
                identity::REQUEST.to_vec(),
                vec![0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7]
            ]
        );
    }

    #[test]
    fn applying_live_settings_leaves_a_verified_edit_active() {
        let before = with(&[(42, 33)]);
        let after = with(&[(42, 53)]);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (set(Control::EqFreq2, 53), vec![]),
            (PodxtRequest::EditBuffer, vec![after.clone()]),
        ];
        let target = LiveTarget::Basic(Control::EqFreq2, Target::Offset(20));
        let mut log = Quiet;
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let LiveApply::Applied(edit) = apply_live_settings(&mut session, &[target]).unwrap() else {
            panic!("expected requested EQ edit to remain active");
        };
        assert_eq!(edit.before.raw, before);
        assert_eq!(edit.after.raw, after);
        assert_eq!(edit.changes[0].target, 53);
        assert_eq!(session.link.writes.len(), 4);
    }

    #[test]
    fn retaining_edits_requires_a_separate_gate_and_unconfirmed_edits_are_undone() {
        struct UndoOnly;
        impl crate::gate::GatePolicy for UndoOnly {
            fn allows(&self, operation: Operation) -> bool {
                operation != Operation::PodxtKeepEditBufferChanges
            }
        }
        let targets = [LiveTarget::Additional(Setting::Tempo, 1135)];
        let mut log = Quiet;
        let link = Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]);
        let mut session = Session::new(link, &UndoOnly, &mut log);
        assert!(apply_live_settings(&mut session, &targets).is_err());
        assert!(session.link.writes.is_empty());

        let before = with(&[(89, 7), (90, 41)]);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (setting_message(Setting::Tempo, 1135, 0), vec![]),
            (setting_message(Setting::Tempo, 1135, 1), vec![]),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (setting_message(Setting::Tempo, 937, 0), vec![]),
            (setting_message(Setting::Tempo, 937, 1), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let LiveApply::UndoAttempted(test) = apply_live_settings(&mut session, &targets).unwrap()
        else {
            panic!("expected automatic undo after failed confirmation");
        };
        assert!(!test.stuck() && test.exact_undo());
    }

    #[test]
    fn model_selection_checks_both_models_before_any_edit() {
        let locked = Setting::AmpModel.model_index(0x00020002).unwrap();
        for (current, wanted, reply, failure) in [
            (22, locked, Some(packs_reply(4)), "requested model"),
            (
                locked,
                22,
                Some(packs_reply(4)),
                "original (needed for undo) model",
            ),
            (127, 22, Some(packs_reply(4)), "outside the PODxt Live list"),
            (22, 3, None, "No installed-pack reply"),
        ] {
            let mut log = Quiet;
            let script = vec![
                (
                    PodxtRequest::Identity,
                    vec![live_identity_reply([0, 3, 0, 1])],
                ),
                (PodxtRequest::EditBuffer, vec![with(&[(12, current as u8)])]),
                (PodxtRequest::InstalledPacks, reply.into_iter().collect()),
            ];
            let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
            let targets = [
                LiveTarget::Basic(Control::Drive, Target::Offset(1)),
                LiveTarget::Additional(Setting::AmpModel, wanted),
            ];
            assert!(
                live_settings_test(&mut session, &targets)
                    .err()
                    .unwrap()
                    .contains(failure)
            );
            assert_eq!(session.link.writes.len(), 3);
        }
    }

    #[test]
    fn additional_edits_require_their_own_gate_and_the_live_member() {
        struct ReadAndBasic;
        impl crate::gate::GatePolicy for ReadAndBasic {
            fn allows(&self, operation: Operation) -> bool {
                operation != Operation::PodxtAdditionalEditBufferWrite
            }
        }
        let targets = [LiveTarget::Additional(Setting::Tempo, 1135)];
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]),
            &ReadAndBasic,
            &mut log,
        );
        assert!(
            live_settings_test(&mut session, &targets)
                .err()
                .unwrap()
                .contains("closed")
        );
        assert!(session.link.writes.is_empty());

        let mut other = live_identity_reply([0, 3, 0, 1]);
        other[10] = 0x09;
        let script = vec![(PodxtRequest::Identity, vec![other])];
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let invalid = [LiveTarget::Additional(Setting::Tempo, 2500)];
        assert!(
            live_settings_test(&mut session, &invalid)
                .err()
                .unwrap()
                .contains("invalid")
        );
        assert!(session.link.writes.is_empty());
        assert!(
            live_settings_test(&mut session, &targets)
                .err()
                .unwrap()
                .contains("member 0x0009")
        );
        assert_eq!(session.link.writes.len(), 1);
    }

    #[test]
    fn undoing_a_wah_model_change_restores_the_original_wah_position() {
        let before = with(&[(4, 63), (91, 0)]);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (PodxtRequest::InstalledPacks, vec![packs_reply(4)]),
            (setting_message(Setting::WahModel, 1, 0), vec![]),
            (PodxtRequest::EditBuffer, vec![with(&[(4, 16), (91, 1)])]),
            (setting_message(Setting::WahModel, 0, 0), vec![]),
            (PodxtRequest::EditBuffer, vec![with(&[(4, 0), (91, 0)])]),
            (set(Control::WahPosition, 63), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut log = Quiet;
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let test = live_settings_test(
            &mut session,
            &[LiveTarget::Additional(Setting::WahModel, 1)],
        )
        .unwrap();
        assert_eq!(test.unexpected(), [(4, 63, 16)]);
        assert_eq!(session.sent[7], [0xb0, 4, 63]);
        assert!(test.stuck() && test.exact_undo());
    }

    #[test]
    fn fx_junkie_model_and_tempo_are_verified_and_block_side_effects_are_undone() {
        let mut log = Quiet;
        let before = with(&[(75, 0), (79, 37), (89, 9), (90, 48)]);
        let after = with(&[(75, 15), (79, 63), (89, 8), (90, 111)]);
        let intermediate = with(&[(75, 0), (79, 55), (89, 9), (90, 48)]);
        let tempo = |value, byte| setting_message(Setting::Tempo, value, byte);
        let stomp = |value| setting_message(Setting::StompModel, value, 0);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (PodxtRequest::InstalledPacks, vec![packs_reply(4)]),
            (stomp(15), vec![]),
            (tempo(1135, 0), vec![]),
            (tempo(1135, 1), vec![]),
            (PodxtRequest::EditBuffer, vec![after]),
            (stomp(0), vec![]),
            (tempo(1200, 0), vec![]),
            (tempo(1200, 1), vec![]),
            (PodxtRequest::EditBuffer, vec![intermediate]),
            (set(Control::StompParam2, 37), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let targets = [
            LiveTarget::Additional(Setting::Tempo, 1135),
            LiveTarget::Additional(Setting::StompModel, 15),
        ];
        let test = live_settings_test(&mut session, &targets).unwrap();
        assert_eq!(test.sent, 2);
        assert_eq!(test.unexpected(), [(79, 37, 63)]);
        assert!(test.stuck() && test.exact_undo());
    }

    #[test]
    fn a_failed_second_byte_still_restores_the_full_original_pair() {
        struct FailedSend(Fake);
        impl Link for FailedSend {
            fn path(&self) -> Path {
                self.0.path()
            }
            fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
                self.0.write(bytes)?;
                if bytes == [0xb0, 62, 1] {
                    return Err("write failed".into());
                }
                Ok(())
            }
            fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
                self.0.read(wait)
            }
            fn elapsed(&self) -> Duration {
                self.0.elapsed()
            }
            fn pause(&mut self, duration: Duration) {
                self.0.pause(duration);
            }
        }

        let mut log = Quiet;
        let before = with(&[(30, 42), (62, 7)]);
        let delay = |value, byte| setting_message(Setting::DelayTime, value, byte);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (delay(8193, 0), vec![]),
            (delay(8193, 1), vec![]),
            (delay((42 << 7) | 7, 0), vec![]),
            (delay((42 << 7) | 7, 1), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut session = Session::new(
            FailedSend(Fake::new(Path::PodxtMidi, script)),
            &AllowAll,
            &mut log,
        );
        let test = live_settings_test(
            &mut session,
            &[LiveTarget::Additional(Setting::DelayTime, 8193)],
        )
        .unwrap();
        assert_eq!(test.sent, 0);
        assert!(matches!(&test.after, Err(error) if error == "write failed"));
        assert!(test.restored.is_ok() && test.undo_sent.is_ok());
        assert_eq!(session.link.0.writes.len(), 7);
    }

    #[test]
    fn manual_timing_goes_after_sync_off_and_before_restoring_sync() {
        let mut log = Quiet;
        let before = with(&[(31, 4), (30, 42), (62, 7)]);
        let after = with(&[(31, 0), (30, 64), (62, 1)]);
        let note = |value| setting_message(Setting::DelayNote, value, 0);
        let time = |value, byte| setting_message(Setting::DelayTime, value, byte);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (note(0), vec![]),
            (time(8193, 0), vec![]),
            (time(8193, 1), vec![]),
            (PodxtRequest::EditBuffer, vec![after]),
            (time((42 << 7) | 7, 0), vec![]),
            (time((42 << 7) | 7, 1), vec![]),
            (note(4), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let targets = [
            LiveTarget::Additional(Setting::DelayTime, 8193),
            LiveTarget::Additional(Setting::DelayNote, 0),
        ];
        let test = live_settings_test(&mut session, &targets).unwrap();
        assert!(test.stuck() && test.exact_undo());
    }

    #[test]
    fn routing_switches_use_pre_zero_and_post_127() {
        for (control, cc) in [
            (Control::VolumePedalPrePost, 47),
            (Control::ModPrePost, 57),
            (Control::DelayPrePost, 87),
        ] {
            assert_eq!(
                set(control, control.switch_value(false)).bytes(),
                [0xb0, cc, 0]
            );
            assert_eq!(
                set(control, control.switch_value(true)).bytes(),
                [0xb0, cc, 127]
            );
            assert_eq!(control.describe(0), "pre");
            assert_eq!(control.describe(127), "post");
        }
    }

    #[test]
    fn stomp_test_flips_reads_and_undoes() {
        let mut log = Quiet;
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![with(&[(25, 0)])]),
            (set(Control::StompEnable, 127), vec![]),
            (PodxtRequest::EditBuffer, vec![with(&[(25, 127)])]),
            (set(Control::StompEnable, 0), vec![]),
            (PodxtRequest::EditBuffer, vec![with(&[(25, 0)])]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let test = stomp_test(&mut s).unwrap();
        assert!(test.stuck() && test.undone() && test.unexpected().is_empty());
    }

    #[test]
    fn several_changes_are_sent_checked_and_undone_in_reverse() {
        let mut log = Quiet;
        let before = with(&[(13, 40), (50, 0), (28, 127)]);
        let after = with(&[(13, 100), (50, 127), (28, 0), (17, 81)]); // CC 17 drifted
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (set(Control::Drive, 100), vec![]),
            (set(Control::ModEnable, 127), vec![]),
            (set(Control::DelayEnable, 0), vec![]),
            (PodxtRequest::EditBuffer, vec![after]),
            (set(Control::DelayEnable, 127), vec![]),
            (set(Control::ModEnable, 0), vec![]),
            (set(Control::Drive, 40), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let targets = [
            (Control::Drive, Target::Value(Value::new(100).unwrap())),
            (Control::ModEnable, Target::Toggle),
            (Control::DelayEnable, Target::Off),
        ];
        let test = live_edit_test(&mut s, &targets).unwrap();
        assert!(test.stuck() && test.undone());
        assert_eq!(test.unexpected(), vec![(17, 0, 81)]);
        assert!(test.not_restored().is_empty());
    }

    #[test]
    fn a_device_value_hidden_behind_a_later_echo_is_still_reported() {
        // Seen live: the PODxt sent Presence 0, then our Presence 50 echoed.
        let mut tally = ControllerTally::default();
        for value in [0, 50] {
            tally.add(&crate::midi::Event::Short(vec![0xb0, 21, value]));
        }
        tally.add(&crate::midi::Event::Short(vec![0xb0, 16, 40]));
        assert_eq!(
            device_originated(&tally, &[(21, 50), (16, 40)]),
            vec![(21, 0)]
        );
        assert!(Control::BypassVolume.device_may_override());
    }

    #[test]
    fn echoes_are_not_reported_but_the_devices_own_messages_are() {
        let mut log = Quiet;
        let before = with(&[(17, 85), (21, 50)]);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (
                set(Control::ChannelVolume, 105),
                vec![vec![0xb0, 17, 105], vec![0xb0, 17, 86]],
            ),
            (PodxtRequest::EditBuffer, vec![with(&[(17, 86), (21, 50)])]),
            (
                set(Control::ChannelVolume, 85),
                vec![vec![0xb0, 17, 85], vec![0xb0, 21, 0]],
            ),
            (PodxtRequest::EditBuffer, vec![with(&[(17, 85), (21, 0)])]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let test = live_edit_test(&mut s, &[(Control::ChannelVolume, Target::Offset(20))]).unwrap();
        assert_eq!(test.device_sent_after_changes, vec![(17, 86)]);
        assert_eq!(test.device_sent_after_undo, vec![(21, 0)]);
        assert!(!test.stuck());
        assert_eq!(test.not_restored(), vec![(21, 50, 0)]);
        assert!(
            Control::ChannelVolume.device_may_override() && !Control::Drive.device_may_override()
        );
        assert!(Control::from_name("fx-loop").is_none());
    }

    #[test]
    fn offsets_move_knobs_and_bounce_off_the_ends() {
        let mut log = Quiet;
        let before = with(&[(16, 120), (15, 30)]);
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![before.clone()]),
            (set(Control::Treble, 100), vec![]),
            (set(Control::Mid, 50), vec![]),
            (PodxtRequest::EditBuffer, vec![with(&[(16, 100), (15, 50)])]),
            (set(Control::Mid, 30), vec![]),
            (set(Control::Treble, 120), vec![]),
            (PodxtRequest::EditBuffer, vec![before]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let targets = [
            (Control::Treble, Target::Offset(20)),
            (Control::Mid, Target::Offset(20)),
        ];
        let test = live_edit_test(&mut s, &targets).unwrap();
        assert_eq!((test.changes[0].target, test.changes[1].target), (100, 50));
        assert!(test.stuck() && test.undone());
    }

    #[test]
    fn the_undo_is_sent_even_when_the_check_read_fails() {
        let mut log = Quiet;
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![with(&[(25, 127), (36, 0)])]),
            (set(Control::StompEnable, 0), vec![]),
            (set(Control::ReverbEnable, 127), vec![]),
            (PodxtRequest::EditBuffer, vec![]),
            (set(Control::ReverbEnable, 0), vec![]),
            (set(Control::StompEnable, 127), vec![]),
            (PodxtRequest::EditBuffer, vec![with(&[(25, 127), (36, 0)])]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let targets = [
            (Control::StompEnable, Target::Toggle),
            (Control::ReverbEnable, Target::Toggle),
        ];
        let test = live_edit_test(&mut s, &targets).unwrap();
        assert!(test.after.is_err() && !test.stuck() && test.undone());
    }

    #[test]
    fn duplicates_and_empty_requests_send_nothing_and_a_closed_policy_refuses() {
        let mut log = Quiet;
        let mut s = Session::new(
            Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]),
            &AllowAll,
            &mut log,
        );
        assert!(live_edit_test(&mut s, &[]).is_err());
        let twice = [(Control::Mid, Target::On), (Control::Mid, Target::Off)];
        assert!(live_edit_test(&mut s, &twice).is_err());
        assert!(s.link.writes.is_empty());
        let mut log = Quiet;
        let mut closed = Session::new(
            Fake::new::<PodxtRequest>(Path::PodxtMidi, vec![]),
            &crate::gate::AllClosed,
            &mut log,
        );
        assert!(closed.send(set(Control::StompEnable, 127)).is_err());
        assert!(closed.link.writes.is_empty());
    }

    fn tone(name: &[u8], cc17: u8) -> Vec<u8> {
        let mut raw = vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x74, 0x0a];
        let mut padded = name.to_vec();
        padded.resize(16, b' ');
        raw.extend(padded);
        raw.resize(EDIT_BUFFER_LEN - 1, 0);
        raw.push(0xf7);
        raw[39 + 17] = cc17;
        raw
    }

    #[test]
    fn channel_labels_and_program_changes_follow_line6_edit() {
        let loc = |l: &str| Location::tone_from_label(l).unwrap();
        assert_eq!(
            (loc("1A").index(), loc("7B").index(), loc("32D").index()),
            (0, 25, 127)
        );
        assert!(
            Location::tone_from_label("33A").is_none() && Location::tone_from_label("1E").is_none()
        );
        assert_eq!(PodxtRequest::Program(loc("7B")).bytes(), vec![0xc0, 25]);
        assert_eq!(
            PodxtRequest::Program(loc("1A")).operation(),
            Operation::PodxtChannelSwitch
        );
        assert_eq!(loc("7B").label(), "7B");
    }

    #[test]
    fn switch_and_back_with_a_known_original_channel() {
        let mut log = Quiet;
        let (orig, target) = (
            Location::tone_from_label("7B").unwrap(),
            Location::tone_from_label("3C").unwrap(),
        );
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![tone(b"Boutique", 86)]),
            (PodxtRequest::Stored(orig), vec![tone(b"Boutique", 114)]),
            (PodxtRequest::Stored(target), vec![tone(b"Brit J-800", 90)]),
            (PodxtRequest::Program(target), vec![]),
            (PodxtRequest::EditBuffer, vec![tone(b"Brit J-800", 87)]),
            (PodxtRequest::Program(orig), vec![]),
            (PodxtRequest::EditBuffer, vec![tone(b"Boutique", 87)]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let result = switch_channel(&mut s, target, Some(orig), true).unwrap();
        assert!(result.switched() && result.returned());
    }

    #[test]
    fn channel_review_can_stop_program_change_after_reading_both_linked_tones() {
        let original = Location::tone_from_label("7B").unwrap();
        let target = Location::tone_from_label("3C").unwrap();
        let mut before = tone(b"Old", 42);
        let mut destination = tone(b"New", 63);
        before[39 + 120] = 10;
        destination[39 + 120] = 15;
        for accepted in [false, true] {
            let mut script = vec![
                (
                    PodxtRequest::Identity,
                    vec![live_identity_reply([0, 3, 0, 1])],
                ),
                (PodxtRequest::EditBuffer, vec![before.clone()]),
                (PodxtRequest::Stored(original), vec![before.clone()]),
                (PodxtRequest::Stored(target), vec![destination.clone()]),
            ];
            if accepted {
                script.extend([
                    (PodxtRequest::Program(target), vec![]),
                    (PodxtRequest::EditBuffer, vec![destination.clone()]),
                ]);
            }
            let mut log = Quiet;
            let mut session = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
            let result =
                switch_channel_checked(&mut session, target, Some(original), false, |a, b| {
                    assert_eq!((a.cc(120), b.cc(120)), (10, 15));
                    if accepted {
                        Ok(())
                    } else {
                        Err("Linked model review needed.".into())
                    }
                });
            if accepted {
                assert!(result.unwrap().switched());
            } else {
                assert!(result.is_err());
                assert!(
                    session
                        .sent
                        .iter()
                        .all(|bytes| bytes.first() != Some(&0xc0))
                );
            }
            session.link.assert_finished();
        }
    }

    #[test]
    fn unsaved_edits_refuse_the_switch_before_any_program_change() {
        let mut log = Quiet;
        let orig = Location::tone_from_label("7B").unwrap();
        let mut edited = tone(b"Boutique", 86);
        edited[39 + 13] = 99; // drive changed live, not saved
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::EditBuffer, vec![edited]),
            (PodxtRequest::Stored(orig), vec![tone(b"Boutique", 114)]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let target = Location::tone_from_label("3C").unwrap();
        assert!(switch_channel(&mut s, target, Some(orig), true).is_err());
        assert!(s.sent.iter().all(|b| b[0] != 0xc0));
    }

    #[test]
    fn global_queries_and_tuner_values_follow_line6_edit() {
        assert_eq!(
            PodxtRequest::QueryGlobal(Global::CurrentChannel).bytes(),
            vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x57, 17, 0xf7]
        );
        assert_eq!(
            PodxtRequest::QueryGlobal(Global::TunerNote).operation(),
            Operation::PodxtGlobalRead
        );
        assert_eq!(PodxtRequest::TunerMode(true).bytes(), vec![0xb0, 69, 127]);
        assert_eq!(
            PodxtRequest::TunerMode(false).operation(),
            Operation::PodxtTunerMode
        );
        let reply = [
            0xf0, 0x00, 0x01, 0x0c, 0x03, 0x56, 23, 0x0f, 0x0f, 0x0e, 0x0c, 0xf7,
        ];
        assert_eq!(global_value(&reply, 23), Some(0xffec));
        assert_eq!(tuner_cents(0xffec), -20.0);
        assert_eq!(global_value(&reply, 22), None);
        assert_eq!(tuner_note_name(0xfffd), "-");
        assert_eq!(tuner_note_name(0xfffe), "A");
        assert_eq!(tuner_note_name(5), "E");
        assert_eq!(tuner_note_name(10), "A");
        let mut raw = synthetic();
        raw[39 + 89] = 9;
        raw[39 + 90] = 48;
        assert_eq!(EditBuffer::parse(raw).unwrap().tempo_bpm(), 120.0);
    }

    #[test]
    fn the_tuner_records_missing_values_and_is_always_switched_off() {
        let mut log = Quiet;
        let note = vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x56, 22, 0, 0, 0, 5, 0xf7];
        let script = vec![
            (
                PodxtRequest::Identity,
                vec![live_identity_reply([0, 3, 0, 1])],
            ),
            (PodxtRequest::TunerMode(true), vec![]),
            (PodxtRequest::QueryGlobal(Global::TunerNote), vec![note]),
            (PodxtRequest::QueryGlobal(Global::TunerDeviation), vec![]),
            (PodxtRequest::TunerMode(false), vec![]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let run = tuner(&mut s, Duration::from_millis(400), |_| {}).unwrap();
        assert_eq!(run.readings[0].note, Some(5));
        assert_eq!(run.readings[0].deviation, None);
        assert!(run.off_sent.is_ok());
        assert_eq!(s.sent.last(), Some(&PodxtRequest::TunerMode(false).bytes()));
    }

    #[test]
    fn locations_follow_line6_edit_addressing() {
        let address = |kind, index| Location::new(kind, index).unwrap().address();
        assert_eq!(
            (address(BankKind::Tones, 0), address(BankKind::Tones, 63)),
            (0, 63)
        );
        assert_eq!(
            (address(BankKind::Tones, 64), address(BankKind::Tones, 127)),
            (192, 255)
        );
        assert_eq!(
            (
                address(BankKind::AmpSetups, 0),
                address(BankKind::AmpSetups, 63)
            ),
            (64, 127)
        );
        assert_eq!(
            (
                address(BankKind::AmpSetups, 64),
                address(BankKind::AmpSetups, 106)
            ),
            (256, 298)
        );
        assert_eq!(
            (
                address(BankKind::FxSetups, 0),
                address(BankKind::FxSetups, 63)
            ),
            (128, 191)
        );
        assert!(
            Location::new(BankKind::Tones, 128).is_none()
                && Location::new(BankKind::FxSetups, 64).is_none()
        );
        let mut all: Vec<u16> = Location::all(&BankKind::ALL)
            .map(Location::address)
            .collect();
        assert_eq!(all.len(), 299);
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 299);
        let tone_200 = PodxtRequest::Stored(Location::new(BankKind::Tones, 72).unwrap()).bytes();
        assert_eq!(
            tone_200,
            vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x73, 1, 72, 0, 0, 0xf7]
        ); // address 200
        assert_eq!(Location::new(BankKind::Tones, 5).unwrap().label(), "2B");
        assert_eq!(
            PodxtRequest::Stored(Location::new(BankKind::Tones, 0).unwrap()).operation(),
            Operation::PodxtLibraryRead
        );
    }

    #[test]
    fn library_reads_each_location_and_checks_the_current_tone_twice() {
        let mut log = Quiet;
        let dump = synthetic();
        let mut script = vec![
            (PodxtRequest::Identity, vec![PODXT_LIVE.to_vec()]),
            (PodxtRequest::EditBuffer, vec![dump.clone()]),
        ];
        for location in Location::all(&[BankKind::FxSetups]) {
            script.push((PodxtRequest::Stored(location), vec![dump.clone()]));
        }
        script.push((PodxtRequest::EditBuffer, vec![dump.clone()]));
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let mut seen = 0;
        let library = read_library(&mut s, &[BankKind::FxSetups], |_| seen += 1).unwrap();
        assert_eq!((library.entries.len(), seen), (64, 64));
        assert!(library.stopped.is_none());
        assert_eq!(library.after.unwrap(), library.before);
    }

    #[test]
    fn library_stops_at_a_silent_location_and_still_reads_the_current_tone() {
        let mut log = Quiet;
        let dump = synthetic();
        let first = Location::new(BankKind::Tones, 0).unwrap();
        let second = Location::new(BankKind::Tones, 1).unwrap();
        let script = vec![
            (PodxtRequest::Identity, vec![PODXT_LIVE.to_vec()]),
            (PodxtRequest::EditBuffer, vec![dump.clone()]),
            (PodxtRequest::Stored(first), vec![dump.clone()]),
            (PodxtRequest::Stored(second), vec![]),
            (PodxtRequest::EditBuffer, vec![dump.clone()]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let library = read_library(&mut s, &[BankKind::Tones], |_| {}).unwrap();
        assert_eq!(library.entries.len(), 1);
        assert_eq!(
            library.stopped.as_deref(),
            Some("Tones 1B: no 168-byte dump")
        );
        assert!(library.after.is_ok());
        assert_eq!(s.sent.len(), 5);
    }

    #[test]
    fn listen_sends_only_the_identity() {
        let mut log = Quiet;
        let script = vec![(
            PodxtRequest::Identity,
            vec![PODXT_LIVE.to_vec(), vec![0xb0, 7, 100], vec![0xb0, 7, 20]],
        )];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        let (_, tally) = listen(&mut s, Duration::from_secs(3600)).unwrap();
        assert_eq!(s.sent, vec![PodxtRequest::Identity.bytes()]);
        let pedal = &tally.controllers[&(1, 7)];
        assert_eq!(
            (pedal.count, pedal.min, pedal.max, pedal.last),
            (2, 20, 100, 20)
        );
        assert!(s.link.elapsed() < LISTEN_MAX + Duration::from_secs(1));
    }

    #[test]
    fn read_sends_identity_then_edit_buffer_only_for_a_podxt() {
        let mut log = Quiet;
        let dump = synthetic();
        let script = vec![
            (PodxtRequest::Identity, vec![PODXT_LIVE.to_vec()]),
            (PodxtRequest::EditBuffer, vec![dump]),
        ];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        assert_eq!(read_current(&mut s).unwrap().tone.name(), "Test Tone");
        assert_eq!(
            s.sent,
            vec![
                PodxtRequest::Identity.bytes(),
                PodxtRequest::EditBuffer.bytes()
            ]
        );

        let mut log = Quiet;
        let script = vec![(PodxtRequest::Identity, vec![VARIAX_600.to_vec()])];
        let mut s = Session::new(Fake::new(Path::PodxtMidi, script), &AllowAll, &mut log);
        assert!(read_current(&mut s).is_err());
        assert_eq!(s.sent, vec![PodxtRequest::Identity.bytes()]);
    }
}
