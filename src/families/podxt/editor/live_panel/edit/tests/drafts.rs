use super::*;
use crate::editor::podxt_panel::live_panel::tests::{complete_backup, valid_dump};
use line6_devices::session::{Path as MidiPath, Request};

#[test]
fn checked_edits_keep_separate_drafts_through_failures_and_attachment_changes() {
    let ctx = egui::Context::default();
    let (app, _) = super::super::super::tests::connected_fixture(&ctx);
    let device = app.podxt().live.device.as_ref().unwrap();
    let a = sample(20, 3);
    let b = sample(30, 4);
    let mut editor = LiveEditor::default();
    let draft_a = sample(45, 3);
    editor.publish_test_edit(device, a.clone(), draft_a.clone());
    let saved_draft_a = TonePayload::from_dump(&draft_a.tone).unwrap();
    assert_eq!(editor.draft(a.channel), Some(&saved_draft_a));

    for action in [Action::Get, Action::Recall(b.channel), Action::Controls] {
        assert!(
            editor
                .complete_job(action, device, false, Err("Unverified reply".into()))
                .is_none()
        );
        assert_eq!(editor.draft(a.channel), Some(&saved_draft_a));
        assert_eq!(
            editor.playing_draft_status(a.channel),
            Some("Draft retained · playing tone unverified")
        );
    }
    editor.accept_follow(b.clone());
    for (stopped, pending_reply) in [(true, false), (false, true)] {
        editor.reply_pending.store(pending_reply, Ordering::Release);
        editor.complete_job(
            Action::Controls,
            device,
            stopped,
            Ok(EditOutcome::Buffer(sample(47, 4))),
        );
        assert!(editor.draft(b.channel).is_none());
        editor.reply_pending.store(false, Ordering::Release);
        editor.accept_follow(b.clone());
    }

    editor.complete_job(
        Action::Send,
        device,
        false,
        Ok(EditOutcome::Buffer(sample(56, 4))),
    );
    assert_eq!(editor.draft_counts(), (2, 0));
    editor.complete_job(Action::Undo, device, false, Ok(EditOutcome::Buffer(b)));
    assert_eq!(editor.draft_counts(), (1, 0));

    editor.accept_follow(a.clone());
    editor.complete_job(
        Action::Controls,
        device,
        false,
        Ok(EditOutcome::Buffer(a.clone())),
    );
    assert_eq!(editor.draft(a.channel), Some(&saved_draft_a));

    editor.detach_attachment();
    // Even a fresh read of the same channel and tone cannot rebind the old draft.
    editor.accept_follow(a.clone());
    assert_eq!(editor.draft_counts(), (0, 1));
    assert!(editor.draft(a.channel).is_none());
    assert_eq!(
        editor.drafts.previous_attachments[0][&a.channel.address()],
        saved_draft_a
    );

    assert_eq!(editor.session.unwrap().current.tone, a.tone);
}

#[test]
fn only_matching_verified_explicit_store_clears_the_channel_draft() {
    let ctx = egui::Context::default();
    let (app, _) = super::super::super::tests::connected_fixture(&ctx);
    let device = app.podxt().live.device.as_ref().unwrap();
    let mut editor = LiveEditor::default();
    let original = sample(20, 3);
    let edited = sample(48, 3);
    editor.publish_test_edit(device, original.clone(), edited.clone());
    editor.reconcile_stored(&podxt::StoredTone {
        location: original.channel,
        tone: original.tone,
    });
    editor.reconcile_stored(&podxt::StoredTone {
        location: sample(20, 4).channel,
        tone: edited.tone.clone(),
    });
    assert_eq!(editor.draft_counts(), (1, 0));
    editor.reconcile_stored(&podxt::StoredTone {
        location: edited.channel,
        tone: edited.tone.clone(),
    });
    assert_eq!(editor.draft_counts(), (0, 0));
    assert_eq!(editor.session.unwrap().current.tone, edited.tone);
}

type Script = VecDeque<(Vec<u8>, Vec<u8>)>;

struct ScriptedLink<'a> {
    script: &'a mut Script,
    reply: Option<Vec<u8>>,
    now: Duration,
}

impl Link for ScriptedLink<'_> {
    fn path(&self) -> MidiPath {
        MidiPath::PodxtMidi
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (expected, reply) = self.script.pop_front().expect("unexpected MIDI request");
        assert_eq!(bytes, expected);
        self.reply = (!reply.is_empty()).then_some(reply);
        Ok(())
    }

    fn read(&mut self, duration: Duration) -> Result<Option<Vec<u8>>, String> {
        self.now += duration;
        Ok(self.reply.take())
    }

    fn elapsed(&self) -> Duration {
        self.now
    }

    fn pause(&mut self, duration: Duration) {
        self.now += duration;
    }
}

fn channel(slot: Location) -> (Vec<u8>, Vec<u8>) {
    let index = slot.index() as u8;
    (
        PodxtRequest::QueryGlobal(Global::CurrentChannel).bytes(),
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
}

fn preflight(current: &EditorBuffer) -> Script {
    VecDeque::from([
        (
            PodxtRequest::Identity.bytes(),
            variax_workbench::identity_stage::bridge_reply(),
        ),
        channel(current.channel),
        (PodxtRequest::EditBuffer.bytes(), current.tone.raw.clone()),
    ])
}

fn packs() -> (Vec<u8>, Vec<u8>) {
    (
        PodxtRequest::InstalledPacks.bytes(),
        vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7],
    )
}

fn follow_script(current: &EditorBuffer) -> Script {
    let mut script = preflight(current);
    script.extend([packs(), channel(current.channel)]);
    script
}

fn recall_script(from: &EditorBuffer, to: &EditorBuffer, confirmed: bool) -> Script {
    let mut script = preflight(from);
    if !confirmed {
        script.push_back((
            PodxtRequest::Stored(from.channel).bytes(),
            from.tone.raw.clone(),
        ));
    }
    script.extend([
        (
            PodxtRequest::Stored(to.channel).bytes(),
            to.tone.raw.clone(),
        ),
        packs(),
        (PodxtRequest::EditBuffer.bytes(), from.tone.raw.clone()),
        channel(from.channel),
        (PodxtRequest::Program(to.channel).bytes(), Vec::new()),
        (PodxtRequest::EditBuffer.bytes(), to.tone.raw.clone()),
        channel(to.channel),
    ]);
    script
}

fn replay_script(saved: &EditorBuffer, draft: &EditorBuffer, readback: &EditBuffer) -> Script {
    let mut script = preflight(saved);
    script.extend([
        packs(),
        (
            PodxtRequest::Stored(saved.channel).bytes(),
            saved.tone.raw.clone(),
        ),
        channel(saved.channel),
        (draft.tone.raw.clone(), Vec::new()),
        (PodxtRequest::EditBuffer.bytes(), readback.raw.clone()),
        channel(saved.channel),
    ]);
    script
}

type Release = Box<dyn FnOnce() -> Result<(), String> + Send>;
type Dispatches = Arc<Mutex<VecDeque<(Action, Script, Release)>>>;

struct Fixture {
    ctx: egui::Context,
    app: Workbench,
    panel: LivePanel,
    a: EditorBuffer,
    b: EditorBuffer,
    draft: EditorBuffer,
    dispatches: Dispatches,
}

impl Fixture {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let (mut app, _) = super::super::super::tests::connected_fixture(&ctx);
        let mut panel = std::mem::take(&mut app.podxt_mut().live);
        let device = panel.device.clone().unwrap();
        let mut a = sample(0, 3);
        a.tone = valid_dump("Saved A");
        let mut b = sample(0, 4);
        b.tone = valid_dump("Saved B");
        let mut draft = a.clone();
        draft.tone.raw[39 + usize::from(Control::Drive.cc())] ^= 0x3f;

        panel
            .editor
            .publish_test_edit(&device, a.clone(), draft.clone());
        panel.editor.accept_follow(a.clone());
        panel.resume_live = true;
        panel.accept_editor_read(a.clone(), &ctx);
        panel.live_since = Some(Instant::now() - Duration::from_secs(1));
        panel.startup = None;
        for saved in [&a, &b] {
            panel
                .snapshot
                .library
                .insert(saved.channel.address(), saved.tone.clone());
        }
        let directory = tempfile::tempdir().unwrap();
        let backup = complete_backup(directory.path(), "Saved A", "Saved A");
        panel.editor.replay_backups.insert(
            a.channel.address(),
            Arc::new(podxt::verify_tone_backup(&backup, a.channel).unwrap()),
        );

        let dispatches = Dispatches::default();
        let queue = Arc::clone(&dispatches);
        panel.editor.fake_dispatch = Some(Arc::new(move |job, stop| {
            let (action, mut script, release) = queue
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected edit dispatch");
            assert_eq!(job.action, action);
            let held = Cell::new(false);
            let result = edit_transaction(
                job,
                stop,
                || {
                    assert!(!held.replace(true));
                    Ok(FakeLock(&held))
                },
                |_, run| {
                    assert!(held.get());
                    let result = run(&mut ScriptedLink {
                        script: &mut script,
                        reply: None,
                        now: Duration::ZERO,
                    });
                    assert!(held.get());
                    assert!(
                        script.is_empty(),
                        "{:?} did not exhaust its expected traffic",
                        job.action
                    );
                    // The script whitelists each request, including every write.
                    // A store or any extra upload fails the test.
                    release()?;
                    result
                },
            );
            assert!(!held.get());
            result
        }));
        Self {
            ctx,
            app,
            panel,
            a,
            b,
            draft,
            dispatches,
        }
    }

    fn expect(&self, action: Action, script: Script) {
        self.expect_release(action, script, || Ok(()));
    }

    fn expect_release(
        &self,
        action: Action,
        script: Script,
        release: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) {
        self.dispatches
            .lock()
            .unwrap()
            .push_back((action, script, Box::new(release)));
    }

    fn notice(&mut self, target: Location) {
        let tab = self.app.tabs.entries[0].device.as_mut().unwrap();
        let observed_at = tab
            .device
            .podxt_program
            .map_or(self.panel.live_since.unwrap(), |notice| {
                notice.observed_at + Duration::from_millis(1)
            });
        tab.device.podxt_program = Some(detection::SimulatedPodxtProgram {
            channel: target.index() as u8,
            observed_at,
        });
        assert!(!self.panel.set_device(Some(tab)));
    }

    fn select(&mut self, target: Location, physical: bool) {
        if physical {
            self.notice(target);
            self.panel.poll_follow(&self.ctx);
        } else {
            self.panel.recall(target, &self.ctx);
        }
        assert!(
            self.panel.editor.worker.is_some(),
            "selection was not dispatched"
        );
    }

    fn wait_worker(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let thread = self
            .panel
            .editor
            .worker
            .as_ref()
            .unwrap()
            .thread
            .as_ref()
            .unwrap();
        while !thread.is_finished() {
            assert!(Instant::now() < deadline, "worker did not finish");
            std::thread::yield_now();
        }
    }

    fn finish(&mut self) {
        self.wait_worker();
        self.panel.poll(&self.ctx);
    }

    fn assert_retained(&self) {
        let editor = &self.panel.editor;
        assert_eq!(
            editor.draft(self.a.channel),
            Some(&TonePayload::from_dump(&self.draft.tone).unwrap())
        );
        assert!(editor.draft(self.b.channel).is_none());
        for saved in [&self.a, &self.b] {
            assert_eq!(self.panel.stored_tone(saved.channel), Some(&saved.tone));
        }
    }

    fn assert_idle(&self) {
        assert!(!self.panel.busy());
        assert!(self.dispatches.lock().unwrap().is_empty());
    }
}

#[test]
fn dispatcher_replays_a_retained_draft_for_app_and_footswitch_selection() {
    for physical in [false, true] {
        let mut f = Fixture::new();
        f.panel.editor.accept_follow(f.draft.clone());
        f.panel.accept_editor_read(f.draft.clone(), &f.ctx);
        if physical {
            f.expect(Action::Follow(f.b.channel), follow_script(&f.b));
            f.expect(Action::Follow(f.a.channel), follow_script(&f.a));
        } else {
            let mut prompt = preflight(&f.draft);
            prompt.extend([
                (
                    PodxtRequest::Stored(f.a.channel).bytes(),
                    f.a.tone.raw.clone(),
                ),
                packs(),
                channel(f.a.channel),
            ]);
            f.expect(Action::Recall(f.b.channel), prompt);
            f.expect(
                Action::Recall(f.b.channel),
                recall_script(&f.draft, &f.b, true),
            );
            f.expect(
                Action::Recall(f.a.channel),
                recall_script(&f.b, &f.a, false),
            );
        }
        f.expect(
            Action::ReplayDraft(f.a.channel),
            replay_script(&f.a, &f.draft, &f.draft.tone),
        );

        f.select(f.b.channel, physical);
        f.finish();
        if !physical {
            assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.draft.tone));
            let device = f.panel.device.clone().unwrap();
            let confirmation = f.panel.editor.take_recall_confirmation(&device).unwrap();
            f.panel.editor.start_job(
                Action::Recall(f.b.channel),
                &device,
                &f.ctx,
                Some(confirmation),
                None,
            );
            f.finish();
        }
        assert!(!f.panel.busy(), "B has no draft");
        assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.b.tone));

        f.select(f.a.channel, physical);
        f.finish();
        assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.a.tone));
        assert_eq!(
            f.panel.editor.progress_label(),
            Some("Restore session draft")
        );
        f.finish();
        assert_eq!(f.panel.playing_channel(), Some(f.a.channel));
        assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.draft.tone));
        assert_eq!(
            f.panel.playing_draft_status(),
            Some("Session draft · playing")
        );
        f.assert_retained();
        f.assert_idle();
    }
}

#[test]
fn replay_requires_the_observed_target_a_matching_backup_and_a_fresh_link() {
    let mut f = Fixture::new();
    // The notice names A but the checked read sees B. Keep following B
    // without using A's draft or treating B as a selection for replay.
    f.expect(Action::Follow(f.a.channel), follow_script(&f.b));
    f.select(f.a.channel, true);
    f.finish();
    assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.b.tone));
    f.assert_idle();

    let backup = f
        .panel
        .editor
        .replay_backups
        .remove(&f.a.channel.address())
        .unwrap();
    f.expect(
        Action::Recall(f.a.channel),
        recall_script(&f.b, &f.a, false),
    );
    f.select(f.a.channel, false);
    f.finish();
    assert!(
        f.panel
            .editor
            .status_text()
            .unwrap()
            .contains("Verify a backup")
    );
    f.assert_idle();
    f.panel
        .editor
        .replay_backups
        .insert(f.a.channel.address(), backup);

    f.expect(Action::Recall(f.a.channel), {
        let mut script = preflight(&f.a);
        script.extend([packs(), channel(f.a.channel)]);
        script
    });
    let mut stale = preflight(&f.a);
    stale.truncate(2);
    stale[1] = channel(f.b.channel);
    f.expect(Action::ReplayDraft(f.a.channel), stale);
    f.select(f.a.channel, false);
    f.finish();
    f.finish();
    assert!(
        f.panel
            .editor
            .status_text()
            .unwrap()
            .contains("playing channel changed")
    );
    assert!(f.panel.editor.needs_refresh());
    f.assert_retained();
    f.assert_idle();
}

#[test]
fn failed_selection_release_replay_readback_or_replay_release_keeps_the_draft() {
    for (selection_release_fails, readback_matches) in [(true, true), (false, false), (false, true)]
    {
        let mut f = Fixture::new();
        f.expect_release(
            Action::Follow(f.a.channel),
            follow_script(&f.a),
            move || {
                if selection_release_fails {
                    Err("selection release failed".into())
                } else {
                    Ok(())
                }
            },
        );
        if !selection_release_fails {
            let readback = if readback_matches {
                &f.draft.tone
            } else {
                &f.a.tone
            };
            f.expect_release(
                Action::ReplayDraft(f.a.channel),
                replay_script(&f.a, &f.draft, readback),
                move || {
                    if readback_matches {
                        Err("replay release failed".into())
                    } else {
                        Ok(())
                    }
                },
            );
        }
        f.select(f.a.channel, true);
        f.finish();
        let error = if selection_release_fails {
            "selection release failed"
        } else {
            f.finish();
            if readback_matches {
                "replay release failed"
            } else {
                "readback differs"
            }
        };
        assert!(f.panel.editor.status_text().unwrap().contains(error));
        assert!(f.panel.editor.needs_refresh());
        assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.a.tone));
        f.assert_retained();
        f.assert_idle();
    }
}

#[test]
fn a_newer_program_notice_supersedes_a_checked_selection_before_replay_dispatch() {
    let mut f = Fixture::new();
    f.expect(Action::Follow(f.a.channel), follow_script(&f.a));
    f.select(f.a.channel, true);
    f.wait_worker();
    f.notice(f.b.channel);
    f.finish();
    assert!(
        !f.panel.busy(),
        "the superseded selection cannot dispatch replay"
    );

    f.expect(Action::Follow(f.b.channel), follow_script(&f.b));
    f.panel.poll_follow(&f.ctx);
    f.finish();
    assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.b.tone));
    f.assert_retained();
    f.assert_idle();
}

#[test]
fn stale_replay_completion_cannot_publish_after_a_notice_or_reattachment() {
    for detach in [false, true] {
        let mut f = Fixture::new();
        f.expect(Action::Follow(f.a.channel), follow_script(&f.a));
        let (closing, wait_for_close) = std::sync::mpsc::channel();
        let (release, wait_for_release) = std::sync::mpsc::channel();
        f.expect_release(
            Action::ReplayDraft(f.a.channel),
            replay_script(&f.a, &f.draft, &f.draft.tone),
            move || {
                closing.send(()).unwrap();
                wait_for_release
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
                Ok(())
            },
        );

        f.select(f.a.channel, true);
        f.finish();
        wait_for_close.recv_timeout(Duration::from_secs(5)).unwrap();
        f.panel.poll(&f.ctx);
        assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.a.tone));
        if detach {
            assert!(!f.panel.set_device(None));
            assert!(f.panel.set_device(f.app.tabs.entries[0].device.as_ref()));
            f.panel.startup = None;
        } else {
            f.notice(f.b.channel);
        }
        release.send(()).unwrap();
        f.finish();

        if detach {
            assert!(f.panel.snapshot.tone.is_none());
            assert_eq!(f.panel.draft_counts(), (0, 1));
            assert!(f.panel.editor.replay_backups.is_empty());
        } else {
            assert_eq!(f.panel.snapshot.tone.as_ref(), Some(&f.a.tone));
            assert!(f.panel.editor.needs_refresh());
            f.assert_retained();
        }
        f.assert_idle();
    }
}
