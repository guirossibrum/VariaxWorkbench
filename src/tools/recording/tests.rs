use super::super::podxt_panel::tests::sample_pod_tone;
use super::super::tests::{app, click_prompt, click_toolbar_tools, recording_frame};
use super::*;

fn click_recording(context: &egui::Context, app: &mut Workbench, id: &'static str) {
    recording_frame(context, app, vec![]);
    let pos = app.recording_buttons[id].center();
    recording_frame(
        context,
        app,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    recording_frame(
        context,
        app,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
}

#[test]
fn offline_test_recording_window_does_not_start_a_device_or_a_worker() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("offline-test-take.wav");
    app.dialogs = Box::new(FakeDialogs {
        save: [Some(path.clone())].into(),
        ..Default::default()
    });
    click_toolbar_tools(&context, &mut app);
    click_recording(&context, &mut app, "open-recording");
    assert!(app.recording_open);
    assert!(app.recording_visible.contains(recording::OFFLINE_BANNER));
    assert!(app.recording_visible.contains(
        variax_workbench::offline_device_notice().unwrap_or(recording::DEVICE_CAPTURE_LOCKED)
    ));
    assert!(app.recording_visible.contains(recording::NO_AUTO_START));
    assert!(
        app.recording_visible
            .contains("Native live locked; external input unsupported.")
    );
    assert!(app.recording_visible.contains(recording::PHASE_IDLE));
    assert!(app.recording_visible.contains("not a live route change"));
    assert!(app.variax().read_worker.is_none());
    assert!(!app.variax().is_reading());
    assert_eq!(app.recording_capture_attempts, 0);
    assert_eq!(app.recorder.phase(), recording::Phase::Idle);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    click_recording(&context, &mut app, "start");
    assert_eq!(app.recorder.phase(), recording::Phase::Idle);
    assert!(app.variax().read_worker.is_none());
    click_recording(&context, &mut app, "route-clean");
    click_recording(&context, &mut app, "inspect-native-plan");
    #[cfg(target_os = "linux")]
    {
        assert!(app.recording_visible.contains("Prepared declaration only"));
        assert!(app.recording_visible.contains("78125/2 Hz"));
        assert!(app.recording_visible.contains("2 channels mapped 0→L, 1→R"));
    }
    #[cfg(not(target_os = "linux"))]
    {
        assert!(app.recording_visible.contains("UnverifiedPlatform"));
    }
    assert_eq!(app.recorder.phase(), recording::Phase::Idle);
    assert_eq!(
        app.recorder.route(),
        Some(variax_workbench::pod_audio::RecordRoute::SendCleanGuitar)
    );
    assert!(!app.recorder.live_monitor_changed());
    assert!(!app.recorder.live_route_changed());
    assert_eq!(app.recording_capture_attempts, 0);
    click_recording(&context, &mut app, "check-external-plan");
    assert!(
        app.recording_visible
            .contains("no independently admitted interface record")
    );
    assert_eq!(app.recorder.phase(), recording::Phase::Idle);
    assert_eq!(app.recording_capture_attempts, 0);
    click_recording(&context, &mut app, "clock-44100");
    click_recording(&context, &mut app, "input-synthetic");
    assert!(app.recording_visible.contains("not a live route change"));
    assert!(!app.recorder.live_monitor_changed());
    assert!(!app.recorder.live_route_changed());
    click_recording(&context, &mut app, "start");
    assert_eq!(app.recorder.phase(), recording::Phase::Recording);
    assert_eq!(app.recording_capture_attempts, 0);
    assert!(app.variax().read_worker.is_none());
    assert!(!app.recorder.device_opened());
    click_recording(&context, &mut app, "feed");
    assert!(app.recording_visible.contains("Left peak 1000"));
    assert!(app.recording_visible.contains("Right peak 2000"));
    click_recording(&context, &mut app, "stop");
    click_recording(&context, &mut app, "save");
    app.poll_picker(&context);
    let wav = std::fs::read(&path).unwrap();
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 44_100);
    let sidecar = std::fs::read_to_string(format!("{}.recording.json", path.display())).unwrap();
    assert!(sidecar.contains("\"device_recording\": false"));
    assert!(sidecar.contains("\"sample_rate_hz\": 44100"));
    assert!(!sidecar.contains("39062"));
    assert!(!sidecar.contains("78125"));
    assert!(app.variax().read_worker.is_none());
    app.variax_mut().start_read();
    assert!(app.variax().read_worker.is_none());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn rendered_simulated_callback_workflow_is_synthetic_and_offline() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("simulated-callback.wav");
    app.dialogs = Box::new(FakeDialogs {
        save: [Some(path.clone())].into(),
        ..Default::default()
    });
    click_toolbar_tools(&context, &mut app);
    click_recording(&context, &mut app, "open-recording");
    click_recording(&context, &mut app, "route-clean");
    click_recording(&context, &mut app, "clock-48000");
    click_recording(&context, &mut app, "input-simulated-callback");
    click_recording(&context, &mut app, "start");
    assert_eq!(app.recorder.phase(), recording::Phase::Recording);
    assert!(app.recording_visible.contains("No hardware connection"));
    assert!(
        app.recording_visible
            .contains("Generated software test data")
    );
    assert_eq!(app.recording_capture_attempts, 0);
    assert!(app.variax().read_worker.is_none());
    assert!(!app.recorder.device_opened());
    click_recording(&context, &mut app, "feed");
    click_recording(&context, &mut app, "stop");
    click_recording(&context, &mut app, "save");
    app.poll_picker(&context);
    let wav = std::fs::read(&path).unwrap();
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 48_000);
    assert_eq!(&wav[44..48], &[0xe8, 0x03, 0x30, 0xf8]);
    assert_eq!(app.recording_capture_attempts, 0);
    assert!(app.variax().read_worker.is_none());
    assert!(!app.recorder.device_opened());
}

pub(in crate::editor) fn prepare_quit_take(app: &mut Workbench, clock: ClockSelection) {
    app.recorder
        .select_route(RecordRoute::SendCleanGuitar)
        .unwrap();
    app.recorder.select_clock(clock).unwrap();
    app.recorder
        .select_input(recording::InputSelection::Synthetic)
        .unwrap();
    app.recorder.start().unwrap();
    app.recorder.feed().unwrap();
}

fn prepare_quit_callback_take(app: &mut Workbench, clock: ClockSelection) {
    app.recorder
        .select_route(RecordRoute::SendCleanGuitar)
        .unwrap();
    app.recorder.select_clock(clock).unwrap();
    app.recorder
        .select_input(recording::InputSelection::SimulatedCallback)
        .unwrap();
    app.recorder.start().unwrap();
    app.recorder.feed().unwrap();
}

#[test]
fn recording_quit_cancel_and_failed_save_retain_audio_until_success() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("blocked.wav");
    let good = dir.path().join("saved.wav");
    std::fs::write(&blocked, b"keep").unwrap();
    app.dialogs = Box::new(FakeDialogs {
        save: [None, Some(blocked.clone()), Some(good.clone())].into(),
        ..Default::default()
    });
    prepare_quit_take(&mut app, ClockSelection::Integer(44_100));
    let levels = app.recorder.levels();
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert_eq!(app.recorder.phase(), recording::Phase::Recording);
    assert!(app.recorder.has_unsaved_take());
    assert!(!app.permit_close);
    app.request_close(&context);
    click_prompt(&context, &mut app, "save"); // explicit stop, cancelled dialog
    app.poll_picker(&context);
    assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));
    assert!(app.recorder.has_unsaved_take());
    assert_eq!(app.recorder.levels(), levels);
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "save"); // refused overwrite
    app.poll_picker(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));
    assert!(app.recorder.has_unsaved_take());
    assert_eq!(std::fs::read(&blocked).unwrap(), b"keep");
    assert_eq!(app.recorder.levels(), levels);
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "save");
    app.poll_picker(&context);
    assert!(!app.recorder.has_unsaved_take());
    assert!(app.test_pending().is_none());
    assert!(app.permit_close);
    let decoded = recording::parse_wav(&std::fs::read(&good).unwrap()).unwrap();
    assert!(!decoded.data.is_empty());
    assert!(
        decoded
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .all(|frame| *frame == [0xe8, 3, 0x30, 0xf8])
    );
    assert_eq!(app.recording_capture_attempts, 0);
    assert!(app.variax().read_worker.is_none());
}

#[test]
fn native_clock_quit_cancelled_save_retains_the_take_until_explicit_discard() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    prepare_quit_take(&mut app, ClockSelection::NativePodxt);
    app.dialogs = Box::new(FakeDialogs {
        save: [None].into(),
        ..Default::default()
    });
    app.recorder.stop().unwrap();
    app.request_close(&context);
    click_prompt(&context, &mut app, "save"); // cancelled AIFF picker
    app.poll_picker(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));
    assert!(app.recorder.has_unsaved_take());
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "cancel");
    assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
    assert!(app.recorder.has_unsaved_take());
    app.request_close(&context);
    click_prompt(&context, &mut app, "discard");
    assert!(!app.recorder.has_unsaved_take());
    assert_eq!(app.recorder.phase(), recording::Phase::Idle);
    assert!(app.test_pending().is_none());
    assert!(app.permit_close);
    assert!(!app.recorder.device_opened());
}

#[test]
fn native_recording_quit_saves_an_exact_clock_file_and_retains_its_source() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("native.aiff");
    app.dialogs = Box::new(FakeDialogs {
        save: [Some(path.clone())].into(),
        ..Default::default()
    });
    prepare_quit_take(&mut app, ClockSelection::NativePodxt);
    app.request_close(&context);
    click_prompt(&context, &mut app, "save");
    app.poll_picker(&context);
    assert!(app.test_pending().is_none());
    assert!(app.permit_close);
    assert!(!app.recorder.has_unsaved_take());
    assert_eq!(app.recorder.phase(), recording::Phase::Stopped);
    let bytes = std::fs::read(&path).unwrap();
    let source = recording::aiff::decode(&bytes).unwrap();
    assert_eq!(source.len(), 256 * 6);
    assert!(
        source
            .as_chunks::<6>()
            .0
            .iter()
            .all(|frame| *frame == [0xe8, 3, 0, 0x30, 0xf8, 0xff])
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(recording::sidecar_path(&path).unwrap()).unwrap())
            .unwrap();
    assert_eq!(metadata["native_clock_exported"], true);
    assert_eq!(metadata["source_clock"]["denominator"], 2);
    app.permit_close = false;
    app.recorder.cancel().unwrap();
    prepare_quit_take(&mut app, ClockSelection::NativePodxt);
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));
    assert!(!app.permit_close);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(app.variax().read_worker.is_none());
    assert_eq!(app.recording_capture_attempts, 0);
}

#[test]
fn guitar_and_pod_quit_continuations_cannot_skip_an_unsaved_take() {
    let (context, mut app) = app();
    context.style_mut(|style| style.animation_time = 0.0);
    let dir = tempfile::tempdir().unwrap();
    let saved = dir.path().join("guitar.vwm");
    app.dialogs = Box::new(FakeDialogs {
        save: [Some(saved.clone())].into(),
        ..Default::default()
    });
    app.variax_mut()
        .change(|preset| preset.rename("Guitar changed"));
    let index = app
        .podxt_mut()
        .pod_library
        .import_bytes(sample_pod_tone())
        .unwrap();
    app.podxt_mut().pod_selected = Some(index);
    app.podxt_mut().pod_name = "POD changed".into();
    app.podxt_mut().apply_pod_name();
    prepare_quit_callback_take(&mut app, ClockSelection::Integer(48_000));
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::Quit)));
    click_prompt(&context, &mut app, "save");
    app.poll_picker(&context);
    assert!(saved.exists());
    assert!(!app.variax().dirty());
    assert!(matches!(app.test_pending(), Some(Pending::QuitPod)));
    assert!(!app.permit_close);
    click_prompt(&context, &mut app, "discard");
    assert!(!app.podxt().pod_library.any_dirty());
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));
    assert!(app.recorder.has_unsaved_take());
    assert!(!app.permit_close);
    // Background editors cannot alter the take while this decision is pending.
    app.recording_open = true;
    click_recording(&context, &mut app, "cancel");
    assert!(app.recorder.has_unsaved_take());
    click_prompt(&context, &mut app, "cancel");
    assert!(app.test_pending().is_none());
    assert!(app.recorder.has_unsaved_take());
    app.request_close(&context);
    assert!(matches!(app.test_pending(), Some(Pending::QuitRecording)));
    click_prompt(&context, &mut app, "discard");
    assert!(app.permit_close);
    assert!(!app.recorder.has_unsaved_take());
    assert!(app.variax().read_worker.is_none());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}

#[test]
fn load_recording_wav_refuses_a_nonregular_path_without_opening_it() {
    let (context, mut app) = app();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("not-a-regular-wav");
    std::fs::create_dir(&path).unwrap();
    app.dialogs = Box::new(FakeDialogs {
        wav: [Some(path)].into(),
        ..Default::default()
    });
    app.load_recording_wav(&context);
    app.poll_picker(&context);
    assert!(app.recorder.message().contains("regular file"));
    assert!(app.recorder.message().contains("not opened"));
    assert!(app.variax().read_worker.is_none());
    assert!(!app.variax().is_reading());
    assert_eq!(app.recording_capture_attempts, 0);
    assert!(!app.recorder.device_opened());
    const {
        assert!(!hardware_gates::is_locked(Gate::CurrentSettingsRead));
    }
}
