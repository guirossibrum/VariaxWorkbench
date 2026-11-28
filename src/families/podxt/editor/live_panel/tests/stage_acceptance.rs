use super::super::edit::tests::{ScriptedAction, ScriptedKind};
use super::*;
use crate::editor::FakeDialogs;
use serde_json::Value as LogRow;
use variax_workbench::session_log::{SessionGuard, SessionLog, Severity};

fn visible_text(frame: &egui::FullOutput, label: &str) -> Option<Pos2> {
    fn find(shape: &egui::epaint::Shape, label: &str) -> Option<Pos2> {
        match shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|item| find(item, label)),
            _ => None,
        }
    }
    frame.shapes.iter().find_map(|item| {
        let pos = find(&item.shape, label)?;
        item.clip_rect.contains(pos).then_some(pos)
    })
}

fn click_text(ctx: &egui::Context, app: &mut Workbench, label: &str) {
    let frame = render_stage(ctx, app, vec![]);
    let pos = visible_text(&frame, label)
        .unwrap_or_else(|| panic!("no visible {label:?}: {}", stage_frame(ctx, app)));

    click_position(ctx, app, pos);
}

fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn click_position(ctx: &egui::Context, app: &mut Workbench, pos: Pos2) {
    render_stage(
        ctx,
        app,
        vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
    );
    render_stage(ctx, app, vec![pointer(pos, false)]);
}

fn current_fixture(ctx: &egui::Context) -> (Workbench, Presence) {
    let (mut app, presence) = connected_fixture(ctx);
    let panel = &mut app.podxt_mut().live;
    let first_slot = Location::new(BankKind::Tones, 0).unwrap();
    panel
        .snapshot
        .apply(ReadUpdate::Channel(first_slot), Instant::now());
    let playing = valid_dump("Playing Tone");
    let saved = valid_dump("Saved Channel");

    for (command, script, expected_writes) in [
        (
            ReadCommand::Current,
            VecDeque::from([
                (
                    PodxtRequest::Identity,
                    variax_workbench::identity_stage::bridge_reply(),
                ),
                (PodxtRequest::EditBuffer, playing.raw.clone()),
            ]),
            2,
        ),
        (
            ReadCommand::ToneBank,
            std::iter::once((
                PodxtRequest::Identity,
                variax_workbench::identity_stage::bridge_reply(),
            ))
            .chain(
                Location::all(&[BankKind::Tones])
                    .map(|slot| (PodxtRequest::Stored(slot), saved.raw.clone())),
            )
            .collect(),
            129,
        ),
    ] {
        let held = Cell::new(false);
        let writes = Cell::new(0);
        transaction(
            command,
            &AtomicBool::new(false),
            &AtomicBool::new(false),
            || {
                assert!(!held.replace(true));
                Ok(FakeLock(&held))
            },
            |_, run| {
                run(&mut FakeLink {
                    held: &held,
                    writes: &writes,
                    script,
                    reply: None,
                    time: &Cell::new(Duration::ZERO),
                })
            },
            |update| panel.snapshot.apply(update, Instant::now()),
        )
        .unwrap();
        assert_eq!(writes.get(), expected_writes);
        assert!(!held.get());
    }
    assert_eq!(panel.snapshot.library_count, Some(128));
    (app, presence)
}

fn local_tone(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("local.l6t");
    let tone = pod_preset::PodTone::new_podxt_tone("Local Document").unwrap();
    std::fs::write(&path, tone.into_bytes()).unwrap();
    path
}

fn rendered(ctx: &egui::Context, app: &mut Workbench, labels: &[&str]) -> String {
    let visible = stage_frame(ctx, app);
    for label in labels {
        assert!(visible.contains(label), "missing {label:?}: {visible}");
    }
    visible
}

fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

fn finish_playing_read(ctx: &egui::Context, app: &mut Workbench, requests: usize) {
    assert_eq!(app.podxt().live.device_requests, requests);
    let deadline = Instant::now() + Duration::from_secs(2);
    while app.podxt().busy() {
        assert!(
            Instant::now() < deadline,
            "fake playing-tone read did not finish"
        );
        std::thread::yield_now();
        render_stage(ctx, app, vec![]);
    }
    assert_eq!(
        app.podxt().live.snapshot.tone.as_ref().unwrap().name(),
        "Read Live Tone"
    );
    assert!(!app.podxt().live.editor.needs_refresh());
    assert!(app.podxt().stage_is_current());
}

#[test]
fn playing_tone_read_uses_one_command_for_pointer_recovery_and_keyboard_read() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let live = &mut app.podxt_mut().live;
    live.startup = None;
    live.resume_live = false;
    live.editor.use_fake_get();
    assert!(live.editor.needs_refresh());
    let requests = live.device_requests;

    let label = "Read playing tone";
    let visible = rendered(
        &ctx,
        &mut app,
        &[label, "Apply to playing tone", "Store to POD…"],
    );
    assert_eq!(visible.lines().filter(|line| *line == label).count(), 1);
    assert!(!visible.contains("Open tone or bank…"));
    click_text(&ctx, &mut app, label);
    assert!(app.podxt().busy());
    assert!(!app.podxt().live.can_refresh_current());
    finish_playing_read(&ctx, &mut app, requests + 1);

    let frame = render_stage(&ctx, &mut app, vec![]);
    let pos = visible_text(&frame, label).expect("visible Read playing tone");
    assert!(
        (0..50).any(|_| {
            render_stage(&ctx, &mut app, vec![key_event(egui::Key::Tab, true)]);
            render_stage(&ctx, &mut app, vec![key_event(egui::Key::Tab, false)]);
            ctx.memory(|m| m.focused())
                .and_then(|id| ctx.read_response(id))
                .is_some_and(|response| response.rect.center().distance(pos) < 3.0)
        }),
        "Read playing tone is keyboard focusable"
    );
    render_stage(&ctx, &mut app, vec![key_event(egui::Key::Enter, true)]);
    render_stage(&ctx, &mut app, vec![key_event(egui::Key::Enter, false)]);
    finish_playing_read(&ctx, &mut app, requests + 2);
    click_text(&ctx, &mut app, "File actions");
    rendered(&ctx, &mut app, &["Save copy (disk)…"]);
}

#[test]
fn playing_tone_read_refuses_unavailable_route_staged_edits_and_pending_reply() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let live = &mut app.podxt_mut().live;
    live.startup = None;
    live.resume_live = false;
    assert!(!live.can_refresh_current());
    let requests = live.device_requests;
    click_text(&ctx, &mut app, "Read playing tone");
    assert_eq!(app.podxt().live.device_requests, requests);

    let live = &mut app.podxt_mut().live;
    live.editor.use_fake_get();
    assert!(live.can_refresh_current());
    live.editor.stage_test_target(podxt::LiveTarget::Basic(
        Control::Drive,
        podxt::Target::Value(podxt::Value::new(73).unwrap()),
    ));
    assert!(!live.can_refresh_current());
    click_text(&ctx, &mut app, "Read playing tone");
    let live = &mut app.podxt_mut().live;
    assert_eq!(live.device_requests, requests);
    assert!(live.has_staged_edits());
    live.editor.discard_staged();

    live.editor.reply_pending.store(true, Ordering::Release);
    assert!(!live.can_refresh_current());
    click_text(&ctx, &mut app, "Read playing tone");
    let live = &app.podxt().live;
    assert_eq!(live.device_requests, requests);
    assert!(!live.busy());
    assert_eq!(live.snapshot.tone.as_ref().unwrap().name(), "Stage Current");
}

#[test]
fn shell_open_keeps_a_dirty_podxt_document_when_the_prompt_is_cancelled() {
    let ctx = egui::Context::default();
    let mut app = startup_app(&ctx, None);
    app.new_tab(&super::super::super::FAMILY);
    app.podxt_mut().table_view_in_test = true;
    let directory = tempfile::tempdir().unwrap();
    let path = local_tone(directory.path());
    let bytes = std::fs::read(&path).unwrap();
    let next = directory.path().join("next.l6t");
    std::fs::write(&next, &bytes).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(path.clone()), Some(next)].into(),
        ..Default::default()
    });
    click_text(&ctx, &mut app, "Open…");
    let tab = app.tabs.active;
    let editor = app.podxt_mut();
    editor.pod_name = "Edited File".into();
    editor.apply_pod_name();
    assert!(editor.dirty());
    click_text(&ctx, &mut app, "Open…");
    assert!(app.podxt().stage_has_pending_document());
    stage_frame(&ctx, &mut app);
    click_text(&ctx, &mut app, "Cancel");

    assert!(!app.podxt().stage_has_pending_document());
    assert!(app.podxt().dirty());
    assert_eq!(app.podxt().source_path(), Some(path.as_path()));
    assert_eq!(app.tabs.active, tab);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn connected_file_and_device_library_previews_keep_the_playing_snapshot_until_disconnect() {
    let ctx = egui::Context::default();
    let (mut app, presence) = current_fixture(&ctx);
    let directory = tempfile::tempdir().unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(local_tone(directory.path()))].into(),
        ..Default::default()
    });
    rendered(
        &ctx,
        &mut app,
        &["Connected", "Playing · 1A", "Playing Tone", "On PODxt Live"],
    );
    let before = app.podxt().live.snapshot.tone.clone();
    let requests = app.podxt().live.device_requests;

    // A library click in the playing view with Live editing off cannot recall.
    click_text(&ctx, &mut app, "2A  Saved Channel");
    assert!(app.podxt().stage_is_current());
    assert_eq!(app.podxt().live.device_requests, requests);

    click_text(&ctx, &mut app, "Open…");
    rendered(
        &ctx,
        &mut app,
        &["File editor", "Local Document", "Back to playing tone"],
    );
    assert!(!app.podxt().stage_is_current());
    let editor = app.podxt_mut();
    editor.pod_name = "Dirty Document".into();
    editor.apply_pod_name();
    click_text(&ctx, &mut app, "2A  Saved Channel");
    rendered(
        &ctx,
        &mut app,
        &[
            "Open another PODxt document",
            "Dirty Document",
            "Playing · 1A",
        ],
    );
    click_text(&ctx, &mut app, "Cancel");

    click_text(&ctx, &mut app, "Back to playing tone");
    assert!(app.podxt().stage_is_current());
    assert!(app.podxt().dirty());
    app.podxt_mut().show_file_stage();
    rendered(
        &ctx,
        &mut app,
        &["Dirty Document", "Unsaved", "Playing · 1A"],
    );
    click_text(&ctx, &mut app, "2A  Saved Channel");
    click_text(&ctx, &mut app, "Discard and open");
    rendered(&ctx, &mut app, &["Device library copy", "Saved Channel"]);
    assert_eq!(app.podxt().live.snapshot.tone, before);
    assert_eq!(
        app.podxt().live.playing_channel(),
        Location::tone_from_label("1A")
    );
    assert_eq!(app.podxt().live.device_requests, requests);

    click_text(&ctx, &mut app, "Back to playing tone");
    assert!(app.podxt().stage_is_current());
    rendered(&ctx, &mut app, &["Playing Tone", "Playing · 1A"]);

    presence.attached.set(false);
    app.next_detection = Instant::now();
    let visible = rendered(
        &ctx,
        &mut app,
        &["Disconnected", "Device library copy", "Saved Channel"],
    );
    assert!(!visible.contains("Playing Tone"), "{visible}");
    assert!(!visible.contains("Playing ·"), "{visible}");
    assert!(!visible.contains("Back to playing tone"), "{visible}");
    assert!(app.podxt().live.snapshot.tone.is_none());
    assert!(app.podxt().live.snapshot.library.is_empty());
    assert_eq!(app.podxt().live.historical_library.len(), 128);
    assert!(!app.podxt().live.connected());
    assert!(!app.podxt().live.live_editing);
    assert_eq!(app.podxt().live.device_requests, requests);
    assert!(!app.podxt().busy());
}

#[test]
fn retained_draft_status_survives_file_preview_and_warns_before_close() {
    let ctx = egui::Context::default();
    let (mut app, presence) = current_fixture(&ctx);
    let slot = Location::tone_from_label("1A").unwrap();
    let saved = app.podxt().live.stored_tone(slot).unwrap().clone();
    let before = podxt::EditorBuffer {
        channel: slot,
        tone: valid_dump("Playing Tone"),
        packs: Packs(4),
    };
    let mut draft = before.clone();
    draft.tone.raw[39 + usize::from(Control::Drive.cc())] ^= 0x3f;
    {
        let panel = &mut app.podxt_mut().live;
        panel.editor.publish_test_edit(
            panel.device.as_ref().unwrap(),
            before.clone(),
            draft.clone(),
        );
        panel.update_playing(draft.clone());
    }
    rendered(
        &ctx,
        &mut app,
        &["Session draft · playing", "1A · Draft", "Saved Channel"],
    );
    assert_eq!(app.podxt().live.stored_tone(slot), Some(&saved));
    assert!(app.podxt().dirty());

    let directory = tempfile::tempdir().unwrap();
    let path = local_tone(directory.path());
    app.podxt_mut()
        .open_stage_document(std::fs::read(path).unwrap());
    rendered(&ctx, &mut app, &["Local Document", "1A · Draft"]);
    assert_eq!(app.podxt().live.snapshot.tone, Some(draft.tone));
    assert_eq!(app.podxt().live.draft_counts(), (1, 0));
    click_text(&ctx, &mut app, "Back to playing tone");
    assert!(app.podxt().stage_is_current());

    let requests = app.podxt().live.device_requests;
    presence.attached.set(false);
    app.next_detection = Instant::now();
    let visible = rendered(&ctx, &mut app, &["Drafts from a previous PODxt connection"]);
    assert!(!visible.contains("1A · Draft"));
    assert_eq!(app.podxt().live.draft_counts(), (0, 1));

    app.request_close(&ctx);
    rendered(
        &ctx,
        &mut app,
        &["Closing discards one retained PODxt channel draft."],
    );
    click_text(&ctx, &mut app, "Keep drafts");
    assert!(!app.permit_close);
    assert_eq!(app.podxt().live.draft_counts(), (0, 1));
    assert_eq!(app.podxt().live.device_requests, requests);

    let podxt_tab = app.tabs.active.unwrap();
    app.new_tab(crate::editor::registry::LEGACY_STARTUP);
    let variax_tab = app.tabs.active.unwrap();
    app.variax_mut()
        .change(|preset| preset.rename("Pending edit"));

    app.request_close(&ctx);
    click_text(&ctx, &mut app, "Discard drafts and close");
    click_text(&ctx, &mut app, "Keep editing");
    assert!(!app.permit_close);
    assert!(app.variax().dirty());
    assert_eq!(app.podxt().live.draft_counts(), (0, 1));

    app.request_close(&ctx);
    click_text(&ctx, &mut app, "Keep drafts");
    assert!(!app.permit_close);
    app.close_tab(podxt_tab, &ctx);
    click_text(&ctx, &mut app, "Discard drafts and close");
    assert!(app.tabs.index(podxt_tab).is_none());
    assert!(app.tabs.index(variax_tab).is_some());
    assert!(!app.permit_close);
}

#[test]
fn playing_channel_update_reveals_the_row_and_live_click_uses_checked_recall() {
    let ctx = egui::Context::default();
    let (mut app, _) = current_fixture(&ctx);
    let final_row = "32D  Saved Channel";
    let initial = render_stage(&ctx, &mut app, vec![]);
    assert!(visible_text(&initial, final_row).is_none());

    let panel = &mut app.podxt_mut().live;
    let slot = Location::tone_from_label("32D").unwrap();
    // Inject a completed physical program-change publication, without a device.
    panel.snapshot.channel = Some(slot);
    panel.snapshot.tone = panel.stored_tone(slot).cloned();
    panel.live_editing = true;
    panel.reveal_playing = true;

    let mut revealed = false;
    for _ in 0..24 {
        ctx.request_repaint();
        let frame = render_stage(&ctx, &mut app, vec![]);
        assert!(visible_text(&frame, "Playing · 32D").is_some());
        if visible_text(&frame, final_row).is_some() {
            revealed = true;
            break;
        }
    }
    assert!(
        revealed,
        "physical update should scroll to the playing channel"
    );

    let live = &mut app.podxt_mut().live;
    live.startup = None;
    live.editor.set_live(true);
    live.editor.use_fake_recall();
    let requests = live.device_requests;
    let log = EditorLog::attach(&mut app);
    click_text(&ctx, &mut app, "31D  Saved Channel");
    let deadline = Instant::now() + Duration::from_secs(2);
    while app.podxt().live.editor.busy() {
        assert!(
            Instant::now() < deadline,
            "fake channel recall did not finish"
        );
        std::thread::yield_now();
        render_stage(&ctx, &mut app, vec![]);
    }
    let panel = &app.podxt().live;
    assert_eq!(panel.device_requests, requests + 1);
    assert_eq!(panel.playing_channel(), Location::tone_from_label("31D"));
    assert_eq!(
        panel.snapshot.tone.as_ref().unwrap().name(),
        "Recalled tone"
    );
    let visible = rendered(&ctx, &mut app, &["Playing · 31D"]);
    assert!(!visible.contains("Read playing tone"));
    assert!(!visible.contains("Reconnect"));
    let target = Location::tone_from_label("31D").unwrap();
    let rows = operation_for(&log.read(&["Before recall", "Recalled tone"]), target);
    let checked = position(&rows, |row| row["evidence"] == "addressed_slot");
    assert_eq!(rows[checked]["source"], 127);
    assert_eq!(rows[checked]["target"], 123);
    assert!(rows.iter().any(program_sent));
}

#[test]
fn live_sidebar_protects_unsaved_tone_after_an_unobserved_device_save() {
    let ctx = egui::Context::default();
    let (mut app, _) = current_fixture(&ctx);
    let source = Location::tone_from_label("32D").unwrap();
    let target = Location::tone_from_label("1B").unwrap();
    let cached_source = valid_dump("Before recall");
    let panel = &mut app.podxt_mut().live;
    panel
        .snapshot
        .library
        .insert(source.address(), cached_source.clone());
    let downloaded = panel.snapshot.library.clone();
    assert_eq!(downloaded.len(), 128);

    // The user has since saved a different Drive value on the POD and
    // adjusted the live knob back to the downloaded value. The complete
    // snapshot and a fresh current-buffer read both agree with the old value.
    let playing = podxt::EditorBuffer {
        channel: source,
        tone: cached_source.clone(),
        packs: Packs(4),
    };
    panel.update_playing(playing.clone());
    panel.editor.accept_follow(playing);
    assert!(panel.editor.recall_ready());
    panel.startup = None;
    panel.live_editing = true;
    panel.editor.set_live(true);
    panel.editor.use_fake_recall_with_changed_source();
    let requests = panel.device_requests;
    click_text(&ctx, &mut app, "1B  Saved Channel");

    let deadline = Instant::now() + Duration::from_secs(2);
    while app.podxt().live.editor.busy() {
        assert!(
            Instant::now() < deadline,
            "fake channel recall did not finish"
        );
        std::thread::yield_now();
        render_stage(&ctx, &mut app, vec![]);
    }

    let panel = &app.podxt().live;
    assert_eq!(panel.device_requests, requests + 1);
    assert!(panel.editor.awaiting_recall_confirmation());
    assert_eq!(panel.playing_channel(), Some(source));
    assert_eq!(panel.snapshot.tone.as_ref(), Some(&cached_source));
    assert_eq!(panel.snapshot.library, downloaded);
    assert_eq!(panel.stored_tone(target).unwrap().name(), "Saved Channel");
    let visible = rendered(&ctx, &mut app, &["Switch to 1B and discard changes"]);
    assert!(!visible.contains("No channel switch was sent."));
    click_text(&ctx, &mut app, "Cancel switch");
    let panel = &app.podxt().live;
    assert!(!panel.editor.awaiting_recall_confirmation());
    assert!(!panel.editor.busy());
    assert_eq!(panel.playing_channel(), Some(source));
    assert_eq!(panel.snapshot.tone.as_ref(), Some(&cached_source));
    assert_eq!(panel.device_requests, requests + 1);
}

#[test]
fn offline_startup_imports_full_backup_and_browses_only_channels_in_stage() {
    let ctx = egui::Context::default();
    let directory = tempfile::tempdir().unwrap();
    let preferences = directory.path().join("preferences.json");
    let backup = complete_backup(directory.path(), "Backup Playing", "Backup Tone");
    let manifest = std::fs::read(backup.join("MANIFEST.sha256")).unwrap();
    let mut app = startup_app(&ctx, Some(preferences.clone()));
    app.new_tab(&super::super::super::FAMILY);
    app.podxt_mut().table_view_in_test = true;
    app.dialogs = Box::new(FakeDialogs {
        folder: [Some(backup.clone())].into(),
        pod_open: [Some(local_tone(directory.path()))].into(),
        ..Default::default()
    });

    // A complete backup on disk does not bypass explicit import.
    let visible = rendered(&ctx, &mut app, &["Default Tone", "offline", "Amp ·"]);
    assert!(!visible.contains("Backup Playing"), "{visible}");
    assert!(
        !visible.contains("Open a tone or create a new one"),
        "{visible}"
    );
    click_text(&ctx, &mut app, "File actions");
    click_text(&ctx, &mut app, "Use library backup…");
    rendered(
        &ctx,
        &mut app,
        &["Backup Playing", "Library backup", "1A  Backup Tone"],
    );
    assert_eq!(
        pod_preset::latest_podxt_backup(&preferences)
            .unwrap()
            .entries
            .len(),
        299
    );

    click_text(&ctx, &mut app, "1A  Backup Tone");
    rendered(&ctx, &mut app, &["Backup Tone", "Library backup · offline"]);
    assert!(!app.podxt().stage_is_current());
    click_text(&ctx, &mut app, "Open…");
    rendered(&ctx, &mut app, &["Local Document", "File editor"]);

    let visible = rendered(&ctx, &mut app, &["Tone channels", "1A  Backup Tone"]);
    for hidden in [
        "Amp Setups",
        "FX Setups",
        "Backup Amp",
        "Backup FX",
        "Playing ·",
        "Back to playing tone",
    ] {
        assert!(!visible.contains(hidden), "{visible}");
    }

    assert!(!app.podxt().busy());
    assert!(!app.podxt().dirty());
    assert!(app.podxt().live.snapshot.tone.is_none());
    assert_eq!(app.podxt().live.device_requests, 0);
    assert_eq!(
        std::fs::read(backup.join("MANIFEST.sha256")).unwrap(),
        manifest
    );
}

#[test]
fn follow_renders_tuner_and_fx_updates_without_replacing_an_edited_file_preview() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let playing_slot = Location::tone_from_label("5B").unwrap();
    let saved_fx = Location::new(BankKind::FxSetups, 11).unwrap();
    let neighbor_fx = Location::new(BankKind::FxSetups, 12).unwrap();
    let playing = valid_dump("Stage Current");
    let mut recalled = playing.clone();
    let delay_mix = Control::DelayMix.cc();
    recalled.raw[39 + usize::from(delay_mix)] = 82;

    let directory = tempfile::tempdir().unwrap();
    let path = local_tone(directory.path());
    let disk_bytes = fs::read(&path).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(path.clone())].into(),
        ..Default::default()
    });
    click_text(&ctx, &mut app, "Open…");
    let editor = app.podxt_mut();
    editor.pod_name = "Edited File".into();
    editor.apply_pod_name();
    let edited_bytes = editor.pod_library.tone(0).unwrap().as_bytes().to_vec();
    assert!(editor.dirty());
    assert_ne!(edited_bytes, disk_bytes);

    let panel = &mut editor.live;
    for (slot, name) in [(saved_fx, b"Old FX".as_slice()), (neighbor_fx, b"Other FX")] {
        panel.snapshot.apply(
            ReadUpdate::Stored(StoredTone {
                location: slot,
                tone: tone(name),
            }),
            Instant::now(),
        );
    }
    let library_before = panel.snapshot.library.clone();
    rendered(&ctx, &mut app, &["Edited File", "Unsaved", "Playing · 5B"]);
    click_text(&ctx, &mut app, "Back to playing tone");

    // Keep the worker present while each publication goes through the real
    // editor poll. The test drives its transaction synchronously for exact
    // assertions between notifications, without racing egui against a reader.
    let panel = &mut app.podxt_mut().live;
    let snapshot = Arc::new(Mutex::new(panel.snapshot.clone()));
    let stop = Arc::new(AtomicBool::new(false));
    let (finish, wait) = std::sync::mpsc::channel();
    panel.worker = Some(Worker {
        acquired: None,
        command: ReadCommand::Follow,
        device: panel.device.clone().unwrap(),
        stop: Arc::clone(&stop),
        snapshot: Arc::clone(&snapshot),
        thread: Some(std::thread::spawn(move || {
            wait.recv().map_err(|error| error.to_string())
        })),
    });
    let requests = panel.device_requests;

    let mut first_reply = playing.raw.clone();
    first_reply.extend([0xb4, 69, 127, 0xb4, 69, 0]);
    first_reply.extend([0xf0, 0, 1, 0x0c, 3, 0x10, 4, 19, 11, 0xf7]);
    first_reply.extend([0xf0, 0, 1, 0x0c, 3, 0x24, 3, 11, 0xf7]);
    let saved = valid_dump("New FX");
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let clock = Cell::new(Duration::ZERO);
    let mut notice_time = None;
    let mut publications = VecDeque::from(["initial", "tuner on", "tuner off", "recall", "save"]);
    let result = transaction(
        ReadCommand::Follow,
        &stop,
        &AtomicBool::new(false),
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
                    (PodxtRequest::EditBuffer, first_reply),
                    (PodxtRequest::EditBuffer, recalled.raw.clone()),
                    (
                        PodxtRequest::Stored(saved_fx),
                        [saved.raw.clone(), vec![0xf0, 0, 1, 0x0c, 3, 0x72, 0xf7]].concat(),
                    ),
                ]),
                reply: None,
                time: &clock,
                writes: &writes,
            })
        },
        |update| {
            let phase = publications
                .pop_front()
                .expect("unexpected Follow publication");
            snapshot.lock().unwrap().apply(update, Instant::now());
            match phase {
                "initial" => {
                    let visible = rendered(&ctx, &mut app, &["Stage Current", "Playing · 5B"]);
                    assert!(!visible.contains("Tuner active on POD"));
                }
                "tuner on" => {
                    rendered(&ctx, &mut app, &["Tuner active on POD", "Stage Current"]);
                    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&playing));
                }
                "tuner off" => {
                    // Recall and save notifications follow in this same MIDI packet.
                    notice_time = Some(clock.get());
                    let visible = rendered(&ctx, &mut app, &["Stage Current"]);
                    assert!(!visible.contains("Tuner active on POD"));
                    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&playing));
                    click_text(&ctx, &mut app, "File actions");
                    click_text(&ctx, &mut app, "Open file editor");
                }
                "recall" => {
                    rendered(&ctx, &mut app, &["Edited File", "Unsaved", "Playing · 5B"]);
                    assert!(!app.podxt().stage_is_current());
                    let panel = &app.podxt().live;
                    assert_eq!(panel.snapshot.tone.as_ref(), Some(&recalled));
                    assert_eq!(panel.snapshot.library, library_before);
                    assert_eq!(writes.get(), 3);
                }
                "save" => {
                    // The dump poll and the shared 100 ms marker window precede publication.
                    let request_time =
                        clock.get() - line6_devices::session::POLL - Duration::from_millis(100);
                    assert_eq!(
                        request_time - notice_time.unwrap(),
                        Duration::from_millis(2500)
                    );
                    let visible =
                        rendered(&ctx, &mut app, &["Edited File", "Unsaved", "Playing · 5B"]);
                    for hidden in ["FX Setups", "Old FX", "New FX", "Other FX"] {
                        assert!(!visible.contains(hidden), "{visible}");
                    }
                    let editor = app.podxt();
                    assert!(!editor.stage_is_current());
                    assert_eq!(editor.pod_library.tone(0).unwrap().as_bytes(), edited_bytes);
                    assert_eq!(editor.stage.path.as_ref(), Some(&path));
                    assert_eq!(editor.live.snapshot.tone.as_ref(), Some(&recalled));
                    let mut library_after = library_before.clone();
                    library_after.insert(saved_fx.address(), saved.clone());
                    assert_eq!(editor.live.snapshot.library, library_after);
                    stop.store(true, Ordering::Release);
                }
                _ => unreachable!(),
            }
            assert_eq!(app.podxt().live.playing_channel(), Some(playing_slot));
        },
    );
    assert_eq!(result, Err("PODxt read cancelled.".into()));
    assert!(publications.is_empty());
    assert_eq!(writes.get(), 4);
    assert!(!held.get());
    assert_eq!(fs::read(path).unwrap(), disk_bytes);

    click_text(&ctx, &mut app, "Back to playing tone");
    rendered(&ctx, &mut app, &["Stage Current", "Playing · 5B"]);
    let editor = app.podxt_mut();
    assert!(editor.stage_is_current());
    let (displayed, _) = editor.live.stage_tone().unwrap().unwrap();
    assert_eq!(
        displayed.to_podxt_payload().unwrap()[32 + usize::from(delay_mix)],
        82
    );
    assert!(editor.dirty());
    assert_eq!(editor.live.device_requests, requests);
    finish.send(()).unwrap();
    let mut worker = editor.live.worker.take().unwrap();
    worker.thread.take().unwrap().join().unwrap().unwrap();
}

#[test]
fn connection_sync_adopts_only_a_complete_released_result() {
    let ctx = egui::Context::default();
    let (mut app, _) = current_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    let slot = Location::all(&[BankKind::Tones]).next().unwrap();

    for (count, result, stopped, expected) in [
        (19, Ok(()), false, "ended after 19 entries"),
        (
            128,
            Err("USB release failed".into()),
            false,
            "USB release failed",
        ),
        (128, Ok(()), true, "read cancelled"),
        (128, Ok(()), false, ""),
    ] {
        let mut snapshot = Snapshot::default();
        snapshot.apply(ReadUpdate::LibraryStarted, Instant::now());
        for location in Location::all(&[BankKind::Tones]).take(count) {
            snapshot
                .library
                .insert(location.address(), valid_dump("New snapshot"));
        }
        snapshot.library_count = Some(count);
        let thread = std::thread::spawn(move || result);
        while !thread.is_finished() {
            std::thread::yield_now();
        }
        panel.worker = Some(Worker {
            command: ReadCommand::ToneBank,
            device: panel.device.clone().unwrap(),
            stop: Arc::new(AtomicBool::new(stopped)),
            acquired: Some(Arc::new(AtomicBool::new(true))),
            snapshot: Arc::new(Mutex::new(snapshot)),
            thread: Some(thread),
        });
        panel.poll(&ctx);
        match panel.connection_sync_update().unwrap() {
            SyncUpdate::Finished(Ok(())) => {
                assert!(expected.is_empty());
                assert_eq!(panel.stored_tone(slot).unwrap().name(), "New snapshot");
            }
            SyncUpdate::Finished(Err(error)) => {
                assert!(!expected.is_empty() && error.contains(expected), "{error}");
                assert_eq!(panel.stored_tone(slot).unwrap().name(), "Saved Channel");
            }
            _ => panic!("worker should have finalized"),
        }
        assert_eq!(panel.snapshot.library.len(), 128);
        assert!(panel.connection_sync_update().is_none());
    }
}

#[test]
fn same_verified_attachment_recovers_current_without_downloading_the_completed_bank() {
    let ctx = egui::Context::default();
    let (mut app, _) = current_fixture(&ctx);
    let mut panel = std::mem::take(&mut app.podxt_mut().live);
    let device = app.tabs.entries[0].device.as_ref();
    let saved = panel.snapshot.library.clone();

    assert!(panel.has_session_library());
    assert!(!panel.set_device(None));
    assert!(panel.has_session_library());
    assert!(panel.snapshot.library.is_empty());
    assert_eq!(panel.historical_library, saved);
    assert!(panel.set_device(device));
    assert!(panel.has_session_library());
    assert_eq!(panel.snapshot.library, saved);
    assert!(panel.historical_library.is_empty());
    assert!(!panel.has_current());
    assert!(matches!(
        panel.connection_sync_admission(),
        SyncAdmission::Busy
    ));

    panel.editor.use_fake_get();
    let requests = panel.device_requests;
    panel.poll(&ctx);
    let deadline = Instant::now() + Duration::from_secs(5);
    while panel.editor.busy() {
        assert!(Instant::now() < deadline, "current read did not finish");
        std::thread::yield_now();
        panel.poll(&ctx);
    }
    assert_eq!(panel.device_requests, requests + 1);
    assert!(panel.has_current());
    assert!(panel.live_editing);
    assert!(matches!(
        panel.connection_sync_admission(),
        SyncAdmission::Complete
    ));
    assert_eq!(panel.next_startup(), None);
    assert_eq!(panel.snapshot.library, saved);
}

#[test]
fn home_reopen_checks_the_current_tone_before_checked_recall() {
    let ctx = egui::Context::default();
    let (mut app, _) = current_fixture(&ctx);
    app.connections.dismiss();
    let tab = app.tabs.active.unwrap();
    let old_key = app.podxt().live.device.clone().unwrap();
    let requests = app.podxt().live.device_requests;
    app.podxt_mut().live.editor.use_fake_get();
    app.podxt_mut()
        .live
        .editor
        .use_fake_recall_after_current_read();

    render_stage(&ctx, &mut app, vec![]);
    let close = ctx.data(|data| {
        data.get_temp::<egui::Rect>(egui::Id::new(("test-close-tab", tab)))
            .unwrap()
            .center()
    });
    for pressed in [true, false] {
        render_stage(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(close),
                egui::Event::PointerButton {
                    pos: close,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(app.tabs.entries.is_empty());
    click_text(&ctx, &mut app, "Open PODxt Live");
    assert_eq!(app.tabs.active, Some(tab));
    render_stage(&ctx, &mut app, vec![]);
    finish_playing_read(&ctx, &mut app, requests + 1);
    let panel = &app.podxt().live;
    assert!(panel.live_editing);
    assert_eq!(panel.device.as_ref(), Some(&old_key));
    assert_eq!(panel.snapshot.library_count, Some(128));
    assert!(matches!(
        panel.connection_sync_admission(),
        SyncAdmission::Complete
    ));

    click_text(&ctx, &mut app, "1B  Saved Channel");
    let deadline = Instant::now() + Duration::from_secs(2);
    while app.podxt().live.editor.busy() {
        assert!(Instant::now() < deadline, "fake recall did not finish");
        std::thread::yield_now();
        render_stage(&ctx, &mut app, vec![]);
    }
    let panel = &app.podxt().live;
    assert_eq!(panel.device_requests, requests + 2);
    assert_eq!(panel.playing_channel(), Location::tone_from_label("1B"));
    assert_eq!(
        panel.snapshot.tone.as_ref().unwrap().name(),
        "Recalled tone"
    );
    assert_eq!(panel.snapshot.library_count, Some(128));
}

#[test]
fn same_port_replacement_keeps_failed_and_cancelled_downloads_as_historical() {
    let ctx = egui::Context::default();
    let (mut app, presence) = current_fixture(&ctx);
    let tab = app.tabs.active.unwrap();
    let old = app.podxt().live.device.clone().unwrap();
    let slot = Location::new(BankKind::Tones, 0).unwrap();
    presence.attached.set(false);
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    presence.attached.set(true);
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    let replacement = app.podxt().live.device.clone().unwrap();
    assert_eq!(app.tabs.active, Some(tab));
    assert_eq!(replacement.attachment.path, old.attachment.path);
    assert_ne!(replacement, old);
    assert!(app.podxt().live.has_session_library());
    assert!(!matches!(
        app.podxt().live.connection_sync_admission(),
        SyncAdmission::Complete
    ));

    assert_ne!(app.podxt().live.startup, Some(Startup::Tones));

    for (result, cancelled) in [(Err("failed read".into()), false), (Ok(()), true)] {
        let panel = &mut app.podxt_mut().live;
        // Even a complete captured buffer needs a successful worker and release.
        let worker = std::thread::spawn(move || result);
        while !worker.is_finished() {
            std::thread::yield_now();
        }
        let mut captured = Snapshot::default();
        captured.apply(ReadUpdate::LibraryStarted, Instant::now());
        for location in Location::all(&[BankKind::Tones]) {
            captured.apply(
                ReadUpdate::Stored(StoredTone {
                    location,
                    tone: valid_dump("Replacement"),
                }),
                Instant::now(),
            );
        }
        panel.worker = Some(Worker {
            command: ReadCommand::ToneBank,
            device: replacement.clone(),
            acquired: Some(Arc::new(AtomicBool::new(true))),
            stop: Arc::new(AtomicBool::new(cancelled)),
            snapshot: Arc::new(Mutex::new(captured)),
            thread: Some(worker),
        });
        panel.snapshot.channel = Some(slot);
        panel.snapshot.tone = Some(valid_dump("Replacement current"));
        panel.live_editing = true;
        let visible = stage_frame(&ctx, &mut app);
        assert!(
            visible.contains("Previous PODxt library · offline"),
            "{visible}"
        );
        assert!(!visible.contains("On PODxt Live"), "{visible}");
        assert!(!app.podxt().live.has_library_bank(BankKind::Tones));
        assert!(app.podxt().live.stored_tone(slot).is_none());
        assert!(!app.podxt().live.channel_modified(slot));
        assert_eq!(
            app.podxt().live.historical_tone(slot).unwrap().name(),
            "Saved Channel"
        );
    }

    let requests = app.podxt().live.device_requests;
    click_text(&ctx, &mut app, "1A  Saved Channel");
    assert_eq!(app.podxt().live.device_requests, requests);
    assert!(!app.podxt().stage_is_current());
    let preview = stage_frame(&ctx, &mut app);
    assert!(
        preview.contains("Previous PODxt library copy · file only"),
        "{preview}"
    );
}

type RecallSteps = Arc<Mutex<VecDeque<super::super::edit::tests::ScriptedAction>>>;

fn recall_channel(slot: Location) -> (PodxtRequest, Vec<u8>) {
    let index = slot.index() as u8;
    (
        PodxtRequest::QueryGlobal(podxt::Global::CurrentChannel),
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

fn recall_source(slot: Location, tone: &EditBuffer) -> Vec<(PodxtRequest, Vec<u8>)> {
    vec![
        (
            PodxtRequest::Identity,
            variax_workbench::identity_stage::bridge_reply(),
        ),
        recall_channel(slot),
        (PodxtRequest::EditBuffer, tone.raw.clone()),
    ]
}

fn recall_packs() -> (PodxtRequest, Vec<u8>) {
    (
        PodxtRequest::InstalledPacks,
        vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7],
    )
}

fn expect_action(
    steps: &RecallSteps,
    target: Option<Location>,
    requests: Vec<(PodxtRequest, Vec<u8>)>,
    release: impl FnOnce() -> Result<(), String> + Send + 'static,
) {
    steps.lock().unwrap().push_back(ScriptedAction {
        kind: target.map_or(ScriptedKind::Get, ScriptedKind::Recall),
        requests: requests.into(),
        release: Box::new(release),
    });
}

fn finish_action(ctx: &egui::Context, app: &mut Workbench, steps: &RecallSteps) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.podxt().live.editor.busy() {
        assert!(
            Instant::now() < deadline,
            "scripted editor action did not finish"
        );
        std::thread::yield_now();
        render_stage(ctx, app, vec![]);
    }
    assert!(steps.lock().unwrap().is_empty());
}

fn recall_pair() -> (Location, Location) {
    (
        Location::tone_from_label("2A").unwrap(),
        Location::tone_from_label("2B").unwrap(),
    )
}

fn recall_fixture(
    ctx: &egui::Context,
    playing: &EditBuffer,
    cached: &EditBuffer,
) -> (Workbench, Presence, RecallSteps, EditBuffer) {
    let (mut app, presence) = connected_fixture(ctx);
    let (a, b) = recall_pair();
    let saved_b = valid_dump("Saved 2B");
    let panel = &mut app.podxt_mut().live;
    panel.snapshot = Snapshot::default();
    let steps = RecallSteps::default();
    panel.editor.use_scripted_actions(Arc::clone(&steps));
    panel.startup = Some(Startup::Current);
    panel.resume_live = true;
    let mut requests = recall_source(a, playing);
    requests.extend([recall_packs(), recall_channel(a)]);
    expect_action(&steps, None, requests, || Ok(()));
    render_stage(ctx, &mut app, vec![]);
    finish_action(ctx, &mut app, &steps);
    assert!(app.podxt().live.live_editing);
    assert_eq!(app.podxt().live.playing_channel(), Some(a));
    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(playing));
    assert!(app.podxt().live.snapshot.library.is_empty());
    assert!(app.podxt().live.channel_draft(a).is_none());

    // Take the startup Current -> ToneBank handoff using the real read
    // transaction and the managed worker completion path.
    let panel = &mut app.podxt_mut().live;
    assert_eq!(panel.next_startup(), Some(Startup::Tones));
    let mut captured = panel.snapshot.clone();
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let default = valid_dump("Saved Channel");
    let script = std::iter::once((
        PodxtRequest::Identity,
        variax_workbench::identity_stage::bridge_reply(),
    ))
    .chain(Location::all(&[BankKind::Tones]).map(|slot| {
        let tone = if slot == a {
            cached
        } else if slot == b {
            &saved_b
        } else {
            &default
        };
        (PodxtRequest::Stored(slot), tone.raw.clone())
    }))
    .collect();
    let result = transaction(
        ReadCommand::ToneBank,
        &AtomicBool::new(false),
        &panel.editor.reply_pending,
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
        |update| captured.apply(update, Instant::now()),
    );
    assert_eq!(writes.get(), 129);
    assert!(!held.get());
    let thread = std::thread::spawn(move || result);
    while !thread.is_finished() {
        std::thread::yield_now();
    }
    panel.worker = Some(Worker {
        command: ReadCommand::ToneBank,
        device: panel.device.clone().unwrap(),
        stop: Arc::default(),
        acquired: Some(Arc::new(AtomicBool::new(true))),
        snapshot: Arc::new(Mutex::new(captured)),
        thread: Some(thread),
    });
    panel.poll(ctx);
    assert!(matches!(
        panel.connection_sync_update(),
        Some(SyncUpdate::Finished(Ok(())))
    ));
    assert_eq!(panel.snapshot.library_count, Some(128));
    assert_eq!(panel.snapshot.tone.as_ref(), Some(playing));
    assert_eq!(panel.stored_tone(a), Some(cached));
    (app, presence, steps, saved_b)
}

struct EditorLog {
    _folder: tempfile::TempDir,
    guard: SessionGuard,
    sink: SessionLog,
}

impl EditorLog {
    fn attach(app: &mut Workbench) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let (sink, guard) = SessionLog::open(Some(folder.path().join("logs")), Severity::Trace);
        app.podxt_mut().live.editor.use_session_log(sink.clone());
        Self {
            _folder: folder,
            guard,
            sink,
        }
    }

    fn read(self, private_values: &[&str]) -> Vec<LogRow> {
        drop(self.guard);
        let log = std::fs::read_to_string(self.sink.status().path.unwrap()).unwrap();
        for value in private_values {
            assert!(!log.contains(value), "private value recorded");
        }
        log.lines()
            .map(|line| serde_json::from_str::<LogRow>(line).unwrap())
            .filter(|row| row["endpoint"] == "podxt")
            .collect()
    }
}

fn position(rows: &[LogRow], predicate: impl Fn(&LogRow) -> bool) -> usize {
    rows.iter().position(predicate).expect("expected log fact")
}

fn operation_for(rows: &[LogRow], target: Location) -> Vec<LogRow> {
    let click = position(rows, |row| {
        row["reason"] == "requested" && row["target"] == target.index()
    });
    let id = rows[click]["operation"].as_u64().unwrap();
    let operation: Vec<_> = rows
        .iter()
        .filter(|row| row["operation"] == id)
        .cloned()
        .collect();
    assert_eq!(operation[0]["control"], "podxt_channel");
    assert_eq!(operation[0]["action"]["kind"], "button");
    operation
}

fn program_sent(row: &LogRow) -> bool {
    row["observation"]["stage"] == "Send"
        && row["observation"]["request"] == "PodxtChannelSwitch"
        && row["observation"]["outcome"] == "Sent"
}

fn assert_recall_checked(rows: &[LogRow], source: Location) {
    let sent = position(rows, program_sent);
    let readback = position(rows, |row| {
        row["reason"] == "verified" && row["evidence"] == "verified_playing"
    });
    let finished = position(rows, |row| {
        row["category"] == "operation_finished" && row["success"] == true
    });
    assert_eq!(rows[readback]["source"], source.index());
    assert!(sent < readback && readback < finished);
    assert!(
        rows[readback + 1..finished]
            .iter()
            .any(|row| { row["category"] == "checked_release" && row["success"] == true })
    );
}

fn recall_destination(
    requests: &mut Vec<(PodxtRequest, Vec<u8>)>,
    source: Location,
    playing: &EditBuffer,
    target: Location,
    saved: &EditBuffer,
    readback: &EditBuffer,
) {
    requests.extend([
        (PodxtRequest::Stored(target), saved.raw.clone()),
        recall_packs(),
        (PodxtRequest::EditBuffer, playing.raw.clone()),
        recall_channel(source),
        (PodxtRequest::Program(target), vec![]),
        (PodxtRequest::EditBuffer, readback.raw.clone()),
        recall_channel(target),
    ]);
}

#[test]
fn connected_sidebar_recall_2a_2b_2a_publishes_only_after_checked_release() {
    let ctx = egui::Context::default();
    let (a, b) = recall_pair();
    let saved_a = valid_dump("Saved 2A");
    let mut playing_a = saved_a.clone();
    playing_a.raw[39 + 7] = 91;
    let (mut app, _, steps, saved_b) = recall_fixture(&ctx, &playing_a, &saved_a);
    let log = EditorLog::attach(&mut app);
    assert!(!app.podxt().live.channel_modified(a));
    let library = app.podxt().live.snapshot.library.clone();

    for (source, playing, saved, target, after, row) in [
        (a, &playing_a, &saved_a, b, &saved_b, "2B  Saved 2B"),
        (b, &saved_b, &saved_b, a, &saved_a, "2A  Saved 2A"),
    ] {
        let mut requests = recall_source(source, playing);
        requests.push((PodxtRequest::Stored(source), saved.raw.clone()));
        recall_destination(&mut requests, source, playing, target, after, after);
        let (read_back, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        expect_action(&steps, Some(target), requests, move || {
            read_back.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(())
        });

        click_text(&ctx, &mut app, row);
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        rendered(&ctx, &mut app, &[&format!("Playing · {}", source.label())]);
        assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(playing));
        release.send(()).unwrap();
        finish_action(&ctx, &mut app, &steps);
        let visible = rendered(&ctx, &mut app, &[&format!("Playing · {}", target.label())]);
        assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(after));
        assert!(!visible.contains("Read playing tone"));
        assert!(!visible.contains("Connected · firmware"));
        assert!(!visible.contains("Live · edit buffer"));
    }
    assert_eq!(app.podxt().live.snapshot.library, library);
    let rows = log.read(&["Saved 2A", "Saved 2B"]);
    for (source, target) in [(a, b), (b, a)] {
        let rows = operation_for(&rows, target);
        let comparison = position(&rows, |row| {
            row["reason"] == "verified" && row["evidence"] == "addressed_slot"
        });
        assert_eq!(rows[comparison]["source"], source.index());
        assert!(comparison < position(&rows, program_sent));
        if source == a {
            assert!(rows.iter().any(|row| {
                row["reason"] == "volatile_difference"
                    && row["difference"]
                        == serde_json::json!({
                            "kind": "cc", "number": 7, "playing": 91,
                            "saved": saved_a.cc(7), "class": "volatile"
                        })
            }));
        }
        assert_recall_checked(&rows, source);
    }
}

#[test]
fn connected_sidebar_consent_survives_volatile_motion_and_second_destination() {
    let ctx = egui::Context::default();
    let (a, b) = recall_pair();
    let mut saved_a = valid_dump("Saved 2A");
    let mut edited = saved_a.clone();
    edited.raw[39 + usize::from(Control::Drive.cc())] = 42;
    let (mut app, _, steps, saved_b) = recall_fixture(&ctx, &edited, &saved_a);
    let log = EditorLog::attach(&mut app);
    assert!(app.podxt().live.channel_modified(a));
    // Unknown bytes cannot be assumed to be a known editable parameter.
    saved_a.raw[23] ^= 1;
    saved_a.raw[7] = b'X';
    let mut requests = recall_source(a, &edited);
    requests.extend([
        (PodxtRequest::Stored(a), saved_a.raw.clone()),
        recall_packs(),
        recall_channel(a),
    ]);
    expect_action(&steps, Some(b), requests, || Ok(()));
    click_text(&ctx, &mut app, "2B  Saved 2B");
    finish_action(&ctx, &mut app, &steps);

    let affirmative = "Switch to 2B and discard changes";
    click_text(&ctx, &mut app, "Playing tone comparison");
    let text = rendered(
        &ctx,
        &mut app,
        &[affirmative, "Addressed saved slot", "drive, CC 13"],
    );
    assert!(!text.contains("No channel switch was sent."));
    click_text(&ctx, &mut app, "1B  Saved Channel");
    rendered(&ctx, &mut app, &[affirmative, "Cancel the pending switch"]);
    assert!(steps.lock().unwrap().is_empty());

    let mut moved = edited;
    moved.raw[39 + 7] = 99;
    let update = podxt::EditorBuffer {
        channel: a,
        tone: moved.clone(),
        packs: Packs(4),
    };
    let panel = &mut app.podxt_mut().live;
    panel.editor.accept_follow(update.clone());
    panel.update_playing(update);
    rendered(&ctx, &mut app, &[affirmative]);

    let mut requests = recall_source(a, &moved);
    recall_destination(&mut requests, a, &moved, b, &saved_b, &saved_b);
    expect_action(&steps, Some(b), requests, || Ok(()));
    click_text(&ctx, &mut app, affirmative);
    finish_action(&ctx, &mut app, &steps);
    rendered(&ctx, &mut app, &["Playing · 2B"]);
    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&saved_b));
    let rows = operation_for(&log.read(&["Saved 2A", "Xaved 2A", "Saved 2B"]), b);
    let name_difference = position(&rows, |row| row["reason"] == "name_difference");
    assert_eq!(rows[name_difference]["source"], a.index());
    assert_eq!(rows[name_difference]["evidence"], "addressed_slot");
    assert!(rows[name_difference]["difference"].is_null());
    let asked = position(&rows, |row| row["reason"] == "confirmation_required");
    let approved = position(&rows, |row| row["reason"] == "confirmed");
    assert!(asked < approved && approved < position(&rows, program_sent));
    assert!(rows.iter().any(|row| {
        row["source"] == a.index()
            && row["evidence"] == "addressed_slot"
            && row["difference"]
                == serde_json::json!({
                    "kind": "cc", "number": 13, "playing": 42, "saved": saved_a.cc(13),
                    "class": "named"
                })
    }));
    assert_recall_checked(&rows, a);
}

#[test]
fn connected_sidebar_distinguishes_cached_and_opaque_comparisons_and_cancel_sends_zero() {
    let (a, b) = recall_pair();
    for stale in [false, true] {
        let ctx = egui::Context::default();
        let playing = valid_dump("Saved 2A");
        let mut cached = playing.clone();
        let mut addressed = playing.clone();
        // A cached CC difference has no bearing on the checked protection.
        if stale {
            cached.raw[39 + usize::from(Control::Drive.cc())] = 25;
        }
        addressed.raw[23] ^= 1;
        addressed.raw[38] ^= 2;
        let (mut app, _, steps, _) = recall_fixture(&ctx, &playing, &cached);
        let log = EditorLog::attach(&mut app);
        assert_eq!(app.podxt().live.channel_modified(a), stale);
        if stale {
            let details = app.podxt().live.channel_comparison_text(a).unwrap();
            assert!(details.contains("Cached saved slot"));
            assert!(details.contains("drive, CC 13"));
        }
        let mut requests = recall_source(a, &playing);
        requests.extend([
            (PodxtRequest::Stored(a), addressed.raw),
            recall_packs(),
            recall_channel(a),
        ]);
        expect_action(&steps, Some(b), requests, || Ok(()));
        click_text(&ctx, &mut app, "2B  Saved 2B");
        finish_action(&ctx, &mut app, &steps);
        click_text(&ctx, &mut app, "Playing tone comparison");
        rendered(
            &ctx,
            &mut app,
            &[
                "Addressed saved slot",
                "Opaque dump offsets (zero-based) 23, 38",
            ],
        );
        let before = app.podxt().live.snapshot.library.clone();
        click_text(&ctx, &mut app, "Cancel switch");
        rendered(&ctx, &mut app, &["Playing · 2A"]);
        assert!(!app.podxt().live.editor.awaiting_recall_confirmation());
        assert!(!app.podxt().live.editor.busy());
        assert_eq!(app.podxt().live.snapshot.library, before);
        assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&playing));
        let rows = operation_for(&log.read(&["Saved 2A", "Saved 2B"]), b);
        assert_eq!(
            rows.iter()
                .filter(|row| row["reason"] == "opaque_difference")
                .count(),
            2
        );
        for (offset, changed_bits) in [(23, 1), (38, 2)] {
            assert!(rows.iter().any(|row| {
                row["source"] == a.index()
                    && row["evidence"] == "addressed_slot"
                    && row["difference"]
                        == serde_json::json!({
                            "kind": "opaque_bits", "offset": offset, "changed_bits": changed_bits
                        })
            }));
        }
        assert!(rows.iter().all(|row| row["reason"] != "control_difference"));
        let cancelled = position(&rows, |row| row["reason"] == "cancelled");
        assert_eq!(rows[cancelled - 1]["disposition"], "cancelled");
        assert!(!rows.iter().any(program_sent));
    }
}

#[test]
fn connected_sidebar_keeps_old_highlight_after_uncertain_readback_or_release() {
    let (a, b) = recall_pair();
    for release_failed in [false, true] {
        let ctx = egui::Context::default();
        let saved_a = valid_dump("Saved 2A");
        let (mut app, _, steps, saved_b) = recall_fixture(&ctx, &saved_a, &saved_a);
        let log = EditorLog::attach(&mut app);
        let readback = if release_failed { &saved_b } else { &saved_a };
        let mut requests = recall_source(a, &saved_a);
        requests.push((PodxtRequest::Stored(a), saved_a.raw.clone()));
        recall_destination(&mut requests, a, &saved_a, b, &saved_b, readback);
        expect_action(&steps, Some(b), requests, move || {
            if release_failed {
                Err("USB release failed.".into())
            } else {
                Ok(())
            }
        });

        click_text(&ctx, &mut app, "2B  Saved 2B");
        finish_action(&ctx, &mut app, &steps);
        rendered(
            &ctx,
            &mut app,
            &[
                "Playing · 2A",
                "Read playing tone",
                if release_failed {
                    "USB release failed"
                } else {
                    "The channel switch could not be verified"
                },
            ],
        );
        assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&saved_a));
        assert!(app.podxt().live.editor.needs_refresh());
        let rows = operation_for(&log.read(&["USB release failed.", "Saved 2A"]), b);
        let sent = position(&rows, program_sent);
        let readback = position(&rows, |row| row["evidence"] == "verified_playing");
        let release = position(&rows, |row| row["category"] == "checked_release");
        let finish = position(&rows, |row| row["category"] == "operation_finished");
        assert_eq!(
            rows[readback]["reason"],
            if release_failed { "verified" } else { "failed" }
        );
        assert_eq!(rows[release]["success"], !release_failed);
        assert_eq!(rows[finish]["success"], false);
        assert!(sent < readback && readback < release && release < finish);
    }
}

#[test]
fn connected_sidebar_follows_latest_physical_notice_across_recall_release() {
    let ctx = egui::Context::default();
    let (a, b) = recall_pair();
    let c = Location::tone_from_label("2C").unwrap();
    let d = Location::tone_from_label("2D").unwrap();
    let saved_a = valid_dump("Saved 2A");
    let (mut app, presence, steps, saved_b) = recall_fixture(&ctx, &saved_a, &saved_a);
    let mut requests = recall_source(a, &saved_a);
    requests.push((PodxtRequest::Stored(a), saved_a.raw.clone()));
    recall_destination(&mut requests, a, &saved_a, b, &saved_b, &saved_b);
    let (read_back, ready) = std::sync::mpsc::channel();
    let (release, wait) = std::sync::mpsc::channel();
    expect_action(&steps, Some(b), requests, move || {
        read_back.send(()).unwrap();
        wait.recv_timeout(Duration::from_secs(5)).unwrap();
        Ok(())
    });
    click_text(&ctx, &mut app, "2B  Saved 2B");
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let now = Instant::now();
    app.podxt_mut().live.live_since = Some(now - Duration::from_secs(1));
    for (index, slot) in [c, d].into_iter().enumerate() {
        let observed_at = now - Duration::from_millis(200 - index as u64);
        presence
            .notices
            .borrow_mut()
            .push(detection::SimulatedNotice::PodxtProgram(
                detection::SimulatedPodxtProgram {
                    channel: slot.index() as u8,
                    observed_at,
                },
            ));
        app.next_detection = Instant::now();
        render_stage(&ctx, &mut app, vec![]);
    }
    assert_eq!(app.podxt().live.playing_channel(), Some(a));

    let latest = valid_dump("Physical 2D");
    let mut requests = recall_source(d, &latest);
    requests.extend([recall_packs(), recall_channel(d)]);
    let (follow_read, follow_ready) = std::sync::mpsc::channel();
    let (follow_release, follow_wait) = std::sync::mpsc::channel();
    expect_action(&steps, None, requests, move || {
        follow_read.send(()).unwrap();
        follow_wait.recv_timeout(Duration::from_secs(5)).unwrap();
        Ok(())
    });
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while follow_ready.try_recv().is_err() {
        assert!(
            Instant::now() < deadline,
            "latest checked read did not finish"
        );
        assert_eq!(app.podxt().live.playing_channel(), Some(a));
        std::thread::yield_now();
        render_stage(&ctx, &mut app, vec![]);
    }
    assert_eq!(app.podxt().live.playing_channel(), Some(a));
    follow_release.send(()).unwrap();
    finish_action(&ctx, &mut app, &steps);
    rendered(&ctx, &mut app, &["Playing · 2D", "Physical 2D"]);
    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&latest));
}

#[test]
fn fresh_startup_can_show_a_hardware_buffer_difference_without_any_app_edit() {
    let ctx = egui::Context::default();
    let (a, b) = recall_pair();
    let saved = valid_dump("Bridge of Sighs");
    let mut playing = saved.clone();
    playing.raw[39 + usize::from(Control::Drive.cc())] = 42;
    let (mut app, _, steps, saved_b) = recall_fixture(&ctx, &playing, &saved);
    rendered(&ctx, &mut app, &["Playing · 2A", "2A !"]);
    assert!(!app.podxt().dirty());
    assert!(app.podxt().live.channel_draft(a).is_none());
    assert!(!app.podxt().live.has_staged_edits());

    // Now simulate the slot being saved on the hardware outside the cached
    // bank. The fresh addressed read matches the current tone, so the cache
    // marker cannot block recall or claim a verified unretained edit.
    let mut requests = recall_source(a, &playing);
    requests.push((PodxtRequest::Stored(a), playing.raw.clone()));
    recall_destination(&mut requests, a, &playing, b, &saved_b, &saved_b);
    expect_action(&steps, Some(b), requests, || Ok(()));
    click_text(&ctx, &mut app, "2B  Saved 2B");
    finish_action(&ctx, &mut app, &steps);
    rendered(&ctx, &mut app, &["Playing · 2B"]);
    assert!(!app.podxt().live.editor.awaiting_recall_confirmation());
    assert_eq!(app.podxt().live.stored_tone(a), Some(&saved));
}

#[test]
fn one_sidebar_click_retains_exact_startup_device_edit_before_switching() {
    let (a, b) = recall_pair();
    for drift in [false, true] {
        let ctx = egui::Context::default();
        let saved = valid_dump("Bridge of Sighs");
        let mut playing = saved.clone();
        playing.raw[39 + usize::from(Control::Drive.cc())] = 42;
        let (mut app, _, steps, saved_b) = recall_fixture(&ctx, &playing, &saved);
        let log = EditorLog::attach(&mut app);
        rendered(&ctx, &mut app, &["Playing · 2A", "2A !"]);
        assert!(!app.podxt().dirty());

        let mut requests = recall_source(a, &playing);
        requests.extend([
            (PodxtRequest::Stored(a), saved.raw.clone()),
            recall_packs(),
            recall_channel(a),
        ]);
        expect_action(&steps, Some(b), requests, || Ok(()));

        let mut latest = playing.clone();
        if drift {
            latest.raw[39 + 7] = 111;
        }
        let mut requests = recall_source(a, &latest);
        if drift {
            // Exact retention is stricter than the comparison used to
            // describe a playing tone. This continuation must not switch.
            requests.extend([recall_packs(), recall_channel(a)]);
        } else {
            requests.push((PodxtRequest::Stored(a), saved.raw.clone()));
            recall_destination(&mut requests, a, &latest, b, &saved_b, &saved_b);
        }
        let (read_back, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        expect_action(&steps, Some(b), requests, move || {
            read_back.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(())
        });

        click_text(&ctx, &mut app, "2B  Saved 2B");
        let deadline = Instant::now() + Duration::from_secs(5);
        while ready.try_recv().is_err() {
            assert!(Instant::now() < deadline, "retained recall did not run");
            assert_eq!(app.podxt().live.playing_channel(), Some(a));
            std::thread::yield_now();
            render_stage(&ctx, &mut app, vec![]);
        }
        let live = &app.podxt().live;
        assert_eq!(live.playing_channel(), Some(a));
        assert_eq!(
            live.channel_draft(a),
            Some(&podxt::TonePayload::from_dump(&playing).unwrap())
        );
        assert!(live.channel_draft_is_imported(a));
        assert_eq!(live.stored_tone(a), Some(&saved));
        release.send(()).unwrap();
        finish_action(&ctx, &mut app, &steps);

        let live = &app.podxt().live;
        assert_eq!(
            live.channel_draft(a),
            Some(&podxt::TonePayload::from_dump(&playing).unwrap())
        );
        assert_eq!(live.stored_tone(a), Some(&saved));
        assert_eq!(live.stored_tone(b), Some(&saved_b));
        assert!(!live.editor.awaiting_recall_confirmation());
        assert_eq!(live.playing_channel(), Some(if drift { a } else { b }));
        if drift {
            rendered(
                &ctx,
                &mut app,
                &["The playing tone changed. Select the channel again."],
            );
        } else {
            rendered(&ctx, &mut app, &["Playing · 2B", "2A · Draft"]);
        }
        let rows = operation_for(&log.read(&["Bridge of Sighs", "Saved 2B"]), b);
        let release = position(&rows, |row| row["category"] == "checked_release");
        let retained = position(&rows, |row| row["reason"] == "retained_draft");
        assert_eq!(rows[release]["success"], true);
        assert_eq!(rows[retained]["source"], a.index());
        assert_eq!(rows[retained]["evidence"], "checked_release");
        assert_eq!(rows[retained]["draft_origin"], "device_buffer");
        assert!(release < retained);
        assert!(!rows[..retained].iter().any(program_sent));
        if drift {
            assert!(
                rows[retained + 1..]
                    .iter()
                    .any(|row| row["reason"] == "expired")
            );
            assert!(!rows.iter().any(program_sent));
        } else {
            assert_recall_checked(&rows, a);
        }
        assert_eq!(
            rows.iter()
                .filter(|row| row["category"] == "checked_release")
                .count(),
            2
        );
    }
}

#[test]
fn normal_live_drive_drag_and_amp_switch_log_one_coalesced_checked_batch() {
    let ctx = egui::Context::default();
    let (slot, _) = recall_pair();
    let mut before = valid_dump("Private knob tone");
    before.raw[39 + usize::from(Control::Drive.cc())] = 20;
    before.raw[39 + usize::from(Control::AmpEnable.cc())] = Control::AmpEnable.switch_value(true);
    let (mut app, _, steps, _) = recall_fixture(&ctx, &before, &before);
    let log = EditorLog::attach(&mut app);
    rendered(&ctx, &mut app, &["Drive"]);
    let start = app.podxt().pod_buttons["stage-drive"].center();
    let amp = app.podxt().pod_buttons["pod-block-amp"].left_top() + egui::vec2(11.0, 15.0);

    // Hold the existing rate limiter while egui sends both drag changes and
    // the switch click. The production queue still decides what is dispatched.
    app.podxt_mut()
        .live
        .editor
        .set_test_last_dispatch(Some(Instant::now() + Duration::from_secs(5)));
    render_stage(
        &ctx,
        &mut app,
        vec![egui::Event::PointerMoved(start), pointer(start, true)],
    );
    let first = start - egui::vec2(0.0, 18.0);
    let last = start - egui::vec2(0.0, 36.0);
    for pos in [first, last] {
        render_stage(&ctx, &mut app, vec![egui::Event::PointerMoved(pos)]);
    }
    render_stage(&ctx, &mut app, vec![pointer(last, false)]);
    click_position(&ctx, &mut app, amp);
    assert!(app.podxt().live.editor.has_pending());

    let value = 58;
    let disabled = Control::AmpEnable.switch_value(false);
    let mut after = before.clone();
    after.raw[39 + usize::from(Control::Drive.cc())] = value;
    after.raw[39 + usize::from(Control::AmpEnable.cc())] = disabled;
    let mut requests = recall_source(slot, &before);
    requests.extend([
        recall_channel(slot),
        (
            PodxtRequest::Set(Control::Drive, podxt::Value::new(value).unwrap()),
            vec![],
        ),
        (
            PodxtRequest::Set(Control::AmpEnable, podxt::Value::new(disabled).unwrap()),
            vec![],
        ),
        (PodxtRequest::EditBuffer, after.raw.clone()),
        recall_channel(slot),
    ]);
    steps.lock().unwrap().push_back(ScriptedAction {
        kind: ScriptedKind::Controls,
        requests: requests.into(),
        release: Box::new(|| Ok(())),
    });
    app.podxt_mut().live.editor.set_test_last_dispatch(None);
    finish_action(&ctx, &mut app, &steps);
    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&after));

    let rows = log.read(&["Private knob tone"]);
    let operation = rows[0]["operation"].as_u64().unwrap();
    assert!(rows.iter().all(|row| row["operation"] == operation));
    let release = position(&rows, |row| row["category"] == "checked_release");
    let finish = position(&rows, |row| row["category"] == "operation_finished");
    assert_eq!(rows[release]["success"], true);
    assert_eq!(rows[finish]["success"], true);
    assert!(release < finish);
    let events: Vec<_> = rows
        .iter()
        .filter(|row| row["control"] == "podxt_control")
        .map(|row| (row["disposition"].as_str().unwrap(), row["action"].clone()))
        .collect();
    let expected = [
        (
            "queued",
            serde_json::json!({"kind": "dial", "old": 20, "new": 39}),
        ),
        (
            "queued",
            serde_json::json!({"kind": "dial", "old": 39, "new": value}),
        ),
        (
            "coalesced",
            serde_json::json!({"kind": "dial", "old": 20, "new": 39}),
        ),
        (
            "queued",
            serde_json::json!({"kind": "switch", "old": true, "new": false}),
        ),
        (
            "accepted",
            serde_json::json!({"kind": "dial", "old": 39, "new": value}),
        ),
        (
            "accepted",
            serde_json::json!({"kind": "switch", "old": true, "new": false}),
        ),
        (
            "applied",
            serde_json::json!({"kind": "dial", "old": 39, "new": value}),
        ),
        (
            "applied",
            serde_json::json!({"kind": "switch", "old": true, "new": false}),
        ),
    ];
    assert_eq!(events, expected);
}
