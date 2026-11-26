use super::super::tests::{app, click_prompt, prompt_frame, recording_frame};
use super::*;

#[test]
fn dirty_open_cancel_failed_save_retry_and_discard_follow_actual_prompt_buttons() {
    let dir = tempfile::tempdir().unwrap();
    let next = dir.path().join("next.vwm");
    let saved = dir.path().join("saved.vwm");
    let mut preset =
        PresetFile::parse(include_bytes!("../../../../tests/fixtures/editor-preset.vwm").to_vec())
            .unwrap();
    preset.rename("Next document").unwrap();
    std::fs::write(&next, preset.as_bytes()).unwrap();
    let (context, mut app) = app();
    context.style_mut(|s| s.animation_time = 0.);
    app.dialogs = Box::new(FakeDialogs {
        open: [None, Some(next.clone()), Some(next.clone())].into(),
        save: [None, Some(next.clone()), Some(saved.clone())].into(),
        ..Default::default()
    });
    app.variax_mut().change(|p| p.rename("Keep me"));
    let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    app.open_dialog();
    assert!(matches!(app.test_pending(), Some(Pending::Open)));
    assert!(!app.picker.is_open());
    click_prompt(&context, &mut app, "discard"); // Cancelling the picker retains the edits.
    assert!(app.test_pending().is_none());
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    app.open_dialog();
    assert!(matches!(app.test_pending(), Some(Pending::Open)));
    app.variax_mut().change(|p| p.rename("Behind prompt"));
    app.variax_mut().undo();
    app.variax_mut().redo();
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "save-as");
    assert!(app.test_pending().is_some());
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    click_prompt(&context, &mut app, "save-as"); // Existing destination is refused.
    assert!(app.test_pending().is_some());
    assert!(app.variax().error.is_some());
    assert_eq!(std::fs::read(&next).unwrap(), preset.as_bytes());
    click_prompt(&context, &mut app, "save-as");
    assert!(app.picker.is_open());
    app.poll_picker(&context);
    assert!(app.test_pending().is_none());
    assert_eq!(std::fs::read(saved).unwrap(), before);
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Next document"
    );
    assert!(app.variax().error.is_none());
    app.variax_mut().change(|p| p.rename("Discard me"));
    app.open_dialog();
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Discard me"
    );
    app.open_dialog();
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "discard");
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Next document"
    );
}

#[test]
fn clean_document_save_cancellation_does_not_continue_an_existing_prompt() {
    let (context, mut app) = app();
    context.style_mut(|s| s.animation_time = 0.);
    app.dialogs = Box::new(FakeDialogs {
        save: [None].into(),
        ..Default::default()
    });
    app.force_test_quit::<VariaxEditor>();
    assert!(!app.variax().dirty());
    prompt_frame(&context, &mut app, vec![]);
    click_prompt(&context, &mut app, "save");
    assert!(matches!(app.test_pending(), Some(Pending::Quit)));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "discard");
    assert!(app.permit_close);
    assert!(app.test_pending().is_none());
}

#[test]
fn failed_open_and_missing_save_directory_preserve_dirty_document_and_history() {
    let (context, mut app) = app();
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.vwb");
    std::fs::write(&bad, b"invalid container").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        open: [Some(bad)].into(),
        save: [Some(dir.path().join("missing/copy.vwm"))].into(),
        ..Default::default()
    });
    app.variax_mut().change(|p| p.rename("Retained"));
    let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    app.save_dialog();
    assert!(app.variax().error.is_some());
    app.open_dialog();
    click_prompt(&context, &mut app, "discard");
    assert!(app.variax().error.is_some());
    assert!(app.variax().dirty());
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    app.variax_mut().undo();
    assert!(!app.variax().dirty());
    app.variax_mut().redo();
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
}

#[test]
fn close_during_fake_operation_cancels_it_and_retains_document_after_disconnect() {
    let (context, mut app) = app();
    app.variax_mut().change(|p| p.rename("Retained"));
    let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (sender, receiver) = std::sync::mpsc::channel();
    app.variax_mut().read_receiver = Some(receiver);
    app.variax_mut().read_worker = Some((cancel.clone(), std::thread::spawn(|| {})));
    app.request_close(&context);
    assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
    assert!(!app.permit_close);
    assert!(app.test_pending().is_none());
    app.open_dialog();
    app.save_dialog(); // Fake adapter panics if invoked.
    drop(sender);
    app.variax_mut().poll_read();
    assert!(!app.variax().is_reading());
    assert!(app.variax().read_worker.is_none());
    assert!(
        app.variax()
            .error
            .as_deref()
            .unwrap()
            .contains("unexpectedly")
    );
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::Quit)));
    click_prompt(&context, &mut app, "discard");
    assert!(app.permit_close);
}

#[test]
fn actual_pickup_slider_drag_is_one_undo_group() {
    fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.variax_mut().float_control(
                        ui,
                        PICKUP1,
                        0x200000,
                        "Position",
                        ValueFormat::POSITION,
                    )
                });
            },
        );
    }
    let (context, mut app) = app();
    context.style_mut(|style| style.spacing.slider_width = 200.);
    app.variax_mut().load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm"),
    );
    let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    frame(&context, &mut app, vec![]);
    let rect = app.variax().pickup_slider.unwrap();
    let start = Pos2::new(rect.left() + 30., rect.center().y);
    let end = start + Vec2::new(90., 0.);
    frame(
        &context,
        &mut app,
        vec![
            egui::Event::PointerMoved(start),
            egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    for pos in [start + Vec2::new(40., 0.), end] {
        frame(&context, &mut app, vec![egui::Event::PointerMoved(pos)]);
    }
    frame(
        &context,
        &mut app,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    let changed = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    assert_ne!(changed, before);
    app.variax_mut().undo();
    let doc = app.variax().document.as_ref().unwrap();
    assert_eq!(doc.as_bytes(), before);
    assert!(!doc.can_undo());
    app.variax_mut().redo();
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), changed);
}

#[test]
fn dragging_either_pickup_follows_the_pointer_and_undoes_in_one_step() {
    fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    for (slot, other_slot, direction) in [(PICKUP1, PICKUP2, -1.0), (PICKUP2, PICKUP1, 1.0)] {
        let context = egui::Context::default();
        let artwork = tempfile::tempdir().unwrap();
        let mut app = Workbench::with_context(&context, None, artwork.path().to_owned(), None);
        app.theme_polled = Instant::now();
        app.variax_mut().install_pickup_fixture(&context);
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        let initial = app.variax().float(slot, 0x200000).unwrap();
        let other = app.variax().float(other_slot, 0x200000).unwrap();
        frame(&context, &mut app, vec![]);
        frame(&context, &mut app, vec![]);
        let start = app.variax().pickup_centers[&slot];
        let step = Vec2::new(8.0 * direction, 0.0);
        frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(
            &context,
            &mut app,
            vec![egui::Event::PointerMoved(start + step)],
        );
        let moved = app.variax().float(slot, 0x200000).unwrap();
        assert!((moved - initial) * direction > 0.0);
        frame(&context, &mut app, vec![]);
        assert_eq!(app.variax().float(slot, 0x200000), Some(moved));

        let second = start + step * 2.0 + Vec2::new(0.0, 10.0);
        frame(&context, &mut app, vec![egui::Event::PointerMoved(second)]);
        let moved_twice = app.variax().float(slot, 0x200000).unwrap();
        assert!(
            (moved_twice - (2.0 * moved - initial).clamp(0.1, 9.5)).abs() < 1e-5,
            "{slot:x}: {initial} -> {moved} -> {moved_twice}"
        );

        let end = second + step;
        frame(&context, &mut app, vec![button(end, false)]);
        assert_eq!(app.variax().float(slot, 0x200000), Some(moved_twice));
        assert_eq!(app.variax().float(other_slot, 0x200000), Some(other));
        let changed = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        app.variax_mut().undo();
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.as_bytes(), before);
        assert!(!document.can_undo());
        app.variax_mut().redo();
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), changed);
    }
}

#[test]
fn fretboard_enable_button_uses_preset_history_and_respects_pending_actions() {
    fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 680.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.guitar_workspace(ui));
            },
        );
    }

    let (context, mut app) = app();
    app.variax_mut().load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm"),
    );
    app.variax_mut().tab = Tab::Pickups;
    let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let tuning_enabled = |app: &Workbench| {
        app.variax()
            .document
            .as_ref()
            .unwrap()
            .preset()
            .model(TUNING)
            .unwrap()
            .enabled
    };
    let was_enabled = tuning_enabled(&app);

    let click_toggle = |app: &mut Workbench| {
        frame(&context, app, vec![]);
        frame(&context, app, vec![]);
        let pos = app.variax().tuning_toggle.unwrap().center();
        for pressed in [true, false] {
            frame(
                &context,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    };

    app.variax_mut().pending = Some(GuitarPending::Quit);
    click_toggle(&mut app);
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);

    app.cancel_test_prompt();
    click_toggle(&mut app);
    assert_ne!(tuning_enabled(&app), was_enabled);
    assert_eq!(app.variax().tab, Tab::Pickups);
    app.variax_mut().undo();
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
    assert!(!app.variax().document.as_ref().unwrap().can_undo());
    app.variax_mut().redo();
    assert_ne!(tuning_enabled(&app), was_enabled);

    app.variax_mut().tab = Tab::Controls;
    click_toggle(&mut app);
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
    assert_eq!(app.variax().tab, Tab::Controls);
}

#[test]
fn pickup_enable_keeps_one_on_and_front_follows_the_original() {
    let (context, mut app) = app();
    app.variax_mut().load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm"),
    );
    app.variax_mut().tab = Tab::Pickups;
    let enabled = |app: &Workbench, slot: u32| {
        app.variax()
            .document
            .as_ref()
            .unwrap()
            .preset()
            .model(slot)
            .unwrap()
            .enabled
    };
    assert!(enabled(&app, PICKUP1));
    assert!(enabled(&app, PICKUP2));
    assert_eq!(app.variax().pickup_front, PICKUP1);
    let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();

    let click = |context: &egui::Context, app: &mut Workbench, slot: u32, kind: &'static str| {
        recording_frame(context, app, vec![]);
        let pos = app.variax().pickup_controls[&(slot, kind)].center();
        for pressed in [true, false] {
            recording_frame(
                context,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
    };

    click(&context, &mut app, PICKUP1, "enabled");
    assert!(!enabled(&app, PICKUP1));
    assert!(enabled(&app, PICKUP2));
    assert_eq!(app.variax().pickup_front, PICKUP2);
    app.variax_mut().undo();
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
    assert!(!app.variax().document.as_ref().unwrap().can_undo());

    click(&context, &mut app, PICKUP2, "front");
    assert_eq!(app.variax().pickup_front, PICKUP2);
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
    assert!(!app.variax().document.as_ref().unwrap().can_undo());

    click(&context, &mut app, PICKUP2, "enabled");
    assert!(enabled(&app, PICKUP1));
    assert!(!enabled(&app, PICKUP2));
    assert_eq!(app.variax().pickup_front, PICKUP1);
    click(&context, &mut app, PICKUP1, "enabled");
    assert!(!enabled(&app, PICKUP1));
    assert!(enabled(&app, PICKUP2));
    assert_eq!(app.variax().pickup_front, PICKUP2);
    app.variax_mut().undo();
    assert!(enabled(&app, PICKUP1));
    assert!(!enabled(&app, PICKUP2));
    assert!(app.variax().document.as_ref().unwrap().can_undo());

    let mut both_off = original.clone();
    for slot in [PICKUP1, PICKUP2] {
        let pattern = slot.to_be_bytes();
        let at = both_off
            .windows(4)
            .enumerate()
            .find_map(|(index, window)| {
                (window == pattern && index >= 4 && both_off[index - 4] == 0x03)
                    .then_some(index + 7)
            })
            .unwrap();
        both_off[at] = 0;
    }
    let preset = PresetFile::parse(both_off.clone()).unwrap();
    app.variax_mut().document = Some(EditorDocument::from_preset(
        preset,
        PathBuf::from("both-off.vwm"),
    ));
    recording_frame(&context, &mut app, vec![]);
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), both_off);
    assert!(!enabled(&app, PICKUP1));
    assert!(!enabled(&app, PICKUP2));
    assert!(!app.variax().document.as_ref().unwrap().can_undo());
}

fn pinf_name_offset(bytes: &[u8]) -> usize {
    let mut cursor = 12;
    while cursor + 8 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        if &bytes[cursor..cursor + 4] == b"PINF" {
            return cursor + 16;
        }
        cursor = cursor + 8 + length + (length & 1);
    }
    panic!("missing PINF");
}

#[test]
fn auto_select_follows_pickup_models_and_is_remembered() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.txt");
    assert!(load_auto_select(&missing));
    let garbage = directory.path().join("garbage.txt");
    std::fs::write(&garbage, "maybe\n").unwrap();
    assert!(load_auto_select(&garbage));
    let path = directory.path().join("editor-preferences.txt");
    save_auto_select(&path, false).unwrap();
    assert!(!load_auto_select(&path));

    let context = egui::Context::default();
    let mut app = Workbench::with_context(&context, None, PathBuf::new(), Some("controls"));
    assert!(app.variax().auto_select_controls);
    assert!(app.variax().error.is_none());
    app.variax_mut().preferences_path = Some(path.clone());
    recording_frame(&context, &mut app, vec![]);
    assert!(app.variax().auto_select_checkbox.is_some());

    let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let taper = app.variax().integer(WIRING, 0x200001);
    app.variax_mut().select_pickup_model(PICKUP1, 0x03118000);
    assert!(app.variax().error.is_none());
    assert_eq!(app.variax().model_id(PICKUP1), Some(0x03118000));
    assert_eq!(app.variax().float(WIRING, 0x200000), Some(500.0));
    assert_eq!(app.variax().float(WIRING, 0x200002), Some(500.0));
    assert_eq!(app.variax().float(WIRING, 0x200003), Some(0.047));
    assert_eq!(app.variax().integer(WIRING, 0x200001), taper);
    assert!(app.variax().document.as_ref().unwrap().can_undo());
    app.variax_mut().undo();
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    assert!(!app.variax().document.as_ref().unwrap().can_undo());

    app.variax_mut().select_pickup_model(PICKUP2, 0x03110002);
    assert_eq!(app.variax().float(WIRING, 0x200000), Some(250.0));
    assert_eq!(app.variax().float(WIRING, 0x200002), Some(250.0));
    assert_eq!(app.variax().float(WIRING, 0x200003), Some(0.022));
    app.variax_mut().undo();

    app.variax_mut().set_auto_select_controls(false);
    assert!(app.variax().error.is_none());
    assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "0");
    app.variax_mut().select_pickup_model(PICKUP1, 0x03118007);
    assert_eq!(app.variax().model_id(PICKUP1), Some(0x03118007));
    assert_eq!(app.variax().float(WIRING, 0x200002), Some(470.0));
    assert_eq!(app.variax().float(WIRING, 0x200003), Some(0.011));

    app.variax_mut().set_auto_select_controls(true);
    assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), "1");
    assert_eq!(app.variax().model_id(PICKUP1), Some(0x03118007));
    assert_eq!(app.variax().float(WIRING, 0x200000), Some(500.0));
    assert_eq!(app.variax().float(WIRING, 0x200002), Some(500.0));
    assert_eq!(app.variax().float(WIRING, 0x200003), Some(0.047));
    app.variax_mut().undo();
    assert_eq!(app.variax().model_id(PICKUP1), Some(0x03118007));
    assert_eq!(app.variax().float(WIRING, 0x200002), Some(470.0));
    assert_eq!(app.variax().float(WIRING, 0x200003), Some(0.011));
}

#[test]
fn legacy_name_limit_keeps_a_saved_name_and_marks_it() {
    let (context, mut app) = app();
    let stored = app
        .variax()
        .document
        .as_ref()
        .unwrap()
        .preset()
        .name()
        .to_owned();
    assert!(
        app.variax()
            .document
            .as_ref()
            .unwrap()
            .preset()
            .limits_legacy_name()
    );
    assert!(variax_workbench::preset::legacy_text_issue(&stored, true).is_none());
    recording_frame(&context, &mut app, vec![]);
    assert!(app.variax().name_notice.is_empty());

    let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let at = pinf_name_offset(&original);
    let needle: Vec<u8> = stored.encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert_eq!(&original[at..at + needle.len()], needle.as_slice());
    let long = "Too long for guitar!";
    assert_eq!(long.len(), 20);
    let encoded: Vec<u8> = long.encode_utf16().flat_map(u16::to_be_bytes).collect();
    assert!(at + 64 <= original.len());
    let mut bytes = original.clone();
    bytes[at..at + 64].fill(0);
    bytes[at..at + encoded.len()].copy_from_slice(&encoded);
    assert_eq!(&bytes[..at], &original[..at]);
    assert_eq!(&bytes[at + 64..], &original[at + 64..]);
    let preset = PresetFile::parse(bytes.clone()).unwrap();
    assert_eq!(preset.name(), long);
    assert!(preset.limits_legacy_name());
    app.variax_mut().document = Some(EditorDocument::from_preset(
        preset,
        PathBuf::from("kept.vwm"),
    ));
    recording_frame(&context, &mut app, vec![]);
    assert!(app.variax().name_notice.contains("Model: Kept as saved"));
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), bytes);
    assert!(!app.variax().document.as_ref().unwrap().is_dirty());
    assert!(!app.variax().document.as_ref().unwrap().can_undo());

    app.variax_mut().name_draft = Some("caf\u{e9}".into());
    recording_frame(&context, &mut app, vec![]);
    assert!(app.variax().name_notice.contains("Model: Not saved"));
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        long
    );
    assert!(!app.variax().document.as_ref().unwrap().can_undo());

    app.variax_mut().name_draft = None;
    app.variax_mut()
        .change(|preset| preset.rename("1234567890123456789"));
    assert!(app.variax().error.as_ref().unwrap().contains("18"));
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), bytes);
    app.variax_mut().error = None;
    app.variax_mut()
        .change(|preset| preset.rename("Short name"));
    assert!(app.variax().error.is_none());
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Short name"
    );
    app.variax_mut().undo();
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        long
    );
    assert!(app.variax().name_draft.is_none());
    assert!(!app.variax().document.as_ref().unwrap().can_undo());
}

#[test]
fn offline_example_opens_as_a_clean_editable_preset() {
    let context = egui::Context::default();
    let mut app = Workbench::with_context(&context, None, PathBuf::new(), None);

    assert!(app.variax().error.is_none());
    assert_eq!(app.variax().tab, Tab::Pickups);
    assert_eq!(app.variax().model_name(BODY), "Spank");
    assert_eq!(app.variax().float(SYSTEM, 0x200000), Some(0.0));
    let document = app.variax().document.as_ref().unwrap();
    assert!(!document.is_dirty());
    assert!(!document.can_undo());
    let original = document.as_bytes().to_vec();

    app.variax_mut()
        .change(|preset| preset.rename("My example"));
    assert!(app.variax().dirty());
    app.variax_mut().undo();
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
}

#[test]
fn artwork_dialog_cancel_and_folder_recovery_preserve_document() {
    let (context, mut app) = app();
    let directory = tempfile::tempdir().unwrap();
    let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let old = app.variax().artwork.clone();
    app.dialogs = Box::new(FakeDialogs {
        artwork: [None, Some(directory.path().to_owned())].into(),
        ..Default::default()
    });
    assert!(
        app.variax_mut()
            .texture(&context, "missing-test.png")
            .is_none()
    );
    app.artwork_dialog();
    assert_eq!(app.variax().artwork, old);
    assert!(app.variax().error.is_none());
    assert_eq!(app.variax().missing_textures.len(), 1);
    app.artwork_dialog();
    assert_eq!(app.variax().artwork, directory.path());
    assert!(app.variax().error.is_none());
    assert!(app.variax().missing_textures.is_empty());
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    assert!(!app.variax().dirty());
}

#[test]
fn active_read_blocks_document_mutations_and_shortcuts() {
    let (context, mut app) = app();
    app.variax_mut()
        .change(|preset| preset.rename("Before read"));
    let before = app
        .variax()
        .document
        .as_ref()
        .unwrap()
        .preset()
        .as_bytes()
        .to_vec();
    let (sender, receiver) = std::sync::mpsc::channel();
    app.variax_mut().read_receiver = Some(receiver);
    app.variax_mut()
        .change(|preset| preset.rename("Must not replace it"));
    app.variax_mut().undo();
    app.variax_mut().redo();
    app.variax_mut()
        .load(Path::new("this-file-must-not-be-opened.vwm"));
    app.open_dialog();
    app.save_dialog();
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120.0, 760.0))),
        modifiers,
        events: vec![egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let output = context.run(input, |ctx| app.render(ctx));
    assert!(!output.shapes.is_empty());
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().as_bytes(),
        before
    );
    assert!(app.variax().is_reading());
    sender.send(Err("simulated read failure".into())).unwrap();
    app.variax_mut().poll_read();
    assert!(!app.variax().is_reading());
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().as_bytes(),
        before
    );
    assert_eq!(
        app.variax().error.as_deref(),
        Some("simulated read failure")
    );
}

#[test]
fn a_test_build_never_starts_a_live_read_worker() {
    // The settings-read gate is released (2026-11-23); the test-build
    // guard still stops the read before any worker or device access.
    let (_, mut app) = app();
    app.variax_mut().start_read();
    assert!(!app.variax().is_reading());
    assert!(app.variax().read_worker.is_none());
    assert!(app.test_pending().is_none());
    assert!(
        app.variax().status.contains("Test builds"),
        "{}",
        app.variax().status
    );
    app.variax_mut().launch_read();
    assert!(!app.variax().is_reading());
    assert!(app.variax().read_worker.is_none());
}

#[test]
fn rendering_imported_out_of_range_values_does_not_edit_or_create_history() {
    let (context, mut app) = app();
    let mut bytes = include_bytes!("../../../../tests/fixtures/synthetic-host-preset.vwm").to_vec();
    for (id, kind, old, new) in [
        (0x3f200000u32, 1u32, 6f32.to_bits(), f32::NAN.to_bits()),
        (0x3f200001, 1, 45f32.to_bits(), 95f32.to_bits()),
        (0x3f200000, 0, (-2i32) as u32, 1000),
        (0x3f200006, 1, 50f32.to_bits(), 200f32.to_bits()),
    ] {
        let pattern = [id.to_be_bytes(), kind.to_be_bytes(), old.to_be_bytes()].concat();
        let offset = bytes
            .windows(pattern.len())
            .position(|w| w == pattern)
            .unwrap();
        bytes[offset + 8..offset + 12].copy_from_slice(&new.to_be_bytes());
    }
    app.variax_mut().document = Some(EditorDocument::from_preset(
        PresetFile::parse(bytes.clone()).unwrap(),
        "imported.vwm".into(),
    ));
    for tab in [Tab::Body, Tab::Pickups, Tab::Controls] {
        app.variax_mut().tab = tab;
        for _ in 0..2 {
            let _ = context.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.guitar_workspace(ui));
            });
        }
        let doc = app.variax().document.as_ref().unwrap();
        assert_eq!(doc.preset().as_bytes(), bytes);
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
    }
}

#[test]
fn missing_artwork_is_cached_without_altering_the_document() {
    let (context, mut app) = app();
    let before = app
        .variax()
        .document
        .as_ref()
        .unwrap()
        .preset()
        .as_bytes()
        .to_vec();
    let path = "definitely-not-an-original-asset.png";
    assert!(app.variax_mut().texture(&context, path).is_none());
    assert!(app.variax().missing_textures.contains(path));
    assert!(app.variax_mut().texture(&context, path).is_none());
    assert_eq!(app.variax().missing_textures.len(), 1);
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().as_bytes(),
        before
    );
    assert!(app.variax().error.is_none());
}

#[test]
fn bank_dropdown_and_slots_preserve_edits_and_block_browsing_during_an_operation() {
    fn frame(context: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = context.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1120., 760.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| app.guitar_workspace(ui))
                    .inner
            },
        );
    }

    fn click(context: &egui::Context, app: &mut Workbench, pos: Pos2) {
        for pressed in [true, false] {
            frame(
                context,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        frame(context, app, vec![]);
    }

    fn bank(context: &egui::Context, app: &mut Workbench, bank: usize) {
        click(context, app, app.variax().bank_dropdown.unwrap().center());
        let pos = app.variax().bank_options[&bank].center();
        click(context, app, pos);
    }

    use variax_workbench::preset::PresetText;
    for name in ["synthetic-bank.vwb", "synthetic-collection.l6c"] {
        let (context, mut app) = app();
        context.style_mut(|style| style.animation_time = 0.);
        app.variax_mut().load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        );
        frame(&context, &mut app, vec![]);
        frame(&context, &mut app, vec![]);
        assert_eq!(
            app.variax().document.as_ref().unwrap().selection(),
            Some((1, 0))
        );
        bank(&context, &mut app, 0); // The first bank is empty.
        assert_eq!(
            app.variax().document.as_ref().unwrap().selection(),
            Some((1, 0))
        );
        click(&context, &mut app, Pos2::new(1050.0, 120.0));
        bank(&context, &mut app, 2);
        let slot = app.variax().bank_slots[&1].center();
        click(&context, &mut app, slot);
        assert_eq!(
            app.variax().document.as_ref().unwrap().selection(),
            Some((2, 1))
        );
        app.variax_mut()
            .change(|p| p.set_metadata(PresetText::Author, "Guitarist"));
        app.variax_mut()
            .change(|p| p.set_metadata(PresetText::Notes, "Drop D\nStudio take 🎸"));
        bank(&context, &mut app, 1);

        let doc = app.variax().document.as_ref().unwrap();
        assert_eq!(doc.selection(), Some((1, 1)));
        assert!(doc.slot_is_dirty(2, 1));
        assert!(!doc.slot_is_dirty(1, 1));
        assert!(doc.is_dirty());

        let (sender, receiver) = std::sync::mpsc::channel();
        app.variax_mut().read_receiver = Some(receiver);
        frame(&context, &mut app, vec![]);
        let slot = app.variax().bank_slots[&0].center();
        click(&context, &mut app, slot);
        assert_eq!(
            app.variax().document.as_ref().unwrap().selection(),
            Some((1, 1))
        );
        sender
            .send(Err("offline simulated failure".into()))
            .unwrap();
        app.variax_mut().poll_read();

        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join(name);
        let doc = app.variax_mut().document.as_mut().unwrap();
        doc.save_copy(&destination).unwrap();
        assert!(!doc.slot_is_dirty(2, 1));
        assert!(!doc.is_dirty());
        let mut reopened = EditorDocument::open(&destination).unwrap();
        reopened.select(2, 1).unwrap();
        assert_eq!(
            reopened.preset().metadata(PresetText::Author).unwrap(),
            "Guitarist"
        );
        assert_eq!(
            reopened.preset().metadata(PresetText::Notes).unwrap(),
            "Drop D\nStudio take 🎸"
        );

        app.variax_mut().undo();
        assert!(app.variax().document.as_ref().unwrap().slot_is_dirty(2, 1));
        app.variax_mut().redo();
        assert!(!app.variax().document.as_ref().unwrap().is_dirty());
    }
}

#[test]
fn delayed_open_keeps_rendering_and_completes_in_the_originating_variax_tab() {
    let (ctx, mut app) = app();
    let origin = app.test_editor_id::<VariaxEditor>();
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/synthetic-host-preset.vwm");
    let expected = std::fs::read(&path).unwrap();
    let ready = std::rc::Rc::new(std::cell::Cell::new(false));
    let signal = ready.clone();
    app.dialogs = Box::new(FakeDialogs {
        next_picker: Some(Box::pin(std::future::poll_fn(move |_| {
            if signal.get() {
                std::task::Poll::Ready(Some(path.clone()))
            } else {
                std::task::Poll::Pending
            }
        }))),
        ..Default::default()
    });

    app.test_action(|editor: &mut VariaxEditor, services| {
        editor.open_dialog(&ctx, services);
        editor.artwork_dialog(&ctx, services);
    });
    assert!(app.picker.is_open());
    let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    let other = app
        .tabs
        .entries
        .iter()
        .find(|entry| entry.id != origin)
        .unwrap()
        .id;
    app.tabs.active = Some(other);
    let frame_count = app.frame_count;
    let _ = ctx.run(Default::default(), |ctx| app.render(ctx));
    assert!(app.frame_count > frame_count);
    assert!(app.picker.is_open());
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);

    ready.set(true);
    app.poll_picker(&ctx);
    assert!(!app.picker.is_open());
    assert_eq!(app.tabs.active, Some(other));
    assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), expected);
}

#[test]
fn delayed_save_and_close_waits_for_atomic_publication() {
    let (ctx, mut app) = app();
    let id = app.test_editor_id::<VariaxEditor>();
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("saved.vwm");
    let path = destination.clone();
    let ready = std::rc::Rc::new(std::cell::Cell::new(false));
    let signal = ready.clone();
    app.dialogs = Box::new(FakeDialogs {
        next_picker: Some(Box::pin(std::future::poll_fn(move |_| {
            if signal.get() {
                std::task::Poll::Ready(Some(path.clone()))
            } else {
                std::task::Poll::Pending
            }
        }))),
        ..Default::default()
    });
    app.variax_mut()
        .change(|preset| preset.rename("Save before close"));
    let expected = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
    app.close_tab(id, &ctx);
    click_prompt(&ctx, &mut app, "save");
    assert!(app.picker.is_open());
    assert_eq!(app.closing_tab, Some(id));
    assert!(app.tabs.index(id).is_some());
    assert!(!destination.exists());

    ready.set(true);
    app.poll_picker(&ctx);
    assert!(!app.picker.is_open());
    assert!(app.tabs.index(id).is_none());
    assert_eq!(std::fs::read(destination).unwrap(), expected);
}

#[test]
fn tuning_off_warns_and_dims_but_keeps_hand_edits_audibly_off() {
    const NOTICE: &str = "Alternate tuning is off: these offsets are saved but not played.";
    let (context, mut app) = app();
    app.variax_mut().open_example();
    let panel = variax_workbench::ui::theme::PANEL;
    let [r, g, b, _] = panel.to_array();
    let dim = Color32::from_rgba_unmultiplied(r, g, b, 96);
    let rendered = |app: &mut Workbench| {
        let output = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.variax_mut().fretboard(ui, true));
        });
        (
            output.shapes.iter().any(|item| {
                matches!(&item.shape, egui::Shape::Text(text) if text.galley.text() == NOTICE)
            }),
            output.shapes.iter().any(|item| {
                matches!(&item.shape, egui::Shape::Rect(rect) if rect.fill == dim)
            }),
        )
    };
    app.variax_mut()
        .change(|preset| preset.set_enabled(TUNING, false));
    app.variax_mut().set_tuning([1, -1, 0, 0, 0, 0], false);
    assert_eq!(rendered(&mut app), (true, true));
    assert_eq!(app.variax().integer(TUNING, 0x200000), Some(1));
    assert_eq!(app.variax().integer(TUNING, 0x200001), Some(-1));
    app.variax_mut().set_tuning([-2, 0, 0, 0, 0, 0], true);
    assert_eq!(rendered(&mut app), (false, false));
}

#[test]
fn body_and_inspector_cards_stay_put_when_alternate_tuning_is_toggled() {
    for width in [960.0, 1440.0] {
        let (context, mut app) = app();
        app.variax_mut().open_example();
        let render = |app: &mut Workbench, events| {
            let _ = context.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 900.0))),
                    events,
                    ..Default::default()
                },
                |ctx| app.render(ctx),
            );
        };
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        let initial_placement = app.variax().workspace_cards.unwrap();
        let tuning_enabled = |app: &Workbench| {
            app.variax()
                .document
                .as_ref()
                .unwrap()
                .preset()
                .model(TUNING)
                .unwrap()
                .enabled
        };
        let was_enabled = tuning_enabled(&app);
        let pos = app.variax().tuning_toggle.unwrap().center();
        for pressed in [true, false] {
            render(
                &mut app,
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
        render(&mut app, vec![]);
        assert_ne!(tuning_enabled(&app), was_enabled);
        assert_eq!(
            app.variax().workspace_cards,
            Some(initial_placement),
            "at width {width}"
        );
    }
}

#[test]
fn acoustic_body_hides_pickup_sprites_and_disables_electrics_with_workbench_labels() {
    fn has_text(shape: &egui::Shape, expected: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.text() == expected,
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| has_text(shape, expected)),
            _ => false,
        }
    }
    let (ctx, mut app) = app();
    app.variax_mut().open_example();
    app.variax_mut().install_pickup_fixture(&ctx);
    let render = |app: &mut Workbench| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 960.0))),
                ..Default::default()
            },
            |ctx| app.render(ctx),
        )
    };
    let expect = |output: egui::FullOutput, label| {
        assert!(
            output
                .shapes
                .iter()
                .any(|shape| has_text(&shape.shape, label)),
            "missing {label}"
        );
    };
    let _ = render(&mut app);
    assert_eq!(app.variax().pickup_centers.len(), 2);
    app.variax_mut()
        .change(|preset| preset.select_model(BODY, 0x0310_0012));
    app.variax_mut().tab = Tab::Pickups;
    expect(render(&mut app), "No pickups available");
    expect(
        render(&mut app),
        "Pickup models are not available when using an acoustic body model.",
    );
    assert!(app.variax().pickup_centers.is_empty());
    app.variax_mut().tab = Tab::Controls;
    expect(render(&mut app), "Param not available");
    expect(render(&mut app), "No 12-String available");
    assert!(!app.variax().can_edit(WIRING, None));
}
