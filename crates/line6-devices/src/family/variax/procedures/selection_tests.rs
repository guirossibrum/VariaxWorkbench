use super::*;
use crate::{
    gate::GatePolicy,
    session::{
        Path, Quiet, Request,
        testing::{Fake, packets},
    },
};
use std::cell::Cell;

type Script = Vec<(Vec<u8>, Vec<Vec<u8>>)>;

struct SelectionPolicy {
    recall: bool,
    upload: bool,
    read: bool,
}

impl GatePolicy for SelectionPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxPresetSwitch => self.recall,
            Operation::VariaxEditBufferWrite => self.upload,
            Operation::VariaxRead => self.read,
            _ => false,
        }
    }
}

const RECALL: SelectionPolicy = SelectionPolicy {
    recall: true,
    upload: false,
    read: true,
};
const UPLOAD: SelectionPolicy = SelectionPolicy {
    recall: false,
    upload: true,
    read: true,
};
const TARGET: [u8; 2] = [0xc0, 64]; // T-Model 1
const ORIGINAL: [u8; 2] = [0xc0, 55]; // Custom 2, position 5

fn start(script: &mut Script) {
    script.extend([
        (vec![0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7], vec![]),
        (vec![0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 1, 0xf7], vec![]),
    ]);
}

fn finish(script: &mut Script) {
    script.extend([
        (vec![0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7], vec![]),
        (vec![0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7], vec![]),
    ]);
}

fn nibbles(words: &[[u8; 3]]) -> impl Iterator<Item = u8> {
    words
        .iter()
        .flat_map(|word| word.iter().rev().flat_map(|byte| [byte & 15, byte >> 4]))
}

fn span_step(span: PresetSpan, image: &Image) -> (Vec<u8>, Vec<Vec<u8>>) {
    let start = usize::from(span.start()) * 3;
    let end = start + usize::from(span.count()) * 3;
    let reply: Vec<_> = [0xf0, 0, 1, 0x0c, 7, 0, 0x3c]
        .into_iter()
        .chain(nibbles(image[start..end].as_chunks::<3>().0))
        .chain([0xf7])
        .collect();
    (span.bytes(), packets(&reply))
}

fn readback(script: &mut Script, program_change: Option<[u8; 2]>, image: &Image) {
    start(script);
    if let Some(pc) = program_change {
        script.push((pc.to_vec(), vec![]));
    }
    script.extend(chunks().into_iter().map(|span| span_step(span, image)));
    finish(script);
}

fn edited_image() -> Image {
    let mut image = [0; PROGRAM_BYTES];
    image[METADATA.start..METADATA.start + 18].copy_from_slice(b"Edited model      ");
    set_firmware_checksum(&mut image);
    image
}

fn upload_packets(image: &Image) -> Vec<Vec<u8>> {
    let frame: Vec<_> = [0xf0, 0, 1, 0x0c, 7, 0, 0x43, 4, 4, 4, 0, 0, 0]
        .into_iter()
        .chain(nibbles(image.as_chunks::<3>().0))
        .chain([0xf7])
        .collect();
    frame.chunks(256).map(<[u8]>::to_vec).collect()
}

fn verify_upload(
    session: &mut Session<Fake>,
    program: &Program<'_>,
    stopping: impl FnMut() -> bool,
) -> Result<Image, String> {
    send_buffer_verified(
        session,
        BufferEdit::Full(program),
        "Selection upload readback",
        stopping,
        || Ok(|_| Ok(())),
    )
}

#[test]
fn synchronized_slot_recalls_once_and_compares_the_full_device_image() {
    assert!(Preset::new(12, 0).is_none());
    assert!(Preset::new(0, 5).is_none());
    let mut device_image = edited_image();
    device_image[PROGRAM_BYTES - 1] ^= 1;
    let mut script = vec![];
    readback(&mut script, Some(TARGET), &device_image);
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &RECALL,
        &mut log,
    );

    let actual = read_unmodified(
        &mut session,
        Preset::new(1, 0).unwrap(),
        || false,
        |_| Ok(()),
    )
    .unwrap();
    compare_image(&actual, &device_image, "Recall readback").unwrap();
    let error = compare_image(&actual, &edited_image(), "Recall readback").unwrap_err();
    assert!(error.contains("differs at word 1091"));
    session.link.assert_finished();
}

#[test]
fn edited_model_uploads_one_working_buffer_and_verifies_it_without_recall() {
    let image = edited_image();
    let program = Program::new("Edited model", &image).unwrap();
    let mut different = image;
    different[PROGRAM_BYTES - 1] ^= 1;

    for actual in [&image, &different] {
        let mut script = vec![];
        start(&mut script);
        script.extend(
            upload_packets(&image)
                .into_iter()
                .map(|packet| (packet, vec![])),
        );
        finish(&mut script);
        readback(&mut script, None, actual);
        let mut log = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::VariaxViaPodxt, script),
            &UPLOAD,
            &mut log,
        );

        let result = verify_upload(&mut session, &program, || false);
        if actual == &image {
            assert_eq!(result.unwrap(), image);
        } else {
            assert!(result.unwrap_err().contains("differs at word 1091"));
        }
        session.link.assert_finished();
    }
}

#[test]
fn gates_wrong_route_and_preflight_cancellation_send_nothing() {
    let image = edited_image();
    let program = Program::new("Edited model", &image).unwrap();
    let without_read = SelectionPolicy {
        read: false,
        ..RECALL
    };
    for (path, policy, cancelled, error) in [
        (Path::VariaxViaPodxt, &UPLOAD, false, "VariaxPresetSwitch"),
        (Path::VariaxViaPodxt, &without_read, false, "VariaxRead"),
        (Path::PodxtMidi, &RECALL, false, "bridge"),
        (Path::VariaxViaPodxt, &RECALL, true, "cancelled"),
    ] {
        let mut log = Quiet;
        let mut session = Session::new(Fake::from_bytes(path, vec![]), policy, &mut log);
        let result = read_unmodified(
            &mut session,
            Preset::new(1, 0).unwrap(),
            || cancelled,
            |_| Ok(()),
        );
        assert!(result.unwrap_err().contains(error));
        session.link.assert_finished();
    }
    let without_read = SelectionPolicy {
        read: false,
        ..UPLOAD
    };
    for (path, policy, cancelled, error) in [
        (Path::VariaxViaPodxt, &RECALL, false, "edit gates"),
        (Path::VariaxViaPodxt, &without_read, false, "VariaxRead"),
        (Path::PodxtMidi, &UPLOAD, false, "bridged"),
        (Path::VariaxViaPodxt, &UPLOAD, true, "stopped"),
    ] {
        let mut log = Quiet;
        let mut session = Session::new(Fake::from_bytes(path, vec![]), policy, &mut log);
        assert!(
            verify_upload(&mut session, &program, || cancelled)
                .unwrap_err()
                .contains(error)
        );
        session.link.assert_finished();
    }
}

#[test]
fn cancelled_recall_cleans_up_before_a_separate_checked_return() {
    let original = [0; PROGRAM_BYTES];
    let mut script = vec![];
    start(&mut script);
    script.push((TARGET.to_vec(), vec![]));
    script.push(span_step(chunks()[0], &edited_image()));
    finish(&mut script);
    let forward_writes = script.len();
    readback(&mut script, Some(ORIGINAL), &original);
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &RECALL,
        &mut log,
    );
    let stopping = Cell::new(false);

    let error = read_unmodified(
        &mut session,
        Preset::new(1, 0).unwrap(),
        || stopping.get(),
        |_| {
            stopping.set(true);
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.contains("cancelled"));
    assert_eq!(session.link.writes.len(), forward_writes);
    return_to_preset(&mut session, Preset::new(11, 4).unwrap(), &original, |_| {
        Ok(())
    })
    .unwrap();
    session.link.assert_finished();
}

#[test]
fn cancelled_partial_upload_terminates_then_releases_without_automatic_return() {
    let image = edited_image();
    let program = Program::new("Edited model", &image).unwrap();
    let mut script = vec![];
    start(&mut script);
    script.push((upload_packets(&image).remove(0), vec![]));
    script.push((vec![0xf7], vec![]));
    finish(&mut script);
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &UPLOAD,
        &mut log,
    );
    let checks = Cell::new(0);

    let error = verify_upload(&mut session, &program, || {
        checks.set(checks.get() + 1);
        checks.get() > 2
    })
    .unwrap_err();
    assert!(error.contains("stopped"));
    session.link.assert_finished();
}

#[test]
fn transport_failure_and_cleanup_failure_both_reach_the_caller() {
    let mut script = vec![];
    start(&mut script);
    script.push((TARGET.to_vec(), vec![]));
    finish(&mut script);
    let mut fake = Fake::from_bytes(Path::VariaxViaPodxt, script);
    fake.fail_at = vec![2, 3, 4]; // PC, session off, bridge off
    let mut log = Quiet;
    let mut session = Session::new(fake, &RECALL, &mut log);

    let error = read_unmodified(
        &mut session,
        Preset::new(1, 0).unwrap(),
        || false,
        |_| Ok(()),
    )
    .unwrap_err();
    assert!(error.starts_with("simulated write failure"));
    assert!(error.contains("Session off also failed"));
    assert!(error.contains("Variax-messages release failed"));
    session.link.assert_finished();
}

#[test]
fn incoming_program_change_invalidates_active_buffer_readback() {
    let mut script = vec![];
    start(&mut script);
    script.push((TARGET.to_vec(), vec![]));
    script.push((chunks()[0].bytes(), vec![ORIGINAL.to_vec()]));
    finish(&mut script);
    let mut log = Quiet;
    let mut session = Session::new(
        Fake::from_bytes(Path::VariaxViaPodxt, script),
        &RECALL,
        &mut log,
    );

    let error = read_unmodified(
        &mut session,
        Preset::new(1, 0).unwrap(),
        || false,
        |_| panic!("intervening program change must invalidate readback"),
    )
    .unwrap_err();
    assert!(error.contains("Unexpected MIDI message"));
    session.link.assert_finished();
}
