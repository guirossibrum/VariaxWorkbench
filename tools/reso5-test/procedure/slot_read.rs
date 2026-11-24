use super::{METADATA, NAME, PreparedStore, prepare_store};
use line6_devices::{
    backup,
    family::variax::{
        self,
        slot_read::{SlotIndex, SlotReply, read_slots},
    },
    gate::{GatePolicy, Operation},
    identity::Identity,
    session::{Link, Log, Session},
};
use std::path::Path;
use variax_workbench::{
    document::save_new,
    factory_reference::PROGRAM_BYTES,
    hardware_gates::{Gate, is_locked},
};

const FACTORY: SlotIndex = SlotIndex::new(0x32).unwrap();
const SPARE_CANDIDATE: SlotIndex = SlotIndex::new(0x3e).unwrap();
const INDICES: [SlotIndex; 3] = [SlotIndex::CURRENT, FACTORY, SPARE_CANDIDATE];

struct SlotReadPolicy;

impl GatePolicy for SlotReadPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => !is_locked(Gate::IdentityRequest),
            Operation::VariaxSlotRead => !is_locked(Gate::FullBackupRead),
            _ => false,
        }
    }
}

pub struct PreparedSlotRead(pub(super) PreparedStore);

pub fn prepare(
    variax_backup: &Path,
    podxt_backup: &Path,
    today: &str,
    archive: &Path,
) -> Result<PreparedSlotRead, String> {
    check_podxt_backup(podxt_backup, today)?;
    prepare_store(variax_backup, today, archive).map(PreparedSlotRead)
}

fn check_podxt_backup(dir: &Path, today: &str) -> Result<(), String> {
    if dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("-INCOMPLETE") || name.ends_with(".partial"))
    {
        return Err("Select a completed PODxt Live backup folder.".into());
    }
    let manifest = super::read(&dir.join("MANIFEST.sha256"))?;
    let manifest = std::str::from_utf8(&manifest).map_err(|error| error.to_string())?;
    for required in ["identity.syx", "log.txt"] {
        if !manifest.lines().any(|line| {
            line.split_once("  ")
                .is_some_and(|(_, name)| name == required)
        }) {
            return Err(format!("PODxt Live backup manifest omits {required}."));
        }
    }
    backup::verify_manifest(dir)?;
    let log = super::read(&dir.join("log.txt"))?;
    let log = std::str::from_utf8(&log).map_err(|error| error.to_string())?;
    if !log.lines().next().is_some_and(|line| {
        line.strip_prefix("PODxt Live library backup at ")
            .and_then(|stamp| stamp.split_once(' '))
            .is_some_and(|(date, _)| date == today)
    }) {
        return Err("Select a Device check PODxt Live library backup made today (UTC).".into());
    }
    let identity = Identity::parse(&super::read(&dir.join("identity.syx"))?)
        .ok_or("The PODxt Live backup identity is invalid.")?;
    super::podxt_identity(&identity)
}

/// Verify the PODxt over its MIDI link before opening the Variax interface.
pub fn after_podxt_identity<L: Link, T>(
    podxt: L,
    log: &mut dyn Log,
    on_verified: impl FnOnce(&mut dyn Log) -> Result<T, String>,
) -> Result<T, String> {
    super::check_podxt(podxt, log)?;
    on_verified(log)
}

/// Verify the Variax identity, then capture through the G5 session. Each
/// completed program survives a later failure, and comparisons run after cleanup.
pub fn capture<L: Link>(
    link: L,
    log: &mut dyn Log,
    prepared: &PreparedSlotRead,
    dir: &Path,
    mut stopping: impl FnMut() -> bool,
) -> Result<[SlotReply; 3], String> {
    if stopping() {
        return Err("Cancelled or log unavailable before Variax identity.".into());
    }
    let mut session = Session::new(link, &SlotReadPolicy, log);
    session.require_expected_sysex();
    let identified = variax::identify(&mut session)?;
    super::variax_identity(&identified.identity)?;
    session.note("Verified Variax 300/600 firmware 3.10.");

    let mut replies = Vec::with_capacity(INDICES.len());
    read_slots(&mut session, &INDICES, stopping, |reply| {
        let index = reply.index;
        save_new(
            &dir.join(format!("index-{:02X}.program", index.value())),
            &reply.program,
        )?;
        // Check the active metadata before asking for either stored index.
        if index == SlotIndex::CURRENT && reply.program[METADATA] != prepared.0.settings {
            return Err(format!(
                "The 0x7F program metadata differs from the approved {NAME} baseline."
            ));
        }
        replies.push(reply);
        Ok(())
    })?;
    replies
        .try_into()
        .map_err(|_| "Incomplete slot read capture.".into())
}

fn comparison(program: &[u8; PROGRAM_BYTES], reference: &[u8; PROGRAM_BYTES]) -> String {
    let difference = program
        .as_chunks::<3>()
        .0
        .iter()
        .zip(reference.as_chunks::<3>().0)
        .enumerate()
        .find(|(_, (actual, expected))| actual != expected);
    let Some((index, (actual, expected))) = difference else {
        return "equal".into();
    };
    let word = |&[a, b, c]: &[u8; 3]| u32::from_be_bytes([0, a, b, c]);
    format!(
        "first difference at word {index} (zero-based), read 0x{:06X}, reference 0x{:06X}",
        word(actual),
        word(expected)
    )
}

pub fn summarize(
    prepared: &PreparedSlotRead,
    [current, factory, spare]: &[SlotReply; 3],
) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Index 0x32 vs factory Reso-5 ({}): {}.",
            super::STORE_HASH,
            comparison(&factory.program, &prepared.0.program)
        ),
        format!(
            "Index 0x3E blank across the returned program bytes (all FF): {}.",
            if spare.program.iter().all(|&byte| byte == 0xff) {
                "yes"
            } else {
                "no"
            }
        ),
        format!(
            "RAM buffer (0x7F) vs index 0x32: {}.",
            comparison(&current.program, &factory.program)
        ),
    ];
    lines.extend([current, factory, spare].map(|reply| {
        format!(
            "Index 0x{:02X} checksum: {} (stored 0x{:06X}, computed 0x{:06X}).",
            reply.index.value(),
            if reply.checksum.matches() {
                "matches"
            } else {
                "mismatch"
            },
            reply.checksum.stored,
            reply.checksum.computed,
        )
    }));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use line6_devices::family::podxt;

    #[test]
    fn offline_podxt_preflight_requires_a_current_verified_301_capture() {
        let root = tempfile::tempdir().unwrap();
        for (name, version, complete, passes) in [
            ("right", [0, 3, 0, 1], true, true),
            ("wrong-version", [0, 3, 0, 0], true, false),
            ("incomplete", [0, 3, 0, 1], false, false),
        ] {
            let mut folder = backup::BackupFolder::create(root.path(), name).unwrap();
            folder.record("PODxt Live library backup at 2026-11-24 12:00:00 UTC.".into());
            let mut identity = vec![
                0xf0,
                0x7e,
                0x7f,
                6,
                2,
                0,
                1,
                0x0c,
                3,
                0,
                podxt::PODXT_LIVE_MEMBER as u8,
                0,
            ];
            identity.extend(version);
            identity.push(0xf7);
            folder.file("identity.syx", &identity).unwrap();
            let path = folder.finish(complete).unwrap();
            assert_eq!(check_podxt_backup(&path, "2026-11-24").is_ok(), passes);
            assert!(check_podxt_backup(&path, "2026-11-25").is_err());
            let unused = root.path().join("unused");
            let Err(error) = prepare(&unused, &path, "2026-11-25", &unused) else {
                panic!("preflight accepted a stale backup");
            };
            assert!(error.contains("PODxt Live"));
        }
    }
}
