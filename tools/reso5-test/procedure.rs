pub mod full_read;
pub mod selection;
pub mod slot_read;
pub mod store;

use line6_devices::{
    backup,
    family::{podxt, variax, variax::write},
    gate::{GatePolicy, Operation},
    identity::Identity,
    session::{Link, Log, Session, Wait},
};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, time::Duration};
use variax_workbench::{
    PresetMetadata,
    factory_reference::{FactoryReference, PROGRAM_BYTES},
    firmware::{VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION},
    hardware_gates::{Gate, is_locked},
    host::{HostEngine, HostParameters, ModelLibrary},
    preset::PresetFile,
};

// Give the reattached kernel driver time to finish its identity traffic.
const IDENTITY_RETRY_DELAY: Duration = Duration::from_secs(2);
const NAME: &str = "Reso-5";
const SETTINGS_HASH: &str = "13f8a99828851153eebff71b2fa644bf390cf144f9da6b73b9ee6c66f502aaf7";
const OWN_HASH: &str = "7a245957b3263a9dc225633cdabdd2e94ce65d1ad3a6c0eb2cab23ef4e01e6a1";
const STORE_HASH: &str = "f8694ec3353c4d112bef4b171c7c91110f5a23f53082a1560addfd7e7b2983c5";
/// Expected firmware-stamped image for a future direct-interface readback.
const STORED_HASH: &str = "90d79131f7909639aa2d0793cb3f617fbd95a95a1ada7a54f90ec84de4e28164";
const TRIM_HASH: &str = "ad38b28c0390afced301367ea1cc9b2c0a5242cd1a4fb9a3250843c31f82175d";
const METADATA: std::ops::Range<usize> = 2553..2745;
const SYSTEM_SLOT: u32 = 0x02010000;
const TRIM_PARAM: u32 = 0x200000;
const TRIM_DB: f32 = -6.0;

#[derive(Clone, Copy)]
pub enum Stage {
    Own,
    Trim,
}

impl Stage {
    pub fn number(self) -> u8 {
        match self {
            Self::Own => 1,
            Self::Trim => 2,
        }
    }

    fn hash(self) -> &'static str {
        match self {
            Self::Own => OWN_HASH,
            Self::Trim => TRIM_HASH,
        }
    }
}

pub struct VariaxPolicy;

impl GatePolicy for VariaxPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => !is_locked(Gate::IdentityRequest),
            Operation::VariaxRead => !is_locked(Gate::CurrentSettingsRead),
            Operation::VariaxEditBufferWrite => !is_locked(Gate::EditBufferWrite),
            _ => false,
        }
    }
}

pub struct StorePolicy;

impl GatePolicy for StorePolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => !is_locked(Gate::IdentityRequest),
            Operation::VariaxRead => !is_locked(Gate::CurrentSettingsRead),
            Operation::VariaxStoreRestore => !is_locked(Gate::StoreRestore),
            _ => false,
        }
    }
}

struct PodxtIdentityOnly;

impl GatePolicy for PodxtIdentityOnly {
    fn allows(&self, operation: Operation) -> bool {
        operation == Operation::PodxtIdentity && !is_locked(Gate::IdentityRequest)
    }
}

fn variax_identity(identity: &Identity) -> Result<(), String> {
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

pub fn check_podxt<L: Link>(link: L, log: &mut dyn Log) -> Result<(), String> {
    let mut session = Session::new(link, &PodxtIdentityOnly, log);
    session.require_expected_sysex();
    session.send(podxt::PodxtRequest::Identity)?;
    let Wait::Found(reply) = session.wait_for(
        Duration::from_secs(2),
        |m| Identity::parse(m).is_some(),
        false,
    )?
    else {
        return Err("No PODxt Live identity reply.".into());
    };
    let identity = Identity::parse(&reply).expect("matched identity reply");
    podxt_identity(&identity)?;
    session.note("Verified PODxt Live firmware 3.01.");
    Ok(())
}

fn podxt_identity(identity: &Identity) -> Result<(), String> {
    if !identity.is_line6()
        || (identity.family, identity.member, identity.version)
            != (
                podxt::IDENTITY_FAMILY,
                podxt::PODXT_LIVE_MEMBER,
                [0, 3, 0, 1],
            )
    {
        return Err(format!(
            "Expected PODxt Live firmware 3.01, got {identity:?}."
        ));
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn location() -> variax::Preset {
    variax::Preset::new(10, 4).expect("Reso position 5")
}

fn compare_settings(actual: &[u8], expected: &[u8], field: &str) -> Result<(), String> {
    if actual.len() != expected.len() {
        return Err(format!(
            "{field} length differs: found {} bytes, expected {}.",
            actual.len(),
            expected.len()
        ));
    }
    for (offset, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        if actual != expected {
            let name = variax::PresetMetadata::field_at(offset).unwrap_or("unmapped settings");
            return Err(format!(
                "{field} differs in {name} at settings byte {offset}: found 0x{actual:02X}, expected 0x{expected:02X}."
            ));
        }
    }
    Ok(())
}

fn check_backup(dir: &Path, expected: &[u8; 192]) -> Result<[u8; 192], String> {
    if dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("-INCOMPLETE") || name.ends_with(".partial"))
    {
        return Err("Select a completed Variax backup folder.".into());
    }
    let manifest = read(&dir.join("MANIFEST.sha256"))?;
    let names: std::collections::BTreeSet<_> = std::str::from_utf8(&manifest)
        .map_err(|e| e.to_string())?
        .lines()
        .filter_map(|line| line.split_once("  ").map(|(_, name)| name))
        .collect();
    for name in ["log.txt", "identity.syx"] {
        if !names.contains(name) {
            return Err(format!("Backup manifest omits {name}."));
        }
    }
    for preset in variax::Preset::all() {
        let stem = backup::variax_preset_stem(preset);
        for block in [1, 2] {
            if !names.contains(format!("{stem}-{block}.syx").as_str()) {
                return Err(format!("Backup manifest omits {stem}-{block}.syx."));
            }
        }
    }
    let library = backup::load_variax_library(dir)?;
    let log = String::from_utf8(read(&dir.join("log.txt"))?).map_err(|e| e.to_string())?;
    if !log
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("Variax library backup at "))
    {
        return Err("Select a Device check Variax library backup.".into());
    }
    let identity = Identity::parse(&read(&dir.join("identity.syx"))?)
        .ok_or("The Variax backup identity is invalid.")?;
    variax_identity(&identity)?;
    if library.presets.len() != 60 {
        return Err("The Variax backup must contain all 60 presets.".into());
    }
    let (_, metadata) = library
        .presets
        .iter()
        .find(|(preset, _)| *preset == location())
        .ok_or("The Variax backup has no Reso position 5.")?;
    compare_settings(&metadata.raw, expected, "The backup's slot 50 settings")?;
    Ok(metadata.raw)
}

pub struct Prepared {
    start: [u8; 192],
    end: [u8; 192],
    program: [u8; PROGRAM_BYTES],
}

pub fn prepare(
    stage: Stage,
    backup: &Path,
    archive: &Path,
    models: &Path,
) -> Result<Prepared, String> {
    if variax_workbench::math::PROFILE != "host-std-v1" {
        return Err("This probe requires host-std-v1. Build without portable-math.".into());
    }
    let factory = FactoryReference::parse(&read(archive)?)?;
    let slot = &factory.slots()[50];
    let start = slot.metadata.raw;
    let mut preset = slot.settings_file()?;
    if preset.name() != NAME || !matches_hash(preset.as_bytes(), SETTINGS_HASH) {
        return Err("The archived Reso-5 settings do not match the approved fingerprint.".into());
    }
    let start = check_backup(backup, &start)?;
    if let Stage::Trim = stage {
        preset.set_parameter(SYSTEM_SLOT, TRIM_PARAM, 1, TRIM_DB.to_bits())?;
    }
    let parameters = HostParameters::from_preset(&preset)?;
    let settings = parameters.map(Default::default())?;
    let models = ModelLibrary::load_manifest(models)?;
    let generated = HostEngine::new(&models).generate(&settings)?;
    let program = parameters.finish(NAME, &settings, generated.packed());
    let firmware: Vec<_> = slot
        .raw_program()
        .as_chunks::<3>()
        .0
        .iter()
        .flat_map(|&[a, b, c]| [c, b, a])
        .collect();
    check_program(&program, stage.hash(), &firmware)?;
    let end = program[METADATA].try_into().expect("fixed metadata range");
    check_rendered_settings(&preset, end)?;
    if matches!(stage, Stage::Own) && end != start {
        return Err("Stage 1 does not retain the full original metadata block.".into());
    }
    Ok(Prepared {
        start,
        end,
        program,
    })
}

pub struct PreparedStore {
    settings: [u8; 192],
    program: [u8; PROGRAM_BYTES],
}

pub fn prepare_store(backup: &Path, archive: &Path) -> Result<PreparedStore, String> {
    let factory = FactoryReference::parse(&read(archive)?)?;
    let slot = &factory.slots()[50];
    let program = canonical_store_program(slot.raw_program(), STORE_HASH)?;
    if !matches_hash(&expected_stored_image(&program), STORED_HASH) {
        return Err("The expected stamped slot 50 image has an incorrect SHA-256.".into());
    }
    let settings = program[METADATA].try_into().expect("fixed metadata range");
    let settings = check_backup(backup, &settings)?;
    Ok(PreparedStore { settings, program })
}

fn canonical_store_program(raw: &[u8], hash: &str) -> Result<[u8; PROGRAM_BYTES], String> {
    let mut program: [u8; PROGRAM_BYTES] = raw
        .try_into()
        .map_err(|_| "The archived slot program must contain 3,276 bytes.")?;
    for word in program.as_chunks_mut::<3>().0 {
        word.reverse();
    }
    if !matches_hash(&program, hash) {
        return Err("The archived slot 50 program differs from the approved SHA-256.".into());
    }
    write::Program::new(NAME, &program)?;
    Ok(program)
}

fn expected_stored_image(program: &[u8; PROGRAM_BYTES]) -> [u8; PROGRAM_BYTES] {
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

fn set_firmware_checksum(image: &mut [u8; PROGRAM_BYTES]) {
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

fn check_rendered_settings(preset: &PresetFile, raw: [u8; 192]) -> Result<(), String> {
    let decoded = PresetFile::from_metadata(
        &PresetMetadata::from_bytes(raw)?,
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
    )?;
    if decoded.as_bytes() != preset.as_bytes() {
        return Err("Rendered metadata differs from the stage's approved settings.".into());
    }
    Ok(())
}

fn matches_hash(bytes: &[u8], expected: &str) -> bool {
    format!("{:x}", Sha256::digest(bytes)) == expected
}

fn check_program(
    candidate: &[u8; PROGRAM_BYTES],
    hash: &str,
    firmware: &[u8],
) -> Result<(), String> {
    if !matches_hash(candidate, hash) {
        return Err(
            "The generated program differs from the original Mac Workbench reference.".into(),
        );
    }
    write::Program::new(NAME, candidate)?;
    const UNITY: i32 = 1 << 23;
    const MIN_MARGIN: i32 = 838_861;
    let word = |bytes: &[u8], index: usize| {
        let &[a, b, c] = &bytes.as_chunks::<3>().0[index];
        i32::from_be_bytes([a, b, c, 0]) >> 8
    };
    for index in (632..=640).chain(643..=678) {
        let rendered = word(candidate, index);
        let archived = word(firmware, index);
        if (rendered - archived).abs() > 2 || UNITY - rendered.abs() < MIN_MARGIN {
            return Err(format!(
                "The provisional reverb bound failed at word {index}."
            ));
        }
    }
    Ok(())
}

pub fn attempt<L: Link>(
    session: &mut Session<L>,
    prepared: &Prepared,
    mut cancelled: impl FnMut() -> bool,
) -> Result<(), String> {
    session.require_expected_sysex();
    if cancelled() {
        return Err("Cancelled before reading the guitar.".into());
    }
    let current = variax::read_current_checked_retrying_silence(
        session,
        IDENTITY_RETRY_DELAY,
        variax_identity,
    )?;
    compare_settings(
        &current.metadata.raw,
        &prepared.start,
        "The active Variax settings compared with the backup's slot 50 settings",
    )?;
    if cancelled() {
        return Err("Cancelled before opening the edit session.".into());
    }

    let program = write::Program::new(NAME, &prepared.program)?;
    session.note("Baseline verified. Sending the one approved edit-buffer program.");
    write::edit(session, &[write::Edit::Full(&program)], &mut cancelled)?;
    if cancelled() {
        return Err(
            "Cancelled after the edit. Reselect the stored preset per the test card.".into(),
        );
    }
    let readback = variax::read_current_checked_retrying_silence(
        session,
        IDENTITY_RETRY_DELAY,
        variax_identity,
    )?;
    if readback.metadata.name != NAME || readback.metadata.raw != prepared.end {
        return Err(
            "Settings read-back differs from the expected edit. Follow the stop rules.".into(),
        );
    }
    if cancelled() {
        return Err(
            "Cancelled during read-back. Reselect the stored preset per the test card.".into(),
        );
    }
    let Some(variax::ParameterValue::Float(trim)) = readback.metadata.parameters.get("trim") else {
        return Err("The verified metadata does not contain an output trim.".into());
    };
    session.note(format!(
        "All 192 metadata bytes match. Output trim {trim:.1} dB."
    ));
    Ok(())
}

#[cfg(test)]
mod tests;
