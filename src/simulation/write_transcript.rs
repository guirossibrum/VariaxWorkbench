//! Test-only checks for #175 host traffic. The updater receiver does not
//! implement the application write handlers, so this replay records those gaps.

use super::{ModelError, Profile, Receiver, Stage, VirtualControl, VirtualSessionState};
use std::collections::BTreeSet;

const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
const FULL_COUNT: &[u8] = &[4, 4, 4, 0, 0, 0]; // 1092 low-first nibbles

#[derive(Debug, PartialEq, Eq)]
enum ReplayError {
    Framing,
    Truncated,
    WrongCount,
    InvalidShape,
    StoreWithoutFull,
}

fn replay(chunks: impl IntoIterator<Item = Vec<u8>>) -> Result<BTreeSet<u8>, ReplayError> {
    let mut control = VirtualControl::new([0; 192]).unwrap();
    // The virtual read-session fixture requires identification before 0x2A.
    // #175 assumes a known target and contains no identity request.
    control
        .route(1, &crate::read_session::IDENTITY_REQUEST)
        .unwrap();
    let mut frame = Vec::new();
    let mut gaps = BTreeSet::new();
    let mut full_seen = false;
    for chunk in chunks {
        match chunk.as_slice() {
            [0xb0, 79, 0..=127] | [0xc0, 0..=127] if frame.is_empty() => continue,
            _ => {}
        }
        for byte in chunk {
            match byte {
                0xf0 if frame.is_empty() => frame.push(byte),
                0..=127 if !frame.is_empty() && frame.len() < 8192 => frame.push(byte),
                0xf7 if !frame.is_empty() => {
                    frame.push(byte);
                    if frame == BRIDGE_ON || frame == BRIDGE_OFF {
                        full_seen = false;
                    } else {
                        check_frame(&frame, &mut control, &mut full_seen, &mut gaps)?;
                    }
                    frame.clear();
                }
                _ => return Err(ReplayError::Framing),
            }
        }
    }
    if !frame.is_empty() {
        return Err(ReplayError::Truncated);
    }
    assert_eq!(control.state(), VirtualSessionState::Disabled);
    Ok(gaps)
}

fn check_frame(
    frame: &[u8],
    control: &mut VirtualControl,
    full_seen: &mut bool,
    gaps: &mut BTreeSet<u8>,
) -> Result<(), ReplayError> {
    if frame.len() < 8 || !frame.starts_with(&crate::HEADER) {
        return Err(ReplayError::Framing);
    }
    let command = frame[6];
    let body = &frame[7..frame.len() - 1];
    match command {
        0x2a => {
            control
                .route(1, frame)
                .map_err(|_| ReplayError::InvalidShape)?;
            if body == [0] {
                *full_seen = false;
            }
            return Ok(());
        }
        0x43 => {
            let count = body.get(..6).ok_or(ReplayError::Truncated)?;
            if count != FULL_COUNT {
                return Err(ReplayError::WrongCount);
            }
            if body.len() < 6 + 1092 * 6 {
                return Err(ReplayError::Truncated);
            }
            if body.len() != 6 + 1092 * 6 || body.iter().any(|&byte| byte > 15) {
                return Err(ReplayError::InvalidShape);
            }
            *full_seen = true;
        }
        0x3b if body.len() >= 18
            && body.len().is_multiple_of(6)
            && body.iter().all(|&byte| byte <= 15) => {}
        0x4c if matches!(body, [0 | 1]) => {}
        0x61 if matches!(body, [0..=59]) => {
            if !*full_seen {
                return Err(ReplayError::StoreWithoutFull);
            }
            *full_seen = false;
        }
        0x6c if body.len() == 18 => {}
        0x6d if body.is_empty() => {}
        _ => return Err(ReplayError::InvalidShape),
    }

    // No application write op is a Variax updater receiver stage. Keep that
    // boundary explicit instead of inferring firmware behavior from the host.
    let mut receiver = Receiver::new(Profile::Variax600, Stage::MainCode).unwrap();
    assert_eq!(receiver.feed(frame), Err(ModelError::WrongStage));
    assert!(!receiver.report().possible_persistent_change());
    assert_eq!(receiver.report().modeled_acknowledgment(), None);
    gaps.insert(command);
    Ok(())
}

fn guitar(command: u8, body: &[u8]) -> Vec<u8> {
    [crate::HEADER.as_slice(), &[command], body, &[0xf7]].concat()
}

fn full() -> Vec<u8> {
    guitar(0x43, &[FULL_COUNT, &[0; 1092 * 6]].concat())
}

#[test]
fn host_shape_checks_reject_short_full_wrong_count_missing_eox_and_store_without_full() {
    let mut short = full();
    short.drain(20..26);
    let mut wrong_count = full();
    wrong_count[7] = 5;
    let mut missing_eox = full();
    missing_eox.pop();
    let store = guitar(0x61, &[50]);

    for (frames, expected) in [
        (vec![short], ReplayError::Truncated),
        (vec![wrong_count], ReplayError::WrongCount),
        (vec![missing_eox], ReplayError::Truncated),
        (vec![store.clone()], ReplayError::StoreWithoutFull),
        (
            vec![full(), guitar(0x2a, &[0]), store],
            ReplayError::StoreWithoutFull,
        ),
    ] {
        let chunks = frames
            .into_iter()
            .flat_map(|frame| frame.chunks(256).map(<[u8]>::to_vec).collect::<Vec<_>>());
        assert_eq!(replay(chunks), Err(expected));
    }
}

#[test]
#[ignore = "requires original Workbench transcripts generated by verify_variax_write.py"]
fn original_write_transcripts() {
    let root = std::env::var_os("VARIAX_WRITE_REFERENCE").expect("run verify_variax_write.py");
    let root = std::path::Path::new(&root);
    for index in 0..60 {
        for (kind, expected) in [
            ("edit", &[0x3b, 0x43, 0x4c][..]),
            ("store", &[0x43, 0x4c, 0x61]),
            ("bank", &[0x4c, 0x6c, 0x6d]),
        ] {
            let path = root.join(format!("reference-{index}-{kind}.txt"));
            let transcript = std::fs::read_to_string(path).unwrap();
            let chunks = transcript.lines().filter_map(|line| {
                let (action, data) = line.split_once(' ').unwrap();
                match action {
                    "out" => Some(
                        data.as_bytes()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|pair| {
                                u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()
                            })
                            .collect::<Vec<_>>(),
                    ),
                    "pause" => None,
                    _ => panic!("unknown transcript event {action}"),
                }
            });
            let gaps =
                replay(chunks).unwrap_or_else(|error| panic!("slot {index} {kind}: {error:?}"));
            assert_eq!(gaps, expected.iter().copied().collect());
        }
    }
}
