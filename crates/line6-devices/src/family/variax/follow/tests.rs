use super::*;
use crate::{
    gate::GatePolicy,
    identity::{REQUEST as IDENTITY, tests::VARIAX_600},
    session::{
        Quiet,
        testing::{Fake, packets},
    },
};
use std::{cell::Cell, time::Duration};

const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
const SESSION_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 1, 0xf7];
const SESSION_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7];
const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
const WRITES: [&[u8]; 5] = [&IDENTITY, BRIDGE_ON, SESSION_ON, SESSION_OFF, BRIDGE_OFF];

struct ReadPolicy {
    identity: bool,
    read: bool,
}

impl GatePolicy for ReadPolicy {
    fn allows(&self, operation: Operation) -> bool {
        match operation {
            Operation::VariaxIdentity => self.identity,
            Operation::VariaxRead => self.read,
            _ => false,
        }
    }
}

const READS: ReadPolicy = ReadPolicy {
    identity: true,
    read: true,
};

fn identity() -> [u8; 17] {
    let mut reply = VARIAX_600;
    reply[12..16].copy_from_slice(b"0310");
    reply
}

fn script(updates: Vec<Vec<u8>>) -> Fake {
    Fake::from_bytes(
        Path::VariaxViaPodxt,
        vec![
            (IDENTITY.into(), packets(&identity())),
            (BRIDGE_ON.into(), vec![]),
            (SESSION_ON.into(), updates),
            (SESSION_OFF.into(), vec![]),
            (BRIDGE_OFF.into(), vec![]),
        ],
    )
}

fn notice(id: u8, opcode: u8) -> Vec<u8> {
    vec![0xf0, 0, 1, 0x0c, 7, id, opcode, 0xf7]
}

#[test]
fn follows_knobs_presets_and_notices_in_wire_order_with_no_domain_sends() {
    let mut foreign_clip = notice(4, 0x28);
    foreign_clip[4] = 3;
    let mut updates = vec![
        foreign_clip,
        notice(0, 0x71),
        vec![0xb0, 10, 40, 7, 127], // CC 7 in running status.
        vec![0xb4, 79],             // Tone value arrives in the next USB packet.
        vec![43, 0xc4, 54, 105],    // Unmapped PC 54, then Reso 5 in running status.
        notice(0, 0x28),
    ];
    let overload = notice(4, 0x4d);
    updates.extend([overload[..4].to_vec(), overload[4..].to_vec()]);
    updates.push(notice(4, 0x74));

    let stopped = Cell::new(false);
    let mut received = Vec::new();
    let mut quiet = Quiet;
    let mut session = Session::new(script(updates), &READS, &mut quiet);
    follow_on(
        &mut session,
        None,
        || stopped.get(),
        |update| {
            received.push(update);
            if update == FollowUpdate::MidiQueueOverflow {
                stopped.set(true);
            }
        },
    )
    .unwrap();
    assert_eq!(
        received,
        [
            FollowUpdate::Volume(127),
            FollowUpdate::TonePosition(43),
            FollowUpdate::ProgramChange(Preset::new(10, 4).unwrap()),
            FollowUpdate::Clip,
            FollowUpdate::DspOverload,
            FollowUpdate::MidiQueueOverflow,
        ]
    );
    assert_eq!(session.link.writes, WRITES);
    session.link.assert_finished();
}

#[test]
fn follow_labels_all_sixty_programs_and_ignores_unmapped_values() {
    // Programs run backward within each five-position bank.
    for (bank, highest) in [4, 64, 69, 74, 79, 84, 89, 94, 99, 104, 109, 59]
        .into_iter()
        .enumerate()
    {
        for position in 0..5 {
            assert_eq!(
                decode(&Event::Short(vec![0xc0, highest - position])),
                Some(FollowUpdate::ProgramChange(
                    Preset::new(bank as u8, position).unwrap()
                ))
            );
        }
    }
    for unmapped in (5..55).chain(110..128) {
        assert_eq!(decode(&Event::Short(vec![0xc0, unmapped])), None);
    }
}

#[test]
fn follow_requires_both_read_gates_and_the_variax_path_before_sending() {
    let no_identity = ReadPolicy {
        identity: false,
        ..READS
    };
    let no_read = ReadPolicy {
        read: false,
        ..READS
    };
    for (path, policy) in [
        (Path::VariaxViaPodxt, &no_identity),
        (Path::VariaxViaPodxt, &no_read),
        (Path::PodxtMidi, &READS),
    ] {
        let mut quiet = Quiet;
        let mut session = Session::new(Fake::from_bytes(path, vec![]), policy, &mut quiet);
        assert!(
            follow_on(
                &mut session,
                None,
                || false,
                |_| panic!("unexpected update")
            )
            .is_err()
        );
        assert!(session.link.writes.is_empty());
    }
}

#[test]
fn follow_refuses_an_unverified_guitar_before_enabling_the_bridge() {
    for (index, value) in [(5, 1), (8, 3), (10, 0), (14, b'0')] {
        let mut wrong = identity();
        wrong[index] = value;
        let mut quiet = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(
                Path::VariaxViaPodxt,
                vec![(IDENTITY.into(), packets(&wrong))],
            ),
            &READS,
            &mut quiet,
        );
        assert!(
            follow_on(
                &mut session,
                None,
                || false,
                |_| panic!("unexpected update")
            )
            .is_err()
        );
        assert_eq!(session.link.writes, [IDENTITY]);
        session.link.assert_finished();
    }
}

#[test]
fn cancellation_before_identity_or_after_its_reply_stops_before_the_bridge() {
    for during_identity in [false, true] {
        let identity_script = during_identity
            .then(|| (IDENTITY.into(), packets(&identity())))
            .into_iter()
            .collect();
        let mut quiet = Quiet;
        let mut session = Session::new(
            Fake::from_bytes(Path::VariaxViaPodxt, identity_script),
            &READS,
            &mut quiet,
        );
        let checks = Cell::new(0);
        follow_on(
            &mut session,
            None,
            || {
                checks.set(checks.get() + 1);
                !during_identity || checks.get() > 1
            },
            |_| panic!("unexpected update"),
        )
        .unwrap();
        session.link.assert_finished();
    }
}

#[test]
fn cancel_while_idle_closes_the_session_and_releases_the_bridge() {
    let checks = Cell::new(0);
    let mut quiet = Quiet;
    let mut session = Session::new(script(vec![]), &READS, &mut quiet);
    follow_on(
        &mut session,
        None,
        || {
            checks.set(checks.get() + 1);
            checks.get() == 4
        },
        |_| panic!("unexpected update"),
    )
    .unwrap();
    assert_eq!(session.link.writes, WRITES);
    assert!(session.link.elapsed() >= Duration::from_millis(200));
    session.link.assert_finished();
}

struct Disconnect(Fake);

impl Link for Disconnect {
    fn path(&self) -> Path {
        self.0.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0.write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        if self.0.writes.len() >= 3 {
            Err("simulated disconnect".into())
        } else {
            self.0.read(wait)
        }
    }

    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.0.pause(duration);
    }
}

#[test]
fn disconnect_attempts_both_cleanup_sends_and_preserves_all_failures() {
    for fail_at in [vec![], vec![3, 4]] {
        let mut fake = script(vec![]);
        let fail_cleanup = !fail_at.is_empty();
        fake.fail_at = fail_at;
        let mut quiet = Quiet;
        let mut session = Session::new(Disconnect(fake), &READS, &mut quiet);
        let reported = Cell::new(false);
        let error = follow_on_reporting_cleanup(
            &mut session,
            None,
            || false,
            |_| panic!("unexpected update"),
            || reported.set(true),
        )
        .unwrap_err();
        assert!(error.contains("simulated disconnect"));
        assert_eq!(reported.get(), fail_cleanup);
        assert_eq!(error.contains("Session off also failed"), fail_cleanup);
        assert_eq!(
            error.contains("PODxt Variax-messages release failed"),
            fail_cleanup
        );
        assert_eq!(session.link.0.writes, WRITES);
        session.link.0.assert_finished();
    }
}

#[test]
fn cancellation_with_a_failed_session_off_still_releases_the_bridge() {
    let mut fake = script(vec![vec![0xb0, 7, 1]]);
    fake.fail_at = vec![3];
    let mut quiet = Quiet;
    let mut session = Session::new(fake, &READS, &mut quiet);
    let stopped = Cell::new(false);
    let error = follow_on(&mut session, None, || stopped.get(), |_| stopped.set(true)).unwrap_err();
    assert!(error.contains("Session off failed"));
    assert_eq!(session.link.writes, WRITES);
    session.link.assert_finished();
}

#[test]
fn an_assembler_error_still_closes_the_session_and_bridge() {
    let malformed = [vec![0xf0], vec![0; 8192]].concat();
    let mut quiet = Quiet;
    let mut session = Session::new(script(packets(&malformed)), &READS, &mut quiet);
    let error = follow_on(
        &mut session,
        None,
        || false,
        |_| panic!("unexpected update"),
    )
    .unwrap_err();
    assert!(error.contains("unexpectedly large SysEx"));
    assert_eq!(session.link.writes, WRITES);
    session.link.assert_finished();
}

fn connection(state: u8) -> Vec<u8> {
    // The state is byte 9. Adjacent nonzero bytes also appear on disconnect.
    vec![0xf0, 0, 1, 0x0c, 0x03, 0x11, 0, 2, 9, state, 0xf7]
}

fn disconnected() -> Vec<u8> {
    vec![0xf0, 0, 1, 0x0c, 0x7f, 0x03, 0, 0, 0, 6, 0, 0xf7]
}

#[test]
fn connection_state_is_reported_once_until_the_disconnect() {
    let connected = connection(2);
    let packets = vec![
        connected[..7].to_vec(),
        connected[7..].to_vec(),
        vec![0xb0, 7, 55],
        connection(1),
        connection(0),
    ];
    let mut quiet = Quiet;
    let mut session = Session::new(script(packets), &READS, &mut quiet);
    let mut updates = Vec::new();
    follow_on(&mut session, None, || false, |update| updates.push(update)).unwrap();
    assert_eq!(
        updates,
        [
            FollowUpdate::Connected,
            FollowUpdate::Volume(55),
            FollowUpdate::Disconnected
        ]
    );
    assert_eq!(session.link.writes, WRITES);
    session.link.assert_finished();
}

#[test]
fn either_disconnect_frame_stops_once_even_if_repeated_in_the_same_packet() {
    for frame in [connection(0), disconnected()] {
        let mut quiet = Quiet;
        let prefix = frame[..frame.len() - 1].to_vec();
        let packet = [vec![0xf7], frame, vec![0xb0, 79, 63]].concat();
        let mut session = Session::new(script(vec![prefix, packet]), &READS, &mut quiet);
        let mut updates = Vec::new();
        follow_on(&mut session, None, || false, |update| updates.push(update)).unwrap();
        assert_eq!(updates, [FollowUpdate::Disconnected]);
        assert_eq!(session.link.writes, WRITES);
        session.link.assert_finished();
    }
}

#[test]
fn malformed_lengths_and_unknown_frames_are_logged_and_ignored() {
    #[derive(Default)]
    struct Notes(Vec<String>);
    impl crate::session::Log for Notes {
        fn line(&mut self, line: String) {
            self.0.push(line);
        }
    }

    let mut unknown = Vec::new();
    for mut frame in [connection(0), disconnected()] {
        let index = frame.len() - 2;
        let mut short = frame.clone();
        short.remove(index);
        unknown.push(short);
        frame.insert(index, 0);
        unknown.push(frame);
    }
    let mut foreign = disconnected();
    foreign[3] = 0x0d;
    unknown.push(foreign);
    let mut unknown_opcode = connection(0);
    unknown_opcode[5] = 0x12;
    unknown.push(unknown_opcode);

    let mut notes = Notes::default();
    let mut packets = unknown.clone();
    packets.push(disconnected());
    let mut session = Session::new(script(packets), &READS, &mut notes);
    session.log_packets = false;

    let mut updates = Vec::new();
    follow_on(&mut session, None, || false, |update| updates.push(update)).unwrap();
    assert_eq!(updates, [FollowUpdate::Disconnected]);
    assert_eq!(session.link.writes, WRITES);
    session.link.assert_finished();
    let ignored: Vec<_> = notes
        .0
        .iter()
        .filter_map(|line| {
            line.split_once("Ignored unrecognized Follow SysEx frame: ")
                .map(|(_, raw)| raw)
        })
        .collect();
    assert_eq!(
        ignored,
        unknown.iter().map(|frame| hex(frame)).collect::<Vec<_>>()
    );
}

#[test]
fn follows_cc84_on_and_off_without_guessing_a_relay_subtype() {
    let frames = vec![
        vec![0xb0, 84, 127, 84, 0],
        vec![0xf0, 0, 1, 0x0c, 3, 7, 13, 2, 2, 0, 0, 0, 1, 0xf7],
        notice(4, 0x74),
    ];

    let stopped = Cell::new(false);
    let mut received = Vec::new();
    let mut quiet = Quiet;
    let mut session = Session::new(script(frames), &READS, &mut quiet);
    follow_on(
        &mut session,
        None,
        || stopped.get(),
        |update| {
            received.push(update);
            if update == FollowUpdate::MidiQueueOverflow {
                stopped.set(true);
            }
        },
    )
    .unwrap();
    assert_eq!(
        received,
        [
            FollowUpdate::TuningOverride(true),
            FollowUpdate::TuningOverride(false),
            FollowUpdate::MidiQueueOverflow,
        ]
    );
    assert_eq!(session.link.writes, WRITES);
    session.link.assert_finished();
}
