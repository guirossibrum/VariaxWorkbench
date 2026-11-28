use super::*;
use std::cell::Cell;
use std::rc::Rc;
use std::task::Poll;
use variax_workbench::session_log::{SessionGuard, SessionLog, Severity, SinkState};

fn frame(
    app: &mut Workbench,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<(String, Rect)> {
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1120., 760.))),
            events,
            ..Default::default()
        },
        |ctx| app.render(ctx),
    );
    painted_text(&output)
}

fn click(app: &mut Workbench, ctx: &egui::Context, label: &str) {
    frame(app, ctx, vec![]);
    let labels = frame(app, ctx, vec![]);
    let pos = labels
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("missing {label} in {labels:?}"))
        .1
        .center();
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(app, ctx, vec![egui::Event::PointerMoved(pos), button(true)]);
    frame(app, ctx, vec![button(false)]);
}

fn open_from_help(app: &mut Workbench, ctx: &egui::Context) {
    click(app, ctx, "Help");
    click(app, ctx, "Diagnostics…");
}

fn wait_for_visible(app: &mut Workbench, ctx: &egui::Context, expected: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let labels = frame(app, ctx, vec![]);
        if labels.iter().any(|(text, _)| text.starts_with(expected)) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "missing {expected} in {labels:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn with_log(
    app: &mut Workbench,
    folder: PathBuf,
    expected: SinkState,
) -> (SessionLog, SessionGuard) {
    let (log, guard) = SessionLog::open(Some(folder), Severity::Info);
    let deadline = Instant::now() + Duration::from_secs(5);
    while log.status().state == SinkState::Starting {
        assert!(Instant::now() < deadline, "log worker did not start");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(log.status().state, expected);
    app.session_log_panel.use_log(log.clone());
    (log, guard)
}

#[test]
fn help_diagnostics_shows_the_session_and_exports_through_the_shared_picker() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("logs");
    let (log, _guard) = with_log(&mut app, folder.clone(), SinkState::Recording);
    let source = log.status().path.unwrap();
    let target = root.path().join("export.jsonl");
    let ready = Rc::new(Cell::new(false));
    let select = ready.clone();
    let selected = target.clone();
    app.dialogs = Box::new(FakeDialogs {
        next_picker: Some(Box::pin(std::future::poll_fn(move |_| {
            if select.get() {
                Poll::Ready(Some(selected.clone()))
            } else {
                Poll::Pending
            }
        }))),
        ..Default::default()
    });

    open_from_help(&mut app, &ctx);
    for expected in [
        "Session log",
        "Recording to the session log.",
        "Minimum level: INFO",
        &source.to_string_lossy(),
        &folder.to_string_lossy(),
        "Open log folder",
    ] {
        wait_for_visible(&mut app, &ctx, expected);
    }
    click(&mut app, &ctx, "Export session log…");
    assert!(app.picker.is_open());
    assert!(!app.request_picker(&ctx, PickRequest::Folder, PickerAction::OpenFile));
    assert!(!target.exists());
    ready.set(true);
    frame(&mut app, &ctx, vec![]);
    assert!(app.session_log_panel.export_pending());
    wait_for_visible(&mut app, &ctx, "Session log exported to ");
    assert!(!app.picker.is_open());
    let bytes = fs::read(target).unwrap();
    assert!(!bytes.is_empty());
    assert_eq!(bytes, fs::read(source).unwrap());
}

#[test]
fn help_diagnostics_with_no_session_log_keeps_export_disabled() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    open_from_help(&mut app, &ctx);
    wait_for_visible(&mut app, &ctx, "Session logging is inactive.");
    wait_for_visible(&mut app, &ctx, "Minimum level: Unavailable");
    click(&mut app, &ctx, "Export session log…");
    assert!(!app.picker.is_open());
}

#[test]
fn help_diagnostics_shows_an_unwritable_sink_and_folder_open_error() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("logs");
    fs::write(&folder, b"blocks the log folder").unwrap();
    let (_log, _guard) = with_log(&mut app, folder.clone(), SinkState::Unavailable);

    open_from_help(&mut app, &ctx);
    wait_for_visible(&mut app, &ctx, "Workbench cannot write the session log.");
    wait_for_visible(&mut app, &ctx, &folder.to_string_lossy());
    click(&mut app, &ctx, "Export session log…");
    assert!(!app.picker.is_open());
    click(&mut app, &ctx, "Open log folder");
    wait_for_visible(&mut app, &ctx, "Could not open log folder:");
    assert_eq!(fs::read(folder).unwrap(), b"blocks the log folder");
}

#[test]
fn help_diagnostics_handles_cancel_collision_and_destination_failure() {
    let ctx = egui::Context::default();
    let mut app = empty_shell(&ctx);
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("logs");
    let (_log, _guard) = with_log(&mut app, folder, SinkState::Recording);
    let existing = root.path().join("existing.jsonl");
    let missing = root.path().join("missing").join("export.jsonl");
    fs::write(&existing, b"preserve contents").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        save: [None, Some(existing.clone()), Some(missing.clone())].into(),
        ..Default::default()
    });

    open_from_help(&mut app, &ctx);
    click(&mut app, &ctx, "Export session log…");
    wait_for_visible(&mut app, &ctx, "Session log export canceled.");
    assert!(!app.picker.is_open());
    assert_eq!(root.path().read_dir().unwrap().count(), 2);

    click(&mut app, &ctx, "Export session log…");
    wait_for_visible(&mut app, &ctx, "That file already exists.");
    assert_eq!(fs::read(existing).unwrap(), b"preserve contents");

    click(&mut app, &ctx, "Export session log…");
    wait_for_visible(&mut app, &ctx, "Could not export session log:");
    assert!(!missing.exists());
}
