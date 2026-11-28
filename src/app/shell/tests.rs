use super::podxt_panel::tests::sample_pod_tone;
use super::tests::{click_prompt, prompt_frame};
use super::*;

const GUITAR: &[u8] = include_bytes!("../../../tests/fixtures/editor-preset.vwm");
const HD: &[u8] = include_bytes!("../../../tests/fixtures/type12-source.h5ep");

fn empty_shell(ctx: &egui::Context) -> Workbench {
    let mut app = Workbench::new(
        ctx,
        EditorDefaults {
            artwork: PathBuf::new(),
            preferences: None,
            startup_panel: None,
        },
    );
    app.theme_polled = Instant::now();
    app
}

fn guitar_editor(app: &mut Workbench, id: TabId, name: &str) {
    app.with_tab(id, |entry, _| {
        let editor = entry.editor.as_mut() as &mut dyn std::any::Any;
        editor
            .downcast_mut::<VariaxEditor>()
            .unwrap()
            .change(|preset| preset.rename(name));
    });
}

#[test]
fn default_startup_renders_an_empty_shell_with_offline_entry_points() {
    fn text(shape: &egui::Shape, labels: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(shape) => labels.push(shape.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| text(shape, labels)),
            _ => {}
        }
    }

    let ctx = egui::Context::default();
    let directory = tempfile::tempdir().unwrap();
    let options = parse_options(
        [
            "--data-dir".to_owned(),
            directory.path().display().to_string(),
        ],
        directory.path(),
    )
    .unwrap();
    let mut app = Workbench::from_options(&ctx, options);
    app.theme_polled = Instant::now();
    let first = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
    let mut first_labels = Vec::new();
    for shape in &first.shapes {
        text(&shape.shape, &mut first_labels);
    }
    assert!(
        first_labels
            .iter()
            .any(|label| label == "Device connections")
    );
    assert!(!first_labels.iter().any(|label| label == "Home"));
    assert!(!first_labels.iter().any(|label| label == "Open file…"));
    app.startup_connection = false;
    app.connections.dismiss();
    let output = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
    let mut labels = Vec::new();
    output
        .shapes
        .iter()
        .for_each(|shape| text(&shape.shape, &mut labels));
    for required in [
        "Open file…",
        "Detect devices",
        "Test recording…",
        "Device check…",
        "Help",
        "Home",
        "Edit offline",
        "No file open",
        "No device",
    ] {
        assert!(
            labels.iter().any(|label| label == required),
            "missing {required}"
        );
    }
    assert!(labels.iter().all(|label| label != "Milestone & Help"));
    assert!(
        labels
            .iter()
            .all(|label| !label.starts_with("New offline tab · "))
    );
    assert!(labels.iter().all(|label| !label.starts_with("build ")));
    assert!(
        labels
            .iter()
            .any(|label| label == &format!("Build {}", crate::BUILD_NUMBER)),
        "missing build number"
    );
    for family in registry::FAMILIES {
        assert!(
            labels
                .iter()
                .any(|label| label.ends_with(family.info().new_label))
        );
    }
    assert!(app.tabs.active.is_none());
    assert!(app.tabs.entries.is_empty());
}

#[test]
fn app_open_routes_files_by_content_and_ignores_misleading_suffixes() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    let directory = tempfile::tempdir().unwrap();
    for (name, bytes, family) in [
        ("guitar.5xe", GUITAR.to_vec(), "variax"),
        ("pod.vwm", sample_pod_tone(), "podxt"),
        ("hd.vwb", HD.to_vec(), "hd500x"),
    ] {
        let path = directory.path().join(name);
        fs::write(&path, bytes).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(path)].into(),
            ..Default::default()
        });
        app.request_open_file(&ctx);
        app.poll_picker(&ctx);
        assert!(app.shell_error.is_none());
        let active = app.tabs.active.unwrap();
        let index = app.tabs.index(active).unwrap();
        assert_eq!(app.tabs.entries[index].family.info().id, family);
        assert!(app.tabs.entries[index].title().ends_with(name));
        assert!(!app.tabs.entries[index].editor.dirty());
    }
    let previous = app.tabs.active;
    let bad = directory.path().join("truncated.5xe");
    fs::write(&bad, b"H5EP").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        open: [Some(bad)].into(),
        ..Default::default()
    });
    app.request_open_file(&ctx);
    app.poll_picker(&ctx);
    assert!(app.shell_error.is_some());
    assert_eq!(app.tabs.active, previous);
    assert_eq!(app.tabs.entries.len(), 3);
}

#[test]
fn pending_picker_keeps_rendering_and_routes_an_editor_result_to_its_origin() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    let origin = app.tabs.active.unwrap();
    let result = std::rc::Rc::new(std::cell::Cell::new(false));
    let finish = result.clone();
    app.dialogs = Box::new(FakeDialogs {
        next_picker: Some(Box::pin(std::future::poll_fn(move |_| {
            if finish.get() {
                std::task::Poll::Ready(Some(PathBuf::from("selected.vwm")))
            } else {
                std::task::Poll::Pending
            }
        }))),
        ..Default::default()
    });
    let routed = std::rc::Rc::new(std::cell::Cell::new(false));
    let completed = routed.clone();
    app.with_tab(origin, |_, services| {
        assert!(services.request_picker::<VariaxEditor>(
            &ctx,
            PickRequest::SaveWorkbenchFile {
                name: "suggested.vwm".into(),
                extension: "vwm".into()
            },
            move |editor, path, _| {
                assert_eq!(path, Some(PathBuf::from("selected.vwm")));
                editor.change(|preset| preset.rename("Picker result"));
                completed.set(true);
            }
        ));
    });
    app.new_tab(registry::LEGACY_STARTUP);
    let other = app.tabs.active;
    app.request_open_file(&ctx); // No fake result was supplied for a second picker.
    let before = app.frame_count;
    for _ in 0..3 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
    }
    assert!(app.frame_count >= before + 3);
    assert!(app.picker.is_open());
    assert!(!routed.get());

    result.set(true);
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
    assert!(!app.picker.is_open());
    assert!(routed.get());
    assert_eq!(app.tabs.active, other);
    assert!(
        app.tabs.entries[app.tabs.index(origin).unwrap()]
            .editor
            .dirty()
    );
    assert!(
        !app.tabs.entries[app.tabs.index(other.unwrap()).unwrap()]
            .editor
            .dirty()
    );
}

#[test]
fn cli_startup_routes_hd_files_and_keeps_the_explicit_variax_view_option() {
    let ctx = egui::Context::default();
    let directory = tempfile::tempdir().unwrap();
    let hd = directory.path().join("direct.5xe");
    fs::write(&hd, HD).unwrap();
    let data_root = directory.path().display().to_string();
    let options = parse_options(
        [
            hd.display().to_string(),
            "--data-dir".into(),
            data_root.clone(),
        ],
        directory.path(),
    )
    .unwrap();
    let app = Workbench::from_options(&ctx, options);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(app.tabs.entries[0].family.info().id, "hd500x");

    let options = parse_options(
        [
            "--tab".into(),
            "controls".into(),
            "--data-dir".into(),
            data_root,
        ],
        directory.path(),
    )
    .unwrap();
    let app = Workbench::from_options(&ctx, options);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(app.variax().tab, Tab::Controls);
    assert!(!app.variax().has_document());
}

#[test]
fn same_family_tabs_keep_separate_documents_and_dirty_close_resolves_one_tab() {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    let first = app.tabs.active.unwrap();
    app.new_tab(registry::LEGACY_STARTUP);
    let second = app.tabs.active.unwrap();
    assert_ne!(first, second);
    guitar_editor(&mut app, first, "First tab");
    assert!(app.tabs.entries[0].editor.dirty());
    assert!(!app.tabs.entries[1].editor.dirty());
    guitar_editor(&mut app, second, "Second tab");

    let directory = tempfile::tempdir().unwrap();
    let collision = directory.path().join("existing.vwb");
    fs::write(&collision, b"keep").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        save: [Some(collision.clone())].into(),
        ..Default::default()
    });

    app.close_tab(first, &ctx);
    assert_eq!(app.tabs.active, Some(first));
    click_prompt(&ctx, &mut app, "save");
    assert_eq!(app.closing_tab, Some(first));
    assert_eq!(fs::read(collision).unwrap(), b"keep");
    click_prompt(&ctx, &mut app, "cancel");
    assert!(app.close_request.is_none());
    assert_eq!(app.tabs.entries.len(), 2);

    app.close_tab(first, &ctx);
    click_prompt(&ctx, &mut app, "discard");
    assert_eq!(app.tabs.active, Some(second));
    assert_eq!(app.tabs.entries.len(), 1);
    assert!(app.tabs.entries[0].editor.dirty());
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Second tab"
    );
}

#[test]
fn app_quit_checks_every_dirty_tab_and_keeps_all_tabs_when_cancelled() {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    let first = app.tabs.active.unwrap();
    app.new_tab(registry::LEGACY_STARTUP);
    let second = app.tabs.active.unwrap();
    guitar_editor(&mut app, first, "First");
    guitar_editor(&mut app, second, "Second");

    app.request_close(&ctx);
    assert_eq!(app.closing_tab, Some(first));
    click_prompt(&ctx, &mut app, "discard");
    assert_eq!(app.closing_tab, Some(second));
    click_prompt(&ctx, &mut app, "cancel");
    assert!(!app.permit_close);
    assert_eq!(app.tabs.entries.len(), 2);
    assert!(app.tabs.entries.iter().all(|entry| entry.editor.dirty()));

    app.request_close(&ctx);
    assert_eq!(app.closing_tab, Some(first));
    click_prompt(&ctx, &mut app, "discard");
    prompt_frame(&ctx, &mut app, vec![]);
    click_prompt(&ctx, &mut app, "discard");
    assert!(app.permit_close);
}

#[test]
fn control_q_uses_the_existing_quit_prompt_and_plain_q_does_nothing() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    let id = app.tabs.active.unwrap();
    guitar_editor(&mut app, id, "Unsaved");

    for (pressed, modifiers) in [
        (true, egui::Modifiers::NONE),
        (false, egui::Modifiers::NONE),
        (true, egui::Modifiers::CTRL),
    ] {
        let _ = ctx.run(
            egui::RawInput {
                modifiers,
                events: vec![egui::Event::Key {
                    key: egui::Key::Q,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers,
                }],
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
        if modifiers == egui::Modifiers::NONE {
            assert!(app.close_request.is_none());
        }
    }

    assert_eq!(app.closing_tab, Some(id));
    assert!(!app.permit_close);
    assert!(app.tabs.entries[0].editor.dirty());
}

#[test]
fn build_number_stays_bottom_right_with_status_error_and_a_long_name() {
    let caption = format!("Build {}", crate::BUILD_NUMBER);
    let long_name = format!(
        "Saved in /{}New Collection.l6c.",
        "project-directory/".repeat(9)
    );
    for editor in [false, true] {
        for &(width, height) in &[(1120.0_f32, 760.0_f32), (720.0, 640.0)] {
            for pixels_per_point in [1.0_f32, 2.0] {
                let where_ = format!("editor={editor} {width}x{height} ppp={pixels_per_point}");
                let ctx = egui::Context::default();
                let mut app = empty_shell(&ctx);
                if editor {
                    app.new_tab(registry::LEGACY_STARTUP);
                }
                app.shell_status = long_name.clone();
                app.shell_error = Some("Could not write the file.".into());
                app.detection_status = Some("Guitar offline".into());
                let input = layout_input(width, height, pixels_per_point);
                let _ = ctx.run(input.clone(), |ctx| app.render(ctx));
                let output = ctx.run(input, |ctx| app.render(ctx));
                let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
                let footer =
                    egui::containers::panel::PanelState::load(&ctx, egui::Id::new("status"))
                        .expect("footer")
                        .rect;
                let texts = painted_text(&output);
                for required in [
                    long_name.as_str(),
                    "Could not write the file.",
                    "Guitar offline",
                ] {
                    let rect = texts
                        .iter()
                        .filter(|(text, _)| text == required)
                        .map(|(_, rect)| *rect)
                        .find(|rect| footer.contains(rect.center()))
                        .unwrap_or_else(|| panic!("{where_} missing {required} in the footer"));
                    assert!(
                        text_inside(screen, rect),
                        "{where_} {required} at {rect:?} leaves {screen:?}"
                    );
                }
                assert_build_number(&where_, screen, footer, &texts);
                assert!(
                    texts.iter().any(|(text, _)| text == &caption),
                    "{where_} missing {caption}"
                );
                // The long name, device line, and error wrap to a few rows.
                // The bound is high enough for 720 points and still rejects a
                // footer that grows without limit.
                assert!(
                    footer.height() <= 120.0,
                    "{where_} footer {}",
                    footer.height()
                );
            }
        }
    }
}

#[test]
fn wrapped_status_does_not_grow_over_successive_frames() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    app.shell_status = format!(
        "Saved in /{}New Collection.l6c.",
        "project-directory/".repeat(9)
    );
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1120.0, 760.0));
    let mut heights = Vec::new();
    let mut editor_heights = Vec::new();

    for _ in 0..24 {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
        let footer = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("status"))
            .expect("shell footer");
        heights.push(footer.rect.height());
        editor_heights.push(ctx.available_rect().height());
    }

    assert!(heights[0] > 29.0, "the status should wrap: {heights:?}");
    // The build number keeps the right side, and the long filename is on
    // its own row so it cannot slide off the window. The height stays put.
    assert!(
        heights.iter().all(|height| *height <= 58.0),
        "footer heights: {heights:?}"
    );
    assert!(
        editor_heights.iter().all(|height| *height >= 560.0),
        "editor heights: {editor_heights:?}"
    );
}

fn shortcut_input(ctx: &egui::Context, key: egui::Key, shift: bool) -> egui::RawInput {
    let mac = ctx.os().is_mac();
    let modifiers = egui::Modifiers {
        command: true,
        ctrl: !mac,
        mac_cmd: mac,
        shift,
        ..Default::default()
    };
    egui::RawInput {
        modifiers,
        events: [true, false]
            .into_iter()
            .map(|pressed| egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            })
            .collect(),
        ..layout_input(1120.0, 760.0, 1.0)
    }
}

fn shortcut(ctx: &egui::Context, app: &mut Workbench, key: egui::Key, shift: bool) {
    let _ = ctx.run(shortcut_input(ctx, key, shift), |ctx| app.render(ctx));
}

#[test]
fn variax_file_commands_keep_the_active_tab_and_use_save_as_destination_afterward() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first.vwm");
    let other = directory.path().join("next.vwm");
    let copy = directory.path().join("copy.vwm");
    for path in [&first, &other] {
        variax_workbench::document::save_new(path, GUITAR).unwrap();
    }
    app.dialogs = Box::new(FakeDialogs {
        open: [Some(first.clone())].into(),
        ..Default::default()
    });
    app.request_open_file(&ctx);
    app.poll_picker(&ctx);
    let id = app.tabs.active.unwrap();
    guitar_editor(&mut app, id, "Saved before open");
    app.dialogs = Box::new(FakeDialogs {
        open: [Some(other.clone())].into(),
        save: [Some(copy.clone())].into(),
        ..Default::default()
    });

    shortcut(&ctx, &mut app, egui::Key::O, false);
    assert!(app.has_prompt());
    prompt_frame(&ctx, &mut app, vec![]);
    click_prompt(&ctx, &mut app, "save");
    assert!(!app.has_prompt());
    assert_eq!(app.tabs.active, Some(id));
    assert_eq!(app.tabs.entries.len(), 1);
    assert!(app.tabs.entries[0].title().ends_with("next.vwm"));
    assert_eq!(
        EditorDocument::open(&first).unwrap().preset().name(),
        "Saved before open"
    );
    assert_eq!(fs::read(&other).unwrap(), GUITAR);

    guitar_editor(&mut app, id, "Saved as copy");
    shortcut(&ctx, &mut app, egui::Key::S, true);
    assert!(app.tabs.entries[0].title().ends_with("copy.vwm"));
    assert_eq!(
        EditorDocument::open(&copy).unwrap().preset().name(),
        "Saved as copy"
    );
    assert_eq!(fs::read(&other).unwrap(), GUITAR);
    guitar_editor(&mut app, id, "Saved to copy");
    shortcut(&ctx, &mut app, egui::Key::S, false);
    assert_eq!(
        EditorDocument::open(&copy).unwrap().preset().name(),
        "Saved to copy"
    );
    assert!(!app.tabs.entries[0].editor.dirty());
}

fn shell_command_positions(ctx: &egui::Context, app: &mut Workbench) -> Vec<(String, egui::Rect)> {
    let mut output = egui::FullOutput::default();
    for _ in 0..2 {
        output = ctx.run(layout_input(1120.0, 760.0, 1.0), |ctx| app.render(ctx));
    }
    let header = egui::containers::panel::PanelState::load(ctx, egui::Id::new("header"))
        .unwrap()
        .rect;
    painted_text(&output)
        .into_iter()
        .filter(|(_, rect)| header.contains(rect.center()))
        .collect()
}

fn click_shell_command(ctx: &egui::Context, app: &mut Workbench, label: &str) {
    let pos = shell_command_positions(ctx, app)
        .into_iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("missing shell command {label}"))
        .1
        .center();
    for pressed in [true, false] {
        let mut input = layout_input(1120.0, 760.0, 1.0);
        input.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ];
        let _ = ctx.run(input, |ctx| app.render(ctx));
    }
}

#[test]
fn podxt_visible_open_and_shortcut_share_the_editor_and_new_tab_is_separate() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    app.new_tab(&super::podxt_panel::FAMILY);
    let original = app.tabs.active;
    let directory = tempfile::tempdir().unwrap();
    let visible = directory.path().join("Visible.l6t");
    let keyboard = directory.path().join("Keyboard.l6t");
    let new_tab = directory.path().join("Another.vwm");
    for path in [&visible, &keyboard] {
        fs::write(path, sample_pod_tone()).unwrap();
    }
    fs::write(&new_tab, GUITAR).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        pod_open: [Some(visible.clone()), Some(keyboard.clone())].into(),
        open: [Some(new_tab.clone())].into(),
        ..Default::default()
    });
    let commands = shell_command_positions(&ctx, &mut app);
    assert!(commands.iter().all(|(text, _)| text != "Save copy…"));

    click_shell_command(&ctx, &mut app, "Open…");
    assert_eq!(app.tabs.active, original);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(
        app.tabs.entries[0].editor.source_path(),
        Some(visible.as_path())
    );

    ctx.set_os(egui::os::OperatingSystem::Mac);
    shortcut(&ctx, &mut app, egui::Key::O, false);
    assert_eq!(app.tabs.active, original);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(
        app.tabs.entries[0].editor.source_path(),
        Some(keyboard.as_path())
    );

    click_shell_command(&ctx, &mut app, "Open in new tab…");
    assert_eq!(app.tabs.entries.len(), 2);
    assert_ne!(app.tabs.active, original);
    assert_eq!(
        app.tabs.entries[0].editor.source_path(),
        Some(keyboard.as_path())
    );
    assert_eq!(
        app.tabs.entries[1].editor.source_path(),
        Some(new_tab.as_path())
    );
}

#[test]
fn podxt_save_shortcuts_use_the_existing_copy_workflow() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("Original.l6t");
    let bytes = sample_pod_tone();
    fs::write(&original, &bytes).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        open: [Some(original.clone())].into(),
        ..Default::default()
    });
    app.open_file_dialog();

    let copy = directory.path().join("Copy.l6t");
    let shifted = directory.path().join("Shifted.l6t");
    app.dialogs = Box::new(FakeDialogs {
        pod_save: [Some(copy.clone()), Some(shifted.clone())].into(),
        ..Default::default()
    });
    shortcut(&ctx, &mut app, egui::Key::S, false);
    shortcut(&ctx, &mut app, egui::Key::S, true);
    for path in [&original, &copy, &shifted] {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    assert_eq!(
        app.tabs.entries[0].editor.source_path(),
        Some(original.as_path())
    );
}

#[test]
fn shell_shortcuts_leave_focused_text_and_document_edits_alone() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    let tab = app.tabs.active.unwrap();
    guitar_editor(&mut app, tab, "Document edit");
    let title = app.tabs.entries[0].title();
    app.dialogs = Box::new(FakeDialogs::default());
    let id = egui::Id::new("shell-test-text");
    let mut text = String::new();
    let mut frame = |input| {
        let _ = ctx.run(input, |ctx| {
            app.render(ctx);
            egui::Window::new("Text entry").show(ctx, |ui| {
                ui.add(egui::TextEdit::singleline(&mut text).id(id));
            });
        });
    };
    frame(layout_input(1120.0, 760.0, 1.0));
    ctx.memory_mut(|memory| memory.request_focus(id));
    let mut typed = layout_input(1120.0, 760.0, 1.0);
    typed.events.push(egui::Event::Text("Typed text".into()));
    frame(typed);
    for (key, shift) in [
        (egui::Key::O, false),
        (egui::Key::S, false),
        (egui::Key::Z, false),
    ] {
        frame(shortcut_input(&ctx, key, shift));
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    }
    assert!(text.is_empty(), "the focused text edit should receive Undo");
    assert!(app.tabs.entries[0].editor.dirty());
    assert_eq!(app.tabs.entries[0].title(), title);
    assert!(!app.picker.is_open());
}

#[test]
fn shortcut_help_tracks_the_active_editor_and_platform() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    let variax = app.tabs.active;
    app.new_tab(&super::podxt_panel::FAMILY);
    let podxt = app.tabs.active;
    app.help = true;
    for (tab, platform, expected, excluded) in [
        (
            variax,
            egui::os::OperatingSystem::Nix,
            [
                "Ctrl+O",
                "Open a file in the active Variax editor",
                "Save the active Variax file as a new copy",
            ],
            "Save a new copy of the PODxt file selection",
        ),
        (
            podxt,
            egui::os::OperatingSystem::Mac,
            [
                "Cmd+O",
                "Open a tone or bank in the active PODxt editor",
                "Save a new copy of the PODxt file selection",
            ],
            "Save the active Variax file as a new copy",
        ),
    ] {
        app.tabs.active = tab;
        ctx.set_os(platform);
        // The first frame measures the window before settling its size.
        let _ = ctx.run(layout_input(1120.0, 760.0, 1.0), |ctx| app.render(ctx));
        let output = ctx.run(layout_input(1120.0, 760.0, 1.0), |ctx| app.render(ctx));
        let texts = painted_text(&output);
        for label in expected {
            assert!(
                texts.iter().any(|(text, _)| text == label),
                "missing {label}"
            );
        }
        assert!(texts.iter().all(|(text, _)| text != excluded));
    }
}

const SHELL_COMMANDS: &[&str] = &[
    "Open file…",
    "Open…",
    "Open in new tab…",
    "Save",
    "Save As…",
    "Save copy…",
    "New offline tab",
    "Detect devices",
    "Device check…",
    "Test recording…",
    "Help",
    "New Variax file",
    "Tools",
];

fn layout_input(width: f32, height: f32, pixels_per_point: f32) -> egui::RawInput {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, height),
        )),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(pixels_per_point);
    input
}

fn glyph_rect(text: &egui::epaint::TextShape) -> egui::Rect {
    let bounds = text.galley.mesh_bounds;
    if bounds.width() > 0.0 && bounds.height() > 0.0 {
        bounds.translate(text.pos.to_vec2())
    } else {
        text.galley.rect.translate(text.pos.to_vec2())
    }
}

/// Glyph bounds that survive `clip`. A scroll area records the whole galley
/// and clips it at tessellation, so the unclipped rect can reach the footer
/// after the glyphs have been cut off.
fn visible_glyph_rect(text: &egui::epaint::TextShape, clip: egui::Rect) -> Option<egui::Rect> {
    let visible = glyph_rect(text).intersect(clip);
    (visible.width() > 0.5 && visible.height() > 0.5).then_some(visible)
}

fn painted_text(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, found: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text) => {
                if let Some(visible) = visible_glyph_rect(text, clip) {
                    found.push((text.galley.text().to_owned(), visible));
                }
            }
            egui::Shape::Vec(shapes) => {
                shapes.iter().for_each(|shape| walk(shape, clip, found));
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, clipped.clip_rect, &mut found);
    }
    found
}

fn text_overlaps(left: egui::Rect, right: egui::Rect) -> bool {
    let shared = left.intersect(right);
    shared.width() > 1.0 && shared.height() > 1.0
}

fn assert_build_number(
    where_: &str,
    screen: egui::Rect,
    footer: egui::Rect,
    texts: &[(String, egui::Rect)],
) {
    let caption = format!("Build {}", crate::BUILD_NUMBER);
    let rect = texts
        .iter()
        .find(|(text, _)| text == &caption)
        .unwrap_or_else(|| panic!("{where_} missing {caption}"))
        .1;
    assert!(
        text_inside(screen, rect),
        "{where_} build {rect:?} leaves {screen:?}"
    );
    assert!(
        footer.contains(rect.center()),
        "{where_} build {rect:?} is outside footer {footer:?}"
    );
    assert!(
        (footer.bottom() - rect.bottom()).abs() < 14.0,
        "{where_} build bottom {} footer {}",
        rect.bottom(),
        footer.bottom()
    );
    assert!(
        rect.right() > footer.right() - 28.0 && rect.right() <= footer.right() - 8.0,
        "{where_} build right {} footer {}",
        rect.right(),
        footer.right()
    );
    if let Some(text) = footer_overlap(&caption, rect, footer, texts) {
        panic!("{where_} build overlaps {text}");
    }
}

fn footer_overlap<'a>(
    caption: &str,
    build: egui::Rect,
    footer: egui::Rect,
    texts: &'a [(String, egui::Rect)],
) -> Option<&'a str> {
    texts.iter().find_map(|(text, other)| {
        if text == caption || !footer.contains(other.center()) {
            return None;
        }
        text_overlaps(build, *other).then_some(text.as_str())
    })
}

fn text_inside(screen: egui::Rect, rect: egui::Rect) -> bool {
    rect.left() >= screen.left() - 0.5
        && rect.right() <= screen.right() + 0.5
        && rect.top() >= screen.top() - 0.5
        && rect.bottom() <= screen.bottom() + 0.5
}

fn shell_at(
    editor: bool,
    missing_art: bool,
    clear_window_controls: bool,
    width: f32,
    height: f32,
    pixels_per_point: f32,
) -> (egui::Context, egui::FullOutput) {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    if editor {
        app.new_tab(registry::LEGACY_STARTUP);
    }
    app.clear_window_controls = clear_window_controls;
    if missing_art {
        super::shell::force_missing_shell_art(&ctx, true);
    }
    let output = ctx.run(layout_input(width, height, pixels_per_point), |ctx| {
        app.render(ctx)
    });
    (ctx, output)
}

#[test]
fn shell_layout_stays_readable_at_laptop_and_narrow_widths() {
    let screens = [(1120.0_f32, 760.0_f32), (960.0, 680.0), (720.0, 640.0)];
    for editor in [false, true] {
        for missing_art in [false, true] {
            for &(width, height) in &screens {
                for clear_window_controls in [false, true] {
                    let mut title_height: Option<f32> = None;
                    for pixels_per_point in [1.0_f32, 2.0] {
                        let where_ = format!(
                            "editor={editor} missing={missing_art} inset={clear_window_controls} {width}x{height} ppp={pixels_per_point}"
                        );
                        let (ctx, output) = shell_at(
                            editor,
                            missing_art,
                            clear_window_controls,
                            width,
                            height,
                            pixels_per_point,
                        );
                        assert!(
                            (output.pixels_per_point - pixels_per_point).abs() < 0.01,
                            "{where_} density {}",
                            output.pixels_per_point
                        );
                        let screen =
                            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
                        let texts = painted_text(&output);
                        assert!(
                            texts.iter().all(|(text, _)| !text.contains("shell/")),
                            "{where_} painted an asset path"
                        );
                        let header = egui::containers::panel::PanelState::load(
                            &ctx,
                            egui::Id::new("header"),
                        )
                        .expect("header")
                        .rect;
                        let footer = egui::containers::panel::PanelState::load(
                            &ctx,
                            egui::Id::new("status"),
                        )
                        .expect("footer")
                        .rect;
                        let title = texts
                            .iter()
                            .filter(|(text, _)| text == "Variax Workbench")
                            .min_by(|left, right| left.1.top().total_cmp(&right.1.top()))
                            .unwrap_or_else(|| panic!("missing title at {where_}"))
                            .1;
                        assert!(text_inside(screen, title), "{where_} title {title:?}");
                        if clear_window_controls {
                            assert!(
                                title.left() >= 78.0 - 0.5,
                                "{where_} title starts at {}",
                                title.left()
                            );
                        }
                        if let Some(previous) = title_height {
                            assert!(
                                (previous - title.height()).abs() < 1.0,
                                "{where_} title height {previous} vs {}",
                                title.height()
                            );
                        }
                        title_height = Some(title.height());

                        let required: &[&str] = if editor {
                            &[
                                "Open…",
                                "Open in new tab…",
                                "Save",
                                "Save As…",
                                "New offline tab",
                                "Detect devices",
                                "New Variax file",
                                "Guitar offline",
                                "Tools",
                                "Help",
                            ]
                        } else {
                            &[
                                "Open file…",
                                "New offline tab",
                                "Detect devices",
                                "Device check…",
                                "Test recording…",
                                "Help",
                                "Home",
                                "Edit offline",
                            ]
                        };
                        for label in required {
                            assert!(
                                texts.iter().any(|(text, _)| text == label),
                                "{where_} missing {label}"
                            );
                        }

                        let mut commands = Vec::new();
                        for (text, rect) in &texts {
                            if !header.contains(rect.center()) {
                                continue;
                            }
                            assert!(
                                text_inside(screen, *rect),
                                "{where_} header text {text:?} at {rect:?}"
                            );
                            if clear_window_controls {
                                assert!(
                                    rect.left() >= 78.0 - 0.5,
                                    "{where_} {text:?} starts at {}",
                                    rect.left()
                                );
                            }
                            if SHELL_COMMANDS.contains(&text.as_str()) {
                                assert!(
                                    !text_overlaps(*rect, title),
                                    "{where_} {text:?} overlaps the title"
                                );
                                commands.push((text.clone(), *rect));
                            }
                        }
                        for (index, (left_text, left_rect)) in commands.iter().enumerate() {
                            for (right_text, right_rect) in commands.iter().skip(index + 1) {
                                assert!(
                                    !text_overlaps(*left_rect, *right_rect),
                                    "{where_} {left_text:?} {left_rect:?} overlaps {right_text:?} {right_rect:?}"
                                );
                            }
                        }
                        if !editor {
                            let mut family_titles = Vec::new();
                            for family in registry::FAMILIES {
                                let label = family.info().new_label;
                                let rect = texts
                                    .iter()
                                    .filter(|(text, _)| text == label)
                                    .map(|(_, rect)| *rect)
                                    .max_by(|left, right| left.top().total_cmp(&right.top()))
                                    .unwrap_or_else(|| panic!("{where_} missing {label}"));
                                assert!(text_inside(screen, rect), "{where_} {label} {rect:?}");
                                family_titles.push(rect);
                            }
                            for (index, left) in family_titles.iter().enumerate() {
                                for right in family_titles.iter().skip(index + 1) {
                                    assert!(
                                        !text_overlaps(*left, *right),
                                        "{where_} family titles overlap"
                                    );
                                }
                            }
                            let min_top = family_titles
                                .iter()
                                .map(|rect| rect.top())
                                .fold(f32::MAX, f32::min);
                            let max_top = family_titles
                                .iter()
                                .map(|rect| rect.top())
                                .fold(f32::MIN, f32::max);
                            if width >= 960.0 {
                                assert!(
                                    max_top - min_top < 4.0,
                                    "{where_} family cards wrapped ({min_top}..{max_top})"
                                );
                            } else {
                                assert!(
                                    max_top - min_top > 40.0,
                                    "{where_} family cards stayed on one row"
                                );
                            }
                        }

                        if missing_art {
                            let mark = texts
                                .iter()
                                .find(|(text, _)| text == "WB")
                                .unwrap_or_else(|| panic!("{where_} missing product mark"));
                            assert!(mark.1.width() <= 28.0 && mark.1.height() <= 28.0);
                            if !editor {
                                assert!(texts.iter().any(|(text, _)| text == "PODxt"));
                                assert!(texts.iter().any(|(text, _)| text == "HD"));
                            }
                        } else {
                            assert!(texts.iter().all(|(text, _)| text != "WB"));
                        }
                        assert_build_number(&where_, screen, footer, &texts);
                    }
                }
            }
        }
    }
}

#[test]
fn clipped_tuning_scale_does_not_count_as_a_footer_overlap() {
    let (ctx, mut output) = shell_at(true, false, false, 1120.0, 760.0, 1.0);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1120.0, 760.0));
    let footer = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("status"))
        .expect("footer")
        .rect;
    let visible = painted_text(&output);
    assert_build_number("1120x760 ppp=1", screen, footer, &visible);

    let caption = format!("Build {}", crate::BUILD_NUMBER);
    let build = visible
        .iter()
        .find(|(text, _)| text == &caption)
        .expect("build caption")
        .1;
    let galley = ctx.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            "+12".into(),
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        )
    });
    let mut scale = egui::epaint::TextShape::new(egui::Pos2::ZERO, galley, egui::Color32::WHITE);
    scale.pos = build.center() - glyph_rect(&scale).center().to_vec2();
    assert!(text_overlaps(build, glyph_rect(&scale)));
    output.shapes.push(egui::epaint::ClippedShape {
        clip_rect: egui::Rect::from_min_max(screen.min, egui::pos2(screen.right(), footer.top())),
        shape: egui::Shape::Text(scale),
    });
    let clipped = painted_text(&output);
    assert_build_number("clipped +12", screen, footer, &clipped);
    assert_eq!(footer_overlap(&caption, build, footer, &clipped), None);

    // The identical glyph must count as a collision when its clip is removed.
    output.shapes.last_mut().unwrap().clip_rect = screen;
    let unclipped = painted_text(&output);
    assert_eq!(
        footer_overlap(&caption, build, footer, &unclipped),
        Some("+12")
    );
}

#[test]
fn shell_icon_keeps_aspect_and_a_missing_file_stays_in_its_slot() {
    let ctx = egui::Context::default();
    let texture = ctx.load_texture(
        "wide-shell-mark",
        egui::ColorImage::filled([80, 40], egui::Color32::WHITE),
        egui::TextureOptions::LINEAR,
    );
    let loaded = variax_workbench::ui::assets::UiImage {
        name: "shell/wide.png".into(),
        pixels: [80, 40],
        texture: Some(texture),
        missing: false,
    };
    let mut slot = egui::Rect::NOTHING;
    let output = ctx.run(layout_input(200.0, 160.0, 2.0), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
            slot = rect;
            super::shell::paint_shell_icon(ui, &loaded, rect, "WB");
        });
    });
    let mut meshes = Vec::new();
    fn walk(shape: &egui::Shape, meshes: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Mesh(mesh) => {
                let mut rect = egui::Rect::NOTHING;
                for vertex in &mesh.vertices {
                    rect.extend_with(vertex.pos);
                }
                if rect.width() > 0.0 && rect.height() > 0.0 {
                    meshes.push(rect);
                }
            }
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, meshes)),
            _ => {}
        }
    }
    output
        .shapes
        .iter()
        .for_each(|shape| walk(&shape.shape, &mut meshes));
    assert_eq!(meshes.len(), 1, "{meshes:?}");
    let image = meshes[0];
    assert!((image.width() / image.height() - 2.0).abs() < 0.05);
    assert!(
        image.left() >= slot.left() - 0.5
            && image.right() <= slot.right() + 0.5
            && image.top() >= slot.top() - 0.5
            && image.bottom() <= slot.bottom() + 0.5,
        "{image:?} outside {slot:?}"
    );
    assert!((output.pixels_per_point - 2.0).abs() < 0.01);

    let missing = variax_workbench::ui::assets::UiImage::missing("shell/variax.png");
    let mut missing_slot = egui::Rect::NOTHING;
    let output = ctx.run(layout_input(200.0, 160.0, 1.0), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::hover());
            missing_slot = rect;
            super::shell::paint_shell_icon(ui, &missing, rect, "Variax");
        });
    });
    let texts = painted_text(&output);
    assert!(texts.iter().all(|(text, _)| !text.contains("shell/")));
    let mark = texts
        .iter()
        .find(|(text, _)| text == "Variax")
        .expect("missing mark");
    assert!(
        mark.1.left() >= missing_slot.left() - 0.5
            && mark.1.right() <= missing_slot.right() + 0.5
            && mark.1.top() >= missing_slot.top() - 0.5
            && mark.1.bottom() <= missing_slot.bottom() + 0.5,
        "{:?} outside {missing_slot:?}",
        mark.1
    );
}

#[test]
fn home_and_editor_tabs_keep_keyboard_focus_and_unsaved_edits_during_connection_updates() {
    use variax_workbench::ui::connections::{Attachment, Detection, Device, Verification};

    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    app.new_tab(registry::LEGACY_STARTUP);
    let editor = app.tabs.active.unwrap();
    guitar_editor(&mut app, editor, "Unsaved edit");
    assert!(app.tabs.entries[0].editor.dirty());

    fn draw(app: &mut Workbench, ctx: &egui::Context, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1120.0, 760.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn key(key: egui::Key, pressed: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    draw(&mut app, &ctx, vec![]);
    draw(&mut app, &ctx, vec![]);
    let pos = app.tab_buttons["home"].center();
    let pointer = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    draw(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(pos), pointer(true)],
    );
    draw(&mut app, &ctx, vec![pointer(false)]);
    assert!(app.tabs.active.is_none());
    let home_focus = (0..20)
        .find_map(|_| {
            draw(&mut app, &ctx, vec![key(egui::Key::Tab, true)]);
            draw(&mut app, &ctx, vec![key(egui::Key::Tab, false)]);
            let focused = ctx.memory(|memory| memory.focused())?;
            let response = ctx.read_response(focused)?;
            (response.rect == app.tab_buttons["home"]).then_some(focused)
        })
        .expect("Home is keyboard focusable");

    let pod = Attachment {
        device: 1,
        generation: 1,
    };
    app.connections.detection(Detection::Checking);
    app.connections.arrived(
        pod,
        Device {
            name: "PODxt Live".into(),
            artwork: Some("shell/podxt-live.png"),
            offline_editor: Some("podxt"),
            upstream: None,
        },
    );
    app.connections.verified(pod, Verification::Verified);
    app.connections.detection(Detection::Complete);
    let read = app
        .connections
        .started(pod, 299.try_into().unwrap())
        .unwrap();
    draw(&mut app, &ctx, vec![]);
    draw(&mut app, &ctx, vec![]);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(home_focus));
    assert!(app.tabs.active.is_none());

    app.connections.progress(read, 128);
    draw(&mut app, &ctx, vec![key(egui::Key::Tab, true)]);
    draw(
        &mut app,
        &ctx,
        vec![key(egui::Key::Tab, false), key(egui::Key::Enter, true)],
    );
    assert_eq!(app.tabs.active, Some(editor));
    assert_eq!(app.tabs.entries.len(), 1);
    assert!(app.tabs.entries[0].editor.dirty());
    app.with_tab(editor, |entry, _| {
        let editor = (&mut *entry.editor as &mut dyn std::any::Any)
            .downcast_mut::<VariaxEditor>()
            .unwrap();
        assert_eq!(
            editor.document.as_ref().unwrap().preset().name(),
            "Unsaved edit"
        );
    });
}
