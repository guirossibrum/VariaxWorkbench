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
    assert!(!app.podxt().live.busy(), "identity alone starts no read");

    let cancel = Arc::new(AtomicBool::new(false));
    let (done, finish) = std::sync::mpsc::channel();
    let snapshot = Snapshot {
        tone: Some(tone(b"Old attachment")),
        ..Snapshot::default()
    };
    app.podxt_mut().live.worker = Some(Worker {
        command: ReadCommand::Follow,
        device: device.clone(),
        stop: Arc::clone(&cancel),
        snapshot: Arc::new(Mutex::new(snapshot)),
        thread: Some(std::thread::spawn(move || {
            finish.recv().map_err(|error| error.to_string())
        })),
    });
    assert!(app.podxt().busy());

    presence.attached.set(false);
    frame(&mut app);
    assert!(cancel.load(Ordering::Acquire));
    assert!(app.podxt().live.device.is_none());
    assert!(app.podxt().live.snapshot.tone.is_none());
    done.send(()).unwrap();
    let worker = app.podxt_mut().live.worker.as_mut().unwrap();
    worker.thread.take().unwrap().join().unwrap().unwrap();
    app.podxt_mut().live.worker = None;

    presence.attached.set(true);
    presence.identified.set(false);
    frame(&mut app);
    let new_device = app
        .tabs
        .entries
        .iter()
        .filter_map(|entry| entry.device.as_ref())
        .find(|tab| tab.connection == tabs::Connection::Present)
        .expect("new attachment");
    assert_ne!(new_device.device.key, device);
    assert_eq!(
        new_device.device.support,
        detection::DeviceSupport::IdentityPending
    );
}

struct FakeLock<'a>(&'a Cell<bool>);

impl Drop for FakeLock<'_> {
    fn drop(&mut self) {
        assert!(self.0.replace(false));
    }
}

struct FakeLink<'a> {
    held: &'a Cell<bool>,
    script: VecDeque<(PodxtRequest, Vec<u8>)>,
    reply: Option<Vec<u8>>,
    time: Duration,
    writes: &'a Cell<usize>,
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
        self.time += duration;
        Ok(self.reply.take())
    }

    fn elapsed(&self) -> Duration {
        self.time
    }

    fn pause(&mut self, duration: Duration) {
        self.time += duration;
    }
}

impl Drop for FakeLink<'_> {
    fn drop(&mut self) {
        assert!(self.held.get(), "port closed after releasing the rig lock");
        assert!(self.script.is_empty());
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
        || {
            assert!(!held.replace(true));
            Ok(FakeLock(&held))
        },
        |_| {
            assert!(held.get());
            Ok(FakeLink {
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
                time: Duration::ZERO,
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
        || Err::<(), _>("Rig busy".into()),
        |_| -> Result<FakeLink<'_>, _> { panic!("lock refusal must prevent opening the link") },
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
