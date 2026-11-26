use super::*;
use crate::editor::FakeDialogs;

fn click_text(ctx: &egui::Context, app: &mut Workbench, label: &str) {
    fn find(shape: &egui::epaint::Shape, label: &str) -> Option<Pos2> {
        match shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|item| find(item, label)),
            _ => None,
        }
    }
    let frame = render_stage(ctx, app, vec![]);
    let pos = frame
        .shapes
        .iter()
        .find_map(|item| {
            let pos = find(&item.shape, label)?;
            item.clip_rect.contains(pos).then_some(pos)
        })
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
                    time: Duration::ZERO,
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
        &["Connected", "Channel 1A", "Playing Tone", "On PODxt Live"],
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
        &["File editor", "Local Document", "Current POD tone"],
    );
    assert!(!app.podxt().stage_is_current());

    click_text(&ctx, &mut app, "2A  Saved Channel");
    rendered(&ctx, &mut app, &["Device library copy", "Saved Channel"]);
    assert_eq!(app.podxt().live.snapshot.tone, before);
    assert_eq!(
        app.podxt().live.playing_channel(),
        Location::tone_from_label("1A")
    );
    assert_eq!(app.podxt().live.device_requests, requests);

    click_text(&ctx, &mut app, "Current POD tone");
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
    assert!(app.podxt().live.snapshot.tone.is_none());
    assert!(app.podxt().live.snapshot.library.is_empty());
    assert!(!app.podxt().live.connected());
    assert!(!app.podxt().live.live_editing);
    assert_eq!(app.podxt().live.device_requests, requests);
    assert!(!app.podxt().busy());
}

#[test]
fn offline_startup_imports_and_browses_all_three_backup_banks_without_device_requests() {
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

    click_text(&ctx, &mut app, "Tones");
    click_text(&ctx, &mut app, "Amp Setups");
    rendered(&ctx, &mut app, &["Recallable amp", "AMP 1  Backup Amp"]);
    click_text(&ctx, &mut app, "AMP 1  Backup Amp");
    let amp = app.podxt().pod_library.tone(0).unwrap();
    assert_eq!(amp.name(), "Backup Amp");
    assert_eq!(amp.container(), pod_preset::ContainerKind::L6as);

    click_text(&ctx, &mut app, "Amp Setups");
    click_text(&ctx, &mut app, "FX Setups");
    rendered(&ctx, &mut app, &["Recallable effect", "FX 1  Backup FX"]);
    click_text(&ctx, &mut app, "FX 1  Backup FX");
    let fx = app.podxt().pod_library.tone(0).unwrap();
    assert_eq!(fx.name(), "Backup FX");
    assert_eq!(fx.container(), pod_preset::ContainerKind::L6fs);

    assert!(!app.podxt().busy());
    assert!(!app.podxt().dirty());
    assert!(app.podxt().live.snapshot.tone.is_none());
    assert_eq!(app.podxt().live.device_requests, 0);
    assert_eq!(
        std::fs::read(backup.join("MANIFEST.sha256")).unwrap(),
        manifest
    );
}
