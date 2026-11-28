use super::*;
use std::sync::mpsc;

struct FakeTuner {
    snapshot: Arc<Mutex<Snapshot>>,
    stop: Arc<AtomicBool>,
    finish: mpsc::Sender<Result<(), String>>,
}

impl FakeTuner {
    fn start(app: &mut Workbench) -> Self {
        let live = &mut app.podxt_mut().live;
        assert!(live.worker.is_none());
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (finish, release) = mpsc::channel();
        live.worker = Some(Worker {
            command: ReadCommand::Tuner,
            device: live.device.clone().expect("verified fake attachment"),
            snapshot: Arc::clone(&snapshot),
            stop: Arc::clone(&stop),
            acquired: None,
            thread: Some(std::thread::spawn(move || {
                release
                    .recv()
                    .unwrap_or_else(|error| Err(error.to_string()))
            })),
        });
        live.tuner.started(Ok(()));
        Self {
            snapshot,
            stop,
            finish,
        }
    }

    fn publish(&self, note: Option<u16>, deviation: Option<u16>, received_at: Instant) {
        self.snapshot.lock().unwrap().apply(
            ReadUpdate::Tuner(TunerReading {
                at: Duration::from_secs(59),
                note,
                deviation,
            }),
            received_at,
        );
    }

    fn complete(self, app: &Workbench, result: Result<(), String>) {
        self.finish.send(result).unwrap();
        let thread = app
            .podxt()
            .live
            .worker
            .as_ref()
            .unwrap()
            .thread
            .as_ref()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !thread.is_finished() {
            assert!(Instant::now() < deadline, "fake worker did not release");
            std::thread::yield_now();
        }
    }
}

fn escape(ctx: &egui::Context, app: &mut Workbench) {
    render_stage(
        ctx,
        app,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
}

fn text_centers(output: &egui::FullOutput, label: &str) -> Vec<Pos2> {
    fn collect(shape: &egui::Shape, label: &str, clip: Rect, centers: &mut Vec<Pos2>) {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let center = text.pos + text.galley.rect.center().to_vec2();
                if clip.contains(center) {
                    centers.push(center);
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect(shape, label, clip, centers);
                }
            }
            _ => {}
        }
    }
    let mut centers = Vec::new();
    for item in &output.shapes {
        collect(&item.shape, label, item.clip_rect, &mut centers);
    }
    centers
}

fn popup_close(ctx: &egui::Context) -> Pos2 {
    let window = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("shared-tuner-popup")))
        .unwrap();
    let title_height = theme::TARGET + 24.0;
    window.right_top() + egui::vec2(-1.0 - title_height / 2.0, 1.0 + title_height / 2.0)
}

fn click(ctx: &egui::Context, app: &mut Workbench, pos: Pos2) {
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

#[test]
fn tuner_pairs_use_receipt_time_and_escape_waits_for_worker_release() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let requests = app.podxt().live.device_requests;
    let playing = app.podxt().live.snapshot.tone.clone();
    let fake = FakeTuner::start(&mut app);
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("Starting tuner…"), "{visible}");
    assert!(app.podxt().busy());
    assert!(!matches!(
        app.podxt().live.connection_sync_admission(),
        SyncAdmission::Ready(_)
    ));

    fake.publish(Some(1), Some(0xfffc), Instant::now());
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("−4 cents"), "{visible}");
    assert!(visible.contains("◀ Flat"), "{visible}");
    fake.publish(Some(u16::MAX - 2), Some(12), Instant::now());
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("No signal"), "{visible}");
    assert!(!visible.contains("+12 cents"), "{visible}");
    fake.publish(None, Some(12), Instant::now());
    assert!(stage_frame(&ctx, &mut app).contains("Waiting for reading"));
    fake.publish(Some(1), Some(12), Instant::now() - Duration::from_secs(2));
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("Waiting for reading"), "{visible}");
    assert!(!visible.contains("+12 cents"), "{visible}");

    escape(&ctx, &mut app);
    assert!(fake.stop.load(Ordering::Acquire));
    assert!(app.podxt().busy());
    fake.publish(Some(1), Some(33), Instant::now());
    escape(&ctx, &mut app);
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("Stopping tuner…"), "{visible}");
    assert!(!visible.contains("+33 cents"), "{visible}");
    assert!(app.podxt().busy());
    fake.complete(&app, Err("PODxt read cancelled.".into()));

    assert!(stage_frame(&ctx, &mut app).contains("Tuner stopped"));
    assert!(!app.podxt().busy());
    escape(&ctx, &mut app);
    let output = render_stage(&ctx, &mut app, vec![]);
    let fallback = ctx
        .memory(|memory| memory.focused())
        .expect("restore Tone Locker focus");
    let response = ctx.read_response(fallback).unwrap();
    let tone_locker = text_centers(&output, "Tone Locker");
    assert!(
        tone_locker.iter().any(|pos| response.rect.contains(*pos)),
        "restored {:?}, Tone Locker {tone_locker:?}",
        response.rect
    );
    assert!(!stage_frame(&ctx, &mut app).contains("Tuner stopped"));
    assert_eq!(app.podxt().live.device_requests, requests);
    assert_eq!(app.podxt().live.snapshot.tone, playing);
}

#[test]
fn tuner_close_requests_stop_and_cleanup_error_survives_attachment_loss() {
    let ctx = egui::Context::default();
    let (mut app, presence) = connected_fixture(&ctx);
    let requests = app.podxt().live.device_requests;
    let fake = FakeTuner::start(&mut app);
    stage_frame(&ctx, &mut app);
    click(&ctx, &mut app, popup_close(&ctx));
    assert!(
        fake.stop.load(Ordering::Acquire),
        "title Close requests stop"
    );
    assert!(stage_frame(&ctx, &mut app).contains("Stopping tuner…"));

    presence.attached.set(false);
    app.next_detection = Instant::now();
    fake.publish(Some(1), Some(21), Instant::now());
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("Stopping tuner…"), "{visible}");
    assert!(!visible.contains("+21 cents"), "{visible}");
    assert!(!app.podxt().live.connected());
    assert!(app.podxt().busy());

    fake.complete(
        &app,
        Err("PODxt read cancelled. Tuner off also failed: link lost. Cleanup also failed: release unconfirmed".into()),
    );
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("Tuner error"), "{visible}");
    assert!(
        visible.contains("Tuner off also failed: link lost"),
        "{visible}"
    );
    assert!(!app.podxt().busy());
    assert_eq!(app.podxt().live.device_requests, requests);
    click(&ctx, &mut app, popup_close(&ctx));
    assert!(!app.podxt().live.tuner.is_open());
}

#[test]
fn tuner_disconnect_cancels_old_worker_and_new_run_has_no_old_reading() {
    let ctx = egui::Context::default();
    let (mut app, presence) = connected_fixture(&ctx);
    let fake = FakeTuner::start(&mut app);
    fake.publish(Some(1), Some(17), Instant::now());
    assert!(stage_frame(&ctx, &mut app).contains("+17 cents"));

    presence.attached.set(false);
    app.next_detection = Instant::now();
    assert!(stage_frame(&ctx, &mut app).contains("Stopping tuner…"));
    assert!(fake.stop.load(Ordering::Acquire));
    fake.complete(&app, Ok(()));
    assert!(stage_frame(&ctx, &mut app).contains("PODxt Live disconnected"));
    escape(&ctx, &mut app);
    assert!(!app.podxt().live.tuner.is_open());

    presence.attached.set(true);
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);
    let next = FakeTuner::start(&mut app);
    let visible = stage_frame(&ctx, &mut app);
    assert!(visible.contains("Starting tuner…"), "{visible}");
    assert!(!visible.contains("+17 cents"), "{visible}");
    assert!(!next.stop.load(Ordering::Acquire));
    next.complete(&app, Ok(()));
    assert!(stage_frame(&ctx, &mut app).contains("Tuner stopped"));
}
