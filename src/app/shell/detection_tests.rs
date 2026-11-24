use super::super::detection::{
    AttachmentKey, DeviceSupport, PresenceSource, SimulatedGuitar, SimulatedJob, SimulatedNotice,
    SimulatedOutcome,
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
    notices: std::collections::BTreeMap<AttachmentKey, Vec<SimulatedNotice>>,
    fail: bool,
    reads: usize,
    address: Option<u8>,
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

    fn usb_address(&self, _: &str) -> Option<u8> {
        self.0.borrow().address.or(Some(10))
    }

    fn simulated_identity(&self, job: &SimulatedJob) -> Option<SimulatedOutcome> {
        let mut snapshot = self.0.borrow_mut();
        let outcome = snapshot.results.remove(&job.attachment)?;
        snapshot.completed.push(job.clone());
        Some(outcome)
    }
    fn simulated_notices(&self, attachment: &AttachmentKey) -> Vec<SimulatedNotice> {
        self.0
            .borrow_mut()
            .notices
            .remove(attachment)
            .unwrap_or_default()
    }
}

impl FakePresence {
    fn notice(&self, parent: &AttachmentKey, notice: SimulatedNotice) {
        self.0
            .borrow_mut()
            .notices
            .entry(parent.clone())
            .or_default()
            .push(notice);
    }

    fn usb(&self, pod: bool, hd: bool) {
        self.usb_paths(if pod { &["8-2.1"] } else { &[] }, hd);
    }

    fn usb_paths(&self, paths: &[&str], hd: bool) {
        let mut usb = SyntheticSysfs::new();
        for path in paths {
            usb.add_device(
                path,
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
    assert_eq!(entries(&app).len(), 1);
    assert_eq!(entries(&app)[0].2.connection, Connection::Disconnected);
    let next = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.finish(next, SimulatedGuitar::Silent);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].1, id);
    assert_eq!(tabs[0].2.connection, Connection::Present);
    assert_ne!(tabs[0].2.device.key, first_key);
}

#[test]
fn reconnect_uses_disconnect_order_and_leaves_other_tabs_offline() {
    let (ctx, mut app, source) = fixture();
    source.usb_paths(&["8-2.1", "8-3.1"], false);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    let first = tabs[0].1;
    let second = tabs[1].1;
    source.finish(
        tabs[0].2.device.key.attachment.clone(),
        SimulatedGuitar::Silent,
    );
    source.finish(
        tabs[1].2.device.key.attachment.clone(),
        SimulatedGuitar::Silent,
    );
    tick(&ctx, &mut app);
    tick(&ctx, &mut app);

    // Disconnect the most recently opened tab first.
    source.usb_paths(&["8-2.1"], false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);
    source.usb_paths(&[], false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[0].2.connection, Connection::Disconnected);

    source.usb_paths(&["8-4.1"], false);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 2);
    let key = app.detection.as_ref().unwrap().attachment_key("8-4.1");
    source.finish(key, SimulatedGuitar::Silent);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs.len(), 2);
    assert_eq!(
        (tabs[0].1, tabs[0].2.connection),
        (first, Connection::Present)
    );
    assert_eq!(
        (tabs[1].1, tabs[1].2.connection),
        (second, Connection::Disconnected)
    );
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
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0].1, guitar);
    assert_eq!(tabs[0].2.connection, Connection::Present);
    assert_eq!(app.tabs.active, Some(guitar));
    assert!(tabs[0].2.reconnected_with_edits);
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Unsaved guitar"
    );
    assert!(has_label(
        &frame(&ctx, &mut app, vec![]).shapes,
        tabs::RECONNECTED_WITH_EDITS
    ));
    assert!(app.variax().read_worker.is_none());
    assert_eq!(source.0.borrow().completed.len(), 2);
    app.tabs.entries[0].editor.undo();
    assert_ne!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Unsaved guitar"
    );
    app.tabs.entries[0].editor.redo();
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
fn preflight_or_local_contention_stays_in_checking_status() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let key = entries(&app)[0].2.device.key.attachment.clone();
    source
        .0
        .borrow_mut()
        .results
        .insert(key, SimulatedOutcome::Deferred);
    tick(&ctx, &mut app);
    assert_eq!(
        entries(&app)[0].2.device.guitar_status.as_deref(),
        Some("Guitar: checking…")
    );
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
        .insert(attachment.clone(), SimulatedOutcome::OtherProcessBusy);
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
    assert_eq!(entries(&app).len(), 1);
    let new = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.finish(
        new.clone(),
        SimulatedGuitar::Unavailable("Linux refused the open.".into()),
    );
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    source.guitar(new.clone());
    app.refresh_guitar(&new);
    assert_eq!(entries(&app).len(), 2);
}

#[test]
fn pod_notices_trigger_identity_and_duplicates_do_not_recheck() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    source.finish(attachment.clone(), SimulatedGuitar::Silent);
    tick(&ctx, &mut app);

    source.notice(&attachment, SimulatedNotice::Connected);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1, "a notice alone cannot add a tab");
    assert!(
        entries(&app)[0]
            .2
            .device
            .guitar_status
            .as_ref()
            .unwrap()
            .contains("checking")
    );
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    let original = entries(&app)[1].2.device.key.clone();
    assert_eq!(source.0.borrow().completed.len(), 2);

    source.notice(&attachment, SimulatedNotice::Connected);
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 2);
    assert_eq!(
        entries(&app)[1].2.device.support,
        DeviceSupport::Verified("3.10")
    );

    source.notice(&attachment, SimulatedNotice::Disconnected);
    tick(&ctx, &mut app);
    assert_eq!(
        entries(&app)[1].2.device.support,
        DeviceSupport::Refreshing("3.10")
    );
    source.finish(attachment.clone(), SimulatedGuitar::Silent);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);

    source.notice(&attachment, SimulatedNotice::Connected);
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 4);
    assert_eq!(entries(&app).len(), 2);
    assert_eq!(entries(&app)[1].2.connection, Connection::Present);
    assert_ne!(entries(&app)[1].2.device.key, original);

    // A disconnect and reconnect in one read window still recheck once.
    source.notice(&attachment, SimulatedNotice::Disconnected);
    source.notice(&attachment, SimulatedNotice::Connected);
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 5);
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 5);
}

#[test]
fn older_guitar_firmware_warns_on_the_pod_without_admitting_a_guitar_tab() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    source.finish(
        attachment.clone(),
        SimulatedGuitar::OlderFirmware("3.06".into()),
    );
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    let warning = entries(&app)[0].2.device.guitar_warning.clone().unwrap();
    assert!(
        warning.contains("Update Firmware") && warning.contains("3.06") && warning.contains("3.07")
    );
    assert!(has_label(&frame(&ctx, &mut app, vec![]).shapes, &warning));

    source.notice(&attachment, SimulatedNotice::Connected);
    source.guitar(attachment);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 2);
    assert!(entries(&app)[0].2.device.guitar_warning.is_none());
}

#[test]
fn same_port_replug_hands_the_verified_new_address_to_the_existing_editor() {
    type Attachments = Rc<RefCell<Vec<Option<(String, u8)>>>>;
    struct EditorSpy(Attachments);

    impl FamilyTab for EditorSpy {
        fn show(&mut self, _: &mut egui::Ui, _: &mut EditorServices<'_>) {}
        fn dirty(&self) -> bool {
            false
        }
        fn close_prompt(&mut self, _: &egui::Context, _: &mut EditorServices<'_>) {}
        fn status(&self) -> (&str, Option<&str>) {
            ("", None)
        }
        fn save_copy(&mut self, _: &egui::Context, _: &mut EditorServices<'_>) {}
        fn observe_device(&mut self, device: Option<&DeviceTab>) {
            self.0.borrow_mut().push(
                device
                    .and_then(DeviceTab::verified_attachment)
                    .map(|(path, address)| (path.to_owned(), address)),
            );
        }
    }

    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let first = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.guitar(first.clone());
    tick(&ctx, &mut app);
    let guitar_id = entries(&app)[1].1;
    let seen = Attachments::default();
    let index = app.tabs.index(guitar_id).unwrap();
    app.tabs.entries[index].editor = Box::new(EditorSpy(seen.clone()));
    tick(&ctx, &mut app);
    assert_eq!(seen.borrow().last(), Some(&Some(("8-2.1".into(), 10))));

    // No absence was observed between the two enumerations.
    source.0.borrow_mut().address = Some(11);
    tick(&ctx, &mut app);
    assert_eq!(seen.borrow().last(), Some(&None));
    let second = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    assert_ne!(first, second);
    source.guitar(second);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].1, guitar_id);
    assert_eq!(seen.borrow().last(), Some(&Some(("8-2.1".into(), 11))));
}

#[test]
fn full_backup_refuses_a_guitar_replaced_while_the_folder_picker_is_open() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.guitar(attachment);
    tick(&ctx, &mut app);
    let target = app.verified_backup_guitar().unwrap().clone();

    source.usb(false, false);
    tick(&ctx, &mut app);
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.guitar(attachment);
    tick(&ctx, &mut app);
    assert_ne!(app.verified_backup_guitar(), Some(&target));
    app.start_backup_in(
        BackupChoice::VariaxFull,
        Some(target),
        Some("unused".into()),
    );
    assert_eq!(
        app.device_check_notice,
        "The verified Variax connection changed. Nothing was sent."
    );
    assert!(app.backup_rx.is_none());
}

#[test]
fn editor_refresh_uses_its_current_attachment_even_when_another_tab_is_active() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let old = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(old.clone());
    tick(&ctx, &mut app);
    let guitar = entries(&app)[1].1;

    app.new_tab(registry::LEGACY_STARTUP);
    let offline = app.tabs.active.unwrap();
    assert!(!app.test_request_guitar_refresh(offline));
    assert!(app.test_request_guitar_refresh(guitar));
    assert_eq!(app.tabs.active, Some(offline));
    assert_eq!(
        entries(&app)[1].2.device.support,
        DeviceSupport::Refreshing("3.10")
    );
    assert!(!app.test_request_guitar_refresh(guitar));

    source.guitar(old.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.last().unwrap().attachment, old);
    source.usb(false, false);
    tick(&ctx, &mut app);
    assert!(!app.test_request_guitar_refresh(guitar));

    source.usb_paths(&["8-4.1"], false);
    tick(&ctx, &mut app);
    let new = app.detection.as_ref().unwrap().attachment_key("8-4.1");
    source.guitar(new.clone());
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].1, guitar);
    assert!(app.test_request_guitar_refresh(guitar));
    source.guitar(new.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.last().unwrap().attachment, new);
}

#[test]
fn follow_disconnect_keeps_the_tab_and_reattaches_only_after_reconnect_identity() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let parent = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(parent.clone());
    tick(&ctx, &mut app);
    let (_, guitar, device) = entries(&app)[1];
    let old = device.device.key.clone();
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
    app.with_tab(guitar, |entry, _| {
        let editor = entry.editor.as_mut() as &mut dyn std::any::Any;
        let editor = editor.downcast_mut::<VariaxEditor>().unwrap();
        editor.load(&file);
        editor.change(|preset| preset.rename("Unsaved model"));
    });

    app.apply_device_event(guitar, tabs::DeviceEvent::GuitarDisconnected(old.clone()));
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);
    assert_eq!(entries(&app)[0].2.connection, Connection::Present);
    assert_eq!(source.0.borrow().completed.len(), 1);

    source.notice(&parent, SimulatedNotice::Connected);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);
    source.guitar(parent);
    tick(&ctx, &mut app);

    let tabs = entries(&app);
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[1].1, guitar);
    assert_eq!(tabs[1].2.connection, Connection::Present);
    assert_ne!(tabs[1].2.device.key, old);
    assert!(tabs[1].2.reconnected_with_edits);
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Unsaved model"
    );
}

#[test]
fn follow_notice_for_another_attachment_or_tab_is_ignored() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let parent = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(parent);
    tick(&ctx, &mut app);
    let (_, guitar, device) = entries(&app)[1];
    let old = device.device.key.clone();

    source.usb(false, false);
    tick(&ctx, &mut app);
    source.usb_paths(&["8-4.1"], false);
    tick(&ctx, &mut app);
    let parent = app.detection.as_ref().unwrap().attachment_key("8-4.1");
    source.guitar(parent);
    tick(&ctx, &mut app);
    let pod = entries(&app)[0].1;
    let (_, reattached, device) = entries(&app)[1];
    assert_eq!(reattached, guitar);
    let current = device.device.key.clone();

    app.apply_device_event(guitar, tabs::DeviceEvent::GuitarDisconnected(old));
    app.apply_device_event(pod, tabs::DeviceEvent::GuitarDisconnected(current.clone()));
    assert_eq!(entries(&app)[1].2.connection, Connection::Present);
    assert_eq!(entries(&app)[1].2.device.key, current);
    assert_eq!(source.0.borrow().completed.len(), 2);
}
