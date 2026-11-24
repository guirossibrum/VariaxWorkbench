use super::*;
use crate::pod_preset::PodPresetFile;
use crate::pod_preset::TONE_BYTES_CEILING;
use crate::shared::file::ContinuousReader;
use std::cell::Cell;
use std::io::Cursor;
use std::rc::Rc;

fn synthetic_h5ep(extra: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; H5EP_MIN_BYTES];
    bytes[..4].copy_from_slice(b"H5EP");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    let body = &mut bytes[H5EP_HEADER_BYTES..];
    for offset in [0x228, 0x328] {
        body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
    }
    for index in 0..HD_EFFECT_COUNT {
        let offset = HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE;
        body[offset..offset + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
    }
    bytes.extend_from_slice(extra);
    bytes
}

fn synthetic_octave_fuzz_h5ep(effect_slot: usize, with_tail: bool) -> Vec<u8> {
    let mut bytes = synthetic_h5ep(if with_tail { b"private-tail" } else { b"" });
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
    bytes[block..block + 4].copy_from_slice(&HD_OCTAVE_FUZZ_MODEL_ID.to_be_bytes());
    bytes[block + 4..block + 8].copy_from_slice(&0x1122_3344u32.to_be_bytes());
    bytes[block + 8] = (effect_slot % 2) as u8;
    bytes[block + 9] = 0x5a;
    bytes[block + 10] = 0xa5;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 8;

    let permutation = [
        HD_TYPE1_OUTPUT_KEY,
        HD_TYPE1_DRIVE_KEY,
        HD_TYPE1_THIRD_KEY,
        HD_TYPE1_BASS_KEY,
        HD_TYPE1_TREBLE_KEY,
    ];
    for row in 0..HD_PARAMETER_ROW_COUNT {
        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
        let key = if row < permutation.len() {
            permutation[row]
        } else if row == 6 {
            HD_TYPE1_OUTPUT_KEY
        } else {
            0x6abc_0000 + row as u32
        };
        bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&(0x3e80_0000u32 + row as u32).to_be_bytes());
        bytes[row_at + 8..row_at + 12]
            .copy_from_slice(&(0x7100_0000u32 + row as u32).to_be_bytes());
        bytes[row_at + 12..row_at + 16]
            .copy_from_slice(&(0x7200_0000u32 + row as u32).to_be_bytes());
        bytes[row_at + 0x10] = (row % 3) as u8;
        bytes[row_at + 0x11..row_at + 0x14].copy_from_slice(&[
            0xc1 + row as u8,
            0xd1 + row as u8,
            0xe1 + row as u8,
        ]);
    }
    bytes
}

fn five_key_permutations() -> Vec<[u32; 5]> {
    fn visit(values: &mut [u32; 5], start: usize, output: &mut Vec<[u32; 5]>) {
        if start == values.len() {
            output.push(*values);
            return;
        }
        for index in start..values.len() {
            values.swap(start, index);
            visit(values, start + 1, output);
            values.swap(start, index);
        }
    }
    let mut values = HD_SCREAMER_DEFAULT_KEYS;
    let mut output = Vec::with_capacity(120);
    visit(&mut values, 0, &mut output);
    output
}

fn synthetic_type9_h5ep(slot: usize, row: usize, enabled: u8, value: u32) -> Vec<u8> {
    synthetic_type9_h5ep_for(
        slot,
        row,
        enabled,
        value,
        HD_TYPE9_CHOICE_MODEL_ID,
        0x3f10_0004,
    )
}

fn synthetic_type9_h5ep_for(
    slot: usize,
    row: usize,
    enabled: u8,
    value: u32,
    model_id: u32,
    key: u32,
) -> Vec<u8> {
    let mut bytes = synthetic_h5ep(b"type9-tail");
    bytes[8..H5EP_HEADER_BYTES].fill(0x6d);
    bytes[H5EP_HEADER_BYTES + 0x040..H5EP_HEADER_BYTES + 0x050].fill(0x5a);
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
    bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
    bytes[block + 8] = enabled;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 12;
    let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
    bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
    bytes[row_at + 4..row_at + 8].copy_from_slice(&value.to_be_bytes());
    bytes[row_at + 8..row_at + 0x10].copy_from_slice(&[0x31; 8]);
    bytes[row_at + 0x10..row_at + HD_PARAMETER_ROW_STRIDE].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
    for other_slot in 0..HD_EFFECT_COUNT {
        if other_slot != slot {
            let other =
                H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + other_slot * HD_EFFECT_BLOCK_STRIDE;
            bytes[other + 4..other + 8].fill(0x73);
        }
    }
    for other_row in 0..HD_PARAMETER_ROW_COUNT {
        if other_row != row {
            let other = block + HD_EFFECT_ROWS_OFFSET + other_row * HD_PARAMETER_ROW_STRIDE;
            bytes[other + 8..other + 0x10].fill(0x24 + other_row as u8);
            bytes[other + 0x11..other + HD_PARAMETER_ROW_STRIDE].fill(0x44 + other_row as u8);
        }
    }
    bytes
}

fn synthetic_type8_h5ep(slot: usize, row: usize, enabled: u8, value: u32) -> Vec<u8> {
    let mut bytes = synthetic_h5ep(b"type8-tail");
    // Keep this procedural matrix builder independent of the type-9
    // fixture builder; the fixed full-file vectors below are the oracle.
    let block = H5EP_HEADER_BYTES + 0x428 + slot * 0x100;
    bytes[block..block + 4].copy_from_slice(&HD_TYPE8_CHOICE_MODEL_ID.to_be_bytes());
    bytes[block + 8] = enabled;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 12;
    let row_at = block + 0x10 + row * 0x14;
    bytes[row_at..row_at + 4].copy_from_slice(&HD_TYPE8_CHOICE_KEY.to_be_bytes());
    bytes[row_at + 4..row_at + 8].copy_from_slice(&value.to_be_bytes());
    bytes[row_at + 8..row_at + 0x10].copy_from_slice(&[0x31; 8]);
    bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
    bytes
}

fn synthetic_type10_h5ep_for(
    slot: usize,
    row: usize,
    enabled: u8,
    value: u32,
    model_id: u32,
    key: u32,
) -> Vec<u8> {
    let mut bytes = synthetic_h5ep(b"type10-tail");
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
    bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
    bytes[block + 8] = enabled;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 12;
    let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
    bytes[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
    bytes[row_at + 4..row_at + 8].copy_from_slice(&value.to_be_bytes());
    bytes[row_at + 8..row_at + 0x10].fill(0x35);
    bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x42, 0x43, 0x44]);
    bytes
}

fn synthetic_type10_h5ep(slot: usize, row: usize, enabled: u8, value: u32) -> Vec<u8> {
    synthetic_type10_h5ep_for(
        slot,
        row,
        enabled,
        value,
        HD_TYPE10_CHOICE_MODEL_ID,
        HD_TYPE10_CHOICE_KEY,
    )
}

fn synthetic_type11_h5ep(
    slot: usize,
    target_row: usize,
    enabled: u8,
    model_id: u32,
    control: HdType11Control,
    value: u32,
) -> Vec<u8> {
    assert_eq!(control.model_id(), model_id);
    let mut bytes = synthetic_h5ep(b"type11-tail");
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
    bytes[block..block + 4].copy_from_slice(&model_id.to_be_bytes());
    bytes[block + 8] = enabled;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 12;
    for row in 0..HD_PARAMETER_ROW_COUNT {
        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
        bytes[row_at..row_at + 4].copy_from_slice(&(0x5500_0000 + row as u32).to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&(0x3e00_0000 + row as u32).to_be_bytes());
        bytes[row_at + 8..row_at + 0x10].fill(0x61 + row as u8);
        bytes[row_at + 0x10..row_at + 0x14].copy_from_slice(&[0, 0x71, 0x72, 0x73]);
    }
    let controls = HdType11Control::for_model(model_id);
    assert_eq!(controls.len(), 2);
    let sibling = controls
        .iter()
        .copied()
        .find(|candidate| *candidate != control)
        .unwrap();
    let sibling_row = if target_row == 11 { 0 } else { 11 };
    for (row, selected_control, bits) in [
        (target_row, control, value),
        (sibling_row, sibling, 0x3eaa_1234),
    ] {
        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
        bytes[row_at..row_at + 4].copy_from_slice(&selected_control.key().to_be_bytes());
        bytes[row_at + 4..row_at + 8].copy_from_slice(&bits.to_be_bytes());
    }
    bytes
}

#[test]
fn type11_choices_are_closed_typed_and_preserve_every_non_target_byte() {
    assert_eq!(HD_TYPE11_CHOICE_MODEL_KEYS.len(), 4);
    assert_eq!(HdType11Control::ALL.len(), 4);
    assert_eq!(HdType11Choice::ALL.len(), 5);
    for (index, choice) in HdType11Choice::ALL.into_iter().enumerate() {
        assert_eq!(HdType11Choice::from_index(index as u8), Ok(choice));
        assert_eq!(choice.index(), index as u8);
    }
    assert!(HdType11Choice::from_index(5).is_err());
    assert_eq!(
        HdType11Choice::ALL.map(HdType11Choice::bits),
        [
            0x0000_0000,
            0x3e80_0000,
            0x3f00_0000,
            0x3f40_0000,
            0x3f80_0000
        ]
    );
    for unknown in [0x020a_001c, 0x020a_001f, 0x020a_0020] {
        assert!(
            HdType11Control::ALL
                .iter()
                .all(|control| control.model_id() != unknown)
        );
    }

    for control in HdType11Control::ALL {
        for slot_index in 0..HD_EFFECT_COUNT {
            for target_row in [0, 5, 11] {
                for enabled in [0, 1] {
                    for choice in HdType11Choice::ALL {
                        let original = synthetic_type11_h5ep(
                            slot_index,
                            target_row,
                            enabled,
                            control.model_id(),
                            control,
                            0x3eaa_4321,
                        );
                        let value_at = H5EP_HEADER_BYTES
                            + HD_EFFECT_BLOCK_OFFSET
                            + slot_index * HD_EFFECT_BLOCK_STRIDE
                            + HD_EFFECT_ROWS_OFFSET
                            + target_row * HD_PARAMETER_ROW_STRIDE
                            + 4;
                        let mut expected = original.clone();
                        expected[value_at..value_at + 4]
                            .copy_from_slice(&choice.bits().to_be_bytes());
                        let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
                        let slot = HdFxSlot::new(slot_index).unwrap();
                        assert_eq!(
                            preset.hd_type11_choice_value(slot, control),
                            HdType11ChoiceValue::UnknownFinite(0x3eaa_4321)
                        );
                        preset.set_hd_type11_choice(slot, control, choice).unwrap();
                        assert_eq!(preset.as_bytes(), expected);
                        let same_word = preset.as_bytes().to_vec();
                        preset.set_hd_type11_choice(slot, control, choice).unwrap();
                        assert_eq!(preset.as_bytes(), same_word, "guarded same-word no-op");
                        assert_eq!(
                            preset.hd_type11_choice_value(slot, control),
                            HdType11ChoiceValue::Choice(choice)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn type11_strict_guards_precede_noop_and_reject_without_mutation() {
    for control in HdType11Control::ALL {
        let slot = HdFxSlot::new(5).unwrap();
        let source = synthetic_type11_h5ep(
            5,
            4,
            1,
            control.model_id(),
            control,
            HdType11Choice::Choice2.bits(),
        );
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
        let row_at = block + HD_EFFECT_ROWS_OFFSET + 4 * HD_PARAMETER_ROW_STRIDE;
        let sibling = HdType11Control::for_model(control.model_id())
            .iter()
            .copied()
            .find(|candidate| *candidate != control)
            .unwrap();
        let mut cases = Vec::new();
        let mut foreign_model = source.clone();
        foreign_model[block..block + 4].copy_from_slice(&0x020a_001c_u32.to_be_bytes());
        cases.push(("foreign model", foreign_model));
        let mut foreign_key = source.clone();
        foreign_key[row_at..row_at + 4].copy_from_slice(&sibling.key().to_be_bytes());
        cases.push(("wrong control key", foreign_key));
        let mut missing = source.clone();
        missing[row_at..row_at + 4].fill(0);
        cases.push(("missing target key", missing));
        let mut duplicate = source.clone();
        let late_row = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
        duplicate[late_row..late_row + 4].copy_from_slice(&control.key().to_be_bytes());
        duplicate[block + 0x0f] = 5;
        cases.push(("duplicate beyond declared count", duplicate));
        let mut beyond = source.clone();
        beyond[row_at..row_at + 4].fill(0);
        let row11 = block + HD_EFFECT_ROWS_OFFSET + 11 * HD_PARAMETER_ROW_STRIDE;
        beyond[row11..row11 + 4].copy_from_slice(&control.key().to_be_bytes());
        beyond[block + 0x0f] = 11;
        cases.push(("target beyond declared count", beyond));
        let mut assigned = source.clone();
        assigned[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        cases.push(("assigned target marker", assigned));
        for (offset, value, label) in [
            (0x0b, 0, "invalid effect marker"),
            (8, 2, "noncanonical enable"),
            (0x0f, 0, "zero row count"),
            (0x0f, 13, "excessive row count"),
        ] {
            let mut invalid = source.clone();
            invalid[block + offset] = value;
            cases.push((label, invalid));
        }
        for bits in [0x7fc0_0001_u32, 0x7f80_0000, 0xff80_0000] {
            let mut nonfinite = source.clone();
            nonfinite[row_at + 4..row_at + 8].copy_from_slice(&bits.to_be_bytes());
            cases.push(("non-finite current value", nonfinite));
        }
        for (label, bytes) in cases {
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            assert!(
                preset
                    .set_hd_type11_choice(slot, control, HdType11Choice::Choice2)
                    .is_err(),
                "{label}"
            );
            assert_eq!(preset.as_bytes(), bytes, "{label} must be atomic");
        }
        let mut malformed_noop = source.clone();
        malformed_noop[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        let mut preset = OpaqueH5epPreset::parse(malformed_noop.clone()).unwrap();
        assert!(
            preset
                .set_hd_type11_choice(slot, control, HdType11Choice::Choice2)
                .is_err()
        );
        assert_eq!(
            preset.as_bytes(),
            malformed_noop,
            "guards run before same-word no-op"
        );
    }
}

fn synthetic_type12_h5ep(slot: usize, target_row: usize, enabled: u8, value: u32) -> Vec<u8> {
    let mut bytes = synthetic_h5ep(b"type12-opaque-suffix");
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
    bytes[block..block + 4].copy_from_slice(&HD_TYPE12_CHOICE_MODEL_ID.to_be_bytes());
    bytes[block + 8] = enabled;
    bytes[block + 0x0b] = 1;
    bytes[block + 0x0f] = 12;
    for row in 0..HD_PARAMETER_ROW_COUNT {
        let at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
        bytes[at..at + 4].copy_from_slice(&(0x5500_0000 + row as u32).to_be_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&(0x3e00_0000 + row as u32).to_be_bytes());
        bytes[at + 8..at + 0x10].fill(0x51 + row as u8);
        bytes[at + 0x10..at + 0x14].copy_from_slice(&[0, 0x71, 0x72, 0x73]);
    }
    let row = block + HD_EFFECT_ROWS_OFFSET + target_row * HD_PARAMETER_ROW_STRIDE;
    bytes[row..row + 4].copy_from_slice(&HD_TYPE12_CHOICE_KEY.to_be_bytes());
    bytes[row + 4..row + 8].copy_from_slice(&value.to_be_bytes());
    bytes
}

#[test]
fn type12_choice_is_bounded_and_patches_only_the_value_word() {
    assert_eq!(HdType12Control::ALL.len(), 1);
    assert_eq!(HdType12Choice::ALL.len(), 2);
    assert_eq!(
        HdType12Choice::ALL.map(HdType12Choice::bits),
        [0, 0x3f80_0000]
    );
    assert_eq!(
        HdType12Control::for_model(HD_TYPE12_CHOICE_MODEL_ID),
        &HdType12Control::ALL
    );
    for (index, choice) in HdType12Choice::ALL.into_iter().enumerate() {
        assert_eq!(HdType12Choice::from_index(index as u8), Ok(choice));
    }
    assert!(HdType12Choice::from_index(2).is_err());
    let control = HdType12Control::Model020a001eKey3f100004;
    for slot_index in 0..HD_EFFECT_COUNT {
        for row in [0, 5, 11] {
            for enabled in [0, 1] {
                for choice in HdType12Choice::ALL {
                    let original = synthetic_type12_h5ep(slot_index, row, enabled, 0x3eaa_4321);
                    let value_at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row * HD_PARAMETER_ROW_STRIDE
                        + 4;
                    let mut expected = original.clone();
                    expected[value_at..value_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
                    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                    let slot = HdFxSlot::new(slot_index).unwrap();
                    assert_eq!(
                        preset.hd_type12_choice_value(slot, control),
                        HdType12ChoiceValue::UnknownFinite(0x3eaa_4321)
                    );
                    preset.set_hd_type12_choice(slot, control, choice).unwrap();
                    assert_eq!(preset.as_bytes(), expected);
                    let same_word = preset.as_bytes().to_vec();
                    preset.set_hd_type12_choice(slot, control, choice).unwrap();
                    assert_eq!(preset.as_bytes(), same_word, "validated no-op");
                }
            }
        }
    }
}

#[test]
fn type12_strict_guards_precede_noop_and_reject_atomically() {
    let control = HdType12Control::Model020a001eKey3f100004;
    let slot = HdFxSlot::new(5).unwrap();
    let source = synthetic_type12_h5ep(5, 4, 1, HdType12Choice::Choice1.bits());
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
    let row = block + HD_EFFECT_ROWS_OFFSET + 4 * HD_PARAMETER_ROW_STRIDE;
    let mut cases = Vec::new();
    let mut foreign = source.clone();
    foreign[block..block + 4].copy_from_slice(&0x020a001d_u32.to_be_bytes());
    cases.push(("foreign model", foreign));
    let mut missing = source.clone();
    missing[row..row + 4].fill(0);
    cases.push(("missing key", missing));
    let mut duplicate = source.clone();
    let late = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
    duplicate[late..late + 4].copy_from_slice(&HD_TYPE12_CHOICE_KEY.to_be_bytes());
    duplicate[block + 0x0f] = 5;
    cases.push(("trailing duplicate", duplicate));
    let mut beyond = source.clone();
    beyond[row..row + 4].fill(0);
    let row11 = block + HD_EFFECT_ROWS_OFFSET + 11 * HD_PARAMETER_ROW_STRIDE;
    beyond[row11..row11 + 4].copy_from_slice(&HD_TYPE12_CHOICE_KEY.to_be_bytes());
    beyond[block + 0x0f] = 11;
    cases.push(("out of count", beyond));
    let mut assigned = source.clone();
    assigned[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    cases.push(("assigned", assigned));
    for (offset, value, label) in [
        (0x0b, 0, "invalid effect"),
        (8, 2, "enable"),
        (0x0f, 0, "zero count"),
        (0x0f, 13, "excess count"),
    ] {
        let mut bytes = source.clone();
        bytes[block + offset] = value;
        cases.push((label, bytes));
    }
    for bits in [0x7fc0_0001_u32, 0x7f80_0000, 0xff80_0000] {
        let mut bytes = source.clone();
        bytes[row + 4..row + 8].copy_from_slice(&bits.to_be_bytes());
        cases.push(("nonfinite", bytes));
    }
    for (label, bytes) in cases {
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert!(
            preset
                .set_hd_type12_choice(slot, control, HdType12Choice::Choice1)
                .is_err(),
            "{label}"
        );
        assert_eq!(preset.as_bytes(), bytes, "{label}");
    }
    let mut invalid_noop = source.clone();
    invalid_noop[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    let mut preset = OpaqueH5epPreset::parse(invalid_noop.clone()).unwrap();
    assert!(
        preset
            .set_hd_type12_choice(slot, control, HdType12Choice::Choice1)
            .is_err()
    );
    assert_eq!(preset.as_bytes(), invalid_noop, "guards precede no-op");
    let nonfinite = synthetic_type12_h5ep(5, 4, 1, 0x7fc0_0011);
    let parsed = OpaqueH5epPreset::parse(nonfinite).unwrap();
    assert_eq!(
        parsed.hd_type12_choice_value(slot, control),
        HdType12ChoiceValue::NonFinite(0x7fc0_0011)
    );
}

#[test]
fn type12_static_complete_vectors_are_exactly_the_two_value_words() {
    let source = include_bytes!("../../../../tests/fixtures/type12-source.h5ep");
    let outputs = [
        include_bytes!("../../../../tests/fixtures/type12-choice0.h5ep"),
        include_bytes!("../../../../tests/fixtures/type12-choice1.h5ep"),
    ];
    let control = HdType12Control::Model020a001eKey3f100004;
    let preset = OpaqueH5epPreset::parse(source.to_vec()).unwrap();
    let slot = HdFxSlot::new(4).unwrap();
    assert_eq!(
        preset.hd_type12_choice_value(slot, control),
        HdType12ChoiceValue::UnknownFinite(0x3eaaaaaa)
    );
    for (choice, expected) in HdType12Choice::ALL.into_iter().zip(outputs) {
        let mut edited = OpaqueH5epPreset::parse(source.to_vec()).unwrap();
        edited.set_hd_type12_choice(slot, control, choice).unwrap();
        assert_eq!(edited.as_bytes(), expected);
    }
}

#[test]
fn type13_static_complete_vectors_match_rounded_editor_words_and_preserve_siblings() {
    let source = include_bytes!("../../../../tests/fixtures/type13-model-020a001d-source.h5ep");
    let outputs = [
        include_bytes!("../../../../tests/fixtures/type13-model-020a001d-choice0.h5ep"),
        include_bytes!("../../../../tests/fixtures/type13-model-020a001d-choice1.h5ep"),
        include_bytes!("../../../../tests/fixtures/type13-model-020a001d-choice2.h5ep"),
        include_bytes!("../../../../tests/fixtures/type13-model-020a001d-choice3.h5ep"),
    ];
    assert_eq!(
        HdType13Choice::ALL.map(HdType13Choice::bits),
        [0, 0x3eaa_aaab, 0x3f2a_aaab, 0x3f80_0000]
    );
    assert!(HdType13Choice::from_index(4).is_err());
    let control = HdType13Control::Model020a001dKey3f100003;
    assert_eq!(
        HdType13Control::for_model(0x020a_001d),
        &HdType13Control::ALL
    );
    assert!(HdType13Control::for_model(0x020a_0011).is_empty());
    let slot = HdFxSlot::new(4).unwrap();
    let preset = OpaqueH5epPreset::parse(source.to_vec()).unwrap();
    let target_value = H5EP_HEADER_BYTES
        + HD_EFFECT_BLOCK_OFFSET
        + slot.index() * HD_EFFECT_BLOCK_STRIDE
        + HD_EFFECT_ROWS_OFFSET
        + 9 * HD_PARAMETER_ROW_STRIDE
        + 4;
    assert_eq!(
        preset.hd_type13_choice_value(slot, control),
        HdType13ChoiceValue::UnknownFinite(0x3eaa_aaaa)
    );
    for (choice, expected) in HdType13Choice::ALL.into_iter().zip(outputs) {
        assert_eq!(expected.len(), source.len());
        for offset in 0..source.len() {
            if !(target_value..target_value + 4).contains(&offset) {
                assert_eq!(
                    expected[offset], source[offset],
                    "expected output changed byte {offset:#x}"
                );
            }
        }
        assert_eq!(
            &expected[target_value..target_value + 4],
            &choice.bits().to_be_bytes()
        );
        let mut edited = OpaqueH5epPreset::parse(source.to_vec()).unwrap();
        edited.set_hd_type13_choice(slot, control, choice).unwrap();
        assert_eq!(edited.as_bytes(), expected);
        let no_op = edited.as_bytes().to_vec();
        edited.set_hd_type13_choice(slot, control, choice).unwrap();
        assert_eq!(edited.as_bytes(), no_op, "guarded same-word no-op");
    }
    // Every slot, representative row positions, and both enable bytes;
    // preserve exactly the requested value word and opaque surroundings.
    for slot_index in 0..HD_EFFECT_COUNT {
        for row in [0, 5, 11] {
            for enabled in [0, 1] {
                for choice in HdType13Choice::ALL {
                    let mut original = synthetic_type12_h5ep(slot_index, row, enabled, 0x3e12_3456);
                    let block = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE;
                    original[block..block + 4]
                        .copy_from_slice(&HD_TYPE13_CHOICE_MODEL_ID.to_be_bytes());
                    let at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                    original[at..at + 4].copy_from_slice(&HD_TYPE13_CHOICE_KEY.to_be_bytes());
                    let value_at = at + 4;
                    let mut expected = original.clone();
                    expected[value_at..value_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
                    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                    preset
                        .set_hd_type13_choice(HdFxSlot::new(slot_index).unwrap(), control, choice)
                        .unwrap();
                    assert_eq!(preset.as_bytes(), expected);
                }
            }
        }
    }
}

#[test]
fn type13_guards_precede_noop_and_preserve_nonfinite_and_unknown_words() {
    let source =
        include_bytes!("../../../../tests/fixtures/type13-model-020a001d-source.h5ep").to_vec();
    let slot = HdFxSlot::new(4).unwrap();
    let control = HdType13Control::Model020a001dKey3f100003;
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 4 * HD_EFFECT_BLOCK_STRIDE;
    let row = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
    let mut cases = Vec::new();
    let mut foreign = source.clone();
    foreign[block..block + 4].copy_from_slice(&0x020a001e_u32.to_be_bytes());
    cases.push(("foreign model", foreign));
    let mut missing = source.clone();
    missing[row..row + 4].fill(0);
    cases.push(("missing key", missing));
    let mut duplicate = source.clone();
    let late = block + HD_EFFECT_ROWS_OFFSET + 11 * HD_PARAMETER_ROW_STRIDE;
    duplicate[late..late + 4].copy_from_slice(&HD_TYPE13_CHOICE_KEY.to_be_bytes());
    duplicate[block + 0x0f] = 10;
    assert!(
        9 < duplicate[block + 0x0f] as usize,
        "target row 9 remains in count"
    );
    assert!(
        11 >= duplicate[block + 0x0f] as usize,
        "duplicate row 11 is outside count"
    );
    cases.push(("trailing duplicate beyond count", duplicate));
    let mut beyond = source.clone();
    beyond[block + 0x0f] = 9;
    cases.push(("out of count", beyond));
    let mut assigned = source.clone();
    assigned[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    cases.push(("assigned", assigned));
    for (offset, value, label) in [
        (0x0b, 0, "invalid effect"),
        (8, 2, "enable"),
        (0x0f, 0, "zero count"),
        (0x0f, 13, "excess count"),
    ] {
        let mut b = source.clone();
        b[block + offset] = value;
        cases.push((label, b));
    }
    for bits in [0x7fc0_0011_u32, 0x7f80_0000, 0xff80_0000] {
        let mut b = source.clone();
        b[row + 4..row + 8].copy_from_slice(&bits.to_be_bytes());
        cases.push(("nonfinite", b));
    }
    for (label, bytes) in cases {
        let mut p = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert!(
            p.set_hd_type13_choice(slot, control, HdType13Choice::Choice1)
                .is_err(),
            "{label}"
        );
        assert_eq!(p.as_bytes(), bytes, "{label}");
    }
    let nonfinite = {
        let mut b = source.clone();
        b[row + 4..row + 8].copy_from_slice(&0x7fc0_0011_u32.to_be_bytes());
        b
    };
    assert_eq!(
        OpaqueH5epPreset::parse(nonfinite)
            .unwrap()
            .hd_type13_choice_value(slot, control),
        HdType13ChoiceValue::NonFinite(0x7fc0_0011)
    );
    let mut invalid_noop = source.clone();
    invalid_noop[row + 4..row + 8].copy_from_slice(&HdType13Choice::Choice2.bits().to_be_bytes());
    invalid_noop[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    assert_eq!(
        &invalid_noop[row + 4..row + 8],
        &HdType13Choice::Choice2.bits().to_be_bytes(),
        "requested choice is already present before the guard is corrupted"
    );
    let mut p = OpaqueH5epPreset::parse(invalid_noop.clone()).unwrap();
    assert!(
        p.set_hd_type13_choice(slot, control, HdType13Choice::Choice2)
            .is_err()
    );
    assert_eq!(p.as_bytes(), invalid_noop);
}

#[test]
fn type19_static_complete_vectors_match_exact_words_and_preserve_siblings() {
    let source = include_bytes!("../../../../tests/fixtures/type19-model-020a0011-source.h5ep");
    let outputs = [
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice0.h5ep"),
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice1.h5ep"),
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice2.h5ep"),
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice3.h5ep"),
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice4.h5ep"),
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice5.h5ep"),
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice6.h5ep"),
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-choice7.h5ep"),
    ];
    assert_eq!(
        HdType19Choice::ALL.map(HdType19Choice::bits),
        [
            0,
            0x3e00_0000,
            0x3e80_0000,
            0x3ec0_0000,
            0x3f00_0000,
            0x3f20_0000,
            0x3f40_0000,
            0x3f60_0000
        ]
    );
    assert!(HdType19Choice::from_index(8).is_err());
    let control = HdType19Control::Model020a0011Key3f100003;
    assert_eq!(
        HdType19Control::for_model(0x020a_0011),
        &HdType19Control::ALL
    );
    assert!(HdType19Control::for_model(0x020a_001d).is_empty());
    let slot = HdFxSlot::new(4).unwrap();
    let preset = OpaqueH5epPreset::parse(source.to_vec()).unwrap();
    let target_value = H5EP_HEADER_BYTES
        + HD_EFFECT_BLOCK_OFFSET
        + slot.index() * HD_EFFECT_BLOCK_STRIDE
        + HD_EFFECT_ROWS_OFFSET
        + 9 * HD_PARAMETER_ROW_STRIDE
        + 4;
    assert_eq!(
        preset.hd_type19_choice_value(slot, control),
        HdType19ChoiceValue::UnknownFinite(0x3e12_3456)
    );
    for (choice, expected) in HdType19Choice::ALL.into_iter().zip(outputs) {
        assert_eq!(expected.len(), source.len());
        for offset in 0..source.len() {
            if !(target_value..target_value + 4).contains(&offset) {
                assert_eq!(
                    expected[offset], source[offset],
                    "expected output changed byte {offset:#x}"
                );
            }
        }
        assert_eq!(
            &expected[target_value..target_value + 4],
            &choice.bits().to_be_bytes()
        );
        let mut edited = OpaqueH5epPreset::parse(source.to_vec()).unwrap();
        edited.set_hd_type19_choice(slot, control, choice).unwrap();
        assert_eq!(edited.as_bytes(), expected);
        let no_op = edited.as_bytes().to_vec();
        edited.set_hd_type19_choice(slot, control, choice).unwrap();
        assert_eq!(edited.as_bytes(), no_op, "guarded same-word no-op");
    }
    // Every slot, representative row positions, and both enable bytes;
    // preserve exactly the requested value word and opaque surroundings.
    for slot_index in 0..HD_EFFECT_COUNT {
        for row in [0, 5, 11] {
            for enabled in [0, 1] {
                for choice in HdType19Choice::ALL {
                    let mut original = synthetic_type12_h5ep(slot_index, row, enabled, 0x3e12_3456);
                    let block = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE;
                    original[block..block + 4]
                        .copy_from_slice(&HD_TYPE19_CHOICE_MODEL_ID.to_be_bytes());
                    let at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                    original[at..at + 4].copy_from_slice(&HD_TYPE19_CHOICE_KEY.to_be_bytes());
                    let value_at = at + 4;
                    let mut expected = original.clone();
                    expected[value_at..value_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
                    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                    preset
                        .set_hd_type19_choice(HdFxSlot::new(slot_index).unwrap(), control, choice)
                        .unwrap();
                    assert_eq!(preset.as_bytes(), expected);
                }
            }
        }
    }
}

#[test]
fn type19_guards_precede_noop_and_preserve_nonfinite_and_unknown_words() {
    let source =
        include_bytes!("../../../../tests/fixtures/type19-model-020a0011-source.h5ep").to_vec();
    let slot = HdFxSlot::new(4).unwrap();
    let control = HdType19Control::Model020a0011Key3f100003;
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 4 * HD_EFFECT_BLOCK_STRIDE;
    let row = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
    let mut cases = Vec::new();
    let mut foreign = source.clone();
    foreign[block..block + 4].copy_from_slice(&0x020a001e_u32.to_be_bytes());
    cases.push(("foreign model", foreign));
    let mut missing = source.clone();
    missing[row..row + 4].fill(0);
    cases.push(("missing key", missing));
    let mut duplicate = source.clone();
    let late = block + HD_EFFECT_ROWS_OFFSET + 11 * HD_PARAMETER_ROW_STRIDE;
    duplicate[late..late + 4].copy_from_slice(&HD_TYPE19_CHOICE_KEY.to_be_bytes());
    duplicate[block + 0x0f] = 10;
    assert!(
        9 < duplicate[block + 0x0f] as usize,
        "target row 9 remains in count"
    );
    assert!(
        11 >= duplicate[block + 0x0f] as usize,
        "duplicate row 11 is outside count"
    );
    cases.push(("trailing duplicate beyond count", duplicate));
    let mut beyond = source.clone();
    beyond[block + 0x0f] = 9;
    cases.push(("out of count", beyond));
    let mut assigned = source.clone();
    assigned[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    cases.push(("assigned", assigned));
    for (offset, value, label) in [
        (0x0b, 0, "invalid effect"),
        (8, 2, "enable"),
        (0x0f, 0, "zero count"),
        (0x0f, 13, "excess count"),
    ] {
        let mut b = source.clone();
        b[block + offset] = value;
        cases.push((label, b));
    }
    for bits in [0x7fc0_0011_u32, 0x7f80_0000, 0xff80_0000] {
        let mut b = source.clone();
        b[row + 4..row + 8].copy_from_slice(&bits.to_be_bytes());
        cases.push(("nonfinite", b));
    }
    for (label, bytes) in cases {
        let mut p = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert!(
            p.set_hd_type19_choice(slot, control, HdType19Choice::Choice1)
                .is_err(),
            "{label}"
        );
        assert_eq!(p.as_bytes(), bytes, "{label}");
    }
    let nonfinite = {
        let mut b = source.clone();
        b[row + 4..row + 8].copy_from_slice(&0x7fc0_0011_u32.to_be_bytes());
        b
    };
    assert_eq!(
        OpaqueH5epPreset::parse(nonfinite)
            .unwrap()
            .hd_type19_choice_value(slot, control),
        HdType19ChoiceValue::NonFinite(0x7fc0_0011)
    );
    let mut invalid_noop = source.clone();
    invalid_noop[row + 4..row + 8].copy_from_slice(&HdType19Choice::Choice2.bits().to_be_bytes());
    invalid_noop[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    assert_eq!(
        &invalid_noop[row + 4..row + 8],
        &HdType19Choice::Choice2.bits().to_be_bytes(),
        "requested choice is already present before the guard is corrupted"
    );
    let mut p = OpaqueH5epPreset::parse(invalid_noop.clone()).unwrap();
    assert!(
        p.set_hd_type19_choice(slot, control, HdType19Choice::Choice2)
            .is_err()
    );
    assert_eq!(p.as_bytes(), invalid_noop);
}

fn synthetic_enable_h5ep(effect_slot: usize, enabled_byte: u8) -> Vec<u8> {
    let mut bytes = synthetic_octave_fuzz_h5ep(effect_slot, true);
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
    bytes[block + 8] = enabled_byte;
    bytes
}

#[test]
fn admitted_enable_edits_one_byte_for_every_effect_slot() {
    for effect_slot in 0..HD_EFFECT_COUNT {
        for requested in [false, true] {
            let old = u8::from(!requested);
            let original = synthetic_enable_h5ep(effect_slot, old);
            let mut expected = original.clone();
            let enable_at = H5EP_HEADER_BYTES
                + HD_EFFECT_BLOCK_OFFSET
                + effect_slot * HD_EFFECT_BLOCK_STRIDE
                + 8;
            expected[enable_at] = u8::from(requested);

            let mut preset = OpaqueH5epPreset::parse(original).unwrap();
            let slot = HdFxSlot::new(effect_slot).unwrap();
            assert_eq!(preset.effect_enabled(slot), Some(!requested));
            preset.set_effect_enabled(slot, requested).unwrap();
            assert_eq!(
                preset.as_bytes(),
                expected,
                "slot {effect_slot}, {requested}"
            );
            assert_eq!(preset.effect_enabled(slot), Some(requested));
        }
    }
    assert!(HdFxSlot::new(HD_EFFECT_COUNT).is_err());
}

#[test]
fn admitted_enable_noops_preserve_the_whole_file_and_noncanonical_writes_reject() {
    for raw in [0, 1] {
        let bytes = synthetic_enable_h5ep(3, raw);
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        preset
            .set_effect_enabled(HdFxSlot::new(3).unwrap(), raw != 0)
            .unwrap();
        assert_eq!(preset.as_bytes(), bytes, "canonical no-op for {raw:#04x}");
    }

    for raw in [0x02, 0x7f, 0x80, 0xff] {
        let bytes = synthetic_enable_h5ep(3, raw);
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert_eq!(preset.effect_enabled(HdFxSlot::new(3).unwrap()), Some(true));
        for requested in [false, true] {
            assert!(
                preset
                    .set_effect_enabled(HdFxSlot::new(3).unwrap(), requested)
                    .is_err(),
                "raw {raw:#04x}, requested {requested}"
            );
            assert_eq!(preset.as_bytes(), bytes, "raw {raw:#04x}");
        }
    }
}

#[test]
fn admitted_enable_rejects_unknown_empty_invalid_and_unsupported_records_atomically() {
    let slot = HdFxSlot::new(0).unwrap();
    for (model, valid) in [
        (0x02ad_beef, 1),
        (HD_EMPTY_EFFECT_MODEL_ID, 1),
        (HD_SCREAMER_MODEL_ID, 0),
        (HD_SCREAMER_MODEL_ID, 2),
    ] {
        let mut bytes = synthetic_enable_h5ep(0, 0);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
        bytes[block..block + 4].copy_from_slice(&model.to_be_bytes());
        bytes[block + 0x0b] = valid;
        let original = bytes.clone();
        let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
        assert!(preset.set_effect_enabled(slot, true).is_err());
        assert_eq!(
            preset.as_bytes(),
            original,
            "model {model:#010x}, valid {valid}"
        );
        assert_eq!(preset.effect_enabled(slot), None);
    }

    let mut bad_version = synthetic_enable_h5ep(0, 0);
    bad_version[7] = 2;
    assert!(OpaqueH5epPreset::parse(bad_version).is_err());
}

#[test]
fn admitted_enable_dirty_state_returns_to_saved_baseline() {
    let mut library = HdEditorLibrary::default();
    let mut original = synthetic_enable_h5ep(0, 0);
    let other_block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_BLOCK_STRIDE;
    original[other_block..other_block + 4].copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
    original[other_block + 8] = 0x7f;
    original[other_block + 0x0b] = 1;
    let index = library.import_bytes(original.clone()).unwrap();
    let slot = HdFxSlot::new(0).unwrap();
    library.set_effect_enabled(index, slot, true).unwrap();
    assert!(library.is_dirty(index));

    let dirty_bytes = library.file(index).unwrap().as_bytes().to_vec();
    let noncanonical_slot = HdFxSlot::new(1).unwrap();
    for requested in [false, true] {
        assert!(
            library
                .set_effect_enabled(index, noncanonical_slot, requested)
                .is_err()
        );
        assert_eq!(library.file(index).unwrap().as_bytes(), dirty_bytes);
        assert!(library.is_dirty(index));
    }

    library.set_effect_enabled(index, slot, false).unwrap();
    assert!(!library.is_dirty(index));
    assert!(!library.any_dirty());
    assert_eq!(library.file(index).unwrap().as_bytes(), original);
}

#[test]
fn opaque_h5ep_dispatch_and_noop_round_trip_preserve_every_byte() {
    let mut original = synthetic_h5ep(b"uninterpreted trailing bytes");
    // Exercise noncanonical/unknown header and body values, including bytes
    // that resemble duplicated fields. No field is normalized or decoded.
    original[8..H5EP_HEADER_BYTES].fill(0xa5);
    original[H5EP_HEADER_BYTES + 0xd90..H5EP_HEADER_BYTES + 0xda0].fill(0x5a);
    original[0x0b] = 0x2e;
    original[0x13..0x16].copy_from_slice(&[0x06, 0x02, 0x62]);
    let file = PodPresetFile::parse(original.clone()).unwrap();
    let PodPresetFile::H5ep(envelope) = file else {
        panic!("H5EP must dispatch to the structured editor record");
    };
    assert_eq!(envelope.header_bytes().len(), H5EP_HEADER_BYTES);
    assert_eq!(envelope.body_bytes().len(), H5EP_BODY_BYTES);
    assert_eq!(envelope.trailing_bytes(), b"uninterpreted trailing bytes");
    assert_eq!(envelope.serialize_noop(), original);
}

#[test]
fn opaque_h5ep_rejects_bad_magic_version_truncation_and_oversize() {
    let valid = synthetic_h5ep(&[]);
    for len in [0, 3, 4, H5EP_HEADER_BYTES - 1, H5EP_MIN_BYTES - 1] {
        assert!(
            OpaqueH5epPreset::parse(valid[..len].to_vec()).is_err(),
            "len {len}"
        );
    }
    let mut bad_magic = valid.clone();
    bad_magic[..4].copy_from_slice(b"H5ES");
    assert!(OpaqueH5epPreset::parse(bad_magic).is_err());
    let mut bad_version = valid;
    bad_version[7] = 2;
    assert!(OpaqueH5epPreset::parse(bad_version).is_err());
    assert!(OpaqueH5epPreset::parse(vec![0; TONE_BYTES_CEILING as usize + 1]).is_err());
}

#[test]
fn octave_fuzz_replacement_rebuilds_the_ordered_prefix_for_every_fx_slot() {
    for effect_slot in 0..HD_EFFECT_COUNT {
        let original = synthetic_octave_fuzz_h5ep(effect_slot, true);
        let mut expected = original.clone();
        let block =
            H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        expected[block..block + 4].copy_from_slice(&HD_VINTAGE_PRE_MODEL_ID.to_be_bytes());
        for (row, key) in HD_VINTAGE_PRE_DEFAULT_KEYS.into_iter().enumerate() {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
        }
        expected[block + 0x0f] = 5;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
        }

        let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
        preset
            .replace_octave_fuzz_with_vintage_pre_defaults(HdFxSlot::new(effect_slot).unwrap())
            .unwrap();
        assert_eq!(preset.as_bytes(), expected, "FX slot {effect_slot}");
        assert_eq!(preset.trailing_bytes(), b"private-tail");
        assert_eq!(
            preset.effect_model_id(effect_slot),
            Some(HD_VINTAGE_PRE_MODEL_ID)
        );

        let row6 = block + HD_EFFECT_ROWS_OFFSET + 6 * HD_PARAMETER_ROW_STRIDE;
        assert_eq!(
            &preset.as_bytes()[row6..row6 + 8],
            &original[row6..row6 + 8]
        );
        assert_eq!(
            &preset.as_bytes()[block + 4..block + 0x0b],
            &original[block + 4..block + 0x0b]
        );
    }
    assert!(HdFxSlot::new(8).is_err());
}

#[test]
fn octave_fuzz_to_screamer_uses_literal_defaults_and_preserves_the_bounded_write_set() {
    const SCREAMER_ID: u32 = 0x0205_000e;
    const EXPECTED_KEYS: [u32; 5] = [
        0x3f10_0001,
        0x3f10_0002,
        0x3f10_0003,
        0x3f10_0004,
        0x3f10_0005,
    ];
    for effect_slot in 0..HD_EFFECT_COUNT {
        let original = synthetic_octave_fuzz_h5ep(effect_slot, true);
        let mut expected = original.clone();
        let block =
            H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        expected[block..block + 4].copy_from_slice(&SCREAMER_ID.to_be_bytes());
        for (row, key) in EXPECTED_KEYS.into_iter().enumerate() {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
        }
        expected[block + 0x0f] = 5;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
        }

        let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
        preset
            .replace_octave_fuzz_with_screamer_defaults(HdFxSlot::new(effect_slot).unwrap())
            .unwrap();
        assert_eq!(preset.as_bytes(), expected, "FX slot {effect_slot}");
        assert_eq!(preset.effect_model_id(effect_slot), Some(SCREAMER_ID));
        assert_eq!(preset.trailing_bytes(), b"private-tail");
    }
}

#[test]
fn octave_fuzz_to_sub_octave_fuzz_uses_literal_defaults_for_all_public_slots() {
    const TARGET_ID: u32 = 0x0205_0014;
    const EXPECTED_KEYS: [u32; 5] = [
        0x3f10_0001,
        0x3f10_0002,
        0x3f10_0003,
        0x3f10_0004,
        0x3f10_0005,
    ];
    for public_slot in 0..HD_EFFECT_COUNT {
        let original = synthetic_octave_fuzz_h5ep(public_slot, true);
        let mut expected = original.clone();
        let block =
            H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + public_slot * HD_EFFECT_BLOCK_STRIDE;
        expected[block..block + 4].copy_from_slice(&TARGET_ID.to_be_bytes());
        for (row, key) in EXPECTED_KEYS.into_iter().enumerate() {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
        }
        expected[block + 0x0f] = 5;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
        }

        let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
        preset
            .replace_octave_fuzz_with_sub_octave_fuzz_defaults(HdFxSlot::new(public_slot).unwrap())
            .unwrap();
        assert_eq!(preset.as_bytes(), expected, "public FX slot {public_slot}");
        assert_eq!(preset.effect_model_id(public_slot), Some(TARGET_ID));
        assert_eq!(preset.trailing_bytes(), b"private-tail");
    }
    assert!(HdFxSlot::new(8).is_err());
}

#[test]
fn octave_fuzz_to_noise_gate_matches_exact_expected_bytes_across_slots_and_source_orders() {
    const TARGET_ID: u32 = 0x0200_0011;
    const TARGET_KEYS: [u32; 2] = [0x3f10_0001, 0x3f10_0002];
    let permutations = five_key_permutations();
    assert_eq!(permutations.len(), 120);

    for slot in 0..HD_EFFECT_COUNT {
        for enabled in [0, 1] {
            for source_count in [5, 12] {
                for source_keys in &permutations {
                    let mut original = synthetic_octave_fuzz_h5ep(slot, true);
                    let block =
                        H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + slot * HD_EFFECT_BLOCK_STRIDE;
                    original[block + 8] = enabled;
                    original[block + 0x0f] = source_count;
                    for (row, key) in source_keys.iter().copied().enumerate() {
                        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                        original[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                    }

                    // Independently authored expected write set: model, the
                    // two ordered key/value rows, count and marker byte 0
                    // for every row. Every other byte comes from source.
                    let mut expected = original.clone();
                    expected[block..block + 4].copy_from_slice(&TARGET_ID.to_be_bytes());
                    for (row, key) in TARGET_KEYS.iter().copied().enumerate() {
                        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                        expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                        expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
                    }
                    expected[block + 0x0f] = 2;
                    for row in 0..HD_PARAMETER_ROW_COUNT {
                        expected[block
                            + HD_EFFECT_ROWS_OFFSET
                            + row * HD_PARAMETER_ROW_STRIDE
                            + 0x10] = 0;
                    }

                    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                    preset
                        .replace_octave_fuzz_with_noise_gate_defaults(HdFxSlot::new(slot).unwrap())
                        .unwrap();
                    assert_eq!(
                        preset.as_bytes(),
                        expected,
                        "slot={slot}, enabled={enabled}, count={source_count}, keys={source_keys:?}"
                    );
                    assert_eq!(preset.effect_model_id(slot), Some(TARGET_ID));
                    assert_eq!(preset.trailing_bytes(), b"private-tail");
                }
            }
        }
    }
}

#[test]
fn noise_gate_replacement_checks_each_marker_value_position_and_rejects_atomically() {
    let source = synthetic_octave_fuzz_h5ep(0, true);
    for row in 0..HD_PARAMETER_ROW_COUNT {
        for marker in 0..=2 {
            let mut bytes = source.clone();
            let at = H5EP_HEADER_BYTES
                + HD_EFFECT_BLOCK_OFFSET
                + HD_EFFECT_ROWS_OFFSET
                + row * HD_PARAMETER_ROW_STRIDE
                + 0x10;
            bytes[at] = marker;
            let preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            let mut expected = bytes;
            let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
            expected[block..block + 4].copy_from_slice(&HD_NOISE_GATE_MODEL_ID.to_be_bytes());
            for (index, key) in HD_NOISE_GATE_DEFAULT_KEYS.iter().copied().enumerate() {
                let row_at = block + HD_EFFECT_ROWS_OFFSET + index * HD_PARAMETER_ROW_STRIDE;
                expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
            }
            expected[block + 0x0f] = 2;
            for index in 0..HD_PARAMETER_ROW_COUNT {
                expected[block + HD_EFFECT_ROWS_OFFSET + index * HD_PARAMETER_ROW_STRIDE + 0x10] =
                    0;
            }
            let mut preset = preset;
            preset
                .replace_octave_fuzz_with_noise_gate_defaults(HdFxSlot::new(0).unwrap())
                .unwrap();
            assert_eq!(
                preset.as_bytes(),
                expected,
                "marker row={row}, value={marker}"
            );
        }

        let mut bytes = source.clone();
        let at = H5EP_HEADER_BYTES
            + HD_EFFECT_BLOCK_OFFSET
            + HD_EFFECT_ROWS_OFFSET
            + row * HD_PARAMETER_ROW_STRIDE
            + 0x10;
        bytes[at] = 3;
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert!(
            preset
                .replace_octave_fuzz_with_noise_gate_defaults(HdFxSlot::new(0).unwrap())
                .is_err()
        );
        assert_eq!(preset.as_bytes(), bytes, "invalid marker row={row}");
    }
}

#[test]
fn noise_gate_replacement_preserves_unknown_finite_and_nonfinite_suffix_words() {
    let mut original = synthetic_octave_fuzz_h5ep(3, true);
    original.extend_from_slice(&[0x3f, 0x80, 0, 0, 0x7f, 0xc0, 0, 1]);
    let mut expected = original.clone();
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 3 * HD_EFFECT_BLOCK_STRIDE;
    expected[block..block + 4].copy_from_slice(&HD_NOISE_GATE_MODEL_ID.to_be_bytes());
    for (row, key) in HD_NOISE_GATE_DEFAULT_KEYS.iter().copied().enumerate() {
        let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
        expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
        expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
    }
    expected[block + 0x0f] = 2;
    for row in 0..HD_PARAMETER_ROW_COUNT {
        expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
    }
    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
    preset
        .replace_octave_fuzz_with_noise_gate_defaults(HdFxSlot::new(3).unwrap())
        .unwrap();
    assert_eq!(preset.as_bytes(), expected);
    assert!(
        preset
            .as_bytes()
            .ends_with(&[0x3f, 0x80, 0, 0, 0x7f, 0xc0, 0, 1])
    );
}

#[test]
fn noise_gate_library_operation_rejects_container_parents_unchanged() {
    let mut library = HdEditorLibrary::default();
    let index = library
        .import_bytes(synthetic_h5es(b"parent-tail"))
        .unwrap();
    let before = library.file(index).unwrap().as_bytes().to_vec();
    assert!(
        library
            .replace_octave_fuzz_with_noise_gate_defaults(index, HdFxSlot::new(0).unwrap())
            .is_err()
    );
    assert_eq!(library.file(index).unwrap().as_bytes(), before);
    assert!(!library.is_dirty(index));
}

#[test]
fn noise_gate_replacement_rejects_guard_failures_and_preserves_library_baseline() {
    let source = synthetic_octave_fuzz_h5ep(0, true);
    let mut cases = Vec::new();
    for model in [
        HD_NOISE_GATE_MODEL_ID,
        HD_SCREAMER_MODEL_ID,
        HD_SUB_OCTAVE_FUZZ_MODEL_ID,
    ] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET
            ..H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 4]
            .copy_from_slice(&model.to_be_bytes());
        cases.push(("wrong current model", bytes));
    }
    for validity in [0, 2] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0b] = validity;
        cases.push(("validity", bytes));
    }
    let mut bytes = source.clone();
    bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8] = 2;
    cases.push(("enable", bytes));
    for count in [4, 13] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0f] = count;
        cases.push(("count", bytes));
    }
    for row in 0..5 {
        let mut bytes = source.clone();
        let at = H5EP_HEADER_BYTES
            + HD_EFFECT_BLOCK_OFFSET
            + HD_EFFECT_ROWS_OFFSET
            + row * HD_PARAMETER_ROW_STRIDE;
        bytes[at..at + 4].copy_from_slice(&0xfeed_beefu32.to_be_bytes());
        cases.push(("missing source key", bytes));
    }
    let mut bytes = source.clone();
    let row1 = H5EP_HEADER_BYTES
        + HD_EFFECT_BLOCK_OFFSET
        + HD_EFFECT_ROWS_OFFSET
        + HD_PARAMETER_ROW_STRIDE;
    bytes[row1..row1 + 4].copy_from_slice(&HD_TYPE1_OUTPUT_KEY.to_be_bytes());
    cases.push(("duplicate source key", bytes));

    for (label, bytes) in cases {
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert!(
            preset
                .replace_octave_fuzz_with_noise_gate_defaults(HdFxSlot::new(0).unwrap())
                .is_err(),
            "{label}"
        );
        assert_eq!(preset.as_bytes(), bytes, "{label}");
        let mut library = HdEditorLibrary::default();
        let index = library.import_bytes(bytes.clone()).unwrap();
        assert!(
            library
                .replace_octave_fuzz_with_noise_gate_defaults(index, HdFxSlot::new(0).unwrap())
                .is_err(),
            "library {label}"
        );
        assert_eq!(
            library.file(index).unwrap().as_bytes(),
            bytes,
            "library {label}"
        );
        assert!(!library.is_dirty(index), "library baseline {label}");
    }

    let mut library = HdEditorLibrary::default();
    let index = library.import_bytes(source.clone()).unwrap();
    library
        .replace_octave_fuzz_with_screamer_defaults(index, HdFxSlot::new(0).unwrap())
        .unwrap();
    let dirty_before = library.file(index).unwrap().as_bytes().to_vec();
    assert!(library.is_dirty(index));
    assert!(
        library
            .replace_octave_fuzz_with_noise_gate_defaults(index, HdFxSlot::new(0).unwrap())
            .is_err()
    );
    assert_eq!(library.file(index).unwrap().as_bytes(), dirty_before);
    assert!(library.is_dirty(index));
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.file(index).unwrap().as_bytes(), source);
    assert!(!library.is_dirty(index));
    assert!(HdFxSlot::new(8).is_err());
}

#[test]
fn admitted_dynamics_defaults_match_all_corrected_v2_full_envelope_fixtures() {
    const SOURCE: &[u8] = include_bytes!(
        "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/before-ocfz-file-slot5-be.h5e"
    );
    const TARGETS: [(HdDynamicsDefault, u32, &[u32], &[u8]); 8] = [
        (
            HdDynamicsDefault::HardGate,
            0x0200_0013,
            &[0x3f10_0000, 0x3f10_0001, 0x3f10_0002, 0x3f10_0003],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-hard-gate-slot5.expected.h5e"
            ),
        ),
        (
            HdDynamicsDefault::BoostComp,
            0x0200_0012,
            &[
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-boost-comp-slot5.expected.h5e"
            ),
        ),
        (
            HdDynamicsDefault::RedComp,
            0x0200_000b,
            &[0x3f10_0001, 0x3f10_0002],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-red-comp-slot5.expected.h5e"
            ),
        ),
        (
            HdDynamicsDefault::BlueComp,
            0x0200_000c,
            &[0x3f10_0001, 0x3f10_0002],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-blue-comp-slot5.expected.h5e"
            ),
        ),
        (
            HdDynamicsDefault::BlueCompTreb,
            0x0200_000d,
            &[0x3f10_0002, 0x3f10_0001],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-blue-comp-treb-slot5.expected.h5e"
            ),
        ),
        (
            HdDynamicsDefault::VettaComp,
            0x0200_000f,
            &[0x3f10_0001, 0x3f10_0002],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-vetta-comp-slot5.expected.h5e"
            ),
        ),
        (
            HdDynamicsDefault::VettaJuice,
            0x0200_0010,
            &[0x3f10_0001, 0x3f10_0002],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-vetta-juice-slot5.expected.h5e"
            ),
        ),
        (
            HdDynamicsDefault::TubeComp,
            0x0200_000e,
            &[0x3f10_0001, 0x3f10_0002],
            include_bytes!(
                "../../../../tests/fixtures/hd500x-dynamics-defaults-v2/after-tube-comp-slot5.expected.h5e"
            ),
        ),
    ];
    assert_eq!(SOURCE.len(), 4151);
    assert_eq!(&SOURCE[0x950..0x954], &[0x02, 0x05, 0x00, 0x0c]);
    assert!(SOURCE.ends_with(b"PRIVATE-TAIL-01"));
    for (target, target_id, keys, expected) in TARGETS {
        assert_eq!(expected.len(), SOURCE.len(), "target={target:?}");
        let mut preset = OpaqueH5epPreset::parse(SOURCE.to_vec()).unwrap();
        preset
            .replace_octave_fuzz_with_admitted_dynamics_defaults(HdFxSlot::new(5).unwrap(), target)
            .unwrap();
        assert_eq!(preset.effect_model_id(5), Some(target_id));
        assert_eq!(
            (0..keys.len())
                .map(|row| {
                    let at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + 5 * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row * HD_PARAMETER_ROW_STRIDE;
                    read_be_u32(preset.as_bytes(), at).unwrap()
                })
                .collect::<Vec<_>>(),
            keys.to_vec(),
            "target={target:?}"
        );
        assert_eq!(preset.as_bytes(), expected, "target={target:?}");
    }
}

#[test]
fn octave_fuzz_admitted_dynamics_defaults_match_every_source_prefix_and_write_set() {
    const TARGETS: [(HdDynamicsDefault, u32, &[u32]); 8] = [
        (
            HdDynamicsDefault::HardGate,
            0x0200_0013,
            &[0x3f10_0000, 0x3f10_0001, 0x3f10_0002, 0x3f10_0003],
        ),
        (
            HdDynamicsDefault::BoostComp,
            0x0200_0012,
            &[
                0x3f10_0001,
                0x3f10_0002,
                0x3f10_0003,
                0x3f10_0004,
                0x3f10_0005,
            ],
        ),
        (
            HdDynamicsDefault::RedComp,
            0x0200_000b,
            &[0x3f10_0001, 0x3f10_0002],
        ),
        (
            HdDynamicsDefault::BlueComp,
            0x0200_000c,
            &[0x3f10_0001, 0x3f10_0002],
        ),
        (
            HdDynamicsDefault::BlueCompTreb,
            0x0200_000d,
            &[0x3f10_0002, 0x3f10_0001],
        ),
        (
            HdDynamicsDefault::VettaComp,
            0x0200_000f,
            &[0x3f10_0001, 0x3f10_0002],
        ),
        (
            HdDynamicsDefault::VettaJuice,
            0x0200_0010,
            &[0x3f10_0001, 0x3f10_0002],
        ),
        (
            HdDynamicsDefault::TubeComp,
            0x0200_000e,
            &[0x3f10_0001, 0x3f10_0002],
        ),
    ];
    assert_eq!(HdDynamicsDefault::ALL.len(), TARGETS.len());
    assert_eq!(HdDynamicsDefault::ALL, TARGETS.map(|(target, _, _)| target));
    for (target, model_id, keys) in TARGETS {
        assert_eq!(target.model_id(), model_id);
        assert_eq!(target.default_keys(), keys);
    }

    let permutations = five_key_permutations();
    assert_eq!(permutations.len(), 120);
    for (target, target_id, target_keys) in TARGETS {
        for slot in 0..HD_EFFECT_COUNT {
            for enabled in [0, 1] {
                for source_count in [5, 12] {
                    for source_keys in &permutations {
                        let mut original = synthetic_octave_fuzz_h5ep(slot, true);
                        let block = H5EP_HEADER_BYTES
                            + HD_EFFECT_BLOCK_OFFSET
                            + slot * HD_EFFECT_BLOCK_STRIDE;
                        original[block + 8] = enabled;
                        original[block + 0x0f] = source_count;
                        for (row, key) in source_keys.iter().copied().enumerate() {
                            let row_at =
                                block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                            original[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                        }

                        // Independent expected full-file write set, using
                        // the admitted source table above rather than the
                        // implementation's selected constants.
                        let mut expected = original.clone();
                        expected[block..block + 4].copy_from_slice(&target_id.to_be_bytes());
                        for (row, key) in target_keys.iter().copied().enumerate() {
                            let row_at =
                                block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                            expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                            expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
                        }
                        expected[block + 0x0f] = target_keys.len() as u8;
                        for row in 0..HD_PARAMETER_ROW_COUNT {
                            expected[block
                                + HD_EFFECT_ROWS_OFFSET
                                + row * HD_PARAMETER_ROW_STRIDE
                                + 0x10] = 0;
                        }

                        let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                        preset
                            .replace_octave_fuzz_with_admitted_dynamics_defaults(
                                HdFxSlot::new(slot).unwrap(),
                                target,
                            )
                            .unwrap();
                        assert_eq!(
                            preset.as_bytes(),
                            expected,
                            "target={target:?}, slot={slot}, enabled={enabled}, count={source_count}, keys={source_keys:?}"
                        );
                        assert_eq!(preset.effect_model_id(slot), Some(target_id));
                        assert_eq!(preset.trailing_bytes(), b"private-tail");
                    }
                }
            }
        }
    }
}

#[test]
fn admitted_dynamics_defaults_cover_all_canonical_markers_and_reject_bad_sources_atomically() {
    for target in HdDynamicsDefault::ALL {
        for slot in 0..HD_EFFECT_COUNT {
            for row in 0..HD_PARAMETER_ROW_COUNT {
                for marker in 0..=2 {
                    let mut bytes = synthetic_octave_fuzz_h5ep(slot, true);
                    let at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row * HD_PARAMETER_ROW_STRIDE
                        + 0x10;
                    bytes[at] = marker;
                    let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
                    preset
                        .replace_octave_fuzz_with_admitted_dynamics_defaults(
                            HdFxSlot::new(slot).unwrap(),
                            target,
                        )
                        .unwrap();
                    assert!(
                        (0..HD_PARAMETER_ROW_COUNT).all(|index| {
                            let marker_at = H5EP_HEADER_BYTES
                                + HD_EFFECT_BLOCK_OFFSET
                                + slot * HD_EFFECT_BLOCK_STRIDE
                                + HD_EFFECT_ROWS_OFFSET
                                + index * HD_PARAMETER_ROW_STRIDE
                                + 0x10;
                            preset.as_bytes()[marker_at] == 0
                        }),
                        "target={target:?}, slot={slot}, row={row}, marker={marker}"
                    );
                }
                let mut bytes = synthetic_octave_fuzz_h5ep(slot, true);
                let at = H5EP_HEADER_BYTES
                    + HD_EFFECT_BLOCK_OFFSET
                    + slot * HD_EFFECT_BLOCK_STRIDE
                    + HD_EFFECT_ROWS_OFFSET
                    + row * HD_PARAMETER_ROW_STRIDE
                    + 0x10;
                bytes[at] = 3;
                let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
                assert!(
                    preset
                        .replace_octave_fuzz_with_admitted_dynamics_defaults(
                            HdFxSlot::new(slot).unwrap(),
                            target,
                        )
                        .is_err()
                );
                assert_eq!(preset.as_bytes(), bytes);
            }
        }

        let source = synthetic_octave_fuzz_h5ep(0, true);
        let mut invalid = Vec::new();
        for model in [
            HD_NOISE_GATE_MODEL_ID,
            HD_SCREAMER_MODEL_ID,
            target.model_id(),
        ] {
            let mut bytes = source.clone();
            let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
            bytes[block..block + 4].copy_from_slice(&model.to_be_bytes());
            invalid.push(("wrong current model", bytes));
        }
        for validity in [0, 2] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0b] = validity;
            invalid.push(("validity", bytes));
        }
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8] = 2;
        invalid.push(("enable", bytes));
        for count in [4, 13] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0f] = count;
            invalid.push(("count", bytes));
        }
        let mut bytes = source.clone();
        let row = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_ROWS_OFFSET;
        bytes[row..row + 4].copy_from_slice(&0xfeed_beefu32.to_be_bytes());
        invalid.push(("missing required key", bytes));
        let mut bytes = source.clone();
        let row = H5EP_HEADER_BYTES
            + HD_EFFECT_BLOCK_OFFSET
            + HD_EFFECT_ROWS_OFFSET
            + HD_PARAMETER_ROW_STRIDE;
        bytes[row..row + 4].copy_from_slice(&HD_TYPE1_OUTPUT_KEY.to_be_bytes());
        invalid.push(("duplicate in first five", bytes));

        for (label, bytes) in invalid {
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            assert!(
                preset
                    .replace_octave_fuzz_with_admitted_dynamics_defaults(
                        HdFxSlot::new(0).unwrap(),
                        target,
                    )
                    .is_err(),
                "target={target:?}, case={label}"
            );
            assert_eq!(preset.as_bytes(), bytes, "target={target:?}, case={label}");
        }
    }
    assert!(HdFxSlot::new(8).is_err());
}

#[test]
fn admitted_reverb_defaults_match_independent_full_file_vectors() {
    const SOURCE: &[u8] = include_bytes!(
        "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/before-ocfz-file-slot5-be.h5e"
    );
    const TARGETS: [(HdReverbDefault, u32, &[u8]); 8] = [
        (
            HdReverbDefault::Octo,
            0x0204_0023,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-octo-slot5.expected.h5e"
            ),
        ),
        (
            HdReverbDefault::Ducking,
            0x0204_0022,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-duck-slot5.expected.h5e"
            ),
        ),
        (
            HdReverbDefault::Plate,
            0x0204_001e,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-plat-slot5.expected.h5e"
            ),
        ),
        (
            HdReverbDefault::Room,
            0x0204_001f,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-room-slot5.expected.h5e"
            ),
        ),
        (
            HdReverbDefault::Chamber,
            0x0204_0020,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-chmb-slot5.expected.h5e"
            ),
        ),
        (
            HdReverbDefault::Hall,
            0x0204_0021,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-hall-slot5.expected.h5e"
            ),
        ),
        (
            HdReverbDefault::Cave,
            0x0204_0024,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-cave-slot5.expected.h5e"
            ),
        ),
        (
            HdReverbDefault::Tile,
            0x0204_0025,
            include_bytes!(
                "../../../../tests/fixtures/hd500x-next-fx-defaults-v1/after-tile-slot5.expected.h5e"
            ),
        ),
    ];
    const KEYS: [u32; 4] = [0x3f10_0001, 0x3f10_0002, 0x3f10_0003, 0x3f10_0005];
    assert_eq!(SOURCE.len(), 4151);
    assert_eq!(HdReverbDefault::ALL, TARGETS.map(|(target, _, _)| target));
    for (target, expected_model, expected) in TARGETS {
        assert_eq!(expected.len(), SOURCE.len());
        let mut preset = OpaqueH5epPreset::parse(SOURCE.to_vec()).unwrap();
        preset
            .replace_octave_fuzz_with_admitted_reverb_defaults(HdFxSlot::new(5).unwrap(), target)
            .unwrap();
        assert_eq!(preset.effect_model_id(5), Some(expected_model));
        assert_eq!(preset.as_bytes(), expected, "target={target:?}");
    }
    assert_eq!(HdReverbDefault::Octo.default_keys(), &KEYS);
    assert!(HdFxSlot::new(8).is_err());
}

#[test]
fn admitted_reverb_defaults_cover_slots_enable_counts_key_orders_and_guards() {
    const TARGETS: [(HdReverbDefault, u32); 8] = [
        (HdReverbDefault::Octo, 0x0204_0023),
        (HdReverbDefault::Ducking, 0x0204_0022),
        (HdReverbDefault::Plate, 0x0204_001e),
        (HdReverbDefault::Room, 0x0204_001f),
        (HdReverbDefault::Chamber, 0x0204_0020),
        (HdReverbDefault::Hall, 0x0204_0021),
        (HdReverbDefault::Cave, 0x0204_0024),
        (HdReverbDefault::Tile, 0x0204_0025),
    ];
    const KEYS: [u32; 4] = [0x3f10_0001, 0x3f10_0002, 0x3f10_0003, 0x3f10_0005];
    assert_eq!(HdReverbDefault::ALL, TARGETS.map(|(target, _)| target));
    let permutations = five_key_permutations();
    assert_eq!(permutations.len(), 120);
    for (target, target_model) in TARGETS {
        assert_eq!(target.model_id(), target_model);
        assert_eq!(target.default_keys(), &KEYS);
        for slot in 0..HD_EFFECT_COUNT {
            for enabled in [0, 1] {
                for count in [5, 12] {
                    for order in &permutations {
                        let mut original = synthetic_octave_fuzz_h5ep(slot, true);
                        let block = H5EP_HEADER_BYTES
                            + HD_EFFECT_BLOCK_OFFSET
                            + slot * HD_EFFECT_BLOCK_STRIDE;
                        original[block + 8] = enabled;
                        original[block + 0x0f] = count;
                        for (row, key) in order.iter().copied().enumerate() {
                            let at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                            original[at..at + 4].copy_from_slice(&key.to_be_bytes());
                        }
                        let mut expected = original.clone();
                        expected[block..block + 4].copy_from_slice(&target_model.to_be_bytes());
                        for (row, key) in KEYS.into_iter().enumerate() {
                            let at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                            expected[at..at + 4].copy_from_slice(&key.to_be_bytes());
                            expected[at + 4..at + 8].copy_from_slice(&0u32.to_be_bytes());
                        }
                        expected[block + 0x0f] = 4;
                        for row in 0..HD_PARAMETER_ROW_COUNT {
                            expected[block
                                + HD_EFFECT_ROWS_OFFSET
                                + row * HD_PARAMETER_ROW_STRIDE
                                + 0x10] = 0;
                        }
                        let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                        preset
                            .replace_octave_fuzz_with_admitted_reverb_defaults(
                                HdFxSlot::new(slot).unwrap(),
                                target,
                            )
                            .unwrap();
                        assert_eq!(
                            preset.as_bytes(),
                            expected,
                            "target={target:?}, slot={slot}, enabled={enabled}, count={count}, order={order:?}"
                        );
                    }
                }
            }
            for row in 0..HD_PARAMETER_ROW_COUNT {
                for marker in 0..=2 {
                    let mut bytes = synthetic_octave_fuzz_h5ep(slot, true);
                    let at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row * HD_PARAMETER_ROW_STRIDE
                        + 0x10;
                    bytes[at] = marker;
                    let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
                    preset
                        .replace_octave_fuzz_with_admitted_reverb_defaults(
                            HdFxSlot::new(slot).unwrap(),
                            target,
                        )
                        .unwrap();
                }
            }
        }
        let source = synthetic_octave_fuzz_h5ep(0, true);
        let mut invalid = Vec::new();
        for model in [HD_NOISE_GATE_MODEL_ID, HD_SCREAMER_MODEL_ID, target_model] {
            let mut bytes = source.clone();
            let at = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
            bytes[at..at + 4].copy_from_slice(&model.to_be_bytes());
            invalid.push(bytes);
        }
        for validity in [0, 2] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0b] = validity;
            invalid.push(bytes);
        }
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8] = 2;
        invalid.push(bytes);
        for count in [4, 13] {
            let mut bytes = source.clone();
            bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0f] = count;
            invalid.push(bytes);
        }
        let mut bytes = source.clone();
        let row = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_ROWS_OFFSET;
        bytes[row..row + 4].copy_from_slice(&0xfeed_beefu32.to_be_bytes());
        invalid.push(bytes);
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES
            + HD_EFFECT_BLOCK_OFFSET
            + HD_EFFECT_ROWS_OFFSET
            + HD_PARAMETER_ROW_STRIDE..][..4]
            .copy_from_slice(&HD_TYPE1_OUTPUT_KEY.to_be_bytes());
        invalid.push(bytes);
        let mut bytes = source.clone();
        let marker = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_ROWS_OFFSET + 0x10;
        bytes[marker] = 3;
        invalid.push(bytes);
        for bytes in invalid {
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            assert!(
                preset
                    .replace_octave_fuzz_with_admitted_reverb_defaults(
                        HdFxSlot::new(0).unwrap(),
                        target,
                    )
                    .is_err()
            );
            assert_eq!(preset.as_bytes(), bytes, "target={target:?}");
        }
    }
}

#[test]
fn octave_fuzz_admitted_distortion_defaults_match_the_exact_closed_set_and_write_set() {
    const TARGETS: [(HdDistortionDefault, u32); 12] = [
        (HdDistortionDefault::JetFuzz, 0x0205_000a),
        (HdDistortionDefault::BuzzSaw, 0x0205_0015),
        (HdDistortionDefault::ClassicDistortion, 0x0205_000b),
        (HdDistortionDefault::ColorDrive, 0x0205_0018),
        (HdDistortionDefault::FacialFuzz, 0x0205_0011),
        (HdDistortionDefault::FuzzPi, 0x0205_000f),
        (HdDistortionDefault::HeavyDistortion, 0x0205_0016),
        (HdDistortionDefault::JumboFuzz, 0x0205_0017),
        (HdDistortionDefault::Line6Distortion, 0x0205_0012),
        (HdDistortionDefault::Line6Drive, 0x0205_0013),
        (HdDistortionDefault::Overdrive, 0x0205_0010),
        (HdDistortionDefault::TubeDrive, 0x0205_000d),
    ];
    assert_eq!(HdDistortionDefault::ALL.len(), 12);
    assert_eq!(HdDistortionDefault::ALL, TARGETS.map(|(target, _)| target));
    assert_eq!(
        TARGETS.map(|(target, _)| target.model_id()),
        TARGETS.map(|(_, model_id)| model_id)
    );
    const KEYS: [u32; 5] = [
        0x3f10_0001,
        0x3f10_0002,
        0x3f10_0003,
        0x3f10_0004,
        0x3f10_0005,
    ];

    for (target, target_id) in TARGETS {
        for public_slot in 0..HD_EFFECT_COUNT {
            let original = synthetic_octave_fuzz_h5ep(public_slot, true);
            let mut expected = original.clone();
            let block =
                H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + public_slot * HD_EFFECT_BLOCK_STRIDE;
            expected[block..block + 4].copy_from_slice(&target_id.to_be_bytes());
            for (row, key) in KEYS.into_iter().enumerate() {
                let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
                expected[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
                expected[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
            }
            expected[block + 0x0f] = 5;
            for row in 0..HD_PARAMETER_ROW_COUNT {
                expected[block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10] = 0;
            }

            let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
            preset
                .replace_octave_fuzz_with_admitted_defaults(
                    HdFxSlot::new(public_slot).unwrap(),
                    target,
                )
                .unwrap();
            assert_eq!(
                preset.as_bytes(),
                expected,
                "{target:?}, slot {public_slot}"
            );
            assert_eq!(preset.effect_model_id(public_slot), Some(target_id));
            assert_eq!(preset.trailing_bytes(), b"private-tail");
        }
    }
    assert!(HdFxSlot::new(8).is_err());
}

#[test]
fn admitted_distortion_defaults_reject_guard_failures_without_changing_library_baselines() {
    let source = synthetic_octave_fuzz_h5ep(0, true);
    let mut cases = Vec::new();
    for model in [
        0x0205_000a,
        0x0205_0015,
        0x0205_000b,
        0x0205_0018,
        0x0205_0011,
        0x0205_000f,
        0x0205_0016,
        0x0205_0017,
        0x0205_0012,
        0x0205_0013,
        0x0205_0010,
        0x0205_000d,
        HD_SCREAMER_MODEL_ID,
        HD_SUB_OCTAVE_FUZZ_MODEL_ID,
        0x0200_0013,
        HD_FX_LOOP_MODEL_ID,
    ] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET
            ..H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 4]
            .copy_from_slice(&model.to_be_bytes());
        cases.push(("wrong old model", bytes));
    }
    for validity in [0, 2] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0b] = validity;
        cases.push(("validity", bytes));
    }
    let mut bytes = source.clone();
    bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8] = 2;
    cases.push(("enable", bytes));
    for count in [4, 13] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0f] = count;
        cases.push(("count", bytes));
    }
    let row0 = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_ROWS_OFFSET;
    let mut bytes = source.clone();
    bytes[row0..row0 + 4].copy_from_slice(&0xfeed_beefu32.to_be_bytes());
    cases.push(("unknown prefix key", bytes));
    let mut bytes = source.clone();
    let row1 = row0 + HD_PARAMETER_ROW_STRIDE;
    bytes[row1..row1 + 4].copy_from_slice(&HD_TYPE1_OUTPUT_KEY.to_be_bytes());
    cases.push(("duplicate prefix key", bytes));
    let marker = H5EP_HEADER_BYTES
        + HD_EFFECT_BLOCK_OFFSET
        + HD_EFFECT_ROWS_OFFSET
        + 11 * HD_PARAMETER_ROW_STRIDE
        + 0x10;
    let mut bytes = source.clone();
    bytes[marker] = 3;
    cases.push(("marker", bytes));

    for target in HdDistortionDefault::ALL {
        for (label, bytes) in &cases {
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            let before = preset.as_bytes().to_vec();
            assert!(
                preset
                    .replace_octave_fuzz_with_admitted_defaults(HdFxSlot::new(0).unwrap(), target,)
                    .is_err(),
                "{target:?}: {label}"
            );
            assert_eq!(preset.as_bytes(), before, "{target:?}: {label}");

            let mut library = HdEditorLibrary::default();
            let index = library.import_bytes(bytes.clone()).unwrap();
            let saved = library.file(index).unwrap().as_bytes().to_vec();
            assert!(
                library
                    .replace_octave_fuzz_with_admitted_defaults(
                        index,
                        HdFxSlot::new(0).unwrap(),
                        target,
                    )
                    .is_err(),
                "library {target:?}: {label}"
            );
            assert_eq!(library.file(index).unwrap().as_bytes(), saved);
            assert!(!library.is_dirty(index), "library {target:?}: {label}");
        }
    }
}

#[test]
fn octave_fuzz_default_replacements_reject_invalid_inputs_atomically_for_both_targets() {
    type PresetReplace = fn(&mut OpaqueH5epPreset, HdFxSlot) -> Result<(), ToneError>;
    type LibraryReplace = fn(&mut HdEditorLibrary, usize, HdFxSlot) -> Result<(), ToneError>;
    let replacements: [(&str, PresetReplace, LibraryReplace); 3] = [
        (
            "Vintage Pre",
            OpaqueH5epPreset::replace_octave_fuzz_with_vintage_pre_defaults,
            HdEditorLibrary::replace_octave_fuzz_with_vintage_pre_defaults,
        ),
        (
            "Screamer",
            OpaqueH5epPreset::replace_octave_fuzz_with_screamer_defaults,
            HdEditorLibrary::replace_octave_fuzz_with_screamer_defaults,
        ),
        (
            "Sub Octave Fuzz",
            OpaqueH5epPreset::replace_octave_fuzz_with_sub_octave_fuzz_defaults,
            HdEditorLibrary::replace_octave_fuzz_with_sub_octave_fuzz_defaults,
        ),
    ];
    let source = synthetic_octave_fuzz_h5ep(0, true);
    let mut cases = Vec::new();
    for model in [
        HD_TUBE_DRIVE_MODEL_ID,
        HD_SCREAMER_MODEL_ID,
        HD_SUB_OCTAVE_FUZZ_MODEL_ID,
        0x0200_0013, // sentinel model
        HD_FX_LOOP_MODEL_ID,
    ] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET
            ..H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 4]
            .copy_from_slice(&model.to_be_bytes());
        cases.push(("wrong old model", bytes));
    }
    for validity in [0, 2] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0b] = validity;
        cases.push(("validity", bytes));
    }
    let mut invalid_enable = source.clone();
    invalid_enable[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8] = 2;
    cases.push(("enable", invalid_enable));
    for count in [4, 13] {
        let mut bytes = source.clone();
        bytes[H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 0x0f] = count;
        cases.push(("count", bytes));
    }
    let row0 = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + HD_EFFECT_ROWS_OFFSET;
    let mut unknown_prefix = source.clone();
    unknown_prefix[row0..row0 + 4].copy_from_slice(&0xfeed_beefu32.to_be_bytes());
    cases.push(("unknown first-five key", unknown_prefix));
    let mut duplicate_prefix = source.clone();
    let row1 = row0 + HD_PARAMETER_ROW_STRIDE;
    duplicate_prefix[row1..row1 + 4].copy_from_slice(&HD_TYPE1_OUTPUT_KEY.to_be_bytes());
    cases.push(("duplicate prefix", duplicate_prefix));
    let mut invalid_marker = source.clone();
    let marker = H5EP_HEADER_BYTES
        + HD_EFFECT_BLOCK_OFFSET
        + HD_EFFECT_ROWS_OFFSET
        + 11 * HD_PARAMETER_ROW_STRIDE
        + 0x10;
    invalid_marker[marker] = 3;
    cases.push(("assignment marker", invalid_marker));

    for (target, replace_preset, replace_library) in replacements {
        for (label, bytes) in &cases {
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            let before = preset.as_bytes().to_vec();
            assert!(
                replace_preset(&mut preset, HdFxSlot::new(0).unwrap()).is_err(),
                "{target}: {label}"
            );
            assert_eq!(preset.as_bytes(), before, "{target}: {label} raw bytes");

            let mut library = HdEditorLibrary::default();
            let index = library.import_bytes(bytes.clone()).unwrap();
            assert!(!library.is_dirty(index));
            assert!(
                replace_library(&mut library, index, HdFxSlot::new(0).unwrap()).is_err(),
                "{target}: {label} shared library"
            );
            assert!(!library.is_dirty(index), "{target}: {label} dirty state");
            assert_eq!(library.file(index).unwrap().as_bytes(), bytes);
            library.revert_unsaved(index).unwrap();
            assert_eq!(
                library.file(index).unwrap().as_bytes(),
                bytes,
                "{target}: {label} saved baseline"
            );
        }

        let parent_bytes = synthetic_h5es(&[]);
        let mut library = HdEditorLibrary::default();
        let parent = library.import_bytes(parent_bytes.clone()).unwrap();
        assert!(
            replace_library(&mut library, parent, HdFxSlot::new(0).unwrap()).is_err(),
            "{target}: reject container parent"
        );
        assert!(!library.is_dirty(parent), "{target}: parent remains clean");
        assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
    }
}

#[test]
fn ceiling_is_checked_after_valid_magic_and_version_by_parser_and_dispatcher() {
    let mut at_ceiling = synthetic_h5ep(&[]);
    at_ceiling.resize(TONE_BYTES_CEILING as usize, 0x6d);
    let parsed = OpaqueH5epPreset::parse(at_ceiling.clone()).unwrap();
    assert_eq!(parsed.serialize_noop(), at_ceiling);
    let dispatched = PodPresetFile::parse(at_ceiling.clone()).unwrap();
    assert_eq!(dispatched.serialize_noop(), at_ceiling);

    let mut over_ceiling = at_ceiling.clone();
    over_ceiling.push(0x6d);
    for result in [
        OpaqueH5epPreset::parse(over_ceiling.clone()),
        match PodPresetFile::parse(over_ceiling.clone()) {
            Err(error) => Err(error),
            Ok(_) => panic!("oversized H5EP must fail dispatch"),
        },
    ] {
        assert!(matches!(result, Err(ToneError::Format { detail }) if detail.contains("ceiling")));
    }
}

fn synthetic_h5es(extra: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xa7; H5ES_HEADER_BYTES];
    bytes[..4].copy_from_slice(b"H5ES");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    for _ in 0..H5ES_PRESET_COUNT {
        bytes.extend_from_slice(&synthetic_h5ep(&[]));
    }
    bytes.extend_from_slice(extra);
    bytes
}

fn synthetic_h5eb(extra: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x39; H5EB_HEADER_BYTES];
    bytes[..4].copy_from_slice(b"H5EB");
    bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
    for _ in 0..H5EB_SETLIST_COUNT {
        bytes.extend_from_slice(&synthetic_h5es(&[]));
    }
    bytes.extend_from_slice(extra);
    bytes
}

#[test]
fn enable_action_edits_only_extracted_setlist_and_bundle_children() {
    let slot = HdFxSlot::new(0).unwrap();
    let child = synthetic_enable_h5ep(0, 0);

    let mut setlist_bytes = synthetic_h5es(&[]);
    setlist_bytes[H5ES_HEADER_BYTES..H5ES_HEADER_BYTES + H5EP_MIN_BYTES]
        .copy_from_slice(&child[..H5EP_MIN_BYTES]);
    let mut setlist_library = HdEditorLibrary::default();
    let setlist_index = setlist_library.import_bytes(setlist_bytes.clone()).unwrap();
    assert!(
        setlist_library
            .set_effect_enabled(setlist_index, slot, true)
            .is_err()
    );
    let extracted = setlist_library.extract_preset(setlist_index, 0, 0).unwrap();
    setlist_library
        .set_effect_enabled(extracted, slot, true)
        .unwrap();
    assert_eq!(
        setlist_library.file(setlist_index).unwrap().as_bytes(),
        setlist_bytes
    );
    assert!(setlist_library.is_dirty(extracted));
    assert_eq!(
        setlist_library.file(extracted).unwrap().as_bytes()
            [H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8],
        1
    );

    let mut bundle_bytes = synthetic_h5eb(&[]);
    let child_at = H5EB_HEADER_BYTES + H5ES_HEADER_BYTES;
    bundle_bytes[child_at..child_at + H5EP_MIN_BYTES].copy_from_slice(&child[..H5EP_MIN_BYTES]);
    let mut bundle_library = HdEditorLibrary::default();
    let bundle_index = bundle_library.import_bytes(bundle_bytes.clone()).unwrap();
    assert!(
        bundle_library
            .set_effect_enabled(bundle_index, slot, true)
            .is_err()
    );
    let extracted = bundle_library.extract_preset(bundle_index, 0, 0).unwrap();
    bundle_library
        .set_effect_enabled(extracted, slot, true)
        .unwrap();
    assert_eq!(
        bundle_library.file(bundle_index).unwrap().as_bytes(),
        bundle_bytes
    );
    assert!(bundle_library.is_dirty(extracted));
    assert_eq!(
        bundle_library.file(extracted).unwrap().as_bytes()
            [H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 8],
        1
    );
}

#[test]
fn h5es_and_h5eb_parse_fixed_nested_records_and_preserve_tails() {
    let setlist_bytes = synthetic_h5es(b"setlist tail");
    assert_eq!(setlist_bytes.len(), H5ES_MIN_BYTES + 12);
    let dispatched = PodPresetFile::parse(setlist_bytes.clone()).unwrap();
    let PodPresetFile::H5es(setlist) = dispatched else {
        panic!("H5ES should dispatch to a setlist");
    };
    assert_eq!(setlist.preset_bytes(0).unwrap().len(), H5EP_MIN_BYTES);
    assert_eq!(setlist.preset_bytes(63).unwrap().len(), H5EP_MIN_BYTES);
    assert!(setlist.preset_bytes(64).is_none());
    assert_eq!(setlist.trailing_bytes(), b"setlist tail");
    assert_eq!(setlist.as_bytes(), setlist_bytes);

    let bundle_bytes = synthetic_h5eb(b"bundle tail");
    assert_eq!(bundle_bytes.len(), H5EB_MIN_BYTES + 11);
    let dispatched = PodPresetFile::parse(bundle_bytes.clone()).unwrap();
    let PodPresetFile::H5eb(bundle) = dispatched else {
        panic!("H5EB should dispatch to a bundle");
    };
    assert_eq!(bundle.setlist_bytes(0).unwrap().len(), H5ES_MIN_BYTES);
    assert_eq!(bundle.setlist_bytes(7).unwrap().len(), H5ES_MIN_BYTES);
    assert!(bundle.setlist_bytes(8).is_none());
    assert_eq!(bundle.trailing_bytes(), b"bundle tail");
    assert_eq!(bundle.as_bytes(), bundle_bytes);
}

#[test]
fn h5es_and_h5eb_reject_bad_nested_tags_versions_truncation_and_caps() {
    let setlist = synthetic_h5es(&[]);
    assert!(H5esSetlist::parse(setlist[..H5ES_MIN_BYTES - 1].to_vec()).is_err());
    let mut bad_child = setlist.clone();
    bad_child[H5ES_HEADER_BYTES..H5ES_HEADER_BYTES + 4].copy_from_slice(b"H5EB");
    assert!(H5esSetlist::parse(bad_child).is_err());
    let mut bad_version = setlist;
    bad_version[7] = 2;
    assert!(H5esSetlist::parse(bad_version).is_err());

    let bundle = synthetic_h5eb(&[]);
    assert!(H5ebBundle::parse(bundle[..H5EB_MIN_BYTES - 1].to_vec()).is_err());
    let mut bad_nested_setlist = bundle.clone();
    bad_nested_setlist[H5EB_HEADER_BYTES..H5EB_HEADER_BYTES + 4].copy_from_slice(b"H5EP");
    assert!(H5ebBundle::parse(bad_nested_setlist).is_err());
    let mut over_cap = bundle;
    over_cap.resize(HD_BUNDLE_BYTES_CEILING + 1, 0);
    assert!(
        matches!(H5ebBundle::parse(over_cap), Err(ToneError::Format { detail }) if detail.contains("ceiling"))
    );
}

#[test]
fn hd_editor_library_tracks_edits_and_extracts_children_without_changing_parents() {
    let mut bytes = synthetic_h5ep(&[]);
    let body = H5EP_HEADER_BYTES;
    let block = HD_EFFECT_BLOCK_OFFSET;
    bytes[body + block..body + block + 4].copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
    let row = body + block + HD_EFFECT_ROWS_OFFSET;
    bytes[row..row + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
    bytes[row + 4..row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());

    let mut library = HdEditorLibrary::default();
    let preset = library.import_bytes(bytes.clone()).unwrap();
    assert_eq!(library.file(preset).unwrap().kind(), "H5EP preset");
    assert!(!library.any_dirty());
    let Some(HdEditFile::H5ep(preset_file)) = library.file_mut(preset) else {
        panic!("expected H5EP");
    };
    assert_eq!(preset_file.screamer_drive_percent(0), Some(25));
    preset_file.set_screamer_drive_percent(0, 40).unwrap();
    assert!(library.is_dirty(preset));
    library.note_saved(preset).unwrap();
    assert!(!library.is_dirty(preset));

    let parent_bytes = synthetic_h5es(b"tail");
    let parent = library.import_bytes(parent_bytes.clone()).unwrap();
    let child = library.extract_preset(parent, 0, 17).unwrap();
    assert_ne!(parent, child);
    assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
    let start = H5ES_HEADER_BYTES + 17 * H5EP_MIN_BYTES;
    assert_eq!(
        library.file(child).unwrap().as_bytes(),
        &parent_bytes[start..start + H5EP_MIN_BYTES]
    );
    assert!(!library.any_dirty());
}

#[test]
fn vintage_pre_replacement_operates_on_extracted_setlist_and_bundle_children_only() {
    let slot = HdFxSlot::new(3).unwrap();
    let source_child = synthetic_octave_fuzz_h5ep(slot.index(), false);
    let mut expected_child = OpaqueH5epPreset::parse(source_child.clone()).unwrap();
    expected_child
        .replace_octave_fuzz_with_vintage_pre_defaults(slot)
        .unwrap();

    let mut setlist = synthetic_h5es(&[]);
    let setlist_child_at = H5ES_HEADER_BYTES + 23 * H5EP_MIN_BYTES;
    setlist[setlist_child_at..setlist_child_at + H5EP_MIN_BYTES].copy_from_slice(&source_child);
    let mut library = HdEditorLibrary::default();
    let parent = library.import_bytes(setlist.clone()).unwrap();
    let child = library.extract_preset(parent, 0, 23).unwrap();
    assert!(!library.is_dirty(parent));
    library
        .replace_octave_fuzz_with_vintage_pre_defaults(child, slot)
        .unwrap();
    assert_eq!(library.file(parent).unwrap().as_bytes(), setlist);
    assert_eq!(
        library.file(child).unwrap().as_bytes(),
        expected_child.as_bytes()
    );
    assert!(!library.is_dirty(parent));
    assert!(library.is_dirty(child));
    library.revert_unsaved(child).unwrap();
    assert_eq!(library.file(child).unwrap().as_bytes(), source_child);
    assert!(!library.is_dirty(child));

    let mut bundle = synthetic_h5eb(&[]);
    let bundle_child_at =
        H5EB_HEADER_BYTES + 6 * H5ES_MIN_BYTES + H5ES_HEADER_BYTES + 57 * H5EP_MIN_BYTES;
    bundle[bundle_child_at..bundle_child_at + H5EP_MIN_BYTES].copy_from_slice(&source_child);
    let bundle_original = bundle.clone();
    let parent = library.import_bytes(bundle).unwrap();
    let child = library.extract_preset(parent, 6, 57).unwrap();
    library
        .replace_octave_fuzz_with_vintage_pre_defaults(child, slot)
        .unwrap();
    assert_eq!(library.file(parent).unwrap().as_bytes(), bundle_original);
    assert_eq!(
        library.file(child).unwrap().as_bytes(),
        expected_child.as_bytes()
    );
    assert!(!library.is_dirty(parent));
    assert!(library.is_dirty(child));
}

#[test]
fn sub_octave_fuzz_replacement_edits_only_the_extracted_bundle_child() {
    let slot = HdFxSlot::new(7).unwrap();
    let source_child = synthetic_octave_fuzz_h5ep(slot.index(), false);
    let mut expected_child = OpaqueH5epPreset::parse(source_child.clone()).unwrap();
    expected_child
        .replace_octave_fuzz_with_sub_octave_fuzz_defaults(slot)
        .unwrap();

    let mut bundle = synthetic_h5eb(b"bundle tail");
    let parent_child_at =
        H5EB_HEADER_BYTES + 7 * H5ES_MIN_BYTES + H5ES_HEADER_BYTES + 63 * H5EP_MIN_BYTES;
    bundle[parent_child_at..parent_child_at + H5EP_MIN_BYTES].copy_from_slice(&source_child);
    let parent_bytes = bundle.clone();
    let mut library = HdEditorLibrary::default();
    let parent = library.import_bytes(bundle).unwrap();
    let child = library.extract_preset(parent, 7, 63).unwrap();
    library
        .replace_octave_fuzz_with_sub_octave_fuzz_defaults(child, slot)
        .unwrap();

    assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
    assert_eq!(
        library.file(child).unwrap().as_bytes(),
        expected_child.as_bytes()
    );
    assert!(!library.is_dirty(parent));
    assert!(library.is_dirty(child));
    library.revert_unsaved(child).unwrap();
    assert_eq!(library.file(child).unwrap().as_bytes(), source_child);
    assert!(!library.is_dirty(child));
    assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
}

#[test]
fn hd_editor_input_reader_checks_regular_file_and_growth_bounds() {
    let called = std::cell::Cell::new(false);
    assert!(
        read_regular_hd_editor_file(
            Ok(PathFacts {
                regular_file: false,
                len: 0,
            }),
            || {
                called.set(true);
                Ok(Cursor::new(vec![]))
            }
        )
        .is_err()
    );
    assert!(!called.get());
    assert!(
        read_regular_hd_editor_file(
            Ok(PathFacts {
                regular_file: true,
                len: (HD_BUNDLE_BYTES_CEILING + 1) as u64,
            }),
            || {
                called.set(true);
                Ok(Cursor::new(vec![]))
            }
        )
        .is_err()
    );
    assert!(!called.get());
    assert_eq!(
        read_regular_hd_editor_file(
            Ok(PathFacts {
                regular_file: true,
                len: 3,
            }),
            || Ok(Cursor::new(vec![1, 2, 3])),
        )
        .unwrap(),
        [1, 2, 3]
    );

    let consumed = Rc::new(Cell::new(0));
    let error = read_regular_hd_editor_file(
        Ok(PathFacts {
            regular_file: true,
            len: 1,
        }),
        || Ok(ContinuousReader(Rc::clone(&consumed))),
    )
    .unwrap_err();
    assert!(error.detail().contains("ceiling"));
    assert_eq!(consumed.get(), HD_BUNDLE_BYTES_CEILING + 1);
}

#[test]
fn h5ep_body_conversion_is_involutive_and_known_drive_edit_is_narrow() {
    let mut original = synthetic_h5ep(b"opaque tail");
    let body_start = H5EP_HEADER_BYTES;
    let block = HD_EFFECT_BLOCK_OFFSET;
    original[body_start + block..body_start + block + 4]
        .copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
    let row0 = body_start + block + HD_EFFECT_ROWS_OFFSET;
    original[row0..row0 + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
    original[row0 + 4..row0 + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
    original[row0 + 0x10..row0 + 0x14].copy_from_slice(&[0x00, 0xde, 0xad, 0xbe]);
    let row1 = row0 + HD_PARAMETER_ROW_STRIDE;
    original[row1..row1 + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
    original[row1 + 4..row1 + 8].copy_from_slice(&0.75f32.to_bits().to_be_bytes());
    let file = OpaqueH5epPreset::parse(original.clone()).unwrap();
    assert_eq!(file.screamer_drive_percent(0), Some(50));

    let disk_body = file.body_bytes().to_vec();
    let mut transformed = disk_body.clone();
    transform_hd_body_words(&mut transformed);
    transform_hd_body_words(&mut transformed);
    assert_eq!(transformed, disk_body);
    let editor_body = file.body_in_editor_order();
    let mut round_trip_body = editor_body;
    transform_hd_body_words(&mut round_trip_body);
    assert_eq!(round_trip_body, disk_body);
    assert_eq!(file.serialize_noop(), original);

    let mut edited = file;
    for percent in [0, 50, 100] {
        edited.set_screamer_drive_percent(0, percent).unwrap();
        assert_eq!(edited.screamer_drive_percent(0), Some(percent));
    }
    let mut expected = original.clone();
    expected[row0 + 4..row0 + 8].copy_from_slice(&1.0f32.to_bits().to_be_bytes());
    assert_eq!(edited.as_bytes(), expected);
    assert_eq!(
        &edited.as_bytes()[row0 + 0x10..row0 + 0x14],
        &[0, 0xde, 0xad, 0xbe]
    );
    let before = edited.as_bytes().to_vec();
    assert!(edited.set_screamer_drive_percent(0, 101).is_err());
    assert!(edited.set_screamer_drive_percent(8, 50).is_err());
    assert_eq!(edited.as_bytes(), before);

    let mut non_grid = original.clone();
    non_grid[row0 + 4..row0 + 8].copy_from_slice(&0.123_456_7f32.to_bits().to_be_bytes());
    let mut non_grid = OpaqueH5epPreset::parse(non_grid).unwrap();
    assert_eq!(non_grid.screamer_drive_percent(0), None);
    let non_grid_before = non_grid.as_bytes().to_vec();
    assert!(non_grid.set_screamer_drive_percent(0, 12).is_err());
    assert_eq!(non_grid.as_bytes(), non_grid_before);
}

#[test]
fn admitted_primary_type1_controls_edit_only_their_value_word() {
    for (model, controls) in [
        (HD_SCREAMER_MODEL_ID, &HdType1Control::ALL[..5]),
        (HD_TUBE_DRIVE_MODEL_ID, &HdType1Control::ALL[5..10]),
        (HD_OCTAVE_FUZZ_MODEL_ID, &HdType1Control::ALL[10..15]),
        (HD_BAND_SHIFT_EQ_MODEL_ID, &HdType1Control::ALL[15..16]),
        (HD_STEREO_DELAY_MODEL_ID, &HdType1Control::ALL[16..19]),
        (HD_FX_LOOP_MODEL_ID, &HdType1Control::ALL[19..20]),
    ] {
        let mut original = synthetic_h5ep(&[]);
        let body_start = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        original[body_start + block..body_start + block + 4].copy_from_slice(&model.to_be_bytes());
        let first_row = body_start + block + HD_EFFECT_ROWS_OFFSET;
        for (index, control) in controls.iter().copied().enumerate() {
            let row = first_row + index * HD_PARAMETER_ROW_STRIDE;
            original[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
            original[row + 4..row + 8]
                .copy_from_slice(&((index as u32 + 10) as f32 / 100.0).to_bits().to_be_bytes());
            if model == HD_OCTAVE_FUZZ_MODEL_ID && (1..=3).contains(&index) {
                original[row + 8..row + 16]
                    .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
                original[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);
            }
            original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
        }
        let mut preset = OpaqueH5epPreset::parse(original).unwrap();

        for (index, control) in controls.iter().copied().enumerate() {
            assert_eq!(
                preset.hd_type1_control_percent(0, control),
                Some(10 + index as u8)
            );
            let before = preset.as_bytes().to_vec();
            preset.set_hd_type1_control_percent(0, control, 73).unwrap();
            assert_eq!(preset.hd_type1_control_percent(0, control), Some(73));
            let value_at = first_row + index * HD_PARAMETER_ROW_STRIDE + 4;
            let mut expected = before;
            expected[value_at..value_at + 4].copy_from_slice(&0.73f32.to_bits().to_be_bytes());
            assert_eq!(preset.as_bytes(), expected);
        }
    }
}

#[test]
fn selected_effect_type1_descriptors_are_model_gated_and_write_only_the_value_word() {
    assert_eq!(HD_TYPE1_MAPPED_CONTROLS.len(), 364);
    let mut source_rows = std::collections::BTreeSet::new();
    for control in HD_TYPE1_MAPPED_CONTROLS {
        assert!(
            source_rows.insert((control.model_id, control.key)),
            "duplicate source row for {} {}",
            control.model_label,
            control.label
        );
    }
    for (model_id, label) in [
        (HD_SCREAMER_MODEL_ID, "TONE"),
        (HD_TUBE_DRIVE_MODEL_ID, "MID"),
        (HD_OCTAVE_FUZZ_MODEL_ID, "MID"),
    ] {
        assert_eq!(
            HD_TYPE1_MAPPED_CONTROLS
                .iter()
                .filter(|control| (control.model_id, control.label) == (model_id, label))
                .count(),
            1
        );
    }
    for control in HD_TYPE1_MAPPED_CONTROLS {
        let mut original = synthetic_h5ep(&[]);
        let body = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        original[body + block..body + block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let row = body + block + HD_EFFECT_ROWS_OFFSET;
        original[row..row + 4].copy_from_slice(&control.key.to_be_bytes());
        original[row + 4..row + 8].copy_from_slice(&0.37f32.to_bits().to_be_bytes());
        original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;

        let mut preset = OpaqueH5epPreset::parse(original).unwrap();
        assert_eq!(preset.hd_type1_mapped_control_percent(0, control), Some(37));
        let before = preset.as_bytes().to_vec();
        preset
            .set_hd_type1_mapped_control_percent(0, control, 73)
            .unwrap();
        assert_eq!(preset.hd_type1_mapped_control_percent(0, control), Some(73));
        let mut expected = before;
        expected[row + 4..row + 8].copy_from_slice(&0.73f32.to_bits().to_be_bytes());
        assert_eq!(
            preset.as_bytes(),
            expected,
            "{} {}",
            control.model_label(),
            control.label()
        );

        let wrong_model_id = if control.model_id() == HD_SCREAMER_MODEL_ID {
            HD_TUBE_DRIVE_MODEL_ID
        } else {
            HD_SCREAMER_MODEL_ID
        };
        let mut wrong_model = preset.clone();
        wrong_model.bytes[body + block..body + block + 4]
            .copy_from_slice(&wrong_model_id.to_be_bytes());
        let before_wrong_model = wrong_model.as_bytes().to_vec();
        assert_eq!(
            wrong_model.hd_type1_mapped_control_percent(0, control),
            None
        );
        assert!(
            wrong_model
                .set_hd_type1_mapped_control_percent(0, control, 50)
                .is_err()
        );
        assert_eq!(wrong_model.as_bytes(), before_wrong_model);

        let mut wrong_key = preset.clone();
        wrong_key.bytes[row..row + 4].copy_from_slice(&(control.key() ^ 0x8000_0000).to_be_bytes());
        let before_wrong_key = wrong_key.as_bytes().to_vec();
        assert_eq!(wrong_key.hd_type1_mapped_control_percent(0, control), None);
        assert!(
            wrong_key
                .set_hd_type1_mapped_control_percent(0, control, 50)
                .is_err()
        );
        assert_eq!(wrong_key.as_bytes(), before_wrong_key);

        let before_invalid = preset.as_bytes().to_vec();
        assert!(
            preset
                .set_hd_type1_mapped_control_percent(0, control, 101)
                .is_err()
        );
        assert_eq!(preset.as_bytes(), before_invalid);

        let mut assigned = preset.clone();
        assigned.bytes[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        let before_assigned = assigned.as_bytes().to_vec();
        assert_eq!(assigned.hd_type1_mapped_control_percent(0, control), None);
        assert!(
            assigned
                .set_hd_type1_mapped_control_percent(0, control, 50)
                .is_err()
        );
        assert_eq!(assigned.as_bytes(), before_assigned);
    }
}

#[test]
fn selected_non_type1_controls_use_their_source_integer_grid_and_preserve_other_bytes() {
    assert_eq!(HD_NUMERIC_CONTROLS.len(), 80);
    let unique_rows = HD_NUMERIC_CONTROLS
        .iter()
        .map(|control| (control.model_id(), control.key()))
        .collect::<std::collections::BTreeSet<_>>();
    let unique_models = HD_NUMERIC_CONTROLS
        .iter()
        .map(|control| control.model_id())
        .collect::<std::collections::BTreeSet<_>>();
    let unique_tags = HD_NUMERIC_CONTROLS
        .iter()
        .map(|control| control.raw_type())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(unique_rows.len(), 80);
    assert_eq!(unique_models.len(), 65);
    assert_eq!(unique_tags.len(), 17);
    for control in HD_NUMERIC_CONTROLS {
        assert_ne!(control.raw_type(), 0x01);
        if control.raw_type() == 0x1b {
            assert_eq!(
                (
                    control.model_id(),
                    control.key(),
                    control.min(),
                    control.max()
                ),
                (0x020c_000e, 0x3f10_0005, 0, 15_000)
            );
        }
        let mut original = synthetic_h5ep(&[]);
        let body = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        original[body + block..body + block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let row = body + block + HD_EFFECT_ROWS_OFFSET;
        original[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
        let initial = control.min() + (control.max() - control.min()) / 3;
        let encoded = normalized_numeric_value(initial, control.min(), control.max());
        original[row + 4..row + 8].copy_from_slice(&encoded.to_bits().to_be_bytes());
        original[row + 8..row + 16]
            .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
        original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
        original[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);

        let mut preset = OpaqueH5epPreset::parse(original).unwrap();
        assert_eq!(
            preset.hd_numeric_control_value(0, control),
            Some(initial),
            "{} {}",
            control.model_label(),
            control.label()
        );
        let before = preset.as_bytes().to_vec();
        preset
            .set_hd_numeric_control_value(0, control, control.max())
            .unwrap();
        assert_eq!(
            preset.hd_numeric_control_value(0, control),
            Some(control.max())
        );
        let mut expected = before;
        let value_at = row + 4;
        expected[value_at..value_at + 4].copy_from_slice(
            &normalized_numeric_value(control.max(), control.min(), control.max())
                .to_bits()
                .to_be_bytes(),
        );
        assert_eq!(preset.as_bytes(), expected);

        let before_rejection = preset.as_bytes().to_vec();
        assert!(
            preset
                .set_hd_numeric_control_value(0, control, control.max() + 1)
                .is_err()
        );
        assert_eq!(preset.as_bytes(), before_rejection);

        let mut wrong_model = preset.clone();
        let wrong_model_id = if control.model_id() == HD_GRAPHIC_EQ_MODEL_ID {
            HD_STUDIO_EQ_MODEL_ID
        } else {
            HD_GRAPHIC_EQ_MODEL_ID
        };
        let wrong_model_at = H5EP_HEADER_BYTES + block;
        wrong_model.bytes[wrong_model_at..wrong_model_at + 4]
            .copy_from_slice(&wrong_model_id.to_be_bytes());
        let before_wrong_model = wrong_model.as_bytes().to_vec();
        assert_eq!(wrong_model.hd_numeric_control_value(0, control), None);
        assert!(
            wrong_model
                .set_hd_numeric_control_value(0, control, initial)
                .is_err()
        );
        assert_eq!(wrong_model.as_bytes(), before_wrong_model);

        let mut flagged = preset.clone();
        flagged.bytes[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        let before_flagged = flagged.as_bytes().to_vec();
        assert_eq!(flagged.hd_numeric_control_value(0, control), None);
        assert!(
            flagged
                .set_hd_numeric_control_value(0, control, initial)
                .is_err()
        );
        assert_eq!(flagged.as_bytes(), before_flagged);

        let mut off_grid = preset.clone();
        off_grid.bytes[row + 4..row + 8].copy_from_slice(&0.12345f32.to_bits().to_be_bytes());
        let before_off_grid = off_grid.as_bytes().to_vec();
        assert_eq!(off_grid.hd_numeric_control_value(0, control), None);
        assert!(
            off_grid
                .set_hd_numeric_control_value(0, control, initial)
                .is_err()
        );
        assert_eq!(off_grid.as_bytes(), before_off_grid);
    }
}

#[test]
fn vintage_pre_type1b_uses_fixed_source_words_and_patches_only_first_row() {
    const FIXTURES: &[(i32, u32)] = &[
        (0, 0x0000_0000),
        (1, 0x388b_cf65),
        (2, 0x390b_cf65),
        (3, 0x3951_b717),
        (7_499, 0x3eff_f743),
        (7_500, 0x3f00_0000),
        (7_501, 0x3f00_045e),
        (14_999, 0x3f7f_fba2),
        (15_000, 0x3f80_0000),
    ];
    let control = HD_NUMERIC_CONTROLS
        .iter()
        .copied()
        .find(|control| control.raw_type() == 0x1b)
        .expect("admitted primary Vintage Pre control");
    assert_eq!(control.model_id(), 0x020c_000e);
    assert_eq!(control.key(), 0x3f10_0005);
    assert_eq!((control.min(), control.max()), (0, 15_000));

    // These words were independently rounded from the source's integer
    // divide and DWORD float store for the exact 0..15000 / 0..1 path.
    // The expectations are literals, not values generated by the helper.
    for &(integer, word) in FIXTURES {
        assert_eq!(
            normalized_numeric_value(integer, 0, 15_000).to_bits(),
            word,
            "source word for editor integer {integer}"
        );
    }

    let mut original = synthetic_h5ep(&[]);
    let body = H5EP_HEADER_BYTES;
    let block = HD_EFFECT_BLOCK_OFFSET;
    original[body + block..body + block + 4].copy_from_slice(&control.model_id().to_be_bytes());
    let first_row = body + block + HD_EFFECT_ROWS_OFFSET;
    original[first_row..first_row + 4].copy_from_slice(&control.key().to_be_bytes());
    original[first_row + 4..first_row + 8].copy_from_slice(&FIXTURES[5].1.to_be_bytes());
    original[first_row + 8..first_row + 16]
        .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
    original[first_row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
    original[first_row + 17..first_row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);
    let duplicate = first_row + HD_PARAMETER_ROW_STRIDE;
    original[duplicate..duplicate + 4].copy_from_slice(&control.key().to_be_bytes());
    original[duplicate + 4..duplicate + 8].copy_from_slice(&FIXTURES[6].1.to_be_bytes());
    original[duplicate + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;

    for &(integer, word) in FIXTURES {
        let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
        assert_eq!(preset.hd_numeric_control_value(0, control), Some(7_500));
        preset
            .set_hd_numeric_control_value(0, control, integer)
            .unwrap();

        let mut expected = original.clone();
        expected[first_row + 4..first_row + 8].copy_from_slice(&word.to_be_bytes());
        assert_eq!(preset.as_bytes(), expected, "editor integer {integer}");
        assert_eq!(preset.hd_numeric_control_value(0, control), Some(integer));
        assert_eq!(
            &preset.as_bytes()[duplicate + 4..duplicate + 8],
            &FIXTURES[6].1.to_be_bytes(),
            "a later duplicate must remain unchanged"
        );
    }
}

#[test]
fn vintage_pre_type1b_rejects_unadmitted_rows_and_values_transactionally() {
    let control = HD_NUMERIC_CONTROLS
        .iter()
        .copied()
        .find(|control| control.raw_type() == 0x1b)
        .unwrap();
    let body = H5EP_HEADER_BYTES;
    let block = HD_EFFECT_BLOCK_OFFSET;
    let row = body + block + HD_EFFECT_ROWS_OFFSET;
    let mut valid = synthetic_h5ep(&[]);
    valid[body + block..body + block + 4].copy_from_slice(&control.model_id().to_be_bytes());
    valid[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
    valid[row + 4..row + 8].copy_from_slice(&0x3f00_0000u32.to_be_bytes());
    valid[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;

    let reject_unchanged = |bytes: Vec<u8>, slot: usize, value: i32| {
        let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
        let before = preset.as_bytes().to_vec();
        assert!(
            preset
                .set_hd_numeric_control_value(slot, control, value)
                .is_err()
        );
        assert_eq!(preset.as_bytes(), before);
    };
    reject_unchanged(valid.clone(), HD_EFFECT_COUNT, 12_000);
    reject_unchanged(valid.clone(), 0, -1);
    reject_unchanged(valid.clone(), 0, 15_001);

    let wrong_rows: [(usize, u32); 5] = [
        (body + block, 0x020c_000d),
        (row, 0x3f10_0004),
        (row + 4, 0x7fc0_0000),
        (row + 4, 0x7f80_0000),
        (row + 4, 0x3f00_0001),
    ];
    for (offset, word) in wrong_rows {
        let mut bytes = valid.clone();
        bytes[offset..offset + 4].copy_from_slice(&word.to_be_bytes());
        reject_unchanged(bytes, 0, 12_000);
    }
    let mut assigned = valid;
    assigned[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    reject_unchanged(assigned, 0, 12_000);
}

#[test]
fn non_type1_source_word_fixtures_pin_each_normalization_family() {
    // Independently transcribed from the admitted Edit 2.27 routines in
    // non-type1-converter-and-display-routines.asm (image SHA-256
    // 871c86da6b6810307dc201297d8411f2b2f7eec7e4c3e150f0aa881f8c89afe2).
    // The common routine subtracts integer min/max in 32-bit registers,
    // FILDs the numerator, FIDIVs by the integer width, maps the ratio
    // through the configured 0..1 endpoints, then FSTPs a DWORD. Tag 0x10
    // has its own converter with the same observed endpoint mapping.
    // Expected words below are fixed source-contract fixtures; neither
    // input nor output expectations are generated by the production encoder.
    const FIXTURES: &[(u8, i32, i32, i32, u32, i32, u32)] = &[
        (0x05, 0, 240, 73, 0x3e9b_bbbc, 167, 0x3f32_2222),
        (0x06, 0, 220, 73, 0x3ea9_e413, 147, 0x3f2b_0df7),
        (0x07, 0, 360, 127, 0x3eb4_9f4a, 247, 0x3f2f_a4fa),
        (0x08, -96, 0, -53, 0x3ee5_5555, -32, 0x3f2a_aaab),
        (0x09, -80, 0, -53, 0x3eac_cccd, -27, 0x3f29_999a),
        (0x0a, 0, 24, 7, 0x3e95_5555, 17, 0x3f35_5555),
        (0x0d, 0, 1400, 467, 0x3eaa_c9e0, 933, 0x3f2a_9b10),
        (0x0e, 0, 1400, 523, 0x3ebf_44c1, 1001, 0x3f37_0a3d),
        (0x0f, 0, 3520, 1107, 0x3ea1_04a8, 2347, 0x3f2a_b0df),
        (0x10, 20, 2000, 680, 0x3eaa_aaab, 1320, 0x3f28_14b0),
        (0x11, 0, 200, 61, 0x3e9c_28f6, 137, 0x3f2f_5c29),
        (0x12, 0, 800, 267, 0x3eaa_e148, 533, 0x3f2a_8f5c),
        (0x13, 0, 4000, 1337, 0x3eab_22d1, 2667, 0x3f2a_b021),
        (0x14, -240, 240, -81, 0x3ea9_999a, 80, 0x3f2a_aaab),
        (0x16, 0, 200, 73, 0x3eba_e148, 129, 0x3f25_1eb8),
        (0x1a, 20, 500, 181, 0x3eab_bbbc, 340, 0x3f2a_aaab),
        (0x1b, 0, 15_000, 7_500, 0x3f00_0000, 14_999, 0x3f7f_fba2),
    ];

    for &(raw_type, min, max, initial, initial_word, edited, edited_word) in FIXTURES {
        let control = HD_NUMERIC_CONTROLS
            .iter()
            .find(|control| {
                control.raw_type() == raw_type && control.min() == min && control.max() == max
            })
            .unwrap_or_else(|| panic!("missing admitted fixture family {raw_type:#04x}"));
        let mut original = synthetic_h5ep(&[]);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
        original[block..block + 4].copy_from_slice(&control.model_id().to_be_bytes());
        let row = block + HD_EFFECT_ROWS_OFFSET;
        original[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
        original[row + 4..row + 8].copy_from_slice(&initial_word.to_be_bytes());
        original[row + 8..row + 16]
            .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
        original[row + HD_PARAMETER_ROW_FLAG_OFFSET] = 0;
        original[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);

        let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
        assert_eq!(preset.hd_numeric_control_value(0, *control), Some(initial));
        preset
            .set_hd_numeric_control_value(0, *control, edited)
            .unwrap();

        let mut expected = original;
        expected[row + 4..row + 8].copy_from_slice(&edited_word.to_be_bytes());
        assert_eq!(preset.as_bytes(), expected, "raw family {raw_type:#04x}");

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fixture.h5p");
        preset.save_copy(&path).unwrap();
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(saved, expected, "saved raw family {raw_type:#04x}");
        let reparsed = OpaqueH5epPreset::parse(saved).unwrap();
        assert_eq!(reparsed.hd_numeric_control_value(0, *control), Some(edited));
        assert_eq!(
            &reparsed.as_bytes()[row + 4..row + 8],
            &edited_word.to_be_bytes()
        );
    }
}

#[test]
fn screamer_and_tube_drive_contracts_keep_model_and_third_control_distinct() {
    for (model, controls, selected_label, selected_index) in [
        (
            HD_SCREAMER_MODEL_ID,
            HdType1Control::ALL[..5].to_vec(),
            "TONE",
            2,
        ),
        (
            HD_TUBE_DRIVE_MODEL_ID,
            HdType1Control::ALL[5..10].to_vec(),
            "MID",
            2,
        ),
        (
            HD_OCTAVE_FUZZ_MODEL_ID,
            HdType1Control::ALL[10..15].to_vec(),
            "MID",
            2,
        ),
        (
            HD_BAND_SHIFT_EQ_MODEL_ID,
            HdType1Control::ALL[15..16].to_vec(),
            "SHIFT",
            0,
        ),
        (
            HD_STEREO_DELAY_MODEL_ID,
            HdType1Control::ALL[16..19].to_vec(),
            "MIX",
            2,
        ),
        (
            HD_FX_LOOP_MODEL_ID,
            HdType1Control::ALL[19..20].to_vec(),
            "MIX",
            0,
        ),
    ] {
        let mut bytes = synthetic_h5ep(&[]);
        let body = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        bytes[body + block..body + block + 4].copy_from_slice(&model.to_be_bytes());
        let first = body + block + HD_EFFECT_ROWS_OFFSET;
        for (index, control) in controls.iter().copied().enumerate() {
            let row = first + index * HD_PARAMETER_ROW_STRIDE;
            bytes[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
            bytes[row + 4..row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());
            if model == HD_OCTAVE_FUZZ_MODEL_ID && (1..=3).contains(&index) {
                bytes[row + 8..row + 16]
                    .copy_from_slice(&[0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xb3, 0xb4]);
                bytes[row + 17..row + 20].copy_from_slice(&[0xc1, 0xc2, 0xc3]);
            }
        }
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        let selected = controls[selected_index];
        assert_eq!(selected.label(), selected_label);
        assert_eq!(preset.hd_type1_control_percent(0, selected), Some(25));
        let before = preset.as_bytes().to_vec();
        preset
            .set_hd_type1_control_percent(0, selected, 61)
            .unwrap();
        assert_eq!(preset.hd_type1_control_percent(0, selected), Some(61));
        let mut expected = before;
        let value = first + selected_index * HD_PARAMETER_ROW_STRIDE + 4;
        expected[value..value + 4].copy_from_slice(&0.61f32.to_bits().to_be_bytes());
        assert_eq!(preset.as_bytes(), expected);

        let wrong_model_control = match model {
            HD_SCREAMER_MODEL_ID => HdType1Control::TubeDrive(TubeDriveControl::Mid),
            HD_TUBE_DRIVE_MODEL_ID => HdType1Control::OctaveFuzz(OctaveFuzzControl::Mid),
            HD_OCTAVE_FUZZ_MODEL_ID => HdType1Control::BandShiftEq(BandShiftEqControl::Shift),
            HD_BAND_SHIFT_EQ_MODEL_ID => {
                HdType1Control::StereoDelay(StereoDelayControl::LeftFeedback)
            }
            HD_STEREO_DELAY_MODEL_ID => HdType1Control::FxLoop(FxLoopControl::Mix),
            _ => HdType1Control::Screamer(ScreamerControl::Tone),
        };
        assert_eq!(
            preset.hd_type1_control_percent(0, wrong_model_control),
            None
        );
        let unchanged = preset.as_bytes().to_vec();
        assert!(
            preset
                .set_hd_type1_control_percent(0, wrong_model_control, 50)
                .is_err()
        );
        assert_eq!(preset.as_bytes(), unchanged);
    }
}

#[test]
fn screamer_drive_admission_rejects_nonzero_row_flags_and_bad_values_atomically() {
    let mut eligible = synthetic_h5ep(&[]);
    let body_start = H5EP_HEADER_BYTES;
    let block = HD_EFFECT_BLOCK_OFFSET;
    eligible[body_start + block..body_start + block + 4]
        .copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
    let row = body_start + block + HD_EFFECT_ROWS_OFFSET;
    eligible[row..row + 4].copy_from_slice(&HD_TYPE1_DRIVE_KEY.to_be_bytes());
    eligible[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
    eligible[row + HD_PARAMETER_ROW_FLAG_OFFSET..row + HD_PARAMETER_ROW_STRIDE]
        .copy_from_slice(&[0, 0x11, 0x22, 0x33]);

    // Zero is the only admitted row flag; every nonzero value is treated
    // as unresolved assignment state, without changing any row bytes.
    for flag in 1..=u8::MAX {
        let mut bytes = eligible.clone();
        bytes[row + HD_PARAMETER_ROW_FLAG_OFFSET] = flag;
        let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
        assert_eq!(preset.screamer_drive_percent(0), None, "flag {flag:#04x}");
        let before = preset.as_bytes().to_vec();
        assert!(preset.set_screamer_drive_percent(0, 25).is_err());
        assert_eq!(preset.as_bytes(), before, "flag {flag:#04x}");
    }

    let mut missing_key = eligible.clone();
    missing_key[row..row + 4].copy_from_slice(&0x3f10_0002u32.to_be_bytes());
    let mut missing_key = OpaqueH5epPreset::parse(missing_key).unwrap();
    assert_eq!(missing_key.screamer_drive_percent(0), None);
    let before = missing_key.as_bytes().to_vec();
    assert!(missing_key.set_screamer_drive_percent(0, 25).is_err());
    assert_eq!(missing_key.as_bytes(), before);

    for value in [f32::NAN, f32::INFINITY, -0.01, 1.01, 0.123_456_7] {
        let mut bytes = eligible.clone();
        bytes[row + 4..row + 8].copy_from_slice(&value.to_bits().to_be_bytes());
        let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
        assert_eq!(preset.screamer_drive_percent(0), None);
        let before = preset.as_bytes().to_vec();
        assert!(preset.set_screamer_drive_percent(0, 25).is_err());
        assert_eq!(preset.as_bytes(), before);
    }
}

#[test]
fn header_context_byte_is_preserved_but_not_admitted_as_product_identity() {
    for context_byte in [0x2d, 0x2e] {
        let mut bytes = synthetic_h5ep(&[]);
        bytes[0x0b] = context_byte;
        let parsed = PodPresetFile::parse(bytes.clone()).unwrap();
        assert!(matches!(parsed, PodPresetFile::H5ep(_)));
        assert_eq!(parsed.serialize_noop(), bytes);
    }
}

#[test]
fn opaque_h5ep_save_copy_is_atomic_and_does_not_overwrite() {
    let original = synthetic_h5ep(b"tail");
    let file = PodPresetFile::parse(original.clone()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("copy.5xe");
    file.save_copy(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(file.save_copy(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);

    let blocked_dir = directory.path().join("existing-directory");
    std::fs::create_dir(&blocked_dir).unwrap();
    let sentinel = blocked_dir.join("preserve.bin");
    std::fs::write(&sentinel, b"sentinel").unwrap();
    assert!(file.save_copy(&blocked_dir).is_err());
    assert_eq!(file.as_bytes(), original);
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");

    let invalid_destination = directory.path().join("missing-parent").join("copy.5xe");
    assert!(file.save_copy(&invalid_destination).is_err());
    assert!(!invalid_destination.exists());
    assert_eq!(file.as_bytes(), original);
    assert_eq!(file.serialize_noop(), original);
}
#[test]
fn type9_choice_writer_covers_all_file_slots_rows_choices_and_enable_states() {
    let expected_values: [(HdType9Choice, [u8; 4]); 3] = [
        (HdType9Choice::Choice0, [0x00, 0x00, 0x00, 0x00]),
        (HdType9Choice::Choice1, [0x3f, 0x00, 0x00, 0x00]),
        (HdType9Choice::Choice2, [0x3f, 0x80, 0x00, 0x00]),
    ];
    for slot_index in 0..8 {
        for row_index in [0, 5, 11] {
            for enabled in [0, 1] {
                for (choice, expected_word) in expected_values {
                    let original =
                        synthetic_type9_h5ep(slot_index, row_index, enabled, 0x3eaa_aaab);
                    let mut expected = original.clone();
                    let value_at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row_index * HD_PARAMETER_ROW_STRIDE
                        + 4;
                    expected[value_at..value_at + 4].copy_from_slice(&expected_word);
                    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                    preset
                        .set_hd_type9_choice(HdFxSlot::new(slot_index).unwrap(), choice)
                        .unwrap();
                    assert_eq!(preset.as_bytes(), expected);
                    assert_eq!(
                        preset.as_bytes()[value_at - 4..value_at],
                        0x3f10_0004u32.to_be_bytes()
                    );
                    assert_eq!(preset.as_bytes()[value_at + 4], 0x31);
                    assert_eq!(preset.as_bytes()[value_at + 12], 0);
                    assert_eq!(&preset.as_bytes()[H5EP_MIN_BYTES..], b"type9-tail");
                }
            }
        }
    }
    assert!(HdFxSlot::new(8).is_err());

    let mut count_one = synthetic_type9_h5ep(0, 0, 0, 0x3f40_0000);
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
    count_one[block + 0x0f] = 1;
    let mut preset = OpaqueH5epPreset::parse(count_one).unwrap();
    preset
        .set_hd_type9_choice(HdFxSlot::new(0).unwrap(), HdType9Choice::Choice0)
        .unwrap();
    assert_eq!(preset.as_bytes()[block + 0x0f], 1);
}

#[test]
fn type9_choice_catalog_requires_exact_model_key_pairs_and_preserves_other_bytes() {
    for (association_index, (model_id, key)) in HD_TYPE9_CHOICE_MODEL_KEYS.into_iter().enumerate() {
        assert_eq!(hd_type9_choice_key(model_id), Some(key));
        let slot_index = [0, 3, 7, 5][association_index];
        for row_index in [0, 5, 11] {
            for enabled in [0, 1] {
                for choice in HdType9Choice::ALL {
                    let original = synthetic_type9_h5ep_for(
                        slot_index,
                        row_index,
                        enabled,
                        0x3eaa_aaab,
                        model_id,
                        key,
                    );
                    let mut expected = original.clone();
                    let value_at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row_index * HD_PARAMETER_ROW_STRIDE
                        + 4;
                    expected[value_at..value_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
                    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                    let slot = HdFxSlot::new(slot_index).unwrap();
                    assert_eq!(
                        preset.hd_type9_choice_value(slot),
                        HdType9ChoiceValue::UnknownFinite(0x3eaa_aaab)
                    );
                    preset.set_hd_type9_choice(slot, choice).unwrap();
                    assert_eq!(
                        preset.as_bytes(),
                        expected,
                        "{model_id:#010x}/{key:#010x}, row {row_index}, enable {enabled}, choice {:?}",
                        choice
                    );
                    assert_eq!(
                        preset.hd_type9_choice_value(slot),
                        HdType9ChoiceValue::Choice(choice)
                    );
                }
            }
        }
    }

    for (model_id, key) in HD_TYPE9_CHOICE_MODEL_KEYS {
        let make = || synthetic_type9_h5ep_for(0, 0, 0, 0x3f40_0000, model_id, key);
        let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET;
        let row0 = block + HD_EFFECT_ROWS_OFFSET;
        let mut invalid_cases = Vec::new();

        let mut invalid_effect = make();
        invalid_effect[block + 0x0b] = 0;
        invalid_cases.push(("invalid effect", invalid_effect));
        let mut invalid_enable = make();
        invalid_enable[block + 8] = 2;
        invalid_cases.push(("noncanonical enable", invalid_enable));
        let mut invalid_count = make();
        invalid_count[block + 0x0f] = 0;
        invalid_cases.push(("invalid count", invalid_count));
        let mut beyond_count = make();
        beyond_count[row0..row0 + 4].fill(0);
        let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
        beyond_count[row5..row5 + 4].copy_from_slice(&key.to_be_bytes());
        beyond_count[block + 0x0f] = 5;
        invalid_cases.push(("target beyond count", beyond_count));
        let mut duplicate = make();
        let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
        duplicate[row9..row9 + 4].copy_from_slice(&key.to_be_bytes());
        invalid_cases.push(("duplicate key", duplicate));
        let mut assigned = make();
        assigned[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        invalid_cases.push(("assigned marker", assigned));
        let mut nonfinite = make();
        nonfinite[row0 + 4..row0 + 8].copy_from_slice(&0x7fc0_0001u32.to_be_bytes());
        invalid_cases.push(("non-finite existing value", nonfinite));

        for (reason, bytes) in invalid_cases {
            let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
            assert!(
                preset
                    .set_hd_type9_choice(HdFxSlot::new(0).unwrap(), HdType9Choice::Choice1)
                    .is_err(),
                "{model_id:#010x}/{key:#010x}: {reason} must reject"
            );
            assert_eq!(preset.as_bytes(), bytes, "{reason} must be atomic");
        }
    }

    for (model_id, expected_key, wrong_key) in [
        (0x020a_0010, Some(0x3f10_0004), 0x3f10_0003),
        (0x020a_000f, Some(0x3f10_0004), 0x3f10_0003),
        (0x020a_0012, Some(0x3f10_0003), 0x3f10_0004),
        (0x020a_0014, Some(0x3f10_0004), 0x3f10_0003),
        (0x020a_0013, None, 0x3f10_0004),
    ] {
        assert_eq!(hd_type9_choice_key(model_id), expected_key);
        let bytes = synthetic_type9_h5ep_for(2, 4, 0, 0x3f40_0000, model_id, wrong_key);
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert_eq!(
            preset.hd_type9_choice_value(HdFxSlot::new(2).unwrap()),
            HdType9ChoiceValue::Unavailable
        );
        assert!(
            preset
                .set_hd_type9_choice(HdFxSlot::new(2).unwrap(), HdType9Choice::Choice1)
                .is_err()
        );
        assert_eq!(
            preset.as_bytes(),
            bytes,
            "cross-pair {model_id:#010x}/{wrong_key:#010x}"
        );
    }
}

#[test]
fn type9_choice_writer_rejects_invalid_inputs_atomically_and_scans_all_rows() {
    let slot = HdFxSlot::new(5).unwrap();
    let source = synthetic_type9_h5ep(5, 0, 0, 0x3f00_0000);
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
    let row0 = block + HD_EFFECT_ROWS_OFFSET;
    let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();

    let mut wrong_model = source.clone();
    wrong_model[block..block + 4].copy_from_slice(&0x020a_0015u32.to_be_bytes());
    cases.push(("wrong model", wrong_model));
    let mut invalid_validity = source.clone();
    invalid_validity[block + 0x0b] = 0;
    cases.push(("invalid effect", invalid_validity));
    let mut invalid_enable = source.clone();
    invalid_enable[block + 8] = 2;
    cases.push(("invalid enable", invalid_enable));
    for count in [0, 13] {
        let mut invalid_count = source.clone();
        invalid_count[block + 0x0f] = count;
        cases.push(("invalid count", invalid_count));
    }
    let mut target_beyond_count = source.clone();
    let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
    target_beyond_count[row0..row0 + 4].fill(0);
    target_beyond_count[row5..row5 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
    target_beyond_count[block + 0x0f] = 5;
    cases.push(("target beyond count", target_beyond_count));
    let mut missing = source.clone();
    missing[row0..row0 + 4].fill(0);
    cases.push(("missing key", missing));
    for duplicate_value in [0x3f00_0000u32, 0x3f80_0000] {
        let mut duplicate = source.clone();
        let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
        duplicate[row9..row9 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
        duplicate[row9 + 4..row9 + 8].copy_from_slice(&duplicate_value.to_be_bytes());
        duplicate[block + 0x0f] = 1; // duplicate lies beyond the admitted count
        cases.push(("duplicate key beyond count", duplicate));
    }
    let mut assigned = source.clone();
    assigned[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    cases.push(("assigned marker", assigned));
    for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
        let mut non_finite = source.clone();
        non_finite[row0 + 4..row0 + 8].copy_from_slice(&bits.to_be_bytes());
        cases.push(("non-finite current value", non_finite));
    }

    for (label, bytes) in cases {
        let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
        let before = preset.as_bytes().to_vec();
        assert!(
            preset
                .set_hd_type9_choice(slot, HdType9Choice::Choice1)
                .is_err(),
            "{label} must reject"
        );
        assert_eq!(preset.as_bytes(), before, "{label} must be atomic");
    }

    let mut malformed_magic = source.clone();
    malformed_magic[..4].copy_from_slice(b"H5EX");
    assert!(OpaqueH5epPreset::parse(malformed_magic).is_err());
    let mut malformed_version = source.clone();
    malformed_version[4..8].copy_from_slice(&2u32.to_be_bytes());
    assert!(OpaqueH5epPreset::parse(malformed_version).is_err());
    assert!(OpaqueH5epPreset::parse(source[..H5EP_MIN_BYTES - 1].to_vec()).is_err());

    assert!(HdType9Choice::from_index(3).is_err());
    assert!(HdType9Choice::from_index(255).is_err());
}

#[test]
fn type9_choice_read_is_lossless_and_equal_word_noop_still_checks_guards() {
    let slot = HdFxSlot::new(2).unwrap();
    for (bits, expected) in [
        (
            0x0000_0000,
            HdType9ChoiceValue::Choice(HdType9Choice::Choice0),
        ),
        (
            0x3f00_0000,
            HdType9ChoiceValue::Choice(HdType9Choice::Choice1),
        ),
        (
            0x3f80_0000,
            HdType9ChoiceValue::Choice(HdType9Choice::Choice2),
        ),
        (0x3f40_0000, HdType9ChoiceValue::UnknownFinite(0x3f40_0000)),
        (0x7fc0_0001, HdType9ChoiceValue::NonFinite(0x7fc0_0001)),
        (0x7f80_0000, HdType9ChoiceValue::NonFinite(0x7f80_0000)),
        (0xff80_0000, HdType9ChoiceValue::NonFinite(0xff80_0000)),
    ] {
        let bytes = synthetic_type9_h5ep(2, 11, 1, bits);
        let preset = OpaqueH5epPreset::parse(bytes).unwrap();
        assert_eq!(preset.hd_type9_choice_value(slot), expected);
        assert_eq!(preset.serialize_noop(), preset.as_bytes());
    }
    let mut same = OpaqueH5epPreset::parse(synthetic_type9_h5ep(2, 11, 1, 0x3f00_0000)).unwrap();
    let before = same.as_bytes().to_vec();
    same.set_hd_type9_choice(slot, HdType9Choice::Choice1)
        .unwrap();
    assert_eq!(same.as_bytes(), before);

    let mut bad_marker = before.clone();
    let marker = H5EP_HEADER_BYTES
        + HD_EFFECT_BLOCK_OFFSET
        + 2 * HD_EFFECT_BLOCK_STRIDE
        + HD_EFFECT_ROWS_OFFSET
        + 11 * HD_PARAMETER_ROW_STRIDE
        + HD_PARAMETER_ROW_FLAG_OFFSET;
    bad_marker[marker] = 1;
    let mut bad_marker = OpaqueH5epPreset::parse(bad_marker).unwrap();
    let original_bad_marker = bad_marker.as_bytes().to_vec();
    assert!(
        bad_marker
            .set_hd_type9_choice(slot, HdType9Choice::Choice1)
            .is_err()
    );
    assert_eq!(bad_marker.as_bytes(), original_bad_marker);
}

#[test]
fn type8_choice_writer_patches_only_admitted_word_for_all_slots_rows_and_enable_states() {
    assert_eq!(
        hd_type8_choice_key(HD_TYPE8_CHOICE_MODEL_ID),
        Some(HD_TYPE8_CHOICE_KEY)
    );
    assert_eq!(HdType8Choice::from_index(0), Ok(HdType8Choice::Choice0));
    assert_eq!(HdType8Choice::from_index(1), Ok(HdType8Choice::Choice1));
    assert!(HdType8Choice::from_index(2).is_err());

    for slot_index in 0..8 {
        for row_index in [0, 5, 11] {
            for enabled in [0, 1] {
                for (choice, word) in [
                    (HdType8Choice::Choice0, [0x00, 0x00, 0x00, 0x00]),
                    (HdType8Choice::Choice1, [0x3f, 0x80, 0x00, 0x00]),
                ] {
                    let original =
                        synthetic_type8_h5ep(slot_index, row_index, enabled, 0x3eaa_aaab);
                    let value_at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row_index * HD_PARAMETER_ROW_STRIDE
                        + 4;
                    let mut expected = original.clone();
                    expected[value_at..value_at + 4].copy_from_slice(&word);
                    let mut preset = OpaqueH5epPreset::parse(original).unwrap();
                    let slot = HdFxSlot::new(slot_index).unwrap();
                    assert_eq!(
                        preset.hd_type8_choice_value(slot),
                        HdType8ChoiceValue::UnknownFinite(0x3eaa_aaab)
                    );
                    preset.set_hd_type8_choice(slot, choice).unwrap();
                    assert_eq!(preset.as_bytes(), expected);
                    assert_eq!(
                        preset.hd_type8_choice_value(slot),
                        HdType8ChoiceValue::Choice(choice)
                    );
                }
            }
        }
    }

    let slot = HdFxSlot::new(2).unwrap();
    for (bits, expected) in [
        (
            0x0000_0000,
            HdType8ChoiceValue::Choice(HdType8Choice::Choice0),
        ),
        (
            0x3f80_0000,
            HdType8ChoiceValue::Choice(HdType8Choice::Choice1),
        ),
        (0x3f40_0000, HdType8ChoiceValue::UnknownFinite(0x3f40_0000)),
        (0x7fc0_0001, HdType8ChoiceValue::NonFinite(0x7fc0_0001)),
        (0x7f80_0000, HdType8ChoiceValue::NonFinite(0x7f80_0000)),
        (0xff80_0000, HdType8ChoiceValue::NonFinite(0xff80_0000)),
    ] {
        let preset = OpaqueH5epPreset::parse(synthetic_type8_h5ep(2, 11, 1, bits)).unwrap();
        assert_eq!(preset.hd_type8_choice_value(slot), expected);
    }
}

#[test]
fn type8_choice_writer_rejects_foreign_or_malformed_contexts_atomically() {
    let slot = HdFxSlot::new(5).unwrap();
    let source = synthetic_type8_h5ep(5, 0, 0, 0x3f40_0000);
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
    let row0 = block + HD_EFFECT_ROWS_OFFSET;
    let mut cases = Vec::new();
    let mut wrong_model = source.clone();
    wrong_model[block..block + 4].copy_from_slice(&HD_TYPE9_CHOICE_MODEL_ID.to_be_bytes());
    cases.push(("wrong model", wrong_model));
    let mut wrong_key = source.clone();
    wrong_key[row0..row0 + 4].copy_from_slice(&0x3f10_0003u32.to_be_bytes());
    cases.push(("wrong key", wrong_key));
    for (offset, value, name) in [
        (0x0b, 0, "invalid effect"),
        (8, 2, "noncanonical enable"),
        (0x0f, 0, "zero count"),
        (0x0f, 13, "excessive count"),
    ] {
        let mut invalid = source.clone();
        invalid[block + offset] = value;
        cases.push((name, invalid));
    }
    let mut missing = source.clone();
    missing[row0..row0 + 4].fill(0);
    cases.push(("missing key", missing));
    let mut beyond_count = source.clone();
    let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
    beyond_count[row0..row0 + 4].fill(0);
    beyond_count[row5..row5 + 4].copy_from_slice(&HD_TYPE8_CHOICE_KEY.to_be_bytes());
    beyond_count[block + 0x0f] = 5;
    cases.push(("target beyond count", beyond_count));
    let mut duplicate = source.clone();
    let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
    duplicate[row9..row9 + 4].copy_from_slice(&HD_TYPE8_CHOICE_KEY.to_be_bytes());
    duplicate[block + 0x0f] = 1;
    cases.push(("duplicate outside count", duplicate));
    let mut marker = source.clone();
    marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    cases.push(("assigned marker", marker));
    for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
        let mut nonfinite = source.clone();
        nonfinite[row0 + 4..row0 + 8].copy_from_slice(&bits.to_be_bytes());
        cases.push(("non-finite current value", nonfinite));
    }
    for (reason, bytes) in cases {
        let mut preset = OpaqueH5epPreset::parse(bytes.clone()).unwrap();
        assert!(
            preset
                .set_hd_type8_choice(slot, HdType8Choice::Choice1)
                .is_err(),
            "{reason}"
        );
        assert_eq!(
            preset.as_bytes(),
            bytes,
            "{reason} must preserve source bytes"
        );
    }

    let mut same = OpaqueH5epPreset::parse(synthetic_type8_h5ep(5, 0, 0, 0x3f80_0000)).unwrap();
    let before = same.as_bytes().to_vec();
    same.set_hd_type8_choice(slot, HdType8Choice::Choice1)
        .unwrap();
    assert_eq!(same.as_bytes(), before);
    let mut bad_marker = before.clone();
    bad_marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    let mut bad_marker = OpaqueH5epPreset::parse(bad_marker).unwrap();
    let before = bad_marker.as_bytes().to_vec();
    assert!(
        bad_marker
            .set_hd_type8_choice(slot, HdType8Choice::Choice1)
            .is_err()
    );
    assert_eq!(bad_marker.as_bytes(), before, "guards run before no-op");
}

fn decode_hex_fixture(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("fixture contains hex bytes"))
        .collect()
}

#[test]
fn type11_choices_match_independent_complete_vectors_for_both_keys_per_model() {
    let cases = [
        (
            HdType11Control::Model020a001eKey3f100002,
            include_str!("../../../../tests/fixtures/type11-model-020a001e-source.hex"),
            [
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100002-choice-0.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100002-choice-1.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100002-choice-2.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100002-choice-3.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100002-choice-4.hex"
                ),
            ],
            3usize,
            0x3e12_3456u32,
        ),
        (
            HdType11Control::Model020a001eKey3f100003,
            include_str!("../../../../tests/fixtures/type11-model-020a001e-source.hex"),
            [
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100003-choice-0.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100003-choice-1.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100003-choice-2.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100003-choice-3.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001e-key-3f100003-choice-4.hex"
                ),
            ],
            8usize,
            0x3e65_4321u32,
        ),
        (
            HdType11Control::Model020a001dKey3f100001,
            include_str!("../../../../tests/fixtures/type11-model-020a001d-source.hex"),
            [
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100001-choice-0.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100001-choice-1.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100001-choice-2.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100001-choice-3.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100001-choice-4.hex"
                ),
            ],
            3usize,
            0x3e12_3456u32,
        ),
        (
            HdType11Control::Model020a001dKey3f100002,
            include_str!("../../../../tests/fixtures/type11-model-020a001d-source.hex"),
            [
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100002-choice-0.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100002-choice-1.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100002-choice-2.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100002-choice-3.hex"
                ),
                include_str!(
                    "../../../../tests/fixtures/type11-model-020a001d-key-3f100002-choice-4.hex"
                ),
            ],
            8usize,
            0x3e65_4321u32,
        ),
    ];
    for (control, source_hex, expected_hex, target_row, current_bits) in cases {
        let source = decode_hex_fixture(source_hex);
        assert_eq!(&source[..8], b"H5EP\0\0\0\x01");
        assert_eq!(
            &source[H5EP_MIN_BYTES..],
            b"TYPE11-INDEPENDENT-SYNTHETIC-SUFFIX"
        );
        let block = H5EP_HEADER_BYTES + 0x428 + 5 * 0x100;
        let row = block + 0x10 + target_row * 0x14;
        assert_eq!(&source[row..row + 4], &control.key().to_be_bytes());
        assert_eq!(&source[row + 4..row + 8], &current_bits.to_be_bytes());
        for (choice, expected_hex) in HdType11Choice::ALL.into_iter().zip(expected_hex) {
            let expected = decode_hex_fixture(expected_hex);
            assert_eq!(source.len(), expected.len());
            assert_eq!(&expected[row + 4..row + 8], &choice.bits().to_be_bytes());
            let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
            preset
                .set_hd_type11_choice(HdFxSlot::new(5).unwrap(), control, choice)
                .unwrap();
            assert_eq!(preset.as_bytes(), expected, "{control:?} {choice:?}");
        }
    }
}

#[test]
fn type8_choice_matches_independent_complete_file_vectors_and_noop() {
    // These vectors are checked-in complete H5EP + suffix fixtures. The
    // two expected outputs are literal files, not rebuilt with setter or
    // production offset helpers. The target value is at fixed file offset
    // 0x9dc in this independently specified source layout.
    let source = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type8-choice-source.hex"
    ));
    let choice0 = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type8-choice-0.hex"
    ));
    let choice1 = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type8-choice-1.hex"
    ));
    assert_eq!(source.len(), choice0.len());
    assert_eq!(source.len(), choice1.len());
    assert_eq!(&source[..8], b"H5EP\0\0\0\x01");
    assert_eq!(&source[0x9dc..0x9e0], &[0x3e, 0xaa, 0xaa, 0xab]);
    assert_eq!(&choice0[0x9dc..0x9e0], &[0x00, 0x00, 0x00, 0x00]);
    assert_eq!(&choice1[0x9dc..0x9e0], &[0x3f, 0x80, 0x00, 0x00]);
    for offset in 0..source.len() {
        if !(0x9dc..0x9e0).contains(&offset) {
            assert_eq!(source[offset], choice0[offset], "choice0 byte {offset:#x}");
            assert_eq!(source[offset], choice1[offset], "choice1 byte {offset:#x}");
        }
    }
    assert_eq!(&source[H5EP_MIN_BYTES..], b"TYPE8-SYNTHETIC-SUFFIX");

    let slot = HdFxSlot::new(5).unwrap();
    let mut to_zero = OpaqueH5epPreset::parse(source.clone()).unwrap();
    to_zero
        .set_hd_type8_choice(slot, HdType8Choice::Choice0)
        .unwrap();
    assert_eq!(to_zero.as_bytes(), choice0);
    let mut to_one = OpaqueH5epPreset::parse(source).unwrap();
    to_one
        .set_hd_type8_choice(slot, HdType8Choice::Choice1)
        .unwrap();
    assert_eq!(to_one.as_bytes(), choice1);

    let mut no_op = OpaqueH5epPreset::parse(choice0.clone()).unwrap();
    no_op
        .set_hd_type8_choice(slot, HdType8Choice::Choice0)
        .unwrap();
    assert_eq!(no_op.as_bytes(), choice0);
}

#[test]
fn type10_choice_is_independent_and_preserves_every_non_target_byte() {
    assert_eq!(
        hd_type10_choice_key(HD_TYPE10_CHOICE_MODEL_ID),
        Some(HD_TYPE10_CHOICE_KEY)
    );
    assert_eq!(HdType10Choice::from_index(0), Ok(HdType10Choice::Choice0));
    assert_eq!(HdType10Choice::from_index(1), Ok(HdType10Choice::Choice1));
    assert!(HdType10Choice::from_index(2).is_err());
    assert_eq!(
        hd_type9_choice_key(HD_TYPE10_CHOICE_MODEL_ID),
        Some(0x3f10_0004)
    );
    for slot_index in 0..8 {
        for row_index in [0, 5, 11] {
            for enabled in [0, 1] {
                let original = synthetic_type10_h5ep(slot_index, row_index, enabled, 0x3eaa_aaab);
                let value_at = H5EP_HEADER_BYTES
                    + HD_EFFECT_BLOCK_OFFSET
                    + slot_index * HD_EFFECT_BLOCK_STRIDE
                    + HD_EFFECT_ROWS_OFFSET
                    + row_index * HD_PARAMETER_ROW_STRIDE
                    + 4;
                for (choice, word) in [
                    (HdType10Choice::Choice0, 0x0000_0000u32),
                    (HdType10Choice::Choice1, 0x3f80_0000),
                ] {
                    let mut expected = original.clone();
                    expected[value_at..value_at + 4].copy_from_slice(&word.to_be_bytes());
                    let mut preset = OpaqueH5epPreset::parse(original.clone()).unwrap();
                    let slot = HdFxSlot::new(slot_index).unwrap();
                    assert_eq!(
                        preset.hd_type10_choice_value(slot),
                        HdType10ChoiceValue::UnknownFinite(0x3eaa_aaab)
                    );
                    preset.set_hd_type10_choice(slot, choice).unwrap();
                    assert_eq!(preset.as_bytes(), expected);
                    assert_eq!(
                        preset.hd_type10_choice_value(slot),
                        HdType10ChoiceValue::Choice(choice)
                    );
                }
            }
        }
    }
    let slot = HdFxSlot::new(2).unwrap();
    for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
        let mut p = OpaqueH5epPreset::parse(synthetic_type10_h5ep(2, 11, 1, bits)).unwrap();
        let before = p.as_bytes().to_vec();
        assert!(matches!(
            p.hd_type10_choice_value(slot),
            HdType10ChoiceValue::NonFinite(_)
        ));
        assert!(
            p.set_hd_type10_choice(slot, HdType10Choice::Choice1)
                .is_err()
        );
        assert_eq!(p.as_bytes(), before);
    }
    let slot = HdFxSlot::new(5).unwrap();
    let source = synthetic_type10_h5ep(5, 0, 1, 0x3eaa_aaab);
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
    let row0 = block + HD_EFFECT_ROWS_OFFSET;
    let mut cases = Vec::new();
    let mut wrong_model = source.clone();
    wrong_model[block..block + 4].copy_from_slice(&HD_TYPE9_CHOICE_MODEL_ID.to_be_bytes());
    cases.push(("wrong model", wrong_model));
    let mut wrong_key = source.clone();
    wrong_key[row0..row0 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
    cases.push(("wrong key", wrong_key));
    for (offset, value) in [(0x0b, 0), (8, 2), (0x0f, 0), (0x0f, 13)] {
        let mut bad = source.clone();
        bad[block + offset] = value;
        cases.push(("invalid block guard", bad));
    }
    let mut missing = source.clone();
    missing[row0..row0 + 4].fill(0);
    cases.push(("missing row", missing));
    let mut duplicate = source.clone();
    let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
    duplicate[row9..row9 + 4].copy_from_slice(&HD_TYPE10_CHOICE_KEY.to_be_bytes());
    duplicate[block + 0x0f] = 1;
    cases.push(("duplicate outside count", duplicate));
    let mut beyond = source.clone();
    let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
    beyond[row0..row0 + 4].fill(0);
    beyond[row5..row5 + 4].copy_from_slice(&HD_TYPE10_CHOICE_KEY.to_be_bytes());
    beyond[block + 0x0f] = 5;
    cases.push(("target outside count", beyond));
    let mut marker = source.clone();
    marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
    cases.push(("assigned marker", marker));
    for (reason, bytes) in cases {
        let mut equal_choice = bytes.clone();
        equal_choice[row0 + 4..row0 + 8].copy_from_slice(&0x3f80_0000u32.to_be_bytes());
        let mut preset = OpaqueH5epPreset::parse(equal_choice.clone()).unwrap();
        assert!(
            preset
                .set_hd_type10_choice(slot, HdType10Choice::Choice1)
                .is_err(),
            "{reason} must reject before equal-word no-op"
        );
        assert_eq!(
            preset.as_bytes(),
            equal_choice,
            "{reason} must preserve all bytes"
        );
    }
    assert!(HdFxSlot::new(8).is_err());
    assert_eq!(HdType10Choice::ALL.len(), 2);
}

#[test]
fn type10_choice_is_closed_to_two_associations_and_covers_each_slot_and_row_edge() {
    assert_eq!(HD_TYPE10_CHOICE_MODEL_KEYS.len(), 2);
    assert_eq!(
        hd_type10_choice_key(0x020a_000f),
        Some(HD_TYPE10_CHOICE_KEY)
    );
    for unknown in [0, 0x020a_000e, 0x020a_0011, 0x020a_0014, u32::MAX] {
        assert_eq!(hd_type10_choice_key(unknown), None);
    }

    for (model_id, key) in HD_TYPE10_CHOICE_MODEL_KEYS {
        for slot_index in 0..HD_EFFECT_COUNT {
            for row_index in [0, 5, 11] {
                for enabled in [0, 1] {
                    let source = synthetic_type10_h5ep_for(
                        slot_index,
                        row_index,
                        enabled,
                        0x3eaa_aaab,
                        model_id,
                        key,
                    );
                    let value_at = H5EP_HEADER_BYTES
                        + HD_EFFECT_BLOCK_OFFSET
                        + slot_index * HD_EFFECT_BLOCK_STRIDE
                        + HD_EFFECT_ROWS_OFFSET
                        + row_index * HD_PARAMETER_ROW_STRIDE
                        + 4;
                    let slot = HdFxSlot::new(slot_index).unwrap();
                    for (choice, word) in [
                        (HdType10Choice::Choice0, 0x0000_0000u32),
                        (HdType10Choice::Choice1, 0x3f80_0000),
                    ] {
                        let mut expected = source.clone();
                        expected[value_at..value_at + 4].copy_from_slice(&word.to_be_bytes());
                        let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
                        assert_eq!(
                            preset.hd_type10_choice_value(slot),
                            HdType10ChoiceValue::UnknownFinite(0x3eaa_aaab)
                        );
                        preset.set_hd_type10_choice(slot, choice).unwrap();
                        assert_eq!(preset.as_bytes(), expected);
                        assert_eq!(
                            preset.hd_type10_choice_value(slot),
                            HdType10ChoiceValue::Choice(choice)
                        );

                        let before_noop = preset.as_bytes().to_vec();
                        preset.set_hd_type10_choice(slot, choice).unwrap();
                        assert_eq!(
                            preset.as_bytes(),
                            before_noop,
                            "same-choice no-op is byte-identical"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn type10_nonfinite_and_malformed_equal_word_guards_apply_to_both_associations() {
    let slot = HdFxSlot::new(5).unwrap();
    let block = H5EP_HEADER_BYTES + HD_EFFECT_BLOCK_OFFSET + 5 * HD_EFFECT_BLOCK_STRIDE;
    let row0 = block + HD_EFFECT_ROWS_OFFSET;
    for (model_id, key) in HD_TYPE10_CHOICE_MODEL_KEYS {
        for bits in [0x7fc0_0001u32, 0x7f80_0000, 0xff80_0000] {
            let source = synthetic_type10_h5ep_for(5, 0, 1, bits, model_id, key);
            let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
            assert!(matches!(
                preset.hd_type10_choice_value(slot),
                HdType10ChoiceValue::NonFinite(_)
            ));
            for choice in HdType10Choice::ALL {
                assert!(preset.set_hd_type10_choice(slot, choice).is_err());
                assert_eq!(preset.as_bytes(), source);
            }
        }

        let source = synthetic_type10_h5ep_for(5, 0, 1, 0x3eaa_aaab, model_id, key);
        let mut cases = Vec::new();
        let mut wrong_model = source.clone();
        wrong_model[block..block + 4].copy_from_slice(&HD_TYPE9_CHOICE_MODEL_ID.to_be_bytes());
        cases.push(("foreign model", wrong_model));
        let mut wrong_key = source.clone();
        wrong_key[row0..row0 + 4].copy_from_slice(&0x3f10_0004u32.to_be_bytes());
        cases.push(("type-9 key", wrong_key));
        for (offset, value) in [(0x0b, 0), (8, 2), (0x0f, 0), (0x0f, 13)] {
            let mut bad = source.clone();
            bad[block + offset] = value;
            cases.push(("invalid block guard", bad));
        }
        let mut missing = source.clone();
        missing[row0..row0 + 4].fill(0);
        cases.push(("missing key", missing));
        let mut duplicate = source.clone();
        let row9 = block + HD_EFFECT_ROWS_OFFSET + 9 * HD_PARAMETER_ROW_STRIDE;
        duplicate[row9..row9 + 4].copy_from_slice(&key.to_be_bytes());
        duplicate[block + 0x0f] = 1;
        cases.push(("duplicate beyond count", duplicate));
        let mut beyond = source.clone();
        let row5 = block + HD_EFFECT_ROWS_OFFSET + 5 * HD_PARAMETER_ROW_STRIDE;
        beyond[row0..row0 + 4].fill(0);
        beyond[row5..row5 + 4].copy_from_slice(&key.to_be_bytes());
        beyond[block + 0x0f] = 5;
        cases.push(("target beyond count", beyond));
        let mut marker = source.clone();
        marker[row0 + HD_PARAMETER_ROW_FLAG_OFFSET] = 1;
        cases.push(("assigned marker", marker));

        for (reason, mut malformed) in cases {
            for choice in HdType10Choice::ALL {
                let word_at = row0 + 4;
                malformed[word_at..word_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
                let before = malformed.clone();
                let mut preset = OpaqueH5epPreset::parse(malformed.clone()).unwrap();
                assert!(
                    preset.set_hd_type10_choice(slot, choice).is_err(),
                    "{reason}"
                );
                assert_eq!(preset.as_bytes(), before, "{reason} preserves bytes");
            }
        }
    }
}

#[test]
fn type10_choice_matches_checked_in_complete_h5ep_vectors_and_leaves_type9_control_untouched() {
    let source = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type10-choice-source.hex"
    ));
    let expected0 = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type10-choice-0.hex"
    ));
    let expected1 = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type10-choice-1.hex"
    ));
    assert_eq!(source.len(), expected0.len());
    assert_eq!(source.len(), expected1.len());
    assert_eq!(
        &source[0x950..0x954],
        &HD_TYPE10_CHOICE_MODEL_ID.to_be_bytes()
    );
    assert_eq!(&source[0x9d8..0x9dc], &HD_TYPE10_CHOICE_KEY.to_be_bytes());
    assert_eq!(&source[0x9dc..0x9e0], &[0x3e, 0xaa, 0xaa, 0xab]);
    assert_eq!(&source[0x988..0x98c], &0x3f10_0004u32.to_be_bytes());
    assert_eq!(&source[0x98c..0x990], &0x3f00_0000u32.to_be_bytes());
    assert_eq!(&source[H5EP_MIN_BYTES..], b"TYPE10-SYNTHETIC-SUFFIX");
    for (choice, expected) in [
        (HdType10Choice::Choice0, expected0),
        (HdType10Choice::Choice1, expected1),
    ] {
        assert_eq!(source.len(), expected.len());
        for (i, (before, after)) in source.iter().zip(&expected).enumerate() {
            if !(0x9dc..0x9e0).contains(&i) {
                assert_eq!(before, after, "unexpected vector byte change at {i:#x}");
            }
        }
        let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
        preset
            .set_hd_type10_choice(HdFxSlot::new(5).unwrap(), choice)
            .unwrap();
        assert_eq!(preset.as_bytes(), expected);
    }
    let mut p = OpaqueH5epPreset::parse(source).unwrap();
    p.set_hd_type10_choice(HdFxSlot::new(5).unwrap(), HdType10Choice::Choice1)
        .unwrap();
    assert_eq!(
        &p.as_bytes()[0x988..0x990],
        &[0x3f, 0x10, 0x00, 0x04, 0x3f, 0x00, 0x00, 0x00]
    );
    let mut no_op = p.clone();
    let before = no_op.as_bytes().to_vec();
    no_op
        .set_hd_type10_choice(HdFxSlot::new(5).unwrap(), HdType10Choice::Choice1)
        .unwrap();
    assert_eq!(
        no_op.as_bytes(),
        before,
        "same-word write is byte-identical"
    );
}

#[test]
fn second_type10_association_matches_complete_synthetic_vectors() {
    let source = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type10-choice-000f-source.hex"
    ));
    let expected0 = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type10-choice-000f-choice-0.hex"
    ));
    let expected1 = decode_hex_fixture(include_str!(
        "../../../../tests/fixtures/type10-choice-000f-choice-1.hex"
    ));
    assert_eq!(&source[0x950..0x954], &0x020a_000fu32.to_be_bytes());
    assert_eq!(&source[0x9d8..0x9dc], &HD_TYPE10_CHOICE_KEY.to_be_bytes());
    assert_eq!(&source[0x988..0x98c], &0x3f10_0004u32.to_be_bytes());
    for (choice, expected) in [
        (HdType10Choice::Choice0, expected0),
        (HdType10Choice::Choice1, expected1),
    ] {
        let mut preset = OpaqueH5epPreset::parse(source.clone()).unwrap();
        preset
            .set_hd_type10_choice(HdFxSlot::new(5).unwrap(), choice)
            .unwrap();
        assert_eq!(preset.as_bytes(), expected);
    }
}
