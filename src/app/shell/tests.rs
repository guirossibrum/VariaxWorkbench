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
    let options = parse_options(Vec::new(), directory.path()).unwrap();
    let mut app = Workbench::from_options(&ctx, options);
    app.theme_polled = Instant::now();
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
        "Open a file or start an offline tab",
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
    let options = parse_options([hd.display().to_string()], directory.path()).unwrap();
    let app = Workbench::from_options(&ctx, options);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(app.tabs.entries[0].family.info().id, "hd500x");

    let options = parse_options(["--tab".into(), "controls".into()], directory.path()).unwrap();
    let app = Workbench::from_options(&ctx, options);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(app.variax().tab, Tab::Controls);
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Spank example"
    );
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
