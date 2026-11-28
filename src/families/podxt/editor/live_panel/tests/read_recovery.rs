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
fn library_retries_only_the_missing_address_and_continues_after_the_guard() {
    let first = Location::new(BankKind::Tones, 0).unwrap();
    let next = Location::new(BankKind::Tones, 1).unwrap();
    let first_tone = tone(b"Recovered");
    let next_tone = tone(b"Next slot");

    for command in [ReadCommand::ToneBank, ReadCommand::Library] {
        let pending = AtomicBool::new(false);
        let mut received = Vec::new();
        let (result, writes) = read(
            command,
            vec![
                identity_reply(),
                (PodxtRequest::Stored(first), vec![]),
                (PodxtRequest::Stored(first), vec![]),
                (
                    PodxtRequest::Stored(first),
                    vec![(10, first_tone.raw.clone()), (20, END.to_vec())],
                ),
                (
                    PodxtRequest::Stored(next),
                    vec![(10, next_tone.raw.clone())],
                ),
            ],
            Duration::MAX,
            &pending,
            |update, stop| {
                if let ReadUpdate::Stored(stored) = update {
                    received.push((stored.location, stored.tone));
                    if stored.location == next {
                        stop.store(true, Ordering::Release);
                    }
                }
            },
        );
        assert_eq!(result, Err("PODxt read cancelled.".into()));
        assert_eq!(
            received,
            [(first, first_tone.clone()), (next, next_tone.clone())]
        );
        assert_eq!(writes, 5);
        assert!(!pending.load(Ordering::Acquire));
    }
}

#[test]
fn library_stops_after_three_missing_dumps_at_one_address() {
    let first = Location::new(BankKind::Tones, 0).unwrap();
    let pending = AtomicBool::new(false);
    let mut script = vec![identity_reply()];
    script.extend((0..3).map(|_| (PodxtRequest::Stored(first), vec![])));
    let (result, writes) = read(
        ReadCommand::Library,
        script,
        Duration::MAX,
        &pending,
        |update, _| assert!(matches!(update, ReadUpdate::LibraryStarted)),
    );
    assert!(result.unwrap_err().contains("1A: no stored dump"));
    assert_eq!(writes, 4);
    assert!(pending.load(Ordering::Acquire));
}

#[test]
fn library_refuses_a_second_dump_from_the_retried_address_before_advancing() {
    let first = Location::new(BankKind::Tones, 0).unwrap();
    let reply = tone(b"First slot").raw;
    let pending = AtomicBool::new(false);
    let (result, writes) = read(
        ReadCommand::ToneBank,
        vec![
            identity_reply(),
            // This first attempt's reply arrives after the retry was sent.
            (PodxtRequest::Stored(first), vec![(3400, reply.clone())]),
            (PodxtRequest::Stored(first), vec![(3150, reply)]),
        ],
        Duration::MAX,
        &pending,
        |update, _| assert!(matches!(update, ReadUpdate::LibraryStarted)),
    );
    assert!(result.unwrap_err().contains("Unexpected SysEx reply"));
    assert_eq!(writes, 3);
    assert!(pending.load(Ordering::Acquire));
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

#[test]
fn tab_close_and_reopen_keep_quarantine_until_a_new_verified_attachment() {
    let ctx = egui::Context::default();
    let (mut app, presence) = connected_fixture(&ctx);
    let key = app.podxt().live.device.clone().unwrap();
    let pending = Arc::clone(&app.podxt().live.editor.reply_pending);

    let (result, writes) = read(
        ReadCommand::Current,
        vec![identity_reply(), (PodxtRequest::EditBuffer, vec![])],
        at(3300),
        &pending,
        |_, _| panic!("a cancelled read cannot publish a tone"),
    );
    assert_eq!(result, Err("PODxt read cancelled.".into()));
    assert_eq!(writes, 2);
    assert!(pending.load(Ordering::Acquire));

    let id = app.tabs.active.unwrap();
    app.close_tab(id, &ctx);
    assert!(app.tabs.entries.is_empty());
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    assert!(app.tabs.entries.is_empty(), "a dismissed tab stays closed");

    // Model opening a fresh editor view. Reset only the shell's dismissed-tab
    // list with a fresh tab container, retaining the running detection service.
    app.tabs = tabs::Tabs::default();
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    app.podxt_mut().table_view_in_test = true;
    let text = stage_frame(&ctx, &mut app);
    let live = &mut app.podxt_mut().live;
    assert_eq!(live.device.as_ref(), Some(&key));
    assert!(Arc::ptr_eq(&live.editor.reply_pending, &pending));
    assert!(!live.device_ready());
    assert_eq!(live.next_startup(), None);
    assert!(text.contains(PENDING_REPLY));
    assert!(!text.contains("Reading the playing tone…"));

    let reopened = Arc::clone(&live.editor.reply_pending);
    let (result, writes) = read(
        ReadCommand::Current,
        Vec::new(),
        Duration::MAX,
        &reopened,
        |_, _| panic!("quarantined read must not publish"),
    );
    assert_eq!(result, Err(PENDING_REPLY.into()));
    assert_eq!(writes, 0);
    edit::tests::assert_recall_and_edit_blocked(&reopened);
    store::tests::assert_store_blocked(&reopened);

    presence.attached.set(false);
    presence.identified.set(false);
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    presence.attached.set(true);
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    assert!(!app.podxt().live.device_ready());
    assert!(Arc::ptr_eq(
        &app.podxt().live.editor.reply_pending,
        &pending
    ));

    presence.identified.set(true);
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    let live = &app.podxt().live;
    assert_ne!(live.device.as_ref(), Some(&key));
    assert!(live.device_ready());
    assert!(!Arc::ptr_eq(&live.editor.reply_pending, &pending));
    assert!(pending.load(Ordering::Acquire));
    let fresh = tone(b"Fresh device");
    let (result, writes) = read(
        ReadCommand::Current,
        vec![
            identity_reply(),
            (PodxtRequest::EditBuffer, vec![(1, fresh.raw.clone())]),
        ],
        Duration::MAX,
        &live.editor.reply_pending,
        |update, _| assert!(matches!(update, ReadUpdate::Current(tone) if tone == fresh)),
    );
    assert_eq!(result, Ok(()));
    assert_eq!(writes, 2);
}

#[test]
fn a_reply_quarantined_while_acquiring_the_rig_lock_prevents_open() {
    let pending = AtomicBool::new(false);
    let result = transaction(
        ReadCommand::Current,
        &AtomicBool::new(false),
        &pending,
        || {
            pending.store(true, Ordering::Release);
            Ok(())
        },
        |_, _| panic!("recheck quarantine under the rig lock before opening"),
        |_| panic!("quarantined read must not publish"),
    );
    assert_eq!(result, Err(PENDING_REPLY.into()));
}
