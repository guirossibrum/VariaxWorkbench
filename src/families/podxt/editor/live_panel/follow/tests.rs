use super::super::tests::{FakeLink, FakeLock, connected_fixture, stage_frame, valid_dump};
use super::*;
use podxt::{Global, PodxtRequest};
use std::{cell::Cell, collections::VecDeque};

fn buffer(name: &str, slot: u16) -> EditorBuffer {
    EditorBuffer {
        channel: Location::new(BankKind::Tones, slot).unwrap(),
        tone: valid_dump(name),
        packs: Packs(4),
    }
}

fn read_script(buffer: &EditorBuffer) -> VecDeque<(PodxtRequest, Vec<u8>)> {
    let channel = || {
        (
            PodxtRequest::QueryGlobal(Global::CurrentChannel),
            vec![
                0xf0,
                0,
                1,
                0x0c,
                3,
                0x56,
                17,
                0,
                0,
                (buffer.channel.index() >> 4) as u8,
                (buffer.channel.index() & 15) as u8,
                0xf7,
            ],
        )
    };
    VecDeque::from([
        (
            PodxtRequest::Identity,
            variax_workbench::identity_stage::bridge_reply(),
        ),
        channel(),
        (PodxtRequest::EditBuffer, buffer.tone.raw.clone()),
        (
            PodxtRequest::InstalledPacks,
            vec![0xf0, 0, 1, 0x0c, 3, 0x0e, 0, 4, 0xf7],
        ),
        channel(),
    ])
}

#[test]
fn mac_follow_settles_the_latest_program_and_publishes_after_release() {
    for checked_release_fails in [false, true] {
        let expected = buffer("Physical 32D", 127);
        let held = Cell::new(true);
        let rig = FakeLock(&held);
        let writes = Cell::new(0);
        let clock = Cell::new(Duration::ZERO);
        let pending = AtomicBool::new(false);
        let cleanup = Cell::new(false);
        let ingress = Arc::new(detection::PodxtSaveIngress::default());
        let saves = ingress.begin();
        let baseline = ingress.revision();
        let result = follow_on_link(
            || false,
            &pending,
            false,
            &saves,
            |run| {
                let mut packet = vec![0xb0, 0, 1, 0xc0, 2, 0xc0, 127];
                for (bank, slot) in [(1, 127), (2, 106), (3, 63), (1, 127)] {
                    packet.extend([0xf0, 0, 1, 12, 3, 0x24, bank, slot, 0xf7]);
                }
                let mut link = FakeLink {
                    held: &held,
                    writes: &writes,
                    time: &clock,
                    script: read_script(&expected),
                    // A bank-select CC on its own doesn't select the playing
                    // tone. Two program notices in one packet settle together.
                    reply: Some(packet),
                };
                let outcome = run(&mut link)?;
                assert!(held.get());
                assert!(clock.get() >= CHANNEL_SETTLE);
                assert!(ingress.saved_since(baseline));
                assert_eq!(ingress.revision(), baseline + 5); // four saves and one gap
                drop(link);
                if checked_release_fails {
                    Err("macOS interface release failed".into())
                } else {
                    Ok(outcome)
                }
            },
            || {
                assert!(held.get());
                cleanup.set(true);
            },
        );
        drop(rig);
        assert!(!held.get());
        assert_eq!(writes.get(), 5);
        assert_eq!(cleanup.get(), checked_release_fails);
        assert_eq!(pending.load(Ordering::Acquire), checked_release_fails);
        if checked_release_fails {
            assert_eq!(
                result.err().as_deref(),
                Some("macOS interface release failed")
            );
        } else {
            let Outcome::Changed(actual) = result.unwrap() else {
                panic!("expected settled program change");
            };
            assert_eq!(actual.program_target, Some(expected.channel));
            assert_eq!(actual.buffer.channel, expected.channel);
            assert_eq!(actual.buffer.tone, expected.tone);
        }
    }
}

#[test]
fn yielding_while_the_program_is_settling_closes_without_requesting_a_tone() {
    let held = Cell::new(true);
    let rig = FakeLock(&held);
    let writes = Cell::new(0);
    let clock = Cell::new(Duration::ZERO);
    let pending = AtomicBool::new(false);
    let result = follow_on_link(
        || clock.get() >= CHANNEL_SETTLE * 2,
        &pending,
        false,
        &Arc::new(detection::PodxtSaveIngress::default()).begin(),
        |run| {
            run(&mut FakeLink {
                held: &held,
                writes: &writes,
                time: &clock,
                script: VecDeque::new(),
                reply: Some(vec![0xc0, 6]),
            })
        },
        || panic!("normal yield released cleanly"),
    );
    drop(rig);
    assert!(matches!(result, Ok(Outcome::Stopped)));
    assert_eq!(writes.get(), 0);
    assert!(!pending.load(Ordering::Acquire));
}

fn program(buffer: EditorBuffer) -> Outcome {
    Outcome::Changed(Update {
        program_target: Some(buffer.channel),
        buffer,
    })
}

fn completion(panel: &mut LivePanel, result: Outcome) {
    let thread = std::thread::spawn(move || Ok(result));
    while !thread.is_finished() {
        std::thread::yield_now();
    }
    panel.follow.worker = Some(Worker {
        device: panel.device.clone().expect("connected fixture"),
        saves: Arc::new(panel.save_ingress.begin()),
        stop: Arc::default(),
        thread: Some(thread),
    });
}

#[test]
fn startup_and_foreground_work_take_priority_and_stale_results_are_discarded() {
    let ctx = egui::Context::default();
    let (app, _) = connected_fixture(&ctx);
    let device = app.tabs.entries[0].device.as_ref();
    let mut panel = LivePanel::default();
    assert!(panel.set_device(device));
    assert!(panel.resume_live);
    let initial = buffer("Initial", 0);
    panel.editor.accept_follow(initial.clone());
    panel.accept_editor_read(initial, &ctx);
    assert!(panel.live_editing);
    assert!(!panel.busy());
    panel.startup = Some(Startup::Tones);
    completion(&mut panel, program(buffer("Stale during startup", 11)));
    assert!(!panel.busy(), "Follow cannot block startup admission");
    panel.poll_follow(&ctx);
    assert_eq!(panel.playing_channel(), Location::tone_from_label("1A"));

    panel.startup = None;
    completion(&mut panel, Outcome::Yielded);
    panel.poll_follow(&ctx);
    assert!(panel.follow.reconcile);
    assert!(panel.follow.retry_at.is_some());
    assert!(!panel.busy(), "foreground work may acquire the rig");

    completion(&mut panel, program(buffer("Stale after detach", 12)));
    panel.set_device(None);
    panel.set_device(device);
    panel.poll_follow(&ctx);
    assert!(panel.snapshot.tone.is_none());
    assert!(!panel.live_editing);
    assert_eq!(panel.startup, Some(Startup::Current));
}

#[test]
fn revoked_observation_discards_a_checked_follow_result_on_the_same_attachment() {
    let ctx = egui::Context::default();
    let (app, _) = connected_fixture(&ctx);
    let mut panel = LivePanel::default();
    assert!(panel.set_device(app.tabs.entries[0].device.as_ref()));
    let initial = buffer("Initial", 0);
    panel.editor.accept_follow(initial.clone());
    panel.accept_editor_read(initial.clone(), &ctx);
    panel.startup = None;
    completion(&mut panel, program(buffer("Revoked", 127)));
    panel.save_ingress.interrupt();
    assert!(panel.follow.worker.is_some());
    panel.poll_follow(&ctx);
    assert_eq!(panel.playing_channel(), Some(initial.channel));
    assert!(panel.follow.worker.is_none());
}

#[test]
fn save_during_a_download_rejects_the_superseded_result() {
    let ctx = egui::Context::default();
    let (app, _) = connected_fixture(&ctx);
    let mut panel = LivePanel::default();
    assert!(panel.set_device(app.tabs.entries[0].device.as_ref()));
    panel.startup = None;
    let slot = Location::tone_from_label("1B").unwrap();
    let original = valid_dump("Previous library");
    panel
        .snapshot
        .library
        .insert(slot.address(), original.clone());

    let listener = panel.save_ingress.begin();
    let captured = Snapshot {
        library: Location::all(&[BankKind::Tones])
            .map(|slot| (slot.address(), valid_dump("Superseded download")))
            .collect(),
        library_count: Some(128),
        library_save_baseline: panel.save_ingress.revision(),
        ..Default::default()
    };
    let thread = std::thread::spawn(|| Ok(()));
    while !thread.is_finished() {
        std::thread::yield_now();
    }
    panel.worker = Some(super::super::Worker {
        command: ReadCommand::ToneBank,
        device: panel.device.clone().unwrap(),
        stop: Arc::default(),
        acquired: Some(Arc::new(AtomicBool::new(true))),
        snapshot: Arc::new(Mutex::new(captured)),
        thread: Some(thread),
    });
    listener.observe(slot);
    panel.poll(&ctx);
    assert!(matches!(
        panel.connection_sync_update(),
        Some(SyncUpdate::Finished(Err(_)))
    ));
    assert_eq!(panel.stored_tone(slot), Some(&original));
}

#[test]
fn edit_mode_preserves_staged_values_and_ignores_a_completed_follow() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    let initial = buffer("Edit base", 17);
    panel.editor.accept_follow(initial.clone());
    panel.accept_editor_read(initial.clone(), &ctx);
    completion(panel, program(buffer("Stale change", 21)));
    panel.enter_edit();
    panel.editor.stage_test_target(podxt::LiveTarget::Basic(
        Control::Drive,
        podxt::Target::Value(podxt::Value::new(67).unwrap()),
    ));
    panel.poll_follow(&ctx);
    assert_eq!(panel.snapshot.tone.as_ref(), Some(&initial.tone));
    assert_eq!(panel.playing_channel(), Some(initial.channel));
    assert!(panel.has_staged_edits());
    let requests = panel.device_requests;
    panel.enter_live(&ctx);
    panel.recall(Location::tone_from_label("6B").unwrap(), &ctx);
    assert!(!panel.resume_live);
    assert_eq!(panel.device_requests, requests + 1); // only the explicit recall attempt
    panel.set_device(None);
    assert!(panel.has_staged_edits());
    assert_eq!(panel.snapshot.tone.as_ref(), Some(&initial.tone));
    panel.discard_staged();
    assert!(!panel.has_staged_edits());
}

#[test]
fn physical_follow_moves_the_library_view_to_the_verified_playing_channel() {
    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    panel.resume_live = true;
    panel.startup = None;
    let initial = buffer("Initial 1A", 0);
    panel.editor.accept_follow(initial.clone());
    panel.accept_editor_read(initial, &ctx);
    for slot in Location::all(&[BankKind::Tones]) {
        panel
            .snapshot
            .library
            .insert(slot.address(), valid_dump("Saved"));
    }
    panel.snapshot.library.insert(
        Location::tone_from_label("32D").unwrap().address(),
        valid_dump("Last Saved Row"),
    );
    completion(panel, program(buffer("Physical 32D", 127)));
    let mut visible = String::new();
    for _ in 0..10 {
        visible = stage_frame(&ctx, &mut app);
    }
    assert!(visible.contains("Channel 32D"), "{visible}");
    assert!(visible.contains("Physical 32D"), "{visible}");
    assert!(visible.contains("Last Saved Row"), "{visible}");
    assert_eq!(
        app.podxt().live.playing_channel(),
        Location::tone_from_label("32D")
    );
}

#[test]
fn library_click_routes_to_checked_recall_in_live_and_scroll_never_recalls() {
    use super::super::tests::render_stage;

    fn center(shape: &egui::Shape, label: &str) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| center(shape, label)),
            _ => None,
        }
    }

    let ctx = egui::Context::default();
    let (mut app, _) = connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    panel.resume_live = true;
    panel.startup = None;
    let playing = buffer("Initial", 0);
    panel.editor.accept_follow(playing.clone());
    panel.accept_editor_read(playing, &ctx);
    let slot = Location::tone_from_label("1B").unwrap();
    panel
        .snapshot
        .library
        .insert(slot.address(), valid_dump("Recall Target"));
    let requests = panel.device_requests;
    stage_frame(&ctx, &mut app);
    let frame = render_stage(&ctx, &mut app, vec![]);
    let point = frame
        .shapes
        .iter()
        .find_map(|shape| {
            center(&shape.shape, "1B  Recall Target")
                .filter(|point| shape.clip_rect.contains(*point))
        })
        .expect("visible stored tone row");
    render_stage(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -30.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert_eq!(app.podxt().live.device_requests, requests);
    for pressed in [true, false] {
        render_stage(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(app.podxt().live.device_requests, requests + 1);
    // The checked recall's result, rather than the click, changes the channel.
    assert_eq!(
        app.podxt().live.playing_channel(),
        Location::tone_from_label("1A")
    );
}

#[test]
fn linux_program_reuses_detection_and_waits_for_the_managed_startup_download() {
    let ctx = egui::Context::default();
    let (mut app, presence) = connected_fixture(&ctx);
    let panel = &mut app.podxt_mut().live;
    panel.resume_live = true;
    let initial = buffer("Initial 1A", 0);
    panel.editor.accept_follow(initial.clone());
    panel.accept_editor_read(initial, &ctx);
    let observed_at = Instant::now();
    panel.live_since = Some(observed_at - Duration::from_secs(1));
    panel.startup = Some(Startup::Tones);
    let previous_requests = panel.device_requests;
    presence
        .notices
        .borrow_mut()
        .push(detection::SimulatedNotice::PodxtProgram(
            detection::SimulatedPodxtProgram {
                channel: 127,
                observed_at: observed_at - CHANNEL_SETTLE,
            },
        ));
    app.next_detection = Instant::now();
    stage_frame(&ctx, &mut app);

    // The real shell detector delivered the same-attachment notice. The
    // startup placeholder reserves priority without counting Follow as busy.
    let mut panel = std::mem::take(&mut app.podxt_mut().live);
    let device = app.tabs.entries[0].device.as_ref().unwrap();
    assert!(panel.linux_follow_due.is_some());
    assert_eq!(panel.device_requests, previous_requests);
    assert!(!panel.busy());

    let (release, wait_for_release) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        wait_for_release
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        Ok(())
    });
    let acquired = Arc::new(AtomicBool::new(false));
    let mut captured = Snapshot {
        library_count: Some(128),
        ..Default::default()
    };
    for location in Location::all(&[BankKind::Tones]) {
        captured
            .library
            .insert(location.address(), valid_dump("Managed download"));
    }
    panel.startup = None;
    panel.worker = Some(super::super::Worker {
        command: ReadCommand::ToneBank,
        device: device.device.key.clone(),
        stop: Arc::default(),
        acquired: Some(Arc::clone(&acquired)),
        snapshot: Arc::new(Mutex::new(captured)),
        thread: Some(thread),
    });
    assert!(matches!(
        panel.connection_sync_update(),
        Some(SyncUpdate::WaitingForRig)
    ));
    acquired.store(true, Ordering::Release);
    assert!(matches!(
        panel.connection_sync_update(),
        Some(SyncUpdate::Reading(128))
    ));
    panel.poll(&ctx);
    assert_eq!(panel.device_requests, previous_requests);
    assert!(panel.linux_follow_due.is_some());
    assert_ne!(
        panel
            .stored_tone(Location::tone_from_label("1A").unwrap())
            .map(EditBuffer::name)
            .as_deref(),
        Some("Managed download")
    );

    release.send(()).unwrap();
    while !panel
        .worker
        .as_ref()
        .unwrap()
        .thread
        .as_ref()
        .unwrap()
        .is_finished()
    {
        std::thread::yield_now();
    }
    panel.poll(&ctx);
    assert!(matches!(
        panel.connection_sync_update(),
        Some(SyncUpdate::Finished(Ok(())))
    ));
    assert_eq!(panel.device_requests, previous_requests);
    assert_eq!(panel.snapshot.library.len(), 128);

    panel.editor.stage_test_target(podxt::LiveTarget::Basic(
        Control::Drive,
        podxt::Target::Value(podxt::Value::new(67).unwrap()),
    ));
    panel.poll_follow(&ctx);
    assert!(panel.linux_follow_due.is_some());
    assert_eq!(panel.device_requests, previous_requests);
    panel.editor.discard_staged();

    panel.poll_follow(&ctx);
    assert_eq!(panel.device_requests, previous_requests + 1);
    assert!(panel.linux_follow_due.is_none());
    assert_eq!(panel.playing_channel(), Location::tone_from_label("1A"));

    // The notification alone cannot publish a tone. The checked read supplies it.
    panel.accept_editor_read(buffer("Verified 32D", 127), &ctx);
    assert_eq!(panel.playing_channel(), Location::tone_from_label("32D"));
    assert!(panel.take_playing_reveal());
    panel.enter_edit();
    assert!(!panel.set_device(Some(device)));
    panel.poll_follow(&ctx);
    assert_eq!(panel.device_requests, previous_requests + 1);
    panel.resume_live = true;
    panel.accept_editor_read(buffer("Return to Live", 2), &ctx);
    assert!(!panel.set_device(Some(device)));
    assert!(
        panel.linux_follow_due.is_none(),
        "old notices cannot cross a mode change"
    );
}

#[test]
fn closing_follow_waits_for_the_worker_to_release_the_link() {
    let stop = Arc::new(AtomicBool::new(false));
    let released = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let worker_released = Arc::clone(&released);
    let thread = std::thread::spawn(move || {
        while !worker_stop.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        worker_released.store(true, Ordering::Release);
        Ok(Outcome::Stopped)
    });
    let ctx = egui::Context::default();
    let (app, _) = connected_fixture(&ctx);
    let panel = &app.podxt().live;
    drop(Worker {
        device: panel.device.clone().expect("connected fixture"),
        saves: Arc::new(panel.save_ingress.begin()),
        stop,
        thread: Some(thread),
    });
    assert!(released.load(Ordering::Acquire));
}
