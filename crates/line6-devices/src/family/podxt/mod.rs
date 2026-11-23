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
}

impl Request for PodxtRequest {
    fn bytes(self) -> &'static [u8] {
        match self {
            PodxtRequest::Identity => &identity::REQUEST,
            PodxtRequest::EditBuffer => &[0xf0, 0x00, 0x01, 0x0c, 0x03, 0x75, 0xf7],
        }
    }

    fn path(self) -> Path {
        Path::PodxtMidi
    }

    fn operation(self) -> Operation {
        match self {
            PodxtRequest::Identity => Operation::PodxtIdentity,
            PodxtRequest::EditBuffer => Operation::PodxtRead,
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
    session.send(PodxtRequest::EditBuffer)?;
    match session.wait_for(WINDOW, is_edit_buffer, false)? {
        Wait::Found(dump) => Ok(CurrentTone {
            identity_reply,
            identity,
            tone: EditBuffer::parse(dump)?,
        }),
        _ => Err("No 168-byte edit-buffer dump arrived.".into()),
    }
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
