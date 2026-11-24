use super::{STORE_HASH, canonical_store_program, read, variax_identity};
use line6_devices::{
    family::variax::{
        self,
        span_read::{BUFFER_WORDS, PresetSpan, read_spans},
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

const COUNTS: [u16; 5] = [64, 128, 256, 512, BUFFER_WORDS];
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
    let identified = variax::identify(&mut session)?;
    variax_identity(&identified.identity)?;
    session.note("Verified Variax 300/600 firmware 3.10.");

    let spans = COUNTS.map(|count| PresetSpan::new(0, count).expect("step count is in bounds"));
    let mut full = Vec::new();
    read_spans(&mut session, &spans, stopping, |reply| {
        let count = reply.span.count();
        save_new(&dir.join(format!("start-0-count-{count}.syx")), &reply.raw)?;
        if count == BUFFER_WORDS {
            full = reply.words;
        }
        Ok(())
    })?;
    full.try_into()
        .map_err(|_| "Incomplete full buffer.".into())
}

/// Compare every word while the firmware work identifies which words can
/// change in RAM. A differing live word alone cannot identify a flash change.
pub fn summarize(factory: &[u8; PROGRAM_BYTES], captured: &FullBuffer) -> Vec<String> {
    let mut differences = Vec::new();
    for (index, (&[a, b, c], &active)) in
        factory.as_chunks::<3>().0.iter().zip(captured).enumerate()
    {
        let expected = u32::from_be_bytes([0, a, b, c]);
        if active != expected {
            differences.push(format!(
                "Word {index}: active 0x{active:06X}, factory 0x{expected:06X}"
            ));
        }
    }
    let summary = format!(
        "Compared {BUFFER_WORDS} active words against factory Reso-5. {} differing words (no live-word exclusions).",
        differences.len()
    );
    differences.push(summary);
    differences
}
