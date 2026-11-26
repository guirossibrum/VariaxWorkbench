//! #255 regression built entirely from synthetic words.

use super::*;
use std::fs;
use variax::procedures::{FullBuffer, pack, read_buffer};

const COMPACT_WORDS: usize = 873;
const PRIOR_CHECKSUM: u32 = 0xe9_d2a3;
const MISMATCH: &str = "Firmware checksum mismatch (stored E9D2A3, computed B342A3).";

fn images() -> (FullBuffer, FullBuffer) {
    let mut prior = std::array::from_fn(|index| full_word(4, index as u16));
    *prior.last_mut().unwrap() = PRIOR_CHECKSUM;
    // A compact load replaces only the prefix. The previous checksum stays
    // in the retained tail and still describes the previous full buffer.
    let mut mixed = prior;
    for (index, word) in mixed[..COMPACT_WORDS].iter_mut().enumerate() {
        *word = full_word(3, index as u16);
    }
    (prior, mixed)
}

fn exchanges(image: &FullBuffer) -> Script {
    image
        .chunks(32)
        .enumerate()
        .map(|(index, words)| program_exchange((index * 32) as u16, words.iter().copied()))
        .collect()
}

#[test]
fn compact_load_read_preserves_all_reply_bytes_including_the_stale_checksum() {
    struct ReadOnly;
    impl GatePolicy for ReadOnly {
        fn allows(&self, operation: Operation) -> bool {
            operation == Operation::VariaxRead
        }
    }

    let (prior, mixed) = images();
    assert_eq!(mixed.len() - COMPACT_WORDS, 219);
    assert!(
        mixed[..COMPACT_WORDS]
            .iter()
            .zip(&prior[..COMPACT_WORDS])
            .all(|(mixed, prior)| mixed != prior)
    );
    assert_eq!(mixed[COMPACT_WORDS..], prior[COMPACT_WORDS..]);
    let mut script = vec![(BRIDGE_ON.into(), vec![]), (SESSION_ON.into(), vec![])];
    let replies = exchanges(&mixed);
    let expected: Vec<Vec<u8>> = replies
        .iter()
        .map(|(_, packets)| packets.concat())
        .collect();
    script.extend(replies);
    end(&mut script);
    let fake = Fake::from_bytes(DevicePath::VariaxViaPodxt, script);
    let mut quiet = Quiet;
    let mut session = Session::new(fake, &ReadOnly, &mut quiet);
    let mut received = Vec::new();
    let actual = read_buffer(
        &mut session,
        || false,
        |reply| {
            received.push(reply.raw);
            Ok(())
        },
    )
    .unwrap();
    session.link.assert_finished();
    assert_eq!(received.len(), 35);
    assert_eq!(received, expected);
    assert_eq!(actual, mixed);
    let packed = pack(&actual);
    assert_eq!(packed[PROGRAM_BYTES - 3..], [0xe9, 0xd2, 0xa3]);
    check_program_checksum(&pack(&prior)).unwrap();
    assert_eq!(check_program_checksum(&packed).unwrap_err(), MISMATCH);
}

#[test]
fn compact_load_stops_full_backup_and_only_the_verified_program_is_saved() {
    let (prior, mixed) = images();
    let mut script = full_start(4);
    select(&mut script, 4, 4);
    script.extend(exchanges(&prior));
    select(&mut script, 3, 3);
    script.extend(exchanges(&mixed));
    select(&mut script, 4, 4);
    end(&mut script);

    let mut progress = Vec::new();
    let backup = run_full(script, || false, |stored| progress.push(stored.preset));
    let verified = Preset::new(0, 0).unwrap();
    let rejected = Preset::new(0, 1).unwrap();
    assert_eq!(progress, [verified]);
    assert_eq!(
        backup.settings.stopped.as_ref().unwrap(),
        &format!("Custom 1 2 full program: {MISMATCH}")
    );
    assert_eq!(backup.settings.presets.len(), 2);
    assert_eq!(
        backup.settings.restore,
        Restore::Switched {
            to: verified,
            verified: true
        }
    );
    assert!(backup.settings.release_error.is_none());
    assert!(!backup.complete());
    assert_eq!(backup.programs_read(), 1);
    assert_eq!(
        backup.verified_programs().collect::<Vec<_>>(),
        [(verified, &pack(&prior))]
    );

    let root = snapshot::TestRoot::new("compact-load-capture");
    let mut folder = BackupFolder::create(&root.0, "backup").unwrap();
    backup.save(&mut folder).unwrap();
    let path = folder.finish(backup.complete()).unwrap();
    verify_manifest(&path).unwrap();
    assert_eq!(path.file_name().unwrap(), "backup-INCOMPLETE");
    assert_eq!(
        fs::read(path.join(program_file(verified))).unwrap(),
        pack(&prior)
    );
    let rejected_file = program_file(rejected);
    assert!(!path.join(&rejected_file).exists());
    let index = fs::read_to_string(path.join("index.txt")).unwrap();
    assert!(index.contains(&format!("{rejected_file}: MISSING ({MISMATCH})")));
    assert!(
        !fs::read_to_string(path.join("MANIFEST.sha256"))
            .unwrap()
            .contains(&rejected_file)
    );
    let validation = validate_snapshot(
        &path,
        &backup.settings.identified.identity,
        SnapshotCoverage::FullPrograms,
    );
    assert_eq!(
        validation.err().as_deref(),
        Some("The Variax snapshot is incomplete.")
    );
}

#[test]
fn compact_load_is_refused_as_restore_input_even_with_a_matching_manifest() {
    let (prior, mixed) = images();
    let root = snapshot::TestRoot::new("compact-load-validation");
    let coverage = SnapshotCoverage::FullPrograms;
    let target = Preset::new(0, 1).unwrap();
    let target_file = program_file(target);
    let make_snapshot = |label, image: &FullBuffer| {
        let packed = pack(image);
        snapshot::fixture(
            &root,
            label,
            coverage,
            |name, data| {
                Some(if name == target_file {
                    packed.to_vec()
                } else {
                    data
                })
            },
            true,
        )
    };
    let valid = make_snapshot("prior", &prior);
    let invalid = make_snapshot("mixed", &mixed);
    let identity =
        crate::identity::Identity::parse(&fs::read(valid.join("identity.syx")).unwrap()).unwrap();
    let snapshot = validate_snapshot(&valid, &identity, coverage).unwrap();
    let SnapshotPrograms::Full(programs) = snapshot.programs() else {
        panic!("expected validated full programs");
    };
    assert!(
        programs
            .iter()
            .any(|(preset, image)| *preset == target && *image == pack(&prior))
    );

    verify_manifest(&invalid).unwrap();
    let validation = validate_snapshot(&invalid, &identity, coverage);
    assert_eq!(
        validation.err().unwrap(),
        format!("{target_file}: {MISMATCH}")
    );
    assert_eq!(fs::read(invalid.join(target_file)).unwrap(), pack(&mixed));
    verify_manifest(&invalid).unwrap();
}
