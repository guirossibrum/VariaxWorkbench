//! Expected fake-wire transcripts and synthetic program data.

use super::*;
use crate::session::Request;

pub(super) fn fixture_image(preset: Preset) -> Image {
    let mut image = [0; PROGRAM_BYTES];
    let key = preset.program_number();
    let mut settings = metadata(key);
    let length = settings[..18].iter().position(|&byte| byte == 0).unwrap();
    settings[length..18].fill(b' ');
    image[METADATA].copy_from_slice(&settings);
    image[0] = key;
    procedures::expected_stored_image(&image)
}

// Synthetic trim-only fixture with an otherwise empty DSP program. These
// bytes do not model trim processing or authorize editing captured DSP.
pub(super) fn fixture_edit(baseline: &Image) -> Image {
    let mut image = *baseline;
    image[METADATA.start + 65..][..3].copy_from_slice(&(-6.0_f32).to_be_bytes()[..3]);
    procedures::set_firmware_checksum(&mut image);
    image
}

pub(super) fn alter_program(mut image: Image) -> Image {
    image[3] ^= 1;
    procedures::set_firmware_checksum(&mut image);
    image
}

fn image_for_sweep(preset: Preset, edited: Option<&Image>, fault: Fault) -> Image {
    let mut image = fixture_image(preset);
    if preset == TARGET {
        if let Some(edited) = edited {
            return *edited;
        }
        if fault == Fault::InvalidRestoreStamp {
            image[767 * 3..771 * 3].fill(0);
            procedures::set_firmware_checksum(&mut image);
        }
    }
    if preset == START && matches!(fault, Fault::UnrelatedSlot | Fault::FinalComparison) {
        image = alter_program(image);
    }
    image
}

fn chunks(script: &mut Script, image: &Image) {
    for span in procedures::chunks() {
        let mut reply = [HEADER, &[0x3c]].concat();
        for word in image
            .as_chunks::<3>()
            .0
            .iter()
            .skip(usize::from(span.start()))
            .take(usize::from(span.count()))
        {
            reply.extend(
                std::iter::once(word[2])
                    .chain([word[1], word[0]])
                    .flat_map(|byte| [byte & 15, byte >> 4]),
            );
        }
        reply.push(0xf7);
        script.push((span.bytes(), packets(&reply)));
    }
}

fn read_script(script: &mut Script, selection: Option<Preset>, image: &Image) {
    script.extend([(BRIDGE_ON.into(), vec![]), (SESSION_ON.into(), vec![])]);
    if let Some(preset) = selection {
        script.push((vec![0xc0, preset.program_number()], vec![]));
    }
    chunks(script, image);
    end(script);
}

fn sweep_script(script: &mut Script, edited: Option<&Image>, fault: Fault) {
    script.extend(full_start(START.program_number()).into_iter().take(3));
    blocks(script, fixture_image(START)[METADATA].try_into().unwrap());
    for preset in Preset::all() {
        script.push((vec![0xc0, preset.program_number()], vec![]));
        let image = image_for_sweep(preset, edited, fault);
        let mut settings = <[u8; 192]>::try_from(&image[METADATA]).unwrap();
        // This is a different slot to the starting one, so the library's
        // own verified switch-back cannot hide the comparison fault.
        if preset.program_number() == 3 && fault == Fault::UnrelatedSettings {
            settings[64] ^= 1;
        }
        blocks(script, &settings);
        chunks(script, &image);
        if preset.slot() == 4 {
            let name = if preset.bank() == 6 && fault == Fault::UnrelatedBank {
                "Changed bank".into()
            } else {
                format!("Bank {}", preset.bank())
            };
            bank(script, packets(&bank_reply(name.as_bytes())));
        }
    }
    script.push((vec![0xc0, START.program_number()], vec![]));
    let mut returned = <[u8; 192]>::try_from(&fixture_image(START)[METADATA]).unwrap();
    if fault == Fault::IntermediateReturn {
        returned[64] ^= 1;
    }
    blocks(script, &returned);
    end(script);
}

fn sysex_parts(script: &mut Script, opcode: &[u8], reply: Vec<Vec<u8>>) {
    script.extend([
        (HEADER.into(), vec![]),
        (opcode.into(), vec![]),
        (vec![0xf7], reply),
    ]);
}

fn store_script(script: &mut Script, upload: &Image, fault: Fault) -> Option<usize> {
    script.extend([
        (BRIDGE_ON.into(), vec![]),
        (HEADER.into(), vec![]),
        (vec![0x2a, 1], vec![]),
        (vec![0xf7], vec![]),
    ]);
    sysex_parts(script, &[0x4c, 1], vec![]);
    let mut full = UPLOAD_HEADER.to_vec();
    let nibble = |byte: u8| [byte & 15, byte >> 4];
    full.extend([0x44, 0x04, 0x00].into_iter().flat_map(nibble));
    for [a, b, c] in upload.as_chunks::<3>().0 {
        full.extend([*c, *b, *a].into_iter().flat_map(nibble));
    }
    full.push(0xf7);
    script.extend(full.chunks(256).map(|part| (part.to_vec(), vec![])));
    let reply = match fault {
        Fault::EditSilent => vec![],
        Fault::EditUnexpectedReply => packets(&[HEADER, &[0x70, 0xf7]].concat()),
        _ => packets(&[HEADER, &[0x71, 0xf7]].concat()),
    };
    sysex_parts(script, SLOT_STORE, reply);
    let cleanup = script.len();
    sysex_parts(script, &[0x4c, 0], vec![]);
    sysex_parts(script, &[0x2a, 0], vec![]);
    script.push((BRIDGE_OFF.into(), vec![]));
    match fault {
        Fault::EditSilent | Fault::EditUnexpectedReply => {}
        Fault::EditCleanup => return Some(cleanup + 1),
        Fault::EditIncompleteReadBack => {
            script.extend([
                (BRIDGE_ON.into(), vec![]),
                (SESSION_ON.into(), vec![]),
                (vec![0xc0, TARGET.program_number()], vec![]),
            ]);
            let start = script.len();
            chunks(script, &procedures::expected_stored_image(upload));
            script.truncate(start + 2);
            script[start + 1].1.clear();
            end(script);
        }
        _ => {
            let expected = procedures::expected_stored_image(upload);
            let observed = if fault == Fault::RestoreMismatch {
                alter_program(expected)
            } else {
                expected
            };
            read_script(script, Some(TARGET), &observed);
        }
    }
    None
}

pub(super) fn stopped_script(fault: Fault, edited: &Image) -> (Script, Vec<usize>) {
    use Fault::*;
    let baseline = fixture_image(TARGET);
    let starting = fixture_image(START);
    let mut script = Vec::new();
    let mut fail_at = Vec::new();
    let active = if fault == InvalidStartingImage {
        alter_program(starting)
    } else {
        starting
    };
    read_script(&mut script, Option::None, &active);
    sweep_script(
        &mut script,
        Option::None,
        if fault == InvalidRestoreStamp {
            fault
        } else {
            None
        },
    );
    if matches!(
        fault,
        MissingProgram
            | DuplicateManifest
            | BadChecksum
            | InvalidRestoreStamp
            | InvalidStartingImage
            | UnrelatedEditByte
    ) {
        return (script, fail_at);
    }

    let preflight = if fault == TargetPreflight {
        alter_program(baseline)
    } else {
        baseline
    };
    read_script(&mut script, Some(TARGET), &preflight);
    if fault == TargetPreflight {
        return (script, fail_at);
    }
    let returned = if fault == PreflightReturn {
        alter_program(starting)
    } else {
        starting
    };
    read_script(&mut script, Some(START), &returned);
    if matches!(fault, PreflightReturn | EditConfirmation) {
        return (script, fail_at);
    }

    let edit_fault = if fault == RestoreMismatch {
        None
    } else {
        fault
    };
    fail_at.extend(store_script(&mut script, edited, edit_fault));
    if matches!(
        fault,
        EditSilent | EditUnexpectedReply | EditCleanup | EditIncompleteReadBack
    ) {
        return (script, fail_at);
    }
    read_script(&mut script, Some(START), &starting);
    // FinalComparison belongs only to the final sweep.
    let intermediate_fault = if fault == FinalComparison {
        None
    } else {
        fault
    };
    sweep_script(&mut script, Some(edited), intermediate_fault);
    if matches!(
        fault,
        IntermediateReturn
            | IntermediateCleanup
            | UnrelatedSlot
            | UnrelatedSettings
            | UnrelatedBank
    ) {
        return (script, fail_at);
    }

    let target = if fault == RestorePreflight {
        alter_program(*edited)
    } else {
        *edited
    };
    read_script(&mut script, Some(TARGET), &target);
    if fault == RestorePreflight {
        return (script, fail_at);
    }
    read_script(&mut script, Some(START), &starting);
    if fault == RestoreConfirmation {
        return (script, fail_at);
    }
    store_script(&mut script, &baseline, fault);
    if fault == RestoreMismatch {
        return (script, fail_at);
    }
    read_script(&mut script, Some(START), &starting);
    sweep_script(&mut script, Option::None, fault);
    if fault != FinalComparison {
        read_script(&mut script, Some(START), &starting);
    }
    (script, fail_at)
}
