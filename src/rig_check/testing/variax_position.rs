use super::Script;
use crate::rig_check::{
    catalogue::{RIG_GUARD, StepId},
    differences::{self, Severity},
    port::PracticePort,
    practice::{Faults, Program, SimRig, flash_index, synthetic_program},
    procedures::{CHECKSUM_WORD, same_program, unpack_program},
    record::{Outcome, StepRecord},
    recover::{RigState, match_hash, offer_restore, write_state},
    report::{json_block, render_plain},
    run_on,
    runner::NoReport,
    steps::RunData,
    variax_reads::other_preset,
};
use line6_devices::family::variax::procedures::{METADATA, pack, set_firmware_checksum};

// A valid position with a changed byte in both program words.
const MOVED: f32 = f32::from_bits(0x3f40_0100);

/// Write independent fixture fields in the exact packed format from #181.
fn with_settings(source: &Program, position: f32, saved_mode: u8, trim: f32) -> Program {
    let mut bytes = pack(source);
    let settings = &mut bytes[METADATA];
    settings[61..64].copy_from_slice(&position.to_be_bytes()[..3]);
    settings[64] = saved_mode;
    settings[65..68].copy_from_slice(&trim.to_be_bytes()[..3]);
    set_firmware_checksum(&mut bytes);
    unpack_program(&bytes)
}

#[test]
fn position_matching_and_recovery_hash_require_valid_checksums_and_other_bits_to_match() {
    let baseline = with_settings(&synthetic_program("Position", 42), 0.25, 0, 0.0);
    // The device checksum includes the final two data words, even though
    // the Workbench upload-side checksum normally omits them.
    let mut baseline = baseline;
    baseline[1089] = 0x123456;
    baseline[1090] = 0x654321;
    let baseline = with_settings(&baseline, 0.25, 0, 0.0);
    for (position, mode, trim, volume_taper, matches) in [
        (MOVED, 0, 0.0, false, true),
        (0.25, 1, 0.0, false, false),
        (0.25, 0, -2.0, false, false),
        (MOVED, 1, -2.0, false, false),
        (MOVED, 0, 0.0, true, false),
    ] {
        let mut source = baseline;
        if volume_taper {
            source[871] ^= 0x01_0000;
        }
        let changed = with_settings(&source, position, mode, trim);
        assert_ne!(changed[CHECKSUM_WORD], baseline[CHECKSUM_WORD]);
        assert_eq!(same_program(&baseline, &changed), matches);
        assert!(match_hash(&changed).is_some());
        assert_eq!(match_hash(&baseline) == match_hash(&changed), matches);
    }

    let mut corrupt = with_settings(&baseline, MOVED, 0, 0.0);
    corrupt[CHECKSUM_WORD] ^= 1;
    assert!(!same_program(&baseline, &corrupt));
    assert!(!same_program(&corrupt, &corrupt));
    assert!(match_hash(&corrupt).is_none());

    let mut report = StepRecord::new(StepId(3, 4));
    assert!(!differences::program(
        &mut report,
        "Stored image",
        &corrupt,
        &corrupt,
        Severity::Report,
        true,
    ));
    assert_eq!(report.comparisons.len(), 2);
    assert!(report.comparisons.iter().all(|comparison| {
        comparison.severity == Severity::Stop
            && comparison.details[0].contains("XOR 0x000001 · 1 bit(s)")
    }));
}

#[test]
fn guided_recall_keeps_valid_motion_advisory_but_saves_mixed_and_checksum_stops() {
    let id = StepId(7, 1);
    for (mode, trim, bad_checksum) in [(0, 0.0, false), (1, -2.0, false), (0, 0.0, true)] {
        let rig = SimRig::shared(Faults::default());
        let target = other_preset(rig.borrow().variax.selected.unwrap());
        let baseline = {
            let mut model = rig.borrow_mut();
            let slot = &mut model.variax.stored[flash_index(target)];
            *slot = with_settings(slot, 0.25, 0, 0.0);
            *slot
        };
        let mut readback = with_settings(&baseline, MOVED, mode, trim);
        if bad_checksum {
            readback[CHECKSUM_WORD] ^= 1;
        }
        let mut person = Script::run_all();
        let source = rig.clone();
        let mut preparing_target = true;
        person.on_prepare = Some(Box::new(move |step| {
            if step == id {
                source.borrow_mut().variax.stored[flash_index(target)] =
                    if preparing_target { readback } else { baseline };
                preparing_target = false;
            }
        }));
        let (run, _) = run_on(
            &mut PracticePort::new(rig.clone()),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        let step = run.step(id);
        let plain = render_plain(&run, "practice fixture");
        let json: serde_json::Value = serde_json::from_str(json_block(&plain).unwrap()).unwrap();
        let saved = json["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|step| step["id"] == "7.1")
            .unwrap();
        let blame = "read-only from here: step 7.1 failed";
        if mode == 0 && !bad_checksum {
            assert!(step.outcome.passed(), "{:?}", step.outcome);
            assert_eq!(saved["outcome"], "passed");
            let comparison = &step.comparisons[0];
            assert_eq!(comparison.severity, Severity::Info);
            assert_eq!(comparison.details.len(), 3, "{comparison:?}");
            assert!(
                comparison
                    .details
                    .iter()
                    .any(|value| value.contains("word 1091"))
            );
            assert!(run.step(StepId(4, 2)).outcome.passed());
            assert!(run.step(StepId(5, 1)).outcome.passed());
            continue;
        }

        assert!(
            matches!(&step.outcome, Outcome::Failed(reason) if reason.contains("does not match the 3.4 backup"))
        );
        assert_eq!(saved["outcome"], "failed");
        let stop = step
            .comparisons
            .iter()
            .find(|comparison| comparison.severity == Severity::Stop)
            .unwrap();
        if bad_checksum {
            assert!(stop.summary.contains("actual firmware checksum is invalid"));
            assert!(stop.details[0].contains("XOR 0x000001 · 1 bit(s)"));
            assert!(
                step.comparisons
                    .iter()
                    .all(|comparison| comparison.severity == Severity::Stop)
            );
        } else {
            assert!(
                step.comparisons
                    .iter()
                    .any(|comparison| comparison.severity == Severity::Info
                        && comparison.details.len() == 1)
            );
            assert!(stop.details[0].contains("word 872"));
            assert!(
                stop.details[0]
                    .contains("expected 0x000000 · actual 0x0101C0 · XOR 0x0101C0 · 4 bit(s)")
            );
            assert!(stop.details[0].contains("save_tone_position"));
            assert!(stop.details[0].contains("trim"));
            assert!(stop.details[1].contains("word 1091"));
        }
        let technical = saved["technical"].as_array().unwrap();
        for detail in &stop.details {
            assert!(plain.contains(detail));
            assert!(technical.iter().any(|line| line == detail));
        }
        for later in (1..=8)
            .map(|n| StepId(4, n))
            .chain((1..=4).map(|n| StepId(5, n)))
        {
            assert_eq!(
                run.step(later).outcome,
                Outcome::SkippedNeeds(blame.into()),
                "{later}"
            );
            assert!(run.step(later).messages.is_empty(), "{later}");
        }
        assert!(person.saves.is_empty());
        assert_eq!(rig.borrow().variax.stores, 0);
        assert_eq!(rig.borrow().podxt.stores, 0);
    }
}

#[test]
fn automatic_recovery_refuses_legacy_hash_before_selecting_and_rejects_damaged_reads() {
    for (legacy, corrupt, mode) in [
        (true, false, 0),
        (false, true, 0),
        (false, false, 1),
        (false, false, 0),
    ] {
        let rig = SimRig::shared(Faults::default());
        let (preset, baseline) = {
            let mut model = rig.borrow_mut();
            let preset = model.variax.selected.unwrap();
            let start = with_settings(&model.variax.active, 0.25, 0, 0.0);
            let slot = &mut model.variax.stored[flash_index(preset)];
            *slot = with_settings(&start, MOVED, mode, 0.0);
            if corrupt {
                slot[CHECKSUM_WORD] ^= 1;
            }
            (preset, start)
        };
        let state = RigState {
            variax_preset: Some((preset.bank(), preset.slot())),
            variax_match_sha256: if legacy {
                Some("a".repeat(64))
            } else {
                match_hash(&baseline)
            },
            ..RigState::default()
        };
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("pending.state");
        write_state(&path, &state).unwrap();
        let run = offer_restore(
            &mut PracticePort::new(rig.clone()),
            &mut Script::run_all(),
            &mut NoReport,
            &path,
            &state,
        );
        let passed = !legacy && !corrupt && mode == 0;
        assert_eq!(run.step(RIG_GUARD).outcome.passed(), passed);
        let saved = RigState::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        if !passed {
            assert_eq!(saved, state);
        } else {
            assert!(saved.finished);
        }
        if legacy {
            assert!(rig.borrow().written.is_empty());
        }
        if corrupt {
            assert!(
                run.step(RIG_GUARD)
                    .technical_lines()
                    .any(|line| line.contains("word 1091 (firmware checksum)")
                        && line.contains("XOR 0x000001"))
            );
        }
    }
}

#[test]
fn final_guard_refuses_to_upload_unsaved_edits_over_a_changed_saved_slot() {
    let rig = SimRig::shared(Faults::default());
    let preset = {
        let mut model = rig.borrow_mut();
        let preset = model.variax.selected.unwrap();
        let stored = &mut model.variax.stored[flash_index(preset)];
        *stored = with_settings(stored, 0.25, 0, 0.0);
        let mut active = *stored;
        active[2] ^= 1;
        model.variax.active = with_settings(&active, 0.25, 0, 0.0);
        preset
    };
    let source = rig.clone();
    let mut person = Script::run_all();
    person.on_card = Some(Box::new(move |id| {
        if id == RIG_GUARD {
            let mut model = source.borrow_mut();
            let slot = &mut model.variax.stored[flash_index(preset)];
            *slot = with_settings(slot, MOVED, 1, -2.0);
        }
    }));
    let (run, _) = run_on(
        &mut PracticePort::new(rig),
        &mut person,
        &mut NoReport,
        RunData::default(),
    );
    assert!(run.step(StepId(3, 4)).outcome.passed());
    let guard = run.step(RIG_GUARD);
    assert!(matches!(
        &guard.outcome,
        Outcome::Failed(reason) if reason.contains("The captured edits were not sent")
    ));
    assert!(
        !guard
            .messages
            .iter()
            .any(|line| line.contains("OUT f0 00 01 0c 07 00 43"))
    );
    assert!(guard.comparisons.iter().any(|comparison| {
        comparison.severity == Severity::Stop
            && comparison.summary.starts_with("Saved starting slot")
            && comparison.details.iter().any(|line| {
                line.contains("word 872")
                    && line.contains("save_tone_position")
                    && line.contains("trim")
            })
    }));
}
