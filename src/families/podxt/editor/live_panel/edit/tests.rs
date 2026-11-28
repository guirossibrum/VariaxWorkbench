mod drafts;

use super::super::tests::{FakeLink, FakeLock};
use super::*;
use podxt::{Global, PodxtRequest, Value};
use std::{cell::Cell, collections::VecDeque};

fn sample(drive: u8, index: u16) -> EditorBuffer {
    let mut raw = vec![0; podxt::EDIT_BUFFER_LEN];
    raw[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
    raw[39 + 13] = drive;
    raw[167] = 0xf7;
    EditorBuffer {
        channel: Location::new(BankKind::Tones, index).unwrap(),
        tone: EditBuffer::parse(raw).unwrap(),
        packs: Packs(4),
    }
}

fn drive(value: u8) -> LiveTarget {
    LiveTarget::Basic(Control::Drive, Target::Value(Value::new(value).unwrap()))
}

pub(super) fn fake_get(job: &Job, stop: &AtomicBool) -> Result<EditOutcome, String> {
    assert!(job.action == Action::Get);
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let channel = (
        PodxtRequest::QueryGlobal(Global::CurrentChannel),
        vec![0xf0, 0, 1, 0x0c, 3, 0x56, 17, 0, 0, 1, 1, 0xf7],
    );
    let result = edit_transaction(
        job,
        stop,
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            run(&mut FakeLink {
                held: &held,
                script: VecDeque::from([
                    (
                        PodxtRequest::Identity,
                        variax_workbench::identity_stage::bridge_reply(),
                    ),
                    channel.clone(),
                    (
                        PodxtRequest::EditBuffer,
                        super::super::tests::valid_dump("Read Live Tone").raw,
                    ),
                    (
                        PodxtRequest::InstalledPacks,
                        vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7],
                    ),
                    channel,
                ]),
                reply: None,
                time: &Cell::new(Duration::ZERO),
                writes: &writes,
            })
        },
    );
    assert_eq!(writes.get(), 5);
    assert!(!held.get());
    result
}

#[derive(Clone, Copy)]
pub(super) enum FakeRecall {
    Clean,
    ChangedSource,
}

pub(super) fn fake_recall(
    scenario: FakeRecall,
    job: &Job,
    stop: &AtomicBool,
) -> Result<EditOutcome, String> {
    let Action::Recall(target) = job.action else {
        unreachable!("the fixture only handles recall");
    };
    let before = Location::tone_from_label("32D").unwrap();
    let channel = |slot: Location| {
        let index = slot.index() as u8;
        (
            PodxtRequest::QueryGlobal(Global::CurrentChannel),
            vec![
                0xf0,
                0,
                1,
                0x0c,
                3,
                0x56,
                17,
                0,
                0,
                index >> 4,
                index & 15,
                0xf7,
            ],
        )
    };
    let before_tone = (
        PodxtRequest::EditBuffer,
        super::super::tests::valid_dump("Before recall").raw,
    );
    let packs = (
        PodxtRequest::InstalledPacks,
        vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7],
    );
    let mut saved_source = before_tone.1.clone();
    let after = super::super::tests::valid_dump("Recalled tone").raw;
    let writes_after_source = match scenario {
        FakeRecall::Clean => {
            assert!(
                job.current.is_none(),
                "initial recall must work without a verified buffer"
            );
            vec![
                (PodxtRequest::Stored(target), after.clone()),
                packs,
                before_tone.clone(),
                channel(before),
                (PodxtRequest::Program(target), Vec::new()),
                (PodxtRequest::EditBuffer, after),
                channel(target),
            ]
        }
        FakeRecall::ChangedSource => {
            let current = job.current.as_ref().expect("verified playing buffer");
            assert_eq!(current.channel, before);
            assert_eq!(current.tone.raw, before_tone.1);
            saved_source[39 + usize::from(Control::Drive.cc())] = 77;
            vec![packs, channel(before)]
        }
    };
    let expected_writes = 4 + writes_after_source.len();
    let script = [
        (
            PodxtRequest::Identity,
            variax_workbench::identity_stage::bridge_reply(),
        ),
        channel(before),
        before_tone,
        (PodxtRequest::Stored(before), saved_source),
    ]
    .into_iter()
    .chain(writes_after_source)
    .collect();
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let result = edit_transaction(
        job,
        stop,
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            run(&mut FakeLink {
                held: &held,
                script,
                reply: None,
                time: &Cell::new(Duration::ZERO),
                writes: &writes,
            })
        },
    );
    assert_eq!(writes.get(), expected_writes);
    assert!(!held.get());
    result
}

#[test]
fn queued_drag_edits_keep_the_latest_value_and_honor_the_dispatch_interval() {
    let now = Instant::now();
    let mut editor = LiveEditor {
        last_dispatch: Some(now),
        stage_only: false,
        ..Default::default()
    };
    let control = LiveControl::Basic(Control::Drive);
    let in_flight = [drive(20)];
    assert_eq!(editor.pending.overlay(&in_flight, control, 0), 20);

    for value in [22, 40, 57] {
        editor.pending.set(drive(value));
    }
    assert_eq!(editor.pending.0, [drive(57)]);
    assert_eq!(editor.pending.overlay(&in_flight, control, 0), 57);
    assert!(editor.busy());
    assert_eq!(
        editor.dispatch_wait(now + Duration::from_millis(40)),
        Duration::from_millis(110)
    );
    assert!(editor.dispatch_wait(now + EDIT_INTERVAL).is_zero());

    editor.cancel();
    assert!(!editor.busy());
    editor.stage_only = true;
    editor.pending.set(drive(84));
    assert!(!editor.busy(), "staged values never dispatch automatically");
    editor.detach();
    assert!(editor.pending.0.is_empty());
}

#[test]
fn a_failed_edit_keeps_the_original_snapshot_until_the_playing_channel_changes() {
    let mut editor = LiveEditor::default();
    editor.completed(Action::Get, Ok(sample(12, 17)));
    editor.completed(Action::Controls, Ok(sample(54, 17)));
    editor.pending.set(drive(58));
    editor.completed(Action::Controls, Err("missing readback".into()));
    assert!(editor.needs_get);
    assert!(editor.pending.0.is_empty());
    editor.completed(Action::Get, Ok(sample(55, 17)));
    assert!(!editor.needs_get);
    let active = editor.session.as_ref().unwrap();
    assert_eq!(active.original.tone.cc(13), 12);
    assert_eq!(active.current.tone.cc(13), 55);

    editor.completed(Action::Get, Ok(sample(90, 18)));
    assert_eq!(editor.session.as_ref().unwrap().original.tone.cc(13), 90);
    editor.detach();
    assert!(editor.session.is_none());
}

#[test]
fn g6_transaction_holds_the_rig_lock_for_cc_and_readback_and_denies_other_writes() {
    let before = sample(20, 17);
    let after = sample(45, 17);
    let channel = || {
        (
            PodxtRequest::QueryGlobal(Global::CurrentChannel),
            vec![0xf0, 0, 1, 0x0c, 3, 0x56, 17, 0, 0, 1, 1, 0xf7],
        )
    };
    let job = Job {
        recall_confirmation: None,
        reply_pending: Arc::default(),
        action: Action::Controls,
        whole_tone_backup: None,
        current: Some(before.clone()),
        original: Some(before.clone()),
        targets: vec![drive(45)],
    };
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let stop = AtomicBool::new(false);
    let result = edit_transaction(
        &job,
        &stop,
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            assert!(held.get(), "the lock must cover opening the port");
            run(&mut FakeLink {
                held: &held,
                script: VecDeque::from([
                    (
                        PodxtRequest::Identity,
                        variax_workbench::identity_stage::bridge_reply(),
                    ),
                    channel(),
                    (PodxtRequest::EditBuffer, before.tone.raw),
                    channel(),
                    (
                        PodxtRequest::Set(Control::Drive, Value::new(45).unwrap()),
                        Vec::new(),
                    ),
                    (PodxtRequest::EditBuffer, after.tone.raw.clone()),
                    channel(),
                ]),
                reply: None,
                time: &Cell::new(Duration::ZERO),
                writes: &writes,
            })
        },
    )
    .unwrap();
    let EditOutcome::Buffer(result) = result else {
        panic!("expected edited buffer")
    };
    assert_eq!(result.tone, after.tone);
    assert_eq!(writes.get(), 7);
    assert!(!held.get());

    let busy = edit_transaction(
        &job,
        &stop,
        || Err::<(), _>("Rig busy".into()),
        |_, _| panic!("busy rig must prevent port open"),
    );
    assert_eq!(busy.unwrap_err(), "Rig busy");
    for denied in [
        Operation::PodxtChannelSwitch,
        Operation::PodxtLibraryRead,
        Operation::PodxtLinkedVariaxModelEdit,
        Operation::PodxtVariaxControlRelease,
        Operation::VariaxStoreRestore,
    ] {
        assert!(!EditPolicy(Action::Send).allows(denied));
    }
    assert!(!EditPolicy(Action::Get).allows(Operation::PodxtEditBufferWrite));
}

#[test]
fn model_specific_controls_only_appear_with_their_line6_layout() {
    let table = variax_workbench::pod_preset::pod_parameter_table();
    let names = table.wire_label(Control::StompParam2.cc(), |_| Some(33882112));
    assert_eq!(names.unwrap().label, "Drive");
    assert!(
        table
            .wire_label(Control::StompParam6.cc(), |_| Some(33882112))
            .is_none()
    );
    assert!(
        table.wire_label(96, |_| None).is_some(),
        "the table can describe linked model fields"
    );
    assert!(!Control::ALL.into_iter().any(|control| control.cc() == 96));
    assert!(
        !Setting::ALL
            .into_iter()
            .any(|setting| LiveControl::Additional(setting).cc() == 96)
    );
}

#[test]
fn the_whole_tone_resend_has_its_own_closed_owner_gate() {
    const {
        assert!(!WHOLE_TONE_SEND_APPROVED);
    }
    assert!(!LiveEditor::admitted(Action::ResendWholeTone));
    let policy = EditPolicy(Action::ResendWholeTone);
    assert_eq!(
        policy.allows(Operation::PodxtLibraryRead),
        !hardware_gates::is_locked(Gate::FullBackupRead)
    );
    assert!(!policy.allows(Operation::PodxtStoreRestore));
    assert!(!policy.allows(Operation::PodxtChannelSwitch));
}

#[test]
fn stage_get_recovers_and_recall_sends_one_selection_after_final_read() {
    use super::super::tests::read_recovery::{TimedLink, identity_reply};

    let before = sample(20, 17);
    let target = sample(45, 18);
    let pending = Arc::new(AtomicBool::new(false));
    let held = Cell::new(false);
    let stop = AtomicBool::new(false);
    let channel = |index| {
        (
            PodxtRequest::QueryGlobal(Global::CurrentChannel),
            vec![(1, vec![0xf0, 0, 1, 0x0c, 3, 0x56, 17, 0, 0, 1, index, 0xf7])],
        )
    };
    let packs = (
        PodxtRequest::InstalledPacks,
        vec![(1, vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7])],
    );
    let current = (PodxtRequest::EditBuffer, vec![(1, before.tone.raw.clone())]);
    for (action, script, count, expected) in [
        (
            Action::Get,
            vec![
                identity_reply(),
                channel(1),
                (PodxtRequest::EditBuffer, vec![]),
                current.clone(),
                packs.clone(),
                channel(1),
            ],
            6,
            &before,
        ),
        (
            Action::Recall(target.channel),
            vec![
                identity_reply(),
                channel(1),
                current,
                (
                    PodxtRequest::Stored(before.channel),
                    vec![(1, before.tone.raw.clone())],
                ),
                (
                    PodxtRequest::Stored(target.channel),
                    vec![(1, target.tone.raw.clone())],
                ),
                packs,
                (PodxtRequest::EditBuffer, vec![(1, before.tone.raw.clone())]),
                channel(1),
                (PodxtRequest::Program(target.channel), vec![]),
                (PodxtRequest::EditBuffer, vec![(1, target.tone.raw.clone())]),
                channel(2),
            ],
            11,
            &target,
        ),
    ] {
        let writes = Cell::new(0);
        let result = edit_transaction(
            &Job {
                recall_confirmation: None,
                reply_pending: Arc::clone(&pending),
                action,
                whole_tone_backup: None,
                current: None,
                original: None,
                targets: Vec::new(),
            },
            &stop,
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
                    stop_at: Duration::MAX,
                })
            },
        )
        .unwrap();
        assert_eq!(writes.get(), count);
        assert!(!held.get());
        assert!(!pending.load(Ordering::Acquire));
        let (EditOutcome::Buffer(result) | EditOutcome::Recall(EditorRecall::Playing(result))) =
            result
        else {
            panic!("expected a verified playing buffer");
        };
        assert_eq!(result.channel, expected.channel);
        assert_eq!(result.packs, expected.packs);
        assert_eq!(result.tone, expected.tone);
    }
}

pub(in crate::editor::podxt_panel::live_panel) fn assert_recall_and_edit_blocked(
    pending: &Arc<AtomicBool>,
) {
    let current = sample(20, 17);
    let target = Location::new(BankKind::Tones, 18).unwrap();
    for action in [Action::Recall(target), Action::Controls] {
        let result = edit_transaction(
            &Job {
                recall_confirmation: None,
                reply_pending: Arc::clone(pending),
                action,
                whole_tone_backup: None,
                current: Some(current.clone()),
                original: Some(current.clone()),
                targets: vec![drive(45)],
            },
            &AtomicBool::new(false),
            || panic!("quarantined recall/edit must not acquire the rig lock"),
            |_: &(), _| panic!("quarantined recall/edit must not open MIDI"),
        );
        assert_eq!(result.unwrap_err(), PENDING_REPLY);
    }
}

fn refusal(
    playing: &EditorBuffer,
    target: Location,
    release_ok: bool,
) -> Result<EditOutcome, String> {
    use super::super::tests::read_recovery::{TimedLink, identity_reply};

    let held = Cell::new(false);
    let writes = Cell::new(0);
    let stop = AtomicBool::new(false);
    let channel = (
        PodxtRequest::QueryGlobal(Global::CurrentChannel),
        vec![(1, vec![0xf0, 0, 1, 0x0c, 3, 0x56, 17, 0, 0, 1, 1, 0xf7])],
    );
    let saved = sample(20, 17);
    let result = edit_transaction(
        &Job {
            recall_confirmation: None,
            reply_pending: Arc::default(),
            action: Action::Recall(target),
            whole_tone_backup: None,
            current: None,
            original: None,
            targets: Vec::new(),
        },
        &stop,
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            let result = run(&mut TimedLink {
                held: &held,
                writes: &writes,
                script: VecDeque::from([
                    identity_reply(),
                    channel.clone(),
                    (
                        PodxtRequest::EditBuffer,
                        vec![(1, playing.tone.raw.clone())],
                    ),
                    (
                        PodxtRequest::Stored(saved.channel),
                        vec![(1, saved.tone.raw)],
                    ),
                    (
                        PodxtRequest::InstalledPacks,
                        vec![(1, vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7])],
                    ),
                    channel,
                ]),
                queue: Vec::new(),
                now: Duration::ZERO,
                stop: &stop,
                stop_at: Duration::MAX,
            });
            assert!(held.get(), "release must remain covered by the rig lock");
            if release_ok {
                result
            } else {
                Err("USB release failed.".into())
            }
        },
    );
    assert!(!held.get());
    assert_eq!(writes.get(), 6);
    result
}

#[test]
fn clean_refusal_keeps_controls_ready_and_keep_or_cancel_never_dispatches() {
    let ctx = egui::Context::default();
    let (mut app, _) = super::super::tests::connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    let device = panel.device.clone().unwrap();
    let editor = &mut panel.editor;
    let playing = sample(44, 17);
    let target = Location::tone_from_label("1B").unwrap();

    editor.completed(Action::Get, Ok(sample(20, 17)));
    editor.set_live(true);
    for stopped in [false, true] {
        let observed = editor
            .complete_job(
                Action::Recall(target),
                &device,
                stopped,
                refusal(&playing, target, true),
            )
            .unwrap();
        assert_eq!(observed.tone, playing.tone);
        assert!(editor.recall_ready());
        assert!(!editor.needs_refresh());
        assert_eq!(editor.awaiting_recall_confirmation(), !stopped);
        if !stopped {
            editor.cancel_switch();
        }
        assert!(editor.take_recall_confirmation(&device).is_none());
        assert!(!editor.busy());
    }

    panel.editor.complete_job(
        Action::Recall(target),
        &device,
        false,
        refusal(&playing, target, true),
    );
    panel.live_editing = true;
    panel.startup = None;
    let text = super::super::tests::stage_frame(&ctx, &mut app);
    assert!(text.contains("Cancel switch"), "{text}");
    assert!(text.contains("Switch to 1B and discard changes"), "{text}");
    assert!(!text.lines().any(|line| line == "Read playing tone"));
    assert!(!text.lines().any(|line| line == "Reconnect"));
}

#[test]
fn recall_choice_expires_with_attachment_mode_or_staged_edits() {
    let ctx = egui::Context::default();
    let (mut app, _) = super::super::tests::connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    let device = panel.device.clone().unwrap();
    let mut replacement = device.clone();
    replacement.attachment.path.push_str(".1");
    let editor = &mut panel.editor;
    let playing = sample(44, 17);
    let target = Location::tone_from_label("1B").unwrap();

    editor.set_live(true);
    editor.complete_job(
        Action::Recall(target),
        &device,
        false,
        refusal(&playing, target, true),
    );
    assert_eq!(
        editor.take_recall_confirmation(&device).unwrap().target(),
        target
    );
    assert!(editor.take_recall_confirmation(&device).is_none());

    editor.complete_job(
        Action::Recall(target),
        &device,
        false,
        refusal(&playing, target, true),
    );
    assert!(editor.take_recall_confirmation(&replacement).is_none());

    editor.complete_job(
        Action::Recall(target),
        &device,
        false,
        refusal(&playing, target, true),
    );
    editor.set_live(false);
    assert!(editor.take_recall_confirmation(&device).is_none());

    editor.set_live(true);
    editor.complete_job(
        Action::Recall(target),
        &device,
        false,
        refusal(&playing, target, true),
    );
    editor.pending.set(drive(55));
    assert!(editor.take_recall_confirmation(&device).is_none());
    editor.recall(target, &device, &ctx);
    assert_eq!(editor.pending.0, [drive(55)]);
    assert!(editor.worker.is_none());
}

#[test]
fn failed_release_or_unverified_switch_needs_refresh_and_keeps_staged_values() {
    let ctx = egui::Context::default();
    let (mut app, _) = super::super::tests::connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    let device = panel.device.clone().unwrap();
    let editor = &mut panel.editor;
    let playing = sample(44, 17);
    let target = Location::tone_from_label("1B").unwrap();
    editor.set_live(true);
    for failure in [
        refusal(&playing, target, false),
        Err("The channel switch could not be verified.".into()),
    ] {
        editor.completed(Action::Get, Ok(playing.clone()));
        editor.pending.set(drive(55));
        assert!(
            editor
                .complete_job(Action::Recall(target), &device, false, failure)
                .is_none()
        );
        assert!(editor.needs_refresh());
        assert!(!editor.recall_ready());
        assert!(!editor.stage_editable());
        assert!(!editor.awaiting_recall_confirmation());
        assert_eq!(editor.pending.0, [drive(55)]);
    }

    panel.editor.discard_staged();
    panel.live_editing = true;
    panel.startup = None;
    let text = super::super::tests::stage_frame(&ctx, &mut app);
    assert_eq!(text.lines().filter(|line| *line == "Reconnect").count(), 1);
    assert!(!text.contains("and discard changes"), "{text}");
}

#[test]
fn undo_is_available_only_for_changes_the_session_can_restore() {
    let mut editor = LiveEditor::default();
    let original = sample(20, 3);
    let changed = sample(45, 3);
    assert!(!editor.can_undo());
    editor.completed(Action::Get, Ok(original.clone()));
    assert!(!editor.can_undo());
    editor.completed(Action::Controls, Ok(changed));
    assert!(editor.can_undo());
    editor.pending.set(drive(60));
    assert!(!editor.can_undo());
    editor.discard_staged();
    editor.completed(Action::Undo, Ok(original));
    assert!(!editor.can_undo());
    editor.completed(Action::Get, Ok(sample(45, 4)));
    assert!(!editor.can_undo());
}

#[test]
fn edit_lock_retries_only_busy_and_stops_before_open_on_cancel() {
    use line6_devices::device_lock::LockError;
    let stop = AtomicBool::new(false);
    let mut script = VecDeque::from([Err(LockError::Busy), Err(LockError::BusyHere), Ok(42)]);
    let mut delays = Vec::new();
    assert_eq!(
        wait_for_edit_lock(
            &stop,
            || script.pop_front().unwrap(),
            |delay| delays.push(delay)
        ),
        Ok(42)
    );
    assert_eq!(delays, [Duration::from_millis(500); 2]);
    let result: Result<(), _> = wait_for_edit_lock(
        &stop,
        || {
            Err(LockError::Unavailable(
                "Variax Follow could not cleanly release the rig".into(),
            ))
        },
        |_| panic!("cleanup failure must stop"),
    );
    assert!(result.unwrap_err().contains("could not cleanly release"));
    let attempts = Cell::new(0);
    let result: Result<(), _> = wait_for_edit_lock(
        &stop,
        || {
            attempts.set(attempts.get() + 1);
            Err(LockError::Busy)
        },
        |_| stop.store(true, Ordering::Release),
    );
    assert_eq!(attempts.get(), 1);
    assert!(result.unwrap_err().contains("cancelled"));
}

#[test]
fn automatic_edits_hide_progress_and_success_but_keep_errors_visible() {
    let ctx = egui::Context::default();
    let (mut app, _) = super::super::tests::connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    panel.editor.set_live(true);
    panel.editor.pending.set(drive(23));
    assert!(panel.busy());
    assert!(!panel.shows_operation_progress());

    let editor = &mut panel.editor;
    editor.pending.0.clear();
    for (action, expected) in [
        (Action::Controls, None),
        (Action::Get, Some("Read playing tone")),
        (Action::Send, Some("Send edit buffer")),
    ] {
        editor.worker = Some(EditWorker {
            action,
            device: None,
            stop: Arc::default(),
            targets: Vec::new(),
            thread: None,
        });
        assert_eq!(editor.status_text(), expected);
        assert_eq!(editor.progress_label().is_some(), expected.is_some());
    }
    editor.worker = None;
    editor.completed(Action::Controls, Ok(sample(23, 0)));
    assert_eq!(editor.status_text(), None);
    editor.completed(Action::Controls, Err("Device update failed.".into()));
    assert_eq!(editor.status_text(), Some("Device update failed."));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ControlResult {
    Verified,
    BadReadback,
    CloseFailed,
    PendingReply,
    ChannelChanged,
    ChannelChangedBeforeUndo,
}

#[derive(Clone)]
pub(super) struct FakeEdit {
    outcome: ControlResult,
    actions: Arc<Mutex<Vec<Action>>>,
    readback_done: std::sync::mpsc::Sender<()>,
    release: Arc<Mutex<std::sync::mpsc::Receiver<()>>>,
}

impl FakeEdit {
    pub(super) fn run(&self, job: &Job, stop: &AtomicBool) -> Result<EditOutcome, String> {
        let held = Cell::new(false);
        let writes = Cell::new(0);
        let current = job.current.as_ref().unwrap();
        let original = job.original.as_ref().unwrap();
        let control = job.action == Action::Controls;
        let channel = |index| {
            (
                PodxtRequest::QueryGlobal(Global::CurrentChannel),
                vec![0xf0, 0, 1, 0x0c, 3, 0x56, 17, 0, 0, 0, index, 0xf7],
            )
        };
        let before_channel = if !control && self.outcome == ControlResult::ChannelChangedBeforeUndo
        {
            4
        } else {
            3
        };
        let mut script = VecDeque::from([
            (
                PodxtRequest::Identity,
                variax_workbench::identity_stage::bridge_reply(),
            ),
            channel(before_channel),
        ]);
        if before_channel == 3 {
            let value = if control {
                assert_eq!(job.targets, [drive(45)]);
                45
            } else {
                assert_eq!(job.action, Action::Undo);
                assert!(job.targets.is_empty());
                original.tone.cc(13)
            };
            let readback = if control && self.outcome == ControlResult::BadReadback {
                43
            } else {
                value
            };
            let after_channel = if control && self.outcome == ControlResult::ChannelChanged {
                4
            } else {
                3
            };
            script.extend([
                (PodxtRequest::EditBuffer, current.tone.raw.clone()),
                channel(3),
                (
                    PodxtRequest::Set(Control::Drive, Value::new(value).unwrap()),
                    Vec::new(),
                ),
                (PodxtRequest::EditBuffer, sample(readback, 3).tone.raw),
                channel(after_channel),
            ]);
        }

        let result = edit_transaction(
            job,
            stop,
            || {
                assert!(!held.replace(true));
                Ok(FakeLock(&held))
            },
            |_, run| {
                let expected_actions = if control {
                    vec![]
                } else {
                    vec![Action::Controls]
                };
                assert!(*self.actions.lock().unwrap() == expected_actions);
                let result = run(&mut FakeLink {
                    held: &held,
                    script,
                    reply: None,
                    time: &Cell::new(Duration::ZERO),
                    writes: &writes,
                });
                if control {
                    self.readback_done.send(()).unwrap();
                    self.release
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(5))
                        .unwrap();
                    if self.outcome == ControlResult::PendingReply {
                        job.reply_pending.store(true, Ordering::Release);
                    }
                }
                self.actions.lock().unwrap().push(job.action);
                if control && self.outcome == ControlResult::CloseFailed {
                    Err("Checked release failed.".into())
                } else {
                    result
                }
            },
        );
        assert_eq!(writes.get(), if before_channel == 3 { 7 } else { 2 });
        assert!(!held.get());
        result
    }
}

struct UndoFixture {
    editor: LiveEditor,
    device: detection::DeviceKey,
    ctx: egui::Context,
    readback_done: std::sync::mpsc::Receiver<()>,
    release: std::sync::mpsc::Sender<()>,
    actions: Arc<Mutex<Vec<Action>>>,
}

impl UndoFixture {
    fn new(outcome: ControlResult) -> Self {
        let ctx = egui::Context::default();
        let (app, _) = super::super::tests::connected_fixture(&ctx);
        let device = app.podxt().live.device.clone().unwrap();
        let (readback, readback_done) = std::sync::mpsc::channel();
        let (release, finish) = std::sync::mpsc::channel();
        let actions = Arc::new(Mutex::new(Vec::new()));
        let mut editor = LiveEditor {
            fake_edit: Some(FakeEdit {
                outcome,
                actions: Arc::clone(&actions),
                readback_done: readback,
                release: Arc::new(Mutex::new(finish)),
            }),
            ..Default::default()
        };
        editor.completed(Action::Get, Ok(sample(20, 3)));
        editor.set_live(true);
        Self {
            editor,
            device,
            ctx,
            readback_done,
            release,
            actions,
        }
    }

    fn queue_undo_while_controls_run(&mut self) {
        self.editor.queue_stage_target(drive(45), &self.ctx);
        self.editor.poll(Some(&self.device), true, &self.ctx);
        self.readback_done
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        self.editor.queue_stage_target(drive(60), &self.ctx);
        assert!(self.editor.can_request_undo(&self.device));
        self.editor.request_undo(&self.device, &self.ctx);
        assert!(!self.editor.has_pending());
        assert!(
            self.editor
                .status_text()
                .unwrap()
                .starts_with("Undo pending")
        );
        assert!(!self.editor.can_request_undo(&self.device));
        self.editor.request_undo(&self.device, &self.ctx);
        self.editor.queue_stage_target(drive(70), &self.ctx);
        assert!(
            !self.editor.has_pending(),
            "controls cannot queue behind Undo"
        );
        assert!(
            self.editor
                .poll(Some(&self.device), true, &self.ctx)
                .is_none()
        );
        assert!(
            self.actions.lock().unwrap().is_empty(),
            "no Undo before checked release"
        );
    }

    fn poll_finished(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while self
            .editor
            .worker
            .as_ref()
            .is_some_and(|worker| !worker.thread.as_ref().unwrap().is_finished())
        {
            assert!(Instant::now() < deadline, "fake editor worker timed out");
            std::thread::yield_now();
        }
        self.editor.poll(Some(&self.device), true, &self.ctx);
    }

    fn assert_actions(&self, expected: &[Action]) {
        assert!(*self.actions.lock().unwrap() == expected);
    }
}

#[test]
fn undo_discards_unsent_live_targets_without_sending_or_applying_staged_edits() {
    let mut fixture = UndoFixture::new(ControlResult::Verified);
    let editor = &mut fixture.editor;
    editor.queue_stage_target(drive(60), &fixture.ctx);
    editor.request_undo(&fixture.device, &fixture.ctx);
    assert!(!editor.has_pending());
    assert!(!editor.busy());
    editor.poll(Some(&fixture.device), true, &fixture.ctx);
    assert_eq!(
        editor.session.as_ref().unwrap().current.tone,
        sample(20, 3).tone
    );

    editor.set_live(false);
    editor.queue_stage_target(drive(60), &fixture.ctx);
    assert!(!editor.can_request_undo(&fixture.device));
    editor.request_undo(&fixture.device, &fixture.ctx);
    assert_eq!(editor.pending.0, [drive(60)]);
    assert!(editor.worker.is_none());
    fixture.assert_actions(&[]);
}

#[test]
fn queued_undo_runs_once_after_the_control_readback_and_checked_release() {
    let mut fixture = UndoFixture::new(ControlResult::Verified);
    fixture.queue_undo_while_controls_run();
    assert_eq!(fixture.editor.draft_counts(), (0, 0));
    fixture.release.send(()).unwrap();
    fixture.poll_finished();
    assert_eq!(fixture.editor.draft_counts(), (1, 0));
    assert!(fixture.editor.worker.as_ref().unwrap().action == Action::Undo);
    fixture.poll_finished();
    assert_eq!(fixture.editor.draft_counts(), (0, 0));
    assert_eq!(
        fixture.editor.status_text(),
        Some("Original snapshot verified byte for byte.")
    );
    assert_eq!(
        fixture.editor.session.as_ref().unwrap().current.tone,
        sample(20, 3).tone
    );
    assert!(!fixture.editor.busy());
    fixture.assert_actions(&[Action::Controls, Action::Undo]);
}

#[test]
fn queued_undo_drops_after_unverified_controls_or_ambiguous_reply() {
    for (outcome, reason) in [
        (ControlResult::BadReadback, "read back"),
        (ControlResult::CloseFailed, "Checked release failed"),
        (ControlResult::PendingReply, "reply may still be pending"),
        (ControlResult::ChannelChanged, "playing channel changed"),
    ] {
        let mut fixture = UndoFixture::new(outcome);
        fixture.queue_undo_while_controls_run();
        fixture.release.send(()).unwrap();
        fixture.poll_finished();
        let status = fixture.editor.status_text().unwrap();
        assert!(
            status.starts_with("Undo cancelled.") && status.contains(reason),
            "{status}"
        );
        assert!(!fixture.editor.busy());
        assert!(!fixture.editor.can_request_undo(&fixture.device));
        fixture.assert_actions(&[Action::Controls]);
    }
}

#[test]
fn queued_undo_rechecks_the_playing_channel_before_restore_writes() {
    let mut fixture = UndoFixture::new(ControlResult::ChannelChangedBeforeUndo);
    fixture.queue_undo_while_controls_run();
    fixture.release.send(()).unwrap();
    fixture.poll_finished();
    fixture.poll_finished();
    assert!(
        fixture
            .editor
            .status_text()
            .unwrap()
            .contains("playing channel changed")
    );
    assert!(fixture.editor.needs_refresh());
    fixture.assert_actions(&[Action::Controls, Action::Undo]);
}

#[test]
fn queued_undo_is_cleared_on_attachment_change_or_cancel() {
    for detach in [true, false] {
        let mut fixture = UndoFixture::new(ControlResult::Verified);
        fixture.queue_undo_while_controls_run();
        if detach {
            fixture.editor.cancel();
            fixture.editor.detach_attachment();
        } else {
            fixture
                .editor
                .cancel_queued_undo("The pending restore was cancelled.");
            assert!(
                !fixture
                    .editor
                    .worker
                    .as_ref()
                    .unwrap()
                    .stop
                    .load(Ordering::Acquire)
            );
        }
        fixture.release.send(()).unwrap();
        fixture.poll_finished();
        assert!(
            fixture
                .editor
                .status_text()
                .unwrap()
                .starts_with("Undo cancelled.")
        );
        assert!(!fixture.editor.busy());
        if !detach {
            assert_eq!(
                fixture.editor.session.as_ref().unwrap().current.tone,
                sample(45, 3).tone
            );
        }
        fixture.assert_actions(&[Action::Controls]);
    }
}

#[test]
fn undo_never_queues_behind_non_control_jobs() {
    let mut fixture = UndoFixture::new(ControlResult::Verified);
    for action in [
        Action::Get,
        Action::Send,
        Action::ResendWholeTone,
        Action::Undo,
        Action::Recall(sample(20, 3).channel),
    ] {
        fixture.editor.worker = Some(EditWorker {
            action,
            device: Some(fixture.device.clone()),
            stop: Arc::default(),
            targets: Vec::new(),
            thread: None,
        });
        fixture.editor.request_undo(&fixture.device, &fixture.ctx);
        assert!(fixture.editor.queued_undo.is_none());
    }
}

#[test]
fn queued_undo_expires_when_a_fresh_read_updates_the_snapshot() {
    let mut fixture = UndoFixture::new(ControlResult::Verified);
    fixture.queue_undo_while_controls_run();
    let other_channel = sample(87, 4);
    fixture.editor.accept_follow(other_channel.clone());
    fixture.release.send(()).unwrap();
    fixture.poll_finished();
    assert_eq!(
        fixture.editor.status_text(),
        Some("Undo cancelled. The playing tone changed.")
    );
    let active = fixture.editor.session.as_ref().unwrap();
    assert_eq!(active.current.channel, other_channel.channel);
    assert_eq!(active.original.tone, other_channel.tone);
    fixture.assert_actions(&[Action::Controls]);
}
