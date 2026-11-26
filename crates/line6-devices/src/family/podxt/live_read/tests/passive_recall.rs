use super::*;
use crate::session::{Path, Request};

const EXPECTED: u8 = 41;
const OTHER: u8 = 40;
const LISTEN: Duration = Duration::from_secs(1);
const CANCELLED: &str = "PODxt read cancelled.";

fn channel(program: u8) -> Location {
    Location::new(BankKind::Tones, program.into()).unwrap()
}

// Physical messages arrive at their scheduled times, independently of writes.
// The shared fake still checks the exact requests and supplies their replies.
struct PhysicalRecall {
    device: Fake,
    physical: VecDeque<(Duration, Vec<u8>)>,
    write_at: Vec<Duration>,
}

impl PhysicalRecall {
    fn new(
        physical: impl IntoIterator<Item = (u64, Vec<u8>)>,
        reply: Option<Vec<Vec<u8>>>,
    ) -> Self {
        let script = reply
            .map(|packets| (PodxtRequest::EditBuffer, packets))
            .into_iter()
            .collect();
        Self {
            device: Fake::new(Path::PodxtMidi, script),
            physical: physical
                .into_iter()
                .map(|(millis, bytes)| (Duration::from_millis(millis), bytes))
                .collect(),
            write_at: Vec::new(),
        }
    }

    fn listen(&mut self, window: Duration) -> Result<Option<EditBuffer>, String> {
        let mut log = Quiet;
        let mut session = Session::new(self, &ReadsOnly { stored: false }, &mut log);
        listen_channel_recall(&mut session, channel(EXPECTED), window, || false, |_| {})
    }

    fn assert_one_read_at(&self, millis: u64) {
        assert_eq!(self.write_at, [Duration::from_millis(millis)]);
        assert_eq!(self.device.writes, [PodxtRequest::EditBuffer.bytes()]);
    }
}

impl Link for PhysicalRecall {
    fn path(&self) -> Path {
        self.device.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.write_at.push(self.elapsed());
        self.device.write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let now = self.elapsed();
        if let Some((at, _)) = self.physical.front()
            && *at <= now + wait
        {
            let (at, bytes) = self.physical.pop_front().unwrap();
            self.pause(at.saturating_sub(now));
            return Ok(Some(bytes));
        }
        self.device.read(wait)
    }

    fn elapsed(&self) -> Duration {
        self.device.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.device.pause(duration);
    }
}

#[test]
fn waits_for_latest_matching_notice_to_settle_and_reads_once() {
    let expected = tone(b"Expected tone");
    let mut reply = vec![vec![0xb0, 7, 50]];
    reply.extend(packets(&expected));
    let mut fake = PhysicalRecall::new(
        [
            (7, vec![0xc0, OTHER]),
            (9, vec![0xb0, 7, 50]),
            (10, tone(b"Unsolicited")),
            (20, vec![0xc0, EXPECTED]),
            (30, vec![OTHER]), // Running status supersedes the target.
            (60, vec![EXPECTED]),
            (80, vec![EXPECTED]), // Another notice restarts the settle.
            (100, vec![0xb0, 19, 3]),
        ],
        Some(reply),
    );
    let mut observed = Vec::new();
    let mut log = Quiet;
    let mut session = Session::new(&mut fake, &ReadsOnly { stored: false }, &mut log);
    let readback = listen_channel_recall(
        &mut session,
        channel(EXPECTED),
        LISTEN,
        || false,
        |location| observed.push(location),
    )
    .unwrap()
    .unwrap();

    assert_eq!(readback.raw, expected);
    assert_eq!(
        observed,
        [OTHER, EXPECTED, OTHER, EXPECTED, EXPECTED].map(channel)
    );
    fake.assert_one_read_at(180);
    fake.device.assert_finished();
}

#[test]
fn passive_window_expires_without_sending_if_the_expected_channel_never_settles() {
    for notices in [
        vec![],
        vec![(7, vec![0xc0, OTHER]), (10, vec![0xb0, 19, 3])],
        vec![(7, vec![0xc0, EXPECTED, OTHER])],
        vec![(950, vec![0xc0, EXPECTED])],
    ] {
        let mut fake = PhysicalRecall::new(notices, None);
        assert_eq!(fake.listen(LISTEN), Ok(None));
        assert_eq!(fake.elapsed(), LISTEN);
        assert!(fake.device.writes.is_empty());
        fake.device.assert_finished();
    }
}

#[test]
fn missing_or_partial_reply_times_out_without_retry() {
    let expected = tone(b"Expected tone");
    for reply in [vec![], packets(&expected[16..]), packets(&expected[..160])] {
        let mut fake = PhysicalRecall::new(
            [(7, vec![0xc0, EXPECTED]), (9, vec![0xb0, 7, 50])],
            Some(reply),
        );
        assert_eq!(
            fake.listen(LISTEN),
            Err("No 168-byte edit-buffer dump arrived.".into())
        );
        assert_eq!(fake.elapsed(), Duration::from_millis(107) + WINDOW);
        fake.assert_one_read_at(107);
        fake.device.assert_finished();
    }
}

#[test]
fn program_change_during_read_rejects_the_dump_even_in_its_packet() {
    for program in [OTHER, EXPECTED] {
        let mut mixed = tone(b"Ambiguous tone");
        mixed.extend([0xc0, program]);
        let mut partial = packets(&tone(b"Interrupted tone")[..16]);
        partial.push(vec![0xc0, program]);
        for reply in [partial, vec![mixed]] {
            let mut fake = PhysicalRecall::new([(7, vec![0xc0, EXPECTED])], Some(reply));
            let error = fake.listen(LISTEN).unwrap_err();
            assert!(error.contains("interrupted the edit-buffer readback"));
            fake.assert_one_read_at(107);
            fake.device.assert_finished();
        }
    }
}

#[test]
fn running_status_program_during_read_also_invalidates_readback() {
    let mut fake = PhysicalRecall::new(
        [(7, vec![0xc0, EXPECTED])],
        Some(vec![vec![OTHER], tone(b"Invalidated tone")]),
    );
    let error = fake.listen(LISTEN).unwrap_err();
    assert!(error.contains("interrupted the edit-buffer readback"));
    fake.assert_one_read_at(107);
}

#[test]
fn two_dumps_in_one_packet_are_ambiguous() {
    let mut reply = tone(b"First");
    reply.extend(tone(b"Second"));
    let mut fake = PhysicalRecall::new([(7, vec![0xc0, EXPECTED])], Some(vec![reply]));
    assert_eq!(
        fake.listen(LISTEN),
        Err("More than one PODxt edit-buffer dump arrived in a packet.".into())
    );
    fake.assert_one_read_at(107);
    fake.device.assert_finished();
}

#[test]
fn cancellation_before_or_during_settle_sends_nothing() {
    for stop_after_notice in [false, true] {
        let mut fake = PhysicalRecall::new([(7, vec![0xc0, EXPECTED])], None);
        let cancelled = Cell::new(!stop_after_notice);
        let mut log = Quiet;
        let mut session = Session::new(&mut fake, &ReadsOnly { stored: false }, &mut log);
        assert_eq!(
            listen_channel_recall(
                &mut session,
                channel(EXPECTED),
                LISTEN,
                || cancelled.get(),
                |_| cancelled.set(true),
            ),
            Err(CANCELLED.into())
        );
        assert!(fake.device.writes.is_empty());
    }
}

#[test]
fn cancellation_when_a_dump_arrives_discards_the_reply() {
    struct StopOnReply<'a> {
        fake: PhysicalRecall,
        cancelled: &'a Cell<bool>,
    }

    impl Link for StopOnReply<'_> {
        fn path(&self) -> Path {
            self.fake.path()
        }

        fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.fake.write(bytes)
        }

        fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
            let packet = self.fake.read(wait)?;
            if !self.fake.write_at.is_empty() {
                self.cancelled.set(true);
            }
            Ok(packet)
        }

        fn elapsed(&self) -> Duration {
            self.fake.elapsed()
        }

        fn pause(&mut self, duration: Duration) {
            self.fake.pause(duration);
        }
    }

    let cancelled = Cell::new(false);
    let link = StopOnReply {
        fake: PhysicalRecall::new(
            [(7, vec![0xc0, EXPECTED])],
            Some(vec![tone(b"Cancelled tone")]),
        ),
        cancelled: &cancelled,
    };
    let mut log = Quiet;
    let mut session = Session::new(link, &ReadsOnly { stored: false }, &mut log);
    assert_eq!(
        listen_channel_recall(
            &mut session,
            channel(EXPECTED),
            LISTEN,
            || cancelled.get(),
            |_| {}
        ),
        Err(CANCELLED.into())
    );
    session.link.fake.assert_one_read_at(107);
    session.link.fake.device.assert_finished();
}

#[test]
fn refuses_wrong_link_target_or_missing_permission_before_polling() {
    struct Only(Operation);

    impl GatePolicy for Only {
        fn allows(&self, operation: Operation) -> bool {
            operation == self.0
        }
    }

    for (path, target, policy) in [
        (
            Path::VariaxViaPodxt,
            channel(EXPECTED),
            &ReadsOnly { stored: false } as &dyn GatePolicy,
        ),
        (
            Path::PodxtMidi,
            Location::new(BankKind::AmpSetups, 0).unwrap(),
            &ReadsOnly { stored: false },
        ),
        (
            Path::PodxtMidi,
            channel(EXPECTED),
            &Only(Operation::PodxtIdentity),
        ),
        (
            Path::PodxtMidi,
            channel(EXPECTED),
            &Only(Operation::PodxtRead),
        ),
    ] {
        let fake = Fake::new::<PodxtRequest>(path, vec![]);
        let mut log = Quiet;
        let mut session = Session::new(fake, policy, &mut log);
        assert!(
            listen_channel_recall(&mut session, target, LISTEN, || false, |_| unreachable!())
                .is_err()
        );
        assert_eq!(session.link.elapsed(), Duration::ZERO);
        assert!(session.link.writes.is_empty());
    }
}
