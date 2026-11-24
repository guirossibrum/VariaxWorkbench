use super::*;
use crate::shared::file::{open_regular_file, path_facts};
use std::io::{self, Cursor};

#[cfg(test)]
pub(crate) fn fixture(
    kind: ContainerKind,
    device_id: u32,
    device_version: u32,
    slots: &[(u32, u32)],
) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    let mut info = vec![0; 80];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&device_id.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
    info[72..76].copy_from_slice(&device_version.to_be_bytes());
    info[76..80].copy_from_slice(&[0x5a, 0x5a, 0x5a, 0x5a]);
    let mut patch = b"PATC".to_vec();
    for &(slot, model_id) in slots {
        let mut minf = Vec::new();
        minf.extend_from_slice(&model_id.to_be_bytes());
        minf.extend_from_slice(&slot.to_be_bytes());
        minf.extend_from_slice(&[0x10, 0x20, 0x30, 0x40]);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        model.extend(chunk(b"PARM", &[0x11, 0x22, 0x33, 0x44]));
        model.extend(chunk(
            b"PARM",
            &[
                0xab, 0xcd, 0xef, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
            ],
        ));
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = kind.tag().as_bytes().to_vec();
    data.extend(chunk(b"JUNK", &[9, 8, 7]));
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct BuiltModel<'a> {
    slot: u32,
    model_id: u32,
    tail: &'a [u8],
    parameters: &'a [&'a [u8]],
}

#[cfg(test)]
fn custom_tone(kind: ContainerKind, models: &[BuiltModel<'_>]) -> Vec<u8> {
    custom_tone_with_pinf_len(kind, models, 80)
}

#[cfg(test)]
fn custom_tone_with_pinf_len(
    kind: ContainerKind,
    models: &[BuiltModel<'_>],
    pinf_len: usize,
) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    let mut info = vec![0; pinf_len];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&PODXT_LIVE_TONE_ID.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
    if pinf_len >= 76 {
        info[72..76].copy_from_slice(&PODXT_LIVE_HOST_MIN_VERSION.to_be_bytes());
    }
    if pinf_len >= 80 {
        info[76..80].copy_from_slice(&[0x5a, 0xa5, 0x96, 0x69]);
    }
    let mut patch = b"PATC".to_vec();
    for entry in models {
        let mut minf = Vec::new();
        minf.extend_from_slice(&entry.model_id.to_be_bytes());
        minf.extend_from_slice(&entry.slot.to_be_bytes());
        minf.extend_from_slice(entry.tail);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        for parameter in entry.parameters {
            model.extend(chunk(b"PARM", parameter));
        }
        model.extend(chunk(b"XTRA", &[0xde, 0xad, 0xbe]));
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = kind.tag().as_bytes().to_vec();
    data.extend(chunk(b"ODD!", &[0x17, 0x28, 0x39]));
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

#[cfg(test)]
fn alter_first_model_minf(bytes: &[u8], duplicate: bool) -> Vec<u8> {
    let root = preset::chunks(bytes, 12..bytes.len()).unwrap();
    let patch = root
        .iter()
        .find(|chunk| preset::container(bytes, chunk, b"PATC"))
        .unwrap();
    let models = preset::chunks(bytes, patch.data.start + 4..patch.data.end).unwrap();
    let model = models
        .iter()
        .find(|chunk| preset::container(bytes, chunk, b"MODL"))
        .unwrap();
    let fields = preset::chunks(bytes, model.data.start + 4..model.data.end).unwrap();
    let minf = fields.iter().find(|chunk| chunk.id == *b"MINF").unwrap();
    let chunk_start = minf.data.start - 8;
    let full_end = minf.data.end + (minf.data.len() & 1);
    let raw = bytes[chunk_start..full_end].to_vec();
    let mut result = bytes.to_vec();
    if duplicate {
        result.splice(model.data.end..model.data.end, raw.iter().copied());
    } else {
        result.drain(chunk_start..full_end);
    }
    let delta = raw.len() as i64 * if duplicate { 1 } else { -1 };
    for length_offset in [model.data.start - 4, patch.data.start - 4, 4] {
        let old = u32::from_be_bytes(result[length_offset..length_offset + 4].try_into().unwrap());
        result[length_offset..length_offset + 4]
            .copy_from_slice(&u32::try_from(i64::from(old) + delta).unwrap().to_be_bytes());
    }
    result
}

use crate::pod_assets::{self, SlotRole};

fn drive_record(prefix: u8, id: u32, type_word: u32, bits: u32) -> [u8; 12] {
    let mut bytes = [0; 12];
    bytes[..4].copy_from_slice(&(((prefix as u32) << 24) | id).to_be_bytes());
    bytes[4..8].copy_from_slice(&type_word.to_be_bytes());
    bytes[8..12].copy_from_slice(&bits.to_be_bytes());
    bytes
}

fn int_parameter(id: u32, bits: u32, prefix: u8) -> [u8; 12] {
    drive_record(prefix, id, 0, bits)
}

fn typed_int_parameter(id: u32, bits: u32, prefix: u8, type_word: u32) -> [u8; 12] {
    drive_record(prefix, id, type_word, bits)
}

fn drive_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: model,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

fn stomp_wave_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: model,
            tail: &[0x91, 0x82, 0x73, 0x64],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn delay_note_fixture(
    delay_slot: u32,
    delay_model: u32,
    delay_note: u32,
    delay_time: u32,
    mod_slot: Option<u32>,
    mod_model: u32,
    mod_note: u32,
    tempo: Option<f32>,
) -> Vec<u8> {
    let delay_note_row = drive_record(0x3f, 0x0020_0000, 0, delay_note);
    let delay_time_row = drive_record(0x3f, 0x0010_0000, 1, delay_time);
    let mod_note_row = drive_record(0x3f, 0x0020_0000, 0, mod_note);
    let mod_speed_row = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
    let tempo_row = tempo.map(|value| drive_record(0x3f, 0x0020_0001, 1, value.to_bits()));
    let sentinel = drive_record(0x3f, 0x007a_bcde, 0x7f, 0xdead_beef);
    let delay_rows = [&delay_time_row[..], &delay_note_row[..], &sentinel[..]];
    let mod_rows = [&mod_speed_row[..], &mod_note_row[..]];
    let tempo_rows = tempo_row.as_ref().map(|row| [row.as_slice()]);
    let mut models = vec![BuiltModel {
        slot: delay_slot,
        model_id: delay_model,
        tail: &[1, 2, 3, 4],
        parameters: &delay_rows,
    }];
    if let Some(slot) = mod_slot {
        models.push(BuiltModel {
            slot,
            model_id: mod_model,
            tail: &[5, 6, 7, 8],
            parameters: &mod_rows,
        });
    }
    if tempo_row.is_some() {
        models.push(BuiltModel {
            slot: 0x0001_0000,
            model_id: 0x3e70_0000,
            tail: &[9, 10, 11, 12],
            parameters: tempo_rows.as_ref().unwrap(),
        });
    }
    custom_tone(ContainerKind::L6as, &models)
}

fn delay_context_fixture(
    models: &[(u32, u32, Vec<Vec<u8>>)],
    target: u32,
    version: u32,
) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }

    let mut info = vec![0; 80];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&target.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'Z').to_be_bytes());
    info[72..76].copy_from_slice(&version.to_be_bytes());
    info[76..80].copy_from_slice(&[0x5a, 0xa5, 0x96, 0x69]);

    let mut patch = b"PATC".to_vec();
    for (index, (slot, model_id, parameters)) in models.iter().enumerate() {
        let mut minf = Vec::new();
        minf.extend_from_slice(&model_id.to_be_bytes());
        minf.extend_from_slice(&slot.to_be_bytes());
        minf.extend_from_slice(&[0x11 + index as u8, 0x22, 0x33, 0x44]);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        for parameter in parameters {
            model.extend(chunk(b"PARM", parameter));
        }
        model.extend(chunk(b"XTRA", &[0xde, 0xad, 0xbe]));
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = b"L6AS".to_vec();
    data.extend(chunk(b"ODD!", &[0x17, 0x28, 0x39]));
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

fn mod_operation_fixture(models: &[(u32, u32, Vec<Vec<u8>>)]) -> Vec<u8> {
    delay_context_fixture(models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION)
}

fn mod_test_rows(note: u32, speed_bits: u32) -> Vec<Vec<u8>> {
    vec![
        drive_record(0x3f, 0x0010_0000, 1, speed_bits).to_vec(),
        drive_record(0x3f, 0x0020_0000, 0, note).to_vec(),
        drive_record(0x3f, 0x007a_bcde, 0x7f, 0xfeed_cafe).to_vec(),
    ]
}

fn mod_test_delay_rows(note: u32) -> Vec<Vec<u8>> {
    vec![
        drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
        drive_record(0x3f, 0x0020_0000, 0, note).to_vec(),
        drive_record(0x3f, 0x007a_bcde, 0x7f, 0xcafe_feed).to_vec(),
    ]
}

fn mod_test_system_rows(tempo: f32) -> Vec<Vec<u8>> {
    vec![drive_record(0x3f, 0x0020_0001, 1, tempo.to_bits()).to_vec()]
}

fn patch_expected_parameter(tone: &PodTone, expected: &mut [u8], slot: u32, id: u32, bits: u32) {
    let offset = parameter_word0_offset(tone, slot, id) + 8;
    expected[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
}

fn parm_bits(tone: &PodTone, slot: u32, id: u32) -> u32 {
    let model = tone
        .models()
        .iter()
        .find(|model| model.slot == slot)
        .unwrap();
    model
        .parameters
        .iter()
        .find_map(|parameter| match parameter {
            ModelParameter::Recorded {
                id: found,
                value: ParameterValue::Int { bits } | ParameterValue::Float { bits },
                ..
            } if *found == id => Some(*bits),
            _ => None,
        })
        .unwrap()
}

fn parameter_word0_offset(tone: &PodTone, slot: u32, id: u32) -> usize {
    let model_index = tone
        .models
        .iter()
        .position(|model| model.slot == slot)
        .unwrap();
    let parameter_index = tone.models[model_index].parameters.iter().position(|parameter| {
        matches!(parameter, ModelParameter::Recorded { id: found, .. } if *found == id)
    }).unwrap();
    tone.parameter_value_offsets[model_index][parameter_index].unwrap() - 8
}

fn stomp_harmony_tone_with_models(models: &[BuiltModel<'_>], target: u32, version: u32) -> Vec<u8> {
    let mut bytes = custom_tone(ContainerKind::L6as, models);
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 4..pinf + 8].copy_from_slice(&target.to_be_bytes());
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

fn stomp_harmony_tone(bits1: u32, bits2: u32) -> Vec<u8> {
    let p1 = drive_record(0x3f, 0x0010_0001, 1, bits1);
    let p2 = drive_record(0x3f, 0x0010_0002, 1, bits2);
    stomp_harmony_tone_with_models(
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: 0x020a_0009,
            tail: &[0x91, 0x82, 0x73, 0x64],
            parameters: &[&p1, &p2],
        }],
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
    )
}

fn bundle_with_tone(tone: &[u8]) -> Vec<u8> {
    bundle_with_banks(&[vec![tone.to_vec()]])
}

fn bundle_with_banks(banks: &[Vec<Vec<u8>>]) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_vec();
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            bytes.push(0xa5);
        }
        bytes
    }
    let mut root = b"L6CO".to_vec();
    root.extend(chunk(b"JUNK", &[0x71, 0x72, 0x73]));
    for (index, entries) in banks.iter().enumerate() {
        let mut info = vec![0; 68];
        info[4..6].copy_from_slice(&u16::from(b'A' + index as u8).to_be_bytes());
        let mut bank = if index % 2 == 0 {
            b"BANK".to_vec()
        } else {
            b"FLDR".to_vec()
        };
        bank.extend(chunk(b"BINF", &info));
        bank.extend(chunk(b"NOTE", &[0xde, 0xad, 0xbe]));
        for tone in entries {
            bank.extend_from_slice(tone);
        }
        root.extend(chunk(b"LIST", &bank));
    }
    chunk(b"FORM", &root)
}

fn membership_bundle(
    root_kind: &[u8; 4],
    fixed_tones: &[Vec<u8>],
    folder_children: &[Vec<u8>],
) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_vec();
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            bytes.push(0xa5);
        }
        bytes
    }
    fn bank(kind: &[u8; 4], name: u16, children: &[Vec<u8>]) -> Vec<u8> {
        let mut data = kind.to_vec();
        let mut info = vec![0; 68];
        info[4..6].copy_from_slice(&name.to_be_bytes());
        data.extend(chunk(b"BINF", &info));
        for child in children {
            data.extend_from_slice(child);
        }
        chunk(b"LIST", &data)
    }

    let mut root = root_kind.to_vec();
    root.extend(chunk(b"JUNK", &[0x91, 0x92, 0x93]));
    root.extend(bank(b"BANK", b'A' as u16, fixed_tones));
    root.extend(bank(b"FLDR", b'B' as u16, folder_children));
    root.extend(bank(
        b"FLDR",
        b'C' as u16,
        &[chunk(b"SIDE", &[0x31, 0x32, 0x33])],
    ));
    chunk(b"FORM", &root)
}

fn bundle_with_total_length(tone: &[u8], total_length: usize) -> Vec<u8> {
    let mut bytes = bundle_with_tone(tone);
    assert!(total_length >= bytes.len() + 8);
    let payload_length = total_length - bytes.len() - 8;
    assert!(payload_length.is_multiple_of(2));
    bytes.extend_from_slice(b"JUNK");
    bytes.extend_from_slice(&(payload_length as u32).to_be_bytes());
    bytes.resize(total_length, 0x5a);
    bytes[4..8].copy_from_slice(&((total_length - 8) as u32).to_be_bytes());
    bytes
}

fn tone_with_empty_unknown_chunk(tone: &[u8]) -> Vec<u8> {
    let mut bytes = tone.to_vec();
    bytes.extend_from_slice(b"EXTR\0\0\0\0");
    let form_length = bytes.len() - 8;
    bytes[4..8].copy_from_slice(&(form_length as u32).to_be_bytes());
    bytes
}

fn stomp_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: model,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

fn whammy_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
    stomp_tone(version, model, parameters)
}

fn mod_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: MOD_SLOT,
            model_id: model,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

fn reverb_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: REVERB_SLOT,
            model_id: model,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

fn delay_tone(version: u32, model: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: DELAY_SLOT,
            model_id: model,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

fn eq_tone(version: u32, model: u32, slot: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot,
            model_id: model,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

#[derive(Clone, Copy)]
enum MappedEffect {
    Mod,
    Reverb,
    Delay,
}

fn set_mapped_effect(
    tone: &mut PodTone,
    effect: MappedEffect,
    unsupported_control: bool,
    value: f32,
) -> Result<(), ToneError> {
    match (effect, unsupported_control) {
        (MappedEffect::Mod, false) => tone.set_mod_control_fraction(ModControl::Depth, value),
        (MappedEffect::Mod, true) => tone.set_mod_control_fraction(ModControl::Manual, value),
        (MappedEffect::Reverb, false) => {
            tone.set_reverb_control_fraction(ReverbControl::Dwell, value)
        }
        (MappedEffect::Reverb, true) => {
            tone.set_reverb_control_fraction(ReverbControl::Predelay, value)
        }
        (MappedEffect::Delay, false) => {
            tone.set_delay_control_fraction(DelayControl::Feedback, value)
        }
        (MappedEffect::Delay, true) => tone.set_delay_control_fraction(DelayControl::Drive, value),
    }
}

fn mapped_effect_value(tone: &PodTone, effect: MappedEffect) -> Result<f32, ToneError> {
    match effect {
        MappedEffect::Mod => tone.mod_control_fraction(ModControl::Depth),
        MappedEffect::Reverb => tone.reverb_control_fraction(ReverbControl::Dwell),
        MappedEffect::Delay => tone.delay_control_fraction(DelayControl::Feedback),
    }
}

fn mapped_effect_tone(slot: u32, model_id: u32, version: u32, parameters: &[&[u8]]) -> Vec<u8> {
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot,
            model_id,
            tail: &[0x91, 0x82, 0x73, 0x64],
            parameters,
        }],
    );
    let pinf = find_chunk_data(&bytes, b"PINF");
    bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
    bytes
}

fn enable_tone(amp_model: u32, eq_model: u32, amp_flag: u8, eq_flag: u8) -> Vec<u8> {
    custom_tone(
        ContainerKind::L6as,
        &[
            BuiltModel {
                slot: AMP_SLOT,
                model_id: amp_model,
                tail: &[0x80, 0xa1, 0xb2, amp_flag],
                parameters: &[&[0xde, 0xad, 0xbe]],
            },
            BuiltModel {
                slot: PODXT_EQ_SLOT,
                model_id: eq_model,
                tail: &[0xff, 0xc3, 0xd4, eq_flag],
                parameters: &[],
            },
        ],
    )
}

fn short_enable_tone(minf_len: usize, following_byte: u8) -> Vec<u8> {
    assert!((8..12).contains(&minf_len));
    let tail = vec![0; minf_len - 8];
    let mut bytes = custom_tone(
        ContainerKind::L6as,
        &[
            BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &tail,
                parameters: &[],
            },
            BuiltModel {
                slot: PODXT_EQ_SLOT,
                model_id: PODXT_EQ_MODEL_ID,
                tail: &tail,
                parameters: &[],
            },
        ],
    );
    for (model_id, slot) in [(0xdead_beef, AMP_SLOT), (PODXT_EQ_MODEL_ID, PODXT_EQ_SLOT)] {
        let mut marker = model_id.to_be_bytes().to_vec();
        marker.extend_from_slice(&slot.to_be_bytes());
        let body = bytes
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        // For short MINFs this absolute position is pad or following
        // chunk data. Make it look canonical to prove it is not admitted.
        bytes[body + 11] = following_byte;
    }
    bytes
}

#[test]
fn amp_model_apply_changes_only_the_existing_amp_minf_id_and_noops_same_choice() {
    let opaque_parameter = [0xde, 0xad, 0xbe];
    let unusual_parameter = [
        0xff, 0x12, 0x34, 0x56, 0x7f, 0x00, 0x00, 0x01, 0x7f, 0xc0, 0x00, 0x01,
    ];
    let original = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            // Signed ordinal, reserved bytes and noncanonical enabled byte.
            tail: &[0x80, 0xa1, 0xb2, 0xfe],
            parameters: &[&opaque_parameter, &unusual_parameter],
        }],
    );
    let mut tone = PodTone::parse(original.clone()).unwrap();
    assert_eq!(tone.models()[0].ordinal, Some(-128));
    assert_eq!(tone.models()[0].reserved, Some([0xa1, 0xb2]));
    assert_eq!(tone.models()[0].enabled, Some(false));

    let id_offset = tone.model_minfo_ranges[0].start;
    tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).unwrap();
    let changed = tone.as_bytes().to_vec();
    assert_eq!(id_offset, tone.model_minfo_ranges[0].start);
    for index in 0..original.len() {
        if !(id_offset..id_offset + 4).contains(&index) {
            assert_eq!(changed[index], original[index], "changed byte {index:#x}");
        }
    }
    assert_eq!(
        &changed[id_offset..id_offset + 4],
        &PodAmpModelChoice::NoAmp.model_id().to_be_bytes()
    );
    assert_eq!(
        tone.amp_model_id(),
        Some(PodAmpModelChoice::NoAmp.model_id())
    );
    assert_eq!(tone.models()[0].ordinal, Some(-128));
    assert_eq!(tone.models()[0].reserved, Some([0xa1, 0xb2]));
    assert_eq!(tone.models()[0].enabled, Some(false));
    assert_eq!(
        tone.models()[0].parameters,
        vec![
            ModelParameter::Opaque { len: 3 },
            ModelParameter::Recorded {
                prefix: 0xff,
                id: 0x0012_3456,
                value: ParameterValue::Other {
                    type_word: 0x7f00_0001,
                    bits: 0x7fc0_0001,
                },
            },
        ]
    );

    tone.set_amp_model_choice(PodAmpModelChoice::Line6Clean)
        .unwrap();
    let restored_id = tone.model_minfo_ranges[0].start;
    assert_eq!(
        &tone.as_bytes()[restored_id..restored_id + 4],
        &PodAmpModelChoice::Line6Clean.model_id().to_be_bytes()
    );
    let line6_clean = tone.as_bytes().to_vec();
    tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).unwrap();
    let no_amp_id = tone.model_minfo_ranges[0].start;
    assert_eq!(
        &tone.as_bytes()[no_amp_id..no_amp_id + 4],
        &PodAmpModelChoice::NoAmp.model_id().to_be_bytes()
    );
    tone.set_amp_model_choice(PodAmpModelChoice::Line6Clean)
        .unwrap();
    assert_eq!(tone.as_bytes(), line6_clean);
    let before_noop = tone.as_bytes().to_vec();
    tone.set_amp_model_choice(PodAmpModelChoice::Line6Clean)
        .unwrap();
    assert_eq!(tone.as_bytes(), before_noop);
    assert_eq!(PodAmpModelChoice::try_from(0xdead_beef), Err(0xdead_beef));
}

#[test]
fn amp_choices_match_exact_source_allowlist_and_all_107_current_ids() {
    let expected = [
        0x0002_0000,
        0x0002_002e,
        0x0002_0001,
        0x0002_0005,
        0x0002_0006,
        0x0002_0007,
        0x0002_0008,
        0x0002_000a,
        0x0002_000c,
        0x0002_000f,
        0x0002_0010,
        0x0002_0011,
        0x0002_0012,
        0x0002_0013,
        0x0002_0014,
        0x0002_0015,
        0x0002_0018,
        0x0002_0019,
        0x0002_001a,
        0x0002_001b,
        0x0002_001d,
        0x0002_001e,
        0x0002_0020,
        0x0002_0021,
        0x0002_0022,
        0x0002_0023,
        0x0002_0024,
        0x0002_0026,
        0x0002_0027,
        0x0002_0029,
        0x0002_002a,
        0x0002_002c,
        0x0002_002d,
        0x0002_003d,
        0x0002_003c,
        0x0002_003b,
        0x0002_0032,
        0x0002_004a,
        0x0002_004b,
        0x0002_004c,
        0x0002_004d,
        0x0002_004e,
        0x0002_004f,
    ];
    let actual: Vec<_> = PODXT_AMP_MODEL_CHOICES
        .iter()
        .map(|choice| choice.model_id())
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(
        actual
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        43
    );

    // GetAmpModelList's 107-entry AMP_MODELS initializer includes No Amp;
    // the separate 106-entry control table below omits only that ID. The
    // current classification set also includes all gated model IDs.
    let current_ids = PODXT_AMP_SOURCE_MODEL_IDS.to_vec();
    assert_eq!(current_ids.len(), 107);
    assert_eq!(
        current_ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        107
    );
    let classified_control_ids: std::collections::HashSet<_> = current_ids
        .iter()
        .copied()
        .filter(|id| *id != 0x0002_0000)
        .collect();
    assert_eq!(classified_control_ids.len(), 106);
    assert_eq!(
        classified_control_ids,
        AMP_CONTROL_MODEL_IDS.into_iter().collect()
    );
    let empty: [&[u8]; 0] = [];
    for current in current_ids {
        let bytes = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: current,
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters: &empty,
            }],
        );
        for choice in PODXT_AMP_MODEL_CHOICES {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            tone.set_amp_model_choice(choice).unwrap();
            let after = tone.as_bytes();
            let id_offset = tone.model_minfo_ranges[0].start;
            for i in 0..before.len() {
                if !(id_offset..id_offset + 4).contains(&i) {
                    assert_eq!(
                        after[i],
                        before[i],
                        "current={current:08x}, requested={:08x}, byte={i}",
                        choice.model_id()
                    );
                }
            }
            assert_eq!(tone.amp_model_id(), Some(choice.model_id()));
            assert_eq!(tone.models()[0].ordinal, Some(-128));
            assert_eq!(tone.models()[0].reserved, Some([0xa1, 0xb2]));
            assert_eq!(tone.models()[0].enabled, Some(false));
        }
    }

    let pack_gated: Vec<_> = PODXT_AMP_SOURCE_MODEL_IDS
        .iter()
        .copied()
        .filter(|id| !expected.contains(id))
        .collect();
    assert_eq!(pack_gated.len(), 64);
    for id in pack_gated {
        assert_eq!(PodAmpModelChoice::try_from(id), Err(id));
    }
    for id in [0xdead_beef, 0x7fff_ffff, 0x0003_0000] {
        assert_eq!(PodAmpModelChoice::try_from(id), Err(id));
    }
}

#[test]
fn amp_new_choices_require_known_current_but_legacy_choices_preserve_unknown_compatibility() {
    let empty: [&[u8]; 0] = [];
    let original = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0, 0, 0, 1],
            parameters: &empty,
        }],
    );
    for choice in PODXT_AMP_MODEL_CHOICES {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let before = tone.clone();
        let result = tone.set_amp_model_choice(choice);
        if matches!(
            choice,
            PodAmpModelChoice::NoAmp | PodAmpModelChoice::Line6Clean
        ) {
            result.unwrap();
            assert_eq!(tone.amp_model_id(), Some(choice.model_id()));
        } else {
            assert!(
                result.is_err(),
                "unexpectedly accepted {:08x}",
                choice.model_id()
            );
            assert_eq!(tone, before);
        }
    }
}

#[test]
fn amp_model_apply_uses_the_documented_literal_whole_buffer() {
    let literal = "46 4f 52 4d 00 00 00 aa 4c 36 50 41 4a 55 4e 4b
        00 00 00 03 11 22 33 a5 50 49 4e 46 00 00 00 50
        00 00 00 01 00 03 00 0a 00 41 00 00 00 00 00 00
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
        00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
        00 00 00 00 00 00 00 00 03 00 00 00 5a a5 96 69
        4c 49 53 54 00 00 00 3a 50 41 54 43 4c 49 53 54
        00 00 00 2e 4d 4f 44 4c 4d 49 4e 46 00 00 00 0c
        00 02 00 01 00 03 00 00 fe 12 34 01 50 41 52 4d
        00 00 00 03 de ad be a5 58 54 52 41 00 00 00 02
        55 66";
    let original: Vec<u8> = literal
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    assert_eq!(original.len(), 178);
    let mut tone = PodTone::parse(original.clone()).unwrap();
    tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).unwrap();
    let mut expected = original;
    expected[0x93] = 0x00;
    assert_eq!(tone.as_bytes(), expected);
    assert_eq!(tone.amp_model_id(), Some(0x0002_0000));
}

#[test]
fn expanded_amp_choice_matches_independent_full_buffer_vector_everywhere() {
    let original_hex = concat!(
        "464f524d0000010c4c3650414a554e4b00000003112233a5",
        "50494e4600000050000000010003000a0041000000000000",
        "000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000",
        "0000000000000000030000005aa596694c49535400000090",
        "504154434c495354000000384d4f444c4d494e460000000c",
        "0002000100030000fe1234015041524d0000000c3f001001",
        "000000013f0000004f44442100000003deadbea54c495354",
        "000000384d4f444c4d494e460000000c0102000000030001",
        "80aabbff5041524d0000000c000000000000000300000005",
        "4f44442100000003deadbea54d49432100000003919293a5",
        "5349444500000003313233a5",
    );
    let expected_hex = concat!(
        "464f524d0000010c4c3650414a554e4b00000003112233a5",
        "50494e4600000050000000010003000a0041000000000000",
        "000000000000000000000000000000000000000000000000",
        "000000000000000000000000000000000000000000000000",
        "0000000000000000030000005aa596694c49535400000090",
        "504154434c495354000000384d4f444c4d494e460000000c",
        "0002002e00030000fe1234015041524d0000000c3f001001",
        "000000013f0000004f44442100000003deadbea54c495354",
        "000000384d4f444c4d494e460000000c0102000000030001",
        "80aabbff5041524d0000000c000000000000000300000005",
        "4f44442100000003deadbea54d49432100000003919293a5",
        "5349444500000003313233a5",
    );
    let decode = |hex: &str| {
        let (pairs, remainder) = hex.as_bytes().as_chunks::<2>();
        assert!(remainder.is_empty());
        pairs
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let original = decode(original_hex);
    let expected = decode(expected_hex);
    assert_eq!(original.len(), 276);
    assert_eq!(&original[0x90..0x94], &[0x00, 0x02, 0x00, 0x01]);

    let session_bytes = crate::rig::Session::initial_rig()
        .set_pod_amp_model_choice(
            crate::rig::RoleKind::Pod,
            &original,
            PodAmpModelChoice::Model2002e,
        )
        .unwrap();
    assert_eq!(session_bytes, expected);

    let mut library = Library::default();
    library.import_bytes(original.clone()).unwrap();
    library
        .set_amp_model_choice(0, PodAmpModelChoice::Model2002e)
        .unwrap();
    assert_eq!(library.tone(0).unwrap().as_bytes(), expected);

    let sibling = original.clone();
    let mut bundle =
        PodBundleLibrary::parse(bundle_with_banks(&[vec![original], vec![sibling.clone()]]))
            .unwrap();
    bundle
        .set_amp_model_choice(0, PodAmpModelChoice::Model2002e)
        .unwrap();
    let expected_bundle = bundle_with_banks(&[vec![expected.clone()], vec![sibling.clone()]]);
    assert_eq!(bundle.as_bytes(), expected_bundle);
    assert_eq!(bundle.tone(1, 0).unwrap().as_bytes(), sibling);
}

#[test]
fn amp_model_apply_rejects_scope_and_structure_before_mutation() {
    let parameters: [&[u8]; 0] = [];
    for container in [ContainerKind::L6fs, ContainerKind::L6as] {
        let bytes = custom_tone(
            container,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            }],
        );
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        let before = tone.clone();
        assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
        assert_eq!(tone, before);
    }

    let short_minf = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[],
            parameters: &parameters,
        }],
    );
    let mut tone = PodTone::parse(short_minf).unwrap();
    let before = tone.clone();
    assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
    assert_eq!(tone, before);

    let malformed_same_id = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: PodAmpModelChoice::NoAmp.model_id(),
            tail: &[],
            parameters: &parameters,
        }],
    );
    let mut tone = PodTone::parse(malformed_same_id).unwrap();
    let before = tone.clone();
    assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
    assert_eq!(tone, before);

    let missing_amp = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0, 0, 0, 1],
            parameters: &parameters,
        }],
    );
    let mut tone = PodTone::parse(missing_amp).unwrap();
    let before = tone.clone();
    assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
    assert_eq!(tone, before);

    let mut old_version = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0, 0, 0, 1],
            parameters: &parameters,
        }],
    );
    let pinf = find_chunk_data(&old_version, b"PINF");
    old_version[pinf + 72..pinf + 76]
        .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    let mut tone = PodTone::parse(old_version).unwrap();
    let before = tone.clone();
    assert!(tone.set_amp_model_choice(PodAmpModelChoice::NoAmp).is_err());
    assert_eq!(tone, before);

    let wrong_target = {
        let mut bytes = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 4..pinf + 8].copy_from_slice(&0x0007_0001u32.to_be_bytes());
        bytes
    };
    assert!(PodTone::parse(wrong_target).is_err());

    let wrong_layout_version = {
        let mut bytes = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf..pinf + 4].copy_from_slice(&2u32.to_be_bytes());
        bytes
    };
    assert!(PodTone::parse(wrong_layout_version).is_err());

    let duplicate_slot = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xdead_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            },
            BuiltModel {
                slot: AMP_SLOT,
                model_id: 0xfeed_beef,
                tail: &[0, 0, 0, 1],
                parameters: &parameters,
            },
        ],
    );
    let duplicate_slot_before = duplicate_slot.clone();
    assert!(PodTone::parse(duplicate_slot.clone()).is_err());
    assert_eq!(duplicate_slot, duplicate_slot_before);

    let mut missing_minf = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0, 0, 0, 1],
            parameters: &parameters,
        }],
    );
    let minf = PodTone::parse(missing_minf.clone())
        .unwrap()
        .model_minfo_ranges[0]
        .start;
    missing_minf[minf - 8..minf - 4].copy_from_slice(b"XXXX");
    let missing_minf_before = missing_minf.clone();
    assert!(PodTone::parse(missing_minf.clone()).is_err());
    assert_eq!(missing_minf, missing_minf_before);

    let opaque_parameter = [0xde, 0xad, 0xbe];
    let unusual_parameter = [
        0xff, 0x12, 0x34, 0x56, 0x7f, 0x00, 0x00, 0x01, 0x7f, 0xc0, 0x00, 0x01,
    ];
    let mut duplicate_minf = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0x80, 0xa1, 0xb2, 0xfe],
            parameters: &[&opaque_parameter, &unusual_parameter],
        }],
    );
    let top = preset::chunks(&duplicate_minf, 12..duplicate_minf.len()).unwrap();
    let patch = preset::unique(&top, b"LIST").unwrap();
    let model_chunks =
        preset::chunks(&duplicate_minf, patch.data.start + 4..patch.data.end).unwrap();
    let model = model_chunks
        .iter()
        .find(|chunk| preset::container(&duplicate_minf, chunk, b"MODL"))
        .unwrap();
    let fields = preset::chunks(&duplicate_minf, model.data.start + 4..model.data.end).unwrap();
    let minf = preset::unique(&fields, b"MINF").unwrap();
    let duplicate_chunk = duplicate_minf[minf.data.start - 8..minf.data.end].to_vec();
    let inserted_len = duplicate_chunk.len();
    let model_length_offset = model.data.start - 4;
    let patch_length_offset = patch.data.start - 4;
    let insert_at = model.data.end;
    duplicate_minf.splice(insert_at..insert_at, duplicate_chunk);
    for offset in [model_length_offset, patch_length_offset, 4] {
        let length = preset::word(&duplicate_minf, offset) as usize;
        duplicate_minf[offset..offset + 4]
            .copy_from_slice(&((length + inserted_len) as u32).to_be_bytes());
    }
    let duplicate_minf_before = duplicate_minf.clone();
    assert!(PodTone::parse(duplicate_minf.clone()).is_err());
    assert_eq!(duplicate_minf, duplicate_minf_before);
}

#[test]
fn amp_model_apply_updates_library_and_bundle_dirty_state_and_revert() {
    let original = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0x0002_0000,
            tail: &[0x80, 0xa1, 0xb2, 0xfe],
            parameters: &[],
        }],
    );
    let mut library = Library::default();
    library.import_bytes(original.clone()).unwrap();
    library
        .set_amp_model_choice(0, PodAmpModelChoice::NoAmp)
        .unwrap();
    assert!(!library.is_dirty(0));
    library
        .set_amp_model_choice(0, PodAmpModelChoice::Line6Clean)
        .unwrap();
    assert!(library.is_dirty(0));
    assert_eq!(
        library.tone(0).unwrap().amp_model_id(),
        Some(PodAmpModelChoice::Line6Clean.model_id())
    );
    library.revert_unsaved(0).unwrap();
    assert!(!library.is_dirty(0));
    assert_eq!(library.tone(0).unwrap().as_bytes(), original);

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    bundle
        .set_amp_model_choice(0, PodAmpModelChoice::Line6Clean)
        .unwrap();
    assert!(bundle.is_dirty());
    assert_eq!(
        bundle.tone(0, 0).unwrap().amp_model_id(),
        Some(PodAmpModelChoice::Line6Clean.model_id())
    );
    bundle.revert().unwrap();
    assert!(!bundle.is_dirty());
    assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), original);

    let wrong_container = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0x80, 0xa1, 0xb2, 0xfe],
            parameters: &[],
        }],
    );
    let mut rejected_library = Library::default();
    rejected_library
        .import_bytes(wrong_container.clone())
        .unwrap();
    assert!(
        rejected_library
            .set_amp_model_choice(0, PodAmpModelChoice::NoAmp)
            .is_err()
    );
    assert!(!rejected_library.is_dirty(0));
    assert_eq!(
        rejected_library.tone(0).unwrap().as_bytes(),
        wrong_container
    );

    let mut rejected_bundle = PodBundleLibrary::parse(bundle_with_tone(&wrong_container)).unwrap();
    assert!(
        rejected_bundle
            .set_amp_model_choice(0, PodAmpModelChoice::NoAmp)
            .is_err()
    );
    assert!(!rejected_bundle.is_dirty());
    assert_eq!(
        rejected_bundle.tone(0, 0).unwrap().as_bytes(),
        wrong_container
    );
}

#[test]
fn cab_mic_edits_use_exact_source_ids_and_patch_only_admitted_words() {
    // Kept independent from the production allowlist to catch omissions and accidental ranges.
    let literal_ids = [
        0x0102_0000,
        0x0102_0001,
        0x0102_0002,
        0x0102_0003,
        0x0102_0004,
        0x0102_0005,
        0x0102_0006,
        0x0102_0007,
        0x0102_0008,
        0x0102_0009,
        0x0102_000a,
        0x0102_000b,
        0x0102_000c,
        0x0102_000d,
        0x0102_000e,
        0x0102_000f,
        0x0102_0010,
        0x0102_0011,
        0x0102_0012,
        0x0102_0013,
        0x0102_0014,
        0x0102_0015,
        0x0102_0016,
        0x0102_0018,
        0x0102_001a,
    ];
    assert_eq!(PODXT_CAB_MODEL_IDS, literal_ids);
    let literal_gated_ids = [
        0x0103_0001,
        0x0103_0002,
        0x0103_0003,
        0x0103_0004,
        0x0103_0005,
        0x0103_0006,
        0x0103_0007,
        0x0103_0008,
        0x0103_0009,
        0x0103_000a,
        0x0103_000b,
        0x0103_000c,
        0x0103_000d,
        0x0103_000e,
        0x0103_000f,
        0x0103_0010,
        0x0103_0011,
        0x0103_0012,
        0x0103_0013,
        0x0103_0014,
        0x0103_0015,
        0x0103_0016,
    ];
    assert_eq!(PODXT_BASS_CAB_MODEL_IDS, literal_gated_ids);
    for id in literal_ids {
        let mic = int_parameter(0, 0xdead_beef, 0x3f);
        let original = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: 0xfedc_ba98,
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters: &[&mic],
            }],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(
            tone.mic_choice(),
            None,
            "unsupported stored MIC must remain raw"
        );
        let choice = PodCabModelChoice::try_from(id).unwrap();
        let offset = tone.model_minfo_ranges[0].start;
        let mut expected = original.clone();
        expected[offset..offset + 4].copy_from_slice(&id.to_be_bytes());
        tone.set_cab_model_choice(choice).unwrap();
        assert_eq!(
            tone.as_bytes(),
            expected,
            "unexpected write set for CAB {id:08x}"
        );
        assert_eq!(tone.models()[0].model_id, id);
        assert_eq!(tone.models()[0].ordinal, Some(-128));
        assert_eq!(tone.models()[0].reserved, Some([0xa1, 0xb2]));
        assert_eq!(tone.models()[0].enabled, Some(false));
        tone.set_cab_model_choice(choice).unwrap();
        assert_eq!(tone.as_bytes(), expected, "same CAB choice must be a no-op");
        assert_eq!(
            PodTone::parse(expected.clone()).unwrap().as_bytes(),
            expected
        );
    }
    for id in PODXT_BASS_CAB_MODEL_IDS
        .into_iter()
        .chain([0x0102_0017, 0x0102_0019, 0xdead_beef])
    {
        assert!(
            PodCabModelChoice::try_from(id).is_err(),
            "CAB {id:08x} must reject"
        );
    }
}

#[test]
fn l6pa_cab_mic_r4_vectors_are_exact_and_all_admitted_choices_edit_offline() {
    let cab_before =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
    let cab_after = include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/cab.after.bin");
    let guitar_before =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/mic-guitar.before.bin")
            .to_vec();
    let guitar_after =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/mic-guitar.after.bin");
    let bass_before =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/mic-bass.before.bin").to_vec();
    let bass_after =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/mic-bass.after.bin");

    let mut cab = PodTone::parse(cab_before.clone()).unwrap();
    assert_eq!(cab.container(), ContainerKind::L6pa);
    assert_eq!(cab.pinf_data_len, 76);
    assert_eq!(cab.device_version, PODXT_LIVE_HOST_MIN_VERSION);
    assert!(cab.cab_mic_editable());
    let desired_cab = PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[0]).unwrap();
    cab.set_cab_model_choice(desired_cab).unwrap();
    assert_eq!(cab.as_bytes(), cab_after);
    assert_eq!(cab.cab_edit_model_id(), Some(desired_cab.model_id()));

    // Keep this independent literal request list beside the full-file expected
    // word mask: each transition starts from the different known bass CAB.
    let transition_ids = [
        0x0102_0000,
        0x0102_0001,
        0x0102_0002,
        0x0102_0003,
        0x0102_0004,
        0x0102_0005,
        0x0102_0006,
        0x0102_0007,
        0x0102_0008,
        0x0102_0009,
        0x0102_000a,
        0x0102_000b,
        0x0102_000c,
        0x0102_000d,
        0x0102_000e,
        0x0102_000f,
        0x0102_0010,
        0x0102_0011,
        0x0102_0012,
        0x0102_0013,
        0x0102_0014,
        0x0102_0015,
        0x0102_0016,
        0x0102_0018,
        0x0102_001a,
    ];
    assert_eq!(transition_ids, PODXT_CAB_MODEL_IDS);
    let original_mic_word = &cab_before[0xa8..0xac];
    for id in transition_ids {
        let mut tone = PodTone::parse(cab_before.clone()).unwrap();
        assert_eq!(tone.cab_edit_model_id(), Some(0x0103_0001));
        assert_eq!(tone.cab_edit_class(), Some(PodCabClass::Bass));
        assert_eq!(tone.mic_choice().unwrap().value(), 2);
        let choice = PodCabModelChoice::try_from(id).unwrap();
        tone.set_cab_model_choice(choice).unwrap();

        let mut expected = cab_before.clone();
        expected[0x8c..0x90].copy_from_slice(&id.to_be_bytes());
        assert_eq!(tone.as_bytes(), expected, "CAB transition to {id:08x}");
        assert_eq!(&tone.as_bytes()[0xa8..0xac], original_mic_word);
        assert_eq!(tone.cab_edit_model_id(), Some(id));
        assert_eq!(tone.cab_edit_class(), Some(PodCabClass::Other));
        assert_eq!(tone.mic_choice().unwrap().value(), 2);
        assert_eq!(
            PodTone::parse(expected.clone()).unwrap().as_bytes(),
            expected
        );
    }

    let mic_cases = [
        (guitar_before, guitar_after, 0x0102_0000, 1, 3),
        (bass_before, bass_after, 0x0103_0001, 2, 0),
    ];
    for (before, after, current_cab, source_mic, requested_mic) in mic_cases {
        let tone = PodTone::parse(before.clone()).unwrap();
        assert!(tone.cab_mic_editable());
        assert_eq!(tone.cab_edit_model_id(), Some(current_cab));
        assert_eq!(tone.mic_choice().unwrap().value(), source_mic);
        let mut changed = tone.clone();
        changed
            .set_mic_choice(PodMicChoice::try_from(requested_mic).unwrap())
            .unwrap();
        assert_eq!(changed.as_bytes(), after);
        assert_eq!(changed.mic_choice().unwrap().value(), requested_mic);
    }

    let base = PodTone::parse(cab_before.clone()).unwrap();
    let minf_offset = base.model_minfo_ranges[0].start;
    for id in PODXT_CAB_MODEL_IDS {
        let mut original = cab_before.clone();
        original[minf_offset..minf_offset + 4].copy_from_slice(&id.to_be_bytes());
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let choice = PodCabModelChoice::try_from(id).unwrap();
        let mic_bytes = tone.as_bytes()[0xa8..0xac].to_vec();
        tone.set_cab_model_choice(choice).unwrap();
        assert_eq!(tone.as_bytes()[0xa8..0xac], mic_bytes);
        let mut expected = original;
        expected[minf_offset..minf_offset + 4].copy_from_slice(&id.to_be_bytes());
        assert_eq!(tone.as_bytes(), expected);
    }
    for id in PODXT_BASS_CAB_MODEL_IDS
        .into_iter()
        .chain([0x0102_0017, 0x0102_0019, 0xdead_beef])
    {
        assert!(PodCabModelChoice::try_from(id).is_err());
    }
}

#[test]
fn l6pa_strict_edits_flow_through_library_bundle_and_session_atomically() {
    let original =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/cab.before.bin").to_vec();
    let cab = PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[1]).unwrap();
    let mic = PodMicChoice::try_from(2).unwrap();
    let mut library = Library::default();
    library.import_bytes(original.clone()).unwrap();
    library.set_cab_model_choice(0, cab).unwrap();
    assert!(library.is_dirty(0));
    library.set_mic_choice(0, mic).unwrap();
    let saved = library.tone(0).unwrap().as_bytes().to_vec();
    assert_eq!(PodTone::parse(saved.clone()).unwrap().as_bytes(), saved);
    library.note_saved(0).unwrap();
    assert!(!library.is_dirty(0));
    library.revert_unsaved(0).unwrap();
    assert_eq!(library.tone(0).unwrap().as_bytes(), saved);

    let sibling =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/mic-guitar.before.bin");
    let mut bundle = PodBundleLibrary::parse(bundle_with_banks(&[vec![
        original.clone(),
        sibling.to_vec(),
    ]]))
    .unwrap();
    bundle.set_cab_model_choice(0, cab).unwrap();
    bundle.set_mic_choice(0, mic).unwrap();
    assert!(bundle.is_dirty());
    let reopened = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
    assert_eq!(
        reopened.tone(0, 0).unwrap().cab_edit_model_id(),
        Some(cab.model_id())
    );
    assert_eq!(reopened.tone(0, 0).unwrap().mic_choice(), Some(mic));
    assert_eq!(reopened.tone(0, 1).unwrap().as_bytes(), sibling);
    bundle.revert().unwrap();
    assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), original);
    assert_eq!(bundle.tone(0, 1).unwrap().as_bytes(), sibling);

    let session = crate::rig::Session::initial_rig();
    let changed = session
        .set_pod_cab_model_choice(crate::rig::RoleKind::Pod, &original, cab)
        .unwrap();
    let changed = session
        .set_pod_mic_choice(crate::rig::RoleKind::Pod, &changed, mic)
        .unwrap();
    assert_eq!(
        PodTone::parse(changed.clone()).unwrap().mic_choice(),
        Some(mic)
    );
    let unknown_offset = PodTone::parse(original.clone()).unwrap().model_minfo_ranges[0].start;
    let mut invalid = original.clone();
    invalid[unknown_offset..unknown_offset + 4].copy_from_slice(&0xdead_beefu32.to_be_bytes());
    let mut invalid_tone = PodTone::parse(invalid.clone()).unwrap();
    assert!(
        invalid_tone
            .set_cab_model_choice(PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[0]).unwrap())
            .is_err()
    );
    assert_eq!(invalid_tone.as_bytes(), invalid);
}

#[test]
fn l6pa_admission_is_closed_over_context_mic_and_existing_cab_state() {
    let mic = int_parameter(0, 2, 0x3f);
    let choice = PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[0]).unwrap();
    for current in PODXT_CAB_MODEL_IDS
        .into_iter()
        .chain(PODXT_BASS_CAB_MODEL_IDS)
    {
        let original = custom_tone_with_pinf_len(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: current,
                tail: &[0xa1, 0xb2, 0xc3, 1],
                parameters: &[&mic],
            }],
            76,
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert!(tone.cab_mic_editable(), "current CAB {current:08x}");
        assert_eq!(tone.mic_choice().unwrap().value(), 2);
        tone.set_cab_model_choice(choice).unwrap();
        assert_eq!(tone.cab_edit_model_id(), Some(choice.model_id()));
        let mic_offset = tone.parameter_value_offsets[0][0].unwrap();
        assert_eq!(
            &tone.as_bytes()[mic_offset..mic_offset + 4],
            &2u32.to_be_bytes()
        );
        for value in 0..=3 {
            let mut mic_tone = PodTone::parse(original.clone()).unwrap();
            let mic_choice = PodMicChoice::try_from(value).unwrap();
            mic_tone.set_mic_choice(mic_choice).unwrap();
            assert_eq!(mic_tone.mic_choice(), Some(mic_choice));
            let mut expected = original.clone();
            expected[mic_offset..mic_offset + 4].copy_from_slice(&value.to_be_bytes());
            assert_eq!(mic_tone.as_bytes(), expected);
        }
    }

    let mut cases = Vec::new();
    for value in [4, u32::MAX] {
        let bad = int_parameter(0, value, 0x3f);
        cases.push(vec![bad.to_vec()]);
    }
    let bad_prefix = int_parameter(0, 2, 0x3e);
    cases.push(vec![bad_prefix.to_vec()]);
    let bad_type = typed_int_parameter(0, 2, 0x3f, 7);
    cases.push(vec![bad_type.to_vec()]);
    let opaque = [0x3f, 0, 0, 0, 0xaa];
    cases.push(vec![opaque.to_vec()]);
    let valid = int_parameter(0, 2, 0x3f);
    cases.push(vec![valid.to_vec(), valid.to_vec()]);
    cases.push(vec![]);
    for records in cases {
        let refs: Vec<&[u8]> = records.iter().map(Vec::as_slice).collect();
        let original = custom_tone_with_pinf_len(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: PODXT_CAB_MODEL_IDS[0],
                tail: &[1, 2, 3, 1],
                parameters: &refs,
            }],
            76,
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert!(!tone.cab_mic_editable());
        assert!(tone.set_cab_model_choice(choice).is_err());
        assert!(
            tone.set_mic_choice(PodMicChoice::try_from(2).unwrap())
                .is_err()
        );
        assert_eq!(tone.as_bytes(), original);
    }

    for tail in [&[][..], &[1, 2, 3][..], &[1, 2, 3, 4, 5][..]] {
        let original = custom_tone_with_pinf_len(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: PODXT_CAB_MODEL_IDS[0],
                tail,
                parameters: &[&mic],
            }],
            76,
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert!(tone.set_cab_model_choice(choice).is_err());
        assert!(
            tone.set_mic_choice(PodMicChoice::try_from(3).unwrap())
                .is_err()
        );
        assert_eq!(tone.as_bytes(), original);
    }

    let short_profile = custom_tone_with_pinf_len(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: PODXT_CAB_MODEL_IDS[0],
            tail: &[1, 2, 3, 1],
            parameters: &[&mic],
        }],
        80,
    );
    let mut tone = PodTone::parse(short_profile.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert!(
        tone.set_mic_choice(PodMicChoice::try_from(2).unwrap())
            .is_err()
    );
    assert_eq!(tone.as_bytes(), short_profile);

    let unknown = custom_tone_with_pinf_len(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: 0xdead_beef,
            tail: &[1, 2, 3, 1],
            parameters: &[&mic],
        }],
        76,
    );
    let mut tone = PodTone::parse(unknown.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert!(
        tone.set_mic_choice(PodMicChoice::try_from(2).unwrap())
            .is_err()
    );
    assert_eq!(tone.as_bytes(), unknown);
}

#[test]
fn l6pa_public_boundary_checks_exact_floor_target_layout_container_and_duplicates() {
    let source =
        include_bytes!("../../../../tests/fixtures/pod-l6pa-cab-mic/mic-guitar.before.bin")
            .to_vec();
    let choice = PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[0]).unwrap();
    let mic_choice = PodMicChoice::try_from(1).unwrap();
    let pinf_start = source.windows(4).position(|word| word == b"PINF").unwrap() + 8;

    let mut at_floor = PodTone::parse(source.clone()).unwrap();
    assert_eq!(at_floor.device_version, PODXT_LIVE_HOST_MIN_VERSION);
    at_floor.set_cab_model_choice(choice).unwrap();
    let mut at_floor_mic = PodTone::parse(source.clone()).unwrap();
    at_floor_mic.set_mic_choice(mic_choice).unwrap();

    let mut below_floor = source.clone();
    below_floor[pinf_start + 72..pinf_start + 76]
        .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    let mut tone = PodTone::parse(below_floor.clone()).unwrap();
    assert!(
        tone.set_cab_model_choice(PodCabModelChoice::from_model_id(0x0102_0000).unwrap())
            .is_err()
    );
    assert_eq!(tone.as_bytes(), below_floor);
    let mut tone = PodTone::parse(below_floor.clone()).unwrap();
    assert!(tone.set_mic_choice(mic_choice).is_err());
    assert_eq!(tone.as_bytes(), below_floor);

    let mut wrong_target = source.clone();
    wrong_target[pinf_start + 4..pinf_start + 8].copy_from_slice(&0xdead_beefu32.to_be_bytes());
    assert!(matches!(
        PodTone::parse(wrong_target.clone()),
        Err(ToneError::WrongTarget { .. })
    ));

    let mut wrong_layout = source.clone();
    wrong_layout[pinf_start..pinf_start + 4].copy_from_slice(&2u32.to_be_bytes());
    assert!(PodTone::parse(wrong_layout.clone()).is_err());
    let truncated_pinf = custom_tone_with_pinf_len(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: PODXT_CAB_MODEL_IDS[0],
            tail: &[1, 2, 3, 1],
            parameters: &[],
        }],
        75,
    );
    let before_truncated = truncated_pinf.clone();
    assert!(PodTone::parse(truncated_pinf.clone()).is_err());
    assert_eq!(truncated_pinf, before_truncated);

    let mut wrong_container = source.clone();
    wrong_container[8..12].copy_from_slice(b"L6FS");
    let mut tone = PodTone::parse(wrong_container.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert!(tone.set_mic_choice(mic_choice).is_err());
    assert_eq!(tone.as_bytes(), wrong_container);

    let missing_cab = custom_tone_with_pinf_len(ContainerKind::L6pa, &[], 76);
    let mut tone = PodTone::parse(missing_cab.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert!(tone.set_mic_choice(mic_choice).is_err());
    assert_eq!(tone.as_bytes(), missing_cab);

    let mic = int_parameter(0, 1, 0x3f);
    let duplicate_cab = custom_tone_with_pinf_len(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: CAB_SLOT,
                model_id: PODXT_CAB_MODEL_IDS[0],
                tail: &[1, 2, 3, 1],
                parameters: &[&mic],
            },
            BuiltModel {
                slot: CAB_SLOT,
                model_id: PODXT_CAB_MODEL_IDS[1],
                tail: &[1, 2, 3, 1],
                parameters: &[&mic],
            },
        ],
        76,
    );
    assert!(PodTone::parse(duplicate_cab.clone()).is_err());
    assert!(!duplicate_cab.is_empty());

    let missing_mic = custom_tone_with_pinf_len(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: PODXT_CAB_MODEL_IDS[0],
            tail: &[1, 2, 3, 1],
            parameters: &[],
        }],
        76,
    );
    let mut tone = PodTone::parse(missing_mic.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert!(tone.set_mic_choice(mic_choice).is_err());
    assert_eq!(tone.as_bytes(), missing_mic);

    let duplicate_mic = custom_tone_with_pinf_len(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: PODXT_CAB_MODEL_IDS[0],
            tail: &[1, 2, 3, 1],
            parameters: &[&mic, &mic],
        }],
        76,
    );
    let mut tone = PodTone::parse(duplicate_mic.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert!(tone.set_mic_choice(mic_choice).is_err());
    assert_eq!(tone.as_bytes(), duplicate_mic);

    for remove in [false, true] {
        let mut malformed = source.clone();
        let chunks = preset::chunks(&malformed, 12..malformed.len()).unwrap();
        let pinf = chunks.iter().find(|chunk| chunk.id == *b"PINF").unwrap();
        let start = pinf.data.start - 8;
        let end = pinf.data.end + (pinf.data.len() & 1);
        let raw = malformed[start..end].to_vec();
        if remove {
            malformed.drain(start..end);
        } else {
            malformed.splice(end..end, raw);
        }
        let new_len = u32::try_from(malformed.len() - 8).unwrap();
        malformed[4..8].copy_from_slice(&new_len.to_be_bytes());
        let malformed_before = malformed.clone();
        assert!(PodTone::parse(malformed.clone()).is_err());
        assert_eq!(malformed, malformed_before);
    }
}

#[test]
fn mic_edits_cover_both_classes_all_choices_and_exact_value_word() {
    for cab in [PODXT_CAB_MODEL_IDS[0], PODXT_BASS_CAB_MODEL_IDS[0]] {
        for value in 0..=3 {
            let mic = int_parameter(PODXT_MIC_PARAM, 0xffff_fffe, 0x3f);
            let original = custom_tone(
                ContainerKind::L6as,
                &[BuiltModel {
                    slot: CAB_SLOT,
                    model_id: cab,
                    tail: &[0, 0, 0, 1],
                    parameters: &[&mic],
                }],
            );
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert_eq!(
                tone.cab_class(),
                Some(if PODXT_BASS_CAB_MODEL_IDS.contains(&cab) {
                    PodCabClass::Bass
                } else {
                    PodCabClass::Other
                })
            );
            assert_eq!(
                tone.mic_choice(),
                None,
                "unknown integer must not display as a valid choice"
            );
            let choice = PodMicChoice::try_from(value).unwrap();
            let offset = tone.parameter_value_offsets[0][0].unwrap();
            let mut expected = original;
            expected[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
            tone.set_mic_choice(choice).unwrap();
            assert_eq!(
                tone.as_bytes(),
                expected,
                "MIC value {value} for CAB {cab:08x}"
            );
            assert_eq!(tone.mic_choice(), Some(choice));
            tone.set_mic_choice(choice).unwrap();
            assert_eq!(tone.as_bytes(), expected, "same MIC choice must be a no-op");
        }
    }
}

#[test]
fn cab_apply_preserves_missing_opaque_and_noncanonical_mic_records() {
    let opaque = [0x3f, 0, 0, 0, 0xaa];
    let noncanonical = int_parameter(0, 2, 0x3e);
    for parameters in [&[][..], &[&opaque[..]][..], &[&noncanonical[..]][..]] {
        let original = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: PODXT_BASS_CAB_MODEL_IDS[0],
                tail: &[0x80, 0xa1, 0xb2, 0xfe],
                parameters,
            }],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let mut expected = original;
        let id_offset = tone.model_minfo_ranges[0].start;
        let id = PODXT_CAB_MODEL_IDS[0];
        expected[id_offset..id_offset + 4].copy_from_slice(&id.to_be_bytes());
        tone.set_cab_model_choice(PodCabModelChoice::try_from(id).unwrap())
            .unwrap();
        assert_eq!(tone.as_bytes(), expected);
    }
}

#[test]
fn mic_target_ambiguity_and_inapplicable_cab_reject_atomically() {
    let valid = int_parameter(0, 1, 0x3f);
    let bad_type = typed_int_parameter(0, 1, 0x3f, 7);
    let bad_prefix = int_parameter(0, 1, 0x3e);
    let opaque_target = [0x3f, 0, 0, 0, 0xaa];
    let unrelated_opaque = [0x3f, 0x12, 0x34, 0x56, 0xaa];
    let cases: Vec<Vec<Vec<u8>>> = vec![
        vec![],
        vec![bad_type.to_vec()],
        vec![bad_prefix.to_vec()],
        vec![opaque_target.to_vec()],
        vec![valid.to_vec(), valid.to_vec()],
        vec![unrelated_opaque.to_vec(), valid.to_vec()],
    ];
    for (case_index, records) in cases.into_iter().enumerate() {
        let refs: Vec<&[u8]> = records.iter().map(Vec::as_slice).collect();
        let original = custom_tone(
            ContainerKind::L6as,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: PODXT_CAB_MODEL_IDS[0],
                tail: &[0, 0, 0, 1],
                parameters: &refs,
            }],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        if case_index == 5 {
            tone.set_mic_choice(PodMicChoice::try_from(3).unwrap())
                .unwrap();
            continue;
        }
        let before_models = tone.models().to_vec();
        assert!(
            tone.set_mic_choice(PodMicChoice::try_from(3).unwrap())
                .is_err()
        );
        assert_eq!(tone.as_bytes(), original);
        assert_eq!(tone.models(), before_models);
    }
    let raw = int_parameter(0, 1, 0x3f);
    let unknown_cab = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: 0xdead_beef,
            tail: &[0, 0, 0, 1],
            parameters: &[&raw],
        }],
    );
    let mut tone = PodTone::parse(unknown_cab.clone()).unwrap();
    assert_eq!(tone.mic_choice(), None);
    assert!(
        tone.set_mic_choice(PodMicChoice::try_from(2).unwrap())
            .is_err()
    );
    assert_eq!(tone.as_bytes(), unknown_cab);
}

#[test]
fn cab_mic_validation_and_shared_library_bundle_lifecycle_are_atomic() {
    let mic = int_parameter(0, 1, 0x3f);
    let original = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: PODXT_CAB_MODEL_IDS[0],
            tail: &[0, 0, 0, 1],
            parameters: &[&mic],
        }],
    );
    let mut library = Library::default();
    library.import_bytes(original.clone()).unwrap();
    let cab = PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[1]).unwrap();
    library.set_cab_model_choice(0, cab).unwrap();
    assert!(library.is_dirty(0));
    library
        .set_mic_choice(0, PodMicChoice::try_from(3).unwrap())
        .unwrap();
    assert_eq!(
        library.tone(0).unwrap().cab_model_id(),
        Some(cab.model_id())
    );
    assert_eq!(library.tone(0).unwrap().mic_choice().unwrap().value(), 3);
    library.note_saved(0).unwrap();
    assert!(!library.is_dirty(0));
    let saved = library.tone(0).unwrap().as_bytes().to_vec();
    assert!(
        library
            .set_mic_choice(9, PodMicChoice::try_from(0).unwrap())
            .is_err()
    );
    assert!(!library.is_dirty(0));
    assert_eq!(library.tone(0).unwrap().as_bytes(), saved);
    library
        .set_cab_model_choice(
            0,
            PodCabModelChoice::try_from(PODXT_CAB_MODEL_IDS[2]).unwrap(),
        )
        .unwrap();
    assert!(library.is_dirty(0));
    library.revert_unsaved(0).unwrap();
    assert!(!library.is_dirty(0));
    assert_eq!(library.tone(0).unwrap().as_bytes(), saved);

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    bundle.set_cab_model_choice(0, cab).unwrap();
    bundle
        .set_mic_choice(0, PodMicChoice::try_from(2).unwrap())
        .unwrap();
    assert!(bundle.is_dirty());
    let reopened = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
    assert_eq!(
        reopened.tone(0, 0).unwrap().cab_model_id(),
        Some(cab.model_id())
    );
    assert_eq!(
        reopened.tone(0, 0).unwrap().mic_choice().unwrap().value(),
        2
    );
    bundle.revert().unwrap();
    assert!(!bundle.is_dirty());
    assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), original);
}

#[test]
fn cab_mic_guards_reject_wrong_target_version_container_and_short_or_missing_cab() {
    let id = PODXT_CAB_MODEL_IDS[0];
    let choice = PodCabModelChoice::try_from(id).unwrap();
    let original = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: 0xdead_beef,
            tail: &[0x80, 0xa1, 0xb2, 0xfe],
            parameters: &[],
        }],
    );
    let mut old_version = original.clone();
    let pinf = old_version
        .windows(4)
        .position(|window| window == b"PINF")
        .unwrap()
        + 8;
    old_version[pinf + 72..pinf + 76]
        .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    let mut tone = PodTone::parse(old_version.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), old_version);
    let mut wrong_target = original.clone();
    let pinf = wrong_target
        .windows(4)
        .position(|window| window == b"PINF")
        .unwrap()
        + 8;
    wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000b_u32.to_be_bytes());
    assert!(matches!(
        PodTone::parse(wrong_target),
        Err(ToneError::WrongTarget { .. })
    ));
    let mut wrong_container = original.clone();
    wrong_container[8..12].copy_from_slice(b"L6PA");
    let mut tone = PodTone::parse(wrong_container.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), wrong_container);

    let short = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: 0xdead_beef,
            tail: &[0x80, 0xa1, 0xb2],
            parameters: &[],
        }],
    );
    let mut tone = PodTone::parse(short.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), short);

    let missing = custom_tone(ContainerKind::L6as, &[]);
    let mut tone = PodTone::parse(missing.clone()).unwrap();
    assert!(tone.set_cab_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), missing);

    let duplicate = custom_tone(
        ContainerKind::L6as,
        &[
            BuiltModel {
                slot: CAB_SLOT,
                model_id: 1,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            },
            BuiltModel {
                slot: CAB_SLOT,
                model_id: 2,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            },
        ],
    );
    assert!(PodTone::parse(duplicate).is_err());
}

#[test]
fn stomp_mod_and_delay_bypass_changes_only_the_selected_minf_byte() {
    let original = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: AMP_SLOT,
                model_id: 0x0003_0001,
                tail: &[0x10, 0x20, 0x30, 1],
                parameters: &[&[0x11, 0x22, 0x33]],
            },
            BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x0205_0006,
                tail: &[0x40, 0x50, 0x60, 1],
                parameters: &[&[0x44, 0x55]],
            },
            BuiltModel {
                slot: MOD_SLOT,
                model_id: 0x0206_0001,
                tail: &[0x70, 0x80, 0x90, 1],
                parameters: &[&[0x66]],
            },
            BuiltModel {
                slot: DELAY_SLOT,
                model_id: 0x0207_0002,
                tail: &[0xa0, 0xb0, 0xc0, 1],
                parameters: &[&[0x77, 0x88, 0x99]],
            },
        ],
    );
    let mut tone = PodTone::parse(original).unwrap();
    let identities: Vec<_> = tone
        .models()
        .iter()
        .map(|model| (model.slot, model.model_id, model.parameters.len()))
        .collect();
    for control in [
        PodModelEnable::Stomp,
        PodModelEnable::Mod,
        PodModelEnable::Delay,
    ] {
        let before = tone.as_bytes().to_vec();
        let slot = tone
            .models()
            .iter()
            .find(|model| control.slots().contains(&model.slot))
            .unwrap()
            .slot;
        let model_index = tone
            .models()
            .iter()
            .position(|model| model.slot == slot)
            .unwrap();
        let offset = tone.model_minfo_ranges[model_index].start + 11;
        tone.set_model_enabled(control, false).unwrap();
        let mut expected = before.clone();
        expected[offset] = 0;
        assert_eq!(tone.as_bytes(), expected, "{}", control.label());
        assert_eq!(
            tone.model_enable_state(control).unwrap(),
            ModelEnableState::Canonical(false)
        );
        assert_eq!(
            tone.model_enable_state(PodModelEnable::Amp).unwrap(),
            ModelEnableState::Canonical(true)
        );
        let after: Vec<_> = tone
            .models()
            .iter()
            .map(|model| (model.slot, model.model_id, model.parameters.len()))
            .collect();
        assert_eq!(after, identities);
    }
    let bypassed = tone.as_bytes().to_vec();
    tone.set_model_enabled(PodModelEnable::Stomp, false)
        .unwrap();
    assert_eq!(
        tone.as_bytes(),
        bypassed,
        "bypassing again must keep the bytes"
    );
}

#[test]
fn pod_model_enable_patches_only_the_selected_minf_byte_and_noops_same_value() {
    for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
        for initial in [0u8, 1] {
            let original = enable_tone(0xfedc_ba98, PODXT_EQ_MODEL_ID, initial, initial);
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert_eq!(
                tone.model_enable_state(control).unwrap(),
                ModelEnableState::Canonical(initial != 0)
            );
            let model_index = match control {
                PodModelEnable::Amp => 0,
                PodModelEnable::Eq => 1,
                PodModelEnable::Stomp | PodModelEnable::Mod | PodModelEnable::Delay => {
                    unreachable!("this loop covers only Amp and EQ")
                }
            };
            let enabled_offset = tone.model_minfo_ranges[model_index].start + 11;
            let desired = initial == 0;
            let mut expected = original.clone();
            expected[enabled_offset] = u8::from(desired);
            tone.set_model_enabled(control, desired).unwrap();
            assert_eq!(tone.as_bytes(), expected);
            assert_eq!(tone.models()[model_index].enabled, Some(desired));
            let after_change = tone.as_bytes().to_vec();
            tone.set_model_enabled(control, desired).unwrap();
            assert_eq!(tone.as_bytes(), after_change, "same value changed bytes");
            let (ordinal, reserved) = match control {
                PodModelEnable::Amp => (-128i8, [0xa1, 0xb2]),
                PodModelEnable::Eq => (-1, [0xc3, 0xd4]),
                PodModelEnable::Stomp | PodModelEnable::Mod | PodModelEnable::Delay => {
                    unreachable!("this loop covers only Amp and EQ")
                }
            };
            assert_eq!(tone.models()[model_index].ordinal, Some(ordinal));
            assert_eq!(tone.models()[model_index].reserved, Some(reserved));
            assert!(
                tone.models()[0]
                    .parameters
                    .iter()
                    .any(|parameter| matches!(parameter, ModelParameter::Opaque { len: 3 }))
            );
            assert_eq!(
                PodTone::parse(tone.as_bytes().to_vec()).unwrap().as_bytes(),
                expected
            );
        }
    }
}

#[test]
fn pod_model_enable_reads_signed_noncanonical_bytes_but_rejects_edits_atomically() {
    for raw in [0x02, 0x7f, 0x80, 0xff] {
        let original = enable_tone(0xdead_beef, PODXT_EQ_MODEL_ID, raw, raw);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let source_interpretation = (raw as i8) > 0;
        assert_eq!(
            tone.model_enable_state(PodModelEnable::Amp).unwrap(),
            ModelEnableState::NonCanonical {
                raw,
                source_interpretation,
            }
        );
        assert_eq!(tone.models()[0].enabled, Some(source_interpretation));
        for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
            let before = tone.clone();
            assert!(
                tone.set_model_enabled(control, source_interpretation)
                    .is_err()
            );
            assert_eq!(tone, before, "rejected edit mutated tone for {raw:02x}");
            assert!(
                tone.set_model_enabled(control, !source_interpretation)
                    .is_err()
            );
            assert_eq!(tone, before, "rejected edit mutated tone for {raw:02x}");
        }
    }
}

#[test]
fn pod_model_enable_rejects_wrong_target_missing_short_and_duplicate_models() {
    let mut wrong_eq = PodTone::parse(enable_tone(1, 0x020c_0003, 0, 0)).unwrap();
    let before = wrong_eq.clone();
    assert!(
        wrong_eq
            .set_model_enabled(PodModelEnable::Eq, true)
            .is_err()
    );
    assert_eq!(wrong_eq, before);

    let mut no_amp = PodTone::parse(custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: 1,
            tail: &[0, 0, 0, 1],
            parameters: &[],
        }],
    ))
    .unwrap();
    assert!(
        no_amp
            .set_model_enabled(PodModelEnable::Amp, false)
            .is_err()
    );

    let mut short = PodTone::parse(custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0x80, 0xa1, 0xb2],
            parameters: &[],
        }],
    ))
    .unwrap();
    let before = short.clone();
    assert!(short.set_model_enabled(PodModelEnable::Amp, false).is_err());
    assert_eq!(short, before);

    let duplicate = custom_tone(
        ContainerKind::L6as,
        &[
            BuiltModel {
                slot: AMP_SLOT,
                model_id: 1,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            },
            BuiltModel {
                slot: AMP_SLOT,
                model_id: 2,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            },
        ],
    );
    assert!(PodTone::parse(duplicate).is_err());

    let mut old_version = PodTone::parse(enable_tone(1, PODXT_EQ_MODEL_ID, 0, 0)).unwrap();
    old_version.device_version = PODXT_LIVE_HOST_MIN_VERSION - 1;
    let before = old_version.clone();
    assert!(
        old_version
            .set_model_enabled(PodModelEnable::Amp, true)
            .is_err()
    );
    assert_eq!(old_version, before);
}

#[test]
fn pod_model_enable_rejects_short_minf_before_get_or_mutation() {
    for minf_len in 8..12 {
        for following_byte in [0, 1] {
            let original = short_enable_tone(minf_len, following_byte);
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let before = tone.clone();
            for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
                assert_eq!(
                    tone.model_enable_state(control).unwrap_err().detail(),
                    format!(
                        "{} enable edit requires an existing MINF body of at least 12 bytes",
                        control.label()
                    )
                );
                for requested in [false, true] {
                    assert!(tone.set_model_enabled(control, requested).is_err());
                    assert_eq!(tone, before);
                    assert_eq!(tone.as_bytes(), original);
                }
            }

            let mut library = Library::default();
            let index = library.import_bytes(original.clone()).unwrap();
            for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
                for requested in [false, true] {
                    assert!(
                        library
                            .set_model_enabled(index, control, requested)
                            .is_err()
                    );
                    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
                    assert!(!library.is_dirty(index));
                }
            }

            let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
            let before = bundle.as_bytes().to_vec();
            for control in [PodModelEnable::Amp, PodModelEnable::Eq] {
                assert!(
                    bundle
                        .tone(0, 0)
                        .unwrap()
                        .model_enable_state(control)
                        .is_err()
                );
                for requested in [false, true] {
                    assert!(bundle.set_model_enabled(0, control, requested).is_err());
                    assert_eq!(bundle.as_bytes(), before);
                    assert!(!bundle.is_dirty());
                }
            }
        }
    }
}

#[test]
fn pod_model_enable_library_and_bundle_edits_track_dirty_revert_and_save_copy() {
    let original = enable_tone(0xdead_beef, PODXT_EQ_MODEL_ID, 0, 1);
    let wrong_eq = enable_tone(0xdead_beef, 0x020c_0003, 0, 1);
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    let rejected_index = library.import_bytes(wrong_eq.clone()).unwrap();
    assert!(
        library
            .set_model_enabled(rejected_index, PodModelEnable::Eq, true)
            .is_err()
    );
    assert_eq!(library.tone(rejected_index).unwrap().as_bytes(), wrong_eq);
    assert!(!library.is_dirty(rejected_index));
    library
        .set_model_enabled(index, PodModelEnable::Amp, true)
        .unwrap();
    assert!(library.is_dirty(index));
    let edited = library.tone(index).unwrap().as_bytes().to_vec();
    let expected_offset = library.tone(index).unwrap().model_minfo_ranges[0].start + 11;
    assert_eq!(edited[expected_offset], 1);
    library
        .set_model_enabled(index, PodModelEnable::Amp, true)
        .unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), edited);
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    assert!(!library.is_dirty(index));

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let bundle_before = bundle.as_bytes().to_vec();
    let mut rejected_bundle = PodBundleLibrary::parse(bundle_with_tone(&wrong_eq)).unwrap();
    let rejected_bytes = rejected_bundle.as_bytes().to_vec();
    assert!(
        rejected_bundle
            .set_model_enabled(0, PodModelEnable::Eq, true)
            .is_err()
    );
    assert_eq!(rejected_bundle.as_bytes(), rejected_bytes);
    assert!(!rejected_bundle.is_dirty());
    assert!(!bundle.is_dirty());
    bundle
        .set_model_enabled(0, PodModelEnable::Eq, false)
        .unwrap();
    assert!(bundle.is_dirty());
    let edited_bundle = bundle.as_bytes().to_vec();
    bundle
        .set_model_enabled(0, PodModelEnable::Eq, false)
        .unwrap();
    assert_eq!(bundle.as_bytes(), edited_bundle);
    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("enable-copy.vwb");
    bundle.save_copy(&copy).unwrap();
    assert!(bundle.save_copy(&copy).is_err());
    assert_eq!(std::fs::read(&copy).unwrap(), edited_bundle);
    let reopened = PodBundleLibrary::parse(std::fs::read(&copy).unwrap()).unwrap();
    assert_eq!(
        reopened
            .tone(0, 0)
            .unwrap()
            .model_enable_state(PodModelEnable::Eq),
        Ok(ModelEnableState::Canonical(false))
    );
    assert!(
        bundle.is_dirty(),
        "saving a copy must not move the baseline"
    );
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), bundle_before);
    assert!(!bundle.is_dirty());
}

#[test]
fn podxt_eq_band_uses_source_f32_graph_conversion_and_patches_only_its_pair() {
    assert_eq!(
        PodxtEqBand::ALL.map(PodxtEqBand::selector_ids),
        [
            (0x0009_0001, 0x0009_0002),
            (0x0009_0003, 0x0009_0004),
            (0x0009_0005, 0x0009_0006),
            (0x0009_0007, 0x0009_0008),
        ]
    );
    assert_eq!(
        PodxtEqBand::ALL.map(PodxtEqBand::parameter_ids),
        [
            (0x0010_0000, 0x0010_0001),
            (0x0010_0002, 0x0010_0003),
            (0x0010_0004, 0x0010_0005),
            (0x0010_0006, 0x0010_0007),
        ]
    );
    let mut duplicated_records: Vec<_> = (0..8)
        .map(|index| drive_record(0x3f, 0x0010_0000 + index, 1, 0x3f00_0000))
        .collect();
    duplicated_records.push(drive_record(0x3f, 0x0010_0000, 1, 0x3e00_0000));
    let duplicated_refs: Vec<_> = duplicated_records
        .iter()
        .map(|record| &record[..])
        .collect();
    let mut duplicated = PodTone::parse(eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &duplicated_refs,
    ))
    .unwrap();
    duplicated
        .set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0)
        .unwrap();
    let eq_model = duplicated
        .models
        .iter()
        .position(|model| model.slot == PODXT_EQ_SLOT)
        .unwrap();
    let matching: Vec<_> = duplicated.models[eq_model]
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            matches!(
                parameter,
                ModelParameter::Recorded {
                    id: 0x0010_0000,
                    ..
                }
            )
            .then_some(index)
        })
        .collect();
    assert_eq!(matching.len(), 2);
    let first_offset = duplicated.parameter_value_offsets[eq_model][matching[0]].unwrap();
    let last_offset = duplicated.parameter_value_offsets[eq_model][matching[1]].unwrap();
    assert_eq!(
        u32::from_be_bytes(
            duplicated.as_bytes()[first_offset..first_offset + 4]
                .try_into()
                .unwrap()
        ),
        0x3f00_0000
    );
    assert_eq!(
        u32::from_be_bytes(
            duplicated.as_bytes()[last_offset..last_offset + 4]
                .try_into()
                .unwrap()
        ),
        0x3d80_0000
    );

    let records: Vec<_> = (0..8)
        .map(|index| drive_record(0x3f, 0x0010_0000 + index as u32, 1, 0x3f00_0000).to_vec())
        .collect();
    let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
    let original = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &refs,
    );
    let interiors = [90.0f32, 90.0, 500.0, 700.0];
    for (index, band) in PodxtEqBand::ALL.into_iter().enumerate() {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_podxt_eq_band(band, interiors[index], 6.3f32)
            .unwrap();
        let (frequency_id, gain_id) = band.parameter_ids();
        let eq_model = tone
            .models
            .iter()
            .position(|model| model.slot == PODXT_EQ_SLOT)
            .unwrap();
        let frequency_index = tone.models[eq_model].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == frequency_id)).unwrap();
        let gain_index = tone.models[eq_model].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == gain_id)).unwrap();
        assert_eq!(
            tone.parameter_value_offsets[eq_model][frequency_index].map(|offset| {
                u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap())
            }),
            Some(0x3d80_0000)
        );
        assert_eq!(
            tone.parameter_value_offsets[eq_model][gain_index].map(|offset| u32::from_be_bytes(
                tone.as_bytes()[offset..offset + 4].try_into().unwrap()
            )),
            Some(0x3f40_8102)
        );
        let mut expected = original.clone();
        let frequency_offset = tone.parameter_value_offsets[eq_model][frequency_index].unwrap();
        let gain_offset = tone.parameter_value_offsets[eq_model][gain_index].unwrap();
        expected[frequency_offset..frequency_offset + 4]
            .copy_from_slice(&0x3d80_0000u32.to_be_bytes());
        expected[gain_offset..gain_offset + 4].copy_from_slice(&0x3f40_8102u32.to_be_bytes());
        assert_eq!(tone.as_bytes(), expected);
    }

    let mut tone = PodTone::parse(original.clone()).unwrap();
    tone.set_podxt_eq_band(PodxtEqBand::One, 209.999f32, -12.8f32)
        .unwrap();
    let (frequency_id, gain_id) = PodxtEqBand::One.parameter_ids();
    let model_index = tone
        .models
        .iter()
        .position(|model| model.slot == PODXT_EQ_SLOT)
        .unwrap();
    for (id, expected) in [(frequency_id, 0x3e7f_ff96), (gain_id, 0x0000_0000)] {
        let parameter = tone.models[model_index].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id: found, .. } if *found == id)).unwrap();
        let offset = tone.parameter_value_offsets[model_index][parameter].unwrap();
        assert_eq!(
            u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap()),
            expected
        );
    }

    let preceding = [
        [0x3dff_fffd, 0x3eff_fffe, 0x3f7f_fffe],
        [0x3dff_fffd, 0x3eff_ffff, 0x3f7f_ffff],
        [0x3dff_ffff, 0x3eff_ffff, 0x3f7f_ffff],
        [0x3dff_fffd, 0x3eff_ffff, 0x3f7f_ffff],
    ];
    for (band_index, band) in PodxtEqBand::ALL.into_iter().enumerate() {
        for (boundary_index, point) in [1usize, 4, 8].into_iter().enumerate() {
            let boundary = PODXT_EQ_FREQUENCY_TABLES[band_index][point] as f32;
            let adjacent = f32::from_bits(boundary.to_bits() - 1);
            let mut tone = PodTone::parse(original.clone()).unwrap();
            tone.set_podxt_eq_band(band, adjacent, 0.0).unwrap();
            let (frequency_id, _) = band.parameter_ids();
            let model_index = tone
                .models
                .iter()
                .position(|model| model.slot == PODXT_EQ_SLOT)
                .unwrap();
            let parameter_index = tone.models[model_index].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == frequency_id)).unwrap();
            let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
            assert_eq!(
                u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap()),
                preceding[band_index][boundary_index]
            );
            tone.set_podxt_eq_band(band, boundary, 12.6).unwrap();
            let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
            assert_eq!(
                u32::from_be_bytes(tone.as_bytes()[offset..offset + 4].try_into().unwrap()),
                [0x3e00_0000, 0x3f00_0000, 0x3f80_0000][boundary_index]
            );
            let (_, gain_id) = band.parameter_ids();
            let gain_index = tone.models[model_index].parameters.iter().rposition(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == gain_id)).unwrap();
            let gain_offset = tone.parameter_value_offsets[model_index][gain_index].unwrap();
            assert_eq!(
                u32::from_be_bytes(
                    tone.as_bytes()[gain_offset..gain_offset + 4]
                        .try_into()
                        .unwrap()
                ),
                0x3f80_0000
            );
        }
    }
}

#[test]
fn podxt_eq_display_matches_source_endpoints_and_double_key_boundaries() {
    let bands = PodxtEqBand::ALL;
    let first = [0x4248_0000, 0x4248_0000, 0x42c8_0000, 0x43fa_0000];
    let last = [0x442c_8000, 0x45bd_1000, 0x4630_9000, 0x4611_5000];
    let boundary_cases = [
        (
            [0x3b81_0203, 0x3b81_0204, 0x3b81_0205],
            [
                [0x4248_0000, 0x4248_0000, 0x425c_0000],
                [0x4248_0000, 0x4248_0000, 0x425c_0000],
                [0x42c8_0000, 0x42c8_0000, 0x4316_0000],
                [0x43fa_0000, 0x43fa_0000, 0x4403_4000],
            ],
        ),
        (
            [0x3df9_f3e7, 0x3df9_f3e8, 0x3df9_f3e9],
            [
                [0x42fa_0000, 0x4302_0000, 0x4302_0000],
                [0x42fa_0000, 0x4302_0000, 0x4302_0000],
                [0x4454_8000, 0x4461_0000, 0x4461_0000],
                [0x445a_c000, 0x4461_0000, 0x4461_0000],
            ],
        ),
        (
            [0x3f7e_fdfb, 0x3f7e_fdfc, 0x3f7e_fdfd],
            [
                [0x442a_0000, 0x442c_8000, 0x442c_8000],
                [0x45b6_d000, 0x45bd_1000, 0x45bd_1000],
                [0x462d_7000, 0x4630_9000, 0x4630_9000],
                [0x460e_3000, 0x4611_5000, 0x4611_5000],
            ],
        ),
    ];
    for (band_index, band) in bands.into_iter().enumerate() {
        for (raw, expected) in [
            (0u32, first[band_index]),
            (0x3f80_0000, last[band_index]),
            (0x3b81_0204, first[band_index]),
        ] {
            let refs: Vec<_> = (0..8)
                .map(|i| drive_record(0x3f, 0x0010_0000 + i, 1, if i % 2 == 0 { raw } else { 0 }))
                .collect();
            let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
            let tone = PodTone::parse(eq_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                PODXT_EQ_MODEL_ID,
                PODXT_EQ_SLOT,
                &refs,
            ))
            .unwrap();
            let (frequency, _) = tone.podxt_eq_band_display_values(band).unwrap();
            assert_eq!(
                frequency.to_bits(),
                expected,
                "band {band_index}, raw {raw:#010x}"
            );
        }
        for (raws, by_band) in boundary_cases {
            for (raw, expected) in raws.into_iter().zip(by_band[band_index]) {
                let refs: Vec<_> = (0..8)
                    .map(|i| {
                        drive_record(0x3f, 0x0010_0000 + i, 1, if i % 2 == 0 { raw } else { 0 })
                    })
                    .collect();
                let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
                let tone = PodTone::parse(eq_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    PODXT_EQ_MODEL_ID,
                    PODXT_EQ_SLOT,
                    &refs,
                ))
                .unwrap();
                let (frequency, _) = tone.podxt_eq_band_display_values(band).unwrap();
                assert_eq!(
                    frequency.to_bits(),
                    expected,
                    "band {band_index}, boundary raw {raw:#010x}"
                );
            }
        }
        let refs: Vec<_> = (0..8)
            .map(|i| drive_record(0x3f, 0x0010_0000 + i, 1, 0x3f80_0000))
            .collect();
        let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
        let mut tone = PodTone::parse(eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &refs,
        ))
        .unwrap();
        let (_, gain) = tone.podxt_eq_band_display_values(band).unwrap();
        assert_eq!(gain.to_bits(), 0x4149_999a);
        tone.set_podxt_eq_band(band, f32::from_bits(last[band_index]), gain)
            .unwrap();
        let (frequency_word, gain_word) = tone.podxt_eq_band_raw_values(band).unwrap();
        assert_eq!(frequency_word.to_bits(), 0x3f80_0000);
        assert_eq!(gain_word.to_bits(), 0x3f80_0000);

        for (raw_gain, expected) in [
            (0u32, 0xc14c_cccd),
            (1, 0xc14c_cccd),
            (0x3f7f_fffe, 0x4149_9998),
            (0x3f7f_ffff, 0x4149_9999),
            (0x3f80_0000, 0x4149_999a),
        ] {
            let refs: Vec<_> = (0..8)
                .map(|i| {
                    drive_record(
                        0x3f,
                        0x0010_0000 + i,
                        1,
                        if i % 2 == 0 { 0x3f80_0000 } else { raw_gain },
                    )
                })
                .collect();
            let refs: Vec<_> = refs.iter().map(|v| &v[..]).collect();
            let tone = PodTone::parse(eq_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                PODXT_EQ_MODEL_ID,
                PODXT_EQ_SLOT,
                &refs,
            ))
            .unwrap();
            let (_, displayed) = tone.podxt_eq_band_display_values(band).unwrap();
            assert_eq!(
                displayed.to_bits(),
                expected,
                "band {band_index}, gain raw {raw_gain:#010x}"
            );
        }
    }
}

#[test]
fn podxt_eq_pair_rejects_invalid_inputs_and_targets_without_partial_mutation() {
    let good = [
        drive_record(0x3f, 0x0010_0000, 1, 0x3f00_0000),
        drive_record(0x3f, 0x0010_0001, 1, 0x3f00_0000),
    ];
    let good_refs: Vec<_> = good.iter().map(|record| &record[..]).collect();
    let original = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &good_refs,
    );
    for (frequency, gain) in [
        (f32::NAN, 0.0),
        (f32::INFINITY, 0.0),
        (49.0, 0.0),
        (691.0, 0.0),
        (50.0, f32::NAN),
        (50.0, f32::NEG_INFINITY),
        (50.0, -12.81),
        (50.0, 12.61),
    ] {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert!(
            tone.set_podxt_eq_band(PodxtEqBand::One, frequency, gain)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), original);
    }
    let invalid_current = drive_record(0x3f, 0x0010_0001, 1, f32::NAN.to_bits());
    let invalid_refs: Vec<_> = [&good[0][..], &invalid_current[..]].into_iter().collect();
    let invalid = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &invalid_refs,
    );
    let mut tone = PodTone::parse(invalid.clone()).unwrap();
    assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
    assert_eq!(tone.as_bytes(), invalid);
    for invalid_bits in [
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        0x3f80_0001,
    ] {
        let bad = drive_record(0x3f, 0x0010_0001, 1, invalid_bits);
        let refs: Vec<_> = [&good[0][..], &bad[..]].into_iter().collect();
        let bytes = eq_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &refs,
        );
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
        assert_eq!(tone.as_bytes(), bytes);
    }

    for (version, model, slot, params) in [
        (
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &good_refs[..],
        ),
        (
            PODXT_LIVE_HOST_MIN_VERSION,
            0x020c_0003,
            PODXT_EQ_SLOT,
            &good_refs[..],
        ),
        (
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT + 1,
            &good_refs[..],
        ),
        (
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &good_refs[..1],
        ),
    ] {
        let bytes = eq_tone(version, model, slot, params);
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
        assert_eq!(tone.as_bytes(), bytes);
    }
    let bad_gain = drive_record(0x3e, 0x0010_0001, 1, 0x3f00_0000);
    let bad_refs: Vec<_> = [&good[0][..], &bad_gain[..]].into_iter().collect();
    let bytes = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &bad_refs,
    );
    let mut tone = PodTone::parse(bytes.clone()).unwrap();
    assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
    assert_eq!(tone.as_bytes(), bytes);

    let wrong_type = drive_record(0x3f, 0x0010_0001, 0, 0x3f00_0000);
    let wrong_type_refs: Vec<_> = [&good[0][..], &wrong_type[..]].into_iter().collect();
    let bytes = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &wrong_type_refs,
    );
    let mut tone = PodTone::parse(bytes.clone()).unwrap();
    assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
    assert_eq!(tone.as_bytes(), bytes);

    let opaque_refs: Vec<_> = [&good[0][..], &good[1][..], &[0x12, 0x34, 0x56][..]]
        .into_iter()
        .collect();
    let bytes = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &opaque_refs,
    );
    let mut tone = PodTone::parse(bytes.clone()).unwrap();
    assert!(tone.set_podxt_eq_band(PodxtEqBand::One, 90.0, 0.0).is_err());
    assert_eq!(tone.as_bytes(), bytes);

    let unknown = drive_record(0x3f, 0x0010_0001, 1, 0x3f80_0001);
    let unknown_refs: Vec<_> = [&good[0][..], &unknown[..]].into_iter().collect();
    let bytes = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &unknown_refs,
    );
    let tone = PodTone::parse(bytes.clone()).unwrap();
    assert_eq!(
        tone.podxt_eq_band_raw_values(PodxtEqBand::One)
            .unwrap()
            .1
            .to_bits(),
        0x3f80_0001
    );
    assert!(tone.podxt_eq_band_display_values(PodxtEqBand::One).is_err());
    assert_eq!(tone.as_bytes(), bytes);
}

#[test]
fn podxt_eq_reset_is_atomic_and_writes_only_the_eight_independent_words() {
    const RESET: [u32; 8] = [
        0x3e00_0000,
        0x3f01_0204,
        0x3eec_0000,
        0x3f01_0204,
        0x3ef0_0000,
        0x3f01_0204,
        0x3f5c_0000,
        0x3f01_0204,
    ];
    let records: Vec<_> = (0..8)
        .map(|index| {
            drive_record(
                0x3f,
                0x0010_0000 + index,
                1,
                f32::from_bits(0x3e80_0000 + (index << 12)).to_bits(),
            )
        })
        .collect();
    let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
    let original = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: PODXT_EQ_SLOT,
            model_id: PODXT_EQ_MODEL_ID,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters: &refs,
        }],
    );
    let before = PodTone::parse(original.clone()).unwrap();
    let targets = before.podxt_eq_reset_targets().unwrap();
    let mut expected = original.clone();
    for ((offset, _), word) in targets.iter().zip(RESET) {
        expected[*offset..*offset + 4].copy_from_slice(&word.to_be_bytes());
    }

    let mut tone = before;
    tone.reset_podxt_eq().unwrap();
    assert_eq!(tone.as_bytes(), expected);
    assert_eq!(
        tone.models()[0]
            .parameters
            .iter()
            .filter_map(|parameter| match parameter {
                ModelParameter::Recorded {
                    value: ParameterValue::Float { bits },
                    ..
                } => Some(*bits),
                _ => None,
            })
            .collect::<Vec<_>>(),
        RESET
    );
    let already_reset = tone.as_bytes().to_vec();
    tone.reset_podxt_eq().unwrap();
    assert_eq!(tone.as_bytes(), already_reset);
}

#[test]
fn podxt_eq_reset_matches_an_independent_full_envelope_and_literal_write_mask() {
    let before_hex = "
        464f524d000001804c3650414f44442100000003172839a550494e4600000050000000010003000a0041000000000000
        000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
        0000000000000000030000005aa596694c49535400000110504154434c495354000000c44d4f444c4d494e460000000c
        020c000200030004a1b2c3d45041524d0000000c3f100000000000013e8000005041524d0000000c3f10000100000001
        3f0000005041524d0000000c3f100002000000013e4000005041524d0000000c3f100003000000013f4000005041524d
        0000000c3f100004000000013e2000005041524d0000000c3f100005000000013f2000005041524d0000000c3f100006
        000000013e1000005041524d0000000c3f100007000000013e0000005854524100000003deadbea54c49535400000038
        4d4f444c4d494e460000000c0200000100030000112233445041524d0000000c3f00abcd000000001234567858545241
        00000003cafe01a5
    ";
    let after_hex = "
        464f524d000001804c3650414f44442100000003172839a550494e4600000050000000010003000a0041000000000000
        000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
        0000000000000000030000005aa596694c49535400000110504154434c495354000000c44d4f444c4d494e460000000c
        020c000200030004a1b2c3d45041524d0000000c3f100000000000013e0000005041524d0000000c3f10000100000001
        3f0102045041524d0000000c3f100002000000013eec00005041524d0000000c3f100003000000013f0102045041524d
        0000000c3f100004000000013ef000005041524d0000000c3f100005000000013f0102045041524d0000000c3f100006
        000000013f5c00005041524d0000000c3f100007000000013f0102045854524100000003deadbea54c49535400000038
        4d4f444c4d494e460000000c0200000100030000112233445041524d0000000c3f00abcd000000001234567858545241
        00000003cafe01a5
    ";
    let decode = |hex: &str| {
        hex.bytes()
            .filter(u8::is_ascii_hexdigit)
            .collect::<Vec<_>>()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let before = decode(before_hex);
    let expected = decode(after_hex);
    assert_eq!(before.len(), 392);
    assert_eq!(expected.len(), before.len());

    // Offsets are literal envelope coordinates, authored from the independent
    // chunk layout above; the production target finder is intentionally unused.
    let value_offsets = [0xac, 0xc0, 0xd4, 0xe8, 0xfc, 0x110, 0x124, 0x138];
    let reset_words = [
        0x3e00_0000u32,
        0x3f01_0204,
        0x3eec_0000,
        0x3f01_0204,
        0x3ef0_0000,
        0x3f01_0204,
        0x3f5c_0000,
        0x3f01_0204,
    ];
    let previous_words = [
        0x3e80_0000u32,
        0x3f00_0000,
        0x3e40_0000,
        0x3f40_0000,
        0x3e20_0000,
        0x3f20_0000,
        0x3e10_0000,
        0x3e00_0000,
    ];
    for ((offset, before_word), after_word) in value_offsets
        .into_iter()
        .zip(previous_words)
        .zip(reset_words)
    {
        assert_eq!(&before[offset..offset + 4], &before_word.to_be_bytes());
        assert_eq!(&expected[offset..offset + 4], &after_word.to_be_bytes());
    }
    let permitted: Vec<_> = value_offsets
        .into_iter()
        .flat_map(|offset| offset..offset + 4)
        .collect();
    assert!(
        before
            .iter()
            .zip(&expected)
            .enumerate()
            .all(|(offset, (old, new))| old == new || permitted.contains(&offset))
    );

    let mut tone = PodTone::parse(before).unwrap();
    tone.reset_podxt_eq().unwrap();
    assert_eq!(tone.as_bytes(), expected);
    let reparsed = PodTone::parse(expected.clone()).unwrap();
    assert_eq!(reparsed.as_bytes(), expected);
}

#[test]
fn podxt_eq_reset_rejects_unsupported_target_without_mutation() {
    let records: Vec<_> = (0..8)
        .map(|index| drive_record(0x3f, 0x0010_0000 + index, 1, 0x3f00_0000))
        .collect();
    let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
    for (kind, version, model, slot, parameters) in [
        (
            ContainerKind::L6as,
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &refs[..],
        ),
        (
            ContainerKind::L6pa,
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &refs[..],
        ),
        (
            ContainerKind::L6pa,
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID + 1,
            PODXT_EQ_SLOT,
            &refs[..],
        ),
        (
            ContainerKind::L6pa,
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT + 1,
            &refs[..],
        ),
        (
            ContainerKind::L6pa,
            PODXT_LIVE_HOST_MIN_VERSION,
            PODXT_EQ_MODEL_ID,
            PODXT_EQ_SLOT,
            &refs[..7],
        ),
    ] {
        let mut bytes = custom_tone(
            kind,
            &[BuiltModel {
                slot,
                model_id: model,
                tail: &[1, 2, 3, 4],
                parameters,
            }],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76].copy_from_slice(&version.to_be_bytes());
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.reset_podxt_eq().is_err());
        assert_eq!(tone.as_bytes(), bytes);
    }

    let bad_last = drive_record(0x3f, 0x0010_0007, 1, f32::NAN.to_bits());
    let mut bad_records = records[..7].to_vec();
    bad_records.push(bad_last);
    let bad_refs: Vec<_> = bad_records.iter().map(|record| &record[..]).collect();
    let bytes = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: PODXT_EQ_SLOT,
            model_id: PODXT_EQ_MODEL_ID,
            tail: &[1, 2, 3, 4],
            parameters: &bad_refs,
        }],
    );
    let mut tone = PodTone::parse(bytes.clone()).unwrap();
    let parsed_before = tone.models().to_vec();
    assert!(tone.reset_podxt_eq().is_err());
    assert_eq!(tone.as_bytes(), bytes, "late guard failure is atomic");
    assert_eq!(
        tone.models(),
        parsed_before,
        "late failure preserves parsed state"
    );
    let mut library = Library::default();
    library.import_bytes(bytes.clone()).unwrap();
    assert!(library.reset_podxt_eq(0).is_err());
    assert_eq!(library.tone(0).unwrap().as_bytes(), bytes);
    assert!(
        !library.is_dirty(0),
        "late failure preserves saved baseline"
    );
}

#[test]
fn podxt_eq_reset_checks_each_parameter_guard_and_duplicate_model_context() {
    fn make_tone(parameters: &[&[u8]], models: Option<&[BuiltModel<'_>]>) -> Vec<u8> {
        let fallback = [BuiltModel {
            slot: PODXT_EQ_SLOT,
            model_id: PODXT_EQ_MODEL_ID,
            tail: &[1, 2, 3, 4],
            parameters,
        }];
        custom_tone(ContainerKind::L6pa, models.unwrap_or(&fallback))
    }
    let records: Vec<_> = (0..8)
        .map(|index| drive_record(0x3f, 0x0010_0000 + index as u32, 1, 0x3f00_0000).to_vec())
        .collect();
    let reset_words = [
        0x3e00_0000u32,
        0x3f01_0204,
        0x3eec_0000,
        0x3f01_0204,
        0x3ef0_0000,
        0x3f01_0204,
        0x3f5c_0000,
        0x3f01_0204,
    ];

    for target in 0..8 {
        let mut cases = Vec::new();
        let missing: Vec<_> = records
            .iter()
            .enumerate()
            .filter_map(|(index, record)| (index != target).then_some(record.clone()))
            .collect();
        cases.push(missing);

        let mut duplicate = records.clone();
        duplicate.push(drive_record(0x3e, 0x0010_0000 + target as u32, 1, 0x3f00_0000).to_vec());
        cases.push(duplicate);

        let mut wrong_prefix = records.clone();
        wrong_prefix[target] =
            drive_record(0x3e, 0x0010_0000 + target as u32, 1, 0x3f00_0000).to_vec();
        cases.push(wrong_prefix);

        let mut wrong_type = records.clone();
        wrong_type[target] =
            drive_record(0x3f, 0x0010_0000 + target as u32, 0, 0x3f00_0000).to_vec();
        cases.push(wrong_type);

        let mut malformed_length = records.clone();
        let mut short = Vec::new();
        short.extend_from_slice(&(0x3f00_0000 | (0x0010_0000 + target as u32)).to_be_bytes());
        short.extend_from_slice(&1u32.to_be_bytes());
        malformed_length.remove(target);
        malformed_length.insert(target, short);
        cases.push(malformed_length);

        for case in cases {
            let refs: Vec<_> = case.iter().map(|record| record.as_slice()).collect();
            let bytes = make_tone(&refs, None);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            let parsed_before = tone.models().to_vec();
            assert!(tone.reset_podxt_eq().is_err(), "target PARM {target}");
            assert_eq!(
                tone.as_bytes(),
                bytes,
                "failure mutated target PARM {target}"
            );
            assert_eq!(tone.models(), parsed_before);
        }

        let boundary_records: Vec<_> = (0..8)
            .map(|index| {
                let value = if index == target { 0.0f32 } else { 1.0f32 };
                drive_record(0x3f, 0x0010_0000 + index as u32, 1, value.to_bits()).to_vec()
            })
            .collect();
        let refs: Vec<_> = boundary_records
            .iter()
            .map(|record| record.as_slice())
            .collect();
        let bytes = make_tone(&refs, None);
        let mut tone = PodTone::parse(bytes).unwrap();
        tone.reset_podxt_eq().unwrap();
        for (index, word) in reset_words.iter().enumerate() {
            let values = tone
                .podxt_eq_band_raw_values(PodxtEqBand::ALL[index / 2])
                .unwrap();
            let actual = if index % 2 == 0 { values.0 } else { values.1 };
            assert_eq!(actual.to_bits(), *word);
        }

        for invalid_value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
            let invalid_records: Vec<_> = (0..8)
                .map(|index| {
                    let value = if index == target { invalid_value } else { 0.5 };
                    drive_record(0x3f, 0x0010_0000 + index as u32, 1, value.to_bits()).to_vec()
                })
                .collect();
            let refs: Vec<_> = invalid_records
                .iter()
                .map(|record| record.as_slice())
                .collect();
            let bytes = make_tone(&refs, None);
            let mut library = Library::default();
            library.import_bytes(bytes.clone()).unwrap();
            assert!(!library.is_dirty(0));
            assert!(library.reset_podxt_eq(0).is_err());
            assert_eq!(library.tone(0).unwrap().as_bytes(), bytes);
            assert!(!library.is_dirty(0), "failure changed dirty baseline");
            library.revert_unsaved(0).unwrap();
            assert_eq!(library.tone(0).unwrap().as_bytes(), bytes);
        }
    }

    let short_opaque = [0x3f, 0x10, 0x00];
    let refs: Vec<_> = records
        .iter()
        .map(|record| record.as_slice())
        .chain([&short_opaque[..]])
        .collect();
    let bytes = make_tone(&refs, None);
    let mut tone = PodTone::parse(bytes.clone()).unwrap();
    assert!(tone.reset_podxt_eq().is_err());
    assert_eq!(tone.as_bytes(), bytes, "short opaque EQ PARM is ambiguous");

    let duplicate_model_opaque = [0x11, 0x22, 0x33];
    let duplicate_model_parameters = [&duplicate_model_opaque[..]];
    let sibling_opaque = [0x44, 0x55, 0x66];
    let sibling_parameters = [&sibling_opaque[..]];
    for already_reset in [false, true] {
        let canonical_records: Vec<_> = reset_words
            .iter()
            .enumerate()
            .map(|(index, word)| {
                let bits = if already_reset { *word } else { 0x3f00_0000 };
                drive_record(0x3f, 0x0010_0000 + index as u32, 1, bits).to_vec()
            })
            .collect();
        let canonical_parameters: Vec<_> = canonical_records
            .iter()
            .map(|record| record.as_slice())
            .collect();
        let duplicate_records: Vec<_> = (0..8)
            .map(|index| {
                let bits = if already_reset {
                    reset_words[index] ^ 0x0000_0001
                } else {
                    0x3e80_0000 + index as u32
                };
                drive_record(0x3f, 0x0010_0000 + index as u32, 1, bits).to_vec()
            })
            .collect();
        let duplicate_parameters: Vec<_> = duplicate_records
            .iter()
            .map(|record| record.as_slice())
            .collect();

        let canonical_eq = BuiltModel {
            slot: PODXT_EQ_SLOT,
            model_id: PODXT_EQ_MODEL_ID,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters: &canonical_parameters,
        };
        let sibling = BuiltModel {
            slot: AMP_SLOT,
            model_id: 0x1111_0002,
            tail: &[0xe1, 0xf2, 0x83, 0x94],
            parameters: &sibling_parameters,
        };

        for (duplicate_eq_model, conflict_tail) in [
            (true, [0x05, 0x16, 0x27, 0x38]),
            (false, [0x49, 0x5a, 0x6b, 0x7c]),
        ] {
            let conflicting_model = if duplicate_eq_model {
                BuiltModel {
                    slot: PODXT_EQ_SLOT + 1,
                    model_id: PODXT_EQ_MODEL_ID,
                    tail: &conflict_tail,
                    parameters: &duplicate_parameters,
                }
            } else {
                BuiltModel {
                    slot: PODXT_EQ_SLOT,
                    model_id: PODXT_EQ_MODEL_ID + 1,
                    tail: &conflict_tail,
                    parameters: &duplicate_model_parameters,
                }
            };
            let models = [canonical_eq, conflicting_model, sibling];
            let bytes = custom_tone(ContainerKind::L6pa, &models);
            let mut tone = if duplicate_eq_model {
                PodTone::parse(bytes.clone()).unwrap()
            } else {
                // A duplicate fixed slot is rejected by the file parser, so
                // exercise the reset guard with the parsed state it must
                // reject while retaining the actual conflicting raw file.
                assert!(PodTone::parse(bytes.clone()).is_err());
                let valid_models = [canonical_eq, sibling];
                let mut parsed =
                    PodTone::parse(custom_tone(ContainerKind::L6pa, &valid_models)).unwrap();
                let conflicting_only =
                    PodTone::parse(custom_tone(ContainerKind::L6pa, &[conflicting_model])).unwrap();
                parsed.bytes = bytes.clone();
                parsed.models.insert(1, conflicting_only.models[0].clone());
                parsed
                    .model_minfo_ranges
                    .insert(1, conflicting_only.model_minfo_ranges[0].clone());
                parsed
                    .model_chunk_ranges
                    .insert(1, conflicting_only.model_chunk_ranges[0].clone());
                parsed
                    .parameter_value_offsets
                    .insert(1, conflicting_only.parameter_value_offsets[0].clone());
                parsed
                    .opaque_parameter_id_hints
                    .insert(1, conflicting_only.opaque_parameter_id_hints[0].clone());
                parsed
            };
            let parsed_before = tone.models().to_vec();
            assert_eq!(tone.models().len(), 3);
            assert_eq!(tone.models()[0].slot, PODXT_EQ_SLOT);
            assert_eq!(tone.models()[0].model_id, PODXT_EQ_MODEL_ID);
            assert_eq!(tone.models()[2].slot, AMP_SLOT);
            assert_eq!(tone.models()[2].model_id, 0x1111_0002);
            if duplicate_eq_model {
                assert_eq!(tone.models()[1].model_id, PODXT_EQ_MODEL_ID);
                assert_ne!(tone.models()[1].slot, PODXT_EQ_SLOT);
            } else {
                assert_eq!(tone.models()[1].slot, PODXT_EQ_SLOT);
                assert_ne!(tone.models()[1].model_id, PODXT_EQ_MODEL_ID);
            }
            assert_eq!(tone.as_bytes(), bytes, "fixture bytes differ before reset");
            let detail =
                format!("duplicate_eq_model={duplicate_eq_model}, already_reset={already_reset}");
            assert!(tone.reset_podxt_eq().is_err(), "{detail}");
            assert_eq!(tone.as_bytes(), bytes, "raw bytes changed: {detail}");
            assert_eq!(
                tone.models(),
                parsed_before,
                "parsed state changed: {detail}"
            );

            if duplicate_eq_model {
                let mut library = Library::default();
                library.import_bytes(bytes.clone()).unwrap();
                assert!(!library.is_dirty(0));
                assert!(library.reset_podxt_eq(0).is_err(), "{detail}");
                assert_eq!(library.tone(0).unwrap().as_bytes(), bytes, "{detail}");
                assert_eq!(library.tone(0).unwrap().models(), parsed_before, "{detail}");
                assert!(
                    !library.is_dirty(0),
                    "saved Library baseline changed: {detail}"
                );
                library.revert_unsaved(0).unwrap();
                assert_eq!(library.tone(0).unwrap().as_bytes(), bytes, "{detail}");

                let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&bytes)).unwrap();
                let bundle_baseline = bundle.as_bytes().to_vec();
                assert!(!bundle.is_dirty());
                assert!(bundle.reset_podxt_eq(0).is_err(), "{detail}");
                assert_eq!(
                    bundle.as_bytes(),
                    bundle_baseline,
                    "bundle changed: {detail}"
                );
                assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), bytes, "{detail}");
                assert!(
                    !bundle.is_dirty(),
                    "saved Bundle baseline changed: {detail}"
                );
                bundle.revert().unwrap();
                assert_eq!(bundle.as_bytes(), bundle_baseline, "{detail}");
            } else {
                // The file parser refuses duplicate fixed slots. Confirm a
                // failed import also leaves an existing saved Library entry
                // and its baseline untouched.
                let mut library = Library::default();
                let valid_models = [canonical_eq, sibling];
                let valid_bytes = custom_tone(ContainerKind::L6pa, &valid_models);
                library.import_bytes(valid_bytes.clone()).unwrap();
                assert!(!library.is_dirty(0));
                assert!(library.import_bytes(bytes.clone()).is_err());
                assert_eq!(library.tone(0).unwrap().as_bytes(), valid_bytes);
                assert!(!library.is_dirty(0));
                library.revert_unsaved(0).unwrap();
                assert_eq!(library.tone(0).unwrap().as_bytes(), valid_bytes);
            }
        }
    }
}

#[test]
fn podxt_eq_reset_library_and_bundle_paths_keep_revert_and_copy_baselines() {
    let records: Vec<_> = (0..8)
        .map(|index| drive_record(0x3f, 0x0010_0000 + index, 1, 0x3f00_0000))
        .collect();
    let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
    let original = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: PODXT_EQ_SLOT,
            model_id: PODXT_EQ_MODEL_ID,
            tail: &[0x91, 0x82, 0x73, 0x64],
            parameters: &refs,
        }],
    );
    let mut expected = PodTone::parse(original.clone()).unwrap();
    expected.reset_podxt_eq().unwrap();
    assert_eq!(
        crate::rig::Session::initial_rig()
            .reset_pod_eq(crate::rig::RoleKind::Pod, &original)
            .unwrap(),
        expected.as_bytes()
    );
    assert!(
        crate::rig::Session::initial_rig()
            .reset_pod_eq(crate::rig::RoleKind::Guitar, &original)
            .is_err()
    );

    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    library.reset_podxt_eq(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), expected.as_bytes());
    assert!(library.is_dirty(index));
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    assert!(!library.is_dirty(index));

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let before = bundle.as_bytes().to_vec();
    bundle.reset_podxt_eq(0).unwrap();
    assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), expected.as_bytes());
    assert!(bundle.is_dirty());
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("occupied-reset-copy.vwb");
    std::fs::write(&collision, b"preserve existing file").unwrap();
    let edited_bundle = bundle.as_bytes().to_vec();
    assert!(bundle.save_copy(&collision).is_err());
    assert_eq!(
        std::fs::read(&collision).unwrap(),
        b"preserve existing file"
    );
    assert_eq!(bundle.as_bytes(), edited_bundle);
    assert!(bundle.is_dirty(), "failed copy preserves edited state");
    let destination = directory.path().join("reset-copy.vwb");
    bundle.save_copy(&destination).unwrap();
    assert_eq!(
        PodBundleLibrary::parse(std::fs::read(&destination).unwrap())
            .unwrap()
            .tone(0, 0)
            .unwrap()
            .as_bytes(),
        expected.as_bytes()
    );
    assert!(bundle.is_dirty(), "saving a copy must keep the baseline");
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), before);
    assert!(!bundle.is_dirty());
}

#[test]
fn podxt_eq_library_and_bundle_edits_revert_save_and_reopen_offline() {
    let records: Vec<_> = (0..8)
        .map(|index| drive_record(0x3f, 0x0010_0000 + index, 1, 0x3f00_0000))
        .collect();
    let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
    let original = eq_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PODXT_EQ_MODEL_ID,
        PODXT_EQ_SLOT,
        &refs,
    );

    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    assert!(!library.is_dirty(index));
    library
        .set_podxt_eq_band(index, PodxtEqBand::Two, 90.0, 0.0)
        .unwrap();
    assert!(library.is_dirty(index));
    assert_eq!(
        library
            .tone(index)
            .unwrap()
            .podxt_eq_band_raw_values(PodxtEqBand::Two)
            .unwrap(),
        (0.0625, 0.503_937)
    );
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    assert!(!library.is_dirty(index));

    let bundle_bytes = bundle_with_tone(&original);
    let mut bundle = PodBundleLibrary::parse(bundle_bytes).unwrap();
    bundle
        .set_podxt_eq_band(0, PodxtEqBand::Four, 700.0, 6.3)
        .unwrap();
    assert!(bundle.is_dirty());
    let location = bundle.entry_location(0).unwrap();
    assert_eq!(
        bundle
            .tone(location.0, location.1)
            .unwrap()
            .podxt_eq_band_raw_values(PodxtEqBand::Four)
            .unwrap(),
        (0.0625, f32::from_bits(0x3f40_8102))
    );
    bundle.revert().unwrap();
    assert!(!bundle.is_dirty());
    assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), original);

    bundle
        .set_podxt_eq_band(0, PodxtEqBand::One, 90.0, 0.0)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("eq-copy.l6b");
    bundle.save_copy(&copy).unwrap();
    let reopened = PodBundleLibrary::parse(std::fs::read(copy).unwrap()).unwrap();
    assert_eq!(
        reopened
            .tone(0, 0)
            .unwrap()
            .podxt_eq_band_raw_values(PodxtEqBand::One)
            .unwrap(),
        (0.0625, 0.503_937)
    );
}

#[test]
fn amp_drive_accepts_host_scale_values_and_changes_only_the_last_raw_value_word() {
    let first = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0x3e80_0000);
    let last = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0x3f00_0000);
    let unknown = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
    let original = drive_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PLEXI_LEAD_100_MODEL,
        &[&first, &unknown, &last],
    );
    let mut tone = PodTone::parse(original.clone()).unwrap();
    assert_eq!(tone.amp_drive_fraction().unwrap(), 0.5);
    tone.set_amp_drive_fraction(0.317).unwrap();
    let changed: Vec<_> = original
        .iter()
        .zip(tone.as_bytes())
        .enumerate()
        .filter_map(|(index, (before, after))| (before != after).then_some(index))
        .collect();
    let last_at = original
        .windows(12)
        .position(|window| window == last)
        .unwrap();
    assert_eq!(changed, (last_at + 8..last_at + 12).collect::<Vec<_>>());
    assert_eq!(
        tone.amp_drive_fraction().unwrap().to_bits(),
        0.317f32.to_bits()
    );
    let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
    assert_eq!(
        reopened.amp_drive_fraction().unwrap().to_bits(),
        0.317f32.to_bits()
    );
    assert!(tone.as_bytes().windows(12).any(|window| window == first));
    assert!(tone.as_bytes().windows(12).any(|window| window == unknown));
}

#[test]
fn amp_drive_finite_endpoints_nonintegers_and_signed_zero_keep_exact_float_bits() {
    let record = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0);
    let values = [0.0, 1.0, 0.314_159_27, f32::from_bits(1), -0.0];
    for value in values {
        let mut tone = PodTone::parse(drive_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PLEXI_LEAD_100_MODEL,
            &[&record],
        ))
        .unwrap();
        tone.set_amp_drive_fraction(value).unwrap();
        assert_eq!(
            tone.amp_drive_fraction().unwrap().to_bits(),
            value.to_bits()
        );
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == value.to_bits().to_be_bytes())
        );
    }
}

#[test]
fn source_mapped_amp_controls_have_distinct_ids_and_preserve_every_other_byte() {
    assert_eq!(AMP_CONTROL_MODEL_IDS.len(), 106);
    assert!(!AMP_CONTROL_MODEL_IDS.contains(&0x0002_0000)); // No Amp
    let mut unique = AMP_CONTROL_MODEL_IDS.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), AMP_CONTROL_MODEL_IDS.len());

    for model_id in AMP_CONTROL_MODEL_IDS {
        let records: Vec<_> = AmpControl::ALL
            .into_iter()
            .enumerate()
            .map(|(index, control)| {
                drive_record(
                    0x3f,
                    control.param_id(),
                    1,
                    (0.1f32 + index as f32 * 0.1).to_bits(),
                )
            })
            .collect();
        let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
        let original = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &refs);
        for control in AmpControl::ALL {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let previous = tone.amp_control_fraction(control).unwrap();
            let new_value = (previous + 0.01).min(1.0);
            tone.set_amp_control_fraction(control, new_value).unwrap();
            assert_eq!(
                tone.amp_control_fraction(control).unwrap().to_bits(),
                new_value.to_bits(),
                "{} on amp model {model_id:08x}",
                control.label()
            );
            let (model_index, parameter_index) = tone.amp_control_location(control).unwrap();
            let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
            let expected = [
                original[..offset].to_vec(),
                new_value.to_bits().to_be_bytes().to_vec(),
                original[offset + 4..].to_vec(),
            ]
            .concat();
            assert_eq!(
                tone.as_bytes(),
                expected,
                "{} on amp model {model_id:08x}",
                control.label()
            );
        }
    }
}

#[test]
fn source_mapped_stomp_controls_patch_only_their_existing_value_words() {
    assert_eq!(STOMP_SLOT, 0x0002_0003);
    assert_eq!(STOMP_DRIVE_MODEL_ID, 0x0205_0000);
    assert_eq!(STOMP_CONTROL_MODEL_IDS.len(), 31);
    let unknown = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
    let profiles = [
        (
            0x0200_0002,
            [
                Some(StompControl::Sustain),
                Some(StompControl::Level),
                None,
                None,
                None,
            ],
        ),
        (
            0x0200_0003,
            [
                Some(StompControl::Level),
                Some(StompControl::Sustain),
                None,
                None,
                None,
            ],
        ),
        (
            0x0200_0004,
            [
                Some(StompControl::Sustain),
                Some(StompControl::Level),
                None,
                None,
                None,
            ],
        ),
        (
            0x0200_0005,
            [
                Some(StompControl::Sensitivity),
                Some(StompControl::Level),
                None,
                None,
                None,
            ],
        ),
        (
            0x0200_0006,
            [
                Some(StompControl::Amount),
                Some(StompControl::Level),
                None,
                None,
                None,
            ],
        ),
        (
            0x0200_0007,
            [
                Some(StompControl::Ramp),
                Some(StompControl::Depth),
                None,
                None,
                None,
            ],
        ),
        (
            0x020a_0000,
            [
                Some(StompControl::Sensitivity),
                Some(StompControl::Q),
                None,
                None,
                None,
            ],
        ),
        (
            0x020a_0001,
            [
                None,
                Some(StompControl::Filter),
                Some(StompControl::Decay),
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_0002,
            [
                None,
                Some(StompControl::Filter),
                Some(StompControl::Attack),
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_0003,
            [
                None,
                Some(StompControl::Filter),
                Some(StompControl::Decay),
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_0004,
            [
                None,
                Some(StompControl::Filter),
                Some(StompControl::Decay),
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_0005,
            [
                None,
                Some(StompControl::Filter),
                Some(StompControl::Decay),
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_0006,
            [
                None,
                Some(StompControl::Filter),
                Some(StompControl::Decay),
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_0007,
            [None, None, None, Some(StompControl::Mix), None],
        ),
        (
            0x020a_0008,
            [
                Some(StompControl::S1Ogt),
                Some(StompControl::S2Ogt),
                None,
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_0009,
            [
                None,
                None,
                Some(StompControl::Wave),
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_000a,
            [
                None,
                Some(StompControl::Sensitivity),
                Some(StompControl::Q),
                None,
                None,
            ],
        ),
        (
            0x020a_000b,
            [
                Some(StompControl::Decay),
                Some(StompControl::Sensitivity),
                Some(StompControl::Q),
                None,
                None,
            ],
        ),
        (
            0x020a_000c,
            [None, None, None, Some(StompControl::Mix), None],
        ),
        (
            0x0205_0000,
            [
                Some(StompControl::Drive),
                Some(StompControl::Gain),
                Some(StompControl::Tone),
                None,
                None,
            ],
        ),
        (
            0x0205_0002,
            [
                Some(StompControl::Drive),
                Some(StompControl::Gain),
                Some(StompControl::Tone),
                None,
                None,
            ],
        ),
        (
            0x0205_0003,
            [
                Some(StompControl::Drive),
                Some(StompControl::Gain),
                Some(StompControl::Tone),
                None,
                None,
            ],
        ),
        (
            0x0205_0004,
            [
                Some(StompControl::Drive),
                Some(StompControl::Gain),
                Some(StompControl::Tone),
                None,
                None,
            ],
        ),
        (
            0x0205_0005,
            [
                Some(StompControl::Drive),
                Some(StompControl::Gain),
                Some(StompControl::Tone),
                None,
                None,
            ],
        ),
        (
            0x0205_0001,
            [
                Some(StompControl::Drive),
                Some(StompControl::Treble),
                Some(StompControl::Gain),
                Some(StompControl::Bass),
                None,
            ],
        ),
        (
            0x0205_0006,
            [
                Some(StompControl::Drive),
                Some(StompControl::Contour),
                Some(StompControl::Gain),
                Some(StompControl::Mid),
                Some(StompControl::MidFrequency),
            ],
        ),
        (
            0x0205_0007,
            [
                Some(StompControl::Gain),
                Some(StompControl::Bass),
                Some(StompControl::Treble),
                Some(StompControl::Mid),
                Some(StompControl::MidFrequency),
            ],
        ),
        (
            0x0205_0008,
            [
                Some(StompControl::Bass),
                Some(StompControl::Treble),
                Some(StompControl::Drive),
                Some(StompControl::Gain),
                None,
            ],
        ),
        (
            0x0205_0009,
            [
                Some(StompControl::Drive),
                Some(StompControl::Tone),
                None,
                Some(StompControl::Blend),
                None,
            ],
        ),
        (
            0x020a_000d,
            [
                Some(StompControl::S1Ogt),
                Some(StompControl::S2Ogt),
                None,
                Some(StompControl::Mix),
                None,
            ],
        ),
        (
            0x020a_000e,
            [
                Some(StompControl::Position),
                None,
                None,
                Some(StompControl::Mix),
                None,
            ],
        ),
    ];
    assert_eq!(profiles.len(), STOMP_CONTROL_MODEL_IDS.len());
    assert_eq!(
        profiles
            .iter()
            .map(|(_, expected_layout)| expected_layout.iter().flatten().count())
            .sum::<usize>(),
        85
    );
    let mut profile_ids: Vec<_> = profiles.iter().map(|(model_id, _)| *model_id).collect();
    profile_ids.sort_unstable();
    profile_ids.dedup();
    assert_eq!(profile_ids.len(), profiles.len());
    for (model_id, expected_layout) in profiles {
        assert!(STOMP_CONTROL_MODEL_IDS.contains(&model_id));
        for control in StompControl::ALL {
            assert_eq!(
                control.selector_id(model_id).is_some(),
                expected_layout.contains(&Some(control)),
                "unexpected {} mapping on model {model_id:08x}",
                control.label()
            );
        }
        for (position, expected_control) in expected_layout.into_iter().enumerate() {
            let Some(control) = expected_control else {
                continue;
            };
            let position = position as u32 + 1;
            assert_eq!(
                control.selector_id(model_id),
                Some(0x0003_0000 + position),
                "{} on model {model_id:08x}",
                control.label()
            );
            let parameter_id = 0x0010_0000 + position;
            assert_eq!(control.param_id(model_id), Some(parameter_id));
            let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
            let earlier_duplicate = drive_record(0x3f, parameter_id, 1, 0x3e00_0000);
            let bytes = stomp_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                model_id,
                &[&unknown, &earlier_duplicate, &record],
            );
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert_eq!(
                tone.stomp_control_fraction(control).unwrap().to_bits(),
                0x3f00_0000
            );
            tone.set_stomp_control_fraction(control, 0.317).unwrap();
            let value_offset = bytes
                .windows(record.len())
                .position(|window| window == record)
                .unwrap()
                + 8;
            let earlier_value_offset = bytes
                .windows(earlier_duplicate.len())
                .position(|window| window == earlier_duplicate)
                .unwrap()
                + 8;
            let expected = [
                bytes[..value_offset].to_vec(),
                0.317f32.to_bits().to_be_bytes().to_vec(),
                bytes[value_offset + 4..].to_vec(),
            ]
            .concat();
            assert_eq!(tone.as_bytes(), expected, "{}", control.label());
            assert_eq!(
                &tone.as_bytes()[earlier_value_offset..earlier_value_offset + 4],
                &bytes[earlier_value_offset..earlier_value_offset + 4],
                "earlier duplicate {}",
                control.label()
            );
            assert_eq!(
                tone.stomp_control_fraction(control).unwrap().to_bits(),
                0.317f32.to_bits(),
                "{} on model {model_id:08x}",
                control.label()
            );
        }
    }
}

#[test]
fn source_mapped_mod_percent_controls_patch_only_their_existing_value_words() {
    assert_eq!(MOD_SLOT, 0x0002_0004);
    assert_eq!(MOD_CONTROL_MODEL_IDS.len(), 24);
    let profiles: [(u32, [Option<ModControl>; 3]); 24] = [
        (
            0x0203_0004,
            [
                Some(ModControl::Depth),
                Some(ModControl::Bass),
                Some(ModControl::Treble),
            ],
        ),
        (
            0x0203_0005,
            [
                Some(ModControl::Depth),
                Some(ModControl::Bass),
                Some(ModControl::Treble),
            ],
        ),
        (0x0203_0006, [Some(ModControl::Depth), None, None]),
        (
            0x0203_0007,
            [
                Some(ModControl::Depth),
                Some(ModControl::Feedback),
                Some(ModControl::Manual),
            ],
        ),
        (0x0203_0008, [Some(ModControl::Feedback), None, None]),
        (0x0203_0009, [Some(ModControl::Depth), None, None]),
        (0x0203_000a, [Some(ModControl::Wave), None, None]),
        (0x0203_000b, [Some(ModControl::Wave), None, None]),
        (0x0203_000c, [None, Some(ModControl::Tone), None]),
        (0x0203_000d, [None, Some(ModControl::Tone), None]),
        (0x0203_000e, [Some(ModControl::Wave), None, None]),
        (
            0x0203_0015,
            [
                Some(ModControl::Depth),
                Some(ModControl::Bass),
                Some(ModControl::Treble),
            ],
        ),
        (
            0x0203_0012,
            [
                Some(ModControl::Depth),
                Some(ModControl::Predelay),
                Some(ModControl::Feedback),
            ],
        ),
        (
            0x0203_001e,
            [
                Some(ModControl::Depth),
                Some(ModControl::Predelay),
                Some(ModControl::Feedback),
            ],
        ),
        (
            0x0203_0014,
            [
                Some(ModControl::Depth),
                Some(ModControl::Bass),
                Some(ModControl::Treble),
            ],
        ),
        (
            0x0203_001f,
            [
                Some(ModControl::Depth),
                Some(ModControl::Predelay),
                Some(ModControl::Feedback),
            ],
        ),
        (
            0x0203_0013,
            [
                Some(ModControl::Depth),
                Some(ModControl::Predelay),
                Some(ModControl::Feedback),
            ],
        ),
        (
            0x0203_0011,
            [
                Some(ModControl::Depth),
                Some(ModControl::Bass),
                Some(ModControl::Treble),
            ],
        ),
        (
            0x0203_001a,
            [Some(ModControl::Depth), Some(ModControl::Q), None],
        ),
        (
            0x0203_001b,
            [
                Some(ModControl::Depth),
                Some(ModControl::Q),
                Some(ModControl::Frequency),
            ],
        ),
        (
            0x0203_0017,
            [Some(ModControl::Feedback), Some(ModControl::Depth), None],
        ),
        (
            0x0203_0018,
            [Some(ModControl::Depth), Some(ModControl::Q), None],
        ),
        (
            0x0203_0019,
            [
                Some(ModControl::Feedback),
                Some(ModControl::Flutter),
                Some(ModControl::Distortion),
            ],
        ),
        (
            0x0203_001c,
            [Some(ModControl::Depth), Some(ModControl::Q), None],
        ),
    ];
    assert_eq!(
        profiles
            .iter()
            .map(|(_, layout)| layout.iter().flatten().count())
            .sum::<usize>(),
        52
    );
    assert_eq!(
        profiles
            .iter()
            .map(|(model_id, _)| *model_id)
            .collect::<Vec<_>>(),
        MOD_CONTROL_MODEL_IDS
    );
    for (model_id, layout) in profiles {
        assert!(MOD_CONTROL_MODEL_IDS.contains(&model_id));
        for control in ModControl::ALL {
            assert_eq!(
                control.selector_id(model_id).is_some(),
                layout.contains(&Some(control)),
                "unexpected {} mapping on model {model_id:08x}",
                control.label()
            );
        }
        let mut records = Vec::new();
        for (index, expected) in layout.into_iter().enumerate() {
            if expected.is_some() {
                let parameter_id = 0x0010_0000 + index as u32 + 1;
                records.push(drive_record(0x3f, parameter_id, 1, 0x3f00_0000));
            }
        }
        let record_refs: Vec<&[u8]> = records.iter().map(|r| r.as_slice()).collect();
        let bytes = mod_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &record_refs);
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        for control in ModControl::ALL {
            let Some(position) = control.selector_id(model_id).map(|id| id - 0x0004_0002) else {
                continue;
            };
            assert_eq!(control.selector_id(model_id), Some(0x0004_0002 + position));
            let parameter_id = 0x0010_0000 + position;
            assert_eq!(control.param_id(model_id), Some(parameter_id));
            assert_eq!(tone.mod_control_fraction(control).unwrap(), 0.5);
            tone.set_mod_control_fraction(control, 0.317).unwrap();
            let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
            let offset = bytes.windows(12).position(|w| w == record).unwrap() + 8;
            assert_eq!(
                &tone.as_bytes()[offset..offset + 4],
                &0.317f32.to_bits().to_be_bytes()
            );
        }
        assert_eq!(tone.models()[0].model_id, model_id);
    }
}

#[test]
fn source_mapped_reverb_percent_controls_patch_only_their_existing_value_words() {
    assert_eq!(REVERB_SLOT, 0x0002_0006);
    let profiles: [(u32, [Option<ReverbControl>; 3]); 15] = [
        (
            0x0204_0002,
            [None, Some(ReverbControl::Dwell), Some(ReverbControl::Tone)],
        ),
        (
            0x0204_0003,
            [None, Some(ReverbControl::Dwell), Some(ReverbControl::Tone)],
        ),
        (
            0x0204_0004,
            [None, Some(ReverbControl::Dwell), Some(ReverbControl::Tone)],
        ),
        (
            0x0204_0005,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_0006,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_0007,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_0008,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_0009,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_000a,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_000b,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_000c,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_000d,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_000e,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_000f,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
        (
            0x0204_0010,
            [
                Some(ReverbControl::Predelay),
                Some(ReverbControl::Decay),
                Some(ReverbControl::Tone),
            ],
        ),
    ];
    assert_eq!(
        profiles.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        REVERB_CONTROL_MODEL_IDS
    );
    for (model_id, layout) in profiles {
        for control in ReverbControl::ALL {
            assert_eq!(
                control.selector_id(model_id).is_some(),
                layout.contains(&Some(control)),
                "unexpected {} mapping on model {model_id:08x}",
                control.label()
            );
        }
        let mut records = Vec::new();
        for control in layout.into_iter().flatten() {
            let parameter_id = control.param_id(model_id).unwrap();
            records.push(drive_record(0x3f, parameter_id, 1, 0x3f00_0000));
        }
        let record_refs: Vec<&[u8]> = records.iter().map(|r| r.as_slice()).collect();
        let bytes = reverb_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &record_refs);
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        for control in ReverbControl::ALL {
            if control.selector_id(model_id).is_none() {
                continue;
            }
            let parameter_id = control.param_id(model_id).unwrap();
            assert_eq!(tone.reverb_control_fraction(control).unwrap(), 0.5);
            tone.set_reverb_control_fraction(control, 0.317).unwrap();
            let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
            let offset = bytes.windows(12).position(|w| w == record).unwrap() + 8;
            assert_eq!(
                &tone.as_bytes()[offset..offset + 4],
                &0.317f32.to_bits().to_be_bytes()
            );
        }
    }
}

#[test]
fn stomp_harmony_choices_match_source_bits_and_patch_only_selected_existing_word() {
    let expected_bits = [
        0xbb81_0204,
        0x3df9_f3e8,
        0x3e7d_fbf8,
        0x3ebf_7efe,
        0x3f00_0000,
        0x3f20_4081,
        0x3f40_8102,
        0x3f60_c183,
        0x3f7e_fdfc,
    ];
    let expected_keys = [0, 16, 32, 48, 64, 80, 96, 112, 127];
    assert_eq!(StompHarmonyControl::Harmony1M335.selector_id(), 0x0003_0001);
    assert_eq!(
        StompHarmonyControl::Harmony1M335.parameter_id(),
        0x0010_0001
    );
    assert_eq!(StompHarmonyControl::Harmony1457.selector_id(), 0x0003_0002);
    assert_eq!(StompHarmonyControl::Harmony1457.parameter_id(), 0x0010_0002);
    assert_eq!(StompHarmony1M335Selection::ALL.len(), 9);
    assert_eq!(StompHarmony1457Selection::ALL.len(), 9);
    let expected_labels1 = [
        "-1 OCT", "-MAJ6TH", "-MIN6TH", "-4TH", "UNISON", "MIN3RD", "MAJ3RD", "5TH", "1 OCT",
    ];
    let expected_labels2 = [
        "-1 OCT", "-5TH", "-4TH", "-2ND", "UNISON", "4TH", "5TH", "7TH", "1 OCT",
    ];
    for (((choice, bits), key), label) in StompHarmony1M335Selection::ALL
        .into_iter()
        .zip(expected_bits)
        .zip(expected_keys)
        .zip(expected_labels1)
    {
        assert_eq!(choice.float_bits(), bits);
        assert_eq!(choice.controller_key(), key);
        assert_eq!(choice.label(), label);
    }
    for (((choice, bits), key), label) in StompHarmony1457Selection::ALL
        .into_iter()
        .zip(expected_bits)
        .zip(expected_keys)
        .zip(expected_labels2)
    {
        assert_eq!(choice.float_bits(), bits);
        assert_eq!(choice.controller_key(), key);
        assert_eq!(choice.label(), label);
    }
    let original = stomp_harmony_tone(0x3f00_0000, 0x3f40_8102);
    let value_start1 = original
        .windows(4)
        .position(|w| w == ((0x3f10_0001u32).to_be_bytes()))
        .unwrap()
        + 8;
    let value_start2 = original
        .windows(4)
        .position(|w| w == ((0x3f10_0002u32).to_be_bytes()))
        .unwrap()
        + 8;
    for (choice, bits) in StompHarmony1M335Selection::ALL
        .into_iter()
        .zip(expected_bits)
    {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_stomp_harmony_1m335_selection(choice).unwrap();
        let mut expected_bytes = original.clone();
        expected_bytes[value_start1..value_start1 + 4].copy_from_slice(&bits.to_be_bytes());
        assert_eq!(tone.as_bytes(), expected_bytes);
        assert_eq!(
            tone.stomp_harmony_1m335_selection().unwrap(),
            StompHarmonyValue::Known(choice)
        );
        assert_eq!(
            tone.stomp_harmony_1457_selection().unwrap(),
            StompHarmonyValue::Known(StompHarmony1457Selection::Fifth)
        );
    }
    for (choice, bits) in StompHarmony1457Selection::ALL
        .into_iter()
        .zip(expected_bits)
    {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_stomp_harmony_1457_selection(choice).unwrap();
        let mut expected_bytes = original.clone();
        expected_bytes[value_start2..value_start2 + 4].copy_from_slice(&bits.to_be_bytes());
        assert_eq!(tone.as_bytes(), expected_bytes);
        assert_eq!(
            tone.stomp_harmony_1457_selection().unwrap(),
            StompHarmonyValue::Known(choice)
        );
        assert_eq!(
            tone.stomp_harmony_1m335_selection().unwrap(),
            StompHarmonyValue::Known(StompHarmony1M335Selection::Unison)
        );
    }
}

#[test]
fn stomp_harmony_unknown_is_preserved_and_invalid_current_values_reject_atomically() {
    let mut tone = PodTone::parse(stomp_harmony_tone(0x3f00_0001, 0x7fc0_1234)).unwrap();
    let before = tone.as_bytes().to_vec();
    assert_eq!(
        tone.stomp_harmony_1m335_selection().unwrap(),
        StompHarmonyValue::Unknown {
            raw_bits: 0x3f00_0001
        }
    );
    assert!(
        tone.set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
            .is_err()
    );
    assert_eq!(tone.as_bytes(), before);
    let mut unknown = PodTone::parse(stomp_harmony_tone(0x3f00_0001, 0x3f00_0000)).unwrap();
    unknown
        .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::MinusOctave)
        .unwrap();
    assert_eq!(
        unknown.stomp_harmony_1m335_selection().unwrap(),
        StompHarmonyValue::Known(StompHarmony1M335Selection::MinusOctave)
    );
    let mut below_profile = stomp_harmony_tone(0x3f00_0000, 0x3f40_8102);
    let pinf = find_chunk_data(&below_profile, b"PINF");
    below_profile[pinf + 72..pinf + 76]
        .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    let mut below_profile = PodTone::parse(below_profile).unwrap();
    let before = below_profile.as_bytes().to_vec();
    assert!(
        below_profile
            .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
            .is_err(),
        "same-choice requests must still validate the host profile"
    );
    assert_eq!(below_profile.as_bytes(), before);
}

#[test]
fn stomp_harmony_rejects_ambiguous_or_malformed_targets_without_mutation() {
    let controls = [
        StompHarmonyControl::Harmony1M335,
        StompHarmonyControl::Harmony1457,
    ];
    for control in controls {
        let target = control.parameter_id();
        let sibling = match control {
            StompHarmonyControl::Harmony1M335 => 0x0010_0002,
            StompHarmonyControl::Harmony1457 => 0x0010_0001,
        };
        let canonical = drive_record(0x3f, target, 1, 0x3f00_0000);
        let sibling_record = drive_record(0x3f, sibling, 1, 0x3f40_8102);
        let equal_duplicate = canonical;
        let different_duplicate = drive_record(0x3f, target, 1, 0x3f20_4081);
        let bad_prefix_duplicate = drive_record(0x3e, target, 1, 0x3f20_4081);
        let wrong_type = drive_record(0x3f, target, 0, 0x3f00_0000);
        let opaque_target = ((0x3fu32 << 24) | target).to_be_bytes();
        let mut invalid = Vec::new();
        for (first, second) in [
            (canonical.as_slice(), equal_duplicate.as_slice()),
            (canonical.as_slice(), different_duplicate.as_slice()),
            (bad_prefix_duplicate.as_slice(), canonical.as_slice()),
        ] {
            invalid.push(stomp_harmony_tone_with_models(
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[first, second, sibling_record.as_slice()],
                }],
                PODXT_LIVE_TONE_ID,
                PODXT_LIVE_HOST_MIN_VERSION,
            ));
        }
        let duplicate_slots = stomp_harmony_tone_with_models(
            &[
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                },
                BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x020a_0009,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &[canonical.as_slice(), sibling_record.as_slice()],
                },
            ],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        );
        assert!(
            PodTone::parse(duplicate_slots.clone()).is_err(),
            "duplicate Stomp slots must be rejected while original bytes stay available"
        );
        assert!(!duplicate_slots.is_empty());
        invalid.push(stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0008,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[canonical.as_slice(), sibling_record.as_slice()],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ));
        invalid.push(stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT + 1,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[canonical.as_slice(), sibling_record.as_slice()],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ));
        invalid.push(stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[sibling_record.as_slice()],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ));
        invalid.push(stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[wrong_type.as_slice(), sibling_record.as_slice()],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ));
        invalid.push(stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[bad_prefix_duplicate.as_slice(), sibling_record.as_slice()],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ));
        invalid.push(stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[
                    opaque_target.as_slice(),
                    canonical.as_slice(),
                    sibling_record.as_slice(),
                ],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ));
        let wrong_target = stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[canonical.as_slice(), sibling_record.as_slice()],
            }],
            0x1234_5678,
            PODXT_LIVE_HOST_MIN_VERSION,
        );
        assert!(PodTone::parse(wrong_target.clone()).is_err());
        assert!(!wrong_target.is_empty());
        invalid.push(stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[canonical.as_slice(), sibling_record.as_slice()],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION - 1,
        ));

        for (case, bytes) in invalid.into_iter().enumerate() {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            let getter_rejected = match control {
                StompHarmonyControl::Harmony1M335 => tone.stomp_harmony_1m335_selection().is_err(),
                StompHarmonyControl::Harmony1457 => tone.stomp_harmony_1457_selection().is_err(),
            };
            assert!(
                getter_rejected,
                "getter admitted invalid case {case} for {control:?}"
            );
            let setter_rejected = match control {
                StompHarmonyControl::Harmony1M335 => tone
                    .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
                    .is_err(),
                StompHarmonyControl::Harmony1457 => tone
                    .set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
                    .is_err(),
            };
            assert!(
                setter_rejected,
                "setter admitted invalid case {case} for {control:?}"
            );
            assert_eq!(tone.as_bytes(), before, "case {case} mutated tone");
        }

        let nonfinite_target = drive_record(0x3f, target, 1, 0x7fc0_1234);
        let nonfinite = stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[nonfinite_target.as_slice(), sibling_record.as_slice()],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        );
        let mut tone = PodTone::parse(nonfinite).unwrap();
        let before = tone.as_bytes().to_vec();
        let setter_rejected = match control {
            StompHarmonyControl::Harmony1M335 => {
                assert_eq!(
                    tone.stomp_harmony_1m335_selection().unwrap(),
                    StompHarmonyValue::Unknown {
                        raw_bits: 0x7fc0_1234
                    }
                );
                tone.set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
                    .is_err()
            }
            StompHarmonyControl::Harmony1457 => {
                assert_eq!(
                    tone.stomp_harmony_1457_selection().unwrap(),
                    StompHarmonyValue::Unknown {
                        raw_bits: 0x7fc0_1234
                    }
                );
                tone.set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
                    .is_err()
            }
        };
        assert!(setter_rejected);
        assert_eq!(tone.as_bytes(), before);

        // A duplicate that happens to contain the requested choice still fails admission.
        let same_choice = stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[
                    canonical.as_slice(),
                    canonical.as_slice(),
                    sibling_record.as_slice(),
                ],
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        );
        let mut tone = PodTone::parse(same_choice).unwrap();
        let before = tone.as_bytes().to_vec();
        let rejected = match control {
            StompHarmonyControl::Harmony1M335 => tone
                .set_stomp_harmony_1m335_selection(StompHarmony1M335Selection::Unison)
                .is_err(),
            StompHarmonyControl::Harmony1457 => tone
                .set_stomp_harmony_1457_selection(StompHarmony1457Selection::Unison)
                .is_err(),
        };
        assert!(rejected, "same-choice duplicate admitted for {control:?}");
        assert_eq!(tone.as_bytes(), before);
    }
}

#[test]
fn stomp_harmony_library_and_bundle_paths_round_trip_offline_edits() {
    let original = stomp_harmony_tone(0x3f00_0000, 0x3f40_8102);
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    library
        .set_stomp_harmony_1m335_selection(index, StompHarmony1M335Selection::MinusOctave)
        .unwrap();
    library
        .set_stomp_harmony_1457_selection(index, StompHarmony1457Selection::Fourth)
        .unwrap();
    assert!(library.is_dirty(index));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("harmony-copy.l6t");
    library.tone(index).unwrap().save_copy(&path).unwrap();
    let reopened = PodTone::parse(std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        reopened.stomp_harmony_1m335_selection().unwrap(),
        StompHarmonyValue::Known(StompHarmony1M335Selection::MinusOctave)
    );
    assert_eq!(
        reopened.stomp_harmony_1457_selection().unwrap(),
        StompHarmonyValue::Known(StompHarmony1457Selection::Fourth)
    );

    let bundle_bytes = bundle_with_tone(&original);
    let mut bundle = PodBundleLibrary::parse(bundle_bytes).unwrap();
    bundle
        .set_stomp_harmony_1m335_selection(0, StompHarmony1M335Selection::Fifth)
        .unwrap();
    bundle
        .set_stomp_harmony_1457_selection(0, StompHarmony1457Selection::Seventh)
        .unwrap();
    assert!(bundle.is_dirty());
    let reopened = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
    let tone = reopened.tone(0, 0).unwrap();
    assert_eq!(
        tone.stomp_harmony_1m335_selection().unwrap(),
        StompHarmonyValue::Known(StompHarmony1M335Selection::Fifth)
    );
    assert_eq!(
        tone.stomp_harmony_1457_selection().unwrap(),
        StompHarmonyValue::Known(StompHarmony1457Selection::Seventh)
    );

    for (target, selector1) in [(0x0010_0001u32, true), (0x0010_0002, false)] {
        let duplicate = drive_record(0x3f, target, 1, 0x3f00_0000);
        let first = drive_record(0x3f, 0x0010_0001, 1, 0x3f00_0000);
        let second = drive_record(0x3f, 0x0010_0002, 1, 0x3f40_8102);
        let parameters = if selector1 {
            [first.as_slice(), duplicate.as_slice(), second.as_slice()]
        } else {
            [first.as_slice(), second.as_slice(), duplicate.as_slice()]
        };
        let duplicate_bytes = stomp_harmony_tone_with_models(
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x020a_0009,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &parameters,
            }],
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        );
        let mut library = Library::default();
        let index = library.import_bytes(duplicate_bytes.clone()).unwrap();
        let setter_rejected = if selector1 {
            library
                .set_stomp_harmony_1m335_selection(index, StompHarmony1M335Selection::Unison)
                .is_err()
        } else {
            library
                .set_stomp_harmony_1457_selection(index, StompHarmony1457Selection::Unison)
                .is_err()
        };
        assert!(setter_rejected);
        assert!(!library.is_dirty(index));
        assert_eq!(library.tone(index).unwrap().as_bytes(), duplicate_bytes);

        let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&duplicate_bytes)).unwrap();
        let setter_rejected = if selector1 {
            bundle
                .set_stomp_harmony_1m335_selection(0, StompHarmony1M335Selection::Unison)
                .is_err()
        } else {
            bundle
                .set_stomp_harmony_1457_selection(0, StompHarmony1457Selection::Unison)
                .is_err()
        };
        assert!(setter_rejected);
        assert!(!bundle.is_dirty());
        assert_eq!(bundle.tone(0, 0).unwrap().as_bytes(), duplicate_bytes);
    }
}

#[test]
fn stomp_wave_choices_match_exact_source_bits_and_all_eight_layouts() {
    let expected = [
        0xbb81_0204,
        0x3df9_f3e8,
        0x3e7d_fbf8,
        0x3ebf_7efe,
        0x3f00_0000,
        0x3f20_4081,
        0x3f40_8102,
        0x3f60_c183,
    ];
    assert_eq!(STOMP_SYNTH_WAVE_MODEL_IDS.len(), 8);
    assert_eq!(STOMP_SYNTH_WAVE_SELECTOR, 0x0003_0001);
    assert_eq!(STOMP_SYNTH_WAVE_PARAM, 0x0010_0001);
    assert_eq!(StompWaveSelection::ALL.len(), expected.len());
    for (choice, expected_bits) in StompWaveSelection::ALL.into_iter().zip(expected) {
        assert_eq!(choice.float_bits(), expected_bits);
        assert_eq!(
            StompWaveSelection::from_float_bits(expected_bits),
            Some(choice)
        );
    }
    assert_eq!(f32::from_bits(expected[0]), -0.003937008);
    assert!(!f32::from_bits(expected[0]).is_sign_positive());

    for model_id in STOMP_SYNTH_WAVE_MODEL_IDS {
        let earlier = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3e80_0000);
        let unrelated = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
        let selected = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
        let records: [&[u8]; 3] = [&earlier, &unrelated, &selected];
        let original = stomp_wave_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &records);
        let selected_offset = original
            .windows(selected.len())
            .rposition(|window| window == selected)
            .unwrap()
            + 8;
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(
            tone.stomp_wave_selection().unwrap(),
            StompWaveValue::Known(StompWaveSelection::Wave5)
        );
        for (choice, expected_bits) in StompWaveSelection::ALL.into_iter().zip(expected) {
            let mut expected_bytes = original.clone();
            expected_bytes[selected_offset..selected_offset + 4]
                .copy_from_slice(&expected_bits.to_be_bytes());
            tone.set_stomp_wave_selection(choice).unwrap();
            assert_eq!(
                tone.as_bytes(),
                expected_bytes,
                "model 0x{model_id:08x} {choice:?}"
            );
            assert_eq!(
                tone.stomp_wave_selection().unwrap(),
                StompWaveValue::Known(choice)
            );
            assert!(tone.as_bytes().windows(earlier.len()).any(|w| w == earlier));
            assert!(
                tone.as_bytes()
                    .windows(unrelated.len())
                    .any(|w| w == unrelated)
            );
        }
    }

    let ninth_candidate = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
    let nine_tone = stomp_wave_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        0x020a_0009,
        &[&ninth_candidate],
    );
    let mut nine_tone = PodTone::parse(nine_tone).unwrap();
    let before = nine_tone.as_bytes().to_vec();
    assert!(nine_tone.stomp_wave_selection().is_err());
    assert!(
        nine_tone
            .set_stomp_wave_selection(StompWaveSelection::Wave2)
            .is_err()
    );
    assert_eq!(nine_tone.as_bytes(), before);
}

#[test]
fn stomp_wave_unknown_words_are_lossless_and_nonfinite_edits_are_atomic() {
    for bits in [
        0x3eaa_aaab,
        0x7fc0_1234,
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
    ] {
        let record = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, bits);
        let original = stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_SYNTH_WAVE_MODEL_IDS[0],
            &[&record],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(
            tone.stomp_wave_selection().unwrap(),
            StompWaveValue::Unknown { raw_bits: bits }
        );
        assert_eq!(tone.as_bytes(), original);
        if f32::from_bits(bits).is_finite() {
            tone.set_stomp_wave_selection(StompWaveSelection::Wave8)
                .unwrap();
            assert_eq!(
                tone.stomp_wave_selection().unwrap(),
                StompWaveValue::Known(StompWaveSelection::Wave8)
            );
        } else {
            assert!(
                tone.set_stomp_wave_selection(StompWaveSelection::Wave8)
                    .is_err()
            );
            assert_eq!(tone.as_bytes(), original);
            assert_eq!(
                tone.stomp_wave_selection().unwrap(),
                StompWaveValue::Unknown { raw_bits: bits }
            );

            let mut library = Library::default();
            let index = library.import_bytes(original.clone()).unwrap();
            assert!(
                library
                    .set_stomp_wave_selection(index, StompWaveSelection::Wave8)
                    .is_err()
            );
            assert!(!library.is_dirty(index));
            assert_eq!(library.tone(index).unwrap().as_bytes(), original);

            let bundle_bytes = bundle_with_tone(&original);
            let mut bundle = PodBundleLibrary::parse(bundle_bytes.clone()).unwrap();
            assert!(
                bundle
                    .set_stomp_wave_selection(0, StompWaveSelection::Wave8)
                    .is_err()
            );
            assert!(!bundle.is_dirty());
            assert_eq!(bundle.as_bytes(), bundle_bytes);
        }
    }
}

#[test]
fn stomp_wave_admission_rejects_wrong_target_model_layout_and_records() {
    let valid = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
    let bad_prefix = drive_record(0x7e, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
    let bad_type = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 0, 0x3f00_0000);
    let opaque = [1, 2, 3, 4];
    let cases = [
        stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            STOMP_SYNTH_WAVE_MODEL_IDS[0],
            &[&valid],
        ),
        stomp_wave_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x020a_0009, &[&valid]),
        stomp_wave_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x020a_0000, &[&valid]),
        stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_SYNTH_WAVE_MODEL_IDS[0],
            &[],
        ),
        stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_SYNTH_WAVE_MODEL_IDS[0],
            &[&opaque],
        ),
        stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_SYNTH_WAVE_MODEL_IDS[0],
            &[&bad_prefix],
        ),
        stomp_wave_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_SYNTH_WAVE_MODEL_IDS[0],
            &[&bad_type],
        ),
        custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: PLEXI_LEAD_100_MODEL,
                tail: &[],
                parameters: &[&valid],
            }],
        ),
        custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: STOMP_SYNTH_WAVE_MODEL_IDS[0],
                tail: &[],
                parameters: &[&valid],
            }],
        ),
    ];
    for bytes in cases {
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.stomp_wave_selection().is_err());
        assert!(
            tone.set_stomp_wave_selection(StompWaveSelection::Wave4)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), bytes);
    }

    let wrong_target = fixture(
        ContainerKind::L6pa,
        0x0003_000b,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(STOMP_SLOT, STOMP_SYNTH_WAVE_MODEL_IDS[0])],
    );
    assert!(matches!(
        PodTone::parse(wrong_target),
        Err(ToneError::WrongTarget { .. })
    ));

    let duplicate_slot = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: STOMP_SLOT,
                model_id: STOMP_SYNTH_WAVE_MODEL_IDS[0],
                tail: &[],
                parameters: &[&valid],
            },
            BuiltModel {
                slot: STOMP_SLOT,
                model_id: STOMP_SYNTH_WAVE_MODEL_IDS[1],
                tail: &[],
                parameters: &[&valid],
            },
        ],
    );
    assert!(PodTone::parse(duplicate_slot).is_err());
}

#[test]
fn stomp_wave_library_bundle_save_and_reopen_preserve_unrelated_bytes() {
    let record = drive_record(0x3f, STOMP_SYNTH_WAVE_PARAM, 1, 0x3f00_0000);
    let original = stomp_wave_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        STOMP_SYNTH_WAVE_MODEL_IDS[3],
        &[&record],
    );
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    assert_eq!(
        library.stomp_wave_selection(index).unwrap(),
        StompWaveValue::Known(StompWaveSelection::Wave5)
    );
    library
        .set_stomp_wave_selection(index, StompWaveSelection::Wave2)
        .unwrap();
    assert!(library.is_dirty(index));
    let edited = library.tone(index).unwrap().as_bytes().to_vec();
    let changed = edited.iter().zip(&original).filter(|(a, b)| a != b).count();
    assert_eq!(changed, 4);
    library.revert_unsaved(index).unwrap();
    assert!(!library.is_dirty(index));
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    library
        .set_stomp_wave_selection(index, StompWaveSelection::Wave2)
        .unwrap();
    assert!(library.is_dirty(index));

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stomp-wave.l6t");
    library.tone(index).unwrap().save_copy(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), edited);
    assert!(library.tone(index).unwrap().save_copy(&path).is_err());
    let reopened = PodTone::parse(std::fs::read(path).unwrap()).unwrap();
    assert_eq!(reopened.as_bytes(), edited);
    assert_eq!(
        reopened.stomp_wave_selection().unwrap(),
        StompWaveValue::Known(StompWaveSelection::Wave2)
    );

    let bundle_bytes = bundle_with_tone(&original);
    let mut bundle = PodBundleLibrary::parse(bundle_bytes.clone()).unwrap();
    assert_eq!(
        bundle.stomp_wave_selection(0).unwrap(),
        StompWaveValue::Known(StompWaveSelection::Wave5)
    );
    bundle
        .set_stomp_wave_selection(0, StompWaveSelection::Wave3)
        .unwrap();
    assert!(bundle.is_dirty());
    let bundled_tone = bundle.tone(0, 0).unwrap();
    assert_eq!(
        bundled_tone.stomp_wave_selection().unwrap(),
        StompWaveValue::Known(StompWaveSelection::Wave3)
    );
    assert!(
        bundled_tone
            .as_bytes()
            .iter()
            .zip(&original)
            .filter(|(a, b)| a != b)
            .count()
            == 4
    );
    let reopened_bundle = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
    assert_eq!(
        reopened_bundle.tone(0, 0).unwrap().as_bytes(),
        bundled_tone.as_bytes()
    );
}

#[test]
fn whammy_knob_position_uses_source_float_quantizer_and_patches_only_selected_word() {
    let heel = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
    let toe = drive_record(0x3f, STOMP_WHAMMY_TOE_PARAM, 1, 0x3f00_0000);
    let unrelated = drive_record(0x3f, 0x0010_00aa, 1, 0x3e00_0000);
    let original = custom_tone(
        ContainerKind::L6as,
        &[
            BuiltModel {
                slot: STOMP_SLOT,
                model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
                tail: &[0x91, 0x82, 0x73, 0x64],
                parameters: &[&heel, &toe, &unrelated],
            },
            BuiltModel {
                slot: AMP_SLOT,
                model_id: PLEXI_45_MODEL,
                tail: &[0xde, 0xad, 0xbe, 0xef],
                parameters: &[&unrelated],
            },
        ],
    );
    let cases: [(f32, u32); 6] = [
        (0.0f32, 0x0000_0000),
        (0.25, 0x3e70_f0f2),
        (0.5, 0x3ef8_f8fa),
        (0.75, 0x3f3c_bcbd),
        (1.0, 0x3f7c_fcfe),
        (0.1234567, 0x3de0_e0e2),
    ];
    for control in StompWhammyKnob::ALL {
        for (position, expected_bits) in cases {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            assert_eq!(
                tone.stomp_whammy_knob_position(control).unwrap().to_bits(),
                if control == StompWhammyKnob::Heel {
                    0x3e80_0000
                } else {
                    0x3f00_0000
                }
            );
            tone.set_stomp_whammy_knob_position(control, position)
                .unwrap();
            let mut expected = original.clone();
            let (_, parameter_index) = tone.stomp_whammy_knob_location(control).unwrap();
            let model_index = tone
                .models
                .iter()
                .position(|model| model.slot == STOMP_SLOT)
                .unwrap();
            let offset = tone.parameter_value_offsets[model_index][parameter_index].unwrap();
            expected[offset..offset + 4].copy_from_slice(&expected_bits.to_be_bytes());
            assert_eq!(tone.as_bytes(), expected, "{control:?} at {position}");
        }
    }
}

#[test]
fn whammy_quantizer_transition_neighbors_and_fixed_points_match_source_float32_steps() {
    let heel = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0);
    let toe = drive_record(0x3f, STOMP_WHAMMY_TOE_PARAM, 1, 0);
    let original = whammy_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        STOMP_WHAMMY_INTERVAL_MODEL_ID,
        &[&heel, &toe],
    );
    let boundaries = [
        (0x3c80_8080, 0x0000_0000),
        (0x3c80_8081, 0x3c80_8081),
        (0x3c80_8082, 0x3c80_8081),
        (0x3f00_8080, 0x3ef8_f8fa),
        (0x3f00_8081, 0x3f00_8081),
        (0x3f00_8082, 0x3f00_8081),
        (0x3f7c_fcfc, 0x3f78_f8fa),
        (0x3f7c_fcfd, 0x3f7c_fcfe),
        (0x3f7c_fcfe, 0x3f7c_fcfe),
    ];
    for (input, expected) in boundaries {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, f32::from_bits(input))
            .unwrap();
        assert_eq!(
            tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
                .unwrap()
                .to_bits(),
            expected,
            "input {input:08x}"
        );
    }
    for control in StompWhammyKnob::ALL {
        for step in 0..=63 {
            let quantized = ((step as f32) * WHAMMY_INTERVAL_RESOLUTION_RECIPROCAL).to_bits();
            let mut tone = PodTone::parse(original.clone()).unwrap();
            tone.set_stomp_whammy_knob_position(control, f32::from_bits(quantized))
                .unwrap();
            assert_eq!(
                tone.stomp_whammy_knob_position(control).unwrap().to_bits(),
                quantized,
                "{control:?}, step {step}"
            );
        }
    }
}

#[test]
fn whammy_knob_rejects_invalid_inputs_and_existing_values_without_mutation() {
    let valid = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
    let original = whammy_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        STOMP_WHAMMY_INTERVAL_MODEL_ID,
        &[&valid],
    );
    for input in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert!(
            tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, input)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), original);
    }
    for bits in [
        f32::NAN.to_bits(),
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        (-0.01f32).to_bits(),
        1.01f32.to_bits(),
    ] {
        let invalid = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, bits);
        let bytes = whammy_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&invalid],
        );
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(
            tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
                .is_err()
        );
        assert!(
            tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.75)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), bytes);
    }
    let imported_raw = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e12_3456);
    let bytes = whammy_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        STOMP_WHAMMY_INTERVAL_MODEL_ID,
        &[&imported_raw],
    );
    let tone = PodTone::parse(bytes.clone()).unwrap();
    assert_eq!(
        tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
            .unwrap()
            .to_bits(),
        0x3e12_3456
    );
    assert_eq!(
        tone.as_bytes(),
        bytes,
        "reading an unquantized imported value is lossless"
    );
}

#[test]
fn whammy_knob_admission_checks_target_version_model_slot_record_prefix_type_and_opaque_data() {
    let valid = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
    for bytes in [
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&valid],
        ),
        stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x020a_000d, &[&valid]),
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[],
        ),
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&drive_record(0x3e, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000)],
        ),
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_WHAMMY_INTERVAL_MODEL_ID,
            &[&drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 2, 0x3e80_0000)],
        ),
    ] {
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(
            tone.stomp_whammy_knob_position(StompWhammyKnob::Heel)
                .is_err()
        );
        assert!(
            tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.5)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), bytes);
    }
    let mut wrong_target = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
            tail: &[],
            parameters: &[&valid],
        }],
    );
    let pinf = find_chunk_data(&wrong_target, b"PINF");
    wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000bu32.to_be_bytes());
    assert!(matches!(
        PodTone::parse(wrong_target),
        Err(ToneError::WrongTarget {
            device_id: 0x0003_000b
        })
    ));

    let wrong_slot = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
            tail: &[],
            parameters: &[&valid],
        }],
    );
    let mut tone = PodTone::parse(wrong_slot.clone()).unwrap();
    assert!(
        tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.5)
            .is_err()
    );
    assert_eq!(tone.as_bytes(), wrong_slot);

    let opaque = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: STOMP_WHAMMY_INTERVAL_MODEL_ID,
            tail: &[],
            parameters: &[&valid, b"\x00\x00"],
        }],
    );
    let mut tone = PodTone::parse(opaque.clone()).unwrap();
    assert!(
        tone.set_stomp_whammy_knob_position(StompWhammyKnob::Heel, 0.5)
            .is_err()
    );
    assert_eq!(tone.as_bytes(), opaque);
}

#[test]
fn whammy_knob_library_and_bundle_edits_revert_save_reopen_and_preserve_first_duplicate() {
    let early = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e00_0000);
    let later = drive_record(0x3f, STOMP_WHAMMY_HEEL_PARAM, 1, 0x3e80_0000);
    let toe = drive_record(0x3f, STOMP_WHAMMY_TOE_PARAM, 1, 0x3f00_0000);
    let original = whammy_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        STOMP_WHAMMY_INTERVAL_MODEL_ID,
        &[&early, &later, &toe],
    );
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    assert_eq!(
        library
            .stomp_whammy_knob_position(index, StompWhammyKnob::Heel)
            .unwrap()
            .to_bits(),
        0x3e80_0000,
        "last matching source record is selected"
    );
    library
        .set_stomp_whammy_knob_position(index, StompWhammyKnob::Heel, 0.75)
        .unwrap();
    assert!(library.is_dirty(index));
    let edited = library.tone(index).unwrap().as_bytes().to_vec();
    let changed: Vec<_> = original
        .iter()
        .zip(&edited)
        .enumerate()
        .filter_map(|(i, (a, b))| (a != b).then_some(i))
        .collect();
    assert_eq!(changed.len(), 4);
    let (_, parameter_index) = library
        .tone(index)
        .unwrap()
        .stomp_whammy_knob_location(StompWhammyKnob::Heel)
        .unwrap();
    let model_index = library
        .tone(index)
        .unwrap()
        .models
        .iter()
        .position(|model| model.slot == STOMP_SLOT)
        .unwrap();
    let selected =
        library.tone(index).unwrap().parameter_value_offsets[model_index][parameter_index].unwrap();
    assert_eq!(changed, (selected..selected + 4).collect::<Vec<_>>());
    let first_parameter = library.tone(index).unwrap().models[model_index].parameters.iter().position(|parameter| matches!(parameter, ModelParameter::Recorded { id, .. } if *id == STOMP_WHAMMY_HEEL_PARAM)).unwrap();
    let earlier =
        library.tone(index).unwrap().parameter_value_offsets[model_index][first_parameter].unwrap();
    assert_eq!(
        &edited[earlier..earlier + 4],
        &original[earlier..earlier + 4]
    );
    library.revert_unsaved(index).unwrap();
    assert!(!library.is_dirty(index));
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    library
        .set_stomp_whammy_knob_position(index, StompWhammyKnob::Heel, 0.75)
        .unwrap();

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("whammy-copy.l6t");
    std::fs::write(&path, b"keep existing copy").unwrap();
    assert!(library.tone(index).unwrap().save_copy(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep existing copy");
    let successful = directory.path().join("whammy-saved.l6t");
    library.tone(index).unwrap().save_copy(&successful).unwrap();
    let reopened = PodTone::parse(std::fs::read(successful).unwrap()).unwrap();
    assert_eq!(
        reopened
            .stomp_whammy_knob_position(StompWhammyKnob::Heel)
            .unwrap()
            .to_bits(),
        0x3f3c_bcbd
    );
    assert_eq!(
        reopened
            .stomp_whammy_knob_position(StompWhammyKnob::Toe)
            .unwrap()
            .to_bits(),
        0x3f00_0000
    );

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    bundle
        .set_stomp_whammy_knob_position(0, StompWhammyKnob::Toe, 0.25)
        .unwrap();
    assert!(bundle.is_dirty());
    let reopened_bundle = PodBundleLibrary::parse(bundle.as_bytes().to_vec()).unwrap();
    assert_eq!(
        reopened_bundle
            .stomp_whammy_knob_position(0, StompWhammyKnob::Toe)
            .unwrap()
            .to_bits(),
        0x3e70_f0f2
    );
    assert_eq!(
        reopened_bundle
            .stomp_whammy_knob_position(0, StompWhammyKnob::Heel)
            .unwrap()
            .to_bits(),
        0x3e80_0000
    );
}

#[test]
fn source_mapped_delay_percent_controls_patch_only_their_existing_value_words() {
    assert_eq!(DELAY_SLOT, 0x0002_0005);
    let profiles: [(u32, [Option<DelayControl>; 4]); 14] = [
        (
            0x0202_0002,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Bass),
                Some(DelayControl::Treble),
            ],
        ),
        (
            0x0202_0003,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::ModSpeed),
                Some(DelayControl::Depth),
            ],
        ),
        (
            0x0202_0004,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Flutter),
                Some(DelayControl::Drive),
            ],
        ),
        (
            0x0202_0005,
            [
                None,
                Some(DelayControl::Feedback),
                None,
                Some(DelayControl::Flutter),
            ],
        ),
        (
            0x0202_0006,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Speed),
                Some(DelayControl::Depth),
            ],
        ),
        (
            0x0202_0007,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Bass),
                Some(DelayControl::Treble),
            ],
        ),
        (
            0x0202_0008,
            [
                None,
                Some(DelayControl::Offset),
                Some(DelayControl::FeedbackLeft),
                Some(DelayControl::FeedbackRight),
            ],
        ),
        (
            0x0202_0009,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Offset),
                Some(DelayControl::Spread),
            ],
        ),
        (
            0x0202_000a,
            [None, Some(DelayControl::Feedback), None, None],
        ),
        (
            0x0202_000c,
            [
                None,
                Some(DelayControl::Feedback),
                None,
                Some(DelayControl::Flutter),
            ],
        ),
        (
            0x0202_000b,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Bass),
                Some(DelayControl::Treble),
            ],
        ),
        (
            0x0202_000d,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Tone),
                None,
            ],
        ),
        (
            0x0202_000e,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::ModSpeed),
                Some(DelayControl::Depth),
            ],
        ),
        (
            0x0202_000f,
            [
                None,
                Some(DelayControl::Feedback),
                Some(DelayControl::Speed),
                Some(DelayControl::Depth),
            ],
        ),
    ];
    assert_eq!(
        profiles.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        DELAY_CONTROL_MODEL_IDS
    );
    assert_eq!(
        profiles
            .iter()
            .map(|(_, row)| row.iter().flatten().count())
            .sum::<usize>(),
        37
    );
    for (model_id, layout) in profiles {
        for control in DelayControl::ALL {
            assert_eq!(
                control.selector_id(model_id).is_some(),
                layout.contains(&Some(control)),
                "unexpected {} mapping on model {model_id:08x}",
                control.label()
            );
        }
        let mut records = Vec::new();
        for control in layout.into_iter().flatten() {
            records.push(drive_record(
                0x3f,
                control.param_id(model_id).unwrap(),
                1,
                0x3f00_0000,
            ));
        }
        let record_refs: Vec<&[u8]> = records.iter().map(|r| r.as_slice()).collect();
        let bytes = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &record_refs);
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        for control in DelayControl::ALL {
            let Some(selector) = control.selector_id(model_id) else {
                continue;
            };
            let position = selector - 0x0005_0002;
            assert_eq!(selector, 0x0005_0002 + position);
            assert_eq!(control.param_id(model_id), Some(0x0010_0000 + position));
            assert_eq!(tone.delay_control_fraction(control).unwrap(), 0.5);
            tone.set_delay_control_fraction(control, 0.317).unwrap();
            let record = drive_record(0x3f, 0x0010_0000 + position, 1, 0x3f00_0000);
            let offset = bytes.windows(12).position(|w| w == record).unwrap() + 8;
            assert_eq!(
                &tone.as_bytes()[offset..offset + 4],
                &0.317f32.to_bits().to_be_bytes()
            );
        }
    }
}

#[test]
fn delay_association_choices_encode_exact_float32_controller_keys() {
    let expected = [
        0xbb81_0204,
        0x3df9_f3e8,
        0x3e7d_fbf8,
        0x3ebf_7efe,
        0x3f00_0000,
        0x3f20_4081,
        0x3f40_8102,
        0x3f60_c183,
        0x3f7e_fdfc,
    ];
    let head_labels = [
        "12--", "1-3-", "1--4", "-23-", "123-", "12-4", "1-34", "-234", "1234",
    ];
    let bit_labels = ["12", "11", "10", "9", "8", "7", "6", "5", "4"];
    assert_eq!(DelayHeadsSelection::ALL.len(), expected.len());
    assert_eq!(DelayBitsSelection::ALL.len(), expected.len());
    for index in 0..expected.len() {
        let heads = DelayHeadsSelection::ALL[index];
        let bits = DelayBitsSelection::ALL[index];
        assert_eq!(heads.label(), head_labels[index]);
        assert_eq!(bits.label(), bit_labels[index]);
        assert_eq!(heads.float_bits(), expected[index]);
        assert_eq!(bits.float_bits(), expected[index]);
        assert!(f32::from_bits(expected[index]).is_finite());
        assert_eq!(
            DelayHeadsSelection::from_float_bits(expected[index]),
            Some(heads)
        );
        assert_eq!(
            DelayBitsSelection::from_float_bits(expected[index]),
            Some(bits)
        );
    }
    assert!(f32::from_bits(expected[0]).is_sign_negative());
}

#[test]
fn delay_association_edits_are_typed_exact_last_match_value_word_patches() {
    let families = [
        (0x0202_0005, 0x0010_0002, 0x0005_0004),
        (0x0202_000d, 0x0010_0003, 0x0005_0005),
    ];
    for (model_id, association_id, selector_id) in families {
        assert_eq!(DELAY_SLOT, 0x0002_0005);
        let unrelated_id = if model_id == 0x0202_0005 {
            0x0010_0003
        } else {
            0x0010_0002
        };
        let first = drive_record(0x3f, association_id, 1, 0x3f00_0000);
        let unrelated = drive_record(0x3f, unrelated_id, 1, 0x3e80_0000);
        let last = drive_record(0x3f, association_id, 1, 0x3f20_4081);
        let original = delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            model_id,
            &[&first, &unrelated, &last],
        );
        let last_offset = original
            .windows(12)
            .rposition(|window| window == last)
            .unwrap();
        let first_offset = original
            .windows(12)
            .position(|window| window == first)
            .unwrap();
        let unrelated_offset = original
            .windows(12)
            .position(|window| window == unrelated)
            .unwrap();

        for index in 0..9 {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            if model_id == 0x0202_0005 {
                assert_eq!(selector_id, 0x0005_0004);
                assert_eq!(
                    tone.delay_heads_selection().unwrap(),
                    DelayAssociationValue::Known(DelayHeadsSelection::OneTwoMinusFour)
                );
                tone.set_delay_heads_selection(DelayHeadsSelection::ALL[index])
                    .unwrap();
                assert_eq!(
                    tone.delay_heads_selection().unwrap(),
                    DelayAssociationValue::Known(DelayHeadsSelection::ALL[index])
                );
            } else {
                assert_eq!(selector_id, 0x0005_0005);
                assert_eq!(
                    tone.delay_bits_selection().unwrap(),
                    DelayAssociationValue::Known(DelayBitsSelection::Seven)
                );
                tone.set_delay_bits_selection(DelayBitsSelection::ALL[index])
                    .unwrap();
                assert_eq!(
                    tone.delay_bits_selection().unwrap(),
                    DelayAssociationValue::Known(DelayBitsSelection::ALL[index])
                );
            }
            let changed: Vec<_> = original
                .iter()
                .zip(tone.as_bytes())
                .enumerate()
                .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
                .collect();
            let expected_bits = if model_id == 0x0202_0005 {
                DelayHeadsSelection::ALL[index].float_bits()
            } else {
                DelayBitsSelection::ALL[index].float_bits()
            };
            assert!(
                changed
                    .iter()
                    .all(|offset| (last_offset + 8..last_offset + 12).contains(offset))
            );
            assert_eq!(
                &tone.as_bytes()[last_offset + 8..last_offset + 12],
                &expected_bits.to_be_bytes()
            );
            assert_eq!(
                &tone.as_bytes()[first_offset..first_offset + 12],
                &original[first_offset..first_offset + 12]
            );
            assert_eq!(
                &tone.as_bytes()[unrelated_offset..unrelated_offset + 12],
                &original[unrelated_offset..unrelated_offset + 12]
            );
            let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
            if model_id == 0x0202_0005 {
                assert_eq!(
                    reopened.delay_heads_selection().unwrap(),
                    DelayAssociationValue::Known(DelayHeadsSelection::ALL[index])
                );
            } else {
                assert_eq!(
                    reopened.delay_bits_selection().unwrap(),
                    DelayAssociationValue::Known(DelayBitsSelection::ALL[index])
                );
            }
        }
    }
}

#[test]
fn delay_associations_preserve_unknown_values_until_an_explicit_choice() {
    for (model_id, parameter_id, is_heads) in [
        (0x0202_0005, 0x0010_0002, true),
        (0x0202_000d, 0x0010_0003, false),
    ] {
        for raw_bits in [0x3f12_3456, 0x7fc0_0001, 0xbb81_0203] {
            let record = drive_record(0x3f, parameter_id, 1, raw_bits);
            let original = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&record]);
            let tone = PodTone::parse(original.clone()).unwrap();
            if is_heads {
                assert_eq!(
                    tone.delay_heads_selection().unwrap(),
                    DelayAssociationValue::Unknown { raw_bits }
                );
            } else {
                assert_eq!(
                    tone.delay_bits_selection().unwrap(),
                    DelayAssociationValue::Unknown { raw_bits }
                );
            }
            assert_eq!(tone.as_bytes(), original);
        }
    }
}

#[test]
fn finite_unknown_delay_association_values_can_be_explicitly_replaced() {
    for (model_id, parameter_id, is_heads) in [
        (0x0202_0005, 0x0010_0002, true),
        (0x0202_000d, 0x0010_0003, false),
    ] {
        for raw_bits in [0x3f12_3456, 0xbb81_0203] {
            assert!(f32::from_bits(raw_bits).is_finite());
            let record = drive_record(0x3f, parameter_id, 1, raw_bits);
            let mut tone = PodTone::parse(delay_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                model_id,
                &[&record],
            ))
            .unwrap();
            let result = if is_heads {
                tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
            } else {
                tone.set_delay_bits_selection(DelayBitsSelection::Four)
            };
            assert!(result.is_ok());
            assert!(if is_heads {
                matches!(
                    tone.delay_heads_selection().unwrap(),
                    DelayAssociationValue::Known(DelayHeadsSelection::OneTwoThreeFour)
                )
            } else {
                matches!(
                    tone.delay_bits_selection().unwrap(),
                    DelayAssociationValue::Known(DelayBitsSelection::Four)
                )
            });
        }
    }
}

#[test]
fn delay_association_setters_and_bundle_edits_reject_nonfinite_existing_values_transactionally() {
    for (model_id, parameter_id, is_heads) in [
        (0x0202_0005, 0x0010_0002, true),
        (0x0202_000d, 0x0010_0003, false),
    ] {
        for raw_bits in [0x7fc0_0001, 0x7f80_0000, 0xff80_0000] {
            let record = drive_record(0x3f, parameter_id, 1, raw_bits);
            let original_tone = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&record]);
            let mut tone = PodTone::parse(original_tone.clone()).unwrap();
            assert!(if is_heads {
                matches!(tone.delay_heads_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
            } else {
                matches!(tone.delay_bits_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
            });
            let result = if is_heads {
                tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
            } else {
                tone.set_delay_bits_selection(DelayBitsSelection::Four)
            };
            assert!(result.is_err());
            assert_eq!(tone.as_bytes(), original_tone);
            assert!(if is_heads {
                matches!(tone.delay_heads_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
            } else {
                matches!(tone.delay_bits_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
            });

            let original_bundle = bundle_with_tone(&original_tone);
            let mut library = PodBundleLibrary::parse(original_bundle.clone()).unwrap();
            assert!(!library.is_dirty());
            let result = library.edit_entry(0, |tone| {
                if is_heads {
                    tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
                } else {
                    tone.set_delay_bits_selection(DelayBitsSelection::Four)
                }
            });
            assert!(result.is_err());
            assert_eq!(library.as_bytes(), original_bundle);
            assert!(!library.is_dirty());
            assert!(if is_heads {
                matches!(library.tone(0, 0).unwrap().delay_heads_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
            } else {
                matches!(library.tone(0, 0).unwrap().delay_bits_selection().unwrap(), DelayAssociationValue::Unknown { raw_bits: found } if found == raw_bits)
            });
        }
    }
}

#[test]
fn delay_association_admission_rejects_wrong_target_layout_and_record_before_mutation() {
    let heads_record = drive_record(0x3f, 0x0010_0002, 1, 0x3f00_0000);
    let bits_record = drive_record(0x3f, 0x0010_0003, 1, 0x3f00_0000);
    let mut wrong_product = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0005, &[&heads_record]);
    let pinf = find_chunk_data(&wrong_product, b"PINF");
    wrong_product[pinf + 4..pinf + 8].copy_from_slice(&0x0003_0009u32.to_be_bytes());

    let wrong_slot = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 0x0202_0005,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters: &[&heads_record],
        }],
    );
    let opaque = [1, 2, 3, 4];
    let heads_bad_cases = [
        delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0006, &[&heads_record]),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            0x0202_0005,
            &[&heads_record],
        ),
        delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0005, &[]),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            0x0202_0005,
            &[&opaque, &heads_record],
        ),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            0x0202_0005,
            &[&drive_record(0x7e, 0x0010_0002, 1, 0x3f00_0000)],
        ),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            0x0202_0005,
            &[&drive_record(0x3f, 0x0010_0002, 0, 0x3f00_0000)],
        ),
        wrong_slot,
    ];
    assert!(matches!(
        PodTone::parse(wrong_product),
        Err(ToneError::WrongTarget { .. })
    ));
    for bytes in heads_bad_cases {
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.delay_heads_selection().is_err());
        assert!(
            tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
                .is_err()
        );
        assert!(
            tone.set_delay_bits_selection(DelayBitsSelection::Four)
                .is_err()
        );
        assert_eq!(tone.as_bytes(), bytes);
    }
    let bits_bad_cases = [
        delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_000c, &[&bits_record]),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            0x0202_000d,
            &[&bits_record],
        ),
        delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_000d, &[]),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            0x0202_000d,
            &[&opaque, &bits_record],
        ),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            0x0202_000d,
            &[&drive_record(0x7e, 0x0010_0003, 1, 0x3f00_0000)],
        ),
        delay_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            0x0202_000d,
            &[&drive_record(0x3f, 0x0010_0003, 0, 0x3f00_0000)],
        ),
    ];
    for bytes in bits_bad_cases {
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.delay_bits_selection().is_err());
        assert!(
            tone.set_delay_bits_selection(DelayBitsSelection::Four)
                .is_err()
        );
        assert!(tone.as_bytes() == bytes);
    }
}

#[test]
fn delay_association_library_edit_save_collision_revert_and_reopen_are_transactional() {
    let parameter = drive_record(0x3f, 0x0010_0002, 1, 0xbb81_0204);
    let original_tone = delay_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0202_0005, &[&parameter]);
    let original_bundle = bundle_with_tone(&original_tone);
    let mut library = PodBundleLibrary::parse(original_bundle.clone()).unwrap();
    assert!(!library.is_dirty());
    library
        .edit_entry(0, |tone| {
            tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
        })
        .unwrap();
    assert!(library.is_dirty());
    let collision_dir = tempfile::tempdir().unwrap();
    let collision_path = collision_dir.path().join("delay.l6b");
    std::fs::write(&collision_path, b"keep destination").unwrap();
    assert!(library.save_copy(&collision_path).is_err());
    assert_eq!(std::fs::read(&collision_path).unwrap(), b"keep destination");
    library.revert().unwrap();
    assert_eq!(library.as_bytes(), original_bundle);
    assert!(!library.is_dirty());

    library
        .edit_entry(0, |tone| {
            tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
        })
        .unwrap();
    let save_dir = tempfile::tempdir().unwrap();
    let save_path = save_dir.path().join("delay.l6b");
    library.save_copy(&save_path).unwrap();
    let reopened = PodBundleLibrary::parse(std::fs::read(save_path).unwrap()).unwrap();
    assert_eq!(
        reopened
            .tone(0, 0)
            .unwrap()
            .delay_heads_selection()
            .unwrap(),
        DelayAssociationValue::Known(DelayHeadsSelection::OneTwoThreeFour)
    );
    library.note_saved();
    assert!(!library.is_dirty());
}

#[test]
fn mapped_effect_family_edits_have_independent_ids_full_file_preservation_and_saved_reparse() {
    let cases = [
        // Line6Edit positions are one-based here; these explicit expected
        // selector/PARM pairs are kept separate from the Rust lookup tables.
        (
            MappedEffect::Mod,
            MOD_SLOT,
            0x0203_0004,
            0x0004_0003,
            0x0010_0001,
        ),
        // Reverb's provider selector order is not the semantic PARM order.
        (
            MappedEffect::Reverb,
            REVERB_SLOT,
            0x0204_0002,
            0x0008_0001,
            0x0010_0000,
        ),
        (
            MappedEffect::Delay,
            DELAY_SLOT,
            0x0202_0002,
            0x0005_0003,
            0x0010_0001,
        ),
    ];

    for (effect, slot, model_id, expected_selector, expected_parameter) in cases {
        match effect {
            MappedEffect::Mod => {
                assert_eq!(
                    ModControl::Depth.selector_id(model_id),
                    Some(expected_selector)
                );
                assert_eq!(
                    ModControl::Depth.param_id(model_id),
                    Some(expected_parameter)
                );
            }
            MappedEffect::Reverb => {
                assert_eq!(
                    ReverbControl::Dwell.selector_id(model_id),
                    Some(expected_selector)
                );
                assert_eq!(
                    ReverbControl::Dwell.param_id(model_id),
                    Some(expected_parameter)
                );
            }
            MappedEffect::Delay => {
                assert_eq!(
                    DelayControl::Feedback.selector_id(model_id),
                    Some(expected_selector)
                );
                assert_eq!(
                    DelayControl::Feedback.param_id(model_id),
                    Some(expected_parameter)
                );
            }
        }

        let earlier = drive_record(0x3f, expected_parameter, 1, 0x3e80_0000);
        let selected = drive_record(0x3f, expected_parameter, 1, 0x3f00_0000);
        let unrelated = drive_record(0xa7, 0x0000_9876, 9, 0x1234_5678);
        let other_model_parameter = drive_record(0x3f, AMP_DRIVE_PARAM, 1, 0x3e00_0000);
        let target_parameters: [&[u8]; 3] = [&earlier, &unrelated, &selected];
        let other_parameters: [&[u8]; 1] = [&other_model_parameter];
        let original = custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot,
                    model_id,
                    tail: &[0x91, 0x82, 0x73, 0x64],
                    parameters: &target_parameters,
                },
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: PLEXI_LEAD_100_MODEL,
                    tail: &[0x19, 0x28, 0x37, 0x46],
                    parameters: &other_parameters,
                },
            ],
        );
        let selected_offset = original
            .windows(selected.len())
            .rposition(|window| window == selected)
            .unwrap()
            + 8;
        let mut expected = original.clone();
        expected[selected_offset..selected_offset + 4]
            .copy_from_slice(&0x3e00_0000u32.to_be_bytes());

        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        match effect {
            MappedEffect::Mod => library
                .set_mod_control_fraction(index, ModControl::Depth, 0.125)
                .unwrap(),
            MappedEffect::Reverb => library
                .set_reverb_control_fraction(index, ReverbControl::Dwell, 0.125)
                .unwrap(),
            MappedEffect::Delay => library
                .set_delay_control_fraction(index, DelayControl::Feedback, 0.125)
                .unwrap(),
        }
        let tone = library.tone(index).unwrap();
        assert_eq!(tone.as_bytes(), expected);
        assert_eq!(
            mapped_effect_value(tone, effect).unwrap().to_bits(),
            0x3e00_0000
        );
        assert!(library.is_dirty(index));

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("mapped-effect.l6t");
        tone.save_copy(&path).unwrap();
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(saved, expected);
        let reopened = PodTone::parse(saved).unwrap();
        assert_eq!(
            mapped_effect_value(&reopened, effect).unwrap().to_bits(),
            0x3e00_0000
        );
        assert_eq!(reopened.as_bytes(), expected);
        assert_eq!(
            &reopened.as_bytes()[selected_offset..selected_offset + 4],
            &0x3e00_0000u32.to_be_bytes()
        );
        assert!(
            reopened
                .as_bytes()
                .windows(earlier.len())
                .any(|w| w == earlier)
        );
        assert!(
            reopened
                .as_bytes()
                .windows(unrelated.len())
                .any(|w| w == unrelated)
        );
        assert!(
            reopened
                .as_bytes()
                .windows(other_model_parameter.len())
                .any(|w| w == other_model_parameter)
        );
    }
}

#[test]
fn each_mapped_effect_family_rejects_bad_targets_records_and_values_transactionally() {
    let cases = [
        (MappedEffect::Mod, MOD_SLOT, 0x0203_0004, 0x0010_0001),
        (MappedEffect::Reverb, REVERB_SLOT, 0x0204_0002, 0x0010_0000),
        (MappedEffect::Delay, DELAY_SLOT, 0x0202_0002, 0x0010_0001),
    ];
    let opaque = [1, 2, 3, 4];

    for (effect, slot, model_id, parameter_id) in cases {
        let valid = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
        let bad_prefix = drive_record(0x7e, parameter_id, 1, 0x3f00_0000);
        let bad_type = drive_record(0x3f, parameter_id, 0, 0x3f00_0000);
        let missing = mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[]);
        let opaque_tone =
            mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&opaque]);
        let wrong_prefix =
            mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&bad_prefix]);
        let wrong_type =
            mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&bad_type]);
        let wrong_version =
            mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION - 1, &[&valid]);
        let unsupported_slot =
            mapped_effect_tone(slot + 1, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&valid]);
        let unsupported_model = mapped_effect_tone(
            slot,
            model_id + 0x1000,
            PODXT_LIVE_HOST_MIN_VERSION,
            &[&valid],
        );
        let nonfinite_current = drive_record(0x3f, parameter_id, 1, f32::NAN.to_bits());
        let out_of_range_current = drive_record(0x3f, parameter_id, 1, 0x3f80_0001);
        let invalid_floats = [
            f32::from_bits(0x8000_0001),
            f32::from_bits(0x3f80_0001),
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ];

        for bytes in [
            missing,
            opaque_tone,
            wrong_prefix,
            wrong_type,
            wrong_version,
            unsupported_slot,
            unsupported_model,
            mapped_effect_tone(
                slot,
                model_id,
                PODXT_LIVE_HOST_MIN_VERSION,
                &[&nonfinite_current],
            ),
            mapped_effect_tone(
                slot,
                model_id,
                PODXT_LIVE_HOST_MIN_VERSION,
                &[&out_of_range_current],
            ),
        ] {
            let original = bytes.clone();
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(set_mapped_effect(&mut tone, effect, false, 0.25).is_err());
            assert_eq!(tone.as_bytes(), original);
            assert!(set_mapped_effect(&mut tone, effect, true, 0.25).is_err());
            assert_eq!(tone.as_bytes(), original);
        }

        for invalid in invalid_floats {
            let bytes = mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&valid]);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(set_mapped_effect(&mut tone, effect, false, invalid).is_err());
            assert_eq!(tone.as_bytes(), bytes);
        }

        let mut wrong_target =
            mapped_effect_tone(slot, model_id, PODXT_LIVE_HOST_MIN_VERSION, &[&valid]);
        let pinf = find_chunk_data(&wrong_target, b"PINF");
        wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_0002u32.to_be_bytes());
        let original = wrong_target.clone();
        assert!(PodTone::parse(wrong_target.clone()).is_err());
        assert_eq!(wrong_target, original);
    }
}

#[test]
fn stomp_controls_reject_unsupported_targets_records_and_values_transactionally() {
    let drive = drive_record(0x3f, STOMP_DRIVE_PARAM, 1, 0x3f00_0000);
    let wrong_prefix = drive_record(0x7e, STOMP_DRIVE_PARAM, 1, 0x3f00_0000);
    let wrong_type = drive_record(0x3f, STOMP_DRIVE_PARAM, 0, 0x3f00_0000);
    let mut wrong_product =
        stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, STOMP_DRIVE_MODEL_ID, &[&drive]);
    let pinf = find_chunk_data(&wrong_product, b"PINF");
    wrong_product[pinf + 4..pinf + 8].copy_from_slice(&0x0003_0002u32.to_be_bytes());
    let wrong_slot = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: STOMP_SLOT + 1,
            model_id: STOMP_DRIVE_MODEL_ID,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters: &[&drive],
        }],
    );
    let invalid_targets = [
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION - 1,
            STOMP_DRIVE_MODEL_ID,
            &[&drive],
        ),
        stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0205_000a, &[&drive]),
        stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, STOMP_DRIVE_MODEL_ID, &[]),
        wrong_slot,
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_DRIVE_MODEL_ID,
            &[&wrong_prefix],
        ),
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_DRIVE_MODEL_ID,
            &[&wrong_type],
        ),
        stomp_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            STOMP_DRIVE_MODEL_ID,
            &[&[1, 2, 3, 4], &drive],
        ),
    ];
    assert!(PodTone::parse(wrong_product).is_err());
    for model_id in STOMP_CONTROL_MODEL_IDS {
        for control in StompControl::ALL {
            let Some(parameter_id) = control.param_id(model_id) else {
                continue;
            };
            let record = drive_record(0x3f, parameter_id, 1, 0x3f00_0000);
            let wrong_prefix = drive_record(0x7e, parameter_id, 1, 0x3f00_0000);
            let wrong_type = drive_record(0x3f, parameter_id, 0, 0x3f00_0000);
            let invalid_model = stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0205_000a, &[&record]);
            let fixtures = invalid_targets.iter().cloned().chain([
                invalid_model,
                stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&wrong_prefix]),
                stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&wrong_type]),
                stomp_tone(
                    PODXT_LIVE_HOST_MIN_VERSION,
                    model_id,
                    &[&[1, 2, 3, 4], &record],
                ),
            ]);
            for bytes in fixtures {
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert!(tone.stomp_control_fraction(control).is_err());
                assert!(tone.set_stomp_control_fraction(control, 0.25).is_err());
                assert_eq!(tone.as_bytes(), bytes);
            }
            for invalid in [
                f32::from_bits(0x8000_0001),
                f32::from_bits(0x3f80_0001),
                f32::NAN,
                f32::INFINITY,
                f32::NEG_INFINITY,
            ] {
                let bytes = stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&record]);
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                assert!(tone.set_stomp_control_fraction(control, invalid).is_err());
                assert_eq!(tone.as_bytes(), bytes);
            }
            let nonfinite = drive_record(0x3f, parameter_id, 1, f32::NAN.to_bits());
            let bytes = stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &[&nonfinite]);
            assert!(
                PodTone::parse(bytes)
                    .unwrap()
                    .stomp_control_fraction(control)
                    .is_err()
            );
        }
    }
}

#[test]
fn stomp_control_library_revert_copy_collision_and_reopen_keep_safe_copy_rules() {
    for model_id in STOMP_CONTROL_MODEL_IDS {
        let records: Vec<_> = StompControl::ALL
            .into_iter()
            .filter_map(|control| {
                control
                    .param_id(model_id)
                    .map(|parameter_id| drive_record(0x3f, parameter_id, 1, 0x3f00_0000))
            })
            .collect();
        let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
        let original = stomp_tone(PODXT_LIVE_HOST_MIN_VERSION, model_id, &refs);
        for control in StompControl::ALL {
            if control.param_id(model_id).is_none() {
                continue;
            }
            let mut library = Library::default();
            let index = library.import_bytes(original.clone()).unwrap();
            library
                .set_stomp_control_fraction(index, control, 0.625)
                .unwrap();
            assert!(library.is_dirty(index), "{}", control.label());
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("stomp.l6t");
            std::fs::write(&path, b"keep existing destination").unwrap();
            assert!(library.tone(index).unwrap().save_copy(&path).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), b"keep existing destination");
            library.revert_unsaved(index).unwrap();
            assert_eq!(library.tone(index).unwrap().as_bytes(), original);
            library
                .set_stomp_control_fraction(index, control, 0.625)
                .unwrap();
            std::fs::remove_file(&path).unwrap();
            library.tone(index).unwrap().save_copy(&path).unwrap();
            let reopened = PodTone::parse(std::fs::read(path).unwrap()).unwrap();
            assert_eq!(reopened.stomp_control_fraction(control).unwrap(), 0.625);
            assert!(library.is_dirty(index));
            library.note_saved(index).unwrap();
            assert!(!library.is_dirty(index));
        }
    }
}

#[test]
fn amp_control_rejections_are_transactional_for_domain_and_target_failures() {
    for control in AmpControl::ALL {
        let record = drive_record(0x3f, control.param_id(), 1, 0x3f00_0000);
        for invalid in [
            f32::from_bits(0x8000_0001),
            f32::from_bits(0x3f80_0001),
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            let bytes = drive_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                PLEXI_LEAD_100_MODEL,
                &[&record],
            );
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.set_amp_control_fraction(control, invalid).is_err());
            assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
        }
        let fixtures = [
            drive_tone(
                PODXT_LIVE_HOST_MIN_VERSION - 1,
                PLEXI_LEAD_100_MODEL,
                &[&record],
            ),
            drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &[]),
            drive_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0002_0000, &[&record]),
            drive_tone(
                PODXT_LIVE_HOST_MIN_VERSION,
                STOMP_DRIVE_MODEL_ID,
                &[&record],
            ),
            drive_tone(PODXT_LIVE_HOST_MIN_VERSION, 0x0002_0050, &[&record]),
        ];
        for bytes in fixtures {
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.set_amp_control_fraction(control, 0.25).is_err());
            assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
        }
        let wrong_prefix = drive_record(0x7e, control.param_id(), 1, 0x3f00_0000);
        let wrong_type = drive_record(0x3f, control.param_id(), 0, 0x3f00_0000);
        for parm in [&wrong_prefix[..], &wrong_type[..], &[1, 2, 3, 4][..]] {
            let bytes = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &[parm]);
            let mut tone = PodTone::parse(bytes.clone()).unwrap();
            assert!(tone.set_amp_control_fraction(control, 0.25).is_err());
            assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
        }
        let nonfinite = drive_record(0x3f, control.param_id(), 1, f32::NAN.to_bits());
        let bytes = drive_tone(
            PODXT_LIVE_HOST_MIN_VERSION,
            PLEXI_LEAD_100_MODEL,
            &[&nonfinite],
        );
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.set_amp_control_fraction(control, 0.25).is_err());
        assert_eq!(tone.as_bytes(), bytes, "{}", control.label());
    }
}

#[test]
fn amp_control_library_edit_revert_save_copy_and_collision_keep_safe_copy_rules() {
    let records: Vec<_> = AmpControl::ALL
        .into_iter()
        .map(|control| drive_record(0x3f, control.param_id(), 1, 0x3f00_0000))
        .collect();
    let refs: Vec<_> = records.iter().map(|record| &record[..]).collect();
    let original = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &refs);
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    for (number, control) in AmpControl::ALL.into_iter().enumerate() {
        library
            .set_amp_control_fraction(index, control, 0.125 + number as f32 * 0.125)
            .unwrap();
    }
    assert!(library.is_dirty(index));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("drive.l6t");
    std::fs::write(&path, b"pre-existing bytes").unwrap();
    assert!(library.tone(index).unwrap().save_copy(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"pre-existing bytes");
    assert!(library.is_dirty(index));
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    for (number, control) in AmpControl::ALL.into_iter().enumerate() {
        library
            .set_amp_control_fraction(index, control, 0.125 + number as f32 * 0.125)
            .unwrap();
    }
    std::fs::remove_file(&path).unwrap();
    library.tone(index).unwrap().save_copy(&path).unwrap();
    assert!(library.tone(index).unwrap().save_copy(&path).is_err());
    let saved = std::fs::read(path).unwrap();
    let reopened = PodTone::parse(saved).unwrap();
    for (number, control) in AmpControl::ALL.into_iter().enumerate() {
        assert_eq!(
            reopened.amp_control_fraction(control).unwrap().to_bits(),
            (0.125 + number as f32 * 0.125).to_bits(),
            "{}",
            control.label()
        );
    }
    assert!(library.is_dirty(index));
    library.note_saved(index).unwrap();
    assert!(!library.is_dirty(index));
}

pub(crate) fn live_tone() -> Vec<u8> {
    fixture(
        ContainerKind::L6as,
        PODXT_LIVE_TONE_ID,
        0x0011_2233,
        &[
            (AMP_SLOT, 0x1111_0002),
            (CAB_SLOT, 0x2222_0003),
            (0x0003_00aa, 0x3333_0004),
        ],
    )
}

#[test]
fn tone_id_matches_the_archived_product_id_and_not_the_midi_hardware_id() {
    assert_eq!(PODXT_LIVE_TONE_ID, crate::firmware::PODXT_LIVE_PRODUCT_ID);
    assert_eq!(PODXT_LIVE_TONE_ID, 196_618);
    assert_ne!(PODXT_LIVE_TONE_ID, 18_000);
    const {
        assert!(TONE_BYTES_CEILING == 1_048_576);
    }
}

#[test]
fn l6as_exposes_raw_amp_and_cab_and_preserves_unknown_bytes() {
    let original = live_tone();
    let mut tone = PodTone::parse(original.clone()).unwrap();
    assert_eq!(tone.as_bytes(), original);
    assert_eq!(tone.container(), ContainerKind::L6as);
    assert_eq!(tone.device_version(), 0x0011_2233);
    assert_ne!(tone.device_version(), crate::firmware::PODXT_LIVE_VERSION);
    assert_eq!(tone.amp_model_id(), Some(0x1111_0002));
    assert_eq!(tone.cab_model_id(), Some(0x2222_0003));
    assert_eq!(tone.amp_asset_name(), None);
    assert_eq!(tone.cab_asset_name(), None);
    assert_eq!(tone.undecoded_slots(), 1);
    assert_eq!(tone.recorded_parameter_chunks(), 3);
    assert_eq!(tone.undecoded_parameter_chunks(), 3);
    assert_eq!(tone.models()[0].ordinal, Some(0x10));
    assert_eq!(tone.models()[0].reserved, Some([0x20, 0x30]));
    assert_eq!(tone.models()[0].enabled, Some(true));
    assert!(matches!(
        tone.models()[0].parameters[0],
        ModelParameter::Opaque { len: 4 }
    ));
    assert!(matches!(
        tone.models()[0].parameters[1],
        ModelParameter::Recorded {
            prefix: 0xab,
            id: 0x00cd_ef01,
            value: ParameterValue::Other {
                type_word: 0x0203_0405,
                bits: 0x0607_0809,
            },
        }
    ));
    assert_eq!(tone.name(), "A");
    tone.rename("Stage").unwrap();
    let reread = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
    assert_eq!(reread.name(), "Stage");
    assert_eq!(reread.amp_model_id(), Some(0x1111_0002));
    assert_eq!(reread.cab_model_id(), Some(0x2222_0003));
    let name_at = tone_name_offset(&original);
    for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
        if !(name_at..name_at + 64).contains(&index) {
            assert_eq!(before, after, "byte {index} changed");
        }
    }
    assert!(
        tone.as_bytes()
            .windows(4)
            .any(|bytes| bytes == [0x11, 0x22, 0x33, 0x44])
    );
    assert!(
        tone.as_bytes()
            .windows(4)
            .any(|bytes| bytes == [0xab, 0xcd, 0xef, 0x01])
    );
    assert!(
        tone.as_bytes()
            .windows(4)
            .any(|bytes| bytes == [0x5a, 0x5a, 0x5a, 0x5a])
    );
    assert!(tone.as_bytes().windows(3).any(|bytes| bytes == [9, 8, 7]));
}

#[test]
fn cab_slot_on_l6pa_and_l6fs_is_not_the_cab_model() {
    for kind in [ContainerKind::L6pa, ContainerKind::L6fs] {
        let tone = PodTone::parse(fixture(
            kind,
            PODXT_LIVE_TONE_ID,
            0,
            &[(AMP_SLOT, 7), (CAB_SLOT, 8)],
        ))
        .unwrap();
        assert_eq!(tone.container(), kind);
        assert_eq!(tone.amp_model_id(), Some(7));
        assert_eq!(tone.cab_model_id(), None);
        assert_eq!(tone.cab_asset_name(), None);
        assert_eq!(tone.undecoded_slots(), 0);
        assert_eq!(
            SlotRole::from_id(tone.models()[1].slot),
            Some(SlotRole::Cab)
        );
        assert_eq!(tone.models()[1].slot, CAB_SLOT);
        assert_eq!(tone.models()[1].model_id, 8);
    }
}

#[test]
fn missing_amp_is_absent_and_other_products_are_rejected() {
    let empty = PodTone::parse(fixture(ContainerKind::L6pa, PODXT_LIVE_TONE_ID, 0, &[])).unwrap();
    assert_eq!(empty.amp_model_id(), None);
    assert_eq!(empty.cab_model_id(), None);
    assert_eq!(empty.undecoded_slots(), 0);
    for device_id in [
        0x0007_0004u32,
        0x0007_0000,
        0x0003_000b,
        0x0003_0002,
        196_608,
    ] {
        let error = PodTone::parse(fixture(ContainerKind::L6pa, device_id, 0, &[(AMP_SLOT, 1)]))
            .unwrap_err();
        assert_eq!(error, ToneError::WrongTarget { device_id });
        assert!(!error.detail().contains("decoded"));
    }
}

#[test]
fn rejects_banks_bad_versions_duplicates_and_truncation() {
    let source = live_tone();
    let mut bank = source.clone();
    bank[8..12].copy_from_slice(b"L6BA");
    assert!(PodTone::parse(bank).is_err());
    let mut version = source.clone();
    let info = find_chunk_data(&version, b"PINF");
    version[info..info + 4].copy_from_slice(&2u32.to_be_bytes());
    assert!(PodTone::parse(version).is_err());
    let mut duplicated = fixture(
        ContainerKind::L6as,
        PODXT_LIVE_TONE_ID,
        1,
        &[(AMP_SLOT, 1), (AMP_SLOT, 2)],
    );
    assert!(
        PodTone::parse(duplicated.clone())
            .unwrap_err()
            .detail()
            .contains("duplicate")
    );
    for end in 0..12 {
        assert!(PodTone::parse(source[..end].to_vec()).is_err());
    }
    duplicated.truncate(20);
    duplicated[4..8].copy_from_slice(&12u32.to_be_bytes());
    assert!(PodTone::parse(duplicated).is_err());
    assert!(PodTone::parse(vec![0; TONE_BYTES_CEILING as usize + 1]).is_err());
}

#[test]
fn recovered_controls_keep_unknown_bytes_and_do_not_edit_parameters() {
    let original = custom_tone(
        ContainerKind::L6as,
        &[
            BuiltModel {
                slot: AMP_SLOT,
                model_id: 131_073,
                tail: &[0x04, 0x00, 0x00, 0x01, 0xaa, 0xbb, 0xcc, 0xdd],
                parameters: &[
                    &[
                        0x3f, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x01, 0x3f, 0x80, 0x00, 0x00,
                    ],
                    &[0x3f, 0x00, 0x00, 0x01],
                    &[
                        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                        0xde, 0xef, 0x01, 0x02,
                    ],
                ],
            },
            BuiltModel {
                slot: CAB_SLOT,
                model_id: 0x0100_000f,
                tail: &[0x02, 0x7f, 0x7e, 0x00],
                parameters: &[],
            },
            BuiltModel {
                slot: 131_074,
                model_id: 0x0205_0000,
                tail: &[0x01, 0x10, 0x20, 0x03],
                parameters: &[],
            },
            BuiltModel {
                slot: 0x0003_00aa,
                model_id: 0x3333_0004,
                tail: &[],
                parameters: &[],
            },
        ],
    );
    let mut tone = PodTone::parse(original.clone()).unwrap();
    assert_eq!(tone.amp_model_id(), Some(131_073));
    assert_eq!(tone.amp_asset_name(), Some("Line 6 Clean"));
    assert_eq!(tone.cab_model_id(), Some(0x0100_000f));
    assert_eq!(tone.cab_asset_name(), Some("No Cabinet"));
    assert_eq!(tone.undecoded_slots(), 1);
    assert_eq!(tone.recorded_parameter_chunks(), 1);
    assert_eq!(tone.undecoded_parameter_chunks(), 2);
    let amp = &tone.models()[0];
    assert_eq!(amp.ordinal, Some(0x04));
    assert_eq!(amp.reserved, Some([0x00, 0x00]));
    assert_eq!(amp.enabled, Some(true));
    assert_eq!(
        amp.parameters[0],
        ModelParameter::Recorded {
            prefix: 0x3f,
            id: 7,
            value: ParameterValue::Float { bits: 0x3f80_0000 },
        }
    );
    assert_eq!(amp.parameters[1], ModelParameter::Opaque { len: 4 });
    assert_eq!(amp.parameters[2], ModelParameter::Opaque { len: 16 });
    let cab = &tone.models()[1];
    assert_eq!(cab.enabled, Some(false));
    assert_eq!(cab.reserved, Some([0x7f, 0x7e]));
    let wah = &tone.models()[2];
    assert_eq!(SlotRole::from_id(wah.slot), Some(SlotRole::Wah));
    assert_eq!(pod_assets::asset_name(wah.model_id), Some("Facial Fuzz"));
    assert_eq!(wah.ordinal, Some(1));
    assert_eq!(wah.enabled, Some(true));
    let unknown = &tone.models()[3];
    assert_eq!(unknown.ordinal, None);
    assert_eq!(unknown.reserved, None);
    assert_eq!(unknown.enabled, None);
    assert_eq!(SlotRole::from_id(unknown.slot), None);
    let lines = tone.control_lines();
    assert!(lines[0].contains("Amp slot 00030000 model Line 6 Clean 00020001"));
    assert!(lines[0].contains(" enabled"));
    assert!(!lines[0].contains("disabled"));
    assert!(lines[0].contains("ordinal 4"));
    assert!(lines[0].contains("reserved 00 00"));
    assert!(lines[1].contains("parameter 000007 prefix 3f float 3f800000 no range"));
    assert!(lines[2].contains("opaque parameter 4 bytes"));
    assert!(lines[3].contains("opaque parameter 16 bytes"));
    assert!(lines[4].contains("Cab slot 00030001 model No Cabinet 0100000f"));
    assert!(lines[4].contains("disabled"));
    assert!(lines[5].contains("Wah slot 00020002 model Facial Fuzz 02050000"));
    assert!(lines[6].contains("no GPXT role slot 000300aa model 33330004"));
    assert!(!lines.iter().any(|line| line.contains("bypass")));
    tone.rename("Named").unwrap();
    assert_eq!(tone.amp_asset_name(), Some("Line 6 Clean"));
    assert_eq!(
        tone.models()[0].parameters,
        amp_parameters_after_rename(&original)
    );
    for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
        let name_at = tone_name_offset(&original);
        if !(name_at..name_at + 64).contains(&index) {
            assert_eq!(before, after, "byte {index} changed");
        }
    }
    assert!(
        tone.as_bytes()
            .windows(4)
            .any(|bytes| bytes == [0xaa, 0xbb, 0xcc, 0xdd])
    );
    let closed = PodTone::parse(custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: CAB_SLOT,
            model_id: 0x0100_000f,
            tail: &[0, 0, 0, 1],
            parameters: &[],
        }],
    ))
    .unwrap();
    assert_eq!(closed.cab_model_id(), None);
    assert_eq!(
        pod_assets::asset_name(closed.models()[0].model_id),
        Some("No Cabinet")
    );
    let partial = PodTone::parse(custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: 1,
            tail: &[0x11, 0x22, 0x33],
            parameters: &[],
        }],
    ))
    .unwrap();
    assert_eq!(partial.models()[0].ordinal, None);
    assert_eq!(partial.models()[0].reserved, None);
    assert_eq!(partial.models()[0].enabled, None);
    assert!(
        partial
            .as_bytes()
            .windows(3)
            .any(|bytes| bytes == [0x11, 0x22, 0x33])
    );
}

#[test]
fn signed_minf_fields_follow_java_read8_and_rename_keeps_the_bytes() {
    // Java `byte` values of the file bytes 0x00, 0x01, 0x7F, 0x80, and 0xFF.
    // Both the ordinal and the enabled byte use each value.
    const TAILS: [[u8; 4]; 5] = [
        [0x00, 0x5a, 0xa5, 0x00],
        [0x01, 0x5a, 0xa5, 0x01],
        [0x7f, 0x5a, 0xa5, 0x7f],
        [0x80, 0x5a, 0xa5, 0x80],
        [0xff, 0x5a, 0xa5, 0xff],
    ];
    let expected: [(u32, i8, bool); 5] = [
        (65_536, 0, false),
        (131_072, 1, true),
        (131_073, 127, true),
        (131_074, -128, false),
        (AMP_SLOT, -1, false),
    ];
    let models: Vec<BuiltModel<'_>> = expected
        .iter()
        .enumerate()
        .map(|(index, &(slot, _, _))| BuiltModel {
            slot,
            model_id: 0x1111_0002,
            tail: &TAILS[index],
            parameters: &[],
        })
        .collect();
    let original = custom_tone(ContainerKind::L6as, &models);
    let mut tone = PodTone::parse(original.clone()).unwrap();
    let lines = tone.control_lines();
    assert_eq!(lines.len(), expected.len());
    for (index, &(slot, ordinal, enabled)) in expected.iter().enumerate() {
        let model = &tone.models()[index];
        assert_eq!(model.slot, slot);
        assert_eq!(model.ordinal, Some(ordinal));
        assert_eq!(model.reserved, Some([0x5a, 0xa5]));
        assert_eq!(model.enabled, Some(enabled));
        assert!(lines[index].ends_with(&format!("ordinal {ordinal} reserved 5a a5")));
        if enabled {
            assert!(lines[index].contains(" enabled"));
            assert!(!lines[index].contains("disabled"));
        } else {
            assert!(lines[index].contains(" disabled"));
        }
    }
    tone.rename("Signed").unwrap();
    assert_eq!(tone.name(), "Signed");
    let name_at = tone_name_offset(&original);
    for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
        if !(name_at..name_at + 64).contains(&index) {
            assert_eq!(before, after, "byte {index} changed");
        }
    }
    for tail in TAILS {
        assert!(
            tone.as_bytes().windows(4).any(|bytes| bytes == tail),
            "MINF tail {tail:02x?} was not preserved"
        );
    }
}

fn amp_parameters_after_rename(original: &[u8]) -> Vec<ModelParameter> {
    PodTone::parse(original.to_vec()).unwrap().models()[0]
        .parameters
        .clone()
}

#[test]
fn library_rename_and_save_do_not_overwrite_or_cross_tones() {
    let mut library = Library::default();
    let first = library.import_bytes(live_tone()).unwrap();
    let second = library
        .import_bytes(fixture(
            ContainerKind::L6pa,
            PODXT_LIVE_TONE_ID,
            4,
            &[(AMP_SLOT, 9)],
        ))
        .unwrap();
    let mut edited = library.tone(first).unwrap().clone();
    edited.rename("Guitare 🎸").unwrap();
    library.replace(first, edited.into_bytes()).unwrap();
    assert!(library.is_dirty(first));
    assert!(!library.is_dirty(second));
    assert_eq!(library.tone(first).unwrap().name(), "Guitare 🎸");
    assert_eq!(library.tone(second).unwrap().amp_model_id(), Some(9));
    assert_eq!(library.tone(second).unwrap().name(), "A");
    assert!(edited_name_too_long(library.tone(first).unwrap()));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("copy.tone");
    library.tone(first).unwrap().save_copy(&path).unwrap();
    assert!(library.is_dirty(first));
    library.note_saved(first).unwrap();
    assert!(!library.is_dirty(first));
    assert!(!library.is_dirty(second));
    let saved = std::fs::read(&path).unwrap();
    assert_eq!(saved, library.tone(first).unwrap().as_bytes());
    assert!(library.tone(first).unwrap().save_copy(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    let mut other = library.tone(second).unwrap().clone();
    other.rename("Other").unwrap();
    library.replace(second, other.into_bytes()).unwrap();
    assert!(!library.is_dirty(first));
    assert!(library.is_dirty(second));
    library.revert_unsaved(second).unwrap();
    assert_eq!(library.tone(second).unwrap().name(), "A");
    assert!(!library.is_dirty(second));
    assert_eq!(library.tone(first).unwrap().name(), "Guitare 🎸");
    assert!(library.replace(7, live_tone()).is_err());
    assert!(library.note_saved(7).is_err());
    assert!(library.revert_unsaved(7).is_err());
}

#[test]
fn pod_bundle_library_admits_each_standalone_patch_form() {
    for kind in [
        ContainerKind::L6pa,
        ContainerKind::L6fs,
        ContainerKind::L6as,
    ] {
        let mut tone = drive_tone(PODXT_LIVE_HOST_MIN_VERSION, PLEXI_LEAD_100_MODEL, &[]);
        tone[8..12].copy_from_slice(kind.tag().as_bytes());
        let bundle = bundle_with_tone(&tone);
        let mut library = PodBundleLibrary::parse(bundle).unwrap();
        assert_eq!(library.tone(0, 0).unwrap().container(), kind);
        library
            .edit_tone(0, 0, |selected| selected.rename("Renamed"))
            .unwrap();
        let reopened = PodBundleLibrary::parse(library.as_bytes().to_vec()).unwrap();
        assert_eq!(reopened.tone(0, 0).unwrap().container(), kind);
        assert_eq!(reopened.tone(0, 0).unwrap().name(), "Renamed");
    }
}

#[test]
fn pod_bundle_library_edits_reverts_and_rejects_mixed_target_collections() {
    let first = drive_record(0x3f, 0x0010_0003, 1, 0x3e00_0000);
    let last = drive_record(0x3f, 0x0010_0003, 1, 0x3e80_0000);
    let opaque = [0xde, 0xad, 0xbe];
    let tone = custom_tone(
        ContainerKind::L6fs,
        &[
            BuiltModel {
                slot: AMP_SLOT,
                model_id: PLEXI_LEAD_100_MODEL,
                tail: &[0xa1, 0xb2, 0xc3, 0xd4],
                parameters: &[&first, &last],
            },
            BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x0200_0002,
                tail: &[0x11, 0x22, 0x33, 0x44],
                parameters: &[&opaque],
            },
        ],
    );
    let ambiguous = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: AMP_SLOT,
            model_id: PLEXI_LEAD_100_MODEL,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters: &[&first, &opaque],
        }],
    );
    let third_parm = drive_record(0x3f, 0x0010_0003, 1, 0x3e00_0000);
    let unselected = drive_tone(
        PODXT_LIVE_HOST_MIN_VERSION,
        PLEXI_LEAD_100_MODEL,
        &[&third_parm],
    );
    let original = bundle_with_banks(&[
        vec![tone.clone(), ambiguous.clone()],
        vec![unselected.clone()],
    ]);
    let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
    assert_eq!(library.bank_count(), 2);
    assert_eq!(library.preset_count(0), Some(2));
    assert_eq!(library.preset_count(1), Some(1));
    assert_eq!(library.bank_name(0), Some("A"));
    assert_eq!(library.bank_name(1), Some("B"));
    assert_eq!(library.tone(0, 0).unwrap().as_bytes(), tone);
    assert!(!library.is_dirty());

    let mut expected = original.clone();
    let tone_start = expected
        .windows(tone.len())
        .position(|window| window == tone)
        .unwrap();
    let parameter_positions: Vec<_> = tone
        .windows(last.len())
        .enumerate()
        .filter_map(|(index, window)| (window == last).then_some(index))
        .collect();
    let value_offset = parameter_positions.last().copied().unwrap() + 8;
    expected[tone_start + value_offset..tone_start + value_offset + 4]
        .copy_from_slice(&0x3f00_0000u32.to_be_bytes());
    library
        .edit_tone(0, 0, |selected| {
            selected.set_amp_control_fraction(AmpControl::Drive, 0.5)
        })
        .unwrap();
    assert_eq!(library.as_bytes(), expected);
    assert!(library.is_dirty());
    assert_eq!(library.tone(1, 0).unwrap().as_bytes(), unselected);
    assert!(
        library
            .tone(0, 0)
            .unwrap()
            .as_bytes()
            .windows(opaque.len())
            .any(|window| window == opaque)
    );
    assert_eq!(
        library.tone(0, 0).unwrap().as_bytes()[value_offset..value_offset + 4],
        0x3f00_0000u32.to_be_bytes()
    );
    let before_failed_edit = library.as_bytes().to_vec();
    assert!(
        library
            .edit_tone(0, 1, |selected| selected
                .set_amp_control_fraction(AmpControl::Drive, 0.5))
            .is_err()
    );
    assert_eq!(library.as_bytes(), before_failed_edit);
    library.revert().unwrap();
    assert_eq!(library.as_bytes(), original);
    assert!(!library.is_dirty());
    assert!(library.edit_tone(2, 0, |_| Ok(())).is_err());

    library
        .edit_tone(0, 0, |selected| {
            selected.set_amp_control_fraction(AmpControl::Drive, 0.5)
        })
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("pod-bank.vwb");
    library.save_copy(&copy).unwrap();
    let saved_copy = std::fs::read(&copy).unwrap();
    assert_eq!(saved_copy, library.as_bytes());
    assert!(!PodBundleLibrary::parse(saved_copy).unwrap().is_dirty());
    assert!(
        library.is_dirty(),
        "copying does not change the source baseline"
    );
    assert!(
        library.save_copy(&copy).is_err(),
        "save copy refuses overwrite"
    );

    let guitar = include_bytes!("../../../../tests/fixtures/editor-preset.vwm").to_vec();
    let mixed = bundle_with_banks(&[vec![tone.clone()], vec![guitar]]);
    assert!(matches!(
        PodBundleLibrary::parse(mixed),
        Err(ToneError::WrongTarget { device_id }) if device_id != PODXT_LIVE_TONE_ID
    ));
    assert!(PodBundleLibrary::parse(bundle_with_banks(&[vec![]])).is_err());
    let mut malformed = original;
    malformed[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(PodBundleLibrary::parse(malformed).is_err());
}

#[test]
fn pod_membership_edits_splice_only_admitted_folder_tone_spans_transactionally() {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_vec();
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            bytes.push(0xa5);
        }
        bytes
    }

    let fixed = live_tone();
    let first = live_tone();
    let second = tone_with_empty_unknown_chunk(&live_tone());
    let before = chunk(b"BEFR", &[0x11, 0x22, 0x33]);
    let middle = chunk(b"MIDD", &[0x44, 0x55, 0x66]);
    let after = chunk(b"AFTR", &[0x77, 0x88, 0x99]);
    let original_children = vec![
        before.clone(),
        first.clone(),
        middle.clone(),
        second.clone(),
        after.clone(),
    ];
    let original = membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &original_children);
    let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
    assert_eq!(library.bank_count(), 3);
    assert!(!library.membership_editable(0));
    assert!(library.membership_editable(1));
    assert!(library.membership_editable(2));

    let duplicate_index = library.duplicate_entry(1, 0).unwrap();
    let duplicated_children = vec![
        before.clone(),
        first.clone(),
        first.clone(),
        middle.clone(),
        second.clone(),
        after.clone(),
    ];
    let duplicated = membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &duplicated_children);
    assert_eq!(library.as_bytes(), duplicated);
    assert_eq!(duplicate_index, 2);
    assert_eq!(library.entry_location(duplicate_index), Some((1, 1)));
    assert_eq!(library.tone(1, 1).unwrap().as_bytes(), first);
    assert!(library.is_dirty());
    let reopened = PodBundleLibrary::parse(library.as_bytes().to_vec()).unwrap();
    assert_eq!(reopened.as_bytes(), duplicated);

    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("membership-copy.l6c");
    library.save_copy(&copy).unwrap();
    assert_eq!(std::fs::read(&copy).unwrap(), duplicated);
    assert!(library.is_dirty(), "save-copy keeps the existing baseline");
    assert!(
        library.save_copy(&copy).is_err(),
        "save-copy refuses overwrite"
    );
    library.revert().unwrap();
    assert_eq!(library.as_bytes(), original);
    assert!(!library.is_dirty());

    let selected = library.reorder_entry(1, 0, 1).unwrap();
    let reordered_children = vec![
        before.clone(),
        second.clone(),
        middle.clone(),
        first.clone(),
        after.clone(),
    ];
    let reordered = membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &reordered_children);
    assert_eq!(library.as_bytes(), reordered);
    assert_eq!(selected, 2);
    assert_eq!(library.tone(1, 1).unwrap().as_bytes(), first);
    let unchanged = library.as_bytes().to_vec();
    assert_eq!(library.reorder_entry(1, 1, 1).unwrap(), 2);
    assert_eq!(
        library.as_bytes(),
        unchanged,
        "same-slot reorder is a no-op"
    );

    let selected = library.remove_entry(1, 0).unwrap();
    let removed_children = vec![before.clone(), middle.clone(), first.clone(), after.clone()];
    let removed = membership_bundle(b"L6CO", std::slice::from_ref(&fixed), &removed_children);
    assert_eq!(library.as_bytes(), removed);
    assert_eq!(selected, 1);
    assert_eq!(library.tone(1, 0).unwrap().as_bytes(), first);
    let empty_folder_children = vec![before, middle, after];
    let empty_folder = membership_bundle(
        b"L6CO",
        std::slice::from_ref(&fixed),
        &empty_folder_children,
    );
    library.remove_entry(1, 0).unwrap();
    assert_eq!(library.as_bytes(), empty_folder);
    assert_eq!(library.preset_count(1), Some(0));
    assert_eq!(
        PodBundleLibrary::parse(empty_folder.clone())
            .unwrap()
            .as_bytes(),
        empty_folder
    );

    let unchanged = library.as_bytes().to_vec();
    assert!(library.duplicate_entry(1, 0).is_err());
    assert!(library.remove_entry(0, 0).is_err());
    assert!(library.reorder_entry(1, 0, 1).is_err());
    assert_eq!(library.as_bytes(), unchanged);

    let fixed_root = membership_bundle(
        b"L6BA",
        std::slice::from_ref(&fixed),
        std::slice::from_ref(&second),
    );
    let mut fixed_library = PodBundleLibrary::parse(fixed_root.clone()).unwrap();
    assert!(!fixed_library.membership_editable(1));
    assert!(fixed_library.duplicate_entry(1, 0).is_err());
    assert_eq!(fixed_library.as_bytes(), fixed_root);

    let only_tone = membership_bundle(b"L6CO", &[], std::slice::from_ref(&second));
    let mut last_tone = PodBundleLibrary::parse(only_tone.clone()).unwrap();
    assert!(last_tone.remove_entry(1, 0).is_err());
    assert_eq!(last_tone.as_bytes(), only_tone);
    assert!(!last_tone.is_dirty());

    let base = membership_bundle(
        b"L6CO",
        std::slice::from_ref(&fixed),
        std::slice::from_ref(&second),
    );
    let filler_len = BUNDLE_BYTES_CEILING as usize - base.len() - 8;
    assert_eq!(filler_len % 2, 0);
    let filler = chunk(b"FILL", &vec![0x6d; filler_len]);
    let capped = membership_bundle(b"L6CO", &[live_tone()], &[second, filler]);
    assert_eq!(capped.len() as u64, BUNDLE_BYTES_CEILING);
    let mut capped_library = PodBundleLibrary::parse(capped.clone()).unwrap();
    assert!(capped_library.duplicate_entry(1, 0).is_err());
    assert_eq!(capped_library.as_bytes(), capped);
    assert!(!capped_library.is_dirty());
}

#[test]
fn fixed_bank_tone_reorder_preserves_raw_spans_tokens_lengths_and_baseline() {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = id.to_vec();
        bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
        bytes.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            bytes.push(0xa5);
        }
        bytes
    }
    fn tone(name: u16, extra_chunks: usize) -> Vec<u8> {
        let mut bytes = fixture(
            ContainerKind::L6pa,
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
            &[(AMP_SLOT, 0x1111_0002)],
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 8..pinf + 10].copy_from_slice(&name.to_be_bytes());
        let junk = find_chunk_data(&bytes, b"JUNK");
        bytes[junk..junk + 3].copy_from_slice(&[
            name as u8,
            (name >> 8) as u8,
            (name as u8) ^ 0x5a,
        ]);
        for _ in 0..extra_chunks {
            bytes.extend_from_slice(&chunk(b"EXTR", &[name as u8; 3]));
        }
        let form_length = bytes.len() - 8;
        bytes[4..8].copy_from_slice(&(form_length as u32).to_be_bytes());
        bytes
    }
    fn bank_length(bytes: &[u8]) -> u32 {
        let kind = bytes
            .windows(4)
            .position(|window| window == b"BANK")
            .expect("fixed BANK type");
        u32::from_be_bytes(bytes[kind - 4..kind].try_into().unwrap())
    }

    let first = tone(u16::from(b'A'), 0);
    let second = tone(u16::from(b'B'), 1);
    let third = tone(u16::from(b'C'), 2);
    assert_ne!(first.len(), second.len());
    assert_ne!(second.len(), third.len());
    let before = chunk(b"BEFR", &[0x11, 0x22, 0x33]);
    let middle_one = chunk(b"MID1", &[0x44, 0x55, 0x66]);
    let middle_two = chunk(b"MID2", &[0x77, 0x88, 0x99]);
    let after = chunk(b"AFTR", &[0xaa, 0xbb, 0xcc]);
    let other_bank_tone = tone(u16::from(b'Z'), 0);
    let original_children = vec![
        before.clone(),
        first.clone(),
        middle_one.clone(),
        second.clone(),
        middle_two.clone(),
        third.clone(),
        after.clone(),
    ];
    let original = membership_bundle(
        b"L6BA",
        &original_children,
        std::slice::from_ref(&other_bank_tone),
    );
    let expected_children = vec![
        before.clone(),
        second.clone(),
        middle_one.clone(),
        third.clone(),
        middle_two.clone(),
        first.clone(),
        after.clone(),
    ];
    let expected = membership_bundle(
        b"L6BA",
        &expected_children,
        std::slice::from_ref(&other_bank_tone),
    );
    let root_length = preset::word(&original, 4);
    let fixed_bank_length = bank_length(&original);
    let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
    assert!(library.fixed_bank_reorder_editable(0));
    assert!(!library.fixed_bank_reorder_editable(1));
    assert!(!library.is_dirty());

    let unchanged = library.as_bytes().to_vec();
    assert_eq!(library.reorder_fixed_bank_entry(0, 1, 1).unwrap(), 1);
    assert_eq!(library.as_bytes(), unchanged);
    assert!(!library.is_dirty(), "same-slot move is a clean byte no-op");

    assert_eq!(library.reorder_fixed_bank_entry(0, 0, 2).unwrap(), 2);
    assert_eq!(library.as_bytes(), expected);
    assert_eq!(preset::word(library.as_bytes(), 4), root_length);
    assert_eq!(bank_length(library.as_bytes()), fixed_bank_length);
    assert!(library.is_dirty());
    assert_eq!(library.preset_count(0), Some(3));
    assert_eq!(library.tone(0, 0).unwrap().as_bytes(), second);
    assert_eq!(library.tone(0, 1).unwrap().as_bytes(), third);
    assert_eq!(library.tone(0, 2).unwrap().as_bytes(), first);
    assert_eq!(library.tone(1, 0).unwrap().as_bytes(), other_bank_tone);
    let reopened = PodBundleLibrary::parse(library.as_bytes().to_vec()).unwrap();
    assert_eq!(reopened.as_bytes(), expected);
    assert_eq!(reopened.tone(0, 0).unwrap().as_bytes(), second);
    assert_eq!(reopened.tone(0, 1).unwrap().as_bytes(), third);
    assert_eq!(reopened.tone(0, 2).unwrap().as_bytes(), first);

    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("fixed-bank.vwb");
    library.save_copy(&copy).unwrap();
    let saved_copy = std::fs::read(&copy).unwrap();
    assert_eq!(saved_copy, expected);
    assert_eq!(
        PodBundleLibrary::parse(saved_copy)
            .unwrap()
            .tone(0, 0)
            .unwrap()
            .as_bytes(),
        second
    );
    assert!(library.save_copy(&copy).is_err());
    assert_eq!(std::fs::read(&copy).unwrap(), expected);
    assert!(library.is_dirty(), "saving a copy does not change baseline");

    assert_eq!(library.reorder_fixed_bank_entry(0, 1, 1).unwrap(), 1);
    assert_eq!(library.as_bytes(), expected);
    assert!(
        library.is_dirty(),
        "no-op preserves an existing dirty state"
    );
    assert_eq!(library.reorder_fixed_bank_entry(0, 2, 0).unwrap(), 0);
    assert_eq!(library.as_bytes(), original);
    assert!(!library.is_dirty(), "returning to saved bytes is clean");

    let before_rejections = library.as_bytes().to_vec();
    assert!(library.reorder_fixed_bank_entry(0, 0, 3).is_err());
    assert!(library.reorder_fixed_bank_entry(99, 0, 0).is_err());
    assert_eq!(library.as_bytes(), before_rejections);
    assert!(!library.is_dirty());
}

#[test]
fn fixed_bank_tone_reorder_rejects_other_roots_banks_and_patch_types_atomically() {
    let tone = || {
        fixture(
            ContainerKind::L6pa,
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
            &[(AMP_SLOT, 0x1111_0002)],
        )
    };
    let alternate = fixture(
        ContainerKind::L6fs,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(AMP_SLOT, 0x1111_0002)],
    );
    let amp_setup = live_tone();
    let live = tone();

    let mixed_root_child = membership_bundle(
        b"L6BA",
        std::slice::from_ref(&live),
        std::slice::from_ref(&live),
    );
    let mut mixed_root = PodBundleLibrary::parse(mixed_root_child).unwrap();
    let original = mixed_root.as_bytes().to_vec();
    assert!(!mixed_root.fixed_bank_reorder_editable(1));
    assert!(mixed_root.reorder_fixed_bank_entry(1, 0, 0).is_err());
    assert_eq!(mixed_root.as_bytes(), original);

    let collection_bank = membership_bundle(
        b"L6CO",
        std::slice::from_ref(&live),
        std::slice::from_ref(&live),
    );
    let mut collection = PodBundleLibrary::parse(collection_bank).unwrap();
    let original = collection.as_bytes().to_vec();
    assert!(!collection.fixed_bank_reorder_editable(0));
    assert!(collection.reorder_fixed_bank_entry(0, 0, 0).is_err());
    assert_eq!(collection.as_bytes(), original);

    let empty_fixed = membership_bundle(b"L6BA", &[], std::slice::from_ref(&live));
    let mut empty = PodBundleLibrary::parse(empty_fixed).unwrap();
    let original = empty.as_bytes().to_vec();
    assert!(!empty.fixed_bank_reorder_editable(0));
    assert!(empty.reorder_fixed_bank_entry(0, 0, 0).is_err());
    assert_eq!(empty.as_bytes(), original);

    for unsupported in [alternate, amp_setup] {
        let mixed = membership_bundle(b"L6BA", &[live.clone(), unsupported], &[]);
        let mut library = PodBundleLibrary::parse(mixed).unwrap();
        let original = library.as_bytes().to_vec();
        assert!(!library.fixed_bank_reorder_editable(0));
        assert!(library.reorder_fixed_bank_entry(0, 0, 1).is_err());
        assert_eq!(library.as_bytes(), original);
        assert!(!library.is_dirty());
    }

    let mut wrong_target = live.clone();
    let pinf = find_chunk_data(&wrong_target, b"PINF");
    wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000b_u32.to_be_bytes());
    let wrong_target_bundle =
        membership_bundle(b"L6BA", &[wrong_target], std::slice::from_ref(&live));
    assert!(PodBundleLibrary::parse(wrong_target_bundle).is_err());
}

#[test]
fn zero_length_nonregular_facts_are_not_read() {
    let mut called = false;
    let error = read_regular_tone(
        Ok(PathFacts {
            regular_file: false,
            len: 0,
        }),
        || {
            called = true;
            Ok(Cursor::new(vec![1]))
        },
    )
    .unwrap_err();
    assert!(!called);
    assert!(error.detail().contains("regular file"));
    assert!(error.detail().contains("not opened"));
    let mut called = false;
    let error = read_regular_tone(
        Ok(PathFacts {
            regular_file: true,
            len: TONE_BYTES_CEILING + 1,
        }),
        || {
            called = true;
            Ok(Cursor::new(vec![1]))
        },
    )
    .unwrap_err();
    assert!(!called);
    assert!(error.detail().contains("ceiling"));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("not-a-tone");
    std::fs::create_dir(&path).unwrap();
    assert!(open_regular_file(&path).is_err());
    let error = read_regular_tone(path_facts(&path), || -> io::Result<Cursor<Vec<u8>>> {
        panic!("directory must not be read")
    })
    .unwrap_err();
    assert!(error.detail().contains("regular file"));
}

#[test]
fn pod_bundle_reader_checks_regular_path_and_16_mib_ceiling_before_read() {
    let mut called = false;
    assert!(
        read_regular_bundle(
            Ok(PathFacts {
                regular_file: false,
                len: 0,
            }),
            || {
                called = true;
                Ok((
                    PathFacts {
                        regular_file: true,
                        len: 0,
                    },
                    std::io::Cursor::new(vec![]),
                ))
            },
        )
        .is_err()
    );
    assert!(!called);
    assert!(
        read_regular_bundle(
            Ok(PathFacts {
                regular_file: true,
                len: BUNDLE_BYTES_CEILING + 1,
            }),
            || {
                called = true;
                Ok((
                    PathFacts {
                        regular_file: true,
                        len: 0,
                    },
                    std::io::Cursor::new(vec![]),
                ))
            },
        )
        .is_err()
    );
    assert!(!called);

    let error = read_regular_bundle(
        Ok(PathFacts {
            regular_file: true,
            len: 1,
        }),
        || {
            Ok((
                PathFacts {
                    regular_file: false,
                    len: 0,
                },
                std::io::Cursor::new(vec![]),
            ))
        },
    )
    .unwrap_err();
    assert!(error.detail().contains("regular file"));
    assert!(error.detail().contains("not read"));

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bounded-bank.vwb");
    std::fs::write(&path, b"offline contents").unwrap();
    let actual = read_regular_bundle(path_facts(&path), || {
        let file = std::fs::File::open(&path)?;
        let metadata = file.metadata()?;
        Ok((
            PathFacts {
                regular_file: metadata.is_file(),
                len: metadata.len(),
            },
            file,
        ))
    })
    .unwrap();
    assert_eq!(actual, b"offline contents");
}

#[test]
fn pod_bundle_reader_stops_at_ceiling_plus_one_when_file_grows() {
    struct GrowingReader {
        remaining: u64,
        consumed: std::rc::Rc<std::cell::Cell<u64>>,
    }

    impl std::io::Read for GrowingReader {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let count = output.len().min(self.remaining as usize);
            output[..count].fill(0x5a);
            self.remaining -= count as u64;
            self.consumed.set(self.consumed.get() + count as u64);
            Ok(count)
        }
    }

    let consumed = std::rc::Rc::new(std::cell::Cell::new(0));
    let reader_consumed = consumed.clone();
    let error = read_regular_bundle(
        Ok(PathFacts {
            regular_file: true,
            len: 12,
        }),
        move || {
            Ok((
                PathFacts {
                    regular_file: true,
                    len: 12,
                },
                GrowingReader {
                    remaining: BUNDLE_BYTES_CEILING * 2,
                    consumed: reader_consumed,
                },
            ))
        },
    )
    .unwrap_err();
    assert!(error.detail().contains("ceiling"));
    assert_eq!(consumed.get(), BUNDLE_BYTES_CEILING + 1);
}

#[test]
fn pod_bundle_parse_rejects_structurally_valid_file_over_total_ceiling() {
    let oversized = bundle_with_total_length(&live_tone(), BUNDLE_BYTES_CEILING as usize + 2);
    assert_eq!(oversized.len(), BUNDLE_BYTES_CEILING as usize + 2);
    let error = PodBundleLibrary::parse(oversized).unwrap_err();
    assert!(error.detail().contains("16 MiB"));
}

#[test]
fn pod_bundle_replacement_growth_over_ceiling_is_transactional() {
    let original = bundle_with_total_length(&live_tone(), BUNDLE_BYTES_CEILING as usize - 2);
    let mut library = PodBundleLibrary::parse(original.clone()).unwrap();
    library
        .edit_entry(0, |tone| tone.rename("Unsaved"))
        .unwrap();
    let before_replace = library.as_bytes().to_vec();
    let saved_before = library.saved.clone();
    assert!(library.is_dirty());
    assert_eq!(library.entry_location(0), Some((0, 0)));

    let replacement = tone_with_empty_unknown_chunk(&live_tone());
    let error = library.replace_entry(0, replacement).unwrap_err();
    assert!(error.detail().contains("16 MiB"));
    assert_eq!(library.as_bytes(), before_replace);
    assert_eq!(library.saved, saved_before);
    assert!(library.is_dirty());
    assert_eq!(library.entry_location(0), Some((0, 0)));
    assert_eq!(library.tone(0, 0).unwrap().name(), "Unsaved");
}

#[test]
fn delay_note_choices_match_source_rounding_at_endpoints_and_interior_tempo() {
    let expected = [
        (0x3f80_0000, 0x3f80_0000, 0x3efd_6a06),
        (0x3f80_0000, 0x3f3f_5a81, 0x3ebc_c486),
        (0x3f80_0000, 0x3efd_6a06, 0x3e78_3e11),
        (0x3f80_0000, 0x3ea7_3807, 0x3e22_0c12),
        (0x3f80_0000, 0x3ebc_c486, 0x3e37_9891),
        (0x3f80_0000, 0x3e78_3e11, 0x3ded_e625),
        (0x3f29_ce02, 0x3e22_0c12, 0x3d97_b426),
        (0x3f3f_5a81, 0x3e37_9891, 0x3dad_40a5),
        (0x3efd_6a06, 0x3ded_e625, 0x3d59_364f),
        (0x3ea7_3807, 0x3d97_b426, 0x3d03_0450),
        (0x3ebc_c486, 0x3dad_40a5, 0x3d18_90cf),
        (0x3e78_3e11, 0x3d59_364f, 0x3caf_d6a1),
        (0x3e22_0c12, 0x3d03_0450, 0x3c33_4945),
    ];
    let tempos = [30.0_f32, 120.0_f32, 240.0_f32];
    for (index, choice) in DelayNoteChoice::ALL.into_iter().enumerate().skip(1) {
        for (tempo_index, tempo) in tempos.into_iter().enumerate() {
            let mut tone = PodTone::parse(delay_note_fixture(
                DELAY_SLOT,
                0x0202_0002,
                0,
                0x3e80_0000,
                Some(MOD_SLOT),
                MOD_CONTROL_MODEL_IDS[0],
                0,
                Some(tempo),
            ))
            .unwrap();
            tone.set_delay_note_choice(choice).unwrap();
            assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0020_0000), index as u32);
            let expected_bits = match tempo_index {
                0 => expected[index - 1].0,
                1 => expected[index - 1].1,
                _ => expected[index - 1].2,
            };
            assert_eq!(
                parm_bits(&tone, DELAY_SLOT, 0x0010_0000),
                expected_bits,
                "NOTE {index}, BPM {tempo}"
            );
        }
    }
}

#[test]
fn mod_note_speed_source_events_match_independent_literal_words() {
    let original = delay_note_fixture(
        DELAY_SLOT,
        DELAY_CONTROL_MODEL_IDS[0],
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        Some(120.0),
    );
    let mut tone = PodTone::parse(original.clone()).unwrap();
    tone.set_mod_note_choice(ModNoteChoice::Quarter).unwrap();
    let mut expected = original.clone();
    patch_expected_parameter(&tone, &mut expected, MOD_SLOT, 0x0020_0000, 6);
    patch_expected_parameter(&tone, &mut expected, MOD_SLOT, 0x0010_0000, 0x3e02_93c2);
    assert_eq!(tone.as_bytes(), expected);
    assert_eq!(
        tone.mod_note_value().unwrap(),
        ModNoteValue::Known(ModNoteChoice::Quarter)
    );

    let mut manual = PodTone::parse(delay_note_fixture(
        DELAY_SLOT,
        DELAY_CONTROL_MODEL_IDS[0],
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        6,
        None,
    ))
    .unwrap();
    let before_manual = manual.as_bytes().to_vec();
    manual.set_mod_speed_hz(3.0).unwrap();
    let mut expected_manual = before_manual;
    patch_expected_parameter(
        &manual,
        &mut expected_manual,
        MOD_SLOT,
        0x0010_0000,
        0x3e47_4d51,
    );
    patch_expected_parameter(&manual, &mut expected_manual, MOD_SLOT, 0x0020_0000, 0);
    assert_eq!(manual.as_bytes(), expected_manual);
}

#[test]
fn mod_note_expected_speed_words_cover_every_choice_and_tempo_boundary() {
    let expected = [
        [
            0x3adb_eb61,
            0x3b92_9ced,
            0x3c24_f08a,
            0x3c97_31d4,
            0x3c80_494f,
            0x3cdb_eb62,
            0x3d32_af40,
            0x3d1b_c6bb,
            0x3d77_68cf,
            0x3dc0_6df6,
            0x3da9_8571,
            0x3e02_93c2,
            0x3e47_4d51,
        ],
        [
            0x3cdb_eb62,
            0x3d1b_c6bb,
            0x3d77_68cf,
            0x3dc0_6df6,
            0x3da9_8571,
            0x3e02_93c2,
            0x3e47_4d51,
            0x3e30_64cd,
            0x3e86_0370,
            0x3eca_bcff,
            0x3eb3_d47a,
            0x3f07_bb47,
            0x3f4c_74d5,
        ],
        [
            0x3d77_68cf,
            0x3da9_8571,
            0x3e02_93c2,
            0x3e47_4d51,
            0x3e30_64cd,
            0x3e86_0370,
            0x3eca_bcff,
            0x3eb3_d47a,
            0x3f07_bb47,
            0x3f4c_74d5,
            0x3f35_8c51,
            0x3f80_0000,
            0x3f80_0000,
        ],
    ];
    for (tempo_index, tempo) in [30.0_f32, 120.0, 240.0].into_iter().enumerate() {
        for (choice_index, choice) in ModNoteChoice::ALL.into_iter().enumerate().skip(1) {
            let bytes = delay_note_fixture(
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                0,
                0x3e80_0000,
                Some(MOD_SLOT),
                MOD_CONTROL_MODEL_IDS[0],
                0,
                Some(tempo),
            );
            let mut tone = PodTone::parse(bytes).unwrap();
            tone.set_mod_note_choice(choice).unwrap();
            assert_eq!(
                parm_bits(&tone, MOD_SLOT, 0x0010_0000),
                expected[tempo_index][choice_index - 1],
                "Mod NOTE {choice_index} at {tempo} BPM"
            );
        }
    }
}

#[test]
fn mod_note_model_slots_and_manual_speed_lower_boundary_are_bounded() {
    for slot in [MOD_SLOT, 0x0005_0003] {
        for model_id in MOD_CONTROL_MODEL_IDS {
            for delay_slot in [DELAY_SLOT, 0x0005_0004] {
                let bytes = mod_operation_fixture(&[
                    (slot, model_id, mod_test_rows(0, 0x3e80_0000)),
                    (
                        delay_slot,
                        DELAY_CONTROL_MODEL_IDS[0],
                        mod_test_delay_rows(0),
                    ),
                    (0x0001_0000, 0x3e70_0000, mod_test_system_rows(120.0)),
                ]);
                let mut tone = PodTone::parse(bytes.clone()).unwrap();
                let mut expected = bytes;
                patch_expected_parameter(&tone, &mut expected, slot, 0x0020_0000, 6);
                patch_expected_parameter(&tone, &mut expected, slot, 0x0010_0000, 0x3e02_93c2);
                tone.set_mod_note_choice(ModNoteChoice::Quarter).unwrap();
                assert_eq!(
                    tone.as_bytes(),
                    expected,
                    "Mod model {model_id:08x}, slot {slot:08x}, Delay slot {delay_slot:08x}"
                );
            }
        }
    }
    let mut lower = PodTone::parse(delay_note_fixture(
        DELAY_SLOT,
        DELAY_CONTROL_MODEL_IDS[0],
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        None,
    ))
    .unwrap();
    lower.set_mod_speed_hz(0.1).unwrap();
    assert_eq!(parm_bits(&lower, MOD_SLOT, 0x0010_0000), 0x0000_0000);

    let mod_speed = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
    let mod_note = drive_record(0x3f, 0x0020_0000, 0, 6);
    let model = BuiltModel {
        slot: MOD_SLOT,
        model_id: MOD_CONTROL_MODEL_IDS[0],
        tail: &[1, 2, 3, 4],
        parameters: &[&mod_speed, &mod_note],
    };
    let bytes = custom_tone(ContainerKind::L6as, &[model]);
    let mut isolated_speed = PodTone::parse(bytes).unwrap();
    isolated_speed.set_mod_speed_hz(3.0).unwrap();
    assert_eq!(
        parm_bits(&isolated_speed, MOD_SLOT, 0x0010_0000),
        0x3e47_4d51
    );
    assert_eq!(parm_bits(&isolated_speed, MOD_SLOT, 0x0020_0000), 0);
}

#[test]
fn mod_note_off_isolated_and_positive_cross_effect_guard_is_atomic() {
    let mut off = PodTone::parse(delay_note_fixture(
        DELAY_SLOT,
        DELAY_CONTROL_MODEL_IDS[0],
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        6,
        None,
    ))
    .unwrap();
    let original = off.as_bytes().to_vec();
    off.set_mod_note_choice(ModNoteChoice::Off).unwrap();
    let mut expected = original;
    patch_expected_parameter(&off, &mut expected, MOD_SLOT, 0x0020_0000, 0);
    assert_eq!(off.as_bytes(), expected);

    let delay_time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
    let delay_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
    let invalid_speed_type = drive_record(0x3f, 0x0010_0000, 0, 0x3e80_0000).to_vec();
    let active_mod_note = drive_record(0x3f, 0x0020_0000, 0, 6).to_vec();
    let isolated_models = vec![
        (
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            vec![delay_time, delay_note],
        ),
        (
            MOD_SLOT,
            MOD_CONTROL_MODEL_IDS[0],
            vec![invalid_speed_type, active_mod_note],
        ),
    ];
    let mut no_speed_read = PodTone::parse(delay_context_fixture(
        &isolated_models,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
    ))
    .unwrap();
    let before = no_speed_read.as_bytes().to_vec();
    no_speed_read
        .set_mod_note_choice(ModNoteChoice::Off)
        .unwrap();
    let mut expected = before;
    patch_expected_parameter(&no_speed_read, &mut expected, MOD_SLOT, 0x0020_0000, 0);
    assert_eq!(no_speed_read.as_bytes(), expected);

    let mut guarded = PodTone::parse(delay_note_fixture(
        DELAY_SLOT,
        DELAY_CONTROL_MODEL_IDS[0],
        6,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        Some(120.0),
    ))
    .unwrap();
    let before = guarded.as_bytes().to_vec();
    assert!(guarded.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
    assert_eq!(guarded.as_bytes(), before);
}

#[test]
fn delay_positive_note_uses_source_default_system_id_only() {
    for system_id in [0x3e70_0000, 0x3e80_0000, 0xdead_beef] {
        let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits());
        let time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
        let note = drive_record(0x3f, 0x0020_0000, 0, 0);
        let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0);
        let models = vec![
            (
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                vec![time.to_vec(), note.to_vec()],
            ),
            (MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], vec![mod_note.to_vec()]),
            (0x0001_0000, system_id, vec![tempo.to_vec()]),
        ];
        let mut tone = PodTone::parse(delay_context_fixture(
            &models,
            PODXT_LIVE_TONE_ID,
            PODXT_LIVE_HOST_MIN_VERSION,
        ))
        .unwrap();
        let original = tone.as_bytes().to_vec();
        let result = tone.set_delay_note_choice(DelayNoteChoice::Quarter);
        if system_id == 0x3e70_0000 {
            assert!(result.is_ok());
            assert_ne!(tone.as_bytes(), original);
        } else {
            assert!(result.is_err());
            assert_eq!(tone.as_bytes(), original);
        }
    }
}

#[test]
fn mod_note_and_speed_rejections_are_transactional() {
    let tempo = || vec![drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec()];
    let delay_rows = |note| {
        vec![
            drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
            drive_record(0x3f, 0x0020_0000, 0, note).to_vec(),
        ]
    };
    let build = |mod_rows: Vec<Vec<u8>>,
                 delays: Vec<(u32, u32, Vec<Vec<u8>>)>,
                 systems: Vec<(u32, u32, Vec<Vec<u8>>)>| {
        let mut models = vec![(MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], mod_rows)];
        models.extend(delays);
        models.extend(systems);
        mod_operation_fixture(&models)
    };
    let good_delay = || vec![(DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(0))];
    let good_system = || vec![(0x0001_0000, 0x3e70_0000, tempo())];
    let speed = || drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
    let note = |prefix, kind, value| drive_record(prefix, 0x0020_0000, kind, value).to_vec();
    let reject_both = |bytes: Vec<u8>, label: &str| {
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err(),
            "{label} NOTE"
        );
        assert_eq!(tone.as_bytes(), before, "{label} NOTE");
        let mut tone = PodTone::parse(bytes).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_mod_speed_hz(3.0).is_err(), "{label} speed");
        assert_eq!(tone.as_bytes(), before, "{label} speed");
    };

    let mut malformed_mod = vec![
        ("missing NOTE", vec![speed()]),
        (
            "duplicate NOTE",
            vec![speed(), note(0x3f, 0, 0), note(0x3f, 0, 0)],
        ),
        ("wrong-type NOTE", vec![speed(), note(0x3f, 1, 0)]),
        ("noncanonical NOTE", vec![speed(), note(0x3e, 0, 0)]),
        ("unknown NOTE", vec![speed(), note(0x3f, 0, 14)]),
        ("missing SPEED", vec![note(0x3f, 0, 0)]),
        ("duplicate SPEED", vec![speed(), speed(), note(0x3f, 0, 0)]),
        (
            "wrong-type SPEED",
            vec![
                drive_record(0x3f, 0x0010_0000, 0, 0).to_vec(),
                note(0x3f, 0, 0),
            ],
        ),
        (
            "noncanonical SPEED",
            vec![
                drive_record(0x3e, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                note(0x3f, 0, 0),
            ],
        ),
        (
            "nonfinite SPEED",
            vec![
                drive_record(0x3f, 0x0010_0000, 1, f32::INFINITY.to_bits()).to_vec(),
                note(0x3f, 0, 0),
            ],
        ),
        (
            "out-of-range SPEED",
            vec![
                drive_record(0x3f, 0x0010_0000, 1, 1.01_f32.to_bits()).to_vec(),
                note(0x3f, 0, 0),
            ],
        ),
        (
            "opaque Mod PARM",
            vec![speed(), note(0x3f, 0, 0), vec![0x81, 0x82, 0x83]],
        ),
    ];
    for (label, rows) in malformed_mod.drain(..) {
        reject_both(build(rows, good_delay(), good_system()), label);
    }

    let missing_mod = build(vec![], good_delay(), good_system());
    let mut tone = PodTone::parse(missing_mod.clone()).unwrap();
    assert!(tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
    assert_eq!(tone.as_bytes(), missing_mod);
    let duplicate_mods = mod_operation_fixture(&[
        (
            MOD_SLOT,
            MOD_CONTROL_MODEL_IDS[0],
            vec![speed(), note(0x3f, 0, 0)],
        ),
        (
            0x0005_0003,
            MOD_CONTROL_MODEL_IDS[1],
            vec![speed(), note(0x3f, 0, 0)],
        ),
        (DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(0)),
        (0x0001_0000, 0x3e70_0000, tempo()),
    ]);
    reject_both(duplicate_mods.clone(), "duplicate Mod slots");
    let mut tone = PodTone::parse(duplicate_mods.clone()).unwrap();
    assert!(tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
    assert_eq!(tone.as_bytes(), duplicate_mods);

    for (label, rows) in [
        ("missing Delay", vec![]),
        (
            "active Delay NOTE",
            vec![(DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(6))],
        ),
        (
            "duplicate Delay slots",
            vec![
                (DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(0)),
                (0x0005_0004, DELAY_CONTROL_MODEL_IDS[1], delay_rows(0)),
            ],
        ),
        (
            "unknown Delay model",
            vec![(DELAY_SLOT, 0xdead_beef, delay_rows(0))],
        ),
        (
            "missing Delay NOTE",
            vec![(
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                vec![drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec()],
            )],
        ),
        (
            "wrong-type Delay NOTE",
            vec![(
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                vec![
                    drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                    note(0x3f, 1, 0),
                ],
            )],
        ),
        (
            "noncanonical Delay NOTE",
            vec![(
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                vec![
                    drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                    note(0x3e, 0, 0),
                ],
            )],
        ),
        (
            "duplicate Delay NOTE",
            vec![(
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                vec![
                    drive_record(0x3f, 0x0020_0000, 0, 0).to_vec(),
                    note(0x3f, 0, 0),
                ],
            )],
        ),
        (
            "unknown Delay NOTE",
            vec![(DELAY_SLOT, DELAY_CONTROL_MODEL_IDS[0], delay_rows(14))],
        ),
        (
            "opaque Delay PARM",
            vec![(
                DELAY_SLOT,
                DELAY_CONTROL_MODEL_IDS[0],
                vec![
                    drive_record(0x3f, 0x0020_0000, 0, 0).to_vec(),
                    vec![0x81, 0x82, 0x83],
                ],
            )],
        ),
    ] {
        let bytes = build(vec![speed(), note(0x3f, 0, 0)], rows, good_system());
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        for choice in [ModNoteChoice::Off, ModNoteChoice::Quarter] {
            let before = tone.as_bytes().to_vec();
            assert!(tone.set_mod_note_choice(choice).is_err(), "{label}");
            assert_eq!(tone.as_bytes(), before, "{label}");
        }
    }

    let mut invalid_systems = vec![
        ("missing System", vec![]),
        ("wrong System ID", vec![(0x0001_0000, 0x3e80_0000, tempo())]),
        (
            "unknown System ID",
            vec![(0x0001_0000, 0xdead_beef, tempo())],
        ),
        ("missing Tempo", vec![(0x0001_0000, 0x3e70_0000, vec![])]),
        (
            "duplicate Tempo",
            vec![(
                0x0001_0000,
                0x3e70_0000,
                vec![tempo()[0].clone(), tempo()[0].clone()],
            )],
        ),
        (
            "wrong-type Tempo",
            vec![(
                0x0001_0000,
                0x3e70_0000,
                vec![drive_record(0x3f, 0x0020_0001, 0, 120).to_vec()],
            )],
        ),
        (
            "noncanonical Tempo",
            vec![(
                0x0001_0000,
                0x3e70_0000,
                vec![drive_record(0x3e, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec()],
            )],
        ),
        (
            "low Tempo",
            vec![(
                0x0001_0000,
                0x3e70_0000,
                vec![drive_record(0x3f, 0x0020_0001, 1, 29.0_f32.to_bits()).to_vec()],
            )],
        ),
        (
            "high Tempo",
            vec![(
                0x0001_0000,
                0x3e70_0000,
                vec![drive_record(0x3f, 0x0020_0001, 1, 241.0_f32.to_bits()).to_vec()],
            )],
        ),
        (
            "nonfinite Tempo",
            vec![(
                0x0001_0000,
                0x3e70_0000,
                vec![drive_record(0x3f, 0x0020_0001, 1, f32::NAN.to_bits()).to_vec()],
            )],
        ),
    ];
    for (label, systems) in invalid_systems.drain(..) {
        let bytes = build(vec![speed(), note(0x3f, 0, 0)], good_delay(), systems);
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err(),
            "{label}"
        );
        assert_eq!(tone.as_bytes(), before, "{label}");
    }
    let duplicate_system = build(
        vec![speed(), note(0x3f, 0, 0)],
        good_delay(),
        vec![
            (0x0001_0000, 0x3e70_0000, tempo()),
            (0x0001_0000, 0x3e70_0000, tempo()),
        ],
    );
    assert!(PodTone::parse(duplicate_system).is_err());

    let already_matching = mod_operation_fixture(&[
        (
            MOD_SLOT,
            MOD_CONTROL_MODEL_IDS[0],
            mod_test_rows(6, 0x3e02_93c2),
        ),
        (
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            mod_test_delay_rows(0),
        ),
        (0x0001_0000, 0x3e80_0000, tempo()),
    ]);
    let mut tone = PodTone::parse(already_matching.clone()).unwrap();
    assert!(tone.set_mod_note_choice(ModNoteChoice::Quarter).is_err());
    assert_eq!(tone.as_bytes(), already_matching);

    let mut note_off_without_speed_or_tempo = PodTone::parse(mod_operation_fixture(&[
        (
            MOD_SLOT,
            MOD_CONTROL_MODEL_IDS[0],
            vec![drive_record(0x3f, 0x0020_0000, 0, 6).to_vec()],
        ),
        (
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            mod_test_delay_rows(0),
        ),
    ]))
    .unwrap();
    let before = note_off_without_speed_or_tempo.as_bytes().to_vec();
    let mut expected = before;
    patch_expected_parameter(
        &note_off_without_speed_or_tempo,
        &mut expected,
        MOD_SLOT,
        0x0020_0000,
        0,
    );
    note_off_without_speed_or_tempo
        .set_mod_note_choice(ModNoteChoice::Off)
        .unwrap();
    assert_eq!(note_off_without_speed_or_tempo.as_bytes(), expected);

    let mut same_speed_clears_note = PodTone::parse(mod_operation_fixture(&[
        (
            MOD_SLOT,
            MOD_CONTROL_MODEL_IDS[0],
            mod_test_rows(6, 0x3e02_93c2),
        ),
        (
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            mod_test_delay_rows(0),
        ),
    ]))
    .unwrap();
    let before = same_speed_clears_note.as_bytes().to_vec();
    let mut expected = before;
    patch_expected_parameter(
        &same_speed_clears_note,
        &mut expected,
        MOD_SLOT,
        0x0020_0000,
        0,
    );
    same_speed_clears_note.set_mod_speed_hz(2.0).unwrap();
    assert_eq!(same_speed_clears_note.as_bytes(), expected);

    for invalid_hz in [f32::NAN, f32::INFINITY, 0.099, 15.001] {
        let bytes = mod_operation_fixture(&[(
            MOD_SLOT,
            MOD_CONTROL_MODEL_IDS[0],
            mod_test_rows(0, 0x3e80_0000),
        )]);
        let mut tone = PodTone::parse(bytes.clone()).unwrap();
        assert!(tone.set_mod_speed_hz(invalid_hz).is_err());
        assert_eq!(tone.as_bytes(), bytes);
    }

    let invalid_speed_library = mod_operation_fixture(&[
        (
            MOD_SLOT,
            MOD_CONTROL_MODEL_IDS[0],
            mod_test_rows(0, 0x3e80_0000),
        ),
        (
            DELAY_SLOT,
            DELAY_CONTROL_MODEL_IDS[0],
            mod_test_delay_rows(0),
        ),
        (0x0001_0000, 0x3e80_0000, tempo()),
    ]);
    let mut library = Library::default();
    library.import_bytes(invalid_speed_library.clone()).unwrap();
    let mut edited = PodTone::parse(invalid_speed_library.clone()).unwrap();
    edited.rename("Unsaved baseline").unwrap();
    library.replace(0, edited.into_bytes()).unwrap();
    let bytes_before = library.tone(0).unwrap().as_bytes().to_vec();
    let saved_before = library.tones[0].saved.clone();
    assert!(library.is_dirty(0));
    assert!(
        library
            .set_mod_note_choice(0, ModNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(library.tone(0).unwrap().as_bytes(), bytes_before);
    assert_eq!(library.tones[0].saved, saved_before);
    assert!(library.is_dirty(0));

    let bundle_bytes = membership_bundle(b"L6CO", &[invalid_speed_library], &[]);
    let mut bundle = PodBundleLibrary::parse(bundle_bytes).unwrap();
    bundle
        .edit_entry(0, |tone| tone.rename("Unsaved bundle"))
        .unwrap();
    let bytes_before = bundle.as_bytes().to_vec();
    let saved_before = bundle.saved.clone();
    assert!(bundle.is_dirty());
    assert!(
        bundle
            .set_mod_note_choice(0, ModNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(bundle.as_bytes(), bytes_before);
    assert_eq!(bundle.saved, saved_before);
    assert!(bundle.is_dirty());
}

#[test]
fn delay_note_models_and_primary_fallback_slots_match_complete_expected_buffers() {
    let delay_time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
    let delay_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
    let delay_sentinel = drive_record(0x3f, 0x007a_bcde, 0x7f, 0xdead_beef).to_vec();
    let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
    let mod_sentinel = drive_record(0x3f, 0x007a_bcde, 0, 0xface_cafe).to_vec();
    let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec();
    let expected_time = [
        0x3f80_0000,
        0x3f3f_5a81,
        0x3efd_6a06,
        0x3ea7_3807,
        0x3ebc_c486,
        0x3e78_3e11,
        0x3e22_0c12,
        0x3e37_9891,
        0x3dede625,
        0x3d97_b426,
        0x3dad_40a5,
        0x3d59_364f,
        0x3d03_0450,
    ];

    for slot in [DELAY_SLOT, 0x0005_0004] {
        for model_id in DELAY_CONTROL_MODEL_IDS {
            let models = vec![
                (
                    slot,
                    model_id,
                    vec![
                        delay_time.clone(),
                        delay_note.clone(),
                        delay_sentinel.clone(),
                    ],
                ),
                (
                    MOD_SLOT,
                    MOD_CONTROL_MODEL_IDS[0],
                    vec![mod_note.clone(), mod_sentinel.clone()],
                ),
                (0x0001_0000, 0x3e70_0000, vec![tempo.clone()]),
            ];
            let original =
                delay_context_fixture(&models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION);
            for (choice_index, choice) in DelayNoteChoice::ALL.into_iter().enumerate().skip(1) {
                let mut tone = PodTone::parse(original.clone()).unwrap();
                let mut expected = original.clone();
                patch_expected_parameter(&tone, &mut expected, slot, 0x0020_0000, choice.value());
                patch_expected_parameter(
                    &tone,
                    &mut expected,
                    slot,
                    0x0010_0000,
                    expected_time[choice_index - 1],
                );
                tone.set_delay_note_choice(choice).unwrap();
                assert_eq!(
                    tone.as_bytes(),
                    expected,
                    "Delay model {model_id:08x}, slot {slot:08x}, choice {choice:?}"
                );
            }
        }
    }
}

#[test]
fn delay_note_context_rejections_preserve_full_bytes_baseline_and_dirty_state() {
    let dtime = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
    let dnote = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
    let mnote = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
    let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec();
    let delay_rows = vec![dtime.clone(), dnote.clone()];
    let normal_mod = vec![(MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], vec![mnote.clone()])];
    let normal_system = vec![(0x0001_0000, 0x3e70_0000, vec![tempo.clone()])];
    let make_tone = |delay_slot: u32,
                     delay_model: u32,
                     delay_rows: Vec<Vec<u8>>,
                     mods: Vec<(u32, u32, Vec<Vec<u8>>)>,
                     systems: Vec<(u32, u32, Vec<Vec<u8>>)>,
                     target: u32| {
        let mut models = vec![(delay_slot, delay_model, delay_rows)];
        models.extend(mods);
        models.extend(systems);
        delay_context_fixture(&models, target, PODXT_LIVE_HOST_MIN_VERSION)
    };

    let mut cases = vec![
        (
            "missing Mod model",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "unsupported Mod model",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![(MOD_SLOT, 0xdead_beef, vec![mnote.clone()])],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "duplicate Mod models",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![
                    normal_mod[0].clone(),
                    (0x0005_0003, MOD_CONTROL_MODEL_IDS[1], vec![mnote.clone()]),
                ],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "opaque Mod PARM context",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![(
                    MOD_SLOT,
                    MOD_CONTROL_MODEL_IDS[0],
                    vec![mnote.clone(), vec![1, 2, 3]],
                )],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "duplicate Mod NOTE rows",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![(
                    MOD_SLOT,
                    MOD_CONTROL_MODEL_IDS[0],
                    vec![mnote.clone(), mnote.clone()],
                )],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "wrong-type Mod NOTE",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![(
                    MOD_SLOT,
                    MOD_CONTROL_MODEL_IDS[0],
                    vec![drive_record(0x3f, 0x0020_0000, 1, 0).to_vec()],
                )],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "noncanonical Mod NOTE",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![(
                    MOD_SLOT,
                    MOD_CONTROL_MODEL_IDS[0],
                    vec![drive_record(0x3e, 0x0020_0000, 0, 0).to_vec()],
                )],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "positive Mod NOTE blocks even NOTE Off",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                vec![(
                    MOD_SLOT,
                    MOD_CONTROL_MODEL_IDS[0],
                    vec![drive_record(0x3f, 0x0020_0000, 0, 6).to_vec()],
                )],
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "missing System model",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "unsupported System model",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![(0x0001_0000, 0xdead_beef, vec![tempo.clone()])],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "opaque System PARM context",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![(0x0001_0000, 0x3e70_0000, vec![tempo.clone(), vec![4, 5, 6]])],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "duplicate Tempo rows",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![(0x0001_0000, 0x3e70_0000, vec![tempo.clone(), tempo.clone()])],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "wrong-type Tempo",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![(
                    0x0001_0000,
                    0x3e80_0000,
                    vec![drive_record(0x3f, 0x0020_0001, 0, 120).to_vec()],
                )],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "noncanonical Tempo",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![(
                    0x0001_0000,
                    0x3e80_0000,
                    vec![drive_record(0x3e, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec()],
                )],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "invalid Tempo range",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![(
                    0x0001_0000,
                    0x3e80_0000,
                    vec![drive_record(0x3f, 0x0020_0001, 1, 29.0_f32.to_bits()).to_vec()],
                )],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "nonfinite Tempo",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                delay_rows.clone(),
                normal_mod.clone(),
                vec![(
                    0x0001_0000,
                    0x3e80_0000,
                    vec![drive_record(0x3f, 0x0020_0001, 1, f32::NAN.to_bits()).to_vec()],
                )],
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "duplicate Delay TIME rows",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                vec![dtime.clone(), dtime.clone(), dnote.clone()],
                normal_mod.clone(),
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "missing Delay TIME row",
            make_tone(
                DELAY_SLOT,
                0x0202_0002,
                vec![dnote.clone()],
                normal_mod.clone(),
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
        (
            "unsupported Delay model",
            make_tone(
                DELAY_SLOT,
                0xdead_beef,
                delay_rows.clone(),
                normal_mod.clone(),
                normal_system.clone(),
                PODXT_LIVE_TONE_ID,
            ),
        ),
    ];
    cases.push(("old profile", {
        let mut bytes = make_tone(
            DELAY_SLOT,
            0x0202_0002,
            delay_rows.clone(),
            normal_mod.clone(),
            normal_system.clone(),
            PODXT_LIVE_TONE_ID,
        );
        let pinf = find_chunk_data(&bytes, b"PINF");
        bytes[pinf + 72..pinf + 76]
            .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
        bytes
    }));

    let duplicate_system = make_tone(
        DELAY_SLOT,
        0x0202_0002,
        delay_rows.clone(),
        normal_mod.clone(),
        vec![normal_system[0].clone(), normal_system[0].clone()],
        PODXT_LIVE_TONE_ID,
    );
    assert!(PodTone::parse(duplicate_system.clone()).is_err());
    assert!(
        Library::default()
            .import_bytes(duplicate_system.clone())
            .is_err()
    );
    let wrong_target = make_tone(
        DELAY_SLOT,
        0x0202_0002,
        delay_rows.clone(),
        normal_mod.clone(),
        normal_system.clone(),
        0x0007_0005,
    );
    assert!(PodTone::parse(wrong_target.clone()).is_err());
    assert!(
        Library::default()
            .import_bytes(wrong_target.clone())
            .is_err()
    );
    assert_eq!(
        wrong_target,
        make_tone(
            DELAY_SLOT,
            0x0202_0002,
            delay_rows.clone(),
            normal_mod.clone(),
            normal_system.clone(),
            0x0007_0005,
        )
    );
    for (label, original) in cases {
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_delay_note_choice(DelayNoteChoice::Quarter)
                .is_err(),
            "{label}"
        );
        assert_eq!(tone.as_bytes(), before, "{label} changed tone bytes");

        let mut library = Library::default();
        let index = library.import_bytes(original.clone()).unwrap();
        library.tones[index].tone.rename("Already dirty").unwrap();
        let dirty_before = library.tone(index).unwrap().as_bytes().to_vec();
        let saved_before = library.tones[index].saved.clone();
        assert!(library.is_dirty(index), "{label} dirty setup");
        assert!(
            library
                .set_delay_note_choice(index, DelayNoteChoice::Quarter)
                .is_err(),
            "{label}"
        );
        assert_eq!(
            library.tone(index).unwrap().as_bytes(),
            dirty_before,
            "{label} changed dirty bytes"
        );
        assert_eq!(
            library.tones[index].saved, saved_before,
            "{label} changed saved baseline"
        );
        assert!(library.is_dirty(index), "{label} cleared dirty state");
    }
}

#[test]
fn delay_note_noop_validates_and_time_same_value_still_clears_positive_note() {
    let time = drive_record(0x3f, 0x0010_0000, 1, 0x3e78_3e11).to_vec();
    let note = drive_record(0x3f, 0x0020_0000, 0, 6).to_vec();
    let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0).to_vec();
    let tempo = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits()).to_vec();
    let models = vec![
        (DELAY_SLOT, 0x0202_0002, vec![time.clone(), note.clone()]),
        (MOD_SLOT, MOD_CONTROL_MODEL_IDS[0], vec![mod_note]),
        (0x0001_0000, 0x3e70_0000, vec![tempo]),
    ];
    let original = delay_context_fixture(&models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION);
    let mut tone = PodTone::parse(original.clone()).unwrap();
    tone.set_delay_note_choice(DelayNoteChoice::Quarter)
        .unwrap();
    assert_eq!(
        tone.as_bytes(),
        original,
        "same-value NOTE must validate then no-op"
    );

    let same_display_time = drive_record(0x3f, 0x0010_0000, 1, 0x3f00_0000).to_vec();
    let models = vec![(DELAY_SLOT, 0x0202_0002, vec![same_display_time, note])];
    let original = delay_context_fixture(&models, PODXT_LIVE_TONE_ID, PODXT_LIVE_HOST_MIN_VERSION);
    let mut tone = PodTone::parse(original.clone()).unwrap();
    let mut expected = original;
    patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0020_0000, 0);
    tone.set_delay_time_ms(1010.0).unwrap();
    assert_eq!(
        tone.as_bytes(),
        expected,
        "same TIME must still clear positive NOTE"
    );
}

#[test]
fn delay_time_rejections_preserve_full_bytes_and_dirty_library_baseline() {
    let normal_time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000).to_vec();
    let normal_note = drive_record(0x3f, 0x0020_0000, 0, 6).to_vec();
    let cases = [
        (
            "duplicate TIME",
            vec![
                normal_time.clone(),
                normal_time.clone(),
                normal_note.clone(),
            ],
            PODXT_LIVE_TONE_ID,
        ),
        (
            "missing TIME",
            vec![normal_note.clone()],
            PODXT_LIVE_TONE_ID,
        ),
        (
            "wrong-type TIME",
            vec![
                drive_record(0x3f, 0x0010_0000, 0, 0x3e80_0000).to_vec(),
                normal_note.clone(),
            ],
            PODXT_LIVE_TONE_ID,
        ),
        (
            "noncanonical TIME",
            vec![
                drive_record(0x3e, 0x0010_0000, 1, 0x3e80_0000).to_vec(),
                normal_note.clone(),
            ],
            PODXT_LIVE_TONE_ID,
        ),
        (
            "unknown NOTE",
            vec![
                normal_time.clone(),
                drive_record(0x3f, 0x0020_0000, 0, 14).to_vec(),
            ],
            PODXT_LIVE_TONE_ID,
        ),
    ];

    for (label, delay_rows, target) in cases {
        let models = vec![(DELAY_SLOT, 0x0202_0002, delay_rows)];
        let original = delay_context_fixture(&models, target, PODXT_LIVE_HOST_MIN_VERSION);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_time_ms(1200.0).is_err(), "{label}");
        assert_eq!(tone.as_bytes(), before, "{label} changed tone bytes");

        let mut library = Library::default();
        let index = library.import_bytes(original).unwrap();
        library.tones[index].tone.rename("Already dirty").unwrap();
        let dirty_before = library.tone(index).unwrap().as_bytes().to_vec();
        let saved_before = library.tones[index].saved.clone();
        assert!(library.set_delay_time_ms(index, 1200.0).is_err(), "{label}");
        assert_eq!(
            library.tone(index).unwrap().as_bytes(),
            dirty_before,
            "{label}"
        );
        assert_eq!(library.tones[index].saved, saved_before, "{label}");
        assert!(library.is_dirty(index), "{label}");
    }
}

#[test]
fn delay_note_off_needs_no_time_or_tempo_and_preserves_time() {
    let note = drive_record(0x3f, 0x0020_0000, 0, 6);
    let mod_note = drive_record(0x3f, 0x0020_0000, 0, 0);
    let models = [
        BuiltModel {
            slot: DELAY_SLOT,
            model_id: 0x0202_0002,
            tail: &[1, 2, 3, 4],
            parameters: &[&note],
        },
        BuiltModel {
            slot: MOD_SLOT,
            model_id: MOD_CONTROL_MODEL_IDS[0],
            tail: &[5, 6, 7, 8],
            parameters: &[&mod_note],
        },
    ];
    let original = custom_tone(ContainerKind::L6as, &models);
    let mut tone = PodTone::parse(original.clone()).unwrap();
    let mut expected = original;
    patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0020_0000, 0);
    tone.set_delay_note_choice(DelayNoteChoice::Off).unwrap();
    assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0020_0000), 0);
    assert_eq!(parm_bits(&tone, MOD_SLOT, 0x0020_0000), 0);
    assert_eq!(tone.as_bytes(), expected);
    assert!(
        tone.models()
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .unwrap()
            .parameters
            .iter()
            .all(|p| !matches!(
                p,
                ModelParameter::Recorded {
                    id: 0x0010_0000,
                    ..
                }
            ))
    );
}

#[test]
fn delay_time_ms_isolated_edit_clears_note_and_uses_double_source_conversion() {
    let note = drive_record(0x3f, 0x0020_0000, 0, 6);
    let time = drive_record(0x3f, 0x0010_0000, 1, 0x3e80_0000);
    let models = [BuiltModel {
        slot: DELAY_SLOT,
        model_id: 0x0202_0002,
        tail: &[1, 2, 3, 4],
        parameters: &[&time, &note],
    }];
    let original = custom_tone(ContainerKind::L6as, &models);
    let mut tone = PodTone::parse(original.clone()).unwrap();
    let mut expected = original.clone();
    patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0010_0000, 0x3f00_0000);
    patch_expected_parameter(&tone, &mut expected, DELAY_SLOT, 0x0020_0000, 0);
    tone.set_delay_time_ms(1010.0).unwrap();
    assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0010_0000), 0x3f00_0000);
    assert_eq!(parm_bits(&tone, DELAY_SLOT, 0x0020_0000), 0);
    assert_eq!(tone.as_bytes(), expected);
    let untouched = PodTone::parse(original).unwrap();
    assert_eq!(tone.models()[0].model_id, untouched.models()[0].model_id);
    assert!(tone.set_delay_time_ms(f32::NAN).is_err());
    assert!(tone.set_delay_time_ms(2000.5).is_err());
}

#[test]
fn delay_note_and_time_edits_reject_bad_context_atomically() {
    let mut positive_mod = PodTone::parse(delay_note_fixture(
        DELAY_SLOT,
        0x0202_0002,
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        6,
        Some(120.0),
    ))
    .unwrap();
    let before = positive_mod.as_bytes().to_vec();
    assert!(
        positive_mod
            .set_delay_note_choice(DelayNoteChoice::Off)
            .is_err()
    );
    assert_eq!(positive_mod.as_bytes(), before);

    let mut missing_tempo = PodTone::parse(delay_note_fixture(
        DELAY_SLOT,
        0x0202_0002,
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        None,
    ))
    .unwrap();
    let before = missing_tempo.as_bytes().to_vec();
    assert!(
        missing_tempo
            .set_delay_note_choice(DelayNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(missing_tempo.as_bytes(), before);

    let mut bad_time = PodTone::parse(delay_note_fixture(
        DELAY_SLOT,
        0x0202_0002,
        0,
        0x7f80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        Some(120.0),
    ))
    .unwrap();
    let before = bad_time.as_bytes().to_vec();
    assert!(
        bad_time
            .set_delay_note_choice(DelayNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(bad_time.as_bytes(), before);
    assert!(bad_time.set_delay_time_ms(1010.0).is_err());
    assert_eq!(bad_time.as_bytes(), before);
}

#[test]
fn delay_note_rejects_ambiguous_duplicate_slots_and_rows() {
    let base = delay_note_fixture(
        DELAY_SLOT,
        0x0202_0002,
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        Some(120.0),
    );
    let parsed = PodTone::parse(base.clone()).unwrap();
    let delay = parsed
        .models()
        .iter()
        .find(|model| model.slot == DELAY_SLOT)
        .unwrap();
    let mod_model = parsed
        .models()
        .iter()
        .find(|model| model.slot == MOD_SLOT)
        .unwrap();
    let d_time = match delay.parameters[0] {
        ModelParameter::Recorded {
            prefix,
            id,
            value: ParameterValue::Float { bits },
        } => drive_record(prefix, id, 1, bits),
        _ => unreachable!(),
    };
    let d_note = match delay.parameters[1] {
        ModelParameter::Recorded {
            prefix,
            id,
            value: ParameterValue::Int { bits },
        } => drive_record(prefix, id, 0, bits),
        _ => unreachable!(),
    };
    let d_sentinel = match delay.parameters[2] {
        ModelParameter::Recorded {
            prefix,
            id,
            value: ParameterValue::Other { type_word, bits },
        } => drive_record(prefix, id, type_word, bits),
        _ => unreachable!(),
    };
    let m_speed = match mod_model.parameters[0] {
        ModelParameter::Recorded {
            prefix,
            id,
            value: ParameterValue::Float { bits },
        } => drive_record(prefix, id, 1, bits),
        _ => unreachable!(),
    };
    let m_note = match mod_model.parameters[1] {
        ModelParameter::Recorded {
            prefix,
            id,
            value: ParameterValue::Int { bits },
        } => drive_record(prefix, id, 0, bits),
        _ => unreachable!(),
    };
    let duplicate_delay_rows = [&d_time[..], &d_note[..], &d_note[..], &d_sentinel[..]];
    let normal_delay_rows = [&d_time[..], &d_note[..]];
    let mod_rows = [&m_speed[..], &m_note[..]];
    let tempo_row = drive_record(0x3f, 0x0020_0001, 1, 120.0_f32.to_bits());
    let tempo_rows = [&tempo_row[..]];
    let dup_delay = [
        BuiltModel {
            slot: DELAY_SLOT,
            model_id: 0x0202_0002,
            tail: &[1, 2, 3, 4],
            parameters: &duplicate_delay_rows,
        },
        BuiltModel {
            slot: MOD_SLOT,
            model_id: MOD_CONTROL_MODEL_IDS[0],
            tail: &[5, 6, 7, 8],
            parameters: &mod_rows,
        },
        BuiltModel {
            slot: 0x0001_0000,
            model_id: 0x3e70_0000,
            tail: &[9, 10, 11, 12],
            parameters: &tempo_rows,
        },
    ];
    let mut tone = PodTone::parse(custom_tone(ContainerKind::L6as, &dup_delay)).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(
        tone.set_delay_note_choice(DelayNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(tone.as_bytes(), before);

    let duplicate_slot_models = vec![
        BuiltModel {
            slot: DELAY_SLOT,
            model_id: 0x0202_0002,
            tail: &[1, 2, 3, 4],
            parameters: &normal_delay_rows,
        },
        BuiltModel {
            slot: 0x0005_0004,
            model_id: 0x0202_0002,
            tail: &[1, 2, 3, 4],
            parameters: &normal_delay_rows,
        },
        BuiltModel {
            slot: MOD_SLOT,
            model_id: MOD_CONTROL_MODEL_IDS[0],
            tail: &[5, 6, 7, 8],
            parameters: &mod_rows,
        },
        BuiltModel {
            slot: 0x0001_0000,
            model_id: 0x3e70_0000,
            tail: &[9, 10, 11, 12],
            parameters: &tempo_rows,
        },
    ];
    let mut tone =
        PodTone::parse(custom_tone(ContainerKind::L6as, &duplicate_slot_models)).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(
        tone.set_delay_note_choice(DelayNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(tone.as_bytes(), before);
}

#[test]
fn delay_note_rejects_noncanonical_prefix_type_unknown_values_and_profiles() {
    let original = delay_note_fixture(
        DELAY_SLOT,
        0x0202_0002,
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        Some(120.0),
    );
    let mut bad_prefix = original.clone();
    let parsed = PodTone::parse(original.clone()).unwrap();
    let prefix_at = parameter_word0_offset(&parsed, DELAY_SLOT, 0x0020_0000);
    bad_prefix[prefix_at] = 0x3e;
    let mut tone = PodTone::parse(bad_prefix).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(
        tone.set_delay_note_choice(DelayNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(tone.as_bytes(), before);

    let mut wrong_type = original.clone();
    wrong_type[prefix_at + 4..prefix_at + 8].copy_from_slice(&1u32.to_be_bytes());
    let mut tone = PodTone::parse(wrong_type).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(
        tone.set_delay_note_choice(DelayNoteChoice::Quarter)
            .is_err()
    );
    assert_eq!(tone.as_bytes(), before);

    let mut unknown_note = original.clone();
    unknown_note[prefix_at + 8..prefix_at + 12].copy_from_slice(&14u32.to_be_bytes());
    let mut tone = PodTone::parse(unknown_note).unwrap();
    assert_eq!(
        tone.delay_note_value().unwrap(),
        DelayNoteValue::Unknown { raw_bits: 14 }
    );
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_delay_note_choice(DelayNoteChoice::Off).is_err());
    assert_eq!(tone.as_bytes(), before);

    let mut old_profile = original.clone();
    let pinf = find_chunk_data(&old_profile, b"PINF");
    old_profile[pinf + 72..pinf + 76]
        .copy_from_slice(&(PODXT_LIVE_HOST_MIN_VERSION - 1).to_be_bytes());
    let mut tone = PodTone::parse(old_profile).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_delay_time_ms(1010.0).is_err());
    assert_eq!(tone.as_bytes(), before);
}

#[test]
fn delay_note_library_and_bundle_paths_revert_save_and_reopen() {
    let original = delay_note_fixture(
        DELAY_SLOT,
        0x0202_0002,
        0,
        0x3e80_0000,
        Some(MOD_SLOT),
        MOD_CONTROL_MODEL_IDS[0],
        0,
        Some(120.0),
    );
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    library
        .set_delay_note_choice(index, DelayNoteChoice::Quarter)
        .unwrap();
    assert_eq!(
        parm_bits(library.tone(index).unwrap(), DELAY_SLOT, 0x0020_0000),
        6
    );
    assert!(library.is_dirty(index));
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    library.set_delay_time_ms(index, 1010.0).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("delay-note.l6c");
    library.tone(index).unwrap().save_copy(&path).unwrap();
    let saved = std::fs::read(&path).unwrap();
    assert_eq!(PodTone::parse(saved.clone()).unwrap().as_bytes(), saved);
    assert!(library.tone(index).unwrap().save_copy(&path).is_err());
    assert_eq!(std::fs::read(path).unwrap(), saved);

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let before = bundle.as_bytes().to_vec();
    bundle
        .set_delay_note_choice(0, DelayNoteChoice::Quarter)
        .unwrap();
    assert!(bundle.is_dirty());
    let (bank, preset) = bundle.entry_location(0).unwrap();
    assert_eq!(
        parm_bits(&bundle.tone(bank, preset).unwrap(), DELAY_SLOT, 0x0020_0000),
        6
    );
    assert_eq!(bundle.as_bytes().len(), before.len());
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), before);
}

#[test]
fn delay_model_id_selection_is_closed_exact_and_lossless() {
    for current in DELAY_CONTROL_MODEL_IDS {
        let original = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: DELAY_SLOT,
                model_id: current,
                tail: &[0x7f, 0xa5, 0x00, 0xfe, 0x55],
                parameters: &[&[0x11, 0x22, 0x33]],
            }],
        );
        for requested in PodDelayModelChoice::MODEL_IDS {
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            tone.set_delay_model_choice(PodDelayModelChoice::try_from(requested).unwrap())
                .unwrap();
            let differences: Vec<_> = before
                .iter()
                .zip(tone.as_bytes())
                .enumerate()
                .filter_map(|(i, (a, b))| (a != b).then_some(i))
                .collect();
            if current == requested {
                assert!(differences.is_empty());
            } else {
                let reparsed = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
                assert_eq!(reparsed.models()[0].model_id, requested);
                let minf_word =
                    tone.model_minfo_ranges[0].start..tone.model_minfo_ranges[0].start + 4;
                assert!(differences.iter().all(|offset| minf_word.contains(offset)));
                assert_eq!(&tone.as_bytes()[minf_word], &requested.to_be_bytes());
            }
        }
    }
}

#[test]
fn mod_model_id_selection_covers_current_and_requested_ids_losslessly() {
    for current in MOD_CONTROL_MODEL_IDS {
        let original = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: MOD_SLOT,
                model_id: current,
                // Preserve noncanonical enable/reserved bytes, opaque tail,
                // parameters, and unrelated children through the edit.
                tail: &[0x7f, 0xa5, 0x00, 0xfe, 0x55],
                parameters: &[&[0x11, 0x22, 0x33]],
            }],
        );
        for requested in PodModModelChoice::MODEL_IDS {
            let choice = PodModModelChoice::try_from(requested).unwrap();
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            let minf_start = tone.model_minfo_ranges[0].start;
            let mut expected = before.clone();
            expected[minf_start..minf_start + 4].copy_from_slice(&requested.to_be_bytes());

            tone.set_mod_model_choice(choice).unwrap();

            assert_eq!(tone.as_bytes(), expected);
            let reparsed = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
            assert_eq!(reparsed.models()[0].model_id, requested);
            assert_eq!(reparsed.models()[0].slot, MOD_SLOT);
        }
    }
}

#[test]
fn stomp_model_choice_partition_and_full_buffer_matrix_are_exact() {
    assert_eq!(STOMP_CONTROL_MODEL_IDS.len(), 31);
    assert_eq!(STOMP_MODEL_CHOICE_IDS.len(), 14);
    for id in STOMP_MODEL_CHOICE_IDS {
        assert!(STOMP_CONTROL_MODEL_IDS.contains(&id));
    }
    let gated = [
        0x0205_0006,
        0x0205_0001,
        0x0200_0006,
        0x0205_0007,
        0x0200_0003,
        0x020a_000a,
        0x020a_000b,
        0x020a_000c,
        0x020a_0008,
        0x020a_0005,
        0x020a_0006,
        0x020a_0007,
        0x020a_0003,
        0x020a_0004,
        0x020a_0009,
        0x020a_0001,
        0x020a_0002,
    ];
    assert_eq!(gated.len(), 17);
    for id in gated {
        assert!(STOMP_CONTROL_MODEL_IDS.contains(&id));
        assert!(!STOMP_MODEL_CHOICE_IDS.contains(&id));
        assert!(PodStompModelChoice::try_from(id).is_err());
    }
    for (index, left) in STOMP_CONTROL_MODEL_IDS.iter().enumerate() {
        for right in STOMP_CONTROL_MODEL_IDS.iter().skip(index + 1) {
            assert_ne!(left, right);
        }
    }

    for current in STOMP_CONTROL_MODEL_IDS {
        let original = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: current,
                tail: &[0x80, 0xa5, 0x00, 0xfe, 0x55],
                parameters: &[&[0xde, 0xad, 0xbe]],
            }],
        );
        for requested in STOMP_MODEL_CHOICE_IDS {
            let choice = PodStompModelChoice::try_from(requested).unwrap();
            let mut tone = PodTone::parse(original.clone()).unwrap();
            let before = tone.as_bytes().to_vec();
            let minf = tone.model_minfo_ranges[0].start;
            let mut expected = before.clone();
            if current != requested {
                expected[minf..minf + 4].copy_from_slice(&requested.to_be_bytes());
            }
            tone.set_stomp_model_choice(choice).unwrap();
            assert_eq!(
                tone.as_bytes(),
                expected,
                "{current:08x} -> {requested:08x}"
            );
            let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
            assert_eq!(reopened.stomp_model_id(), Some(requested));
            assert_eq!(reopened.models()[0].slot, STOMP_SLOT);
            assert_eq!(reopened.models()[0].ordinal, Some(-128));
            assert_eq!(reopened.models()[0].reserved, Some([0xa5, 0x00]));
            assert_eq!(reopened.models()[0].enabled, Some(false));
        }
    }
}

#[test]
fn stomp_model_selection_rejects_bad_structure_and_context_atomically() {
    let choice = PodStompModelChoice::try_from(0x0205_0002).unwrap();
    for (kind, slot, current) in [
        (ContainerKind::L6as, STOMP_SLOT, 0x0205_0000),
        (ContainerKind::L6pa, MOD_SLOT, 0x0205_0000),
        (ContainerKind::L6pa, STOMP_SLOT, 0xdead_beef),
    ] {
        let mut tone = PodTone::parse(custom_tone(
            kind,
            &[BuiltModel {
                slot,
                model_id: current,
                tail: &[1, 2, 3, 4],
                parameters: &[],
            }],
        ))
        .unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_stomp_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }

    for tail in [&[][..], &[0][..], &[0, 0][..], &[0, 0, 1][..]] {
        let mut tone = PodTone::parse(custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x0205_0000,
                tail,
                parameters: &[],
            }],
        ))
        .unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_stomp_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }

    let short_same_id = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: choice.model_id(),
            tail: &[],
            parameters: &[],
        }],
    );
    let mut tone = PodTone::parse(short_same_id).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_stomp_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), before);

    for models in [
        vec![],
        vec![
            BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x0205_0000,
                tail: &[0; 4],
                parameters: &[],
            },
            BuiltModel {
                slot: STOMP_SLOT,
                model_id: 0x0205_0002,
                tail: &[0; 4],
                parameters: &[],
            },
        ],
    ] {
        let bytes = custom_tone(ContainerKind::L6pa, &models);
        if models.len() == 2 {
            assert!(PodTone::parse(bytes.clone()).is_err());
            continue;
        }
        let mut tone = PodTone::parse(bytes).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_stomp_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }

    let mut wrong_target = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: 0x0205_0000,
            tail: &[0; 4],
            parameters: &[],
        }],
    );
    // PINF target resides at the fixture's standard target word offset.
    let target = wrong_target
        .windows(8)
        .position(|window| window == b"\0\0\0\x01\0\x03\0\x0a")
        .unwrap()
        + 4;
    wrong_target[target + 3] ^= 1;
    assert!(PodTone::parse(wrong_target.clone()).is_err());
    assert_eq!(wrong_target[target + 3], 0x0b);

    let mut wrong_layout = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: choice.model_id(),
            tail: &[0; 4],
            parameters: &[],
        }],
    );
    let pinf = find_chunk_data(&wrong_layout, b"PINF");
    wrong_layout[pinf..pinf + 4].copy_from_slice(&2u32.to_be_bytes());
    assert!(PodTone::parse(wrong_layout.clone()).is_err());
    assert_eq!(wrong_layout[pinf..pinf + 4], 2u32.to_be_bytes());

    let old_profile = fixture(
        ContainerKind::L6pa,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION - 1,
        &[(STOMP_SLOT, 0x0205_0000)],
    );
    let mut tone = PodTone::parse(old_profile.clone()).unwrap();
    assert!(tone.set_stomp_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), old_profile);

    fn alter_stomp_minfo(mut bytes: Vec<u8>, duplicate: bool) -> Vec<u8> {
        let root = preset::chunks(&bytes, 12..bytes.len()).unwrap();
        let patch = root
            .iter()
            .find(|chunk| {
                chunk.id == *b"LIST" && &bytes[chunk.data.start..chunk.data.start + 4] == b"PATC"
            })
            .unwrap()
            .clone();
        let models = preset::chunks(&bytes, patch.data.start + 4..patch.data.end).unwrap();
        let model = models
            .iter()
            .find(|chunk| {
                chunk.id == *b"LIST" && &bytes[chunk.data.start..chunk.data.start + 4] == b"MODL"
            })
            .unwrap()
            .clone();
        let fields = preset::chunks(&bytes, model.data.start + 4..model.data.end).unwrap();
        let minf = fields
            .iter()
            .find(|chunk| chunk.id == *b"MINF")
            .unwrap()
            .clone();
        let start = minf.data.start - 8;
        let end = minf.data.end + minf.data.len() % 2;
        let payload = bytes[start..end].to_vec();
        if duplicate {
            bytes.splice(end..end, payload.iter().copied());
        } else {
            bytes.drain(start..end);
        }
        let delta = if duplicate {
            payload.len() as i32
        } else {
            -(payload.len() as i32)
        };
        for chunk_start in [model.data.start - 8, patch.data.start - 8] {
            let length = preset::word(&bytes, chunk_start + 4) as i32;
            bytes[chunk_start + 4..chunk_start + 8]
                .copy_from_slice(&(length + delta).to_be_bytes());
        }
        let root_length = preset::word(&bytes, 4) as i32;
        bytes[4..8].copy_from_slice(&(root_length + delta).to_be_bytes());
        bytes
    }
    for duplicate in [false, true] {
        let malformed = alter_stomp_minfo(
            custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot: STOMP_SLOT,
                    model_id: 0x0205_0000,
                    tail: &[0; 4],
                    parameters: &[],
                }],
            ),
            duplicate,
        );
        assert!(PodTone::parse(malformed).is_err());
    }
}

#[test]
fn stomp_model_choice_matches_independent_whole_file_fixture_and_reopens() {
    fn vector(text: &str, marker: &str, stop: &str) -> Vec<u8> {
        let section = text.split_once(marker).unwrap().1;
        section
            .lines()
            .take_while(|line| !line.starts_with(stop))
            .filter_map(|line| line.split_once(':').map(|(_, bytes)| bytes))
            .flat_map(str::split_whitespace)
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }
    let contract = include_str!("../../../../docs/pod-model-selection-contract.md")
        .split_once("## Stomp model selector: admitted offline ID-only operation")
        .unwrap()
        .1;
    let before = vector(
        contract,
        "before (178 bytes; FORM length 0x000000aa):",
        "after (178 bytes; FORM length 0x000000aa):",
    );
    let expected = vector(
        contract,
        "after (178 bytes; FORM length 0x000000aa):",
        "```",
    );
    assert_eq!(before.len(), 178);
    assert_eq!(expected.len(), 178);
    let mut tone = PodTone::parse(before).unwrap();
    tone.set_stomp_model_choice(PodStompModelChoice::try_from(0x0205_0002).unwrap())
        .unwrap();
    assert_eq!(tone.as_bytes(), expected);
    let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
    assert_eq!(reopened.stomp_model_id(), Some(0x0205_0002));
}

#[test]
fn stomp_model_library_and_bundle_apply_track_dirty_revert_and_save_collision() {
    let original = fixture(
        ContainerKind::L6pa,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(STOMP_SLOT, 0x020a_000a)],
    );
    let choice = PodStompModelChoice::try_from(0x0205_0000).unwrap();
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    library.set_stomp_model_choice(index, choice).unwrap();
    assert!(library.is_dirty(index));
    assert_eq!(
        library.tone(index).unwrap().stomp_model_id(),
        Some(choice.model_id())
    );
    let directory = tempfile::tempdir().unwrap();
    let copy = directory.path().join("stomp-choice.l6t");
    library.tone(index).unwrap().save_copy(&copy).unwrap();
    let saved = std::fs::read(&copy).unwrap();
    assert_eq!(PodTone::parse(saved.clone()).unwrap().as_bytes(), saved);
    assert!(library.tone(index).unwrap().save_copy(&copy).is_err());
    assert_eq!(std::fs::read(&copy).unwrap(), saved);
    library.revert_unsaved(index).unwrap();
    assert!(!library.is_dirty(index));
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let before = bundle.as_bytes().to_vec();
    bundle.set_stomp_model_choice(0, choice).unwrap();
    assert!(bundle.is_dirty());
    let (bank, preset) = bundle.entry_location(0).unwrap();
    assert_eq!(
        bundle.tone(bank, preset).unwrap().stomp_model_id(),
        Some(choice.model_id())
    );
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), before);
    assert!(!bundle.is_dirty());
}

#[test]
fn stomp_model_failed_edit_preserves_dirty_library_and_bundle_baselines() {
    let original = fixture(
        ContainerKind::L6pa,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(STOMP_SLOT, 0x020a_000a)],
    );
    let unknown = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: STOMP_SLOT,
            model_id: 0xdead_beef,
            tail: &[0xa1, 0xb2, 0xc3, 0xd4],
            parameters: &[],
        }],
    );
    let choice = PodStompModelChoice::try_from(0x0205_0000).unwrap();

    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    library
        .set_stomp_model_choice(index, PodStompModelChoice::try_from(0x0205_0002).unwrap())
        .unwrap();
    library.replace(index, unknown.clone()).unwrap();
    let before = library.tone(index).unwrap().as_bytes().to_vec();
    assert!(library.is_dirty(index));
    assert_eq!(
        library.tone(index).unwrap().stomp_model_id(),
        Some(0xdead_beef)
    );
    assert!(library.set_stomp_model_choice(index, choice).is_err());
    assert_eq!(library.tone(index).unwrap().as_bytes(), before);
    assert_eq!(
        library.tone(index).unwrap().stomp_model_id(),
        Some(0xdead_beef)
    );
    assert!(library.is_dirty(index));
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    assert!(!library.is_dirty(index));

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    bundle
        .set_stomp_model_choice(0, PodStompModelChoice::try_from(0x0205_0002).unwrap())
        .unwrap();
    bundle.replace_entry(0, unknown).unwrap();
    let before = bundle.as_bytes().to_vec();
    assert!(bundle.is_dirty());
    let (bank, preset) = bundle.entry_location(0).unwrap();
    assert_eq!(
        bundle.tone(bank, preset).unwrap().stomp_model_id(),
        Some(0xdead_beef)
    );
    assert!(bundle.set_stomp_model_choice(0, choice).is_err());
    assert_eq!(bundle.as_bytes(), before);
    assert_eq!(
        bundle.tone(bank, preset).unwrap().stomp_model_id(),
        Some(0xdead_beef)
    );
    assert!(bundle.is_dirty());
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), bundle_with_tone(&original));
    assert!(!bundle.is_dirty());
}

#[test]
fn mod_model_edit_matches_independent_documented_full_buffer_vector() {
    fn vector(text: &str, marker: &str, stop: &str) -> Vec<u8> {
        let section = text.split_once(marker).unwrap().1;
        section
            .lines()
            .take_while(|line| !line.trim().starts_with("```") && !line.starts_with(stop))
            .filter_map(|line| line.split_once(':').map(|(_, bytes)| bytes))
            .flat_map(str::split_whitespace)
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }
    let contract = include_str!("../../../../docs/pod-model-selection-contract.md")
        .split_once("## Mod model selector")
        .unwrap()
        .1;
    let before = vector(
        contract,
        "before (178 bytes; FORM length 0x000000aa):",
        "after (178 bytes; FORM length 0x000000aa):",
    );
    let expected = vector(
        contract,
        "after (178 bytes; FORM length 0x000000aa):",
        "```",
    );
    assert_eq!(before.len(), 178);
    assert_eq!(expected.len(), 178);
    let mut tone = PodTone::parse(before).unwrap();
    tone.set_mod_model_choice(PodModModelChoice::try_from(0x0203_0005).unwrap())
        .unwrap();
    assert_eq!(tone.as_bytes(), expected);
}

#[test]
fn mod_model_id_selection_rejects_gated_requests_and_invalid_context_atomically() {
    let make = |slot, id| {
        custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot,
                model_id: id,
                tail: &[0x7f, 0xa5, 0x00, 0xfe],
                parameters: &[],
            }],
        )
    };
    let choice = PodModModelChoice::try_from(0x0203_0005).unwrap();
    for gated in [
        0x0203_0015,
        0x0203_0012,
        0x0203_001e,
        0x0203_0014,
        0x0203_001f,
        0x0203_0013,
        0x0203_0011,
        0x0203_001a,
        0x0203_001b,
        0x0203_0017,
        0x0203_0018,
        0x0203_0019,
        0x0203_001c,
    ] {
        assert!(PodModModelChoice::try_from(gated).is_err());
    }

    let fallback = make(0x0005_0003, 0x0203_0015);
    let mut tone = PodTone::parse(fallback).unwrap();
    tone.set_mod_model_choice(choice).unwrap();
    assert_eq!(tone.models()[0].model_id, choice.model_id());
    assert_eq!(tone.models()[0].slot, 0x0005_0003);

    for (kind, slot, id) in [
        (ContainerKind::L6as, MOD_SLOT, 0x0203_0004),
        (ContainerKind::L6pa, MOD_SLOT, 0x3333_0001),
        (ContainerKind::L6pa, 0x0009_0001, 0x0203_0004),
    ] {
        let mut tone = PodTone::parse(custom_tone(
            kind,
            &[BuiltModel {
                slot,
                model_id: id,
                tail: &[1, 2, 3, 4],
                parameters: &[],
            }],
        ))
        .unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_mod_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }

    let both = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: MOD_SLOT,
                model_id: 0x0203_0004,
                tail: &[1, 2, 3, 4],
                parameters: &[],
            },
            BuiltModel {
                slot: 0x0005_0003,
                model_id: 0x0203_0005,
                tail: &[5, 6, 7, 8],
                parameters: &[],
            },
        ],
    );
    let mut tone = PodTone::parse(both).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_mod_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), before);

    let unknown = make(MOD_SLOT, 0x3333_0001);
    let mut tone = PodTone::parse(unknown.clone()).unwrap();
    assert!(tone.set_mod_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), unknown);

    for (tail, body_len) in [
        (&[][..], 8),
        (&[0][..], 9),
        (&[0, 0][..], 10),
        (&[0, 0, 1][..], 11),
    ] {
        let short = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: MOD_SLOT,
                model_id: 0x0203_0004,
                tail,
                parameters: &[],
            }],
        );
        let mut tone = PodTone::parse(short).unwrap();
        assert_eq!(tone.model_minfo_ranges[0].len(), body_len);
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_mod_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }

    for duplicate in [false, true] {
        let valid = make(MOD_SLOT, 0x0203_0004);
        let malformed = alter_first_model_minf(&valid, duplicate);
        assert!(PodTone::parse(malformed).is_err());
    }

    let mut old_profile = make(MOD_SLOT, 0x0203_0004);
    let pinf = find_chunk_data(&old_profile, b"PINF");
    old_profile[pinf + 72..pinf + 76].copy_from_slice(&0x02ff_ffffu32.to_be_bytes());
    let mut tone = PodTone::parse(old_profile).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_mod_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), before);

    let mut wrong_target = make(MOD_SLOT, 0x0203_0004);
    let pinf = find_chunk_data(&wrong_target, b"PINF");
    wrong_target[pinf + 4..pinf + 8].copy_from_slice(&0x0003_000bu32.to_be_bytes());
    let before = wrong_target.clone();
    assert!(PodTone::parse(wrong_target.clone()).is_err());
    assert_eq!(wrong_target, before);

    let mut wrong_layout = make(MOD_SLOT, 0x0203_0004);
    let pinf = find_chunk_data(&wrong_layout, b"PINF");
    wrong_layout[pinf..pinf + 4].copy_from_slice(&2u32.to_be_bytes());
    assert!(PodTone::parse(wrong_layout.clone()).is_err());
    assert_eq!(wrong_layout[pinf..pinf + 4], 2u32.to_be_bytes());

    let duplicate_slot = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: MOD_SLOT,
                model_id: 0x0203_0004,
                tail: &[1, 2, 3, 4],
                parameters: &[],
            },
            BuiltModel {
                slot: MOD_SLOT,
                model_id: 0x0203_0005,
                tail: &[5, 6, 7, 8],
                parameters: &[],
            },
        ],
    );
    assert!(PodTone::parse(duplicate_slot.clone()).is_err());
    assert_eq!(duplicate_slot[8..12], *b"L6PA");

    let same_id_wrong_context = custom_tone(
        ContainerKind::L6as,
        &[BuiltModel {
            slot: MOD_SLOT,
            model_id: choice.model_id(),
            tail: &[1, 2, 3, 4],
            parameters: &[],
        }],
    );
    let mut tone = PodTone::parse(same_id_wrong_context).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_mod_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), before);
}

#[test]
fn mod_model_id_selection_library_bundle_and_saved_baseline_are_isolated() {
    let original = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: MOD_SLOT,
            model_id: 0x0203_0004,
            tail: &[0x7f, 0xa5, 0x00, 0xfe],
            parameters: &[&[0x11, 0x22, 0x33]],
        }],
    );
    let choice = PodModModelChoice::try_from(0x0203_0005).unwrap();

    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    let baseline = library.tone(index).unwrap().as_bytes().to_vec();
    library.set_mod_model_choice(index, choice).unwrap();
    assert!(library.is_dirty(index));
    assert_ne!(library.tone(index).unwrap().as_bytes(), baseline);
    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("existing-mod.l6p");
    std::fs::write(&collision, b"keep existing bytes").unwrap();
    assert!(library.tone(index).unwrap().save_copy(&collision).is_err());
    assert_eq!(std::fs::read(&collision).unwrap(), b"keep existing bytes");
    let saved = directory.path().join("edited-mod.l6p");
    library.tone(index).unwrap().save_copy(&saved).unwrap();
    let reopened = PodTone::parse(std::fs::read(saved).unwrap()).unwrap();
    assert_eq!(reopened.models()[0].model_id, choice.model_id());
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    assert!(!library.is_dirty(index));

    let mut dirty_library = Library::default();
    let dirty_index = dirty_library.import_bytes(original.clone()).unwrap();
    let unknown_current = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: MOD_SLOT,
            model_id: 0x3333_0001,
            tail: &[0x7f, 0xa5, 0x00, 0xfe],
            parameters: &[],
        }],
    );
    dirty_library
        .replace(dirty_index, unknown_current.clone())
        .unwrap();
    let dirty_before_failure = dirty_library.tone(dirty_index).unwrap().as_bytes().to_vec();
    assert!(
        dirty_library
            .set_mod_model_choice(dirty_index, choice)
            .is_err()
    );
    assert_eq!(
        dirty_library.tone(dirty_index).unwrap().as_bytes(),
        dirty_before_failure
    );
    assert!(dirty_library.is_dirty(dirty_index));
    dirty_library.revert_unsaved(dirty_index).unwrap();
    assert_eq!(
        dirty_library.tone(dirty_index).unwrap().as_bytes(),
        original
    );

    let mut dirty_bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let bundle_baseline = dirty_bundle.as_bytes().to_vec();
    dirty_bundle
        .replace_entry(0, unknown_current.clone())
        .unwrap();
    let dirty_bundle_bytes = dirty_bundle.as_bytes().to_vec();
    assert!(dirty_bundle.set_mod_model_choice(0, choice).is_err());
    assert_eq!(dirty_bundle.as_bytes(), dirty_bundle_bytes);
    assert!(dirty_bundle.is_dirty());
    dirty_bundle.revert().unwrap();
    assert_eq!(dirty_bundle.as_bytes(), bundle_baseline);
    assert!(!dirty_bundle.is_dirty());

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let bundle_before = bundle.as_bytes().to_vec();
    bundle.set_mod_model_choice(0, choice).unwrap();
    assert!(bundle.is_dirty());
    let (bank, preset) = bundle.entry_location(0).unwrap();
    assert_eq!(
        bundle.tone(bank, preset).unwrap().models()[0].model_id,
        choice.model_id()
    );
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), bundle_before);
    assert!(!bundle.is_dirty());
}

#[test]
fn delay_model_edit_matches_independent_documented_188_byte_vector() {
    fn vector(text: &str, marker: &str, end_marker: &str) -> Vec<u8> {
        let section = text.split_once(marker).unwrap().1;
        section
            .lines()
            .take_while(|line| !line.trim().starts_with("```") && !line.starts_with(end_marker))
            .filter_map(|line| line.split_once(':').map(|(_, bytes)| bytes))
            .flat_map(str::split_whitespace)
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }
    let contract = include_str!("../../../../docs/pod-model-selection-contract.md");
    let before = vector(contract, "before (188 bytes):", "after (188 bytes):");
    let expected = vector(contract, "after (188 bytes):", "```");
    assert_eq!(before.len(), 188);
    assert_eq!(expected.len(), 188);
    let mut tone = PodTone::parse(before).unwrap();
    tone.set_delay_model_choice(PodDelayModelChoice::try_from(0x0202_0003).unwrap())
        .unwrap();
    assert_eq!(tone.as_bytes(), expected);
}

#[test]
fn delay_model_id_selection_supports_fallback_and_rejects_invalid_context_atomically() {
    let make = |kind, slot, id| {
        custom_tone(
            kind,
            &[BuiltModel {
                slot,
                model_id: id,
                tail: &[1, 0x99, 0x88, 0xfe],
                parameters: &[],
            }],
        )
    };
    let choice = PodDelayModelChoice::try_from(0x0202_0003).unwrap();
    let mut fallback = PodTone::parse(make(ContainerKind::L6pa, 0x0005_0004, 0x0202_000f)).unwrap();
    fallback.set_delay_model_choice(choice).unwrap();
    assert_eq!(fallback.models()[0].model_id, choice.model_id());
    for (kind, slot, id, version) in [
        (
            ContainerKind::L6as,
            DELAY_SLOT,
            0x0202_0002,
            PODXT_LIVE_HOST_MIN_VERSION,
        ),
        (
            ContainerKind::L6pa,
            DELAY_SLOT,
            0x3333_0001,
            PODXT_LIVE_HOST_MIN_VERSION,
        ),
        (
            ContainerKind::L6pa,
            0x0009_0001,
            0x0202_0002,
            PODXT_LIVE_HOST_MIN_VERSION,
        ),
    ] {
        let mut tone = PodTone::parse(make(kind, slot, id)).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
        let _ = version;
    }
    let both = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: DELAY_SLOT,
                model_id: 0x0202_0002,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            },
            BuiltModel {
                slot: 0x0005_0004,
                model_id: 0x0202_0003,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            },
        ],
    );
    let mut tone = PodTone::parse(both).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_delay_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), before);

    let mut old_profile = make(ContainerKind::L6pa, DELAY_SLOT, 0x0202_0002);
    let pinf = find_chunk_data(&old_profile, b"PINF");
    old_profile[pinf + 72..pinf + 76].copy_from_slice(&0x02ff_ffffu32.to_be_bytes());
    let mut tone = PodTone::parse(old_profile).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_delay_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), before);
    for gated in 0x0202_000b..=0x0202_000f {
        assert!(PodDelayModelChoice::try_from(gated).is_err());
    }
    for (tail, body_len) in [
        (&[][..], 8),
        (&[0][..], 9),
        (&[0, 0][..], 10),
        (&[0, 0, 1][..], 11),
    ] {
        let short = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: DELAY_SLOT,
                model_id: 0x0202_0002,
                tail,
                parameters: &[],
            }],
        );
        let mut tone = PodTone::parse(short).unwrap();
        assert_eq!(tone.model_minfo_ranges[0].len(), body_len);
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_delay_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }
    for duplicate in [false, true] {
        let valid = make(ContainerKind::L6pa, DELAY_SLOT, 0x0202_0002);
        let malformed = alter_first_model_minf(&valid, duplicate);
        let untouched = malformed.clone();
        assert!(PodTone::parse(malformed.clone()).is_err());
        assert_eq!(malformed, untouched);
    }
    for (offset, value) in [(4, 2u32), (8, 0x0003_000b)] {
        let mut invalid = make(ContainerKind::L6pa, DELAY_SLOT, 0x0202_0002);
        if offset == 4 {
            let pinf = find_chunk_data(&invalid, b"PINF");
            invalid[pinf..pinf + 4].copy_from_slice(&value.to_be_bytes());
        } else {
            let pinf = find_chunk_data(&invalid, b"PINF");
            invalid[pinf + 4..pinf + 8].copy_from_slice(&value.to_be_bytes());
        }
        let untouched = invalid.clone();
        assert!(PodTone::parse(invalid.clone()).is_err());
        assert_eq!(invalid, untouched);
    }
    let same_id_l6as = make(ContainerKind::L6as, DELAY_SLOT, 0x0202_0003);
    let mut tone = PodTone::parse(same_id_l6as).unwrap();
    let before = tone.as_bytes().to_vec();
    assert!(tone.set_delay_model_choice(choice).is_err());
    assert_eq!(tone.as_bytes(), before);

    let unknown_current = make(ContainerKind::L6pa, DELAY_SLOT, 0x3333_0001);
    let mut library = Library::default();
    let index = library.import_bytes(unknown_current.clone()).unwrap();
    assert!(!library.is_dirty(index));
    assert!(library.set_delay_model_choice(index, choice).is_err());
    assert_eq!(library.tone(index).unwrap().as_bytes(), unknown_current);
    assert!(!library.is_dirty(index));
    let mut changed = PodTone::parse(unknown_current.clone()).unwrap();
    changed.rename("dirty baseline").unwrap();
    library.replace(index, changed.into_bytes()).unwrap();
    let before_failure = library.tone(index).unwrap().as_bytes().to_vec();
    assert!(library.set_delay_model_choice(index, choice).is_err());
    assert_eq!(library.tone(index).unwrap().as_bytes(), before_failure);
    assert!(library.is_dirty(index));
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), unknown_current);

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&unknown_current)).unwrap();
    bundle
        .edit_entry(0, |tone| tone.rename("dirty bundle baseline"))
        .unwrap();
    let before_failure = bundle.as_bytes().to_vec();
    assert!(bundle.set_delay_model_choice(0, choice).is_err());
    assert_eq!(bundle.as_bytes(), before_failure);
    assert!(bundle.is_dirty());
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), bundle_with_tone(&unknown_current));
}

#[test]
fn delay_model_choice_flows_through_library_and_transactional_bundle() {
    let original = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: DELAY_SLOT,
            model_id: 0x0202_0002,
            tail: &[0, 0xa3, 0x91, 0xff],
            parameters: &[&[0x1, 0x2, 0x3]],
        }],
    );
    let choice = PodDelayModelChoice::try_from(0x0202_000a).unwrap();
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    library.set_delay_model_choice(index, choice).unwrap();
    assert!(library.is_dirty(index));
    assert_eq!(
        library.tone(index).unwrap().models()[0].model_id,
        choice.model_id()
    );
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let before = bundle.as_bytes().to_vec();
    bundle.set_delay_model_choice(0, choice).unwrap();
    assert!(bundle.is_dirty());
    let (bank, preset) = bundle.entry_location(0).unwrap();
    assert_eq!(
        bundle.tone(bank, preset).unwrap().models()[0].model_id,
        choice.model_id()
    );
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), before);
}

#[test]
fn reverb_model_id_selector_is_exhaustive_and_changes_only_minf_id() {
    assert_eq!(
        PodReverbModelChoice::MODEL_IDS,
        [
            0x0204_0002,
            0x0204_0003,
            0x0204_0004,
            0x0204_0005,
            0x0204_0006,
            0x0204_0007,
            0x0204_0008,
            0x0204_0009,
            0x0204_000a,
            0x0204_000b,
            0x0204_000c,
            0x0204_000d,
            0x0204_000e,
            0x0204_000f,
            0x0204_0010,
        ]
    );
    for slot in [REVERB_SLOT, REVERB_FALLBACK_SLOT] {
        for current in PodReverbModelChoice::MODEL_IDS {
            let original = custom_tone(
                ContainerKind::L6pa,
                &[BuiltModel {
                    slot,
                    model_id: current,
                    tail: &[0xfe, 0x12, 0x34, 0x01],
                    parameters: &[&[0xde, 0xad, 0xbe]],
                }],
            );
            for requested in PodReverbModelChoice::MODEL_IDS {
                let mut tone = PodTone::parse(original.clone()).unwrap();
                let range = tone.model_minfo_ranges[0].clone();
                tone.set_reverb_model_choice(PodReverbModelChoice::try_from(requested).unwrap())
                    .unwrap();
                let mut expected = original.clone();
                expected[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
                assert_eq!(
                    tone.as_bytes(),
                    expected,
                    "slot={slot:08x} current={current:08x} requested={requested:08x}"
                );
                let reopened = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
                assert_eq!(reopened.reverb_model_id(), Some(requested));
                assert_eq!(reopened.models()[0].slot, slot);
            }
        }
    }
    assert!(PodReverbModelChoice::try_from(0x0204_0001).is_err());
    assert!(PodReverbModelChoice::try_from(0x0204_0011).is_err());
}

#[test]
fn reverb_selector_matches_independent_literal_complete_buffer_fixture() {
    fn vector(text: &str, marker: &str, stop: &str) -> Vec<u8> {
        text.split_once(marker)
            .unwrap()
            .1
            .lines()
            .take_while(|line| !line.starts_with(stop))
            .filter_map(|line| line.split_once(':').map(|(_, bytes)| bytes))
            .flat_map(str::split_whitespace)
            .map(|byte| u8::from_str_radix(byte, 16).unwrap())
            .collect()
    }
    let contract = include_str!("../../../../docs/pod-model-selection-contract.md")
        .split_once("## Reverb model selector: conservative offline ID-only operation")
        .unwrap()
        .1;
    let before = vector(contract, "before (178 bytes):", "after (178 bytes):");
    let expected = vector(contract, "after (178 bytes):", "Only offset");
    assert_eq!(before.len(), 178);
    assert_eq!(expected.len(), 178);
    let mut tone = PodTone::parse(before).unwrap();
    tone.set_reverb_model_choice(PodReverbModelChoice::try_from(0x0204_0003).unwrap())
        .unwrap();
    assert_eq!(tone.as_bytes(), expected);
    assert_eq!(
        PodTone::parse(expected).unwrap().reverb_model_id(),
        Some(0x0204_0003)
    );
}

#[test]
fn reverb_model_selector_rejects_ambiguous_unknown_and_short_models_without_mutation() {
    let choice = PodReverbModelChoice::try_from(0x0204_0003).unwrap();
    let absent = custom_tone(ContainerKind::L6pa, &[]);
    let both = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: REVERB_SLOT,
                model_id: 0x0204_0002,
                tail: &[1; 4],
                parameters: &[],
            },
            BuiltModel {
                slot: REVERB_FALLBACK_SLOT,
                model_id: 0x0204_0002,
                tail: &[2; 4],
                parameters: &[],
            },
        ],
    );
    let duplicate = custom_tone(
        ContainerKind::L6pa,
        &[
            BuiltModel {
                slot: REVERB_SLOT,
                model_id: 0x0204_0002,
                tail: &[1; 4],
                parameters: &[],
            },
            BuiltModel {
                slot: REVERB_SLOT,
                model_id: 0x0204_0002,
                tail: &[2; 4],
                parameters: &[],
            },
        ],
    );
    let unknown = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: REVERB_SLOT,
            model_id: 0xdead_beef,
            tail: &[1; 4],
            parameters: &[],
        }],
    );
    assert!(PodTone::parse(duplicate).is_err());
    for input in [absent, both, unknown] {
        let mut tone = PodTone::parse(input).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(tone.set_reverb_model_choice(choice).is_err());
        assert_eq!(tone.as_bytes(), before);
    }
    for tail_len in 0..4 {
        let input = custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: REVERB_SLOT,
                model_id: 0x0204_0002,
                tail: &[0xaa, 0xbb, 0xcc][..tail_len],
                parameters: &[],
            }],
        );
        let mut tone = PodTone::parse(input).unwrap();
        let before = tone.as_bytes().to_vec();
        assert!(
            tone.set_reverb_model_choice(choice).is_err(),
            "MINF length {}",
            8 + tail_len
        );
        assert_eq!(tone.as_bytes(), before);
    }
    let valid = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: REVERB_SLOT,
            model_id: 0x0204_0002,
            tail: &[1; 4],
            parameters: &[],
        }],
    );
    let malformed_minf = alter_first_model_minf(&valid, true);
    assert!(PodTone::parse(malformed_minf).is_err());
    assert!(PodTone::parse(alter_first_model_minf(&valid, false)).is_err());
}

#[test]
fn reverb_model_selector_enforces_target_container_and_profile_before_same_id_noop() {
    let choice = PodReverbModelChoice::try_from(0x0204_0002).unwrap();
    let wrong_target = fixture(
        ContainerKind::L6pa,
        PODXT_LIVE_TONE_ID + 1,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(REVERB_SLOT, choice.model_id())],
    );
    assert!(
        PodTone::parse(wrong_target).is_err(),
        "wrong-target tone must be rejected at parser ingress"
    );

    let old_profile = fixture(
        ContainerKind::L6pa,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION - 1,
        &[(REVERB_SLOT, choice.model_id())],
    );
    let mut old_profile = PodTone::parse(old_profile)
        .expect("old-profile container is parseable before setter policy is checked");
    let before = old_profile.as_bytes().to_vec();
    assert!(old_profile.set_reverb_model_choice(choice).is_err());
    assert_eq!(old_profile.as_bytes(), before);

    let l6fs = fixture(
        ContainerKind::L6fs,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(REVERB_SLOT, choice.model_id())],
    );
    let mut l6fs =
        PodTone::parse(l6fs).expect("L6FS container is parseable before setter policy is checked");
    let before = l6fs.as_bytes().to_vec();
    assert!(l6fs.set_reverb_model_choice(choice).is_err());
    assert_eq!(l6fs.as_bytes(), before);
    let input = fixture(
        ContainerKind::L6pa,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(REVERB_SLOT, choice.model_id())],
    );
    let mut tone = PodTone::parse(input.clone()).unwrap();
    let before = tone.as_bytes().to_vec();
    tone.set_reverb_model_choice(choice).unwrap();
    assert_eq!(tone.as_bytes(), before);
    let mut wrong_layout = input;
    let pinf = find_chunk_data(&wrong_layout, b"PINF");
    wrong_layout[pinf..pinf + 4].copy_from_slice(&2u32.to_be_bytes());
    assert!(PodTone::parse(wrong_layout).is_err());
}

#[test]
fn reverb_model_selector_library_and_bundle_preserve_dirty_baselines() {
    let original = fixture(
        ContainerKind::L6pa,
        PODXT_LIVE_TONE_ID,
        PODXT_LIVE_HOST_MIN_VERSION,
        &[(REVERB_FALLBACK_SLOT, 0x0204_0002)],
    );
    let choice = PodReverbModelChoice::try_from(0x0204_0003).unwrap();
    let mut library = Library::default();
    let index = library.import_bytes(original.clone()).unwrap();
    library.set_reverb_model_choice(index, choice).unwrap();
    assert!(library.is_dirty(index));
    assert_eq!(
        library.tone(index).unwrap().reverb_model_id(),
        Some(choice.model_id())
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reverb-choice.l6t");
    library.tone(index).unwrap().save_copy(&path).unwrap();
    let saved = std::fs::read(&path).unwrap();
    assert_eq!(PodTone::parse(saved.clone()).unwrap().as_bytes(), saved);
    assert!(library.tone(index).unwrap().save_copy(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    assert!(!library.is_dirty(index));

    let mut bundle = PodBundleLibrary::parse(bundle_with_tone(&original)).unwrap();
    let before = bundle.as_bytes().to_vec();
    bundle.set_reverb_model_choice(0, choice).unwrap();
    assert!(bundle.is_dirty());
    let (bank, preset) = bundle.entry_location(0).unwrap();
    assert_eq!(
        bundle.tone(bank, preset).unwrap().reverb_model_id(),
        Some(choice.model_id())
    );
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), before);
    assert!(!bundle.is_dirty());

    let unknown = custom_tone(
        ContainerKind::L6pa,
        &[BuiltModel {
            slot: REVERB_FALLBACK_SLOT,
            model_id: 0xdead_beef,
            tail: &[0x55; 4],
            parameters: &[],
        }],
    );
    library.set_reverb_model_choice(index, choice).unwrap();
    library.replace(index, unknown.clone()).unwrap();
    let failed_bytes = library.tone(index).unwrap().as_bytes().to_vec();
    assert!(library.is_dirty(index));
    assert!(library.set_reverb_model_choice(index, choice).is_err());
    assert_eq!(library.tone(index).unwrap().as_bytes(), failed_bytes);
    assert_eq!(
        library.tone(index).unwrap().reverb_model_id(),
        Some(0xdead_beef)
    );
    assert!(library.is_dirty(index));
    library.revert_unsaved(index).unwrap();
    assert_eq!(library.tone(index).unwrap().as_bytes(), original);
    assert!(!library.is_dirty(index));

    bundle.set_reverb_model_choice(0, choice).unwrap();
    bundle.replace_entry(0, unknown).unwrap();
    let failed_bytes = bundle.as_bytes().to_vec();
    let (bank, preset) = bundle.entry_location(0).unwrap();
    assert!(bundle.is_dirty());
    assert!(bundle.set_reverb_model_choice(0, choice).is_err());
    assert_eq!(bundle.as_bytes(), failed_bytes);
    assert_eq!(
        bundle.tone(bank, preset).unwrap().reverb_model_id(),
        Some(0xdead_beef)
    );
    assert!(bundle.is_dirty());
    bundle.revert().unwrap();
    assert_eq!(bundle.as_bytes(), before);
    assert!(!bundle.is_dirty());
}

fn edited_name_too_long(tone: &PodTone) -> bool {
    let mut tone = tone.clone();
    tone.rename(&"🎸".repeat(17)).is_err() && tone.rename("bad\0name").is_err()
}

fn tone_name_offset(bytes: &[u8]) -> usize {
    find_chunk_data(bytes, b"PINF") + 8
}

fn find_chunk_data(bytes: &[u8], id: &[u8; 4]) -> usize {
    let children = preset::chunks(bytes, 12..bytes.len()).unwrap();
    preset::unique(&children, id).unwrap().data.start
}

fn table_test_tone() -> PodTone {
    PodTone::parse(custom_tone(ContainerKind::L6pa, &[])).unwrap()
}

fn table_test_control(tone: &PodTone, key: &str) -> PodControl<'static> {
    tone.table_controls(15)
        .into_iter()
        .find(|control| control.parameter.key == key)
        .unwrap_or_else(|| panic!("no table control {key}"))
}

#[test]
fn table_edits_match_original_java_tone_fields() {
    // Captured through #210's VerifyPodxtParameters recording boundary. Each
    // vector uses a synthetic patch with the listed model context. Expected
    // bytes are the original output at the edited field's payload offsets.
    #[derive(serde::Deserialize)]
    struct Vector {
        key: String,
        bits: u32,
        context: Option<(String, u32)>,
        bytes: Vec<(usize, u8)>,
    }
    let vectors: Vec<Vector> =
        serde_json::from_str(include_str!("fixtures/table-writer.json")).unwrap();
    let mut covered = std::collections::BTreeSet::new();
    for vector in &vectors {
        let mut tone = table_test_tone();
        if let Some((key, id)) = &vector.context {
            tone.set_table_model(key, *id, 15, true, &UnknownPodDefaults)
                .unwrap();
        }
        if vector.key == "TWEAK_PARAM" {
            for (key, id) in [
                ("STOMP_MODEL", 0x0205_0006),
                ("MOD_MODEL", 0x0203_0004),
                ("DELAY_MODEL", 0x0202_0002),
                ("REVERB_MODEL", 0x0204_0005),
            ] {
                tone.set_table_model(key, id, 15, true, &UnknownPodDefaults)
                    .unwrap();
            }
        }
        let control = table_test_control(&tone, &vector.key);
        let result = if control.parameter.catalog.is_some() {
            tone.set_table_model(&vector.key, vector.bits, 15, true, &UnknownPodDefaults)
                .map(|_| ())
        } else {
            tone.set_table_parameter(&vector.key, vector.bits)
        };
        result.unwrap_or_else(|error| {
            panic!(
                "{} {:08x} {:?}: {}",
                vector.key,
                vector.bits,
                vector.context,
                error.detail()
            )
        });
        assert_eq!(
            tone.table_tone_field(&vector.key).as_ref(),
            Some(&vector.bytes),
            "{} {:08x} {:?}",
            vector.key,
            vector.bits,
            vector.context
        );
        assert_eq!(PodTone::parse(tone.bytes.clone()).unwrap(), tone);
        covered.insert(vector.key.as_str());
    }
    assert_eq!(vectors.len(), 2539);
    assert_eq!(covered.len(), 86);
    for (key, value) in [
        ("COMP_AMOUNT", 0.5f32.to_bits()),
        ("FXLOOP_ENABLE", 1),
        ("MOD_PARAM5", 0.5f32.to_bits()),
        ("DELAY_PARAM5", 0.5f32.to_bits()),
    ] {
        assert!(
            table_test_tone().set_table_parameter(key, value).is_err(),
            "{key}"
        );
    }
}

#[test]
fn table_model_change_moves_shared_knobs_and_neutralizes_unshared_controls() {
    let mut tone = table_test_tone();
    tone.set_table_model("STOMP_MODEL", 0x0205_0000, 0, true, &UnknownPodDefaults)
        .unwrap();
    tone.set_table_parameter("STOMP_PARAM2", 0.25f32.to_bits())
        .unwrap();
    tone.set_table_parameter("STOMP_PARAM3", 0.75f32.to_bits())
        .unwrap();
    tone.set_table_parameter("STOMP_PARAM4", 0.5f32.to_bits())
        .unwrap();
    let before = tone.clone();
    assert!(
        tone.set_table_model("STOMP_MODEL", 0x0205_0001, 0, true, &UnknownPodDefaults)
            .is_err()
    );
    assert_eq!(tone, before);
    let change = tone
        .set_table_model("STOMP_MODEL", 0x0205_0001, 4, true, &UnknownPodDefaults)
        .unwrap();
    for (key, name, value, uncertain) in [
        ("STOMP_PARAM2", "Drive", 0.25f32, false),
        ("STOMP_PARAM3", "Treble", 0.0, true),
        ("STOMP_PARAM4", "Gain", 0.75, false),
        ("STOMP_PARAM5", "Bass", 0.0, true),
    ] {
        let control = table_test_control(&tone, key);
        assert_eq!(control.label(), name);
        assert_eq!(control.bits, Some(value.to_bits()));
        assert_eq!(
            change
                .default_unknown
                .contains(&control.parameter.selector.unwrap()),
            uncertain
        );
    }

    let change = tone
        .set_table_model("STOMP_MODEL", 0x020a_0009, 4, true, &UnknownPodDefaults)
        .unwrap();
    let harmony = table_test_control(&tone, "STOMP_PARAM3");
    assert_eq!(harmony.label(), "Harmony 1457");
    assert_eq!(harmony.choices.len(), 9);
    assert!(
        change
            .default_unknown
            .contains(&harmony.parameter.selector.unwrap())
    );
    assert_eq!(table_test_control(&tone, "STOMP_PARAM4").label(), "Wave");
    tone.set_table_model("DELAY_MODEL", 0x0202_000c, 4, true, &UnknownPodDefaults)
        .unwrap();
    assert_eq!(table_test_control(&tone, "DELAY_PARAM3").label(), "Heads");
    tone.set_delay_heads_selection(DelayHeadsSelection::OneTwoThreeFour)
        .unwrap();
    assert_eq!(
        tone.delay_heads_selection().unwrap(),
        DelayAssociationValue::Known(DelayHeadsSelection::OneTwoThreeFour)
    );
}

#[test]
fn table_default_provider_obeys_amp_toggle_and_rejects_invalid_values_atomically() {
    struct Defaults(u32);
    impl PodModelDefaults for Defaults {
        fn parameter_bits(&self, _: u32, _: u32, selector: u32) -> Option<u32> {
            (selector == 65538).then_some(self.0)
        }
    }

    let mut tone = table_test_tone();
    tone.set_table_model("AMP_MODEL", 0x0002_0001, 0, true, &UnknownPodDefaults)
        .unwrap();
    tone.set_table_parameter("DRIVE", 0.25f32.to_bits())
        .unwrap();
    let source = Defaults(0.75f32.to_bits());
    tone.set_table_model("AMP_MODEL", 0x0002_0005, 0, false, &source)
        .unwrap();
    assert_eq!(
        table_test_control(&tone, "DRIVE").bits,
        Some(0.25f32.to_bits())
    );
    tone.set_table_model("AMP_MODEL", 0x0002_0001, 0, true, &source)
        .unwrap();
    assert_eq!(
        table_test_control(&tone, "DRIVE").bits,
        Some(0.75f32.to_bits())
    );
    let before = tone.clone();
    assert!(
        tone.set_table_model(
            "AMP_MODEL",
            0x0002_0005,
            0,
            true,
            &Defaults(f32::NAN.to_bits())
        )
        .is_err()
    );
    assert_eq!(tone, before);
}

#[test]
fn table_tempo_and_notes_apply_both_dependencies_and_manual_clear() {
    let mut tone = table_test_tone();
    tone.set_table_parameter("TEMPO", 120.0f32.to_bits())
        .unwrap();
    tone.set_table_parameter("MOD_NOTE", 6).unwrap();
    tone.set_table_parameter("DELAY_NOTE", 6).unwrap();
    assert_eq!(
        table_test_control(&tone, "MOD_SPEED").bits,
        Some(0x3e0293c2)
    );
    assert_eq!(
        table_test_control(&tone, "DELAY_TIME").bits,
        Some(0x3e783e11)
    );
    tone.set_table_parameter("TEMPO", 60.0f32.to_bits())
        .unwrap();
    let speed = table_test_control(&tone, "MOD_SPEED");
    let delay = table_test_control(&tone, "DELAY_TIME");
    assert_eq!(speed.bits, speed.bits_from_display(1.0));
    assert_eq!(
        delay.bits,
        delay.bits_from_display(f64::from(1000.00006f32))
    );
    tone.set_table_parameter("DELAY_TIME", 0.5f32.to_bits())
        .unwrap();
    assert_eq!(table_test_control(&tone, "DELAY_NOTE").bits, Some(0));
    assert_eq!(table_test_control(&tone, "MOD_NOTE").bits, Some(6));
}

#[test]
fn table_metadata_has_original_tag_bytes_and_preserves_unknown_chunks() {
    let mut tone = table_test_tone();
    let original = tone.as_bytes().to_vec();
    for field in PodMetadata::ALL {
        assert_eq!(tone.metadata(field).unwrap(), None);
        tone.set_metadata(field, "Aé𝄞").unwrap();
        assert_eq!(tone.metadata(field).unwrap().as_deref(), Some("Aé𝄞"));
    }
    assert_eq!(&tone.as_bytes()[8..original.len()], &original[8..]);
    let mut expected = b"LIST\0\0\0\x84UNFO".to_vec();
    for tag in [
        b"ISNG", b"IGTR", b"IBND", b"IAUT", b"IPUS", b"IPUP", b"ISTL", b"ICMT",
    ] {
        expected.extend_from_slice(tag);
        expected.extend_from_slice(b"\0\0\0\x08\0A\0\xe9\xd84\xdd\x1e");
    }
    assert_eq!(&tone.as_bytes()[original.len()..], expected);
    let before = tone.clone();
    assert!(tone.set_metadata(PodMetadata::Notes, "Invalid\0").is_err());
    assert_eq!(tone, before);
}

#[test]
fn table_writes_preserve_opaque_records_and_refuse_ambiguous_targets() {
    let drive = [0x3f, 0x10, 0x00, 0x03, 0, 0, 0, 1, 0x3e, 0x80, 0, 0];
    let opaque = [0x12, 0x34, 0x56, 0x78, 0xd0];
    let model = BuiltModel {
        slot: AMP_SLOT,
        model_id: 0x0002_0001,
        tail: &[0, 0, 0, 1],
        parameters: &[&drive, &opaque],
    };
    let mut tone = PodTone::parse(custom_tone(ContainerKind::L6pa, &[model])).unwrap();
    let position = tone
        .as_bytes()
        .windows(drive.len())
        .position(|v| v == drive)
        .unwrap()
        + 8;
    let mut expected = tone.as_bytes().to_vec();
    expected[position..position + 4].copy_from_slice(&0.75f32.to_be_bytes());
    tone.set_table_parameter("DRIVE", 0.75f32.to_bits())
        .unwrap();
    assert_eq!(tone.as_bytes(), expected);

    let duplicate = BuiltModel {
        parameters: &[&drive, &drive, &opaque],
        ..model
    };
    let mut tone = PodTone::parse(custom_tone(ContainerKind::L6pa, &[duplicate])).unwrap();
    let before = tone.clone();
    assert!(
        tone.set_table_parameter("DRIVE", 0.75f32.to_bits())
            .is_err()
    );
    assert_eq!(tone, before);
}
