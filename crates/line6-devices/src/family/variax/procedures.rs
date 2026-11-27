//! Shared Variax 300/600 firmware 3.10 procedures for an already-open link.
//! The caller owns the rig lock, session policy, and device/driver cleanup.

use super::{
    CurrentPreset, Identified, Preset, PresetMetadata, identify_retrying_silence,
    read_current_checked_retrying_silence,
    span_read::{BUFFER_WORDS, PresetSpan, SpanReply, read_selected_spans, read_spans},
    write::{self, Program},
};
use crate::{
    gate::Operation,
    identity::Identity,
    session::{Link, Session},
};
use std::time::Duration;

mod save_identity;
pub use save_identity::{
    SAVE_PREFLIGHT_NO_IDENTITY, SAVE_PREFLIGHT_REOPEN_EXHAUSTED, SaveIdentity,
    identify_after_save_preflight_reopen, identify_for_save_preflight,
};

const IDENTITY_RETRY_DELAY: Duration = Duration::from_secs(2);
pub const SILENT_STORE_WAIT: Duration = Duration::from_secs(120);
const CHUNK_WORDS: u16 = 32;
pub const CHUNK_COUNT: usize = BUFFER_WORDS.div_ceil(CHUNK_WORDS) as usize;
pub const PROGRAM_BYTES: usize = BUFFER_WORDS as usize * 3;
pub const METADATA: std::ops::Range<usize> = 2553..2745;
pub type Image = [u8; PROGRAM_BYTES];
pub type FullBuffer = [u32; BUFFER_WORDS as usize];

pub fn check_identity(identity: &Identity) -> Result<(), String> {
    if identity.is_line6()
        && (identity.family, identity.member, identity.version) == (7, 4, *b"0310")
    {
        Ok(())
    } else {
        Err(format!(
            "Expected Variax 300/600 MIDI identity and firmware 3.10, got {identity:?}."
        ))
    }
}

/// Verify the guitar on this link. Retry an orphan tail immediately or a
/// wholly silent exchange after two seconds, at most once in either case.
pub fn identify<L: Link>(session: &mut Session<L>) -> Result<Identified, String> {
    let device = identify_reported(session)?;
    check_identity(&device.identity)?;
    Ok(device)
}

/// Read the reported identity with the same retry policy for diagnostic display.
/// Call check_identity before admitting any edit or store.
pub fn identify_reported<L: Link>(session: &mut Session<L>) -> Result<Identified, String> {
    session.require_expected_sysex();
    identify_retrying_silence(session, IDENTITY_RETRY_DELAY)
}

/// Verify identity with the same retry policy, then read the two settings blocks.
pub fn read_settings<L: Link>(session: &mut Session<L>) -> Result<CurrentPreset, String> {
    read_settings_checked(session, check_identity)
}

/// Read diagnostic settings with the caller's existing read-only identity policy.
pub fn read_settings_checked<L: Link>(
    session: &mut Session<L>,
    check: impl FnOnce(&Identity) -> Result<(), String>,
) -> Result<CurrentPreset, String> {
    session.require_expected_sysex();
    read_current_checked_retrying_silence(session, IDENTITY_RETRY_DELAY, check)
}

pub fn chunks() -> [PresetSpan; CHUNK_COUNT] {
    std::array::from_fn(|index| {
        let start = index as u16 * CHUNK_WORDS;
        let count = (BUFFER_WORDS - start).min(CHUNK_WORDS);
        PresetSpan::new(start, count).expect("chunk is in bounds")
    })
}

/// Read all 1,092 words. The callback receives each complete reply before
/// the next request is sent.
pub fn read_buffer<L: Link>(
    session: &mut Session<L>,
    stopping: impl FnMut() -> bool,
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<FullBuffer, String> {
    capture(session, None, stopping, received)
}

/// Reload the saved preset by program change and read its complete buffer
/// in the same bridged session.
pub fn read_unmodified<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    stopping: impl FnMut() -> bool,
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<FullBuffer, String> {
    capture(session, Some(preset), stopping, received)
}

fn capture<L: Link>(
    session: &mut Session<L>,
    selected: Option<Preset>,
    stopping: impl FnMut() -> bool,
    mut received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<FullBuffer, String> {
    let mut full = [0; BUFFER_WORDS as usize];
    let mut collect = |reply: SpanReply| {
        let start = usize::from(reply.span.start());
        full[start..start + reply.words.len()].copy_from_slice(&reply.words);
        received(reply)
    };
    if let Some(preset) = selected {
        read_selected_spans(session, preset, &chunks(), stopping, &mut collect)?;
    } else {
        read_spans(session, &chunks(), stopping, &mut collect)?;
    }
    Ok(full)
}

pub fn pack(words: &FullBuffer) -> Image {
    let mut image = [0; std::mem::size_of::<Image>()];
    for (&word, bytes) in words.iter().zip(image.as_chunks_mut::<3>().0) {
        bytes.copy_from_slice(&word.to_be_bytes()[1..]);
    }
    image
}

/// Compare the complete buffer and report its first differing word.
pub fn compare_image(actual: &FullBuffer, expected: &Image, phase: &str) -> Result<(), String> {
    for (index, (&word, &[a, b, c])) in actual.iter().zip(expected.as_chunks::<3>().0).enumerate() {
        let expected = u32::from_be_bytes([0, a, b, c]);
        if word != expected {
            let start = index * 3;
            if METADATA.contains(&start) {
                let byte = word.to_be_bytes()[1..]
                    .iter()
                    .zip([a, b, c])
                    .position(|(&actual, expected)| actual != expected)
                    .expect("different word");
                let offset = start - METADATA.start + byte;
                let name = PresetMetadata::field_at(offset).unwrap_or("unmapped settings");
                return Err(format!(
                    "{phase} differs at word {index} ({name}, settings byte {offset}): read 0x{word:06X}, expected 0x{expected:06X}."
                ));
            }
            return Err(format!(
                "{phase} differs at word {index}: read 0x{word:06X}, expected 0x{expected:06X}."
            ));
        }
    }
    Ok(())
}

/// Reload and verify even when a preceding operation was cancelled. Use
/// only with a starting snapshot known to match the saved program.
pub fn return_to_preset<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    snapshot: &Image,
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<(), String> {
    let actual = read_unmodified(session, preset, || false, received)?;
    compare_image(&actual, snapshot, "Original preset switch-back")
}

/// Full program, ordinary difference, or Workbench's tuning transition
/// followed by the final full program. Supply a verified baseline for a diff.
pub enum BufferEdit<'a> {
    Full(&'a Program<'a>),
    Difference {
        previous: &'a Image,
        next: &'a Program<'a>,
    },
    AlternateTuning {
        previous: &'a Image,
        transition: &'a Program<'a>,
        full: &'a Program<'a>,
    },
}

/// Send the requested edit and verify all 1,092 words. An ordinary diff
/// retains the old checksum word, which 0x3B omits. Returns the exact
/// verified image so the caller can use it as the next baseline.
/// The callback runs after the send and supplies an observer for each raw reply.
pub fn send_buffer_verified<L: Link, R: FnMut(SpanReply) -> Result<(), String>>(
    session: &mut Session<L>,
    edit: BufferEdit<'_>,
    phase: &str,
    mut stopping: impl FnMut() -> bool,
    after_send: impl FnOnce() -> Result<R, String>,
) -> Result<Image, String> {
    if !session.allows(Operation::VariaxRead) {
        return Err("Variax edit verification requires VariaxRead.".into());
    }
    let expected = send_buffer(session, edit, &mut stopping)?;
    let actual = read_buffer(session, &mut stopping, after_send()?)?;
    compare_image(&actual, &expected, phase)?;
    Ok(expected)
}

/// Send an edit and predict the resulting image. A difference consumes its
/// 0x71, which is a transport acknowledgement. Use read_buffer and
/// compare_image to verify it. This split permits a read after a drag ends.
pub fn send_buffer<L: Link>(
    session: &mut Session<L>,
    edit: BufferEdit<'_>,
    mut stopping: impl FnMut() -> bool,
) -> Result<Image, String> {
    let expected = match edit {
        BufferEdit::Full(program) => {
            write::audition(session, program, &mut stopping)?;
            *program.bytes()
        }
        BufferEdit::Difference { previous, next } => {
            let baseline = Program::difference_baseline(previous);
            write::edit(
                session,
                &[write::Edit::Difference {
                    previous: &baseline,
                    next,
                }],
                &mut stopping,
            )?;
            let mut expected = *next.bytes();
            expected[PROGRAM_BYTES - 3..].copy_from_slice(&previous[PROGRAM_BYTES - 3..]);
            expected
        }
        BufferEdit::AlternateTuning {
            previous,
            transition,
            full,
        } => {
            let baseline = Program::difference_baseline(previous);
            write::edit(
                session,
                &[write::Edit::AlternateTuning {
                    previous: &baseline,
                    transition,
                    full,
                }],
                &mut stopping,
            )?;
            *full.bytes()
        }
    };
    Ok(expected)
}

/// Stamp the version and recompute the checksum as firmware 3.10 does.
pub fn expected_stored_image(program: &[u8; PROGRAM_BYTES]) -> [u8; PROGRAM_BYTES] {
    let mut image = *program;
    for (word, &byte) in image.as_chunks_mut::<3>().0[767..771]
        .iter_mut()
        .zip(b"0310")
    {
        *word = [0, 0, byte];
    }
    set_firmware_checksum(&mut image);
    image
}

pub fn set_firmware_checksum(image: &mut [u8; PROGRAM_BYTES]) {
    let (checksum, data) = image
        .as_chunks_mut::<3>()
        .0
        .split_last_mut()
        .expect("program words");
    let sum = data.iter().fold(0u32, |sum, &[a, b, c]| {
        sum.wrapping_add(u32::from_be_bytes([0, a, b, c]))
    });
    checksum.copy_from_slice(&sum.to_be_bytes()[1..]);
}

pub fn spare_warning(preset: Preset) -> String {
    let group = (preset.bank() * 5 + 4 - preset.slot()) / 20;
    let start = group * 20;
    format!(
        "Do not store any slot in {start}–{} until spare {} is known blank.",
        start + 19,
        60 + group
    )
}

#[derive(Debug)]
pub enum VerifiedStoreError {
    /// No slot command was attempted. The working buffer may have changed.
    BeforeCommit(String),
    /// The slot command was attempted and its outcome is unknown.
    Unknown(String),
    /// No reply, followed by the two-minute wait. Leave the rig untouched.
    SilentTimeout,
    /// The guitar acknowledged the store, but the stored image is unverified.
    Verification(String),
}

/// Send one store transaction and compare the full read-back with the
/// firmware-stamped image. The read-back callback runs only after 0x71,
/// without cancellation. It can call read_unmodified or prompt for manual
/// reselection and then call read_buffer.
/// On silence, notify the caller and wait two minutes after cleanup, with no
/// further sends. The caller must stop on an unknown outcome or silence.
pub fn store_verified<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    phase: &str,
    stopping: impl FnMut() -> bool,
    silent: impl FnOnce(&str),
    readback: impl FnOnce(&mut Session<L>) -> Result<FullBuffer, String>,
) -> Result<(), VerifiedStoreError> {
    store_verified_observing(
        session,
        preset,
        program,
        phase,
        stopping,
        only_silent(silent),
        readback,
    )
}

fn only_silent(silent: impl FnOnce(&str)) -> impl FnMut(StoreWait) {
    let mut silent = Some(silent);
    move |wait| {
        if let StoreWait::Silent { notice } = wait
            && let Some(silent) = silent.take()
        {
            silent(notice);
        }
    }
}

fn store_verified_observing<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    phase: &str,
    stopping: impl FnMut() -> bool,
    mut waiting: impl FnMut(StoreWait<'_>),
    readback: impl FnOnce(&mut Session<L>) -> Result<FullBuffer, String>,
) -> Result<(), VerifiedStoreError> {
    if !session.allows(Operation::VariaxRead) {
        return Err(VerifiedStoreError::BeforeCommit(
            "Variax store verification requires VariaxRead.".into(),
        ));
    }
    let notice = format!(
        "No 0x71. Keep the rig powered and untouched for 2 minutes. {}",
        spare_warning(preset)
    );
    store_once_observing(session, preset, program, stopping, &notice, &mut waiting)?;
    waiting(StoreWait::ReadBack);
    let expected = expected_stored_image(program.bytes());
    let actual = readback(session).map_err(VerifiedStoreError::Verification)?;
    compare_image(&actual, &expected, phase).map_err(VerifiedStoreError::Verification)
}

/// Run the store and its silent-timeout wait. A caller with its own comparison
/// rules must verify the reloaded target after acknowledgement.
/// On silence the observer can record instructions before the wait begins.
pub fn store_once<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    stopping: impl FnMut() -> bool,
    timeout_notice: &str,
    on_silent: impl FnOnce(),
) -> Result<(), VerifiedStoreError> {
    store_once_observing(
        session,
        preset,
        program,
        stopping,
        timeout_notice,
        only_silent(|_| on_silent()),
    )
}

/// Store confirmation, the silent hold, or the subsequent read-back.
#[derive(Clone, Copy, Debug)]
pub enum StoreWait<'a> {
    Confirmation,
    Silent { notice: &'a str },
    ReadBack,
}

impl StoreWait<'_> {
    pub fn limit(self) -> Option<Duration> {
        match self {
            Self::Confirmation => Some(write::STORE_ACK),
            Self::Silent { .. } => Some(SILENT_STORE_WAIT),
            Self::ReadBack => None,
        }
    }
}

/// Run the shared store and report the start of each bounded wait.
pub fn store_once_observing<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    stopping: impl FnMut() -> bool,
    timeout_notice: &str,
    mut waiting: impl FnMut(StoreWait<'_>),
) -> Result<(), VerifiedStoreError> {
    let mut committing = false;
    match write::store_observing_commit(session, preset, program, stopping, || {
        committing = true;
        waiting(StoreWait::Confirmation);
    }) {
        Ok(()) => Ok(()),
        Err(write::StoreError::NoConfirmation) => {
            session.note(timeout_notice);
            waiting(StoreWait::Silent {
                notice: timeout_notice,
            });
            session.link.pause(SILENT_STORE_WAIT);
            Err(VerifiedStoreError::SilentTimeout)
        }
        Err(error) if committing => Err(VerifiedStoreError::Unknown(error.to_string())),
        Err(error) => Err(VerifiedStoreError::BeforeCommit(error.to_string())),
    }
}

impl std::fmt::Display for VerifiedStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeCommit(error) | Self::Unknown(error) => f.write_str(error),
            Self::Verification(error) => write!(f, "Store acknowledged; read-back failed: {error}"),
            Self::SilentTimeout => f.write_str("No store reply. Slot state is unknown. Check whether the guitar responds, take a Rig Check backup if it does, and report the result."),
        }
    }
}

impl std::error::Error for VerifiedStoreError {}

/// Editor/Rig Check store path: send once, reload the written slot after
/// 0x71 and verify its full image. Returning to the caller's starting preset
/// is separate; do not send a return command on an unknown outcome or silence.
pub fn store_and_reload<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    stopping: impl FnMut() -> bool,
    silent: impl FnOnce(&str),
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<(), VerifiedStoreError> {
    store_and_reload_observing(
        session,
        preset,
        program,
        stopping,
        only_silent(silent),
        received,
    )
}

/// Verified store with notifications when either bounded wait starts.
pub fn store_and_reload_observing<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    program: &Program<'_>,
    stopping: impl FnMut() -> bool,
    waiting: impl FnMut(StoreWait<'_>),
    received: impl FnMut(SpanReply) -> Result<(), String>,
) -> Result<(), VerifiedStoreError> {
    if !session.allows(Operation::VariaxPresetSwitch) {
        return Err(VerifiedStoreError::BeforeCommit(
            "Variax store verification requires VariaxPresetSwitch.".into(),
        ));
    }
    store_verified_observing(
        session,
        preset,
        program,
        "Stored target read-back",
        stopping,
        waiting,
        |session| read_unmodified(session, preset, || false, received),
    )
}
