use super::*;
use crate::editor::detection::{PresenceSource, SimulatedGuitar, SimulatedJob, SimulatedOutcome};
use crate::editor::tests::recording_frame;
use line6_devices::family::podxt::{EDIT_BUFFER_LEN, PodxtRequest, StoredTone};
use line6_devices::session::{Path as MidiPath, Request};
use std::{cell::Cell, collections::VecDeque, rc::Rc};
use variax_workbench::device_probe::{
    MetadataError, OsMetadataSource, PODXT_LIVE_USB, SoundCard, SyntheticSysfs, UsbDevice,
};

fn tone(name: &[u8]) -> EditBuffer {
    let mut frame = vec![0; EDIT_BUFFER_LEN];
    frame[..7].copy_from_slice(&[0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a]);
    frame[7..7 + name.len()].copy_from_slice(name);
    frame[167] = 0xf7;
    EditBuffer::parse(frame).unwrap()
}

#[derive(Clone, Default)]
struct Presence {
    attached: Rc<Cell<bool>>,
    identified: Rc<Cell<bool>>,
}

impl OsMetadataSource for Presence {
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
        let mut sysfs = SyntheticSysfs::new();
        if self.attached.get() {
            sysfs.add_device(
                "8-2.1",
                PODXT_LIVE_USB.0,
                PODXT_LIVE_USB.1,
                "PODxt Live",
                &[(0, Some("snd_usb_pod"))],
            );
        }
        sysfs.usb_devices()
    }

    fn sound_cards(&self) -> Result<Vec<SoundCard>, MetadataError> {
        panic!("automatic identity uses the injected result")
    }
}

impl PresenceSource for Presence {
    fn metadata(&self) -> &dyn OsMetadataSource {
        self
    }

    fn simulated_identity(&self, _: &SimulatedJob) -> Option<SimulatedOutcome> {
        self.identified.get().then_some(SimulatedOutcome::Finished {
            pod_firmware: Some("3.01"),
            guitar: SimulatedGuitar::Silent,
        })
    }
}

#[test]
fn shell_passes_verified_attachment_and_disconnect_cancels_the_worker() {
    let ctx = egui::Context::default();
    let mut app = Workbench::new(
        &ctx,
        EditorDefaults {
            artwork: PathBuf::new(),
            preferences: None,
            startup_panel: None,
        },
    );
    app.theme_polled = Instant::now();
    let presence = Presence::default();
    presence.attached.set(true);
    app.detection = Some(DetectionService::new(Box::new(presence.clone())));
    let frame = |app: &mut Workbench| {
        app.next_detection = Instant::now();
        recording_frame(&ctx, app, vec![]);
    };

    frame(&mut app);
    assert!(app.podxt().live.visible());
    assert!(app.podxt().live.device.is_none());
    let live = &mut app.podxt_mut().live;
    live.start(ReadCommand::Current, &ctx);
    assert!(!live.busy());

    presence.identified.set(true);
    frame(&mut app);
    let device = app
        .podxt()
        .live
        .device
        .clone()
        .expect("verified by detection");
    assert!(
        !app.podxt().live.busy(),
        "test build starts no native read after identity"
    );

    let cancel = Arc::new(AtomicBool::new(false));
    let (done, finish) = std::sync::mpsc::channel();
    let snapshot = Snapshot {
        tone: Some(tone(b"Old attachment")),
        ..Snapshot::default()
    };
    app.podxt_mut().live.worker = Some(Worker {
        acquired: None,
        command: ReadCommand::Follow,
        device: device.clone(),
        stop: Arc::clone(&cancel),
        snapshot: Arc::new(Mutex::new(snapshot)),
        thread: Some(std::thread::spawn(move || {
            finish.recv().map_err(|error| error.to_string())
        })),
    });
    assert!(app.podxt().busy());
    let old_reply = Arc::clone(&app.podxt().live.editor.reply_pending);
    old_reply.store(true, Ordering::Release);
    assert!(!app.podxt().live.device_ready());

    presence.attached.set(false);
    frame(&mut app);
    assert!(cancel.load(Ordering::Acquire));
    assert!(app.podxt().live.device.is_none());
    assert!(app.podxt().live.snapshot.tone.is_none());
    done.send(()).unwrap();
    let worker = app.podxt_mut().live.worker.as_mut().unwrap();
    worker.thread.take().unwrap().join().unwrap().unwrap();
    app.podxt_mut().live.worker = None;

    let tab_id = app.tabs.entries[0].id;
    presence.attached.set(true);
    presence.identified.set(false);
    frame(&mut app);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(app.tabs.entries[0].id, tab_id);
    let tab = app.tabs.entries[0].device.as_ref().unwrap();
    assert_eq!(tab.connection, tabs::Connection::Disconnected);
    assert_eq!(tab.device.key, device);

    presence.identified.set(true);
    frame(&mut app);
    assert_eq!(app.tabs.entries.len(), 1);
    assert_eq!(app.tabs.entries[0].id, tab_id);
    let tab = app.tabs.entries[0].device.as_ref().unwrap();
    assert_eq!(tab.connection, tabs::Connection::Present);
    assert_ne!(tab.device.key, device);
    assert!(old_reply.load(Ordering::Acquire));
    assert!(app.podxt().live.device_ready());
    assert!(!Arc::ptr_eq(
        &old_reply,
        &app.podxt().live.editor.reply_pending
    ));
    assert_eq!(
        tab.device.support,
        detection::DeviceSupport::Verified("3.01")
    );
    assert!(
        !app.podxt().live.busy(),
        "test build starts no native read after reattachment"
    );
}

pub(super) struct FakeLock<'a>(pub(super) &'a Cell<bool>);

impl Drop for FakeLock<'_> {
    fn drop(&mut self) {
        assert!(self.0.replace(false));
    }
}

pub(super) struct FakeLink<'a> {
    pub(super) held: &'a Cell<bool>,
    pub(super) script: VecDeque<(PodxtRequest, Vec<u8>)>,
    pub(super) reply: Option<Vec<u8>>,
    pub(super) time: &'a Cell<Duration>,
    pub(super) writes: &'a Cell<usize>,
}

impl Link for FakeLink<'_> {
    fn path(&self) -> MidiPath {
        MidiPath::PodxtMidi
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        assert!(self.held.get(), "request without the rig lock");
        let (request, response) = self.script.pop_front().expect("unexpected request");
        assert_eq!(bytes, request.bytes());
        self.writes.set(self.writes.get() + 1);
        self.reply = Some(response);
        Ok(())
    }

    fn read(&mut self, duration: Duration) -> Result<Option<Vec<u8>>, String> {
        assert!(self.held.get(), "read without the rig lock");
        self.time.set(self.time.get() + duration);
        Ok(self.reply.take())
    }

    fn elapsed(&self) -> Duration {
        self.time.get()
    }

    fn pause(&mut self, duration: Duration) {
        self.time.set(self.time.get() + duration);
    }
}

impl Drop for FakeLink<'_> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            assert!(self.held.get(), "port closed after releasing the rig lock");
            assert!(self.script.is_empty());
        }
    }
}

#[test]
fn transaction_locks_before_open_through_reply_and_port_close() {
    let held = Cell::new(false);
    let writes = Cell::new(0);
    let stop = AtomicBool::new(false);
    transaction(
        ReadCommand::Current,
        &stop,
        &AtomicBool::new(false),
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_, run| {
            assert!(held.get());
            run(&mut FakeLink {
                held: &held,
                writes: &writes,
                script: VecDeque::from([
                    (
                        PodxtRequest::Identity,
                        variax_workbench::identity_stage::bridge_reply(),
                    ),
                    (PodxtRequest::EditBuffer, tone(b"Offline fixture").raw),
                ]),
                reply: None,
                time: &Cell::new(Duration::ZERO),
            })
        },
        |update| {
            assert!(held.get());
            assert!(
                matches!(update, ReadUpdate::Current(tone) if tone.name() == "Offline fixture")
            );
        },
    )
    .unwrap();
    assert_eq!(writes.get(), 2);
    assert!(!held.get());

    let busy = transaction(
        ReadCommand::Current,
        &stop,
        &AtomicBool::new(false),
        || Err::<(), _>("Rig busy".into()),
        |_, _| panic!("lock refusal must prevent opening the link"),
        |_| panic!("lock refusal must prevent updates"),
    );
    assert_eq!(busy, Err("Rig busy".into()));
}

#[test]
fn notifications_update_the_current_snapshot_and_saved_preview_independently() {
    let mut snapshot = Snapshot::default();
    let now = Instant::now();
    let slot = Location::new(BankKind::Tones, 17).unwrap();
    snapshot.apply(ReadUpdate::Current(tone(b"Playing")), now);
    snapshot.apply(
        ReadUpdate::Controller {
            number: 11,
            value: 12,
        },
        now,
    );
    snapshot.apply(
        ReadUpdate::Controller {
            number: 13,
            value: 90,
        },
        now,
    );
    snapshot.apply(ReadUpdate::Clip, now);
    snapshot.apply(ReadUpdate::Tap, now);
    snapshot.apply(ReadUpdate::Channel(slot), now);
    snapshot.apply(
        ReadUpdate::Stored(StoredTone {
            location: slot,
            tone: tone(b"Saved"),
        }),
        now,
    );
    assert_eq!(snapshot.library[&slot.address()].name(), "Saved");
    let current = snapshot.tone.unwrap();
    assert_eq!(current.name(), "Playing");
    assert_eq!(current.cc(12), 12);
    assert_eq!(current.cc(13), 90);
    assert_eq!(snapshot.channel, Some(slot));
    assert_eq!(snapshot.clip, Some(now + PULSE));
    assert_eq!(snapshot.tap, Some(now + PULSE));
}

fn render_stage(
    ctx: &egui::Context,
    app: &mut Workbench,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            events,
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 820.0))),
            ..Default::default()
        },
        |ctx| app.render(ctx),
    )
}

fn stage_frame(ctx: &egui::Context, app: &mut Workbench) -> String {
    fn collect(shape: &egui::epaint::Shape, text: &mut String) {
        match shape {
            egui::epaint::Shape::Text(shape) => {
                text.push_str(shape.galley.text());
                text.push('\n');
            }
            egui::epaint::Shape::Vec(shapes) => {
                shapes.iter().for_each(|shape| collect(shape, text))
            }
            _ => {}
        }
    }
    let mut output = String::new();
    for _ in 0..2 {
        let frame = render_stage(ctx, app, vec![]);
        output.clear();
        for shape in &frame.shapes {
            collect(&shape.shape, &mut output);
        }
    }
    output
}

fn valid_dump(name: &str) -> EditBuffer {
    let mut tone = pod_preset::PodTone::new_podxt_tone(name).unwrap();
    tone.set_table_model(
        "AMP_MODEL",
        131092,
        0,
        false,
        &pod_preset::UnknownPodDefaults,
    )
    .unwrap();
    tone.set_table_model(
        "CAB_MODEL",
        0x0102_0007,
        0,
        false,
        &pod_preset::UnknownPodDefaults,
    )
    .unwrap();
    let payload = tone.to_podxt_payload().unwrap();
    let mut raw = vec![0xf0, 0, 1, 0x0c, 3, 0x74, 0x0a];
    raw.extend(payload);
    raw.push(0xf7);
    EditBuffer::parse(raw).unwrap()
}

fn complete_backup(root: &std::path::Path, active: &str, saved: &str) -> PathBuf {
    let mut folder = line6_devices::backup::BackupFolder::create(root, "podxt").unwrap();
    folder.record("PODxt Live library backup at 2024-01-01 00:00:00 UTC.".into());
    folder
        .file(
            "identity.syx",
            &variax_workbench::identity_stage::bridge_reply(),
        )
        .unwrap();
    folder
        .file("current-before.syx", &valid_dump("Before backup").raw)
        .unwrap();
    folder
        .file("current-after.syx", &valid_dump(active).raw)
        .unwrap();
    for (kind, name) in BankKind::ALL
        .into_iter()
        .zip([saved, "Backup Amp", "Backup FX"])
    {
        let dump = valid_dump(name);
        for slot in Location::all(&[kind]) {
            folder
                .file(&podxt::backup::podxt_entry_name(slot), &dump.raw)
                .unwrap();
        }
    }
    folder.finish(true).unwrap()
}

fn startup_app(ctx: &egui::Context, preferences: Option<PathBuf>) -> Workbench {
    let mut app = Workbench::new(
        ctx,
        EditorDefaults {
            artwork: PathBuf::new(),
            preferences,
            startup_panel: None,
        },
    );
    app.theme_polled = Instant::now();
    app.detection = None;
    app
}

fn connected_fixture(ctx: &egui::Context) -> (Workbench, Presence) {
    let mut app = startup_app(ctx, None);
    let presence = Presence::default();
    presence.attached.set(true);
    presence.identified.set(true);
    app.detection = Some(DetectionService::new(Box::new(presence.clone())));
    stage_frame(ctx, &mut app);
    let editor = app.podxt_mut();
    editor.table_view_in_test = true;
    assert!(editor.live.connected());
    let slot = Location::new(BankKind::Tones, 17).unwrap();
    for update in [
        ReadUpdate::Channel(slot),
        ReadUpdate::Current(valid_dump("Stage Current")),
        ReadUpdate::Stored(StoredTone {
            location: slot,
            tone: valid_dump("Saved 5B"),
        }),
    ] {
        editor.live.snapshot.apply(update, Instant::now());
    }
    (app, presence)
}

#[test]
fn startup_is_scheduled_once_per_verified_attachment_and_cancelled_on_detach() {
    let ctx = egui::Context::default();
    let (app, _) = connected_fixture(&ctx);
    let device = app.tabs.entries[0].device.as_ref();
    let mut panel = LivePanel::default();
    assert_eq!(panel.next_startup(), None);
    assert!(panel.set_device(device));
    assert_eq!(panel.next_startup(), Some(Startup::Current));
    assert!(!panel.set_device(device));
    assert_eq!(panel.next_startup(), Some(Startup::Tones));
    assert_eq!(panel.next_startup(), None);

    panel.set_device(None);
    assert!(panel.set_device(device));
    assert_eq!(panel.next_startup(), Some(Startup::Current));
    panel.set_device(None);
    assert_eq!(panel.next_startup(), None);
}

#[test]
fn connected_stage_uses_current_tone_channel_and_librarian_and_follows_updates() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let visible = stage_frame(&ctx, &mut app);
    for label in [
        "Connected",
        "Channel 5B",
        "Stage Current",
        "Saved 5B",
        "Live editing",
        "Tuner",
        "Tone Locker",
    ] {
        assert!(visible.contains(label), "missing {label}: {visible}");
    }
    for hidden in [
        "Advanced",
        "Details",
        "Read installed packs",
        "Read globals",
        "Resend whole current tone",
    ] {
        assert!(!visible.contains(hidden), "{visible}");
    }
    assert!(!app.podxt().dirty());
    let slot = Location::new(BankKind::Tones, 18).unwrap();
    let now = Instant::now();
    let snapshot = &mut app.podxt_mut().live.snapshot;
    snapshot.apply(ReadUpdate::Channel(slot), now);
    snapshot.apply(ReadUpdate::Current(valid_dump("Followed Tone")), now);
    let visible = stage_frame(&ctx, &mut app);
    assert!(
        visible.contains("Channel 5C") && visible.contains("Followed Tone"),
        "{visible}"
    );
}

#[test]
fn followed_tuner_mode_is_visible_without_changing_the_playing_tone() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let now = Instant::now();
    let before = app.podxt().live.snapshot.tone.clone();
    app.podxt_mut()
        .live
        .snapshot
        .apply(ReadUpdate::TunerMode(true), now);
    assert!(stage_frame(&ctx, &mut app).contains("Tuner active on POD"));
    assert_eq!(app.podxt().live.snapshot.tone, before);
    app.podxt_mut()
        .live
        .snapshot
        .apply(ReadUpdate::TunerMode(false), now);
    assert!(!stage_frame(&ctx, &mut app).contains("Tuner active on POD"));
    assert_eq!(app.podxt().live.snapshot.tone, before);
    assert!(!app.podxt().dirty());
}

#[test]
fn playing_view_does_not_substitute_a_file_when_the_dump_is_unreadable() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let editor = app.podxt_mut();
    let file = pod_preset::PodTone::new_podxt_tone("User").unwrap();
    editor.import_pod_document(file.into_bytes());
    let slot = Location::new(BankKind::Tones, 1).unwrap();
    let mut current = valid_dump("Class A-30 TB");
    current.raw[6] = 2;
    editor
        .live
        .snapshot
        .apply(ReadUpdate::Channel(slot), Instant::now());
    editor
        .live
        .snapshot
        .apply(ReadUpdate::Current(current), Instant::now());
    let visible = stage_frame(&ctx, &mut app);
    assert!(
        visible.contains("Channel 1B") && visible.contains("Class A-30 TB"),
        "{visible}"
    );
    assert!(
        visible.contains("edit buffer could not be decoded"),
        "{visible}"
    );
    assert!(
        !visible.contains("Loaded User") && !visible.contains("Plexi 45"),
        "{visible}"
    );

    app.podxt_mut().live.snapshot.apply(
        ReadUpdate::Current(valid_dump("Class A-30 TB")),
        Instant::now(),
    );
    let visible = stage_frame(&ctx, &mut app);
    assert!(
        visible.contains("Class A-30 TB") && visible.contains("Amp ·"),
        "{visible}"
    );
    assert!(!visible.contains("Loaded User"), "{visible}");
}

#[test]
fn live_library_marks_unsynchronized_playing_tone_but_ignores_pedal_motion() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let slot = Location::new(BankKind::Tones, 17).unwrap();
    let mut changed = valid_dump("Saved 5B");
    changed.raw[39 + 7] ^= 1;
    app.podxt_mut().live.snapshot.tone = Some(changed.clone());
    assert!(!app.podxt().live.channel_modified(slot));
    changed.raw[39 + 13] ^= 1;
    app.podxt_mut().live.snapshot.tone = Some(changed);
    assert!(app.podxt().live.channel_modified(slot));
    let visible = stage_frame(&ctx, &mut app);
    assert!(
        visible.contains("Unsynchronized") && visible.contains("5B !  Saved 5B"),
        "{visible}"
    );
}
#[test]
fn disconnected_stage_loads_backup_and_no_backup_shows_labelled_default() {
    let ctx = egui::Context::default();
    let (mut app, presence) = connected_fixture(&ctx);
    presence.attached.set(false);
    app.next_detection = Instant::now();
    let root = tempfile::tempdir().unwrap();
    let preferences = root.path().join("preferences.json");
    let complete = complete_backup(root.path(), "Latest Backup", "Backup Channel");
    pod_preset::remember_podxt_backup(&preferences, &complete).unwrap();
    app.podxt_mut().initialize_stage(Some(preferences));
    let visible = stage_frame(&ctx, &mut app);
    for label in [
        "Disconnected",
        "Latest Backup",
        "Library backup",
        "Backup Channel",
        "Amp ·",
    ] {
        assert!(visible.contains(label), "missing {label}: {visible}");
    }
    assert!(!visible.contains("Stage Current"));
    assert!(!app.podxt().dirty());

    let ctx = egui::Context::default();
    let mut app = startup_app(&ctx, None);
    app.new_tab(&super::super::FAMILY);
    app.podxt_mut().table_view_in_test = true;
    let visible = stage_frame(&ctx, &mut app);
    for label in ["Default Tone", "offline", "Amp ·", "Library"] {
        assert!(visible.contains(label), "missing {label}: {visible}");
    }
    assert!(!visible.contains("Open a tone or create a new one"));
    assert!(!app.podxt().dirty());
}

mod stage_acceptance;

pub(super) mod read_recovery;
