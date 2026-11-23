//! First-generation Variax (300/500/600/700) reached through a PODxt Live.
//!
//! Requests and the metadata layout come from Variax Workbench 1.75
//! (`L6VariaxDevice`, `L6VariaxPatchManager.RequestDevicePatch`,
//! `L6VariaxMIDITranscoder.DecodePatch`), confirmed by owner-run reads on
//! 2026-11-14 and 2026-11-22. Metadata is current-preset settings, not a
//! backup.

use crate::gate::Operation;
use crate::identity::{self, Identity};
use crate::session::{Link, Path, Request, Session, Wait};
use std::collections::BTreeMap;
use std::time::Duration;

pub const IDENTITY_FAMILY: u16 = 0x0007;
const HEADER: [u8; 5] = [0xf0, 0x00, 0x01, 0x0c, 0x07];
const WINDOW: Duration = Duration::from_secs(3);
/// Workbench waits 100 ms after session on.
const SESSION_SETTLE: Duration = Duration::from_millis(100);
pub const LISTEN_MAX: Duration = Duration::from_secs(30);

pub fn member_name(member: u16) -> &'static str {
    match member {
        0 => "Variax 500/700",
        4 => "Variax 300/600",
        _ => "Variax (unknown member)",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariaxRequest {
    Identity,
    /// Session MIDI on (`2A 01`). Workbench sends it before reading.
    SessionOn,
    /// Session MIDI off (`2A 00`). Always follows `SessionOn`.
    SessionOff,
    /// Metadata block 1: 32 words at address 851.
    Metadata1,
    /// Metadata block 2: 32 words at address 883.
    Metadata2,
}

impl Request for VariaxRequest {
    fn bytes(self) -> &'static [u8] {
        match self {
            VariaxRequest::Identity => &identity::REQUEST,
            VariaxRequest::SessionOn => &[0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x2a, 0x01, 0xf7],
            VariaxRequest::SessionOff => &[0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x2a, 0x00, 0xf7],
            VariaxRequest::Metadata1 => &[
                0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x3c, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x03,
                0x05, 0x03, 0x00, 0x00, 0x00, 0xf7,
            ],
            VariaxRequest::Metadata2 => &[
                0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x3c, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x03,
                0x07, 0x03, 0x00, 0x00, 0x00, 0xf7,
            ],
        }
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        match self {
            VariaxRequest::Identity => Operation::VariaxIdentity,
            _ => Operation::VariaxRead,
        }
    }
}

pub struct Identified {
    pub reply: Vec<u8>,
    pub identity: Identity,
    /// 1, or 2 when the first reply lost its start and the request was resent.
    pub requests: u8,
}

fn is_identity_reply(message: &[u8]) -> bool {
    Identity::parse(message).is_some()
}

/// Identity request. On PODxt Live interface 1 the first reply packet after
/// taking the interface can be lost, leaving only its tail. When that tail
/// arrives without its start, the same request is sent exactly once more.
/// Silence is not retried.
pub fn identify<L: Link>(session: &mut Session<L>) -> Result<Identified, String> {
    let orphans_before = session.assembler.orphans;
    session.send(VariaxRequest::Identity)?;
    let mut requests = 1;
    let reply = match session.wait_for(WINDOW, is_identity_reply, true)? {
        Wait::Found(reply) => reply,
        _ if session.assembler.orphans > orphans_before => {
            let lost = session.assembler.orphans - orphans_before;
            session.note(format!(
                "reply tail without its start ({lost} orphan bytes): first packet lost. \
                 Sending the identity request once more."
            ));
            session.send(VariaxRequest::Identity)?;
            requests = 2;
            match session.wait_for(WINDOW, is_identity_reply, false)? {
                Wait::Found(reply) => reply,
                _ => return Err("No complete identity reply after the second request.".into()),
            }
        }
        _ => {
            return Err(
                "No reply to the identity request: nothing arrived from the guitar.".into(),
            );
        }
    };
    let identity = Identity::parse(&reply).expect("checked by is_identity_reply");
    Ok(Identified {
        reply,
        identity,
        requests,
    })
}

fn identify_variax<L: Link>(session: &mut Session<L>) -> Result<Identified, String> {
    let identified = identify(session)?;
    if !identified.identity.is_line6() || identified.identity.family != IDENTITY_FAMILY {
        return Err("The reply is not a Variax identity. Nothing else was sent.".into());
    }
    Ok(identified)
}

/// Session on, `body`, then session off, which is sent even when `body` fails.
fn in_session<L: Link, T>(
    session: &mut Session<L>,
    body: impl FnOnce(&mut Session<L>) -> Result<T, String>,
) -> Result<T, String> {
    if let Err(error) = session.send(VariaxRequest::SessionOn) {
        let _ = session.send(VariaxRequest::SessionOff);
        return Err(error);
    }
    session.link.pause(SESSION_SETTLE);
    let result = body(session);
    let off = session.send(VariaxRequest::SessionOff);
    match (result, off) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(off)) => Err(format!("Session off failed: {off}")),
        (Err(error), Err(off)) => Err(format!("{error} Session off also failed: {off}")),
    }
}

fn is_metadata_reply(message: &[u8]) -> bool {
    message.len() == 200
        && message[..5] == HEADER
        && matches!(message[5], 0 | 4)
        && message[6] == 0x3c
}

fn read_block<L: Link>(
    session: &mut Session<L>,
    request: VariaxRequest,
) -> Result<Vec<u8>, String> {
    session.send(request)?;
    match session.wait_for(WINDOW, is_metadata_reply, false)? {
        Wait::Found(reply) => Ok(reply),
        _ => Err(format!("No 200-byte metadata reply to {request:?}.")),
    }
}

pub struct CurrentPreset {
    pub identified: Identified,
    pub replies: [Vec<u8>; 2],
    pub metadata: PresetMetadata,
}

/// Identity, session on, metadata blocks 1 and 2, session off.
pub fn read_current<L: Link>(session: &mut Session<L>) -> Result<CurrentPreset, String> {
    let identified = identify_variax(session)?;
    let replies = in_session(session, |session| {
        Ok([
            read_block(session, VariaxRequest::Metadata1)?,
            read_block(session, VariaxRequest::Metadata2)?,
        ])
    })?;
    let metadata = PresetMetadata::from_replies(&replies[0], &replies[1])?;
    Ok(CurrentPreset {
        identified,
        replies,
        metadata,
    })
}

/// Identity, session on, listen only (at most [`LISTEN_MAX`]), session off.
pub fn listen<L: Link>(session: &mut Session<L>, window: Duration) -> Result<Identified, String> {
    let identified = identify_variax(session)?;
    let window = window.min(LISTEN_MAX);
    in_session(session, |session| {
        session.note(format!(
            "listening {} s: move the guitar's knobs now",
            window.as_secs()
        ));
        session.listen(window)?;
        session.note("listening finished");
        Ok(())
    })?;
    Ok(identified)
}

// ---- metadata decoder (same layout as the main crate's `PresetMetadata`) ----

fn decode_word(nibbles: &[u8]) -> Result<u32, String> {
    if nibbles.len() != 6 || nibbles.iter().any(|&n| n > 15) {
        return Err("word must contain exactly six nibbles".into());
    }
    Ok(nibbles
        .iter()
        .enumerate()
        .fold(0, |v, (i, &n)| v | (u32::from(n) << (4 * i))))
}

/// 96 bytes from one 200-byte reply: 32 words of six low-to-high nibbles,
/// each word kept as its three big-endian bytes.
pub fn decode_metadata_reply(reply: &[u8]) -> Result<[u8; 96], String> {
    if !is_metadata_reply(reply) || reply[199] != 0xf7 {
        return Err("unexpected metadata reply framing".into());
    }
    let mut output = [0; 96];
    for (i, nibbles) in reply[7..199].chunks(6).enumerate() {
        output[i * 3..i * 3 + 3].copy_from_slice(&decode_word(nibbles)?.to_be_bytes()[1..]);
    }
    Ok(output)
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Integer(i32),
    Boolean(bool),
    Float(f32),
}

/// Names use Workbench's internal string numbering, which runs opposite to
/// the labels shown in the original UI. Raw bytes are kept.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetMetadata {
    pub name: String,
    pub parameters: BTreeMap<&'static str, ParameterValue>,
    pub raw: [u8; 192],
}

impl PresetMetadata {
    /// Replies in request order; the format carries no address to detect a swap.
    pub fn from_replies(first: &[u8], second: &[u8]) -> Result<Self, String> {
        let mut raw = [0; 192];
        raw[..96].copy_from_slice(&decode_metadata_reply(first)?);
        raw[96..].copy_from_slice(&decode_metadata_reply(second)?);
        Self::from_bytes(raw)
    }

    pub fn from_bytes(raw: [u8; 192]) -> Result<Self, String> {
        if !raw[..18].is_ascii() {
            return Err("preset name contains non-ASCII bytes".into());
        }
        let name = String::from_utf8_lossy(&raw[..18])
            .trim_end_matches(['\0', ' '])
            .to_owned();
        let mut parameters = BTreeMap::new();
        let mut cursor = 18;
        // Field order and types from Workbench `DecodePatch`.
        let mut read = |key, width: usize, kind| {
            let bytes = &raw[cursor..cursor + width];
            cursor += width;
            let value = match kind {
                'i' => ParameterValue::Integer(i32::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3],
                ])),
                'b' => ParameterValue::Boolean(bytes[0] != 0),
                'u' => ParameterValue::Integer(i32::from(bytes[0])),
                's' => ParameterValue::Integer(i32::from(bytes[0] as i8)),
                'f' => ParameterValue::Float(f32::from_bits(u32::from_be_bytes([
                    bytes[0], bytes[1], bytes[2], 0,
                ]))),
                _ => unreachable!(),
            };
            parameters.insert(key, value);
        };
        read("body_model", 4, 'i');
        for (enabled, model, position, angle, level) in [
            (
                "pickup1_enabled",
                "pickup1_model",
                "pickup1_position",
                "pickup1_angle",
                "pickup1_level",
            ),
            (
                "pickup2_enabled",
                "pickup2_model",
                "pickup2_position",
                "pickup2_angle",
                "pickup2_level",
            ),
        ] {
            read(enabled, 1, 'b');
            read(model, 4, 'i');
            read(position, 3, 'f');
            read(angle, 3, 'f');
            read(level, 3, 'f');
        }
        read("pickup2_phase_reverse", 1, 'b');
        for key in ["tone_capacitance", "tone_resistance", "volume_resistance"] {
            read(key, 3, 'f');
        }
        read("volume_taper", 1, 'u');
        read("tone_position", 3, 'f');
        read("save_tone_position", 1, 'b');
        read("trim", 3, 'f');
        read("alternate_tuning_enabled", 1, 'b');
        for key in [
            "string1_tuning",
            "string2_tuning",
            "string3_tuning",
            "string4_tuning",
            "string5_tuning",
            "string6_tuning",
        ] {
            read(key, 1, 's');
        }
        for key in [
            "string1_detune",
            "string2_detune",
            "string3_detune",
            "string4_detune",
            "string5_detune",
            "string6_detune",
            "string1_mix",
            "string2_mix",
            "string3_mix",
            "string4_mix",
            "string5_mix",
            "string6_mix",
            "string_spread",
        ] {
            read(key, 3, 'f');
        }
        read("pickups_in_series", 1, 'b');
        Ok(Self {
            name,
            parameters,
            raw,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::identity::tests::{PODXT_LIVE, VARIAX_600};
    use crate::session::Quiet;
    use crate::session::testing::{AllowAll, Fake, packets};

    /// Encodes 96 metadata bytes the way the guitar does (inverse of the decoder).
    pub fn encode_reply(bytes: &[u8; 96]) -> Vec<u8> {
        let mut reply = vec![0xf0, 0x00, 0x01, 0x0c, 0x07, 0x04, 0x3c];
        for word in bytes.chunks(3) {
            let value = u32::from_be_bytes([0, word[0], word[1], word[2]]);
            reply.extend((0..6).map(|i| ((value >> (4 * i)) & 15) as u8));
        }
        reply.push(0xf7);
        reply
    }

    fn zero_reply() -> Vec<u8> {
        encode_reply(&[0; 96])
    }

    fn run<T>(
        script: Vec<(VariaxRequest, Vec<Vec<u8>>)>,
        body: impl FnOnce(&mut Session<Fake>) -> T,
    ) -> (T, Vec<&'static [u8]>, Duration) {
        let mut log = Quiet;
        let mut session =
            Session::new(Fake::new(Path::VariaxViaPodxt, script), &AllowAll, &mut log);
        let result = body(&mut session);
        (result, session.sent.clone(), session.link.elapsed())
    }

    use VariaxRequest::*;

    #[test]
    fn complete_first_reply_needs_one_request() {
        let (result, sent, _) = run(vec![(Identity, packets(&VARIAX_600))], identify);
        assert_eq!(result.unwrap().requests, 1);
        assert_eq!(sent.len(), 1);
    }

    #[test]
    fn lost_first_packet_resends_once_without_waiting_the_whole_window() {
        let (result, sent, elapsed) = run(
            vec![
                (Identity, vec![vec![0xf7]]),
                (Identity, packets(&VARIAX_600)),
            ],
            identify,
        );
        assert_eq!(result.unwrap().requests, 2);
        assert_eq!(sent.len(), 2);
        assert!(elapsed < Duration::from_secs(1));
    }

    #[test]
    fn silence_is_not_resent_and_two_lost_starts_stop() {
        let (result, sent, _) = run(vec![(Identity, vec![])], identify);
        assert!(result.is_err() && sent.len() == 1);
        let (result, sent, _) = run(
            vec![(Identity, vec![vec![0xf7]]), (Identity, vec![vec![0xf7]])],
            identify,
        );
        assert!(result.is_err() && sent.len() == 2);
    }

    #[test]
    fn read_sends_exactly_the_authorized_sequence() {
        let reply = zero_reply();
        let (result, sent, _) = run(
            vec![
                (Identity, packets(&VARIAX_600)),
                (SessionOn, vec![]),
                (Metadata1, packets(&reply)),
                (Metadata2, packets(&reply)),
                (SessionOff, vec![]),
            ],
            read_current,
        );
        assert_eq!(result.unwrap().replies, [reply.clone(), reply]);
        let expected: Vec<&[u8]> = [Identity, SessionOn, Metadata1, Metadata2, SessionOff]
            .iter()
            .map(|r| r.bytes())
            .collect();
        assert_eq!(sent, expected);
    }

    #[test]
    fn session_off_is_sent_when_a_block_never_arrives() {
        let (result, sent, _) = run(
            vec![
                (Identity, packets(&VARIAX_600)),
                (SessionOn, vec![]),
                (Metadata1, packets(&zero_reply())),
                (Metadata2, vec![]),
                (SessionOff, vec![]),
            ],
            read_current,
        );
        assert!(result.is_err());
        assert_eq!(sent.last(), Some(&SessionOff.bytes()));
    }

    #[test]
    fn a_non_variax_identity_stops_before_session_on() {
        let (result, sent, _) = run(vec![(Identity, packets(&PODXT_LIVE))], read_current);
        assert!(result.is_err());
        assert_eq!(sent, vec![Identity.bytes()]);
    }

    #[test]
    fn listen_only_adds_session_on_and_off_and_is_capped() {
        let (result, sent, elapsed) = run(
            vec![
                (Identity, packets(&VARIAX_600)),
                (SessionOn, vec![vec![0xb0, 0x07, 0x0e], vec![0x07, 0x0f]]),
                (SessionOff, vec![]),
            ],
            |s| listen(s, Duration::from_secs(3600)),
        );
        assert!(result.is_ok());
        assert_eq!(
            sent,
            vec![Identity.bytes(), SessionOn.bytes(), SessionOff.bytes()]
        );
        assert!(elapsed < LISTEN_MAX + Duration::from_secs(1));
    }

    #[test]
    fn metadata_fields_decode_from_their_offsets() {
        let mut raw = [0u8; 192];
        raw[..6].copy_from_slice(b"Chime5");
        raw[18..22].copy_from_slice(&0x0310_000c_i32.to_be_bytes());
        raw[22] = 1; // pickup1_enabled
        raw[27..30].copy_from_slice(&2.0f32.to_bits().to_be_bytes()[..3]); // pickup1_position
        let mut first = [0; 96];
        first.copy_from_slice(&raw[..96]);
        let metadata = PresetMetadata::from_replies(&encode_reply(&first), &zero_reply()).unwrap();
        assert_eq!(metadata.name, "Chime5");
        assert_eq!(
            metadata.parameters["body_model"],
            ParameterValue::Integer(0x0310_000c)
        );
        assert_eq!(
            metadata.parameters["pickup1_enabled"],
            ParameterValue::Boolean(true)
        );
        assert_eq!(
            metadata.parameters["pickup1_position"],
            ParameterValue::Float(2.0)
        );
        assert_eq!(metadata.parameters.len(), 40);
    }

    #[test]
    fn requests_stay_on_the_variax_path() {
        for request in [Identity, SessionOn, SessionOff, Metadata1, Metadata2] {
            assert_eq!(request.path(), Path::VariaxViaPodxt);
            let bytes = request.bytes();
            assert!(bytes[0] == 0xf0 && bytes[bytes.len() - 1] == 0xf7);
        }
    }
}
