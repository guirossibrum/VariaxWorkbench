use super::*;

const END: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x72, 0xf7];
type Packets = Vec<(u64, Vec<u8>)>;

fn at(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

pub(in super::super) fn identity_reply() -> (PodxtRequest, Packets) {
    (
        PodxtRequest::Identity,
        vec![(1, variax_workbench::identity_stage::bridge_reply())],
    )
}

/// Replies are scheduled relative to each request. Late input from an older
/// request remains queued when a new request is sent.
pub(in super::super) struct TimedLink<'a> {
    pub held: &'a Cell<bool>,
    pub writes: &'a Cell<usize>,
    pub script: VecDeque<(PodxtRequest, Packets)>,
    pub queue: Vec<(Duration, Vec<u8>)>,
    pub now: Duration,
    pub stop: &'a AtomicBool,
    pub stop_at: Duration,
}

impl Link for TimedLink<'_> {
    fn path(&self) -> MidiPath {
        MidiPath::PodxtMidi
    }

    fn write(&mut self, frame: &[u8]) -> Result<(), String> {
        assert!(self.held.get());
        let (request, replies) = self.script.pop_front().expect("unexpected request");
        assert_eq!(frame, request.bytes());
        self.writes.set(self.writes.get() + 1);
        self.queue.extend(
            replies
                .into_iter()
                .map(|(ms, packet)| (self.now + at(ms), packet)),
        );
        self.queue.sort_by_key(|(due, _)| *due);
        Ok(())
    }

    fn read(&mut self, duration: Duration) -> Result<Option<Vec<u8>>, String> {
        assert!(self.held.get());
        let deadline = self.now + duration;
        let reply = if self.queue.first().is_some_and(|(due, _)| *due <= deadline) {
            let (due, packet) = self.queue.remove(0);
            self.now = self.now.max(due);
            Some(packet)
        } else {
            self.now = deadline;
            None
        };
        if self.now >= self.stop_at {
            self.stop.store(true, Ordering::Release);
        }
        Ok(reply)
    }

    fn elapsed(&self) -> Duration {
        self.now
    }

    fn pause(&mut self, duration: Duration) {
        self.now += duration;
    }
}

impl Drop for TimedLink<'_> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(self.held.get());
            assert!(
                self.script.is_empty(),
                "missing requests: {:?}",
                self.script
            );
        }
    }
}

fn read(
    command: ReadCommand,
    script: Vec<(PodxtRequest, Packets)>,
    stop_at: Duration,
    pending: &AtomicBool,
    publish: impl FnMut(ReadUpdate, &AtomicBool),
) -> (Result<(), String>, usize) {
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let stop = AtomicBool::new(false);
    let mut publish = publish;
    let result = transaction(
        command,
        &stop,
        pending,
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            run(&mut TimedLink {
                held: &held,
                writes: &writes,
                script: script.into(),
                queue: Vec::new(),
                now: Duration::ZERO,
                stop: &stop,
                stop_at,
            })
        },
        |update| publish(update, &stop),
    );
    assert!(!held.get());
    (result, writes.get())
}

#[test]
fn diagnostics_recovers_after_a_timeout_and_discards_a_late_dump_before_resending() {
    let pending = AtomicBool::new(false);
    let wanted = tone(b"Fresh current");
    let (result, writes) = read(
        ReadCommand::Current,
        vec![
            identity_reply(),
            (PodxtRequest::EditBuffer, vec![(2100, tone(b"Stale").raw)]),
            (PodxtRequest::EditBuffer, vec![(10, wanted.raw.clone())]),
        ],
        Duration::MAX,
        &pending,
        |update, _| assert!(matches!(update, ReadUpdate::Current(tone) if tone == wanted)),
    );
    result.unwrap();
    assert_eq!(writes, 3);
    assert!(!pending.load(Ordering::Acquire));
}

#[test]
fn abandoned_or_ambiguous_diagnostics_reads_block_later_workbench_reads() {
    let stale = tone(b"Stale").raw;
    for (attempts, stop_at, expected) in [
        (
            vec![vec![], vec![], vec![]],
            Duration::MAX,
            "all 3 attempts",
        ),
        (vec![vec![]], at(5300), "cancelled"),
        (
            vec![vec![], vec![(10, stale.clone())]],
            at(5600),
            "cancelled",
        ),
        (
            vec![vec![], vec![(10, stale.clone()), (400, stale.clone())]],
            Duration::MAX,
            "extra PODxt",
        ),
        (
            vec![vec![(10, [stale.clone(), stale.clone()].concat())]],
            Duration::MAX,
            "More than one",
        ),
    ] {
        let pending = AtomicBool::new(false);
        let count = 1 + attempts.len();
        let (result, writes) = read(
            ReadCommand::Current,
            std::iter::once(identity_reply())
                .chain(
                    attempts
                        .into_iter()
                        .map(|packets| (PodxtRequest::EditBuffer, packets)),
                )
                .collect(),
            stop_at,
            &pending,
            |_, _| panic!("an unresolved read must not publish a tone"),
        );
        assert!(result.unwrap_err().contains(expected));
        assert_eq!(writes, count);
        assert!(pending.load(Ordering::Acquire));
        let (result, writes) = read(
            ReadCommand::ToneBank,
            Vec::new(),
            Duration::MAX,
            &pending,
            |_, _| panic!("an unresolved dump cannot answer a stored read"),
        );
        assert_eq!(result, Err(PENDING_REPLY.into()));
        assert_eq!(writes, 0);
    }
}

#[test]
fn follow_refresh_retries_while_forwarding_tuner_events_during_drain_and_guard() {
    let pending = AtomicBool::new(false);
    let initial = tone(b"Initial");
    let fresh = tone(b"New channel");
    let channel = Location::new(BankKind::Tones, 41).unwrap();
    let mut sequence = Vec::new();
    let mut tuner_on = 0;
    let (result, writes) = read(
        ReadCommand::Follow,
        vec![
            identity_reply(),
            (
                PodxtRequest::EditBuffer,
                vec![
                    (
                        1,
                        [initial.raw.clone(), vec![0xf0, 0, 1, 0x0c, 3, 0x10, 0]].concat(),
                    ),
                    (2, vec![69, 127, 0xf7]),
                    (3, vec![0xc0, 41]),
                ],
            ),
            (PodxtRequest::EditBuffer, vec![(2100, vec![0xb0, 69, 127])]),
            // The tuner-off notification uses MIDI running status across the retry.
            (
                PodxtRequest::EditBuffer,
                vec![
                    (1, vec![69, 0]),
                    (10, fresh.raw.clone()),
                    (100, vec![0xf0, 0, 1, 0x0c, 3, 0x0f, 0xf7]),
                ],
            ),
        ],
        Duration::MAX,
        &pending,
        |update, stop| {
            sequence.push(match update {
                ReadUpdate::Current(tone) if tone == initial => "initial",
                ReadUpdate::Channel(slot) if slot == channel => "channel",
                ReadUpdate::TunerMode(true) => {
                    tuner_on += 1;
                    if tuner_on == 1 {
                        "split tuner notice"
                    } else {
                        "tuner on"
                    }
                }
                ReadUpdate::TunerMode(false) => "tuner off",
                ReadUpdate::Clip => "clip",
                ReadUpdate::Current(tone) if tone == fresh => {
                    stop.store(true, Ordering::Release);
                    "fresh"
                }
                _ => panic!("unexpected Follow update"),
            })
        },
    );
    assert_eq!(result, Err("PODxt read cancelled.".into()));
    assert_eq!(writes, 4);
    assert_eq!(
        sequence,
        [
            "initial",
            "split tuner notice",
            "channel",
            "tuner on",
            "tuner off",
            "clip",
            "fresh"
        ]
    );
    assert!(!pending.load(Ordering::Acquire));
}

#[test]
fn follow_forwards_channel_and_control_changes_and_refuses_to_retry_across_them() {
    let pending = AtomicBool::new(false);
    let initial = tone(b"Initial");
    let mut sequence = Vec::new();
    let (result, writes) = read(
        ReadCommand::Follow,
        vec![
            identity_reply(),
            (
                PodxtRequest::EditBuffer,
                vec![(1, initial.raw.clone()), (2, vec![0xc0, 41])],
            ),
            (
                PodxtRequest::EditBuffer,
                vec![(2100, vec![0xc0, 42, 0xb0, 13, 7, 0xb0, 69, 127])],
            ),
        ],
        Duration::MAX,
        &pending,
        |update, _| {
            sequence.push(match update {
                ReadUpdate::Current(tone) if tone == initial => "initial",
                ReadUpdate::Channel(slot) if slot.index() == 41 => "channel 41",
                ReadUpdate::Channel(slot) if slot.index() == 42 => "channel 42",
                ReadUpdate::Controller {
                    number: 13,
                    value: 7,
                } => "drive",
                ReadUpdate::TunerMode(true) => "tuner on",
                _ => panic!("unexpected Follow update"),
            })
        },
    );
    assert!(result.unwrap_err().contains("changed during recovery"));
    assert_eq!(
        sequence,
        ["initial", "channel 41", "channel 42", "drive", "tuner on"]
    );
    assert_eq!(writes, 3);
    assert!(pending.load(Ordering::Acquire));
}

#[test]
fn librarian_previews_only_the_expected_dump_with_an_optional_end_marker() {
    let slot = Location::new(BankKind::Tones, 0).unwrap();
    let wanted = tone(b"Stored");
    let next = tone(b"Another slot");
    for (packets, accepted) in [
        (vec![(1, wanted.raw.clone())], true),
        (vec![(1, [wanted.raw.clone(), END.to_vec()].concat())], true),
        (
            vec![
                (1, wanted.raw.clone()),
                (50, END[..5].to_vec()),
                (80, END[5..].to_vec()),
            ],
            true,
        ),
        (vec![(1, END.to_vec()), (50, wanted.raw.clone())], false),
        (
            vec![(1, wanted.raw.clone()), (50, [END, END].concat())],
            false,
        ),
        (vec![(1, wanted.raw.clone()), (50, next.raw.clone())], false),
    ] {
        let pending = AtomicBool::new(false);
        let mut preview = None;
        let (result, writes) = read(
            ReadCommand::ToneBank,
            vec![identity_reply(), (PodxtRequest::Stored(slot), packets)],
            Duration::MAX,
            &pending,
            |update, stop| match update {
                ReadUpdate::LibraryStarted => {}
                ReadUpdate::Stored(stored) => {
                    assert_eq!(stored.location, slot);
                    preview = Some(stored.tone);
                    stop.store(true, Ordering::Release);
                }
                _ => panic!("unexpected librarian update"),
            },
        );
        if accepted {
            assert_eq!(result, Err("PODxt read cancelled.".into()));
            assert_eq!(preview, Some(wanted.clone()));
        } else {
            assert!(result.unwrap_err().contains("Unexpected SysEx reply"));
            assert_eq!(preview, None);
        }
        assert_eq!(pending.load(Ordering::Acquire), !accepted);
        assert_eq!(writes, 2);
    }
}

#[test]
fn follow_refuses_a_stale_dump_between_reads_without_suppressing_notifications() {
    let pending = AtomicBool::new(false);
    let initial = tone(b"Initial");
    let mut publications = 0;
    let (result, writes) = read(
        ReadCommand::Follow,
        vec![
            identity_reply(),
            (
                PodxtRequest::EditBuffer,
                vec![
                    (1, initial.raw.clone()),
                    (
                        50,
                        [tone(b"Stale").raw, vec![0xc0, 41, 0xb0, 13, 7]].concat(),
                    ),
                ],
            ),
        ],
        Duration::MAX,
        &pending,
        |update, _| {
            assert!(match (publications, update) {
                (0, ReadUpdate::Current(tone)) => tone == initial,
                (1, ReadUpdate::Channel(slot)) => slot.index() == 41,
                (
                    2,
                    ReadUpdate::Controller {
                        number: 13,
                        value: 7,
                    },
                ) => true,
                _ => false,
            });
            publications += 1;
        },
    );
    assert!(result.unwrap_err().contains("unsolicited PODxt tone reply"));
    assert_eq!(writes, 2);
    assert_eq!(publications, 3);
    assert!(pending.load(Ordering::Acquire));
}
