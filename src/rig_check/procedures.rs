//! Rig Check's adapter onto the shared device library (`line6-devices`).
//!
//! Each wrapper opens a gated, logged session for one step and calls the
//! library's public procedures, so a pass exercises the same code as the
//! editor. Rig Check keeps its step policy, evidence and comparison rules here.

use super::approvals;
use super::catalogue::StepId;
use super::practice::{PROGRAM_WORDS, Program};
use super::record::Mode;
use super::runner::{Ctx, Progress};
use line6_devices::family::podxt::{self, EditBuffer, Location};
use line6_devices::family::variax::{Preset, procedures as variax};
use line6_devices::gate::{GatePolicy, Operation};
use line6_devices::session::{Link, Log, Session};
use std::cell::RefCell;
use std::time::Duration;

pub(super) const CHECKSUM_WORD: usize = PROGRAM_WORDS - 1;
pub(super) const TONE_POSITION_BYTES: std::ops::Range<usize> = 61..64;

/// Settings bytes 61–63 are the high 24 bits of the tone-position float.
/// Settings start at word 851 and each word holds three big-endian bytes.
pub(super) const fn tone_position_mask(word: usize) -> u32 {
    match word {
        871 => 0x00_ffff,
        872 => 0xff_0000,
        _ => 0,
    }
}

/// The operations one step may use. In real mode the step's owner approval
/// and gates must also be open.
pub struct StepPolicy {
    pub step: StepId,
    pub mode: Mode,
    pub ops: &'static [Operation],
}

impl GatePolicy for StepPolicy {
    fn allows(&self, operation: Operation) -> bool {
        self.ops.contains(&operation)
            && (self.mode == Mode::Practice || approvals::real_device_check(self.step).is_ok())
    }
}

#[derive(Default)]
pub struct Lines(pub Vec<String>);

impl Log for Lines {
    fn line(&mut self, text: String) {
        self.0.push(text);
    }
}

pub type DynSession<'s> = Session<'s, &'s mut dyn Link>;

pub(super) fn absorb(ctx: &mut Ctx<'_>, lines: Lines) {
    let retries = lines
        .0
        .iter()
        .filter(|line| line.contains("Retrying") || line.contains("once more"))
        .count();
    ctx.rec.retries += retries as u32;
    ctx.rec.messages.extend(lines.0);
}

/// Run a shared PODxt Live procedure on a gated session. Rig Check owns
/// the step policy and logs, and the family library owns the device exchange.
pub fn with_podxt<T>(
    ctx: &mut Ctx<'_>,
    ops: &'static [Operation],
    body: impl FnOnce(&mut Session<&mut dyn Link>) -> Result<T, String>,
) -> Result<T, String> {
    with_podxt_stopping(ctx, ops, |session, _| body(session))
}

pub fn with_podxt_stopping<T>(
    ctx: &mut Ctx<'_>,
    ops: &'static [Operation],
    body: impl FnOnce(&mut Session<&mut dyn Link>, &dyn Fn() -> bool) -> Result<T, String>,
) -> Result<T, String> {
    with_podxt_progress(ctx, ops, |session, stopping, _| body(session, stopping))
}

pub fn with_podxt_progress<T>(
    ctx: &mut Ctx<'_>,
    ops: &'static [Operation],
    body: impl FnOnce(&mut DynSession<'_>, &dyn Fn() -> bool, &dyn Fn(Progress)) -> Result<T, String>,
) -> Result<T, String> {
    let policy = StepPolicy {
        step: ctx.step(),
        mode: ctx.mode,
        ops,
    };
    let mut lines = Lines::default();
    let step = ctx.step();
    let op = RefCell::new(&mut *ctx.op);
    let stopping = || op.borrow().stop_requested();
    let progress = |update| op.borrow_mut().progress(update);
    let result = match ctx.port.podxt(step) {
        Ok(link) => {
            let mut session = Session::new(link, &policy, &mut lines);
            body(&mut session, &stopping, &progress)
        }
        Err(error) => Err(error),
    };
    absorb(ctx, lines);
    result
}

/// Read an addressed PODxt tone, keeping the step's lost-reply wording.
pub fn podxt_read_stored<L: Link>(
    session: &mut Session<L>,
    location: Location,
) -> Result<EditBuffer, String> {
    podxt::read_stored_tone(session, location)?.ok_or_else(|| {
        format!(
            "Lost reply: no 168-byte dump for channel {}.",
            location.label()
        )
    })
}

/// Run `body` on a gated session over the guitar interface.
pub fn with_variax<T>(
    ctx: &mut Ctx<'_>,
    ops: &'static [Operation],
    body: impl FnOnce(&mut Session<&mut dyn Link>, &dyn Fn() -> bool) -> Result<T, String>,
) -> Result<T, String> {
    with_variax_progress(ctx, ops, |session, stopping, _| body(session, stopping))
}

pub fn with_variax_progress<T>(
    ctx: &mut Ctx<'_>,
    ops: &'static [Operation],
    body: impl FnOnce(&mut DynSession<'_>, &dyn Fn() -> bool, &dyn Fn(Progress)) -> Result<T, String>,
) -> Result<T, String> {
    let policy = StepPolicy {
        step: ctx.step(),
        mode: ctx.mode,
        ops,
    };
    let mut lines = Lines::default();
    let step = ctx.step();
    let op = RefCell::new(&mut *ctx.op);
    let stopping = || op.borrow().stop_requested();
    let progress = |update| op.borrow_mut().progress(update);
    let result = match ctx.port.variax(step) {
        Ok(link) => {
            let mut session = Session::new(link, &policy, &mut lines);
            session.require_expected_sysex();
            body(&mut session, &stopping, &progress)
        }
        Err(error) => Err(error),
    };
    absorb(ctx, lines);
    result
}

/// Listen briefly (sending nothing) so stale input is logged and does not
/// interrupt the next read.
pub const DRAIN: Duration = Duration::from_millis(300);

pub fn drain<L: Link>(session: &mut Session<L>) -> Result<(), String> {
    session.listen(DRAIN).map(|_| ())
}

/// Report the next chunk as soon as the previous reply was validated.
pub fn program_progress<'a>(progress: &'a dyn Fn(Progress), action: &'a str) -> impl FnMut() + 'a {
    let mut chunks = variax::chunks().into_iter();
    let mut completed = 0;
    let mut advance = move |done| {
        let label = chunks.next().map_or_else(
            || format!("{action}: all replies received"),
            |span| {
                format!(
                    "{action}: reading words {}–{}",
                    span.start(),
                    span.start() + span.count() - 1
                )
            },
        );
        progress(Progress::counted(label, done, variax::CHUNK_COUNT, "reads"));
    };
    advance(0);
    move || {
        completed += 1;
        advance(completed);
    }
}

/// The result of a 35-chunk read.
pub struct FullRead {
    pub words: Program,
    pub raw_replies: Vec<Vec<u8>>,
}

/// Read all 1,092 active words in 35 chunks (#234).
pub fn variax_full_read<L: Link>(
    session: &mut Session<L>,
    stopping: &dyn Fn() -> bool,
    progress: &dyn Fn(Progress),
    action: &str,
) -> Result<FullRead, String> {
    let mut replies = Vec::new();
    let mut update = program_progress(progress, action);
    let words = variax::read_buffer(session, stopping, |reply| {
        update();
        replies.push(reply.raw);
        Ok(())
    })?;
    Ok(FullRead {
        words,
        raw_replies: replies,
    })
}

/// Select a stored preset (program change), then read its 1,092 words.
pub fn variax_read_preset<L: Link>(
    session: &mut Session<L>,
    preset: Preset,
    stopping: &dyn Fn() -> bool,
    progress: &dyn Fn(Progress),
    action: &str,
) -> Result<FullRead, String> {
    let mut replies = Vec::new();
    let mut update = program_progress(progress, action);
    let words = variax::read_unmodified(session, preset, stopping, |reply| {
        update();
        replies.push(reply.raw);
        Ok(())
    })?;
    Ok(FullRead {
        words,
        raw_replies: replies,
    })
}

/// The firmware 3.10 sum over words 0–1,090, reduced to 24 bits.
pub(super) fn firmware_checksum(words: &Program) -> u32 {
    words[..CHECKSUM_WORD]
        .iter()
        .copied()
        .fold(0, u32::wrapping_add)
        & 0xff_ffff
}

/// Check the original checksum before removing the live-position bits and
/// checksum for matching. An error carries the computed checksum.
pub(super) fn neutral_program(words: &Program) -> Result<Program, u32> {
    let checksum = firmware_checksum(words);
    if words[CHECKSUM_WORD] != checksum {
        return Err(checksum);
    }
    let mut neutral = *words;
    for (index, word) in neutral.iter_mut().enumerate() {
        *word &= !tone_position_mask(index);
    }
    neutral[CHECKSUM_WORD] = 0;
    Ok(neutral)
}

/// Same checksum-verified program apart from the live tone position.
pub fn same_program(a: &Program, b: &Program) -> bool {
    match (neutral_program(a), neutral_program(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub fn unpack_program(bytes: &variax::Image) -> Program {
    std::array::from_fn(|index| {
        let [a, b, c] = bytes.as_chunks::<3>().0[index];
        u32::from_be_bytes([0, a, b, c])
    })
}

pub fn program_bytes(words: &Program) -> Vec<u8> {
    variax::pack(words).to_vec()
}

/// Positions of differing words.
pub fn word_differences(a: &Program, b: &Program) -> Vec<usize> {
    (0..PROGRAM_WORDS).filter(|&i| a[i] != b[i]).collect()
}

/// The 192 settings bytes carried by words 851..915.
pub fn settings_of(words: &Program) -> [u8; 192] {
    variax::pack(words)[variax::METADATA]
        .try_into()
        .expect("fixed Variax settings range")
}

pub const PODXT_IDENTITY: &[Operation] = &[Operation::PodxtIdentity];
pub const PODXT_CHANNEL: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtGlobalRead,
];
pub const PODXT_READ: &[Operation] = &[Operation::PodxtIdentity, Operation::PodxtRead];
pub const PODXT_LIBRARY: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtLibraryRead,
];
pub const VARIAX_IDENTITY: &[Operation] = &[Operation::VariaxIdentity];
pub const VARIAX_READ: &[Operation] = &[Operation::VariaxIdentity, Operation::VariaxRead];
pub const VARIAX_SELECT: &[Operation] = &[
    Operation::VariaxIdentity,
    Operation::VariaxRead,
    Operation::VariaxPresetSwitch,
];
pub const GUARD: &[Operation] = &[
    Operation::PodxtIdentity,
    Operation::PodxtRead,
    Operation::PodxtLibraryRead,
    Operation::PodxtChannelSwitch,
];
