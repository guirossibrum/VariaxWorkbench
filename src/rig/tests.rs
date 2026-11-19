#![forbid(unsafe_code)]

use super::capability;
use super::*;
use crate::firmware::synthetic::synthetic_container;
use crate::firmware::{
    PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION,
    VARIAX_500_700_PRODUCT_ID,
};
use crate::linux_transport::PODXT_LIVE_VARIAX_PROPERTIES;

const PRESET: &[u8] = include_bytes!("../../tests/fixtures/editor-preset.vwm");
const BANK: &[u8] = include_bytes!("../../tests/fixtures/synthetic-bank.vwb");

fn synthetic_h5ep() -> Vec<u8> {
    let mut bytes = vec![0; crate::pod_preset::H5EP_MIN_BYTES];
    bytes[..4].copy_from_slice(b"H5EP");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    let body = &mut bytes[crate::pod_preset::H5EP_HEADER_BYTES..];
    for offset in [0x228, 0x328] {
        body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
    }
    for index in 0..8 {
        let offset = 0x428 + index * 0x100;
        body[offset..offset + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
    }
    bytes
}

fn synthetic_h5es() -> Vec<u8> {
    let mut bytes = vec![0; crate::pod_preset::H5ES_MIN_BYTES];
    bytes[..4].copy_from_slice(b"H5ES");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    for index in 0..crate::pod_preset::H5ES_PRESET_COUNT {
        let start =
            crate::pod_preset::H5ES_HEADER_BYTES + index * crate::pod_preset::H5EP_MIN_BYTES;
        bytes[start..start + crate::pod_preset::H5EP_MIN_BYTES].copy_from_slice(&synthetic_h5ep());
    }
    bytes
}

fn synthetic_h5eb_with_type9_child() -> Vec<u8> {
    let mut bytes = vec![0; crate::pod_preset::H5EB_MIN_BYTES];
    bytes[..4].copy_from_slice(b"H5EB");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    for setlist in 0..crate::pod_preset::H5EB_SETLIST_COUNT {
        let start =
            crate::pod_preset::H5EB_HEADER_BYTES + setlist * crate::pod_preset::H5ES_MIN_BYTES;
        bytes[start..start + crate::pod_preset::H5ES_MIN_BYTES].copy_from_slice(&synthetic_h5es());
    }
    let child = crate::pod_preset::H5EB_HEADER_BYTES + crate::pod_preset::H5ES_HEADER_BYTES;
    bytes[child..child + crate::pod_preset::H5EP_MIN_BYTES].copy_from_slice(&type9_h5ep());
    bytes
}

fn type9_h5ep() -> Vec<u8> {
    let mut bytes = synthetic_h5ep();
    let block = crate::pod_preset::H5EP_HEADER_BYTES + 0x428 + 6 * 0x100;
    bytes[block..block + 4].copy_from_slice(&0x020a_0014u32.to_be_bytes());
    bytes[block + 8] = 1;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 1;
    let row = block + 0x10;
    bytes[row..row + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
    bytes[row + 4..row + 8].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
    bytes
}

fn with_device_id(bytes: &[u8], device_id: u32) -> Vec<u8> {
    let mut owned = bytes.to_vec();
    owned[24..28].copy_from_slice(&device_id.to_be_bytes());
    owned
}

#[test]
fn shared_hd_editor_session_keeps_file_context_distinct_from_physical_identity() {
    let session = Session::initial_rig();
    let mut library = session.new_hd_editor_library();
    let mut bytes = synthetic_h5ep();
    // These arbitrary words are accepted and retained as source-qualified
    // Mac writer context, not treated as target or firmware declarations.
    bytes[0x10..0x14].copy_from_slice(&0x0014_0005u32.to_be_bytes());
    bytes[0x14..0x18].copy_from_slice(&0x0000_0000u32.to_be_bytes());
    let body = crate::pod_preset::H5EP_HEADER_BYTES;
    let block = 0x428;
    let row = body + block + 0x10;
    bytes[body + block..body + block + 4].copy_from_slice(&0x0205_000eu32.to_be_bytes());
    bytes[row..row + 4].copy_from_slice(&0x3f10_0001u32.to_be_bytes());
    bytes[row + 4..row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());
    let original = bytes.clone();

    let summary = session
        .import_hd_editor_file(&mut library, bytes)
        .expect("offline H5EP import");
    assert_eq!(summary.kind, "H5EP preset");
    assert_eq!(summary.file_family, OfflineFileFamily::PodHdEdit);
    assert_eq!(
        summary.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert_eq!(summary.header_words_10_14, Some((0x0014_0005, 0)));
    assert_eq!(library.file(summary.index).unwrap().as_bytes(), original);
    assert_eq!(
        session.set_hd_screamer_control_percent(
            &mut library,
            summary.index,
            0,
            crate::pod_preset::ScreamerControl::Drive,
            40,
        ),
        Ok(())
    );
    assert!(library.is_dirty(summary.index));

    let directory = tempfile::tempdir().expect("temporary output directory");
    let output = directory.path().join("edited.h5p");
    session
        .save_hd_editor_copy(&library, &output, summary.index)
        .expect("new offline copy");
    let saved = std::fs::read(&output).expect("saved bytes");
    assert_eq!(&saved[..0x28], &original[..0x28]);
    let value_at = row + 4;
    assert_eq!(&saved[..value_at], &original[..value_at]);
    assert_eq!(&saved[value_at + 4..], &original[value_at + 4..]);
    assert!(matches!(
        session.save_hd_editor_copy(&library, &output, summary.index),
        Err(Rejection::Format { .. })
    ));

    let mut numeric = synthetic_h5ep();
    let control = crate::pod_preset::HD_NUMERIC_CONTROLS[0];
    assert_eq!(
        (control.raw_type(), control.min(), control.max()),
        (0x05, 0, 240)
    );
    let numeric_block = body + 0x428;
    numeric[numeric_block..numeric_block + 4].copy_from_slice(&control.model_id().to_be_bytes());
    let numeric_row = numeric_block + 0x10;
    numeric[numeric_row..numeric_row + 4].copy_from_slice(&0x3f10_0001u32.to_be_bytes());
    numeric[numeric_row + 4..numeric_row + 8].copy_from_slice(&0x3e9b_bbbcu32.to_be_bytes());
    let numeric_before = numeric.clone();
    let numeric_summary = session
        .import_hd_editor_file(&mut library, numeric)
        .expect("offline source-qualified numeric preset");
    assert_eq!(control.model_label(), "Graphic EQ");
    assert_eq!(control.label(), "80Hz");
    assert_eq!(
        library
            .file(numeric_summary.index)
            .and_then(|file| match file {
                crate::pod_preset::PodPresetFile::H5ep(preset) => {
                    preset.hd_numeric_control_value(0, control)
                }
                _ => None,
            }),
        Some(73)
    );
    session
        .set_hd_numeric_control_value(&mut library, numeric_summary.index, 0, control, 167)
        .expect("shared offline numeric edit");
    assert!(library.is_dirty(numeric_summary.index));
    let numeric_after = library.file(numeric_summary.index).unwrap().as_bytes();
    let value_at = numeric_row + 4;
    assert_eq!(&numeric_after[..value_at], &numeric_before[..value_at]);
    assert_eq!(
        &numeric_after[value_at..value_at + 4],
        &0x3f32_2222u32.to_be_bytes()
    );
    assert_eq!(
        &numeric_after[value_at + 4..],
        &numeric_before[value_at + 4..]
    );
    let numeric_output = directory.path().join("numeric-edited.h5p");
    session
        .save_hd_editor_copy(&library, &numeric_output, numeric_summary.index)
        .expect("save shared numeric edit as new copy");
    let saved_numeric = std::fs::read(numeric_output).expect("read shared numeric copy");
    assert_eq!(saved_numeric, numeric_after);
    let reparsed_numeric = crate::pod_preset::OpaqueH5epPreset::parse(saved_numeric).unwrap();
    assert_eq!(
        reparsed_numeric.hd_numeric_control_value(0, control),
        Some(167)
    );

    let control_1b = crate::pod_preset::HD_NUMERIC_CONTROLS
        .iter()
        .copied()
        .find(|control| control.raw_type() == 0x1b)
        .expect("source-joined primary Vintage Pre tag-0x1B control");
    let mut numeric_1b = synthetic_h5ep();
    let block_1b = body + 0x428;
    numeric_1b[block_1b..block_1b + 4].copy_from_slice(&control_1b.model_id().to_be_bytes());
    let row_1b = block_1b + 0x10;
    numeric_1b[row_1b..row_1b + 4].copy_from_slice(&0x3f10_0005u32.to_be_bytes());
    numeric_1b[row_1b + 4..row_1b + 8].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
    let numeric_1b_before = numeric_1b.clone();
    let numeric_1b_summary = session
        .import_hd_editor_file(&mut library, numeric_1b)
        .expect("offline type-0x1B H5EP import");
    assert_eq!(control_1b.model_label(), "Vintage Pre");
    assert_eq!(control_1b.label(), "Key 0x3F100005");
    assert_eq!(control_1b.min(), 0);
    assert_eq!(control_1b.max(), 15_000);
    session
        .set_hd_numeric_control_value(
            &mut library,
            numeric_1b_summary.index,
            0,
            control_1b,
            12_345,
        )
        .expect("shared offline primary type-0x1B value patch");
    assert!(library.is_dirty(numeric_1b_summary.index));
    let numeric_1b_after = library.file(numeric_1b_summary.index).unwrap().as_bytes();
    let mut numeric_1b_expected = numeric_1b_before.clone();
    numeric_1b_expected[row_1b + 4..row_1b + 8].copy_from_slice(&0x3f52_b021u32.to_be_bytes());
    assert_eq!(numeric_1b_after, numeric_1b_expected);
    let numeric_1b_output = directory.path().join("numeric-1b-edited.h5p");
    session
        .save_hd_editor_copy(&library, &numeric_1b_output, numeric_1b_summary.index)
        .expect("save source-qualified numeric type-0x1B copy");
    let saved_numeric_1b = std::fs::read(numeric_1b_output).expect("read saved type-0x1B copy");
    assert_eq!(saved_numeric_1b, numeric_1b_expected);
    let reparsed_numeric_1b = crate::pod_preset::OpaqueH5epPreset::parse(saved_numeric_1b).unwrap();
    assert_eq!(
        reparsed_numeric_1b.hd_numeric_control_value(0, control_1b),
        Some(12_345)
    );

    let mut mapped_context = synthetic_h5ep();
    mapped_context[0x10..0x14].copy_from_slice(&0x0021_0001u32.to_be_bytes());
    mapped_context[0x14..0x18].copy_from_slice(&0x0003_0262u32.to_be_bytes());
    let mapped_summary = session
        .import_hd_editor_file(&mut library, mapped_context)
        .expect("mapped-like context remains structural data");
    assert_eq!(
        mapped_summary.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert_eq!(
        mapped_summary.header_words_10_14,
        Some((0x0021_0001, 0x0003_0262))
    );

    let mut parent_bytes = synthetic_h5es();
    let preset_start = crate::pod_preset::H5ES_HEADER_BYTES
        + 17 * crate::pod_preset::H5EP_MIN_BYTES
        + crate::pod_preset::H5EP_HEADER_BYTES;
    let child_block = preset_start + 0x428;
    parent_bytes[child_block..child_block + 4].copy_from_slice(&0x0205_000cu32.to_be_bytes());
    parent_bytes[child_block + 0x0b] = 1;
    parent_bytes[child_block + 0x0f] = 5;
    for (row, key) in [
        0x3f10_0005u32,
        0x3f10_0001,
        0x3f10_0003,
        0x3f10_0002,
        0x3f10_0004,
    ]
    .into_iter()
    .enumerate()
    {
        let row_at = child_block + 0x10 + row * 0x14;
        parent_bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
    }
    let parent = session
        .import_hd_editor_file(&mut library, parent_bytes.clone())
        .expect("offline H5ES import");
    let child = session
        .extract_hd_editor_preset(&mut library, parent.index, 0, 17)
        .expect("common child extraction");
    assert_eq!(child.kind, "H5EP preset");
    assert_eq!(child.header_words_10_14, Some((0, 0)));
    let mutated_child_before = library.file(child.index).unwrap().as_bytes().to_vec();
    session
        .replace_hd_octave_fuzz_with_screamer_defaults(
            &mut library,
            child.index,
            crate::pod_preset::HdFxSlot::new(0).unwrap(),
        )
        .expect("bounded Screamer replacement on extracted child");
    let crate::pod_preset::PodPresetFile::H5ep(replaced_child) = library.file(child.index).unwrap()
    else {
        panic!("extracted child remains H5EP");
    };
    assert_eq!(replaced_child.effect_model_id(0), Some(0x0205_000e));
    assert_ne!(replaced_child.as_bytes(), mutated_child_before);
    assert_eq!(library.file(parent.index).unwrap().as_bytes(), parent_bytes);

    library.revert_unsaved(child.index).unwrap();
    session
        .replace_hd_octave_fuzz_with_sub_octave_fuzz_defaults(
            &mut library,
            child.index,
            crate::pod_preset::HdFxSlot::new(0).unwrap(),
        )
        .expect("bounded Sub Octave Fuzz replacement through the shared session");
    let crate::pod_preset::PodPresetFile::H5ep(replaced_child) = library.file(child.index).unwrap()
    else {
        panic!("extracted child remains H5EP");
    };
    assert_eq!(replaced_child.effect_model_id(0), Some(0x0205_0014));
    assert_eq!(library.file(parent.index).unwrap().as_bytes(), parent_bytes);

    let batch_targets = [
        (crate::pod_preset::HdDistortionDefault::JetFuzz, 0x0205_000a),
        (crate::pod_preset::HdDistortionDefault::BuzzSaw, 0x0205_0015),
        (
            crate::pod_preset::HdDistortionDefault::ClassicDistortion,
            0x0205_000b,
        ),
        (
            crate::pod_preset::HdDistortionDefault::ColorDrive,
            0x0205_0018,
        ),
        (
            crate::pod_preset::HdDistortionDefault::FacialFuzz,
            0x0205_0011,
        ),
        (crate::pod_preset::HdDistortionDefault::FuzzPi, 0x0205_000f),
        (
            crate::pod_preset::HdDistortionDefault::HeavyDistortion,
            0x0205_0016,
        ),
        (
            crate::pod_preset::HdDistortionDefault::JumboFuzz,
            0x0205_0017,
        ),
        (
            crate::pod_preset::HdDistortionDefault::Line6Distortion,
            0x0205_0012,
        ),
        (
            crate::pod_preset::HdDistortionDefault::Line6Drive,
            0x0205_0013,
        ),
        (
            crate::pod_preset::HdDistortionDefault::Overdrive,
            0x0205_0010,
        ),
        (
            crate::pod_preset::HdDistortionDefault::TubeDrive,
            0x0205_000d,
        ),
    ];
    assert!(
        session
            .replace_hd_octave_fuzz_with_admitted_defaults(
                &mut library,
                parent.index,
                crate::pod_preset::HdFxSlot::new(0).unwrap(),
                crate::pod_preset::HdDistortionDefault::JetFuzz,
            )
            .is_err()
    );
    assert_eq!(library.file(parent.index).unwrap().as_bytes(), parent_bytes);
    for (target, expected_model) in batch_targets {
        library.revert_unsaved(child.index).unwrap();
        session
            .replace_hd_octave_fuzz_with_admitted_defaults(
                &mut library,
                child.index,
                crate::pod_preset::HdFxSlot::new(0).unwrap(),
                target,
            )
            .expect("closed-set batch replacement through shared session");
        let crate::pod_preset::PodPresetFile::H5ep(replaced_child) =
            library.file(child.index).unwrap()
        else {
            panic!("extracted child remains H5EP");
        };
        assert_eq!(replaced_child.effect_model_id(0), Some(expected_model));
        assert!(library.is_dirty(child.index));
        assert_eq!(library.file(parent.index).unwrap().as_bytes(), parent_bytes);
    }

    let podxt = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6pa,
        PODXT_LIVE_PRODUCT_ID,
        1,
        &[(crate::pod_preset::AMP_SLOT, 1)],
    );
    let rejection = session.import_hd_editor_file(&mut library, podxt);
    assert!(matches!(rejection, Err(Rejection::Format { .. })));
    assert_eq!(library.len(), 6, "rejected PODxt file is not imported");

    let mut malformed = synthetic_h5ep();
    malformed[7] = 2;
    assert!(matches!(
        session.import_hd_editor_file(&mut library, malformed),
        Err(Rejection::Format { .. })
    ));
    assert_eq!(library.len(), 6, "malformed file is not imported");

    let mut bundle_bytes = vec![0; crate::pod_preset::H5EB_MIN_BYTES];
    bundle_bytes[..4].copy_from_slice(b"H5EB");
    bundle_bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    for setlist in 0..crate::pod_preset::H5EB_SETLIST_COUNT {
        let setlist_start =
            crate::pod_preset::H5EB_HEADER_BYTES + setlist * crate::pod_preset::H5ES_MIN_BYTES;
        let setlist_bytes = if setlist == 0 {
            parent_bytes.clone()
        } else {
            synthetic_h5es()
        };
        bundle_bytes[setlist_start..setlist_start + crate::pod_preset::H5ES_MIN_BYTES]
            .copy_from_slice(&setlist_bytes);
    }
    let bundle = session
        .import_hd_editor_file(&mut library, bundle_bytes.clone())
        .expect("offline H5EB import");
    let bundle_child = session
        .extract_hd_editor_preset(&mut library, bundle.index, 0, 17)
        .expect("extract Screamer test child from H5EB");
    let bundle_child_original = library
        .file(bundle_child.index)
        .unwrap()
        .as_bytes()
        .to_vec();
    session
        .replace_hd_octave_fuzz_with_screamer_defaults(
            &mut library,
            bundle_child.index,
            crate::pod_preset::HdFxSlot::new(0).unwrap(),
        )
        .expect("bounded replacement in extracted H5EB child");
    assert!(library.is_dirty(bundle_child.index));
    assert_eq!(library.file(bundle.index).unwrap().as_bytes(), bundle_bytes);
    library.revert_unsaved(bundle_child.index).unwrap();
    assert!(!library.is_dirty(bundle_child.index));
    assert_eq!(
        library.file(bundle_child.index).unwrap().as_bytes(),
        bundle_child_original
    );
    assert_eq!(library.file(bundle.index).unwrap().as_bytes(), bundle_bytes);
    session
        .replace_hd_octave_fuzz_with_sub_octave_fuzz_defaults(
            &mut library,
            bundle_child.index,
            crate::pod_preset::HdFxSlot::new(0).unwrap(),
        )
        .expect("bounded Sub Octave Fuzz replacement in extracted H5EB child");
    let crate::pod_preset::PodPresetFile::H5ep(bundle_replaced_child) =
        library.file(bundle_child.index).unwrap()
    else {
        panic!("extracted bundle child remains H5EP");
    };
    assert_eq!(bundle_replaced_child.effect_model_id(0), Some(0x0205_0014));
    assert!(library.is_dirty(bundle_child.index));
    assert_eq!(library.file(bundle.index).unwrap().as_bytes(), bundle_bytes);
    library.revert_unsaved(bundle_child.index).unwrap();
    session
        .replace_hd_octave_fuzz_with_admitted_defaults(
            &mut library,
            bundle_child.index,
            crate::pod_preset::HdFxSlot::new(0).unwrap(),
            crate::pod_preset::HdDistortionDefault::TubeDrive,
        )
        .expect("admitted batch replacement in extracted H5EB child");
    let crate::pod_preset::PodPresetFile::H5ep(bundle_batch_child) =
        library.file(bundle_child.index).unwrap()
    else {
        panic!("extracted bundle child remains H5EP");
    };
    assert_eq!(bundle_batch_child.effect_model_id(0), Some(0x0205_000d));
    assert_eq!(library.file(bundle.index).unwrap().as_bytes(), bundle_bytes);
    library.revert_unsaved(bundle_child.index).unwrap();
    assert_eq!(library.file(bundle.index).unwrap().as_bytes(), bundle_bytes);
    library.revert_unsaved(bundle_child.index).unwrap();
    assert!(!library.is_dirty(bundle_child.index));
    assert_eq!(library.file(bundle.index).unwrap().as_bytes(), bundle_bytes);
    assert_eq!(
        capability::capability(ProductProfile::PodHd500x, Operation::OpenPodPreset).state,
        CapabilityState::Unverified
    );

    // Unrecognized but structurally valid H5EP context remains opaque.
    let mut unknown = synthetic_h5ep();
    unknown[8..0x28].fill(0xa5);
    let unknown_summary = session
        .import_hd_editor_file(&mut library, unknown)
        .expect("unknown context is preserved");
    assert_eq!(
        unknown_summary.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert_eq!(
        unknown_summary.header_words_10_14,
        Some((0xa5a5_a5a5, 0xa5a5_a5a5))
    );
}

#[test]
fn type9_choice_writer_is_gated_to_the_hd500x_extension_and_standalone_h5ep() {
    use crate::pod_preset::{HdFxSlot, HdType9Choice};
    let bytes = type9_h5ep();
    let block = crate::pod_preset::H5EP_HEADER_BYTES + 0x428 + 6 * 0x100;
    let row = block + 0x10;
    let mut library = HdEditorLibrary::default();
    let index = library.import_bytes(bytes.clone()).unwrap();
    let slot = HdFxSlot::new(6).unwrap();
    let hd_session = Session::extension(ProductProfile::PodHd500x).unwrap();

    assert!(matches!(
        Session::initial_rig().set_hd_type9_choice(
            &mut library,
            index,
            slot,
            HdType9Choice::Choice2
        ),
        Err(Rejection::Unsupported {
            profile: ProductProfile::Variax600,
            operation: Operation::EditPodPreset
        })
    ));
    for profile in [ProductProfile::WorkbenchHd, ProductProfile::Jtv69] {
        assert!(matches!(
            Session::extension(profile)
                .unwrap()
                .set_hd_type9_choice(
                    &mut library,
                    index,
                    slot,
                    HdType9Choice::Choice2
                ),
            Err(Rejection::Unsupported { profile: denied, .. }) if denied == profile
        ));
    }
    let mut bad_marker = bytes.clone();
    bad_marker[row + 0x10] = 1;
    let mut rejected_library = HdEditorLibrary::default();
    let rejected_index = rejected_library.import_bytes(bad_marker.clone()).unwrap();
    assert!(
        Session::extension(ProductProfile::PodHd500x)
            .unwrap()
            .set_hd_type9_choice(
                &mut rejected_library,
                rejected_index,
                slot,
                HdType9Choice::Choice1
            )
            .is_err()
    );
    assert_eq!(
        rejected_library.file(rejected_index).unwrap().as_bytes(),
        bad_marker
    );
    assert!(!rejected_library.is_dirty(rejected_index));
    hd_session
        .set_hd_type9_choice(&mut library, index, slot, HdType9Choice::Choice2)
        .expect("admitted offline profile edit");
    assert!(library.is_dirty(index));
    assert_eq!(
        library.file(index).unwrap().as_bytes()[row + 4..row + 8],
        0x3f80_0000u32.to_be_bytes()
    );
    assert_eq!(
        library.file(index).unwrap().as_bytes()[row..row + 4],
        0x3f10_0004u32.to_be_bytes()
    );
    let edited_bytes = library.file(index).unwrap().as_bytes().to_vec();
    hd_session
        .set_hd_type9_choice(&mut library, index, slot, HdType9Choice::Choice2)
        .expect("same-word replacement is an allowed no-op");
    assert_eq!(library.file(index).unwrap().as_bytes(), edited_bytes);
    assert!(
        library.is_dirty(index),
        "same-word no-op preserves dirty state"
    );
    let other_slot = HdFxSlot::new(5).unwrap();
    assert!(
        hd_session
            .set_hd_type9_choice(&mut library, index, other_slot, HdType9Choice::Choice1)
            .is_err()
    );
    assert_eq!(library.file(index).unwrap().as_bytes(), edited_bytes);
    assert!(library.is_dirty(index), "rejection preserves dirty state");
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.file(index).unwrap().as_bytes(), bytes.as_slice());
    assert!(!library.is_dirty(index));

    hd_session
        .set_hd_type9_choice(&mut library, index, slot, HdType9Choice::Choice1)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("existing.5xe");
    std::fs::write(&collision, b"preserve existing output").unwrap();
    assert!(
        hd_session
            .save_hd_editor_copy(&library, &collision, index)
            .is_err()
    );
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"preserve existing output"
    );
    let output = directory.path().join("copy.5xe");
    hd_session
        .save_hd_editor_copy(&library, &output, index)
        .unwrap();
    let saved = std::fs::read(&output).unwrap();
    assert_eq!(saved[row + 4..row + 8], [0x3f, 0x00, 0x00, 0x00]);
    let reopened = crate::pod_preset::OpaqueH5epPreset::parse(saved).unwrap();
    assert_eq!(
        reopened.hd_type9_choice_value(slot),
        crate::pod_preset::HdType9ChoiceValue::Choice(HdType9Choice::Choice1)
    );
    library.note_saved(index).unwrap();
    assert!(!library.is_dirty(index));

    for parent_bytes in [
        {
            let mut bytes = synthetic_h5es();
            bytes[crate::pod_preset::H5ES_HEADER_BYTES
                ..crate::pod_preset::H5ES_HEADER_BYTES + crate::pod_preset::H5EP_MIN_BYTES]
                .copy_from_slice(&type9_h5ep());
            bytes
        },
        synthetic_h5eb_with_type9_child(),
    ] {
        let mut parent = HdEditorLibrary::default();
        let parent_index = parent.import_bytes(parent_bytes.clone()).unwrap();
        assert!(
            hd_session
                .set_hd_type9_choice(&mut parent, parent_index, slot, HdType9Choice::Choice1)
                .is_err()
        );
        assert_eq!(parent.file(parent_index).unwrap().as_bytes(), parent_bytes);
        assert!(!parent.is_dirty(parent_index));
        let child_index = parent.extract_preset(parent_index, 0, 0).unwrap();
        let child_original = parent.file(child_index).unwrap().as_bytes().to_vec();
        hd_session
            .set_hd_type9_choice(&mut parent, child_index, slot, HdType9Choice::Choice1)
            .unwrap();
        assert!(parent.is_dirty(child_index));
        assert_eq!(parent.file(parent_index).unwrap().as_bytes(), parent_bytes);
        let edited_child = parent.file(child_index).unwrap().as_bytes().to_vec();
        assert_ne!(edited_child, child_original);
        assert!(
            hd_session
                .set_hd_type9_choice(
                    &mut parent,
                    child_index,
                    HdFxSlot::new(5).unwrap(),
                    HdType9Choice::Choice2
                )
                .is_err()
        );
        assert_eq!(parent.file(child_index).unwrap().as_bytes(), edited_child);
        assert!(parent.is_dirty(child_index));
        assert_eq!(parent.file(parent_index).unwrap().as_bytes(), parent_bytes);
        parent.revert_unsaved(child_index).unwrap();
        assert_eq!(parent.file(child_index).unwrap().as_bytes(), child_original);
        assert!(!parent.is_dirty(child_index));
        assert_eq!(parent.file(parent_index).unwrap().as_bytes(), parent_bytes);
    }
}

#[test]
fn type10_choice_uses_independent_same_model_key_through_hd500x_session() {
    use crate::pod_preset::{HdFxSlot, HdType10Choice};
    let bytes: Vec<u8> = include_str!("../../tests/fixtures/type10-choice-source.hex")
        .split_ascii_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect();
    let mut library = HdEditorLibrary::default();
    let index = library.import_bytes(bytes.clone()).unwrap();
    let slot = HdFxSlot::new(5).unwrap();
    assert!(
        Session::initial_rig()
            .set_hd_type10_choice(&mut library, index, slot, HdType10Choice::Choice1)
            .is_err()
    );
    let session = Session::extension(ProductProfile::PodHd500x).unwrap();
    session
        .set_hd_type10_choice(&mut library, index, slot, HdType10Choice::Choice1)
        .unwrap();
    let changed = library.file(index).unwrap().as_bytes();
    assert_eq!(&changed[0x9dc..0x9e0], &[0x3f, 0x80, 0, 0]);
    assert_eq!(&changed[0x988..0x990], &bytes[0x988..0x990]);
    assert_ne!(changed, bytes);
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.file(index).unwrap().as_bytes(), bytes);

    let mut setlist = synthetic_h5es();
    let child_start = crate::pod_preset::H5ES_HEADER_BYTES;
    let child_len = crate::pod_preset::H5EP_MIN_BYTES;
    setlist[child_start..child_start + child_len].copy_from_slice(&bytes[..child_len]);
    let mut nested = HdEditorLibrary::default();
    let parent = nested.import_bytes(setlist).unwrap();
    let parent_before = nested.file(parent).unwrap().as_bytes().to_vec();
    let child = Session::initial_rig()
        .extract_hd_editor_preset(&mut nested, parent, 0, 0)
        .unwrap()
        .index;
    session
        .set_hd_type10_choice(&mut nested, child, slot, HdType10Choice::Choice0)
        .unwrap();
    assert_eq!(nested.file(parent).unwrap().as_bytes(), parent_before);
}

#[test]
fn type8_choice_is_a_separate_profile_gated_child_h5ep_edit() {
    use crate::pod_preset::{
        HD_TYPE8_CHOICE_KEY, HD_TYPE8_CHOICE_MODEL_ID, HdFxSlot, HdType8Choice, HdType8ChoiceValue,
        OpaqueH5epPreset,
    };
    let slot = HdFxSlot::new(6).unwrap();
    let block = crate::pod_preset::H5EP_HEADER_BYTES + 0x428 + 6 * 0x100;
    let row = block + 0x10;
    let mut bytes = type9_h5ep();
    bytes[block..block + 4].copy_from_slice(&HD_TYPE8_CHOICE_MODEL_ID.to_be_bytes());
    bytes[row..row + 4].copy_from_slice(&HD_TYPE8_CHOICE_KEY.to_be_bytes());
    bytes[row + 4..row + 8].copy_from_slice(&0x3eaa_aaabu32.to_be_bytes());
    let mut library = HdEditorLibrary::default();
    let index = library.import_bytes(bytes.clone()).unwrap();

    assert!(
        Session::initial_rig()
            .set_hd_type8_choice(&mut library, index, slot, HdType8Choice::Choice1)
            .is_err()
    );
    for profile in [ProductProfile::WorkbenchHd, ProductProfile::Jtv69] {
        assert!(
            Session::extension(profile)
                .unwrap()
                .set_hd_type8_choice(&mut library, index, slot, HdType8Choice::Choice1)
                .is_err()
        );
    }
    let hd = Session::extension(ProductProfile::PodHd500x).unwrap();
    // The type-9 model/key association cannot borrow this distinct two-choice API.
    let mut foreign = bytes.clone();
    foreign[block..block + 4].copy_from_slice(&0x020a_0014u32.to_be_bytes());
    foreign[row..row + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
    let mut foreign_library = HdEditorLibrary::default();
    let foreign_index = foreign_library.import_bytes(foreign.clone()).unwrap();
    assert!(
        hd.set_hd_type8_choice(
            &mut foreign_library,
            foreign_index,
            slot,
            HdType8Choice::Choice1
        )
        .is_err()
    );
    assert_eq!(
        foreign_library.file(foreign_index).unwrap().as_bytes(),
        foreign
    );
    assert!(!foreign_library.is_dirty(foreign_index));

    hd.set_hd_type8_choice(&mut library, index, slot, HdType8Choice::Choice1)
        .unwrap();
    let mut expected = bytes.clone();
    expected[row + 4..row + 8].copy_from_slice(&[0x3f, 0x80, 0x00, 0x00]);
    assert_eq!(library.file(index).unwrap().as_bytes(), expected);
    assert!(library.is_dirty(index));
    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("type8.5xe");
    let collision = directory.path().join("collision.5xe");
    std::fs::write(&collision, b"keep existing bytes").unwrap();
    assert!(hd.save_hd_editor_copy(&library, &collision, index).is_err());
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing bytes");
    hd.save_hd_editor_copy(&library, &copy, index).unwrap();
    assert_eq!(std::fs::read(&copy).unwrap(), expected);
    assert_eq!(
        OpaqueH5epPreset::parse(std::fs::read(copy).unwrap())
            .unwrap()
            .hd_type8_choice_value(slot),
        HdType8ChoiceValue::Choice(HdType8Choice::Choice1)
    );
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.file(index).unwrap().as_bytes(), bytes);
    assert!(!library.is_dirty(index));

    // Editing an extracted child never replaces or dirties the parent.
    let mut parent_bytes = synthetic_h5es();
    let child_offset = crate::pod_preset::H5ES_HEADER_BYTES;
    parent_bytes[child_offset..child_offset + crate::pod_preset::H5EP_MIN_BYTES]
        .copy_from_slice(&bytes);
    let mut parent = HdEditorLibrary::default();
    let parent_index = parent.import_bytes(parent_bytes.clone()).unwrap();
    let child_index = parent.extract_preset(parent_index, 0, 0).unwrap();
    let child_before = parent.file(child_index).unwrap().as_bytes().to_vec();
    assert!(
        hd.set_hd_type8_choice(&mut parent, parent_index, slot, HdType8Choice::Choice1)
            .is_err()
    );
    hd.set_hd_type8_choice(&mut parent, child_index, slot, HdType8Choice::Choice1)
        .unwrap();
    assert_ne!(parent.file(child_index).unwrap().as_bytes(), child_before);
    assert_eq!(parent.file(parent_index).unwrap().as_bytes(), parent_bytes);
    assert!(parent.is_dirty(child_index));
    assert!(!parent.is_dirty(parent_index));
    parent.revert_unsaved(child_index).unwrap();
    assert_eq!(parent.file(child_index).unwrap().as_bytes(), child_before);
    assert_eq!(parent.file(parent_index).unwrap().as_bytes(), parent_bytes);
}

#[test]
fn admitted_type9_catalog_entries_work_through_session_and_save_reopen() {
    use crate::pod_preset::{HdFxSlot, HdType9Choice, HdType9ChoiceValue, OpaqueH5epPreset};
    let associations: [(u32, u32, usize); 3] = [
        (0x020a_0010, 0x3f10_0004, 1),
        (0x020a_000f, 0x3f10_0004, 3),
        (0x020a_0012, 0x3f10_0003, 7),
    ];
    let expected = [
        (HdType9Choice::Choice0, 0x0000_0000u32),
        (HdType9Choice::Choice1, 0x3f00_0000),
        (HdType9Choice::Choice2, 0x3f80_0000),
    ];
    let session = Session::extension(ProductProfile::PodHd500x).unwrap();

    for (model_id, key, slot_index) in associations {
        let mut bytes = type9_h5ep();
        let block = crate::pod_preset::H5EP_HEADER_BYTES + 0x428 + slot_index * 0x100;
        bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
        bytes[block + 8] = (slot_index % 2) as u8;
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 1;
        let row = block + 0x10;
        bytes[row..row + 4].copy_from_slice(&key.to_be_bytes());
        bytes[row + 4..row + 8].copy_from_slice(&0x3e80_0000u32.to_be_bytes());
        let mut library = HdEditorLibrary::default();
        let index = library.import_bytes(bytes.clone()).unwrap();
        let slot = HdFxSlot::new(slot_index).unwrap();
        for (choice, value_word) in expected {
            let before = library.file(index).unwrap().as_bytes().to_vec();
            let mut expected_bytes = before.clone();
            expected_bytes[row + 4..row + 8].copy_from_slice(&value_word.to_be_bytes());
            session
                .set_hd_type9_choice(&mut library, index, slot, choice)
                .unwrap();
            assert_eq!(library.file(index).unwrap().as_bytes(), expected_bytes);
            assert_eq!(
                library.file(index).unwrap().as_bytes()[row..row + 4],
                key.to_be_bytes()
            );
            assert!(library.is_dirty(index));
        }
        let edited = library.file(index).unwrap().as_bytes().to_vec();
        session
            .set_hd_type9_choice(&mut library, index, slot, HdType9Choice::Choice2)
            .unwrap();
        assert_eq!(library.file(index).unwrap().as_bytes(), edited);
        assert!(
            library.is_dirty(index),
            "no-op keeps the edited entry dirty"
        );
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("type9-catalog-copy.5xe");
        session
            .save_hd_editor_copy(&library, &output, index)
            .unwrap();
        let reopened = OpaqueH5epPreset::parse(std::fs::read(output).unwrap()).unwrap();
        assert_eq!(
            reopened.hd_type9_choice_value(slot),
            HdType9ChoiceValue::Choice(HdType9Choice::Choice2)
        );
    }

    let cross_pairs: [(u32, u32); 4] = [
        (0x020a_0010, 0x3f10_0003),
        (0x020a_000f, 0x3f10_0003),
        (0x020a_0012, 0x3f10_0004),
        (0x020a_0013, 0x3f10_0004),
    ];
    for (model_id, wrong_key) in cross_pairs {
        let mut bytes = type9_h5ep();
        let block = crate::pod_preset::H5EP_HEADER_BYTES + 0x428 + 2 * 0x100;
        bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
        bytes[block + 0x0b] = 1;
        bytes[block + 0x0f] = 1;
        bytes[block + 0x10..block + 0x14].copy_from_slice(&wrong_key.to_be_bytes());
        let mut library = HdEditorLibrary::default();
        let index = library.import_bytes(bytes.clone()).unwrap();
        assert!(
            session
                .set_hd_type9_choice(
                    &mut library,
                    index,
                    HdFxSlot::new(2).unwrap(),
                    HdType9Choice::Choice1
                )
                .is_err()
        );
        assert_eq!(library.file(index).unwrap().as_bytes(), bytes);
        assert!(!library.is_dirty(index));
    }

    let mut parent_bytes = synthetic_h5eb_with_type9_child();
    let child_offset = crate::pod_preset::H5EB_HEADER_BYTES + crate::pod_preset::H5ES_HEADER_BYTES;
    let child_block = child_offset + crate::pod_preset::H5EP_HEADER_BYTES + 0x428 + 6 * 0x100;
    parent_bytes[child_block..child_block + 4].copy_from_slice(&0x020a_0012u32.to_be_bytes());
    parent_bytes[child_block + 0x10..child_block + 0x14]
        .copy_from_slice(&0x3f10_0003u32.to_be_bytes());
    let mut parent = HdEditorLibrary::default();
    let parent_index = parent.import_bytes(parent_bytes.clone()).unwrap();
    let child_index = parent.extract_preset(parent_index, 0, 0).unwrap();
    let child_before = parent.file(child_index).unwrap().as_bytes().to_vec();
    session
        .set_hd_type9_choice(
            &mut parent,
            child_index,
            HdFxSlot::new(6).unwrap(),
            HdType9Choice::Choice1,
        )
        .unwrap();
    assert_ne!(parent.file(child_index).unwrap().as_bytes(), child_before);
    assert_eq!(parent.file(parent_index).unwrap().as_bytes(), parent_bytes);
    assert!(parent.is_dirty(child_index));
    assert!(!parent.is_dirty(parent_index));
}

#[test]
fn hd500x_enable_session_is_profile_gated_and_keeps_file_and_version_guards() {
    let hd500x = Session::extension(ProductProfile::PodHd500x).unwrap();
    let mut library = hd500x.new_hd_editor_library();
    let mut bytes = synthetic_h5ep();
    let body = crate::pod_preset::H5EP_HEADER_BYTES;
    let block = body + 0x428;
    bytes[block..block + 4].copy_from_slice(&0x0205_000eu32.to_be_bytes());
    bytes[block + 0x0b] = 1;
    let summary = hd500x
        .import_hd_editor_file(&mut library, bytes.clone())
        .expect("supported H5EP v1");
    let slot = crate::pod_preset::HdFxSlot::new(0).unwrap();
    hd500x
        .set_hd_effect_enabled(&mut library, summary.index, slot, true)
        .expect("bounded HD500X offline enable edit");
    assert!(library.is_dirty(summary.index));
    let crate::pod_preset::PodPresetFile::H5ep(preset) = library.file(summary.index).unwrap()
    else {
        panic!("the imported file remains an H5EP");
    };
    assert_eq!(preset.effect_enabled(slot), Some(true));

    let mut unknown_model = synthetic_h5ep();
    unknown_model[block..block + 4].copy_from_slice(&0x02ad_beefu32.to_be_bytes());
    let unknown = hd500x
        .import_hd_editor_file(&mut library, unknown_model)
        .unwrap();
    let before = library.file(unknown.index).unwrap().as_bytes().to_vec();
    assert!(
        hd500x
            .set_hd_effect_enabled(&mut library, unknown.index, slot, true)
            .is_err()
    );
    assert_eq!(library.file(unknown.index).unwrap().as_bytes(), before);

    for unsupported in [
        Session::initial_rig(),
        Session::extension(ProductProfile::WorkbenchHd).unwrap(),
        Session::extension(ProductProfile::Jtv69).unwrap(),
    ] {
        let before = library.file(summary.index).unwrap().as_bytes().to_vec();
        assert!(
            unsupported
                .set_hd_effect_enabled(&mut library, summary.index, slot, false)
                .is_err()
        );
        assert_eq!(library.file(summary.index).unwrap().as_bytes(), before);
    }

    let mut bad_version = synthetic_h5ep();
    bad_version[7] = 2;
    assert!(
        hd500x
            .import_hd_editor_file(&mut library, bad_version)
            .is_err()
    );
    assert_eq!(
        library.file(summary.index).unwrap().as_bytes()[block + 8],
        1
    );
}

#[test]
fn shared_session_replaces_one_octave_fuzz_with_source_defaults_and_reopens_copy() {
    use crate::pod_preset::HdFxSlot;

    let session = Session::initial_rig();
    let mut library = session.new_hd_editor_library();
    let mut original = synthetic_h5ep();
    let body = crate::pod_preset::H5EP_HEADER_BYTES;
    let block = body + 0x428;
    original[block..block + 4].copy_from_slice(&0x0205_000cu32.to_be_bytes());
    original[block + 4..block + 8].copy_from_slice(&0x1234_5678u32.to_be_bytes());
    original[block + 8..block + 0x0c].copy_from_slice(&[1, 0x5a, 0xa5, 1]);
    original[block + 0x0f] = 8;
    let keys = [
        0x3f10_0005u32,
        0x3f10_0001,
        0x3f10_0003,
        0x3f10_0002,
        0x3f10_0004,
    ];
    for (row, key) in keys.iter().copied().enumerate().chain((5..12).map(|row| {
        let key = if row == 6 {
            0x3f10_0005
        } else {
            0x6abc_0000 + row as u32
        };
        (row, key)
    })) {
        let row_at = block + 0x10 + row * 0x14;
        original[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        original[row_at + 4..row_at + 8]
            .copy_from_slice(&(0x3e80_0000u32 + row as u32).to_be_bytes());
        original[row_at + 8..row_at + 12]
            .copy_from_slice(&(0x7100_0000u32 + row as u32).to_be_bytes());
        original[row_at + 0x10] = (row % 3) as u8;
        original[row_at + 0x11..row_at + 0x14].copy_from_slice(&[
            0xc1 + row as u8,
            0xd1 + row as u8,
            0xe1 + row as u8,
        ]);
    }
    let summary = session
        .import_hd_editor_file(&mut library, original.clone())
        .expect("H5EP file import");
    let slot = HdFxSlot::new(0).unwrap();
    session
        .replace_hd_octave_fuzz_with_vintage_pre_defaults(&mut library, summary.index, slot)
        .expect("typed offline model replacement");
    assert!(library.is_dirty(summary.index));

    let mut expected = original.clone();
    expected[block..block + 4].copy_from_slice(&0x020c_000eu32.to_be_bytes());
    for (row, key) in [
        0x3f10_0002u32,
        0x3f10_0003,
        0x3f10_0004,
        0x3f10_0005,
        0x3f10_0001,
    ]
    .into_iter()
    .enumerate()
    {
        let row_at = block + 0x10 + row * 0x14;
        expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
    }
    expected[block + 0x0f] = 5;
    for row in 0..12 {
        expected[block + 0x10 + row * 0x14 + 0x10] = 0;
    }
    assert_eq!(library.file(summary.index).unwrap().as_bytes(), expected);
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("existing.5xe");
    let output = directory.path().join("vintage-pre-copy.5xe");
    std::fs::write(&collision, b"keep destination").unwrap();
    assert!(
        session
            .save_hd_editor_copy(&library, &collision, summary.index)
            .is_err()
    );
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep destination");
    assert!(library.is_dirty(summary.index));

    session
        .save_hd_editor_copy(&library, &output, summary.index)
        .expect("non-overwriting copy save");
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    library.note_saved(summary.index).unwrap();
    assert!(!library.is_dirty(summary.index));
    assert_eq!(
        crate::pod_preset::OpaqueH5epPreset::parse(std::fs::read(output).unwrap())
            .unwrap()
            .effect_model_id(0),
        Some(0x020c_000e)
    );
    library.revert_unsaved(summary.index).unwrap();
    assert_eq!(library.file(summary.index).unwrap().as_bytes(), expected);
}

#[test]
fn initial_rig_is_one_usb_pod_with_two_logical_roles() {
    let session = Session::initial_rig();
    assert!(session.live_backend_is_absent());
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    let host = session.host().expect("initial rig has a static host");
    assert_eq!(host.vendor_id, 0x0e41);
    assert_eq!(host.product_id, 0x4650);
    assert_eq!(host.roles.len(), 2);
    assert_eq!(host.roles[0].kind, RoleKind::Guitar);
    assert_eq!(host.roles[0].profile, ProductProfile::Variax600);
    assert_eq!(host.roles[0].interface_number, 1);
    assert_eq!(host.roles[0].driver_info, 0);
    assert_eq!(host.roles[1].kind, RoleKind::Pod);
    assert_eq!(host.roles[1].profile, ProductProfile::PodxtLive);
    assert_eq!(host.roles[1].interface_number, 0);
    assert_eq!(host.roles[1].driver_info, 5);
    assert_ne!(
        u32::from(host.roles[0].interface_number),
        PODXT_LIVE_VARIAX_PROPERTIES.ctrl_if
    );
    assert_eq!(WORKBENCH_USB_IS_A_SEPARATE_MATCH.product_id, 0x534d);
    assert!(
        host.roles
            .iter()
            .all(|role| role.profile.archived_firmware_product() != Some(0x534d))
    );
    assert!(Session::extension(ProductProfile::Variax600).is_err());
    assert!(Session::extension(ProductProfile::PodxtLive).is_err());
}

#[test]
fn guitar_files_stay_on_the_variax_backend() {
    let session = Session::initial_rig();
    let preset = session
        .open_preset(RoleKind::Guitar, PRESET)
        .expect("synthetic preset");
    assert_eq!(preset.name, "Synthetic");
    assert_eq!(preset.device_id, VARIAX_300_600_PRODUCT_ID);
    let bank = session.open_guitar_bank(BANK).expect("synthetic bank");
    assert!(bank.fixed);
    assert_eq!(bank.presets, 5);

    let pod_open = session.open_preset(RoleKind::Pod, b"not a preset");
    assert_eq!(
        pod_open,
        Err(Rejection::Unsupported {
            profile: ProductProfile::PodxtLive,
            operation: Operation::OpenGuitarPreset,
        })
    );
    assert!(matches!(
        session.open_preset(RoleKind::Guitar, b"not a preset"),
        Err(Rejection::Format { .. })
    ));
    assert_eq!(
        session.open_preset(
            RoleKind::Guitar,
            &with_device_id(PRESET, VARIAX_500_700_PRODUCT_ID)
        ),
        Err(Rejection::DifferentModel {
            profile: ProductProfile::Variax600,
            device_id: VARIAX_500_700_PRODUCT_ID,
        })
    );
    assert_eq!(
        session.open_preset(
            RoleKind::Guitar,
            &with_device_id(PRESET, PODXT_LIVE_PRODUCT_ID)
        ),
        Err(Rejection::CrossFamily {
            from: DeviceFamily::PodXt,
            to: DeviceFamily::VariaxWorkbench,
            operation: Operation::OpenGuitarPreset,
        })
    );

    let directory = tempfile::tempdir().expect("temp dir");
    let copy = directory.path().join("copy.vwm");
    session.save_guitar_copy(&copy, PRESET).expect("new copy");
    assert!(session.save_guitar_copy(&copy, PRESET).is_err());
    assert_eq!(std::fs::read(&copy).expect("copy"), PRESET);

    session
        .accept_family_math(RoleKind::Guitar, VARIAX_300_600_PRODUCT_ID)
        .expect("same-family id");
    assert!(matches!(
        session.accept_family_math(RoleKind::Guitar, PODXT_LIVE_PRODUCT_ID),
        Err(Rejection::CrossFamily { .. })
    ));
    assert_eq!(
        session.accept_family_math(RoleKind::Guitar, VARIAX_500_700_PRODUCT_ID),
        Err(Rejection::DifferentModel {
            profile: ProductProfile::Variax600,
            device_id: VARIAX_500_700_PRODUCT_ID,
        })
    );
    assert_eq!(
        session.accept_family_math(RoleKind::Pod, VARIAX_300_600_PRODUCT_ID),
        Err(Rejection::Unsupported {
            profile: ProductProfile::PodxtLive,
            operation: Operation::ApplyFamilyMath,
        })
    );
}

#[test]
fn firmware_inspection_keeps_target_gates_and_generic_extension_scope_separate() {
    let session = Session::initial_rig();
    let variax = synthetic_container(
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
        b"variax-image",
    );
    let pod = synthetic_container(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, b"pod-image");
    let wrong_version = synthetic_container(
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION.wrapping_add(1),
        b"variax-image",
    );
    let guitar = session
        .inspect_firmware(RoleKind::Guitar, &variax)
        .expect("variax container");
    assert_eq!(guitar.product_id, VARIAX_300_600_PRODUCT_ID);
    assert_eq!(guitar.version, VARIAX_300_600_VERSION);
    assert!(guitar.integrity_is_not_authentication);
    assert_eq!(
        session.inspect_firmware(RoleKind::Pod, &variax),
        Err(Rejection::WrongProduct {
            declared: VARIAX_300_600_PRODUCT_ID,
            expected: PODXT_LIVE_PRODUCT_ID,
        })
    );
    let pod_summary = session
        .inspect_firmware(RoleKind::Pod, &pod)
        .expect("pod container");
    assert_eq!(pod_summary.product_id, PODXT_LIVE_PRODUCT_ID);
    assert_eq!(
        session.inspect_firmware(RoleKind::Guitar, &pod),
        Err(Rejection::WrongProduct {
            declared: PODXT_LIVE_PRODUCT_ID,
            expected: VARIAX_300_600_PRODUCT_ID,
        })
    );
    assert_eq!(
        session.inspect_firmware(RoleKind::Guitar, &wrong_version),
        Err(Rejection::WrongVersion {
            declared: VARIAX_300_600_VERSION.wrapping_add(1),
            expected: VARIAX_300_600_VERSION,
        })
    );

    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension profile");
    assert_eq!(
        capability::capability(ProductProfile::PodHd500x, Operation::InspectFirmware).state,
        CapabilityState::Offline
    );
    let hd500x_declaration = synthetic_container(0x0014_0005, 0x0262_0000, b"synthetic-p25-image");
    let structural = extension
        .inspect_extension_firmware(&hd500x_declaration)
        .expect("generic structural inspection does not require target applicability");
    assert_eq!(structural.declared_product_id, 0x0014_0005);
    assert_eq!(structural.declared_version, 0x0262_0000);
    assert_eq!(
        structural.catalog_applicability,
        crate::firmware::Applicability::UnknownProduct
    );
    assert_eq!(structural.image_bytes, b"synthetic-p25-image".len());
    assert!(structural.header_md5_matches);
    assert_eq!(structural.region_count, 1);
    assert_eq!(
        structural.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert!(structural.integrity_is_not_authentication);

    // Even a known other-family declaration is only a structurally valid file
    // here; the extension session never changes its physical target gate.
    let other_family = extension
        .inspect_extension_firmware(&synthetic_container(
            PODXT_LIVE_PRODUCT_ID,
            PODXT_LIVE_VERSION,
            b"pod-image",
        ))
        .expect("generic inspection accepts a valid FORM/L6FF container");
    assert_eq!(
        other_family.physical_target,
        PhysicalTargetApplicability::Unknown
    );
    assert_eq!(
        extension.inspect_extension_firmware(b"not a firmware container"),
        Err(Rejection::Format {
            detail: "expected FORM/L6FF".into(),
        })
    );
    assert_eq!(
        Session::extension(ProductProfile::WorkbenchHd)
            .expect("other extension")
            .inspect_extension_firmware(&hd500x_declaration),
        Err(Rejection::Unverified {
            profile: ProductProfile::WorkbenchHd,
            operation: Operation::InspectFirmware,
        })
    );
}

#[test]
fn versions_that_are_unknown_stay_unknown() {
    let session = Session::initial_rig();
    match session
        .version(RoleKind::Guitar, Component::ArchivedContainer)
        .expect("catalog")
    {
        VersionKnowledge::ArchivedDeclaration { word, label, .. } => {
            assert_eq!(word, VARIAX_300_600_VERSION);
            assert_eq!(label, "3.10");
        }
        other => panic!("archived guitar container was {other:?}"),
    }
    assert!(matches!(
        session
            .version(RoleKind::Guitar, Component::InstalledUnitRevision)
            .expect("unit"),
        VersionKnowledge::Unknown { .. }
    ));
    assert!(matches!(
        session
            .version(RoleKind::Guitar, Component::InstalledUsbFirmware)
            .expect("guitar usb component"),
        VersionKnowledge::NotApplicable { .. }
    ));
    match session
        .version(RoleKind::Pod, Component::InstalledUsbFirmware)
        .expect("pod usb component")
    {
        VersionKnowledge::Unknown { evidence } => {
            assert!(evidence.contains("#20"));
            assert!(evidence.contains("bcdDevice"));
            assert!(!VersionKnowledge::Unknown { evidence }.is_known_value());
        }
        other => panic!("usb component was {other:?}"),
    }
    for profile in [
        ProductProfile::WorkbenchHd,
        ProductProfile::Jtv69,
        ProductProfile::PodHd500x,
    ] {
        let extension = Session::extension(profile).expect("extension");
        assert!(profile.archived_firmware_product().is_none());
        assert!(matches!(
            extension
                .extension_version(Component::ArchivedContainer)
                .expect("version"),
            VersionKnowledge::NotEstablished { .. }
        ));
        assert_eq!(
            extension.extension_math(VARIAX_300_600_PRODUCT_ID),
            Err(Rejection::Unverified {
                profile,
                operation: Operation::ApplyFamilyMath,
            })
        );
    }
    assert_ne!(
        ProductProfile::WorkbenchHd.family(),
        ProductProfile::Variax600.family()
    );
    assert_ne!(
        ProductProfile::Jtv69.family(),
        ProductProfile::WorkbenchHd.family()
    );
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::PodxtLive.family()
    );
    assert_ne!(ProductProfile::Jtv69, ProductProfile::WorkbenchHd);
}

#[test]
fn physical_claims_and_planned_work_do_not_start_a_backend() {
    let session = Session::initial_rig();
    for profile in ProductProfile::ALL {
        for operation in Operation::ALL {
            let claimed = capability::resolve(profile, operation, Some(CapabilityState::Offline));
            let bare = capability::resolve(profile, operation, None);
            assert_eq!(claimed, bare);
            if operation.is_physical() {
                assert_eq!(claimed, CapabilityState::Locked);
            } else if profile.is_extension() {
                let expected = if profile == ProductProfile::PodHd500x
                    && operation == Operation::InspectFirmware
                {
                    CapabilityState::Offline
                } else {
                    CapabilityState::Unverified
                };
                assert_eq!(claimed, expected);
            }
        }
    }

    let mut attempts = 0;
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::OpenAudioStream,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::OpenAudioStream,
        })
    );
    let extension = Session::extension(ProductProfile::Jtv69).expect("jtv");
    let extension_locked = extension.reject_extension_physical(
        Operation::EnumerateUsb,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        extension_locked,
        Err(Rejection::Locked {
            operation: Operation::EnumerateUsb,
        })
    );

    assert_eq!(
        session.capability(RoleKind::Pod, Operation::EditPodPreset),
        Ok(Capability {
            profile: ProductProfile::PodxtLive,
            operation: Operation::EditPodPreset,
            state: CapabilityState::Offline,
            note: capability::capability(ProductProfile::PodxtLive, Operation::EditPodPreset).note,
        })
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::OpenPodPreset)
            .expect("pod open")
            .state,
        CapabilityState::Offline
    );
    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::OpenPodPreset)
            .expect("guitar pod open")
            .state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::SelectRecordRoute)
            .expect("route")
            .state,
        CapabilityState::Offline
    );
    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::SelectRecordRoute)
            .expect("guitar route")
            .state,
        CapabilityState::Unsupported
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::ReadInstalledUsbComponent)
            .expect("usb read")
            .state,
        CapabilityState::Locked
    );
}

#[test]
fn pod_record_route_stays_offline_while_capture_stays_locked() {
    let session = Session::initial_rig();
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    assert!(session.live_backend_is_absent());
    let planned = session
        .select_record_route(crate::pod_audio::RecordRoute::SendCleanGuitar, 66, 2)
        .expect("clean route");
    assert!(!planned.stream_opened);
    assert!(planned.outbound.is_empty());
    assert_eq!(planned.behavior.mode_number, 2);
    assert!(!planned.coexistence.guitar.source_initializes_pcm);
    assert!(planned.coexistence.pod.source_initializes_pcm);

    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::SelectRecordRoute)
            .expect("guitar")
            .state,
        CapabilityState::Unsupported
    );
    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension");
    assert_eq!(
        extension.select_record_route(crate::pod_audio::RecordRoute::SendProcessedGuitar, 66, 1),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::SelectRecordRoute,
        })
    );

    let mut attempts = 0;
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::StartAudioCapture,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::StartAudioCapture,
        })
    );
    assert_eq!(
        session.select_record_route(crate::pod_audio::RecordRoute::SendProcessedGuitar, 64, 1),
        Err(Rejection::Format {
            detail: "period byte count is not a multiple of the 6-byte frame".into(),
        })
    );
}

#[test]
fn pod_tone_open_rename_and_save_stay_offline_and_reject_variax() {
    let session = Session::initial_rig();
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    assert!(session.live_backend_is_absent());
    let original = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6as,
        PODXT_LIVE_PRODUCT_ID,
        0x0102_0304,
        &[
            (crate::pod_preset::AMP_SLOT, 0x1111_0002),
            (crate::pod_preset::CAB_SLOT, 0x2222_0003),
            (0x0003_00aa, 0x3333_0004),
        ],
    );
    let summary = session
        .open_pod_preset(RoleKind::Pod, &original)
        .expect("pod tone");
    assert_eq!(summary.container, "L6AS");
    assert_eq!(summary.device_id, PODXT_LIVE_PRODUCT_ID);
    assert_eq!(summary.device_version, 0x0102_0304);
    assert_ne!(summary.device_version, PODXT_LIVE_VERSION);
    assert_eq!(summary.amp_model_id, Some(0x1111_0002));
    assert_eq!(summary.cab_model_id, Some(0x2222_0003));
    assert_eq!(summary.amp_name, None);
    assert_eq!(summary.cab_name, None);
    assert_eq!(summary.undecoded_slots, 1);
    assert_eq!(summary.recorded_parameter_chunks, 3);
    assert_eq!(summary.undecoded_parameter_chunks, 3);
    assert_eq!(
        session.open_pod_preset(RoleKind::Guitar, &original),
        Err(Rejection::Unsupported {
            profile: ProductProfile::Variax600,
            operation: Operation::OpenPodPreset,
        })
    );
    assert_eq!(
        session.open_pod_preset(RoleKind::Pod, PRESET),
        Err(Rejection::CrossFamily {
            from: DeviceFamily::VariaxWorkbench,
            to: DeviceFamily::PodXt,
            operation: Operation::OpenPodPreset,
        })
    );
    let bass = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6pa,
        0x0003_000b,
        1,
        &[(crate::pod_preset::AMP_SLOT, 1)],
    );
    assert!(matches!(
        session.open_pod_preset(RoleKind::Pod, &bass),
        Err(Rejection::Format { detail }) if detail.contains("0003000b")
    ));
    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension");
    assert_eq!(
        extension.open_pod_preset(RoleKind::Pod, &original),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::OpenPodPreset,
        })
    );
    let renamed = session
        .rename_pod_preset(RoleKind::Pod, &original, "Stage")
        .expect("rename");
    assert_ne!(renamed, original);
    let again = session
        .open_pod_preset(RoleKind::Pod, &renamed)
        .expect("renamed");
    assert_eq!(again.name, "Stage");
    assert_eq!(again.amp_model_id, Some(0x1111_0002));
    assert!(
        session
            .rename_pod_preset(RoleKind::Pod, PRESET, "Nope")
            .is_err()
    );
    let directory = tempfile::tempdir().expect("temp dir");
    let copy = directory.path().join("tone.bin");
    session
        .save_pod_copy(RoleKind::Pod, &copy, &renamed)
        .expect("new copy");
    assert!(
        session
            .save_pod_copy(RoleKind::Pod, &copy, &renamed)
            .is_err()
    );
    assert_eq!(std::fs::read(&copy).expect("copy"), renamed);
    assert!(
        session
            .save_pod_copy(RoleKind::Guitar, &copy, &renamed)
            .is_err()
    );
    assert_eq!(std::fs::read(&copy).expect("copy remains"), renamed);
    let mut attempts = 0;
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::SendControl,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::SendControl,
        })
    );
    assert_eq!(
        session
            .capability(RoleKind::Pod, Operation::SwitchBank)
            .expect("bank")
            .state,
        CapabilityState::Locked
    );
}

#[test]
fn offline_recording_gate_does_not_call_capture_or_open_a_stream() {
    let session = Session::initial_rig();
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    assert!(session.live_backend_is_absent());
    let mut attempts = 0;
    let opened = crate::pod_audio::plan(
        crate::pod_audio::RecordRoute::SendCleanGuitar,
        crate::recording::OFFLINE_ROUTE_PERIOD_BYTES,
        crate::recording::OFFLINE_ROUTE_PERIODS,
        || attempts += 1,
    )
    .expect("plan");
    assert_eq!(attempts, 0);
    assert!(!opened.stream_opened);
    assert!(opened.outbound.is_empty());
    let gate = session
        .gate_offline_recording(
            crate::pod_audio::RecordRoute::SendCleanGuitar,
            crate::recording::OFFLINE_ROUTE_PERIOD_BYTES,
            crate::recording::OFFLINE_ROUTE_PERIODS,
            || {
                attempts += 1;
                Ok(())
            },
        )
        .expect("gate");
    assert_eq!(attempts, 0);
    assert!(!gate.stream_opened);
    assert_eq!(gate.outbound_len, 0);
    assert!(gate.capture_stays_locked);
    assert!(crate::pod_audio::iso_buffers_if_known().is_err());
    assert!(crate::pod_audio::max_packet_size().is_err());

    let extension = Session::extension(ProductProfile::PodHd500x).expect("extension");
    let extension_gate = extension.gate_offline_recording(
        crate::pod_audio::RecordRoute::SendProcessedGuitar,
        crate::recording::OFFLINE_ROUTE_PERIOD_BYTES,
        crate::recording::OFFLINE_ROUTE_PERIODS,
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        extension_gate,
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::SelectRecordRoute,
        })
    );
    assert_eq!(
        session
            .capability(RoleKind::Guitar, Operation::SelectRecordRoute)
            .expect("guitar")
            .state,
        CapabilityState::Unsupported
    );
    let locked = session.reject_physical(
        RoleKind::Pod,
        Operation::StartAudioCapture,
        Some(CapabilityState::Offline),
        || {
            attempts += 1;
            Ok(())
        },
    );
    assert_eq!(attempts, 0);
    assert_eq!(
        locked,
        Err(Rejection::Locked {
            operation: Operation::StartAudioCapture,
        })
    );
    assert_eq!(
        session.gate_offline_recording(
            crate::pod_audio::RecordRoute::SendCleanGuitar,
            64,
            1,
            || {
                attempts += 1;
                Ok(())
            },
        ),
        Err(Rejection::Format {
            detail: "period byte count is not a multiple of the 6-byte frame".into(),
        })
    );
    assert_eq!(attempts, 0);
}

#[test]
fn pod_hd500x_identity_stays_on_its_profile_and_does_not_open_other_families() {
    assert!(LIVE_BACKEND_INITIALIZER.is_none());
    let session = Session::initial_rig();
    assert!(session.live_backend_is_absent());
    assert_eq!(
        session.pod_hd500x(),
        Err(crate::pod_hd500x::ProfileMiss::InitialRig)
    );
    let host = session.host().expect("initial host");
    assert_eq!(host.product_id, 0x4650);
    let hd = Session::extension(ProductProfile::PodHd500x).expect("hd500x");
    assert!(hd.live_backend_is_absent());
    assert!(hd.host().is_none());
    let record = hd.pod_hd500x().expect("hd500x record");
    assert_ne!(host.product_id, record.usb_product_id);
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::PodxtLive.family()
    );
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::Jtv69.family()
    );
    assert_ne!(
        ProductProfile::PodHd500x.family(),
        ProductProfile::WorkbenchHd.family()
    );
    assert!(
        ProductProfile::PodHd500x
            .archived_firmware_product()
            .is_none()
    );
    for profile in [ProductProfile::WorkbenchHd, ProductProfile::Jtv69] {
        let other = Session::extension(profile).expect("other extension");
        assert_eq!(
            other.pod_hd500x(),
            Err(crate::pod_hd500x::ProfileMiss::DifferentExtension)
        );
        let note = capability::capability(profile, Operation::OpenPodPreset).note;
        assert!(!note.contains("4159"));
        assert!(!note.contains("48000"));
    }
    let open_note =
        capability::capability(ProductProfile::PodHd500x, Operation::OpenPodPreset).note;
    let route_note =
        capability::capability(ProductProfile::PodHd500x, Operation::SelectRecordRoute).note;
    let firmware_note =
        capability::capability(ProductProfile::PodHd500x, Operation::InspectFirmware).note;
    assert!(open_note.contains("H5EP/H5ES/H5EB framing"));
    assert!(open_note.contains("does not prove physical target compatibility"));
    assert!(route_note.contains("48000"));
    assert!(route_note.contains("EngineFormatLock"));
    assert!(firmware_note.contains("6260"));
    assert!(firmware_note.contains("1.04"));
    match hd
        .extension_version(Component::ArchivedContainer)
        .expect("flash label")
    {
        VersionKnowledge::NotEstablished { evidence } => {
            assert!(evidence.contains("6260"));
            assert!(!VersionKnowledge::NotEstablished { evidence }.is_known_value());
        }
        other => panic!("flash label was {other:?}"),
    }
    match hd
        .extension_version(Component::InstalledUsbFirmware)
        .expect("usb label")
    {
        VersionKnowledge::NotEstablished { evidence } => assert!(evidence.contains("1.04")),
        other => panic!("usb label was {other:?}"),
    }
    let tone = crate::pod_preset::fixture(
        crate::pod_preset::ContainerKind::L6pa,
        PODXT_LIVE_PRODUCT_ID,
        1,
        &[(crate::pod_preset::AMP_SLOT, 1)],
    );
    let h5ep = synthetic_h5ep();
    assert!(matches!(
        crate::pod_preset::PodPresetFile::parse(h5ep.clone()),
        Ok(crate::pod_preset::PodPresetFile::H5ep(_))
    ));
    assert_eq!(
        Session::initial_rig().open_pod_preset(RoleKind::Pod, &h5ep),
        Err(Rejection::Format {
            detail: "expected an individual FORM tone; banks and collections are not POD presets"
                .into(),
        })
    );
    assert_eq!(
        Session::initial_rig().open_pod_preset(RoleKind::Guitar, &h5ep),
        Err(Rejection::Unsupported {
            profile: ProductProfile::Variax600,
            operation: Operation::OpenPodPreset,
        })
    );
    for profile in [
        ProductProfile::PodHd500x,
        ProductProfile::WorkbenchHd,
        ProductProfile::Jtv69,
    ] {
        assert_eq!(
            Session::extension(profile)
                .unwrap()
                .open_pod_preset(RoleKind::Pod, &h5ep),
            Err(Rejection::Unverified {
                profile,
                operation: Operation::OpenPodPreset,
            })
        );
    }
    assert_eq!(
        hd.open_pod_preset(RoleKind::Pod, &tone),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::OpenPodPreset,
        })
    );
    assert_eq!(
        hd.open_pod_preset(RoleKind::Pod, PRESET),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::OpenPodPreset,
        })
    );
    assert_eq!(
        hd.select_record_route(crate::pod_audio::RecordRoute::SendCleanGuitar, 66, 2),
        Err(Rejection::Unverified {
            profile: ProductProfile::PodHd500x,
            operation: Operation::SelectRecordRoute,
        })
    );
    let pod_image = synthetic_container(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, b"pod-image");
    let variax_image = synthetic_container(
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
        b"variax-image",
    );
    for image in [&pod_image, &variax_image] {
        let inspection = hd
            .inspect_extension_firmware(image)
            .expect("generic file inspection is not product application");
        assert_eq!(
            inspection.physical_target,
            PhysicalTargetApplicability::Unknown
        );
    }
    let mut attempts = 0;
    for operation in Operation::ALL.into_iter().filter(|op| op.is_physical()) {
        assert_eq!(
            hd.reject_extension_physical(operation, Some(CapabilityState::Offline), || {
                attempts += 1;
                Ok(())
            }),
            Err(Rejection::Locked { operation })
        );
    }
    assert_eq!(attempts, 0);
    assert_eq!(
        capability::capability(ProductProfile::PodHd500x, Operation::SelectRecordRoute).state,
        CapabilityState::Unverified
    );
    assert_eq!(
        capability::capability(ProductProfile::PodHd500x, Operation::InspectFirmware).state,
        CapabilityState::Offline
    );
    assert_eq!(
        capability::resolve(
            ProductProfile::PodHd500x,
            Operation::OpenPodPreset,
            Some(CapabilityState::Offline)
        ),
        CapabilityState::Unverified
    );
}
