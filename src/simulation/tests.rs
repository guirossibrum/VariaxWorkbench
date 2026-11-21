use super::*;
use crate::read_session::{ControlTransport, IDENTITY_REQUEST, MIDI_DISABLE, read_current_on};
use sha2::{Digest, Sha256};
use std::sync::atomic::AtomicBool;

fn reference() -> serde_json::Value {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../research/updater-oracle-reference.json")).unwrap();
    assert_eq!(
        data["source_sha256"],
        "6786a97458a5a6e5d225e5c2d2804281a8e0a082a0d992f558ce041c144ae078"
    );
    assert_eq!(data["cases"].as_array().unwrap().len(), 46);
    data
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn digest(frames: &[Vec<u8>]) -> String {
    let mut hash = Sha256::new();
    for frame in frames {
        hash.update(frame);
    }
    hex(&hash.finalize())
}

fn main_payload(words: usize) -> Vec<u8> {
    (0..words)
        .flat_map(|i| {
            let value = ((i as u32 * 0x12345 + 0x654321) & 0xff_ffff).to_le_bytes();
            [value[0], value[1], value[2]]
        })
        .collect()
}

fn feed(stage: Stage, frames: &[Vec<u8>], fragment: usize) -> Receiver {
    let mut receiver = Receiver::new(Profile::Variax600, stage).unwrap();
    for frame in frames {
        for part in frame.chunks(fragment) {
            receiver.feed(part).unwrap();
        }
    }
    receiver
}

#[test]
fn original_host_word_boundaries_match_independent_receiver() {
    let reference = reference();
    for case in reference["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["case"] == "code_sender")
    {
        let words = case["words"].as_u64().unwrap() as usize;
        let payload = main_payload(words);
        let frames = wire::variax(Stage::MainCode, &payload).unwrap();
        assert_eq!(digest(&frames), case["outbound_sha256"].as_str().unwrap());
        let lengths: Vec<_> = frames.iter().map(Vec::len).collect();
        assert_eq!(
            serde_json::to_value(lengths).unwrap(),
            case["frame_lengths"]
        );
        if words == 0 {
            let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
            assert_eq!(receiver.feed(&frames[0]), Err(ModelError::UnsupportedCount));
            assert!(!receiver.report().possible_persistent_change());
        } else {
            for fragment in [1, 7, 192] {
                let report = feed(Stage::MainCode, &frames, fragment).finish().unwrap();
                assert_eq!(report.phase(), Phase::ModeledReply);
                assert_eq!(report.integrity(), Integrity::Additive24Matched);
                assert_eq!(report.programming_intent_bytes(), payload.len());
                assert!(!report.durability_known());
                assert!(!report.hardware_safe());
            }
        }
    }
}

#[test]
fn malformed_main_streams_fail_without_inventing_a_write_or_rollback() {
    let frames = wire::variax(Stage::MainCode, &main_payload(33)).unwrap();
    let joined = frames.concat();
    for prefix in [0, 1, 6, 7, 12, 13, 30, joined.len() - 1] {
        let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
        receiver.feed(&joined[..prefix]).unwrap();
        assert!(receiver.finish().is_err());
        assert!(!receiver.report().possible_persistent_change());
    }
    let mut corrupt = joined.clone();
    corrupt[13] ^= 1;
    let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
    assert_eq!(receiver.feed(&corrupt), Err(ModelError::Checksum));
    assert!(!receiver.report().possible_persistent_change());
    for fault in [Fault::Timeout, Fault::Disconnect, Fault::PowerLoss] {
        let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
        receiver.feed(&joined[..20]).unwrap();
        receiver.interrupt(fault);
        assert_eq!(receiver.finish(), Err(ModelError::Interrupted));
        assert_eq!(receiver.recover(), Err(ModelError::UnsupportedRecovery));
        assert!(!receiver.report().hardware_safe());
    }
}

#[test]
fn additive_checksum_is_not_authentication_or_content_identity() {
    let a = vec![1, 0, 0, 2, 0, 0];
    let b = vec![2, 0, 0, 1, 0, 0];
    let fa = wire::variax(Stage::MainCode, &a).unwrap();
    let fb = wire::variax(Stage::MainCode, &b).unwrap();
    assert_ne!(digest(&fa), digest(&fb));
    assert_eq!(fa[fa.len() - 2], fb[fb.len() - 2]);
    for frames in [&fa, &fb] {
        let report = feed(Stage::MainCode, frames, 4).finish().unwrap();
        assert_eq!(report.integrity(), Integrity::Additive24Matched);
        assert!(!report.hardware_safe());
    }
}

#[test]
fn boot_host_shapes_match_reference_but_only_full_region_is_modeled() {
    let reference = reference();
    for case in reference["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["case"] == "boot_sender")
    {
        let len = case["synthetic_bytes"].as_u64().unwrap() as usize;
        let payload: Vec<_> = (0..len).map(|i| (i * 17 + 0xa3) as u8).collect();
        let frames = wire::variax(Stage::BootRegion, &payload).unwrap();
        assert_eq!(digest(&frames), case["outbound_sha256"].as_str().unwrap());
        if len != 16_384 {
            assert_eq!(
                simulate_synthetic(Profile::Variax600, Stage::BootRegion, &payload, 96),
                Err(ModelError::UnsupportedCount)
            );
        } else {
            let report = feed(Stage::BootRegion, &frames, 13).finish().unwrap();
            assert_eq!(report.integrity(), Integrity::AdditiveByteMatched);
            assert_eq!(report.programming_intent_bytes(), len);
            let mut corrupt = frames.clone();
            corrupt[1][0] ^= 1;
            let mut receiver = Receiver::new(Profile::Variax600, Stage::BootRegion).unwrap();
            let error = corrupt.iter().try_for_each(|frame| receiver.feed(frame));
            assert_eq!(error, Err(ModelError::Checksum));
            assert_eq!(receiver.report().programming_intent_bytes(), 0);
        }
    }
}

fn user_slots() -> Vec<u8> {
    (0..60)
        .flat_map(|slot| (0..4096).map(move |i| (slot * 17 + i * 13) as u8))
        .collect()
}

#[test]
fn user_trailers_and_factory_templates_match_original_host_hashes() {
    let reference = reference();
    for (stage, payload, case_name, expected_bytes) in [
        (
            Stage::UserPatches,
            user_slots(),
            "user_patch_sender",
            60 * 3276,
        ),
        (
            Stage::FactoryTemplates,
            (0..130950)
                .map(|i| (i * 11 + (i / 2619) * 17 + 0xa3) as u8)
                .collect(),
            "factory_template_sender",
            130950,
        ),
    ] {
        let frames = wire::variax(stage, &payload).unwrap();
        let case = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["case"] == case_name)
            .unwrap();
        assert_eq!(digest(&frames), case["outbound_sha256"].as_str().unwrap());
        assert_eq!(frames.len() as u64, case["send_calls"].as_u64().unwrap());
        // Partial programming intent survives all modeled interruption kinds.
        // Only one complete word has reached the programming-like helper.
        for fault in [Fault::Timeout, Fault::Disconnect, Fault::PowerLoss] {
            let mut partial = Receiver::new(Profile::Variax600, stage).unwrap();
            partial.feed(&frames[0]).unwrap();
            partial.feed(&frames[1][..6]).unwrap();
            assert_eq!(partial.report().programming_intent_bytes(), 3);
            partial.interrupt(fault);
            assert_eq!(partial.finish(), Err(ModelError::Interrupted));
            assert_eq!(partial.report().programming_intent_bytes(), 3);
            assert!(partial.report().possible_persistent_change());
            assert_eq!(partial.recover(), Err(ModelError::UnsupportedRecovery));
        }
        let mut receiver = feed(stage, &frames, 1);
        let report = receiver.finish().unwrap();
        assert_eq!(report.integrity(), Integrity::NoPacketChecksum);
        assert_eq!(report.programming_intent_bytes(), expected_bytes);
        assert!(!report.durability_known());
        let mut without_end = frames.clone();
        without_end.pop();
        let mut partial = feed(stage, &without_end, 71);
        assert_eq!(partial.finish(), Err(ModelError::Truncated));
        assert_eq!(partial.report().programming_intent_bytes(), expected_bytes);
        partial.interrupt(Fault::PowerLoss);
        assert!(partial.report().possible_persistent_change());
        assert_eq!(partial.recover(), Err(ModelError::UnsupportedRecovery));
    }
}

#[test]
fn patch_data_corruption_is_not_detected_by_an_absent_packet_checksum() {
    let mut frames = wire::variax(Stage::FactoryTemplates, &vec![0x23; 130950]).unwrap();
    frames[1][0] ^= 1;
    let report = feed(Stage::FactoryTemplates, &frames, 192)
        .finish()
        .unwrap();
    assert_eq!(report.phase(), Phase::ModeledReply);
    assert_eq!(report.integrity(), Integrity::NoPacketChecksum);
    assert!(!report.hardware_safe());
}

#[test]
fn erase_intent_precedes_terminator_and_faults_do_not_clear_it() {
    for stage in [Stage::EraseUser, Stage::EraseFactory] {
        let frames = wire::variax(stage, &[]).unwrap();
        let mut receiver = Receiver::new(Profile::Variax600, stage).unwrap();
        receiver.feed(&frames[0]).unwrap();
        assert!(receiver.report().erase_intent());
        assert_eq!(receiver.finish(), Err(ModelError::Truncated));
        assert!(receiver.report().possible_persistent_change());
        receiver.interrupt(Fault::Disconnect);
        assert!(receiver.report().erase_intent());
        assert_eq!(receiver.recover(), Err(ModelError::UnsupportedRecovery));
        let report = feed(stage, &frames, 1).finish().unwrap();
        assert_eq!(report.modeled_acknowledgment(), Some(0x70));
        assert!(!report.durability_known());
    }
}

#[test]
fn model_stage_and_bounds_guards_are_not_claimed_as_hardware_protection() {
    assert!(matches!(
        Receiver::new(Profile::PodxtLive, Stage::MainCode),
        Err(ModelError::WrongFamily)
    ));
    let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
    let wrong = wire::variax(Stage::EraseUser, &[]).unwrap();
    assert_eq!(receiver.feed(&wrong[0]), Err(ModelError::WrongStage));
    assert!(!receiver.report().hardware_safe());
    let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
    assert_eq!(receiver.feed(&vec![0; 8193]), Err(ModelError::Limit));
    assert_eq!(
        simulate_synthetic(Profile::Variax600, Stage::MainCode, &[1, 2, 3, 4], 1),
        Err(ModelError::UnsupportedCount)
    );
    let frames = wire::variax(Stage::MainCode, &main_payload(1)).unwrap();
    let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
    receiver.feed(&frames[0]).unwrap();
    assert_eq!(receiver.feed(&[0x10]), Err(ModelError::NonCanonicalNibble));
}

#[test]
fn pod_packing_matches_host_oracle_without_inventing_a_receiver() {
    let reference = reference();
    for case in reference["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["case"] == "pod_segment_words")
    {
        let count = case["words"].as_u64().unwrap() as usize;
        let payload: Vec<_> = (0..count)
            .flat_map(|i| ((i * 0x4321 + 0x1234) as u16).to_be_bytes())
            .collect();
        let report = describe_synthetic_pod_segment(&payload).unwrap();
        assert_eq!(
            hex(report.encoded_sha256()),
            case["outbound_sha256"].as_str().unwrap()
        );
        assert_eq!(
            report.send_calls() as u64,
            case["send_calls"].as_u64().unwrap()
        );
        assert!(!report.receiver_modeled());
        assert!(!report.acknowledgment_modeled());
        assert!(!report.durability_known());
    }
    assert_eq!(
        describe_synthetic_pod_segment(&[255; 128])
            .unwrap()
            .send_calls(),
        0
    );
    assert!(describe_synthetic_pod_segment(&[1]).is_err());
}

#[test]
fn candidate_gate_rejects_corruption_and_wrong_product_or_version() {
    use crate::firmware::synthetic::{Spec, build};
    let (product, version) = Profile::Variax600.target();
    let image = vec![0x23; 24];
    let valid = build(&Spec::basic(product, version, &image));
    let candidate = Candidate::inspect(Profile::Variax600, &valid).unwrap();
    assert_eq!(candidate.image_bytes(), 24);
    assert!(!candidate.authenticated());
    assert!(Candidate::inspect(Profile::PodxtLive, &valid).is_err());
    let wrong = build(&Spec::basic(product, version + 1, &image));
    assert!(Candidate::inspect(Profile::Variax600, &wrong).is_err());
    for end in [0, 4, valid.len() - 1] {
        assert!(Candidate::inspect(Profile::Variax600, &valid[..end]).is_err());
    }
    let mut damaged = valid;
    damaged[42] ^= 1;
    assert!(Candidate::inspect(Profile::Variax600, &damaged).is_err());
}

fn metadata() -> [u8; 192] {
    let mut raw = [0; 192];
    raw[..9].copy_from_slice(b"SIMULATED");
    for (i, byte) in raw[18..].iter_mut().enumerate() {
        *byte = (i * 13 + 7) as u8;
    }
    raw
}

#[test]
fn virtual_bridge_and_guitar_read_synthetic_metadata_with_fragmentation() {
    for fragment in [1, 7, 32, 200] {
        let raw = metadata();
        let mut rig = VirtualControl::new(raw)
            .unwrap()
            .with_fragmentation(fragment, true)
            .unwrap();
        let result = read_current_on(&mut rig, &AtomicBool::new(false)).unwrap();
        assert_eq!(result.metadata.raw, raw);
        assert_eq!(result.identity.version, Some(0x0001_0000));
        assert_eq!(rig.state(), VirtualSessionState::Disabled);
        assert_eq!(rig.virtual_pause_ms(), 100);
        assert_eq!(rig.disable_attempts(), 1);
        assert!(
            rig.sent_messages()
                .iter()
                .all(|m| crate::read_session::is_read_session_message(m))
        );
    }
}

#[test]
fn virtual_faults_exercise_client_cleanup_without_a_timer_or_device() {
    for fault in [
        ControlFault::DropMetadata(0),
        ControlFault::DropMetadata(1),
        ControlFault::DisconnectBeforeBlock(0),
        ControlFault::CancelBeforeBlock(1),
        ControlFault::FailDisable,
    ] {
        let mut rig = VirtualControl::new(metadata())
            .unwrap()
            .with_fault(fault)
            .unwrap();
        assert!(read_current_on(&mut rig, &AtomicBool::new(false)).is_err());
        assert_eq!(rig.disable_attempts(), 1);
    }
    let mut wrong = VirtualControl::new(metadata())
        .unwrap()
        .with_declared_product(0x0003_000a)
        .unwrap();
    assert!(read_current_on(&mut wrong, &AtomicBool::new(false)).is_err());
    assert_eq!(wrong.sent_messages(), &[IDENTITY_REQUEST.to_vec()]);
    let mut rig = VirtualControl::new(metadata()).unwrap();
    assert!(rig.route(0, &IDENTITY_REQUEST).is_err());
    assert!(rig.send(&crate::PODXT_ROUTING_ENABLE).is_err());
    assert!(rig.send(&crate::metadata_requests()[1]).is_err());
}

#[test]
fn duplicate_unaddressed_metadata_is_an_ambiguity_not_verified_success() {
    let mut rig = VirtualControl::new(metadata())
        .unwrap()
        .with_fault(ControlFault::DuplicateFirstMetadata)
        .unwrap();
    let error = match read_current_on(&mut rig, &AtomicBool::new(false)) {
        Ok(_) => panic!("accepted duplicate unaddressed blocks"),
        Err(error) => error,
    };
    assert!(error.contains("identical"));
    assert!(error.contains("unverified"));
    assert_eq!(rig.disable_attempts(), 1);
    assert_eq!(rig.state(), VirtualSessionState::Disabled);
    assert_eq!(rig.sent_messages().last().unwrap(), &MIDI_DISABLE);
    // Legitimately equal blocks are deliberately declined as indistinguishable.
    let mut equal = VirtualControl::new([0; 192]).unwrap();
    assert!(read_current_on(&mut equal, &AtomicBool::new(false)).is_err());
}

#[test]
fn simulation_never_enables_a_physical_backend() {
    const {
        assert!(!PHYSICAL_OPERATIONS_PERMITTED);
    }
    assert!(crate::rig::LIVE_BACKEND_INITIALIZER.is_none());
    #[cfg(feature = "usb")]
    const {
        assert!(crate::hardware_gates::G1_OS_DISCOVERY_LOCKED);
        assert!(crate::hardware_gates::G2_IDENTITY_REQUEST_LOCKED);
        assert!(crate::hardware_gates::G3_CURRENT_SETTINGS_READ_LOCKED);
        assert!(crate::hardware_gates::G4_AUDIO_CAPTURE_LOCKED);
        assert!(crate::hardware_gates::G5_FULL_BACKUP_READ_LOCKED);
        assert!(crate::hardware_gates::G6_EDIT_BUFFER_WRITE_LOCKED);
        assert!(crate::hardware_gates::G7_STORE_RESTORE_LOCKED);
    }
}

#[test]
fn boot_mode_is_an_assumption_and_rom_recovery_remains_unsupported() {
    let frames = wire::variax(Stage::MainCode, &main_payload(2)).unwrap();
    let mut receiver =
        Receiver::in_mode(Profile::Variax600, OperatingMode::BootCode, Stage::MainCode).unwrap();
    for frame in frames {
        receiver.feed(&frame).unwrap();
    }
    let report = receiver.finish().unwrap();
    assert_eq!(report.mode(), OperatingMode::BootCode);
    assert_eq!(report.phase(), Phase::ModeledReply);
    assert!(!report.peripheral_helpers_modeled());
    assert!(!report.hardware_safe());
    assert_eq!(receiver.recover(), Err(ModelError::UnsupportedRecovery));
    receiver.interrupt(Fault::PowerLoss);
    assert_eq!(receiver.report().modeled_acknowledgment(), Some(0x71));
    assert_eq!(receiver.finish(), Err(ModelError::Interrupted));
    for stage in [
        Stage::UserPatches,
        Stage::FactoryTemplates,
        Stage::EraseUser,
        Stage::EraseFactory,
    ] {
        assert!(matches!(
            Receiver::in_mode(Profile::Variax600, OperatingMode::BootCode, stage),
            Err(ModelError::UnsupportedOperation)
        ));
    }
}

#[test]
fn every_single_bit_mutation_of_a_small_code_transfer_is_declined() {
    let bytes = wire::variax(Stage::MainCode, &main_payload(1))
        .unwrap()
        .concat();
    assert_eq!(bytes.len(), 26);
    let mut scenarios = 0;
    for index in 0..bytes.len() {
        for bit in 0..8 {
            let mut mutated = bytes.clone();
            mutated[index] ^= 1 << bit;
            let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
            let outcome = receiver.feed(&mutated).and_then(|()| receiver.finish());
            assert!(
                outcome.is_err(),
                "accepted mutation at byte {index}, bit {bit}"
            );
            assert!(!receiver.report().hardware_safe());
            scenarios += 1;
        }
    }
    assert_eq!(scenarios, 208);
    assert!(
        VirtualControl::new(metadata())
            .unwrap()
            .with_fault(ControlFault::DropMetadata(2))
            .is_err()
    );
}
