use super::*;
use crate::editor::FakeDialogs;

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

    for pressed in [true, false] {
        render_stage(
            ctx,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
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

    click_text(&ctx, &mut app, "Open tone or bank…");
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
    rendered(&ctx, &mut app, &["Playing Tone", "Channel 1A"]);

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

    let requests = app.podxt().live.device_requests;
    click_text(&ctx, &mut app, "32C  Saved Channel");
    let panel = &app.podxt().live;
    assert_eq!(panel.device_requests, requests + 1);
    assert_eq!(panel.playing_channel(), Some(slot));
    assert_eq!(
        panel.message.as_deref(),
        Some("Refresh the playing tone before selecting a channel.")
    );
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
    click_text(&ctx, &mut app, "Open tone or bank…");
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
    click_text(&ctx, &mut app, "Open tone or bank…");
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
    rendered(
        &ctx,
        &mut app,
        &["Stage Current", "Channel 5B", "Playing · 5B"],
    );
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
