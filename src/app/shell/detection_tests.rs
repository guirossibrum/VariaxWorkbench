use super::super::detection::{
    AttachmentKey, DeviceSupport, PresenceSource, SimulatedGuitar, SimulatedJob, SimulatedOutcome,
};
use super::super::tabs::{Connection, DeviceTab};
use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use variax_workbench::device_probe::{
    MetadataError, OsMetadataSource, PODXT_LIVE_USB, SoundCard, SyntheticSysfs, UsbDevice,
    UsbInterface,
};
use variax_workbench::pod_hd500x::{USB_PRODUCT_ID, USB_VENDOR_ID};

#[derive(Default)]
struct FakeSnapshot {
    usb: Vec<UsbDevice>,
    results: std::collections::BTreeMap<AttachmentKey, SimulatedOutcome>,
    completed: Vec<SimulatedJob>,
    fail: bool,
    reads: usize,
}

#[derive(Clone, Default)]
struct FakePresence(Rc<RefCell<FakeSnapshot>>);

impl OsMetadataSource for FakePresence {
    fn usb_devices(&self) -> Result<Vec<UsbDevice>, MetadataError> {
        let mut snapshot = self.0.borrow_mut();
        snapshot.reads += 1;
        if snapshot.fail {
            Err(MetadataError::Unreadable("injected USB metadata".into()))
        } else {
            Ok(snapshot.usb.clone())
        }
    }

    fn sound_cards(&self) -> Result<Vec<SoundCard>, MetadataError> {
        panic!("presence checks should only use cached USB data")
    }
}

impl PresenceSource for FakePresence {
    fn metadata(&self) -> &dyn OsMetadataSource {
        self
    }

    fn simulated_identity(&self, job: &SimulatedJob) -> Option<SimulatedOutcome> {
        let mut snapshot = self.0.borrow_mut();
        let outcome = snapshot.results.remove(&job.attachment)?;
        snapshot.completed.push(job.clone());
        Some(outcome)
    }
}

impl FakePresence {
    fn usb(&self, pod: bool, hd: bool) {
        let mut usb = SyntheticSysfs::new();
        if pod {
            usb.add_device(
                "8-2.1",
                PODXT_LIVE_USB.0,
                PODXT_LIVE_USB.1,
                "PODxt Live",
                &[(0, Some("snd_usb_pod"))],
            );
        }
        if hd {
            usb.add_device("8-3", USB_VENDOR_ID, USB_PRODUCT_ID, "HD500X", &[]);
        }
        self.0.borrow_mut().usb = usb.usb_devices().unwrap();
    }

    fn guitar(&self, parent: AttachmentKey) {
        self.finish(parent, SimulatedGuitar::Verified("3.10"));
    }

    fn finish(&self, parent: AttachmentKey, guitar: SimulatedGuitar) {
        self.0.borrow_mut().results.insert(
            parent,
            SimulatedOutcome::Finished {
                pod_firmware: Some("3.01"),
                guitar,
            },
        );
    }
}

fn fixture() -> (egui::Context, Workbench, FakePresence) {
    let ctx = egui::Context::default();
    let mut app = Workbench::new(
        &ctx,
        EditorDefaults {
            artwork: PathBuf::new(),
            preferences: None,
            startup_panel: None,
        },
    );
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.theme_polled = Instant::now();
    let source = FakePresence::default();
    app.detection = Some(DetectionService::new(Box::new(source.clone())));
    (ctx, app, source)
}

fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 820.0))),
            events,
            ..Default::default()
        },
        |ctx| app.render(ctx),
    )
}

fn tick(ctx: &egui::Context, app: &mut Workbench) {
    app.next_detection = Instant::now();
    frame(ctx, app, vec![]);
}

fn entries(app: &Workbench) -> Vec<(&'static str, TabId, &DeviceTab)> {
    app.tabs
        .entries
        .iter()
        .filter_map(|entry| {
            entry
                .device
                .as_ref()
                .map(|device| (entry.family.info().id, entry.id, device))
        })
        .collect()
}

fn detection_button(
    ctx: &egui::Context,
    app: &mut Workbench,
    button: &'static str,
) -> egui::FullOutput {
    frame(ctx, app, vec![]);
    let pos = app.detection_buttons[button].center();
    let event = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(ctx, app, vec![egui::Event::PointerMoved(pos), event(true)]);
    frame(ctx, app, vec![event(false)])
}

fn has_label(shapes: &[egui::epaint::ClippedShape], expected: &str) -> bool {
    fn matches(shape: &egui::Shape, expected: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.text() == expected,
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| matches(shape, expected)),
            _ => false,
        }
    }
    shapes.iter().any(|shape| matches(&shape.shape, expected))
}

#[test]
fn usb_hotplug_opens_one_pod_tab_and_detaches_it_on_removal() {
    let (ctx, mut app, source) = fixture();
    tick(&ctx, &mut app);
    assert!(app.tabs.entries.is_empty());

    source.usb(true, false);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs.len(), 1);
    let (family, id, device) = tabs[0];
    assert_eq!(family, "podxt");
    assert_eq!(device.device.support, DeviceSupport::IdentityPending);
    assert_eq!(device.status(), "USB detected · identity pending");
    let first_key = device.device.key.clone();

    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[0].1, id);
    assert_eq!(entries(&app).len(), 1);

    source.usb(false, false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[0].2.connection, Connection::Disconnected);

    source.usb(true, false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 2);
    assert_eq!(entries(&app)[0].2.connection, Connection::Disconnected);
    assert_ne!(entries(&app)[1].2.device.key, first_key);
    app.close_tab(id, &ctx);
    assert_eq!(entries(&app).len(), 1);
    assert_eq!(entries(&app)[0].2.connection, Connection::Present);
}

#[test]
fn verified_identity_opens_variax_without_duplicate_or_stealing_focus() {
    let (ctx, mut app, source) = fixture();
    app.new_tab(registry::LEGACY_STARTUP);
    let offline = app.tabs.active;
    source.usb(true, false);
    tick(&ctx, &mut app);
    let parent = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(parent);
    tick(&ctx, &mut app);

    let tabs = entries(&app);
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[1].0, "variax");
    assert_eq!(tabs[1].2.status(), "Identity verified · firmware 3.10");
    assert_eq!(app.tabs.active, offline);
    assert_eq!(app.tabs.entries.len(), 3);
    let id = tabs[1].1;
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].1, id);
}

#[test]
fn detect_defaults_to_no_and_scans_for_hd500x_only_after_confirmation() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, true);
    tick(&ctx, &mut app);
    source.guitar(entries(&app)[0].2.device.key.attachment.clone());
    tick(&ctx, &mut app);
    assert_eq!(
        entries(&app).iter().map(|tab| tab.0).collect::<Vec<_>>(),
        ["podxt", "variax"]
    );

    detection_button(&ctx, &mut app, "detect");
    assert!(app.detect_confirmation.is_some());
    let output = frame(&ctx, &mut app, vec![]);
    assert!(has_label(
        &output.shapes,
        "Tested on Linux: PODxt Live 3.01 and Variax 600 3.10."
    ));
    let reads = source.0.borrow().reads;
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(app.detect_confirmation.is_none());
    assert_eq!(source.0.borrow().reads, reads);

    detection_button(&ctx, &mut app, "detect");
    detection_button(&ctx, &mut app, "yes");
    assert!(app.detect_confirmation.is_none());
    assert_eq!(source.0.borrow().reads, reads + 1);
    let tabs = entries(&app);
    assert_eq!(
        tabs.iter().map(|tab| tab.0).collect::<Vec<_>>(),
        ["podxt", "variax", "hd500x"]
    );
    assert_eq!(tabs[2].2.status(), "Live connection not supported yet");
    assert_eq!(app.tabs.active, Some(tabs[0].1));
}

#[test]
fn hd500x_replug_requires_a_new_confirmed_scan() {
    let (ctx, mut app, source) = fixture();
    source.usb(false, true);
    tick(&ctx, &mut app);
    assert!(app.tabs.entries.is_empty());
    detection_button(&ctx, &mut app, "start-detect");
    detection_button(&ctx, &mut app, "yes");
    let id = entries(&app)[0].1;
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[0].1, id);
    source.usb(false, false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[0].2.connection, Connection::Disconnected);
    app.close_tab(id, &ctx);
    assert!(app.tabs.entries.is_empty());
    source.usb(false, true);
    tick(&ctx, &mut app);
    assert!(app.tabs.entries.is_empty());
    detection_button(&ctx, &mut app, "start-detect");
    detection_button(&ctx, &mut app, "yes");
    assert_ne!(entries(&app)[0].1, id);
}

#[test]
fn dirty_disconnect_keeps_the_editor_until_the_user_closes_it() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let pod = entries(&app)[0].1;
    let old_parent = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(old_parent.clone());
    tick(&ctx, &mut app);
    let guitar = entries(&app)[1].1;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
    app.with_tab(guitar, |entry, _| {
        let editor = entry.editor.as_mut() as &mut dyn std::any::Any;
        let guitar = editor.downcast_mut::<VariaxEditor>().unwrap();
        guitar.load(&path);
        guitar.change(|preset| preset.rename("Unsaved guitar"));
    });
    app.tabs.active = Some(guitar);

    source.usb(false, false);
    tick(&ctx, &mut app);
    assert_eq!(app.tabs.active, Some(guitar));
    assert_eq!(entries(&app).len(), 2);
    app.close_tab(pod, &ctx);
    assert_eq!(entries(&app).len(), 1);
    assert_eq!(entries(&app)[0].2.connection, Connection::Disconnected);
    assert!(app.variax().dirty());

    source.usb(true, false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 2);
    let new_parent = entries(&app)[1].2.device.key.attachment.clone();
    assert_ne!(old_parent, new_parent);
    source.guitar(new_parent);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs.len(), 3);
    assert_eq!(tabs[0].1, guitar);
    assert_eq!(tabs[0].2.connection, Connection::Disconnected);
    assert_eq!(tabs[2].2.connection, Connection::Present);
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Unsaved guitar"
    );

    app.close_tab(guitar, &ctx);
    assert_eq!(app.closing_tab, Some(guitar));
    super::super::tests::click_prompt(&ctx, &mut app, "discard");
    assert!(app.tabs.index(guitar).is_none());
}

#[test]
fn unreadable_or_ambiguous_snapshots_keep_connected_tabs_until_a_valid_snapshot() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let id = entries(&app)[0].1;

    source.0.borrow_mut().fail = true;
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[0].2.connection, Connection::Uncertain);
    assert!(app.detection_status.is_some());

    source.0.borrow_mut().fail = false;
    let duplicate = source.0.borrow().usb[0].clone();
    source.0.borrow_mut().usb.push(duplicate);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    assert_eq!(entries(&app)[0].2.connection, Connection::Uncertain);

    source.usb(true, false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[0].1, id);
    assert_eq!(entries(&app)[0].2.connection, Connection::Present);
    assert!(app.detection_status.is_none());
}

#[test]
fn dismissal_lasts_only_for_the_current_logical_attachment() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let parent = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(parent.clone());
    tick(&ctx, &mut app);
    let (_, id, device) = entries(&app)[1];
    let old_key = device.device.key.clone();
    app.close_tab(id, &ctx);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);

    source.finish(parent.clone(), SimulatedGuitar::Silent);
    app.refresh_guitar(&parent);
    source.guitar(parent.clone());
    app.refresh_guitar(&parent);
    assert_eq!(entries(&app).len(), 2);
    assert_ne!(entries(&app)[1].2.device.key, old_key);
}

#[test]
fn expected_cached_bridge_binding_only_shows_the_pod_while_checking() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    source.0.borrow_mut().usb[0].interfaces.push(UsbInterface {
        number: 1,
        driver: Some("snd-usb-variax".into()),
    });

    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].0, "podxt");
    assert_eq!(
        tabs[0].2.device.guitar_status.as_deref(),
        Some("Guitar: checking…")
    );
    let parent = tabs[0].2.device.key.attachment.clone();
    source.finish(parent.clone(), SimulatedGuitar::Silent);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs.len(), 1);
    assert_eq!(
        tabs[0].2.device.guitar_status.as_deref(),
        Some("Guitar: no guitar answered")
    );
    assert!(tabs[0].2.device.refresh_guitar);
    source.guitar(parent);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    assert_eq!(source.0.borrow().completed.len(), 1);
}

#[test]
fn refresh_button_waits_while_the_rig_is_busy_and_opens_only_after_verification() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    source.finish(attachment.clone(), SimulatedGuitar::Silent);
    tick(&ctx, &mut app);
    source
        .0
        .borrow_mut()
        .results
        .insert(attachment.clone(), SimulatedOutcome::Deferred);
    detection_button(&ctx, &mut app, "refresh-guitar");
    assert_eq!(entries(&app).len(), 1);
    let pod = entries(&app)[0].2;
    assert_eq!(
        pod.device.guitar_status.as_deref(),
        Some("Guitar: checking… (waiting for the rig)")
    );
    assert!(!pod.device.refresh_guitar);
    source.guitar(attachment);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 2);
    assert_eq!(
        entries(&app)[1].2.device.support,
        DeviceSupport::Verified("3.10")
    );
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 3);
}

#[test]
fn refresh_silence_detaches_guitar_and_keeps_last_verified_identity_as_history() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    let guitar = entries(&app)[1].1;

    app.refresh_guitar(&attachment);
    assert_eq!(
        entries(&app)[1].2.device.support,
        DeviceSupport::Refreshing("3.10")
    );
    source.finish(attachment, SimulatedGuitar::Silent);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs[1].1, guitar);
    assert_eq!(tabs[1].2.connection, Connection::Disconnected);
    assert_eq!(
        tabs[0].2.device.guitar_status.as_deref(),
        Some("Guitar: no guitar answered · last verified Variax 600 3.10")
    );
}

#[test]
fn cleanup_failure_suspends_refresh_and_stale_replies_do_not_open_guitar_tabs() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let old = entries(&app)[0].2.device.key.attachment.clone();
    source.finish(old.clone(), SimulatedGuitar::CleanupFailed);
    tick(&ctx, &mut app);
    let pod = entries(&app)[0].2;
    assert!(
        pod.device
            .guitar_status
            .as_ref()
            .unwrap()
            .contains("cleanup failed")
    );
    assert!(!pod.device.refresh_guitar);
    app.refresh_guitar(&old);
    source.guitar(old);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);

    source.usb(false, false);
    tick(&ctx, &mut app);
    source.usb(true, false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 2);
    let new = entries(&app)[1].2.device.key.attachment.clone();
    source.finish(
        new.clone(),
        SimulatedGuitar::Unavailable("Linux refused the open.".into()),
    );
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 2);
    source.guitar(new.clone());
    app.refresh_guitar(&new);
    assert_eq!(entries(&app).len(), 3);
}
