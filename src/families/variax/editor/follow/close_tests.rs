use super::*;
use crate::editor::{FakeDialogs, Workbench, registry, tests::app};
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

struct DelayedRelease {
    rig: Arc<Mutex<()>>,
    stopped: Sender<()>,
    finish: Receiver<()>,
    fail: bool,
}

static NEXT_RELEASE: Mutex<Option<DelayedRelease>> = Mutex::new(None);
const FRAME_DEADLINE: Duration = Duration::from_secs(3);
const CLEANUP_ERROR: &str = "fake checked driver reattach failed";

fn delayed_follow(
    _: &Attachment,
    stop: &AtomicBool,
    _: &AtomicBool,
    ready: &mut dyn FnMut(),
    publish: &mut dyn FnMut(FollowUpdate, Instant),
) -> Result<ListenEnd, String> {
    let release = NEXT_RELEASE.lock().unwrap().take().unwrap();
    let _lease = release.rig.lock().unwrap();
    ready();
    while !stop.load(Ordering::Relaxed) {
        thread::park_timeout(Duration::from_millis(20));
    }
    publish(FollowUpdate::Disconnected, Instant::now());
    let _ = release.stopped.send(());
    release.finish.recv_timeout(FRAME_DEADLINE * 3).unwrap();
    if release.fail {
        Err(CLEANUP_ERROR.into())
    } else {
        Ok(ListenEnd::Stopped)
    }
}

#[derive(Clone, Copy, Debug)]
enum CloseCase {
    CleanTab,
    DirtyTabDiscard,
    DirtyTabSave,
    ControlQ,
    WindowClose,
}

impl CloseCase {
    fn quit(self) -> bool {
        matches!(self, Self::ControlQ | Self::WindowClose)
    }

    fn dirty(self) -> bool {
        matches!(self, Self::DirtyTabDiscard | Self::DirtyTabSave)
    }
}

fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) -> egui::FullOutput {
    render(
        ctx,
        app,
        egui::RawInput {
            events,
            ..Default::default()
        },
    )
}

fn render(ctx: &egui::Context, app: &mut Workbench, mut input: egui::RawInput) -> egui::FullOutput {
    input.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(1301.0, 974.0),
    ));
    ctx.run(input, |ctx| app.render(ctx))
}

fn click(ctx: &egui::Context, app: &mut Workbench, pos: egui::Pos2) -> egui::FullOutput {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(ctx, app, vec![egui::Event::PointerMoved(pos), button(true)]);
    frame(ctx, app, vec![button(false)])
}

fn click_tab_close(ctx: &egui::Context, app: &mut Workbench, id: u64) -> egui::FullOutput {
    frame(ctx, app, vec![]);
    let pos = ctx.data(|data| {
        data.get_temp::<egui::Rect>(egui::Id::new(("test-close-tab", id)))
            .unwrap()
            .center()
    });
    click(ctx, app, pos)
}

fn viewport_command(output: &egui::FullOutput, command: egui::ViewportCommand) -> bool {
    output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .contains(&command)
}

fn rendered_text(output: &egui::FullOutput, fragment: &str) -> bool {
    fn contains(shape: &egui::Shape, fragment: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.text().contains(fragment),
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains(shape, fragment)),
            _ => false,
        }
    }
    output
        .shapes
        .iter()
        .any(|item| contains(&item.shape, fragment))
}

fn exercise_close(case: CloseCase) {
    let (stopped, wait_stopped) = mpsc::channel();
    let (finish, wait_finish) = mpsc::channel();
    let (responsive, wait_responsive) = mpsc::channel();
    let rig = Arc::new(Mutex::new(()));
    let fail = matches!(case, CloseCase::DirtyTabDiscard);
    *NEXT_RELEASE.lock().unwrap() = Some(DelayedRelease {
        rig: rig.clone(),
        stopped,
        finish: wait_finish,
        fail,
    });
    let renderer = thread::spawn(move || {
        let (ctx, mut app) = app();
        app.detection = None;
        let id = app.test_editor_id::<VariaxEditor>();
        let original = super::super::refresh::tests::verified_tab();
        let mut replacement = original.device.clone();
        let index = app.tabs.index(id).unwrap();
        app.tabs.entries[index].device = Some(original);
        app.variax_mut().follow.test_source = Some(delayed_follow);

        let deadline = Instant::now() + FRAME_DEADLINE;
        loop {
            frame(&ctx, &mut app, vec![]);
            if app
                .variax()
                .follow
                .worker
                .as_ref()
                .is_some_and(|worker| worker.continuous.load(Ordering::Acquire))
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{case:?}: fake Follow never became ready"
            );
            thread::yield_now();
        }
        assert!(!FamilyTab::dirty(app.variax()));

        let temp = tempfile::tempdir().unwrap();
        let save_path = temp.path().join("working.vwm");
        if case.dirty() {
            app.variax_mut()
                .change(|preset| preset.rename("Pending edit"));
            assert!(FamilyTab::dirty(app.variax()));

            click_tab_close(&ctx, &mut app, id);
            crate::editor::tests::click_prompt(&ctx, &mut app, "cancel");
            assert!(app.tabs.index(id).is_some());
            assert!(FamilyTab::dirty(app.variax()));
            assert!(
                !app.variax()
                    .follow
                    .worker
                    .as_ref()
                    .unwrap()
                    .stop
                    .load(Ordering::Relaxed)
            );
        }

        let output = match case {
            CloseCase::ControlQ => {
                crate::editor::tests::prepare_quit_take(
                    &mut app,
                    variax_workbench::recording::ClockSelection::Integer(44_100),
                );
                let modifiers = egui::Modifiers::CTRL;
                render(
                    &ctx,
                    &mut app,
                    egui::RawInput {
                        modifiers,
                        events: vec![egui::Event::Key {
                            key: egui::Key::Q,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers,
                        }],
                        ..Default::default()
                    },
                )
            }
            CloseCase::WindowClose => {
                let mut input = egui::RawInput::default();
                input
                    .viewports
                    .get_mut(&egui::ViewportId::ROOT)
                    .unwrap()
                    .events
                    .push(egui::ViewportEvent::Close);
                render(&ctx, &mut app, input)
            }
            _ => click_tab_close(&ctx, &mut app, id),
        };
        assert!(!viewport_command(&output, egui::ViewportCommand::Close));
        if case.quit() {
            assert!(viewport_command(
                &output,
                egui::ViewportCommand::CancelClose
            ));
        }
        if case.dirty() {
            if matches!(case, CloseCase::DirtyTabSave) {
                app.dialogs = Box::new(FakeDialogs {
                    save: [Some(save_path.clone())].into(),
                    ..Default::default()
                });
                crate::editor::tests::click_prompt(&ctx, &mut app, "save");
                let saved =
                    super::super::PresetFile::parse(std::fs::read(&save_path).unwrap()).unwrap();
                assert_eq!(saved.name(), "Pending edit");
            } else {
                crate::editor::tests::click_prompt(&ctx, &mut app, "discard");
            }
        }

        if matches!(case, CloseCase::ControlQ) {
            assert!(app.recorder.has_unsaved_take());
            assert!(
                !app.variax()
                    .follow
                    .worker
                    .as_ref()
                    .unwrap()
                    .stop
                    .load(Ordering::Relaxed),
                "Follow must keep running until the recording decision"
            );
            crate::editor::tests::click_prompt(&ctx, &mut app, "discard");
            assert!(!app.recorder.has_unsaved_take());
        }

        let pending = frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(egui::pos2(1100.0, 550.0))],
        );
        assert!(!pending.shapes.is_empty());
        assert!(!viewport_command(&pending, egui::ViewportCommand::Close));
        assert!(app.tabs.index(id).is_some());
        assert!(app.variax().follow.busy());
        assert!(!app.variax().follow.disconnected());
        assert!(!app.permit_close);

        if !case.quit() {
            let home = app.tab_buttons["home"].center();
            click(&ctx, &mut app, home);
            assert_eq!(app.tabs.active, None, "Home must take input during release");
        }

        let replacement = if fail {
            replacement.key.attachment.path = "8-2.2".into();
            app.tabs
                .reconcile_devices(vec![replacement.clone()], registry::FAMILIES, &app.defaults)
                .unwrap();
            let current = app
                .tabs
                .entries
                .iter()
                .find(|entry| {
                    entry
                        .device
                        .as_ref()
                        .is_some_and(|device| device.device.key == replacement.key)
                })
                .unwrap();
            assert_ne!(current.id, id, "replace on a separate tab during release");
            Some((current.id, replacement.key))
        } else {
            None
        };

        responsive.send(()).unwrap();
        let deadline = Instant::now() + FRAME_DEADLINE * 2;
        loop {
            let output = frame(&ctx, &mut app, vec![]);
            if case.quit() && app.permit_close {
                assert!(viewport_command(&output, egui::ViewportCommand::Close));
                assert!(!app.variax().follow.busy());
                break;
            }
            if !case.quit() && app.tabs.index(id).is_none() {
                if let Some((replacement_id, key)) = replacement {
                    let device = app.tabs.entries[app.tabs.index(replacement_id).unwrap()]
                        .device
                        .as_ref()
                        .unwrap();
                    assert_eq!(device.connection, crate::editor::tabs::Connection::Present);
                    assert_eq!(device.device.key, key);
                    let warning =
                        format!("Variax Follow cleanup on USB 8-2.1 (address 10): {CLEANUP_ERROR}");
                    let mut output = output;
                    while !rendered_text(&output, &warning) {
                        assert!(
                            Instant::now() < deadline,
                            "cleanup warning must identify the original attachment"
                        );
                        output = frame(&ctx, &mut app, vec![]);
                        thread::sleep(Duration::from_millis(2));
                    }
                }
                break;
            }
            assert!(
                Instant::now() < deadline,
                "{case:?}: close did not complete after checked release"
            );
            thread::sleep(Duration::from_millis(2));
        }
    });

    let stopped = wait_stopped.recv_timeout(FRAME_DEADLINE);
    let responsive = wait_responsive.recv_timeout(FRAME_DEADLINE);
    let lease_held = rig.try_lock().is_err();
    let _ = finish.send(());
    renderer.join().unwrap();
    stopped.expect("Follow never began releasing");
    responsive.expect("input-bearing frame stalled while checked release was outstanding");
    assert!(
        lease_held,
        "the fake rig must stay owned through checked release"
    );
    assert!(rig.try_lock().is_ok());
}

#[test]
fn follow_close_and_quit_frames_wait_for_checked_release_without_blocking() {
    for case in [
        CloseCase::CleanTab,
        CloseCase::DirtyTabDiscard,
        CloseCase::DirtyTabSave,
        CloseCase::ControlQ,
        CloseCase::WindowClose,
    ] {
        exercise_close(case);
    }
}
