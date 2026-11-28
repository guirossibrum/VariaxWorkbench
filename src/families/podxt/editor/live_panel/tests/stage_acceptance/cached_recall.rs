use super::*;

type Requests = Vec<(PodxtRequest, Vec<u8>)>;

const ROW_A: &str = "2A  Saved 2A";
const ROW_B: &str = "2B  Saved 2B";

fn identity() -> (PodxtRequest, Vec<u8>) {
    (
        PodxtRequest::Identity,
        variax_workbench::identity_stage::bridge_reply(),
    )
}

fn edited(value: u8) -> (PodxtRequest, Vec<u8>) {
    (
        PodxtRequest::QueryGlobal(podxt::Global::Edited),
        vec![0xf0, 0, 1, 0x0c, 3, 0x56, 28, 0, 0, 0, value, 0xf7],
    )
}

fn fast_requests(source: Location, target: Location) -> Requests {
    vec![
        identity(),
        recall_channel(source),
        edited(0),
        (PodxtRequest::Program(target), vec![]),
        recall_channel(target),
        edited(0),
    ]
}

fn checked_requests(
    source: Location,
    playing: &EditBuffer,
    target: Location,
    destination: &EditBuffer,
) -> Requests {
    let mut requests = recall_source(source, playing);
    requests.push((PodxtRequest::Stored(source), playing.raw.clone()));
    recall_destination(
        &mut requests,
        source,
        playing,
        target,
        destination,
        destination,
    );
    requests
}

#[test]
fn clean_sidebar_round_trip_uses_six_requests_per_switch_without_tone_dumps() {
    let ctx = egui::Context::default();
    let (a, b) = recall_pair();
    let saved_a = valid_dump("Saved 2A");
    let (mut app, _, steps, saved_b) = recall_fixture_with_observer(&ctx, &saved_a, &saved_a, true);
    let bank = app.podxt().live.snapshot.library.clone();

    for (source, target, before, after, row) in [
        (a, b, &saved_a, &saved_b, ROW_B),
        (b, a, &saved_b, &saved_a, ROW_A),
    ] {
        let (validated, ready) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        // ScriptedAction checks every actual request byte and the exact count.
        expect_action(
            &steps,
            Some(target),
            fast_requests(source, target),
            move || {
                validated.send(()).unwrap();
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                Ok(())
            },
        );
        click_text(&ctx, &mut app, row);
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        rendered(&ctx, &mut app, &[&format!("Playing · {}", source.label())]);
        assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(before));
        release.send(()).unwrap();
        finish_action(&ctx, &mut app, &steps);
        rendered(&ctx, &mut app, &[&format!("Playing · {}", target.label())]);
        assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(after));
    }
    assert_eq!(app.podxt().live.snapshot.library, bank);
    assert!(app.podxt().live.channel_draft(a).is_none());
    assert!(app.podxt().live.channel_draft(b).is_none());
}

#[test]
fn sidebar_checks_the_actual_destination_after_observed_or_missed_save() {
    let (a, b) = recall_pair();
    for missed in [false, true] {
        let ctx = egui::Context::default();
        let saved_a = valid_dump("Saved 2A");
        let (mut app, _, steps, cached_b) =
            recall_fixture_with_observer(&ctx, &saved_a, &saved_a, true);
        let panel = &app.podxt().live;
        let listener = &panel.recall_observer.as_ref().unwrap().listener;
        let _resumed = if missed {
            listener.gap();
            // A fresh listener cannot prove what happened while the rig was yielded.
            Some(panel.save_ingress.begin())
        } else {
            listener.observe(b);
            None
        };
        let addressed_b = valid_dump("New saved 2B");
        expect_action(
            &steps,
            Some(b),
            checked_requests(a, &saved_a, b, &addressed_b),
            || Ok(()),
        );
        click_text(&ctx, &mut app, ROW_B);
        finish_action(&ctx, &mut app, &steps);
        assert_eq!(app.podxt().live.playing_channel(), Some(b));
        assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&addressed_b));
        assert_eq!(app.podxt().live.stored_tone(b), Some(&cached_b));
    }
}

#[test]
fn sidebar_replacement_rejects_old_attachment_proof() {
    let ctx = egui::Context::default();
    let (a, b) = recall_pair();
    let saved_a = valid_dump("Saved 2A");
    let (mut app, presence, steps, _) =
        recall_fixture_with_observer(&ctx, &saved_a, &saved_a, true);
    let old = app.podxt().live.device.clone();
    let mut requests = recall_source(a, &saved_a);
    requests.extend([recall_packs(), recall_channel(a)]);
    expect_action(&steps, None, requests, || Ok(()));

    presence.attached.set(false);
    app.next_detection = Instant::now();
    render_stage(&ctx, &mut app, vec![]);
    presence.attached.set(true);
    app.next_detection = Instant::now();
    render_stage(&ctx, &mut app, vec![]);
    finish_action(&ctx, &mut app, &steps);
    let panel = &mut app.podxt_mut().live;
    assert_ne!(panel.device, old);
    assert!(panel.snapshot.recall_bank.is_none());
    // Even a same-looking bank on the replacement needs its own evidence.
    let replacement_b = valid_dump("Replacement 2B");
    for slot in Location::all(&[BankKind::Tones]) {
        panel.snapshot.library.insert(
            slot.address(),
            if slot == b {
                replacement_b.clone()
            } else {
                saved_a.clone()
            },
        );
    }
    panel.snapshot.library_count = Some(128);
    expect_action(
        &steps,
        Some(b),
        checked_requests(a, &saved_a, b, &replacement_b),
        || Ok(()),
    );
    click_text(&ctx, &mut app, "2B  Replacement 2B");
    finish_action(&ctx, &mut app, &steps);
    assert_eq!(
        app.podxt().live.snapshot.tone.as_ref(),
        Some(&replacement_b)
    );
}

#[test]
fn sidebar_preserves_the_changed_buffer_and_a_distinct_preexisting_draft() {
    let ctx = egui::Context::default();
    let (a, b) = recall_pair();
    let saved_a = valid_dump("Saved 2A");
    let (mut app, _, steps, saved_b) = recall_fixture_with_observer(&ctx, &saved_a, &saved_a, true);
    let mut changed = saved_a.clone();
    changed.raw[39 + usize::from(Control::Drive.cc())] = 42;

    let mut requests = vec![identity(), recall_channel(a), edited(1)];
    requests.extend(recall_source(a, &changed));
    requests.extend([
        (PodxtRequest::Stored(a), saved_a.raw.clone()),
        recall_packs(),
        recall_channel(a),
    ]);
    expect_action(&steps, Some(b), requests, || Ok(()));
    let mut continuation = recall_source(a, &changed);
    continuation.push((PodxtRequest::Stored(a), saved_a.raw.clone()));
    recall_destination(&mut continuation, a, &changed, b, &saved_b, &saved_b);
    expect_action(&steps, Some(b), continuation, || Ok(()));

    click_text(&ctx, &mut app, ROW_B);
    finish_action(&ctx, &mut app, &steps);
    let draft = podxt::TonePayload::from_dump(&changed).unwrap();
    assert_eq!(app.podxt().live.channel_draft(a), Some(&draft));
    assert_eq!(app.podxt().live.playing_channel(), Some(b));

    expect_action(&steps, Some(a), fast_requests(b, a), || Ok(()));
    click_text(&ctx, &mut app, "2A · Draft  Saved 2A");
    finish_action(&ctx, &mut app, &steps);
    assert_eq!(app.podxt().live.channel_draft(a), Some(&draft));
    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&saved_a));

    changed.raw[39 + usize::from(Control::Drive.cc())] = 57;
    // The CC arrives in the same MIDI packet as a clean flag. It still
    // forces the checked path before any Program Change.
    let mut concurrent = edited(0);
    concurrent.1.extend([0xb0, Control::Drive.cc(), 57]);
    let mut requests = vec![identity(), recall_channel(a), concurrent];
    requests.extend(recall_source(a, &changed));
    requests.extend([
        (PodxtRequest::Stored(a), saved_a.raw.clone()),
        recall_packs(),
        recall_channel(a),
    ]);
    expect_action(&steps, Some(b), requests, || Ok(()));
    click_text(&ctx, &mut app, ROW_B);
    finish_action(&ctx, &mut app, &steps);
    assert!(app.podxt().live.editor.awaiting_recall_confirmation());
    assert_eq!(app.podxt().live.channel_draft(a), Some(&draft));
    assert_eq!(app.podxt().live.snapshot.tone.as_ref(), Some(&changed));
    click_text(&ctx, &mut app, "Cancel switch");
    assert_eq!(app.podxt().live.playing_channel(), Some(a));
}

#[test]
fn sidebar_failed_validation_or_release_never_retries_program_or_publishes_cache() {
    let (a, b) = recall_pair();
    for failure in ["wrong channel", "failed release", "gap during release"] {
        let ctx = egui::Context::default();
        let saved_a = valid_dump("Saved 2A");
        let (mut app, _, steps, saved_b) =
            recall_fixture_with_observer(&ctx, &saved_a, &saved_a, true);
        let listener = Arc::clone(&app.podxt().live.recall_observer.as_ref().unwrap().listener);
        let mut requests = fast_requests(a, b);
        if failure == "wrong channel" {
            requests.truncate(4);
            requests.push(recall_channel(a));
        }
        expect_action(&steps, Some(b), requests, move || match failure {
            "failed release" => Err("USB release failed.".into()),
            "gap during release" => {
                listener.gap();
                Ok(())
            }
            _ => Ok(()),
        });

        click_text(&ctx, &mut app, ROW_B);
        finish_action(&ctx, &mut app, &steps);
        let panel = &app.podxt().live;
        assert!(panel.editor.needs_refresh());
        assert_eq!(panel.playing_channel(), Some(a));
        assert_eq!(panel.snapshot.tone.as_ref(), Some(&saved_a));
        assert!(
            panel
                .snapshot
                .recall_bank
                .as_ref()
                .unwrap()
                .slot(a)
                .is_none()
        );

        if failure == "failed release" {
            assert!(panel.pending_reply_message().is_some());
            rendered(&ctx, &mut app, &["Reconnect the PODxt Live"]);
            assert!(steps.lock().unwrap().is_empty());
            continue;
        }
        let actual = if failure == "wrong channel" { a } else { b };
        let playing = if actual == a { &saved_a } else { &saved_b };
        let mut get = recall_source(actual, playing);
        get.extend([recall_packs(), recall_channel(actual)]);
        expect_action(&steps, None, get, || Ok(()));
        click_text(&ctx, &mut app, "Read playing tone");
        finish_action(&ctx, &mut app, &steps);
        assert_eq!(app.podxt().live.playing_channel(), Some(actual));
    }
}

#[test]
fn sidebar_ambiguous_edited_reply_stops_before_program_and_blocks_late_replies() {
    let (a, b) = recall_pair();
    for problem in ["silent", "duplicate", "partial"] {
        let ctx = egui::Context::default();
        let saved_a = valid_dump("Saved 2A");
        let (mut app, _, steps, _) = recall_fixture_with_observer(&ctx, &saved_a, &saved_a, true);
        let mut reply = edited(0);
        match problem {
            "silent" => reply.1.clear(),
            "duplicate" => reply.1.extend(edited(0).1),
            _ => reply.1.extend([0xf0, 0, 1, 0x0c, 3, 0x24]),
        }
        expect_action(
            &steps,
            Some(b),
            vec![identity(), recall_channel(a), reply],
            || Ok(()),
        );

        click_text(&ctx, &mut app, ROW_B);
        finish_action(&ctx, &mut app, &steps);
        let panel = &app.podxt().live;
        assert_eq!(panel.playing_channel(), Some(a));
        assert!(panel.pending_reply_message().is_some());
        assert!(panel.editor.needs_refresh());
    }
}
