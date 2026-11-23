//! PODxt family, reached through the device's own MIDI port.
//!
//! Requests and the edit-buffer layout come from Line 6 Edit 3.06
//! (`L6SharcTonePatchManager.RequestPatch`, `L6SharcToneMIDITranscoder.DecodePatch`),
//! confirmed by owner-run reads on 2026-11-22: name at bytes 7..23, the value
//! of MIDI controller `n` at byte `39 + n`, knobs `value / 127`, switches on
//! when `value > 63`, amp enable inverted (CC 111 set means amp off). Gate
//! threshold, compressor, delay time and tempo use other conversions and are
//! reported raw.

pub mod controls;
pub mod models;

use crate::gate::Operation;
use crate::identity::{self, Identity};
use crate::midi::ControllerTally;
use crate::session::{Link, Path, Request, Session, Wait};
use std::time::Duration;

pub const IDENTITY_FAMILY: u16 = 0x0003;
/// Line 6 Edit waits 1 s for the edit buffer; this allows a little more.
const WINDOW: Duration = Duration::from_secs(2);
pub const EDIT_BUFFER_LEN: usize = 168;
pub const LISTEN_MAX: Duration = Duration::from_secs(30);

pub fn member_name(member: u16) -> &'static str {
    match member {
        0x000a => "PODxt Live",
        _ => "PODxt family (unverified member)",
    }
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
}

impl Request for PodxtRequest {
    fn bytes(self) -> Vec<u8> {
        match self {
            PodxtRequest::Identity => identity::REQUEST.to_vec(),
            PodxtRequest::EditBuffer => vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x75, 0xf7],
            PodxtRequest::InstalledPacks => vec![0xf0, 0x00, 0x01, 0x0c, 0x03, 0x0e, 0x00, 0xf7],
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
        }
    }
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
    session.send(PodxtRequest::InstalledPacks)?;
    match session.wait_for(WINDOW, is_packs_reply, false)? {
        Wait::Found(reply) => Ok((identity_reply, Packs(reply[7]))),
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

    /// Tones as the PODxt shows channels (`1A`..`32D`); setups as `#1`...
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

/// Drain, then identity; errors unless the reply is a PODxt.
fn identify_podxt<L: Link>(session: &mut Session<L>) -> Result<(Vec<u8>, Identity), String> {
    // Drain anything already queued. Sends nothing.
    session.listen(Duration::from_millis(200))?;
    session.send(PodxtRequest::Identity)?;
    let Wait::Found(identity_reply) =
        session.wait_for(WINDOW, |m| Identity::parse(m).is_some(), false)?
    else {
        return Err("No identity reply from the PODxt. Nothing else was sent.".into());
    };
    let identity = Identity::parse(&identity_reply).expect("checked above");
    if !identity.is_line6() || identity.family != IDENTITY_FAMILY {
        return Err("The reply is not a PODxt-family identity. Nothing else was sent.".into());
    }
    Ok((identity_reply, identity))
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

fn read_edit_buffer<L: Link>(session: &mut Session<L>) -> Result<EditBuffer, String> {
    session.send(PodxtRequest::EditBuffer)?;
    match session.wait_for(WINDOW, is_edit_buffer, false)? {
        Wait::Found(dump) => EditBuffer::parse(dump),
        _ => Err("No 168-byte edit-buffer dump arrived.".into()),
    }
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
        session.send(PodxtRequest::Stored(location))?;
        match session.wait_for(STORED_WINDOW, is_edit_buffer, false)? {
            Wait::Found(dump) => {
                let stored = StoredTone {
                    location,
                    tone: EditBuffer::parse(dump)?,
                };
                progress(&stored);
                entries.push(stored);
                session.listen(STORED_GAP)?;
            }
            _ => {
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
            format!("Tempo raw : {}/{}", self.cc(89), self.cc(90)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::tests::{PODXT_LIVE, VARIAX_600};
    use crate::session::Quiet;
    use crate::session::testing::{AllowAll, Fake};

    fn synthetic() -> Vec<u8> {
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
        assert_eq!(controls::CONTROLS.len(), 102);
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
