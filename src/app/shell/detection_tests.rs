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
    registry_id: Option<u64>,
    interfaces_unavailable: bool,
    readiness_reason: Option<String>,
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

    fn usb_registry_id(&self, _: &str) -> Option<u64> {
        self.0.borrow().registry_id
    }

    fn usb_open_candidate(&self, _: &str) -> bool {
        !self.0.borrow().interfaces_unavailable
    }

    fn usb_readiness_reason(&self, _: &str) -> Option<String> {
        self.0.borrow().readiness_reason.clone()
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
fn passive_pod_inventory_stays_visible_without_interfaces_or_access() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    source.0.borrow_mut().registry_id = Some(30);
    source.0.borrow_mut().interfaces_unavailable = true;
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs.len(), 1);
    let (family, tab, device) = tabs[0];
    assert_eq!(family, "podxt");
    assert_eq!(device.device.support, DeviceSupport::UsbOnly);
    assert!(device.verified_attachment().is_none());
    let key = device.device.key.attachment.clone();
    source.0.borrow_mut().results.insert(
        key.clone(),
        SimulatedOutcome::Finished {
            pod_firmware: None,
            guitar: SimulatedGuitar::Unavailable("macOS reports the device is busy".into()),
        },
    );

    tick(&ctx, &mut app);
    assert!(source.0.borrow().completed.is_empty());
    source.0.borrow_mut().interfaces_unavailable = false;
    tick(&ctx, &mut app);
    let (_, same_tab, busy_device) = entries(&app)[0];
    assert_eq!(same_tab, tab);
    assert_eq!(busy_device.device.key.attachment, key);
    assert_eq!(busy_device.device.support, DeviceSupport::UsbOnly);
    assert!(busy_device.verified_attachment().is_none());
    assert_eq!(source.0.borrow().completed.len(), 1);
    source.0.borrow_mut().interfaces_unavailable = true;
    tick(&ctx, &mut app);
    source.guitar(key);
    source.0.borrow_mut().interfaces_unavailable = false;
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 1);
    assert!(entries(&app)[0].2.verified_attachment().is_none());
}

#[test]
fn passive_readiness_reason_updates_without_an_identity_job() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    source.0.borrow_mut().registry_id = Some(30);
    source.0.borrow_mut().interfaces_unavailable = true;

    for reason in [
        "PODxt Live detected. USB route unavailable in this scan.",
        "PODxt Live detected. USB interface 1 was absent from this scan.",
    ] {
        source.0.borrow_mut().readiness_reason = Some(reason.into());
        tick(&ctx, &mut app);
        let (_, _, entry) = entries(&app)[0];
        assert_eq!(entry.device.support, DeviceSupport::UsbOnly);
        assert_eq!(entry.device.guitar_status.as_deref(), Some(reason));
        let output = frame(&ctx, &mut app, vec![]);
        assert!(has_label(&output.shapes, "USB identity unverified"));
        assert!(has_label(&output.shapes, reason));
    }
    assert!(source.0.borrow().completed.is_empty());
}

#[test]
fn passive_publication_text_gives_way_to_the_identity_result() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    source.0.borrow_mut().registry_id = Some(30);
    let ready = "PODxt Live detected. USB interfaces 0 and 1 are published. Identity unverified.";
    source.0.borrow_mut().readiness_reason = Some(ready.into());

    tick(&ctx, &mut app);
    assert_eq!(
        entries(&app)[0].2.device.guitar_status.as_deref(),
        Some(ready)
    );

    let attached = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.guitar(attached);
    tick(&ctx, &mut app);
    assert_ne!(
        entries(&app)[0].2.device.guitar_status.as_deref(),
        Some(ready)
    );
    assert_eq!(source.0.borrow().completed.len(), 1);
}

#[test]
fn passive_inventory_replacement_drops_stale_identity_at_the_same_path_and_address() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    source.0.borrow_mut().registry_id = Some(30);
    tick(&ctx, &mut app);
    let old = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.guitar(old.clone());

    source.0.borrow_mut().registry_id = Some(31);
    tick(&ctx, &mut app);
    let new = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    assert_ne!(new, old);
    assert_eq!(new.usb_location(), old.usb_location());
    assert!(source.0.borrow().completed.is_empty());
    assert_eq!(entries(&app).len(), 1);
    assert!(entries(&app)[0].2.verified_attachment().is_none());
    assert_eq!(entries(&app)[0].2.connection, Connection::Disconnected);
    let visible = frame(&ctx, &mut app, vec![]);
    assert!(has_label(
        &visible.shapes,
        "Checking PODxt Live identity on USB interface 0…"
    ));
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
    let tabs = entries(&app);
    let pod = tabs[0].1;
    let pod_key = tabs[0].2.device.key.clone();
    let guitar = tabs[1].1;
    let original = tabs[1].2.device.key.clone();
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
    app.with_tab(guitar, |entry, _| {
        let editor = entry.editor.as_mut() as &mut dyn std::any::Any;
        let editor = editor.downcast_mut::<VariaxEditor>().unwrap();
        editor.load(&file);
        editor.change(|preset| preset.rename("Unsaved model"));
    });
    assert_eq!(source.0.borrow().completed.len(), 2);

    source.notice(&attachment, SimulatedNotice::Connected);
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 2);
    assert_eq!(
        entries(&app)[1].2.device.support,
        DeviceSupport::Verified("3.10")
    );

    source.notice(&attachment, SimulatedNotice::Disconnected);
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(tabs[0].1, pod);
    assert_eq!(tabs[0].2.device.key, pod_key);
    assert_eq!(tabs[0].2.device.support, DeviceSupport::Verified("3.01"));
    assert_eq!(
        tabs[0].2.device.guitar_status.as_deref(),
        Some("Guitar: disconnected · last verified Variax 600 3.10")
    );
    assert_eq!(tabs[1].1, guitar);
    assert_eq!(tabs[1].2.connection, Connection::Disconnected);
    assert!(tabs[1].2.verified_attachment().is_none());
    assert!(app.verified_backup_guitar().is_none());

    source.notice(&attachment, SimulatedNotice::Disconnected);
    for _ in 0..3 {
        tick(&ctx, &mut app);
    }
    assert_eq!(source.0.borrow().completed.len(), 2);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);

    source.notice(&attachment, SimulatedNotice::Connected);
    tick(&ctx, &mut app);
    let tabs = entries(&app);
    assert_eq!(source.0.borrow().completed.len(), 3);
    assert_eq!(tabs.len(), 2);
    assert_eq!(tabs[0].1, pod);
    assert_eq!(tabs[0].2.device.key, pod_key);
    assert_eq!(tabs[1].1, guitar);
    assert_eq!(tabs[1].2.connection, Connection::Present);
    assert_eq!(tabs[1].2.device.key.attachment, attachment);
    assert_ne!(tabs[1].2.device.key, original);
    assert!(tabs[1].2.reconnected_with_edits);
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Unsaved model"
    );

    // A disconnect and reconnect in one read window still recheck once.
    source.notice(&attachment, SimulatedNotice::Disconnected);
    source.notice(&attachment, SimulatedNotice::Connected);
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 4);
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 4);
}

#[test]
fn linux_return_waits_for_the_rig_and_uses_one_guitar_identity() {
    use super::super::detection::SimulatedJobKind;

    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    source.notice(&attachment, SimulatedNotice::Disconnected);
    tick(&ctx, &mut app);

    source.notice(&attachment, SimulatedNotice::Connected);
    source
        .0
        .borrow_mut()
        .results
        .insert(attachment.clone(), SimulatedOutcome::OtherProcessBusy);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);
    assert!(
        entries(&app)[0]
            .2
            .device
            .guitar_status
            .as_ref()
            .unwrap()
            .contains("waiting for the rig")
    );

    source.notice(&attachment, SimulatedNotice::Connected);
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].2.connection, Connection::Present);
    source.guitar(attachment);
    tick(&ctx, &mut app);
    assert_eq!(
        source
            .0
            .borrow()
            .completed
            .iter()
            .map(|job| job.kind)
            .collect::<Vec<_>>(),
        [
            SimulatedJobKind::Initial,
            SimulatedJobKind::VerifyGuitarOnReturn, // Lock busy, so no exchange.
            SimulatedJobKind::VerifyGuitarOnReturn,
        ]
    );
}

#[test]
fn linux_loss_cancels_a_queued_busy_return_and_stale_attachment_notices_are_ignored() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let old = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(old.clone());
    tick(&ctx, &mut app);

    source.notice(&old, SimulatedNotice::Disconnected);
    source.notice(&old, SimulatedNotice::Connected);
    source
        .0
        .borrow_mut()
        .results
        .insert(old.clone(), SimulatedOutcome::Deferred);
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 2);

    source.notice(&old, SimulatedNotice::Disconnected);
    source.guitar(old.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 2);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);

    source.0.borrow_mut().address = Some(11);
    tick(&ctx, &mut app);
    let new = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    assert_ne!(new, old);
    source.guitar(new.clone());
    tick(&ctx, &mut app);
    source.notice(&old, SimulatedNotice::Disconnected);
    source.notice(&old, SimulatedNotice::Connected);
    source.guitar(new.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 3);
    assert_eq!(entries(&app)[1].2.device.key.attachment, new);
    assert_eq!(entries(&app)[1].2.connection, Connection::Present);
}

#[test]
fn linux_explicit_refresh_can_recover_when_no_return_notice_arrives() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(attachment.clone());
    tick(&ctx, &mut app);

    source.notice(&attachment, SimulatedNotice::Disconnected);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);
    source.guitar(attachment.clone());
    app.refresh_guitar(&attachment);
    assert_eq!(entries(&app)[1].2.connection, Connection::Present);

    source.notice(&attachment, SimulatedNotice::Disconnected);
    tick(&ctx, &mut app);
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);
    assert_eq!(source.0.borrow().completed.len(), 2);
}

fn unverified_guitar(revision: &[u8; 4]) -> SimulatedGuitar {
    let mut reply = variax_workbench::identity_stage::guitar_reply();
    reply[12..16].copy_from_slice(revision);
    let identity = line6_devices::identity::Identity::parse(&reply).unwrap();
    SimulatedGuitar::Unverified(
        variax_workbench::identity_stage::UnverifiedGuitarReply::from_identity(&identity).unwrap(),
    )
}

#[test]
fn unverified_guitar_firmware_warns_on_the_pod_and_opens_an_offline_guitar_tab() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    assert_eq!(
        entries(&app).len(),
        1,
        "presence alone must not open a guitar tab"
    );
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    source.finish(attachment.clone(), unverified_guitar(b"0311"));
    tick(&ctx, &mut app);
    let connected = entries(&app);
    assert_eq!(connected.len(), 2);
    let guitar = connected[1].1;
    let offline = connected[1].2;
    let warning = offline.guitar_firmware_notice().unwrap();
    let text = warning.message();
    assert!(text.contains("3.11") && text.contains("3.10"));
    assert!(!text.to_ascii_lowercase().contains("update"));
    assert_eq!(connected[0].2.device.guitar_warning, Some(warning));
    assert!(connected[0].2.guitar_firmware_notice().is_none());
    assert!(offline.verified_attachment().is_none());
    assert!(offline.guitar_refresh_attachment().is_none());
    assert!(app.verified_backup_guitar().is_none());
    assert!(app.backup_firmware_blocked(BackupChoice::Variax));
    assert!(!app.backup_firmware_blocked(BackupChoice::Podxt));
    assert!(
        app.tabs.entries[app.tabs.index(guitar).unwrap()]
            .title()
            .ends_with("offline only")
    );
    assert!(has_label(&frame(&ctx, &mut app, vec![]).shapes, &text));

    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
    let folder = tempfile::tempdir().unwrap();
    let output = folder.path().join("edited.vwm");
    app.dialogs = Box::new(FakeDialogs {
        save: [Some(output.clone())].into(),
        ..Default::default()
    });
    app.with_tab(guitar, |entry, services| {
        let editor = entry.editor.as_mut() as &mut dyn std::any::Any;
        let editor = editor.downcast_mut::<VariaxEditor>().unwrap();
        editor.load(&input);
        editor.change(|preset| preset.rename("Offline edit"));
        assert!(editor.dirty());
        editor.save_copy(&ctx, services);
    });
    app.poll_picker(&ctx);
    assert_eq!(
        EditorDocument::open(&output).unwrap().preset().name(),
        "Offline edit"
    );
    app.variax_mut()
        .change(|preset| preset.rename("Unsaved edit"));
    assert!(app.variax().dirty());
    assert_eq!(source.0.borrow().completed.len(), 1);

    source.notice(&attachment, SimulatedNotice::Connected);
    source.guitar(attachment);
    tick(&ctx, &mut app);
    let rechecked = entries(&app);
    assert_eq!(rechecked.len(), 2);
    assert!(rechecked[0].2.device.guitar_warning.is_none());
    assert_eq!(rechecked[1].1, guitar);
    assert!(rechecked[1].2.guitar_firmware_notice().is_none());
    assert_eq!(rechecked[1].2.verified_attachment(), Some(("8-2.1", 10)));
    assert!(app.variax().dirty());
    assert_eq!(source.0.borrow().completed.len(), 2);
}

#[test]
fn only_the_attachment_that_reported_unverified_firmware_has_the_notice() {
    let (ctx, mut app, source) = fixture();
    source.usb_paths(&["8-2.1", "8-2.2"], false);
    tick(&ctx, &mut app);
    let detection = app.detection.as_ref().unwrap();
    let first = detection.attachment_key("8-2.1");
    let second = detection.attachment_key("8-2.2");
    source.finish(first, unverified_guitar(b"0309"));
    source.guitar(second);
    tick(&ctx, &mut app);
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 2);
    let connected = entries(&app);
    let (_, unverified_tab, unverified) = connected
        .iter()
        .find(|(family, _, device)| {
            *family == "variax" && device.device.key.attachment.path == "8-2.1"
        })
        .unwrap();
    assert!(unverified.guitar_firmware_notice().is_some());
    assert!(unverified.verified_attachment().is_none());
    let unverified_tab = *unverified_tab;
    let (_, verified_tab, verified) = connected
        .iter()
        .find(|(family, _, device)| {
            *family == "variax" && device.device.key.attachment.path == "8-2.2"
        })
        .unwrap();
    assert!(verified.guitar_firmware_notice().is_none());
    assert_eq!(verified.verified_attachment(), Some(("8-2.2", 10)));
    let verified_tab = *verified_tab;

    source.usb_paths(&["8-2.2"], false);
    tick(&ctx, &mut app);
    let offline = app.tabs.entries[app.tabs.index(unverified_tab).unwrap()]
        .device
        .as_ref()
        .unwrap();
    assert_eq!(offline.connection, Connection::Disconnected);
    assert!(offline.guitar_firmware_notice().is_none());
    assert!(offline.verified_attachment().is_none());
    let still_verified = app.tabs.entries[app.tabs.index(verified_tab).unwrap()]
        .device
        .as_ref()
        .unwrap();
    assert_eq!(still_verified.verified_attachment(), Some(("8-2.2", 10)));

    source.usb_paths(&["8-2.1", "8-2.2"], false);
    tick(&ctx, &mut app);
    let replacement = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.guitar(replacement);
    tick(&ctx, &mut app);
    let reattached = app.tabs.entries[app.tabs.index(unverified_tab).unwrap()]
        .device
        .as_ref()
        .unwrap();
    assert!(reattached.guitar_firmware_notice().is_none());
    assert_eq!(reattached.verified_attachment(), Some(("8-2.1", 10)));
}

#[test]
fn a_pod_reply_on_the_guitar_link_cannot_open_an_offline_guitar_tab() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let attachment = entries(&app)[0].2.device.key.attachment.clone();
    let reply = variax_workbench::identity_stage::bridge_reply();
    let identity = line6_devices::identity::Identity::parse(&reply).unwrap();
    let assessment =
        variax_workbench::identity_stage::UnverifiedGuitarReply::from_identity(&identity).unwrap();
    source.finish(attachment, SimulatedGuitar::Unverified(assessment));
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    assert!(entries(&app)[0].2.device.guitar_warning.is_some());
}

#[test]
fn shell_and_device_check_report_unverified_pod_firmware_and_keep_offline_editing() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let key = entries(&app)[0].2.device.key.attachment.clone();
    let mut reply = variax_workbench::identity_stage::bridge_reply();
    reply[15] = 2;
    let identity = line6_devices::identity::Identity::parse(&reply).unwrap();
    let notice = FirmwareNotice::podxt(&identity).unwrap();
    source
        .0
        .borrow_mut()
        .results
        .insert(key, SimulatedOutcome::UnverifiedPod(notice));
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    assert!(entries(&app)[0].2.verified_attachment().is_none());
    assert!(app.backup_firmware_blocked(BackupChoice::Podxt));
    assert!(app.backup_firmware_blocked(BackupChoice::Variax));
    let text = notice.message();
    assert!(text.contains("3.02") && text.contains("3.01"));
    assert!(has_label(&frame(&ctx, &mut app, vec![]).shapes, &text));

    app.device_check = ProbeSession::new(RigBackend::Simulated(device_probe::SimulatedRig::new(
        device_probe::Scenario::Connected,
    )));
    app.device_check_open = true;
    frame(&ctx, &mut app, vec![]);
    assert!(app.device_check_visible.contains(&text));
    assert!(!app.device_check_buttons.contains_key("backup-podxt"));
    assert!(!app.device_check_buttons.contains_key("tone-try"));

    let path = tempfile::tempdir().unwrap();
    let tone = path.path().join("offline.l6t");
    std::fs::write(&tone, podxt_panel::tests::sample_pod_tone()).unwrap();
    app.device_check_open = false;
    let tab = app.open_file(tone).unwrap();
    let entry = &app.tabs.entries[app.tabs.index(tab).unwrap()];
    assert_eq!(entry.family.info().id, "podxt");
    assert!(entry.device.is_none());
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

#[test]
fn unbound_driver_at_start_requires_consent_before_repair_and_identity() {
    use super::super::detection::SimulatedJobKind;

    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let parent = entries(&app)[0].2.device.key.attachment.clone();
    source.finish(
        parent.clone(),
        SimulatedGuitar::DriverUnbound("driver unbound".into()),
    );
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    assert_eq!(
        entries(&app)[0].2.device.guitar_status.as_deref(),
        Some("Guitar: driver unbound")
    );

    // A passive notice cannot grant consent or send identity on the unbound bridge.
    source.notice(&parent, SimulatedNotice::Connected);
    source.guitar(parent.clone());
    tick(&ctx, &mut app);
    assert_eq!(source.0.borrow().completed.len(), 1);

    detection_button(&ctx, &mut app, "refresh-guitar");
    assert!(matches!(
        app.detect_confirmation,
        Some(DetectConfirmation::Reattach { .. })
    ));
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
    assert_eq!(source.0.borrow().completed.len(), 1);
    assert_eq!(entries(&app).len(), 1);

    detection_button(&ctx, &mut app, "refresh-guitar");
    detection_button(&ctx, &mut app, "yes");
    assert!(app.detect_confirmation.is_none());
    let completed = &source.0.borrow().completed;
    assert_eq!(completed.len(), 2);
    assert_eq!(completed[1].kind, SimulatedJobKind::ReattachGuitarDriver);
    assert_eq!(completed[1].attachment, parent);
    assert_eq!(entries(&app)[1].0, "variax");
}

#[test]
fn driver_repair_consent_does_not_carry_over_to_a_new_usb_address() {
    use super::super::detection::SimulatedJobKind;

    let (ctx, mut app, source) = fixture();
    source.usb(true, false);
    tick(&ctx, &mut app);
    let parent = entries(&app)[0].2.device.key.attachment.clone();
    source.finish(
        parent.clone(),
        SimulatedGuitar::DriverUnbound("driver unbound".into()),
    );
    tick(&ctx, &mut app);
    detection_button(&ctx, &mut app, "refresh-guitar");

    source.0.borrow_mut().address = Some(11);
    source.guitar(parent);
    detection_button(&ctx, &mut app, "yes");
    assert_eq!(source.0.borrow().completed.len(), 1);
    let replacement = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    source.finish(
        replacement,
        SimulatedGuitar::DriverUnbound("replacement driver unbound".into()),
    );
    tick(&ctx, &mut app);
    assert_eq!(entries(&app).len(), 1);
    assert_eq!(
        source.0.borrow().completed[1].kind,
        SimulatedJobKind::Initial
    );
}

#[test]
fn mac_discovery_retries_at_cadence_until_the_two_identities_verify() {
    let (_, mut app, source) = fixture();
    source.usb(true, false);
    source.0.borrow_mut().registry_id = Some(30);
    let service = app.detection.as_mut().unwrap();
    let start = Instant::now();
    let profiles = registry::DEVICE_PROFILES;
    service
        .refresh_at(Scan::Confirmed, profiles, start)
        .unwrap();
    let key = service.attachment_key("8-2.1");
    source
        .0
        .borrow_mut()
        .results
        .insert(key.clone(), SimulatedOutcome::MacPodSilent);

    service
        .refresh_at(Scan::Automatic, profiles, start)
        .unwrap();
    source.finish(key.clone(), SimulatedGuitar::Silent);
    service
        .refresh_at(
            Scan::Automatic,
            profiles,
            start + Duration::from_millis(499),
        )
        .unwrap();
    assert_eq!(source.0.borrow().completed.len(), 1);
    let first_retry = start + Duration::from_millis(500);
    service
        .refresh_at(Scan::Automatic, profiles, first_retry)
        .unwrap();

    source.guitar(key.clone());
    let guitar_retry = first_retry + Duration::from_millis(500);
    source.0.borrow_mut().interfaces_unavailable = true;
    service
        .refresh_at(Scan::Automatic, profiles, guitar_retry)
        .unwrap();
    assert_eq!(source.0.borrow().completed.len(), 2);
    source.0.borrow_mut().interfaces_unavailable = false;
    let devices = service
        .refresh_at(
            Scan::Automatic,
            profiles,
            guitar_retry + Duration::from_millis(500),
        )
        .unwrap();
    let guitar_key = devices
        .iter()
        .find(|device| device.support == DeviceSupport::Verified("3.10"))
        .unwrap()
        .key
        .clone();

    source.guitar(key.clone());
    service
        .refresh_at(
            Scan::Automatic,
            profiles,
            guitar_retry + Duration::from_secs(20),
        )
        .unwrap();
    let kinds: Vec<_> = source
        .0
        .borrow()
        .completed
        .iter()
        .map(|job| job.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            super::super::detection::SimulatedJobKind::Initial,
            super::super::detection::SimulatedJobKind::Initial,
            super::super::detection::SimulatedJobKind::DiscoverGuitar,
        ]
    );

    // Interface publication can temporarily disappear on a still-present USB
    // attachment. It withdraws the tab's connection without rearming identity.
    source.0.borrow_mut().interfaces_unavailable = true;
    let devices = service
        .refresh_at(
            Scan::Automatic,
            profiles,
            guitar_retry + Duration::from_secs(21),
        )
        .unwrap();
    assert!(
        devices
            .iter()
            .all(|device| device.support == DeviceSupport::UsbOnly)
    );
    source.0.borrow_mut().interfaces_unavailable = false;
    let devices = service
        .refresh_at(
            Scan::Automatic,
            profiles,
            guitar_retry + Duration::from_secs(22),
        )
        .unwrap();
    assert!(devices.iter().any(
        |device| device.key == guitar_key && device.support == DeviceSupport::Verified("3.10")
    ));
    assert_eq!(source.0.borrow().completed.len(), 3);

    service
        .report_guitar_disconnect(&guitar_key, profiles)
        .unwrap();
    service
        .refresh_at(
            Scan::Automatic,
            profiles,
            guitar_retry + Duration::from_secs(23),
        )
        .unwrap();
    assert_eq!(
        source.0.borrow().completed.last().unwrap().kind,
        super::super::detection::SimulatedJobKind::DiscoverGuitar
    );
    assert_eq!(source.0.borrow().completed.len(), 4);
}

#[test]
fn mac_discovery_defers_for_a_busy_rig_and_stops_on_failed_cleanup() {
    for busy in [
        SimulatedOutcome::Deferred,
        SimulatedOutcome::OtherProcessBusy,
    ] {
        let (_, mut app, source) = fixture();
        source.usb(true, false);
        source.0.borrow_mut().registry_id = Some(30);
        let service = app.detection.as_mut().unwrap();
        let start = Instant::now();
        let profiles = registry::DEVICE_PROFILES;
        service
            .refresh_at(Scan::Confirmed, profiles, start)
            .unwrap();
        let key = service.attachment_key("8-2.1");
        source.0.borrow_mut().results.insert(key.clone(), busy);
        service
            .refresh_at(Scan::Automatic, profiles, start)
            .unwrap();
        source.finish(key.clone(), SimulatedGuitar::CleanupFailed);
        service
            .refresh_at(
                Scan::Automatic,
                profiles,
                start + Duration::from_millis(499),
            )
            .unwrap();
        assert_eq!(source.0.borrow().completed.len(), 1);
        service
            .refresh_at(
                Scan::Automatic,
                profiles,
                start + Duration::from_millis(500),
            )
            .unwrap();
        source.guitar(key);
        source.0.borrow_mut().interfaces_unavailable = true;
        service
            .refresh_at(Scan::Automatic, profiles, start + Duration::from_secs(1))
            .unwrap();
        source.0.borrow_mut().interfaces_unavailable = false;
        service
            .refresh_at(Scan::Automatic, profiles, start + Duration::from_secs(2))
            .unwrap();
        assert_eq!(source.0.borrow().completed.len(), 2);

        source.0.borrow_mut().registry_id = Some(31);
        service
            .refresh_at(Scan::Confirmed, profiles, start + Duration::from_secs(3))
            .unwrap();
        source.guitar(service.attachment_key("8-2.1"));
        let devices = service
            .refresh_at(Scan::Automatic, profiles, start + Duration::from_secs(3))
            .unwrap();
        assert_eq!(source.0.borrow().completed.len(), 3);
        assert!(
            devices
                .iter()
                .any(|device| device.support == DeviceSupport::Verified("3.10"))
        );
    }
}

#[test]
fn mac_identity_diagnostics_keep_current_usb_and_historical_guitar_distinct() {
    let (_, mut app, source) = fixture();
    source.usb(true, false);
    source.0.borrow_mut().registry_id = Some(30);
    let service = app.detection.as_mut().unwrap();
    let now = Instant::now();
    let profiles = registry::DEVICE_PROFILES;
    service.refresh_at(Scan::Confirmed, profiles, now).unwrap();
    let key = service.attachment_key("8-2.1");
    source.guitar(key.clone());
    service.refresh_at(Scan::Automatic, profiles, now).unwrap();

    assert!(service.queue_guitar_refresh(&key, false));
    source.finish(key.clone(), SimulatedGuitar::Silent);
    let devices = service
        .refresh_at(Scan::Automatic, profiles, now + Duration::from_secs(1))
        .unwrap();
    let [pod] = devices.as_slice() else {
        panic!("only the verified PODxt remains")
    };
    assert_eq!(pod.support, DeviceSupport::Verified("3.01"));
    let status = pod.guitar_status.as_deref().unwrap();
    assert!(status.contains("One Variax identity request went unanswered"));
    assert!(status.contains("USB release completed"));
    assert!(status.contains("last verified Variax 600 3.10"));
    let details = pod.mac_identity_details.as_deref().unwrap();
    assert!(details.contains("path 8-2.1"));
    assert!(details.contains("USB interfaces 0 and 1"));
    assert!(details.contains("USB configuration unknown"));

    assert!(service.queue_guitar_refresh(&key, false));
    source
        .0
        .borrow_mut()
        .results
        .insert(key, SimulatedOutcome::OtherProcessBusy);
    let devices = service
        .refresh_at(Scan::Automatic, profiles, now + Duration::from_secs(2))
        .unwrap();
    let status = devices[0].guitar_status.as_deref().unwrap();
    assert!(status.contains("busy with another operation"));
    assert!(status.contains("last verified Variax 600 3.10"));
}

#[test]
fn mac_discovery_stops_on_unverified_identity_or_receive_error() {
    use line6_devices::identity::Identity;
    use variax_workbench::{firmware_notice::FirmwareNotice, identity_stage::bridge_reply};

    let mut wrong_firmware = bridge_reply();
    wrong_firmware[15] = 2;
    let identity = Identity::parse(&wrong_firmware).unwrap();
    for terminal in [
        SimulatedOutcome::UnverifiedPod(FirmwareNotice::podxt(&identity).unwrap()),
        SimulatedOutcome::Finished {
            pod_firmware: Some("3.01"),
            guitar: SimulatedGuitar::Unavailable(
                "The guitar sent data without a valid identity.".into(),
            ),
        },
    ] {
        let (_, mut app, source) = fixture();
        source.usb(true, false);
        source.0.borrow_mut().registry_id = Some(30);
        let service = app.detection.as_mut().unwrap();
        let start = Instant::now();
        let profiles = registry::DEVICE_PROFILES;
        service
            .refresh_at(Scan::Confirmed, profiles, start)
            .unwrap();
        let key = service.attachment_key("8-2.1");
        source.0.borrow_mut().results.insert(key.clone(), terminal);
        service
            .refresh_at(Scan::Automatic, profiles, start)
            .unwrap();
        source.guitar(key);
        service
            .refresh_at(Scan::Automatic, profiles, start + Duration::from_secs(10))
            .unwrap();
        assert_eq!(source.0.borrow().completed.len(), 1);
    }
}

#[test]
fn connection_window_distinguishes_unavailable_failed_and_empty_inventory() {
    let (ctx, mut app, source) = fixture();
    let detection = app.detection.take();
    app.new_tab(registry::LEGACY_STARTUP);
    let editor = app.tabs.active;
    frame(&ctx, &mut app, vec![]);
    let editing = frame(&ctx, &mut app, vec![]);
    assert!(!has_label(&editing.shapes, "Device detection unavailable"));
    assert_eq!(app.tabs.active, editor);

    app.tabs.active = None;
    frame(&ctx, &mut app, vec![]);
    let offline = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&offline.shapes, "Device detection unavailable"));
    assert!(has_label(&offline.shapes, "Edit offline"));
    assert!(!has_label(
        &offline.shapes,
        "No supported device is connected"
    ));
    assert_eq!(source.0.borrow().reads, 0);

    app.detection = detection;
    source.0.borrow_mut().fail = true;
    tick(&ctx, &mut app);
    let failed = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&failed.shapes, "Device check failed"));
    assert!(has_label(
        &failed.shapes,
        "could not read injected USB metadata"
    ));
    assert!(!has_label(
        &failed.shapes,
        "No supported device is connected"
    ));

    source.0.borrow_mut().fail = false;
    tick(&ctx, &mut app);
    let empty = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&empty.shapes, "No supported device is connected"));
    assert!(!has_label(&empty.shapes, "Device check failed"));
    assert!(app.tabs.active.is_none());
    assert!(entries(&app).is_empty());
}

#[test]
fn connection_window_updates_after_startup_and_keeps_the_open_editor() {
    let (ctx, mut app, source) = fixture();
    app.new_tab(registry::LEGACY_STARTUP);
    let editor = app.tabs.active.unwrap();
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/editor-preset.vwm");
    app.with_tab(editor, |entry, _| {
        let editor = entry.editor.as_mut() as &mut dyn std::any::Any;
        let editor = editor.downcast_mut::<VariaxEditor>().unwrap();
        editor.load(&file);
        editor.change(|preset| preset.rename("Unsaved model"));
    });
    app.tabs.active = None;
    tick(&ctx, &mut app);
    let empty = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&empty.shapes, "No supported device is connected"));

    source.usb(true, true);
    tick(&ctx, &mut app);
    let waiting = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&waiting.shapes, "PODxt Live"));
    assert!(has_label(&waiting.shapes, "Checking connection…"));
    assert!(has_label(&waiting.shapes, "POD HD500X Connected"));
    assert!(has_label(&waiting.shapes, "Live connection not supported"));
    assert!(!has_label(
        &waiting.shapes,
        "No supported device is connected"
    ));
    assert_eq!(
        entries(&app).len(),
        1,
        "passive inventory may not open the HD editor"
    );
    assert!(app.tabs.active.is_none(), "Home stays selected");

    app.tabs.active = Some(editor);
    let relay = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(relay);
    tick(&ctx, &mut app);
    let verified = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&verified.shapes, "PODxt Live Connected"));
    assert!(has_label(&verified.shapes, "Variax 300/600 Connected"));
    #[cfg(feature = "usb")]
    assert!(has_label(
        &verified.shapes,
        "PODxt library download is unavailable in this build."
    ));
    #[cfg(not(feature = "usb"))]
    assert!(has_label(
        &verified.shapes,
        "Library download is unavailable for this connection."
    ));
    assert_eq!(source.0.borrow().completed.len(), 1);
    assert_eq!(app.tabs.active, Some(editor));
    assert_eq!(
        app.variax().document.as_ref().unwrap().preset().name(),
        "Unsaved model"
    );
}

#[test]
fn connection_window_scopes_disconnect_and_reconnect_to_the_current_generation() {
    let (ctx, mut app, source) = fixture();
    source.usb(true, true);
    tick(&ctx, &mut app);
    let relay = entries(&app)[0].2.device.key.attachment.clone();
    source.guitar(relay.clone());
    tick(&ctx, &mut app);
    let (_, guitar, device) = entries(&app)[1];
    let old_guitar = device.device.key.clone();
    let active = app.tabs.active;
    assert!(active.is_some());

    app.apply_device_event(
        guitar,
        tabs::DeviceEvent::GuitarDisconnected(old_guitar.clone()),
    );
    let guitar_lost = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&guitar_lost.shapes, "Disconnected"));
    assert!(!has_label(&guitar_lost.shapes, "Variax 300/600 Connected"));
    assert!(has_label(&guitar_lost.shapes, "PODxt Live Connected"));
    assert!(has_label(&guitar_lost.shapes, "POD HD500X Connected"));

    source.notice(&relay, SimulatedNotice::Connected);
    tick(&ctx, &mut app);
    source.guitar(relay);
    tick(&ctx, &mut app);
    let returned_guitar = entries(&app)[1].2.device.key.clone();
    assert_ne!(returned_guitar, old_guitar);

    source.0.borrow_mut().fail = true;
    tick(&ctx, &mut app);
    let failed = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&failed.shapes, "Device check failed"));
    assert!(has_label(&failed.shapes, "PODxt Live Connected"));

    source.0.borrow_mut().fail = false;
    source.usb(false, true);
    tick(&ctx, &mut app);
    let relay_lost = frame(&ctx, &mut app, vec![]);
    assert!(!has_label(&relay_lost.shapes, "PODxt Live Connected"));
    assert!(!has_label(&relay_lost.shapes, "Variax 300/600 Connected"));
    assert_eq!(entries(&app)[1].2.connection, Connection::Disconnected);
    assert!(has_label(&relay_lost.shapes, "POD HD500X Connected"));

    source.usb(true, true);
    tick(&ctx, &mut app);
    let new_relay = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    assert_ne!(new_relay, old_guitar.attachment);
    source.guitar(new_relay);
    tick(&ctx, &mut app);
    app.apply_device_event(
        guitar,
        tabs::DeviceEvent::GuitarDisconnected(returned_guitar),
    );
    let returned = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&returned.shapes, "Reconnected after disconnect"));
    assert!(has_label(&returned.shapes, "Variax 300/600 Connected"));
    assert!(!has_label(&returned.shapes, "Disconnected"));
    assert_eq!(entries(&app)[1].1, guitar);
    assert_eq!(app.tabs.active, active);
}

#[test]
fn unsupported_only_startup_keeps_no_supported_device_summary() {
    let (ctx, mut app, source) = fixture();
    source.usb(false, true);
    tick(&ctx, &mut app);
    let started = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&started.shapes, "POD HD500X Connected"));
    assert!(has_label(&started.shapes, "Live connection not supported"));
    assert!(has_label(&started.shapes, "Open offline editor"));
    assert!(entries(&app).is_empty());
    assert!(source.0.borrow().completed.is_empty());
    assert!(
        has_label(&started.shapes, "No supported device is connected"),
        "the startup summary disappeared when only an offline-only device was present"
    );
    assert!(has_label(&started.shapes, "Edit offline"));
}

#[derive(Default)]
struct FakeSync {
    starts: usize,
    cancellations: usize,
    update: Option<tabs::SyncUpdate>,
}

struct SyncEditor {
    state: Rc<RefCell<FakeSync>>,
    total: std::num::NonZeroUsize,
    consent: bool,
}

impl FamilyTab for SyncEditor {
    fn show(&mut self, _: &mut egui::Ui, _: &mut EditorServices<'_>) {}
    fn dirty(&self) -> bool {
        false
    }
    fn close_prompt(&mut self, _: &egui::Context, _: &mut EditorServices<'_>) {}
    fn status(&self) -> (&str, Option<&str>) {
        ("", None)
    }
    fn save_copy(&mut self, _: &egui::Context, _: &mut EditorServices<'_>) {}

    fn connection_sync_admission(&self, consent: bool) -> tabs::SyncAdmission {
        if self.consent && !consent {
            tabs::SyncAdmission::Consent("Temporarily switch presets?")
        } else {
            tabs::SyncAdmission::Ready(self.total)
        }
    }

    fn start_connection_sync(&mut self, _: &egui::Context) -> Result<(), String> {
        let mut state = self.state.borrow_mut();
        state.starts += 1;
        state.update = Some(tabs::SyncUpdate::WaitingForRig);
        Ok(())
    }

    fn connection_sync_update(&mut self) -> Option<tabs::SyncUpdate> {
        let mut state = self.state.borrow_mut();
        match state.update {
            Some(tabs::SyncUpdate::Reading(count)) => Some(tabs::SyncUpdate::Reading(count)),
            Some(tabs::SyncUpdate::WaitingForRig) => Some(tabs::SyncUpdate::WaitingForRig),
            _ => state.update.take(),
        }
    }

    fn cancel_connection_sync(&mut self) {
        self.state.borrow_mut().cancellations += 1;
    }
}

#[test]
fn runtime_sync_waits_for_consent_and_shared_release_while_independent_work_continues() {
    use tabs::SyncUpdate::{Finished, Reading, WaitingForRig};

    let (ctx, mut app, source) = fixture();
    source.usb_paths(&["8-2.1", "8-4"], false);
    tick(&ctx, &mut app);
    let relay = app.detection.as_ref().unwrap().attachment_key("8-2.1");
    let independent = app.detection.as_ref().unwrap().attachment_key("8-4");
    source.guitar(relay.clone());
    source.finish(independent, SimulatedGuitar::Silent);
    tick(&ctx, &mut app);

    let mut fake = std::collections::BTreeMap::new();
    for entry in &mut app.tabs.entries {
        let Some(device) = &entry.device else {
            continue;
        };
        let key = &device.device.key;
        let is_guitar = key.family_id == "variax";
        let state = Rc::new(RefCell::new(FakeSync::default()));
        entry.editor = Box::new(SyncEditor {
            state: state.clone(),
            total: if is_guitar { 60 } else { 128 }.try_into().unwrap(),
            consent: is_guitar,
        });
        fake.insert((key.attachment.path.as_str(), key.family_id), state);
    }
    let pod = fake[&("8-2.1", "podxt")].clone();
    let guitar = fake[&("8-2.1", "variax")].clone();
    let other = fake[&("8-4", "podxt")].clone();
    drop(fake);
    app.connection_inventory = Default::default();
    app.connections = Default::default();
    tick(&ctx, &mut app);
    let window = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&window.shapes, "Temporarily switch presets?"));
    assert_eq!(guitar.borrow().starts, 0);
    assert_eq!(pod.borrow().starts, 1);
    assert_eq!(other.borrow().starts, 1);

    let guitar_attachment = app
        .connection_inventory
        .attachment_for_test("8-2.1", "variax");
    app.connection_sync_action(connections::Action::AllowDownload(guitar_attachment), &ctx);
    pod.borrow_mut().update = Some(Reading(127));
    other.borrow_mut().update = Some(Reading(17));
    let overlapping = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&overlapping.shapes, "Model Sync 127 of 128"));
    assert!(has_label(&overlapping.shapes, "Model Sync 17 of 128"));
    assert!(has_label(
        &overlapping.shapes,
        "Waiting to sync · the PODxt Live connection"
    ));

    // Reporting the last entry is still in progress. Checked release is the
    // separate success event and the queued transport can then acquire.
    pod.borrow_mut().update = Some(Reading(128));
    guitar.borrow_mut().update = Some(WaitingForRig);
    other.borrow_mut().update = Some(Reading(41));
    let finishing = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&finishing.shapes, "Model Sync 128 of 128"));
    assert!(!has_label(&finishing.shapes, "Model Sync Complete"));
    assert!(has_label(
        &finishing.shapes,
        "Waiting to sync · the PODxt Live connection"
    ));

    pod.borrow_mut().update = Some(Finished(Ok(())));
    guitar.borrow_mut().update = Some(Reading(19));
    other.borrow_mut().update = Some(Finished(Err("Partial read".into())));
    let partial = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&partial.shapes, "Model Sync Complete"));
    assert!(has_label(&partial.shapes, "Model Sync 19 of 60"));
    assert!(has_label(&partial.shapes, "Partial read"));
    assert!(has_label(&partial.shapes, "Retry"));

    let other_attachment = app.connection_inventory.attachment_for_test("8-4", "podxt");
    app.connection_sync_action(connections::Action::Retry(other_attachment), &ctx);
    guitar.borrow_mut().update = Some(Reading(23));
    frame(&ctx, &mut app, vec![]);
    assert_eq!(other.borrow().starts, 2);

    source.usb_paths(&["8-4"], false);
    guitar.borrow_mut().update = Some(Finished(Ok(())));
    other.borrow_mut().update = Some(Reading(62));
    tick(&ctx, &mut app);
    other.borrow_mut().update = Some(Reading(62));
    let lost = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&lost.shapes, "Disconnected"));
    assert!(!has_label(&lost.shapes, "Model Sync Complete"));
    assert!(has_label(&lost.shapes, "Model Sync 62 of 128"));

    app.connection_sync_action(connections::Action::Retry(guitar_attachment), &ctx);
    app.connection_sync_action(connections::Action::CancelDownload(other_attachment), &ctx);
    assert_eq!(guitar.borrow().starts, 1);
    assert_eq!(other.borrow().cancellations, 1);
    other.borrow_mut().update = Some(Finished(Err("Download cancelled.".into())));
    let cancelled = frame(&ctx, &mut app, vec![]);
    assert!(has_label(&cancelled.shapes, "Download cancelled."));
}
