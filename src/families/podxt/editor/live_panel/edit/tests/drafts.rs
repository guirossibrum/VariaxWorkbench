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

fn checked<T>(
    editor: &LiveEditor,
    action: Action,
    mut script: Script,
    connect: impl FnOnce(&mut MidiSession<&mut dyn Link>) -> Result<T, String>,
) -> Result<T, String> {
    let held = Cell::new(false);
    let result = with_session(
        || false,
        &editor.reply_pending,
        || {
            held.set(true);
            Ok(FakeLock(&held))
        },
        |_, run| {
            assert!(held.get());
            run(&mut ScriptedLink {
                script: &mut script,
                reply: None,
                now: Duration::ZERO,
            })
        },
        EditPolicy(action),
        connect,
    );
    assert!(script.is_empty());
    assert!(!held.get());
    result
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

#[test]
fn draft_replays_over_checked_fake_send_after_ui_and_physical_return() {
    let ctx = egui::Context::default();
    let (app, _) = super::super::super::tests::connected_fixture(&ctx);
    let device = app.podxt().live.device.as_ref().unwrap();
    let mut a = sample(0, 3);
    a.tone = valid_dump("Saved A");
    let mut b = sample(0, 4);
    b.tone = valid_dump("Saved B");
    let mut draft = a.clone();
    draft.tone.raw[39 + usize::from(Control::Drive.cc())] ^= 0x3f;
    let directory = tempfile::tempdir().unwrap();
    let backup_path = complete_backup(directory.path(), "Saved B", "Saved A");
    let backup = podxt::verify_tone_backup(&backup_path, a.channel).unwrap();

    for physical in [false, true] {
        let mut editor = LiveEditor::default();
        editor.set_live(true);
        editor.publish_test_edit(device, a.clone(), draft.clone());
        let retained = editor.draft(a.channel).unwrap().clone();

        let mut outgoing = preflight(&draft);
        outgoing.extend([
            (PodxtRequest::Stored(a.channel).bytes(), a.tone.raw.clone()),
            packs(),
            channel(a.channel),
        ]);
        let action = Action::Recall(b.channel);
        let guarded = checked(&editor, action, outgoing, |session| {
            podxt::recall_live_channel(session, b.channel, None, || false)
        })
        .unwrap();
        assert!(matches!(&guarded, EditorRecall::ConfirmationRequired(_)));
        editor.complete_job(action, device, false, Ok(EditOutcome::Recall(guarded)));
        assert!(editor.awaiting_recall_confirmation());
        assert_eq!(editor.draft(a.channel), Some(&retained));

        editor.accept_follow(b.clone());
        assert!(editor.draft(b.channel).is_none());

        if physical {
            let mut script = preflight(&a);
            script.extend([packs(), channel(a.channel)]);
            let followed = checked(&editor, Action::Get, script, |session| {
                let mut outstanding = false;
                let result =
                    podxt::get_editor_buffer_tracking_reply(session, || false, &mut outstanding);
                assert!(!outstanding);
                result
            })
            .unwrap();
            // Inject the verified publication from the physical-channel reader.
            editor.accept_follow(followed);
        } else {
            let mut script = preflight(&b);
            script.extend([
                (PodxtRequest::Stored(b.channel).bytes(), b.tone.raw.clone()),
                (PodxtRequest::Stored(a.channel).bytes(), a.tone.raw.clone()),
                packs(),
                (PodxtRequest::EditBuffer.bytes(), b.tone.raw.clone()),
                channel(b.channel),
                (PodxtRequest::Program(a.channel).bytes(), Vec::new()),
                (PodxtRequest::EditBuffer.bytes(), a.tone.raw.clone()),
                channel(a.channel),
            ]);
            let action = Action::Recall(a.channel);
            let outcome = checked(&editor, action, script, |session| {
                podxt::recall_live_channel(session, a.channel, None, || false)
            })
            .unwrap();
            assert!(matches!(&outcome, EditorRecall::Playing(_)));
            editor.complete_job(action, device, false, Ok(EditOutcome::Recall(outcome)));
        }

        assert_eq!(editor.draft(a.channel), Some(&retained));
        assert!(!editor.busy(), "verified selection starts no replay worker");
        assert_eq!(
            editor.playing_draft_status(a.channel),
            Some("Draft retained · playing tone differs")
        );
        let confirmed = editor.session.as_ref().unwrap().current.clone();
        assert_eq!(confirmed.tone, a.tone);

        // Exercise the checked contract only on this scripted link. First
        // inject stale channel evidence, then an exact send and readback.
        let mut stale = preflight(&confirmed);
        stale.truncate(2);
        stale[1] = channel(b.channel);
        let send = |session: &mut MidiSession<&mut dyn Link>| {
            podxt::send_whole_tone(
                session,
                &confirmed,
                WholeToneSource::Payload(&retained),
                &backup,
                || false,
            )
        };
        assert!(
            checked(&editor, Action::ResendWholeTone, stale, send)
                .unwrap_err()
                .contains("playing channel changed")
        );
        assert_eq!(editor.draft(a.channel), Some(&retained));

        let upload_script = |readback: &EditBuffer| {
            let mut script = preflight(&confirmed);
            script.extend([
                packs(),
                (PodxtRequest::Stored(a.channel).bytes(), a.tone.raw.clone()),
                channel(a.channel),
                (draft.tone.raw.clone(), Vec::new()),
                (PodxtRequest::EditBuffer.bytes(), readback.raw.clone()),
                channel(a.channel),
            ]);
            script
        };
        let mismatch = upload_script(&a.tone);
        let error = checked(&editor, Action::ResendWholeTone, mismatch, send).unwrap_err();
        assert!(error.contains("readback differs"), "{error}");
        editor.complete_job(Action::ResendWholeTone, device, false, Err(error));
        assert_eq!(editor.draft(a.channel), Some(&retained));
        assert!(!editor.recall_ready());

        // A fresh verification is required before a subsequent attempt. This
        // is a separate fake transaction, with no automatic retry.
        editor.accept_follow(confirmed.clone());
        let replayed = checked(
            &editor,
            Action::ResendWholeTone,
            upload_script(&draft.tone),
            send,
        )
        .unwrap();
        editor.complete_job(
            Action::ResendWholeTone,
            device,
            false,
            Ok(EditOutcome::Buffer(replayed)),
        );
        assert_eq!(
            editor.playing_draft_status(a.channel),
            Some("Session draft · playing")
        );
        assert_eq!(editor.draft(a.channel), Some(&retained));
    }
}
