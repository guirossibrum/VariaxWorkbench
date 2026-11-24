//! First-generation Variax (300/500/600/700) reached through a PODxt Live.
//!
//! Requests and the metadata layout come from Variax Workbench 1.75
//! (`L6VariaxDevice`, `L6VariaxPatchManager.RequestDevicePatch`,
//! `L6VariaxMIDITranscoder.DecodePatch`), confirmed by owner-run reads on
//! 2026-11-14 and 2026-11-22. Metadata is current-preset settings, not a
//! backup.

pub mod backup;
mod follow;
pub use follow::{FollowUpdate, follow};
pub mod slot_read;
pub mod span_read;
pub mod transport;
pub mod write;

use crate::gate::Operation;
use crate::identity::{self, Identity};
use crate::midi::ControllerTally;
use crate::session::{Link, Path, Request, Session, Wait};
use std::collections::BTreeMap;
use std::time::Duration;

pub const IDENTITY_FAMILY: u16 = 0x0007;
const HEADER: [u8; 5] = [0xf0, 0x00, 0x01, 0x0c, 0x07];
const WINDOW: Duration = Duration::from_secs(3);
const BACKUP_BLOCK_WINDOW: Duration = Duration::from_millis(2500);
const BANK_REPLY_WINDOW: Duration = Duration::from_secs(1);
const BANK_NAME_SIZE: usize = 18;
const BRIDGE_CHUNK: usize = 256;
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
    /// Name of the currently selected bank.
    BankName,
    /// Program change to a stored preset: switches the guitar's active preset.
    Program(Preset),
}

impl Request for VariaxRequest {
    fn bytes(self) -> Vec<u8> {
        let fixed: &[u8] = match self {
            // Workbench `L6MIDIManager.SendPC`: status C0 on its transmit channel 1.
            VariaxRequest::Program(preset) => return vec![0xc0, preset.program_number()],
            VariaxRequest::Identity => &identity::REQUEST,
            VariaxRequest::SessionOn => &[0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x2a, 0x01, 0xf7],
            VariaxRequest::SessionOff => &[0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x2a, 0x00, 0xf7],
            VariaxRequest::Metadata1 => &[
                0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x3c, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x03,
                0x05, 0x03, 0x00, 0x00, 0x00, 0xf7,
            ],
            VariaxRequest::BankName => &[0xf0, 0, 1, 0x0c, 7, 0, 0x6d, 0xf7],
            VariaxRequest::Metadata2 => &[
                0xf0, 0x00, 0x01, 0x0c, 0x07, 0x00, 0x3c, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x03,
                0x07, 0x03, 0x00, 0x00, 0x00, 0xf7,
            ],
        };
        fixed.to_vec()
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        match self {
            VariaxRequest::Identity => Operation::VariaxIdentity,
            VariaxRequest::Program(_) => Operation::VariaxPresetSwitch,
            VariaxRequest::BankName => Operation::VariaxBankNameRead,
            _ => Operation::VariaxRead,
        }
    }
}

/// Variax 300/600 banks in Workbench order (`L6VariaxDevice` topology): the
/// model knob positions, five presets each.
pub const BANKS: [&str; 12] = [
    "Custom 1", "T-Model", "Spank", "Lester", "Special", "R-Billy", "Chime", "Semi", "Jazzbox",
    "Acoustic", "Reso", "Custom 2",
];
pub const PRESETS_PER_BANK: u8 = 5;

/// One stored preset location. Only valid locations can be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    bank: u8,
    slot: u8,
}

impl Preset {
    pub fn new(bank: u8, slot: u8) -> Option<Self> {
        (usize::from(bank) < BANKS.len() && slot < PRESETS_PER_BANK).then_some(Self { bank, slot })
    }

    pub fn all() -> impl Iterator<Item = Preset> {
        (0..BANKS.len() as u8)
            .flat_map(|bank| (0..PRESETS_PER_BANK).map(move |slot| Preset { bank, slot }))
    }

    pub fn bank(self) -> u8 {
        self.bank
    }

    pub fn slot(self) -> u8 {
        self.slot
    }

    pub fn bank_name(self) -> &'static str {
        BANKS[usize::from(self.bank)]
    }

    /// Workbench `L6VariaxPatchManager.LocationToProgramNumber`:
    /// `bank * 5 + (4 - slot)`, plus 55 for banks 1..=10.
    pub fn program_number(self) -> u8 {
        let base = self.bank * PRESETS_PER_BANK + (PRESETS_PER_BANK - 1 - self.slot);
        if self.bank == 0 || usize::from(self.bank) == BANKS.len() - 1 {
            base
        } else {
            base + 55
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
    identify_if_present(session)?
        .ok_or_else(|| "No reply to the identity request: nothing arrived from the guitar.".into())
}

/// The same bounded identity exchange as [`identify`]. A timeout without
/// evidence of a lost packet returns `None`. A failed resend or a transport
/// error still stops the caller.
pub fn identify_if_present<L: Link>(
    session: &mut Session<L>,
) -> Result<Option<Identified>, String> {
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
        _ => return Ok(None),
    };
    let identity = Identity::parse(&reply).expect("checked by is_identity_reply");
    Ok(Some(Identified {
        reply,
        identity,
        requests,
    }))
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
        return match session.send(VariaxRequest::SessionOff) {
            Ok(()) => Err(error),
            Err(off) => Err(format!("{error} Session off also failed: {off}")),
        };
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
    window: Duration,
) -> Result<Vec<u8>, String> {
    session.send(request)?;
    match session.wait_for(window, is_metadata_reply, false)? {
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
    read_current_checked(session, |_| Ok(()))
}

/// Read the same two metadata blocks, after checking the identity on this link.
pub fn read_current_checked<L: Link>(
    session: &mut Session<L>,
    check_identity: impl FnOnce(&Identity) -> Result<(), String>,
) -> Result<CurrentPreset, String> {
    let identified = identify_variax(session)?;
    check_identity(&identified.identity)?;
    let replies = in_session(session, |session| {
        Ok([
            read_block(session, VariaxRequest::Metadata1, WINDOW)?,
            read_block(session, VariaxRequest::Metadata2, WINDOW)?,
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
pub fn listen<L: Link>(
    session: &mut Session<L>,
    window: Duration,
) -> Result<(Identified, ControllerTally), String> {
    let identified = identify_variax(session)?;
    let window = window.min(LISTEN_MAX);
    let tally = in_session(session, |session| {
        session.note(format!(
            "listening {} s: move the guitar's knobs now",
            window.as_secs()
        ));
        let tally = session.listen(window)?;
        session.note("listening finished");
        Ok(tally)
    })?;
    Ok((identified, tally))
}

// ---- full library (G5 + preset switching) ----

fn bridge_message(enabled: bool) -> Vec<u8> {
    vec![0xf0, 0, 1, 0x0c, 3, 0x23, u8::from(enabled), 0xf7]
}

#[derive(Clone, Copy, Debug)]
struct ReadBridge(bool);

impl Request for ReadBridge {
    fn bytes(self) -> Vec<u8> {
        bridge_message(self.0)
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        Operation::VariaxRead
    }
}

/// Toggle the PODxt Live's Variax messages on interface 1, under the Variax
/// read gate. The caller must attempt release after enabling.
pub fn set_bridge_for_read<L: Link>(session: &mut Session<L>, enabled: bool) -> Result<(), String> {
    session.send(ReadBridge(enabled))
}

fn with_bridged_read_session<L: Link, T>(
    session: &mut Session<L>,
    bridge_settle: Duration,
    body: impl FnOnce(&mut Session<L>) -> Result<T, String>,
) -> Result<T, String> {
    let result = session.send(ReadBridge(true)).and_then(|()| {
        if !bridge_settle.is_zero() {
            session.link.pause(bridge_settle);
        }
        in_session(session, body)
    });
    let release = session
        .send(ReadBridge(false))
        .err()
        .map(|error| format!("PODxt Variax-messages release failed: {error}"));
    match (result, release) {
        (result, None) => result,
        (Ok(_), Some(error)) => Err(error),
        (Err(error), release) => Err(with_release_error(error, release)),
    }
}

#[derive(Clone, Copy, Debug)]
struct LibraryBridge(bool);

impl Request for LibraryBridge {
    fn bytes(self) -> Vec<u8> {
        bridge_message(self.0)
    }

    fn path(self) -> Path {
        Path::VariaxViaPodxt
    }

    fn operation(self) -> Operation {
        Operation::VariaxLibraryBridge
    }
}

fn with_library_bridge<L: Link, T>(
    session: &mut Session<L>,
    body: impl FnOnce(&mut Session<L>) -> Result<T, String>,
) -> (Result<T, String>, Option<String>) {
    let result = session
        .send(LibraryBridge(true))
        .and_then(|()| body(session));
    // A failed enable can still have reached the bridge.
    let release_error = session
        .send(LibraryBridge(false))
        .err()
        .map(|error| format!("PODxt Variax-messages release failed: {error}"));
    (result, release_error)
}

fn with_release_error(error: String, release_error: Option<String>) -> String {
    match release_error {
        Some(release) => format!("{error} {release}"),
        None => error,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct BankName {
    pub name: String,
    pub reply: Vec<u8>,
}

fn is_reply(message: &[u8], command: u8) -> bool {
    message.starts_with(&HEADER)
        && message
            .get(5)
            .is_some_and(|channel| matches!(channel, 0 | 4))
        && message.get(6) == Some(&command)
        && message.ends_with(&[0xf7])
}

fn is_bank_name_reply(reply: &[u8]) -> bool {
    reply.len() >= 26 && is_reply(reply, 0x6d)
}

impl BankName {
    pub(crate) fn from_reply(reply: Vec<u8>) -> Result<Self, String> {
        if !is_bank_name_reply(&reply) {
            return Err("Unexpected Variax bank-name reply framing.".into());
        }
        let field = &reply[7..7 + BANK_NAME_SIZE];
        if field.iter().any(|byte| !matches!(byte, 0 | 0x20..=0x7e)) {
            return Err("Invalid ASCII in the Variax bank-name reply.".into());
        }
        let name = String::from_utf8_lossy(field)
            .trim_end_matches(['\0', ' '])
            .to_owned();
        Ok(Self { name, reply })
    }
}

fn read_current_bank_name<L: Link>(session: &mut Session<L>) -> Result<BankName, String> {
    session.send_chunked_sysex(
        VariaxRequest::BankName,
        BRIDGE_CHUNK,
        Some(HEADER.len() + 1),
        || Ok(()),
    )?;
    match session.wait_for(BANK_REPLY_WINDOW, is_bank_name_reply, false)? {
        Wait::Found(reply) => BankName::from_reply(reply),
        _ => Err("Timed out waiting for the Variax bank-name reply.".into()),
    }
}

/// Read the selected bank name without a program change.
pub fn read_bank_name<L: Link>(session: &mut Session<L>) -> Result<String, String> {
    if !session.allows(Operation::VariaxBankNameRead) {
        return Err("Reading the Variax bank name requires VariaxBankNameRead.".into());
    }
    let (result, release_error) = with_library_bridge(session, |session| {
        in_session(session, read_current_bank_name)
    });
    match (result, release_error) {
        (Ok(bank), None) => Ok(bank.name),
        (Ok(_), Some(error)) => Err(error),
        (Err(error), release_error) => Err(with_release_error(error, release_error)),
    }
}

pub struct StoredPreset {
    pub preset: Preset,
    pub replies: [Vec<u8>; 2],
    pub metadata: PresetMetadata,
}

#[derive(Debug, PartialEq)]
pub enum Restore {
    /// Switched back to the preset matching the original; `verified` when a
    /// read afterwards matched the original again.
    Switched {
        to: Preset,
        verified: bool,
    },
    /// No single stored preset matched the original; nothing was switched.
    NoUniqueMatch {
        matches: usize,
    },
    Failed(String),
}

pub struct Library {
    pub identified: Identified,
    pub original_replies: [Vec<u8>; 2],
    pub original: PresetMetadata,
    pub presets: Vec<StoredPreset>,
    /// Names read after all five presets of each completed bank.
    pub bank_names: Vec<(u8, BankName)>,
    pub restore: Restore,
    /// Separate from the sweep result so captured presets can still be saved.
    pub release_error: Option<String>,
    /// Why the sweep stopped early, if it did.
    pub stopped: Option<String>,
}

/// Match two active-preset captures for reselection or read-back verification.
/// The three tone-position bytes can reflect knob movement even when saved
/// tone is enabled. The saved-tone flag at byte 64 is part of the match.
pub fn same_preset(a: &PresetMetadata, b: &PresetMetadata) -> bool {
    const TONE_POSITION: std::ops::Range<usize> = 61..64;
    a.raw[..TONE_POSITION.start] == b.raw[..TONE_POSITION.start]
        && a.raw[TONE_POSITION.end..] == b.raw[TONE_POSITION.end..]
}

fn read_metadata<L: Link>(
    session: &mut Session<L>,
) -> Result<([Vec<u8>; 2], PresetMetadata), String> {
    let replies = [
        read_block(session, VariaxRequest::Metadata1, BACKUP_BLOCK_WINDOW)?,
        read_block(session, VariaxRequest::Metadata2, BACKUP_BLOCK_WINDOW)?,
    ];
    let metadata = PresetMetadata::from_replies(&replies[0], &replies[1])?;
    Ok((replies, metadata))
}

fn read_stored<L: Link>(session: &mut Session<L>, preset: Preset) -> Result<StoredPreset, String> {
    session.send(VariaxRequest::Program(preset))?;
    let (replies, metadata) = read_metadata(session)?;
    Ok(StoredPreset {
        preset,
        replies,
        metadata,
    })
}

fn restore<L: Link>(
    session: &mut Session<L>,
    original: &PresetMetadata,
    presets: &[StoredPreset],
) -> Restore {
    let matches: Vec<Preset> = presets
        .iter()
        .filter(|stored| same_preset(&stored.metadata, original))
        .map(|stored| stored.preset)
        .collect();
    let [to] = matches[..] else {
        return Restore::NoUniqueMatch {
            matches: matches.len(),
        };
    };
    if let Err(error) = session.send(VariaxRequest::Program(to)) {
        return Restore::Failed(error);
    }
    match read_metadata(session) {
        Ok((_, now)) => Restore::Switched {
            to,
            verified: same_preset(&now, original),
        },
        Err(error) => Restore::Failed(format!("switched back, but the check read failed: {error}")),
    }
}

/// Identify, enable the bridge and open the MIDI session, then read the
/// current preset. Visit all stored presets and read each bank name after its
/// fifth preset. Stop at the first failed read and try to reselect and verify
/// the original preset. Close the session and release the bridge afterwards.
pub fn read_library<L: Link>(
    session: &mut Session<L>,
    progress: impl FnMut(&StoredPreset),
) -> Result<Library, String> {
    sweep_library(session, || false, progress)
}

fn sweep_library<L: Link>(
    session: &mut Session<L>,
    mut cancelled: impl FnMut() -> bool,
    mut progress: impl FnMut(&StoredPreset),
) -> Result<Library, String> {
    const CANCELLED: &str = "Variax library read cancelled.";
    if cancelled() {
        return Err(CANCELLED.into());
    }
    let identified = identify_variax(session)?;
    if cancelled() {
        return Err(CANCELLED.into());
    }
    let (result, release_error) = with_library_bridge(session, |session| {
        in_session(session, |session| {
            let (original_replies, original) = read_metadata(session)?;
            let mut presets = Vec::new();
            let mut bank_names = Vec::new();
            let mut stopped = None;
            for preset in Preset::all() {
                if cancelled() {
                    stopped = Some(CANCELLED.into());
                    break;
                }
                let stored = match read_stored(session, preset) {
                    Ok(stored) => stored,
                    Err(error) => {
                        stopped = Some(format!(
                            "{} {}: {error}",
                            preset.bank_name(),
                            preset.slot() + 1
                        ));
                        break;
                    }
                };
                progress(&stored);
                presets.push(stored);
                if preset.slot() + 1 == PRESETS_PER_BANK {
                    if cancelled() {
                        stopped = Some(CANCELLED.into());
                        break;
                    }
                    match read_current_bank_name(session) {
                        Ok(name) => bank_names.push((preset.bank(), name)),
                        Err(error) => {
                            stopped = Some(format!("{} bank name: {error}", preset.bank_name()));
                            break;
                        }
                    }
                }
            }
            let restore = restore(session, &original, &presets);
            Ok(Library {
                identified,
                original_replies,
                original,
                presets,
                bank_names,
                restore,
                stopped,
                release_error: None,
            })
        })
    });
    match result {
        Ok(mut library) => {
            library.release_error = release_error;
            Ok(library)
        }
        Err(error) => Err(with_release_error(error, release_error)),
    }
}

// ---- live controls ----

/// A controller the guitar sends on channel 1 while its session is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Control {
    pub name: &'static str,
    /// Decoded by Variax Workbench 1.75 `L6VariaxMIDITranscoder.DecodeCCParamChange`.
    pub workbench: bool,
    /// Seen to follow the physical control in an owner-run test (Variax 600).
    pub verified_on_device: bool,
}

/// Controllers the guitar sends while its session is on.
pub fn control(controller: u8) -> Option<Control> {
    let (name, workbench, verified_on_device) = match controller {
        // Workbench: CC 79 sets TONE_KNOB_POSITION to value / 127.
        // Owner test 2026-11-22: full sweeps gave every value 0..=127.
        79 => ("tone knob position", true, true),
        // Not decoded by Workbench. Owner test 2026-11-22: the volume knob
        // swept 127 -> 0 -> 127 on CC 7, one message per step.
        7 => ("volume knob position", false, true),
        // Workbench: CC 84 calls HandleTuningOverride. Not expected from the
        // Variax 600, whose alternate tunings are stored in each preset.
        84 => ("tuning override", true, false),
        _ => return None,
    };
    Some(Control {
        name,
        workbench,
        verified_on_device,
    })
}

/// Knob position 0.0..=1.0 (value / 127; 127 is fully up).
pub fn knob_position(value: u8) -> f32 {
    f32::from(value) / 127.0
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
    ) -> (T, Vec<Vec<u8>>, Duration) {
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
        let expected: Vec<Vec<u8>> = [Identity, SessionOn, Metadata1, Metadata2, SessionOff]
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
                (
                    SessionOn,
                    vec![vec![0xb0, 0x07, 0x0e], vec![0x07, 0x0f], vec![0xb0, 79, 64]],
                ),
                (SessionOff, vec![]),
            ],
            |s| listen(s, Duration::from_secs(3600)),
        );
        let (_, tally) = result.unwrap();
        let volume = &tally.controllers[&(1, 7)];
        assert_eq!((volume.count, volume.first, volume.last), (2, 14, 15));
        assert_eq!(tally.controllers[&(1, 79)].last, 64);
        assert_eq!(
            sent,
            vec![Identity.bytes(), SessionOn.bytes(), SessionOff.bytes()]
        );
        assert!(elapsed < LISTEN_MAX + Duration::from_secs(1));
    }

    #[test]
    fn knob_controllers_are_named_with_their_evidence() {
        let tone = control(79).unwrap();
        assert!(tone.workbench && tone.verified_on_device);
        let volume = control(7).unwrap();
        assert!(!volume.workbench && volume.verified_on_device);
        assert_eq!(control(84).unwrap().name, "tuning override");
        assert!(control(1).is_none());
        assert_eq!(knob_position(127), 1.0);
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
    fn preset_program_numbers_follow_workbench() {
        let numbers: Vec<u8> = Preset::all().map(Preset::program_number).collect();
        assert_eq!(numbers.len(), 60);
        assert_eq!(&numbers[..5], &[4, 3, 2, 1, 0]); // Custom 1
        assert_eq!(&numbers[5..10], &[64, 63, 62, 61, 60]); // T-Model: 5 + 4 - slot + 55
        assert_eq!(&numbers[55..], &[59, 58, 57, 56, 55]); // Custom 2
        let mut unique = numbers.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 60);
        assert!(numbers.iter().all(|n| *n < 0x80));
        assert!(Preset::new(12, 0).is_none() && Preset::new(0, 5).is_none());
        assert_eq!(Program(Preset::new(6, 4).unwrap()).bytes(), vec![0xc0, 85]);
        assert_eq!(
            Program(Preset::new(6, 4).unwrap()).operation(),
            Operation::VariaxPresetSwitch
        );
    }

    fn metadata_reply_named(name: &[u8]) -> Vec<u8> {
        let mut raw = [0u8; 96];
        raw[..name.len()].copy_from_slice(name);
        encode_reply(&raw)
    }

    type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

    const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
    const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
    const BLOCK_1: &[u8] = &[
        0xf0, 0, 1, 0x0c, 7, 0, 0x3c, 0, 2, 0, 0, 0, 0, 3, 5, 3, 0, 0, 0, 0xf7,
    ];
    const BLOCK_2: &[u8] = &[
        0xf0, 0, 1, 0x0c, 7, 0, 0x3c, 0, 2, 0, 0, 0, 0, 3, 7, 3, 0, 0, 0, 0xf7,
    ];
    const BANK_HEADER: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0];

    fn expect_block_pair(script: &mut Script, reply: &[u8]) {
        script.push((BLOCK_1.to_vec(), packets(reply)));
        script.push((BLOCK_2.to_vec(), packets(&zero_reply())));
    }

    fn expect_bank_request(script: &mut Script, replies: Vec<Vec<u8>>) {
        script.extend([
            (BANK_HEADER.to_vec(), vec![]),
            (vec![0x6d], vec![]),
            (vec![0xf7], replies),
        ]);
    }

    pub(crate) fn bank_reply(name: &[u8]) -> Vec<u8> {
        let mut reply = vec![0xf0, 0, 1, 0x0c, 7, 4, 0x6d];
        reply.extend(name);
        reply.resize(25, 0);
        reply.push(0xf7);
        reply
    }

    fn library_start(original: &[u8]) -> Script {
        let mut script = vec![
            (Identity.bytes(), packets(&VARIAX_600)),
            (BRIDGE_ON.to_vec(), vec![]),
            (SessionOn.bytes(), vec![]),
        ];
        expect_block_pair(&mut script, original);
        script
    }

    fn library_end(script: &mut Script) {
        script.extend([(SessionOff.bytes(), vec![]), (BRIDGE_OFF.to_vec(), vec![])]);
    }

    fn expect_bank_presets(script: &mut Script, bank: u8, stored: impl Fn(u8) -> Vec<u8>) {
        // Independent Workbench bank mapping, in model-knob order.
        let last = [4, 64, 69, 74, 79, 84, 89, 94, 99, 104, 109, 59][bank as usize];
        for program in (last - 4..=last).rev() {
            script.push((vec![0xc0, program], vec![]));
            expect_block_pair(script, &stored(program));
        }
    }

    struct BackupOnly;
    impl crate::gate::GatePolicy for BackupOnly {
        fn allows(&self, operation: Operation) -> bool {
            matches!(
                operation,
                Operation::VariaxIdentity
                    | Operation::VariaxRead
                    | Operation::VariaxPresetSwitch
                    | Operation::VariaxBankNameRead
                    | Operation::VariaxLibraryBridge
            )
        }
    }

    fn run_backup(
        script: Script,
        fail_at: Vec<usize>,
    ) -> (Result<Library, String>, Vec<Vec<u8>>, Duration) {
        let mut log = Quiet;
        let mut fake = Fake::from_bytes(Path::VariaxViaPodxt, script);
        fake.fail_at = fail_at;
        let mut session = Session::new(fake, &BackupOnly, &mut log);
        let result = read_library(&mut session, |_| {});
        session.link.assert_finished();
        (result, session.sent, session.link.elapsed())
    }

    fn named_program(program: u8) -> Vec<u8> {
        metadata_reply_named(format!("P{program}").as_bytes())
    }

    fn complete_sweep() -> (Result<Library, String>, Vec<Vec<u8>>, Duration) {
        let original = named_program(85);
        let mut script = library_start(&original);
        for bank in 0..12 {
            expect_bank_presets(&mut script, bank, named_program);
            expect_bank_request(
                &mut script,
                packets(&bank_reply(format!("Bank {bank} ").as_bytes())),
            );
        }
        script.push((vec![0xc0, 85], vec![]));
        expect_block_pair(&mut script, &original);
        library_end(&mut script);
        run_backup(script, vec![])
    }

    #[test]
    fn library_reads_all_presets_and_names_in_order_then_restores_and_releases() {
        let (result, sent, elapsed) = complete_sweep();
        let library = result.unwrap();
        assert_eq!(library.presets.len(), 60);
        assert_eq!(library.bank_names.len(), 12);
        for (bank, name) in &library.bank_names {
            assert_eq!(name.name, format!("Bank {bank}"));
            assert_eq!(name.reply, bank_reply(format!("Bank {bank} ").as_bytes()));
        }
        assert!(library.stopped.is_none() && library.release_error.is_none());
        assert_eq!(
            library.restore,
            Restore::Switched {
                to: Preset::new(6, 4).unwrap(),
                verified: true
            }
        );
        assert_eq!(library.original.name, "P85");
        assert_eq!(sent.len(), 202);
        // Only the session pause plus 4 ms per scripted input packet.
        assert_eq!(
            elapsed,
            SESSION_SETTLE + Duration::from_millis(4 * (2 + 124 * 13 + 12 * 2))
        );
    }

    #[test]
    #[ignore = "requires the local original Workbench classes and Java backup recorder"]
    fn original_backup_transcript() {
        let path = std::env::var_os("VARIAX_BACKUP_REFERENCE").expect("Java recorder transcript");
        let reference = std::fs::read_to_string(path).unwrap();
        let (result, sent, _) = complete_sweep();
        result.unwrap();
        let sweep_end = 5 + 12 * (5 * 3 + 1);
        // Exclude the app's identity, baseline read, restore/check and
        // session off. Compare the original session on, sweep and bridge pair.
        let actual = sent[1..3]
            .iter()
            .chain(&sent[5..sweep_end])
            .chain(sent.last())
            .map(|message| {
                message
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, reference.lines().collect::<Vec<_>>());
    }

    #[test]
    fn library_without_a_unique_match_does_not_switch_back() {
        let mut script = library_start(&metadata_reply_named(b"Edited"));
        for bank in 0..12 {
            expect_bank_presets(&mut script, bank, named_program);
            expect_bank_request(&mut script, packets(&bank_reply(b"Bank")));
        }
        library_end(&mut script);
        let (result, _, _) = run_backup(script, vec![]);
        assert_eq!(
            result.unwrap().restore,
            Restore::NoUniqueMatch { matches: 0 }
        );
    }

    fn tone_metadata(saved: bool, position: f32) -> PresetMetadata {
        let mut raw = [0; 192];
        raw[..6].copy_from_slice(b"Sample");
        raw[61..64].copy_from_slice(&position.to_be_bytes()[..3]);
        raw[64] = u8::from(saved);
        PresetMetadata::from_bytes(raw).unwrap()
    }

    fn with_preset(slot: u8, metadata: PresetMetadata) -> StoredPreset {
        StoredPreset {
            preset: Preset::new(0, slot).unwrap(),
            replies: [
                encode_reply(metadata.raw[..96].try_into().unwrap()),
                encode_reply(metadata.raw[96..].try_into().unwrap()),
            ],
            metadata,
        }
    }

    #[test]
    fn preset_matching_ignores_only_the_captured_tone_position() {
        for saved in [false, true] {
            let original = tone_metadata(saved, 0.25);
            assert_eq!(
                original.parameters["save_tone_position"],
                ParameterValue::Boolean(saved)
            );
            assert!(same_preset(&original, &tone_metadata(saved, 0.75)));
            assert!(!same_preset(&original, &tone_metadata(!saved, 0.25)));

            for offset in [60, 65] {
                let mut raw = original.raw;
                raw[offset] ^= 1;
                assert!(!same_preset(
                    &original,
                    &PresetMetadata::from_bytes(raw).unwrap()
                ));
            }
        }
    }

    #[test]
    fn restore_selects_the_matching_mode_and_checks_it_again_after_recall() {
        for saved in [false, true] {
            let original = tone_metadata(saved, 0.25);
            let candidates = [
                with_preset(0, tone_metadata(!saved, 0.25)),
                with_preset(1, tone_metadata(saved, 0.5)),
            ];
            let target = candidates[1].preset;
            for recalled_mode in [saved, !saved] {
                let recalled = with_preset(1, tone_metadata(recalled_mode, 0.75));
                let (result, sent, _) = run(
                    vec![
                        (Program(target), vec![]),
                        (Metadata1, packets(&recalled.replies[0])),
                        (Metadata2, packets(&recalled.replies[1])),
                    ],
                    |session| restore(session, &original, &candidates),
                );
                assert_eq!(
                    result,
                    Restore::Switched {
                        to: target,
                        verified: recalled_mode == saved
                    }
                );
                assert_eq!(
                    sent,
                    [
                        Program(target).bytes(),
                        Metadata1.bytes(),
                        Metadata2.bytes()
                    ]
                );
            }
        }
    }

    #[test]
    fn restore_needs_one_mode_match_before_sending_a_program_change() {
        for saved in [false, true] {
            let original = tone_metadata(saved, 0.25);
            let opposite = [with_preset(0, tone_metadata(!saved, 0.25))];
            let ambiguous = [
                with_preset(0, tone_metadata(saved, 0.25)),
                with_preset(1, tone_metadata(saved, 0.75)),
            ];
            for (candidates, matches) in [(&opposite[..], 0), (&ambiguous[..], 2)] {
                let (result, sent, _) =
                    run(vec![], |session| restore(session, &original, candidates));
                assert_eq!(result, Restore::NoUniqueMatch { matches });
                assert!(sent.is_empty());
            }
        }
    }

    #[test]
    fn library_stops_at_a_silent_preset_but_still_restores_and_releases() {
        let original = named_program(4);
        let mut script = library_start(&original);
        script.push((vec![0xc0, 4], vec![]));
        expect_block_pair(&mut script, &original);
        script.extend([(vec![0xc0, 3], vec![]), (BLOCK_1.to_vec(), vec![])]);
        script.push((vec![0xc0, 4], vec![]));
        expect_block_pair(&mut script, &original);
        library_end(&mut script);
        let (result, _, elapsed) = run_backup(script, vec![]);
        let library = result.unwrap();
        assert_eq!(library.presets.len(), 1);
        assert!(library.bank_names.is_empty());
        assert!(library.stopped.unwrap().starts_with("Custom 1 2"));
        assert_eq!(
            library.restore,
            Restore::Switched {
                to: Preset::new(0, 0).unwrap(),
                verified: true
            }
        );
        assert_eq!(
            elapsed,
            Duration::from_millis(2500 + 100 + 4 * (2 + 6 * 13))
        );
    }

    #[test]
    fn bank_name_read_skips_unrelated_and_short_replies() {
        let short = [0xf0, 0, 1, 0x0c, 7, 4, 0x6d, 0xf7];
        let mut script = vec![(BRIDGE_ON.to_vec(), vec![]), (SessionOn.bytes(), vec![])];
        let mut valid = bank_reply(b"Actual name  ");
        valid.insert(25, 0);
        let replies = [packets(&zero_reply()), packets(&short), packets(&valid)].concat();
        expect_bank_request(&mut script, replies);
        library_end(&mut script);
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::VariaxViaPodxt, script),
            &BackupOnly,
            &mut log,
        );
        assert_eq!(read_bank_name(&mut session).unwrap(), "Actual name");
        session.link.assert_finished();
    }

    #[test]
    fn library_stops_at_failed_bank_name_and_reports_release_separately() {
        let short = vec![0xf0, 0, 1, 0x0c, 7, 4, 0x6d, 0xf7];
        for (replies, expected, wait_ms) in [
            (vec![], "Timed out", 1000),
            (vec![short], "Timed out", 1000),
            (packets(&bank_reply(b"Bad\x1f")), "Invalid ASCII", 8),
        ] {
            let original = named_program(4);
            let mut script = library_start(&original);
            expect_bank_presets(&mut script, 0, named_program);
            expect_bank_request(&mut script, replies);
            script.push((vec![0xc0, 4], vec![]));
            expect_block_pair(&mut script, &original);
            library_end(&mut script);
            let failure = script.len() - 1;
            let (result, _, elapsed) = run_backup(script, vec![failure]);
            let library = result.unwrap();
            assert_eq!(library.presets.len(), 5);
            assert!(library.bank_names.is_empty());
            let stopped = library.stopped.unwrap();
            assert!(stopped.starts_with("Custom 1 bank name:") && stopped.contains(expected));
            assert!(library.release_error.unwrap().contains("release failed"));
            assert!(matches!(
                library.restore,
                Restore::Switched { verified: true, .. }
            ));
            assert_eq!(
                elapsed,
                Duration::from_millis(wait_ms + 100 + 4 * (2 + 14 * 13))
            );
        }
    }

    #[test]
    fn library_releases_after_failed_enable_or_current_read() {
        let (result, _, _) = run_backup(
            vec![
                (Identity.bytes(), packets(&VARIAX_600)),
                (BRIDGE_ON.to_vec(), vec![]),
                (BRIDGE_OFF.to_vec(), vec![]),
            ],
            vec![1],
        );
        assert_eq!(result.err().unwrap(), "simulated write failure");

        let mut script = library_start(&named_program(4));
        script.truncate(3);
        script.push((BLOCK_1.to_vec(), vec![]));
        library_end(&mut script);
        let (result, _, elapsed) = run_backup(script, vec![4, 5]);
        let error = result.err().unwrap();
        assert!(error.contains("No 200-byte metadata reply"));
        assert!(error.contains("Session off also failed") && error.contains("release failed"));
        assert_eq!(elapsed, Duration::from_millis(2608));
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
