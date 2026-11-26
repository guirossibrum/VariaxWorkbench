use super::*;
use crate::rig_check::catalogue::StepId;
use crate::rig_check::procedures::{Lines, PODXT_CHANNEL, StepPolicy};
use crate::rig_check::record::Mode;
use line6_devices::session::{Link, Path, Request, Session};
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

type Packets = Vec<(Duration, Vec<u8>)>;

#[derive(Default)]
struct FakeLink {
    time: Rc<Cell<Duration>>,
    attempts: VecDeque<Packets>,
    input: VecDeque<(Duration, Vec<u8>)>,
    writes: Vec<Vec<u8>>,
}

impl Link for FakeLink {
    fn path(&self) -> Path {
        Path::PodxtMidi
    }

    fn elapsed(&self) -> Duration {
        self.time.get()
    }

    fn pause(&mut self, duration: Duration) {
        self.time.set(self.elapsed() + duration);
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writes.push(bytes.to_vec());
        for (delay, packet) in self.attempts.pop_front().expect("unexpected request") {
            self.input.push_back((self.elapsed() + delay, packet));
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let end = self.elapsed() + wait;
        if self.input.front().is_some_and(|(due, _)| *due <= end) {
            let (due, packet) = self.input.pop_front().unwrap();
            self.time.set(due);
            Ok(Some(packet))
        } else {
            self.time.set(end);
            Ok(None)
        }
    }
}

fn frame(marker: u8) -> Vec<u8> {
    let mut bytes = TONE_HEADER.to_vec();
    bytes.push(podxt::PODXT_LIVE_MEMBER as u8);
    bytes.resize(podxt::EDIT_BUFFER_LEN - 1, marker);
    bytes.push(0xf7);
    bytes
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn exercise(
    read: ToneRead,
    scripts: Vec<Packets>,
    stop_at: Duration,
) -> (Result<EditBuffer, String>, FakeLink, Vec<String>) {
    let mut link = FakeLink {
        attempts: scripts.into(),
        ..FakeLink::default()
    };
    let mut lines = Lines::default();
    let policy = StepPolicy {
        step: StepId(7, 4),
        mode: Mode::Practice,
        ops: if matches!(read, ToneRead::Current) {
            PODXT_CHANNEL
        } else {
            PODXT_LIBRARY
        },
    };
    let clock = link.time.clone();
    let mut session = Session::new(&mut link as &mut dyn Link, &policy, &mut lines);
    let result = read_tone(&mut session, &|| clock.get() >= stop_at, read);
    (result, link, lines.0)
}

#[test]
fn retries_the_same_request_after_an_incomplete_reply() {
    for read in [ToneRead::Current, ToneRead::Stored(channel())] {
        let mut short = frame(1);
        short.truncate(40);
        short.push(0xf7);
        let good = frame(2);
        let (result, link, log) = exercise(
            read,
            vec![vec![(ms(10), short)], vec![(ms(20), good.clone())]],
            Duration::MAX,
        );
        assert_eq!(result.unwrap().raw, good);
        assert_eq!(link.writes, vec![read.request().bytes(); 2]);
        assert!(log.iter().any(|line| {
            line.contains("attempt 2/3") && line.contains("41 bytes, expected 168")
        }));
    }
}

#[test]
fn drain_discards_late_complete_dumps_and_resets_partial_frames() {
    let stale = frame(1);
    let fresh = frame(2);
    let (result, link, _) = exercise(
        ToneRead::Current,
        vec![
            vec![
                (ms(10), stale[..50].to_vec()),
                (ms(2_050), stale[50..].to_vec()),
                (ms(2_100), stale.clone()),
                (ms(2_200), stale[..50].to_vec()),
                (ms(2_350), stale[50..].to_vec()),
            ],
            vec![(ms(75), fresh.clone())],
        ],
        Duration::MAX,
    );
    assert_eq!(result.unwrap().raw, fresh);
    assert_eq!(link.writes, vec![PodxtRequest::EditBuffer.bytes(); 2]);
}

#[test]
fn exhaustion_sends_three_reads_and_reports_the_last_failure() {
    let (result, link, log) = exercise(
        ToneRead::Current,
        vec![vec![], vec![], vec![]],
        Duration::MAX,
    );
    assert_eq!(
        result.unwrap_err(),
        "No 168-byte edit-buffer dump arrived. The PODxt tone read used all 3 attempts."
    );
    assert_eq!(link.writes, vec![PodxtRequest::EditBuffer.bytes(); 3]);
    assert_eq!(
        log.iter()
            .filter(|line| line.contains("Retrying PODxt"))
            .count(),
        2
    );
}

#[test]
fn a_complete_different_reply_fails_without_retry() {
    for offset in [5, 6] {
        let mut different = frame(1);
        different[offset] ^= 1;
        let (result, link, _) = exercise(
            ToneRead::Current,
            vec![vec![(ms(10), different)]],
            Duration::MAX,
        );
        assert!(result.unwrap_err().contains("Unexpected PODxt tone reply"));
        assert_eq!(link.writes, [PodxtRequest::EditBuffer.bytes()]);
    }
}

#[test]
fn stop_during_a_read_or_drain_prevents_any_resend() {
    for stop_at in [ms(0), ms(100), ms(2_100)] {
        let (result, link, log) = exercise(ToneRead::Current, vec![vec![]], stop_at);
        assert!(result.unwrap_err().starts_with(STOPPED));
        assert_eq!(link.writes.len(), usize::from(!stop_at.is_zero()));
        assert!(!log.iter().any(|line| line.contains("Retrying PODxt")));
    }
}
