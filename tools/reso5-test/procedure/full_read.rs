use super::{STORE_HASH, canonical_store_program, read, variax_identity};
use line6_devices::{
    family::variax::{
        self,
        span_read::{BUFFER_WORDS, PresetSpan, read_selected_spans, read_spans},
    },
    gate::{GatePolicy, Operation},
    session::{Link, Log, Session},
};
use std::path::Path;
use variax_workbench::{
    document::save_new,
    factory_reference::{FactoryReference, PROGRAM_BYTES},
    hardware_gates::{Gate, is_locked},
};

const CHUNK_WORDS: u16 = 32;
pub const CHUNK_COUNT: usize = BUFFER_WORDS.div_ceil(CHUNK_WORDS) as usize;
pub type FullBuffer = [u32; BUFFER_WORDS as usize];

struct FullReadPolicy;

impl GatePolicy for FullReadPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => !is_locked(Gate::IdentityRequest),
            Operation::VariaxRead => !is_locked(Gate::CurrentSettingsRead),
            _ => false,
        }
    }
}

pub fn prepare(archive: &Path) -> Result<[u8; PROGRAM_BYTES], String> {
    let factory = FactoryReference::parse(&read(archive)?)?;
    canonical_store_program(factory.slots()[50].raw_program(), STORE_HASH)
}

/// Save validated request replies as received. Earlier replies survive a later
/// failure. The enclosing traced link logs partial or malformed input as well.
pub fn capture<L: Link>(
    link: L,
    log: &mut dyn Log,
    dir: &Path,
    mut stopping: impl FnMut() -> bool,
) -> Result<FullBuffer, String> {
    if stopping() {
        return Err("Cancelled or log unavailable before Variax identity.".into());
    }
    let mut session = Session::new(link, &FullReadPolicy, log);
    session.require_expected_sysex();
    let identified = variax::identify_retrying_silence(&mut session, super::IDENTITY_RETRY_DELAY)?;
    variax_identity(&identified.identity)?;
    session.note("Verified Variax 300/600 firmware 3.10.");

    read_chunks(&mut session, dir, stopping)
}

pub(super) fn chunks() -> [PresetSpan; CHUNK_COUNT] {
    std::array::from_fn(|index| {
        let start = index as u16 * CHUNK_WORDS;
        let count = (BUFFER_WORDS - start).min(CHUNK_WORDS);
        PresetSpan::new(start, count).expect("chunk is in bounds")
    })
}

pub(super) fn chunk_path(dir: &Path, span: PresetSpan) -> std::path::PathBuf {
    dir.join(format!("start-{}-count-{}.syx", span.start(), span.count()))
}

pub(super) fn read_chunks<L: Link>(
    session: &mut Session<L>,
    dir: &Path,
    stopping: impl FnMut() -> bool,
) -> Result<FullBuffer, String> {
    read_chunks_selecting(session, dir, None, stopping)
}

pub(super) fn read_chunks_selecting<L: Link>(
    session: &mut Session<L>,
    dir: &Path,
    selected: Option<line6_devices::family::variax::Preset>,
    stopping: impl FnMut() -> bool,
) -> Result<FullBuffer, String> {
    let mut full = Vec::with_capacity(usize::from(BUFFER_WORDS));
    let save = |reply: line6_devices::family::variax::span_read::SpanReply| {
        save_new(&chunk_path(dir, reply.span), &reply.raw)?;
        full.extend(reply.words);
        Ok(())
    };
    if let Some(preset) = selected {
        read_selected_spans(session, preset, &chunks(), stopping, save)?;
    } else {
        read_spans(session, &chunks(), stopping, save)?;
    }
    full.try_into()
        .map_err(|_| "Incomplete full buffer.".into())
}

/// Compare all words, reporting the two words with unresolved tone-position
/// effects separately from other findings (#181 and #234).
pub fn summarize(factory: &[u8; PROGRAM_BYTES], captured: &FullBuffer) -> Vec<String> {
    let mut lines = Vec::new();
    let mut findings = 0;
    let mut unknown = 0;
    for (index, (&[a, b, c], &active)) in
        factory.as_chunks::<3>().0.iter().zip(captured).enumerate()
    {
        let expected = u32::from_be_bytes([0, a, b, c]);
        if active != expected {
            let status = if matches!(index, 871 | 872) {
                unknown += 1;
                "unknown, not a failure"
            } else {
                findings += 1;
                "finding"
            };
            lines.push(format!(
                "Word {index}: active 0x{active:06X}, factory 0x{expected:06X} ({status})"
            ));
        }
    }
    lines.push(format!(
        "Compared {BUFFER_WORDS} active words against factory Reso-5. Findings: {findings}. Unknown differences: {unknown}."
    ));
    lines
}
