use super::*;
use crate::rig_check::practice::Faults;
use crate::rig_check::run_practice;
use crate::rig_check::runner::NoReport;
use crate::rig_check::steps::RunData;
use eframe::egui::{Event, Key, Modifiers, Pos2, RawInput, Rect};

fn frame(ctx: &egui::Context, wizard: &mut Wizard, events: Vec<Event>) -> Vec<String> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 800.0))),
        events,
        ..Default::default()
    };
    let output = ctx.run(input, |ctx| wizard.show(ctx));
    labels(&output).into_iter().map(|(text, _)| text).collect()
}

fn labels(output: &egui::FullOutput) -> Vec<(String, Pos2)> {
    fn walk(shape: &egui::Shape, texts: &mut Vec<(String, Pos2)>) {
        match shape {
            egui::Shape::Text(text) => texts.push((
                text.galley.text().to_owned(),
                text.pos + text.galley.rect.center().to_vec2(),
            )),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, texts)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    output
        .shapes
        .iter()
        .for_each(|s| walk(&s.shape, &mut texts));
    texts
}

fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn wired(prompt: Prompt) -> (Wizard, std::sync::mpsc::Receiver<Reply>) {
    let (tx, rx) = channel();
    let wizard = Wizard {
        mode: Some(Mode::Practice),
        prompt: Some(prompt),
        to_runner: Some(tx),
        ..Wizard::default()
    };
    (wizard, rx)
}

#[test]
fn the_step_card_shows_its_sections_and_enter_skips() {
    let ctx = egui::Context::default();
    apply_theme(&ctx);
    let (mut wizard, rx) = wired(Prompt::Card(StepId(2, 1), Mode::Practice, None));
    let texts = frame(&ctx, &mut wizard, Vec::new());
    for expected in [
        "Step 5 of 37",
        "Catalogue ID 2.1",
        "What it does",
        "What you'll see and hear",
        "Risk",
        "SEE ",
        "HEAR ",
        "Skip  (S)",
        "Run  (R)",
    ] {
        assert!(
            texts.iter().any(|t| t == expected),
            "{expected} in {texts:?}"
        );
    }
    // Focus starts on Skip: Enter presses it (the default is not to run).
    frame(&ctx, &mut wizard, vec![key(Key::Enter)]);
    frame(&ctx, &mut wizard, Vec::new());
    assert!(matches!(rx.try_recv(), Ok(Reply::Decision(Decision::Skip))));
}

#[test]
fn cards_follow_run_order_after_firmware_checks_are_skipped() {
    let ctx = egui::Context::default();
    let (to_ui, from_runner) = channel();
    let mut wizard = Wizard {
        mode: Some(Mode::Practice),
        from_runner: Some(from_runner),
        ..Wizard::default()
    };
    let mut podxt = StepRecord::new(catalogue::PODXT_COMPATIBILITY);
    podxt.outcome = Outcome::verified_firmware_skip("PODxt Live", "3.01");
    to_ui.send(Prompt::Finished(Box::new(podxt))).unwrap();
    let mut variax = StepRecord::new(catalogue::VARIAX_EVIDENCE);
    variax.outcome = Outcome::verified_firmware_skip("Variax", "3.10");
    variax.reused = true;
    to_ui.send(Prompt::Reused(Box::new(variax))).unwrap();

    for (id, ordinal, catalogue_id) in [
        (StepId(7, 5), "Step 19 of 35", "Catalogue ID 7.5"),
        (StepId(7, 3), "Step 20 of 35", "Catalogue ID 7.3"),
        (StepId(7, 4), "Step 21 of 35", "Catalogue ID 7.4"),
        (StepId(4, 7), "Step 29 of 35", "Catalogue ID 4.7"),
    ] {
        to_ui.send(Prompt::Card(id, Mode::Practice, None)).unwrap();
        let texts = frame(&ctx, &mut wizard, Vec::new());
        assert!(texts.iter().any(|line| line == ordinal), "{texts:?}");
        assert!(texts.iter().any(|line| line == catalogue_id), "{texts:?}");
        assert!(matches!(wizard.prompt, Some(Prompt::Card(active, ..)) if active == id));
    }
    assert_eq!(wizard.progress[0].id, catalogue::PODXT_COMPATIBILITY);
    assert_eq!(wizard.progress[1].id, catalogue::VARIAX_EVIDENCE);
    assert!(wizard.progress[1].reused);
}

#[test]
fn running_and_result_keep_the_card_position_across_an_attempt() {
    let ctx = egui::Context::default();
    let step = StepId(4, 7);
    let (mut wizard, replies) = wired(Prompt::Card(step, Mode::Practice, None));
    let (to_ui, from_runner) = channel();
    wizard.from_runner = Some(from_runner);
    let mut skipped = StepRecord::new(StepId(7, 4));
    skipped.outcome = Outcome::SkippedByPerson;
    to_ui.send(Prompt::Finished(Box::new(skipped))).unwrap();
    let texts = frame(&ctx, &mut wizard, Vec::new());
    assert!(texts.iter().any(|line| line == "Step 31 of 37"));
    frame(&ctx, &mut wizard, vec![key(Key::R)]);
    assert!(matches!(
        replies.try_recv(),
        Ok(Reply::Decision(Decision::Run))
    ));
    let texts = frame(&ctx, &mut wizard, Vec::new());
    for expected in ["Step 31 of 37", "Running", "Catalogue ID 4.7"] {
        assert!(texts.iter().any(|line| line == expected), "{texts:?}");
    }

    let mut record = StepRecord::new(step);
    record.attempt = Some(2);
    record.started = Some("practice".into());
    record.outcome = Outcome::SkippedNotApplicable("Discovered after Run".into());
    to_ui.send(Prompt::Finished(Box::new(record))).unwrap();
    let texts = frame(&ctx, &mut wizard, Vec::new());
    for expected in ["Step 31 of 37", "Result · attempt 2", "Catalogue ID 4.7"] {
        assert!(texts.iter().any(|line| line == expected), "{texts:?}");
    }
    assert!(replies.try_recv().is_err());
}

#[test]
fn keyboard_shortcuts_run_and_stop() {
    let ctx = egui::Context::default();
    let (mut wizard, rx) = wired(Prompt::Card(StepId(2, 1), Mode::Practice, None));
    frame(&ctx, &mut wizard, Vec::new());
    frame(&ctx, &mut wizard, vec![key(Key::R)]);
    assert!(matches!(rx.try_recv(), Ok(Reply::Decision(Decision::Run))));
    assert_eq!(wizard.running, Some(StepId(2, 1)));
    let (mut wizard, rx) = wired(Prompt::Card(StepId(2, 1), Mode::Practice, None));
    frame(&ctx, &mut wizard, Vec::new());
    frame(&ctx, &mut wizard, vec![key(Key::Q)]);
    assert!(matches!(rx.try_recv(), Ok(Reply::Decision(Decision::Stop))));
}

#[test]
fn the_start_notice_defaults_to_quit_and_save_needs_the_exact_text() {
    let ctx = egui::Context::default();
    let (mut wizard, rx) = wired(Prompt::Start("notice".into()));
    frame(&ctx, &mut wizard, Vec::new());
    frame(&ctx, &mut wizard, vec![key(Key::Enter)]);
    frame(&ctx, &mut wizard, Vec::new());
    assert!(matches!(rx.try_recv(), Ok(Reply::Accept(false))));
    let (mut wizard, rx) = wired(Prompt::Save {
        text: "save".into(),
        confirmation: "STORE VARIAX Spank 1".into(),
    });
    let texts = frame(&ctx, &mut wizard, Vec::new());
    assert!(
        texts
            .iter()
            .any(|t| t == "Type STORE VARIAX Spank 1 to save:")
    );
    frame(&ctx, &mut wizard, vec![key(Key::Enter)]);
    frame(&ctx, &mut wizard, Vec::new());
    assert!(
        matches!(rx.try_recv(), Ok(Reply::Typed(None))),
        "Enter skips the save"
    );
}

#[test]
fn the_channel_operator_runs_practice_mode_from_another_thread() {
    let (to_ui, from_runner) = channel();
    let (to_runner, from_ui) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let op_stop = stop.clone();
    // A deliberate long walk to the rig, measured by the fake link.
    let guided_delay = super::super::guided::READY_DELAY + Duration::from_secs(40);
    let worker = std::thread::spawn(move || {
        let mut op = ChannelOperator {
            to_ui,
            from_ui,
            stop: op_stop,
            repaint: None,
        };
        let (run, _, rig) = run_practice(
            Faults {
                guided_action_delay: Some(guided_delay),
                ..Faults::default()
            },
            &mut op,
            &mut NoReport,
            RunData::default(),
        );
        let simulated_elapsed = rig.borrow().clock;
        (run.count("passed"), simulated_elapsed)
    });
    // The test plays the person: accept, Run every card, type every
    // confirmation, answer Yes.
    let mut cards = 0;
    let mut results = 0;
    let ctx = egui::Context::default();
    // The worker owns the sender. Channel closure, rather than wall time,
    // signals that the simulated run finished.
    while let Ok(prompt) = from_runner.recv() {
        let reply = match prompt {
            Prompt::Start(_) => Reply::Accept(true),
            Prompt::Card(..) => {
                cards += 1;
                Reply::Decision(Decision::Run)
            }
            Prompt::Save { confirmation, .. } => Reply::Typed(Some(confirmation)),
            Prompt::PrepareAction { .. } => Reply::Decision(Decision::Run),
            Prompt::NotSeen { .. } => Reply::Retry(Retry::Skip),
            Prompt::Ask(_) => Reply::Answer(Answer::Yes, String::new()),
            Prompt::Finished(record) if record.started.is_some() => {
                results += 1;
                assert!(
                    from_runner.try_recv().is_err(),
                    "the runner must wait at {}",
                    record.id
                );
                let (mut wizard, replies) = wired(Prompt::Finished(record.clone()));
                let texts = frame(&ctx, &mut wizard, Vec::new());
                assert!(texts.iter().any(|s| s == record.outcome.label()));
                let expected: &[&str] = match record.id {
                    StepId(0, 1) => &["Practice Linux (simulated)"],
                    StepId(0, 2) => &["USB devices: 2", "Port 1-2: PODxt Live"],
                    StepId(0, 3) => &["Synthetic preset (full pipeline)", "48 of 48 identical"],
                    StepId(1, 1) => &["PODxt Live found on port 1-2"],
                    StepId(1, 2) => &["Guitar interface opened and given back", "snd-usb-variax"],
                    StepId(2, 1) => &["PODxt Live, firmware 3.01 (verified device)"],
                    StepId(2, 2) => &["Current tone: Practice 5B", "Does the PODxt Live display"],
                    StepId(2, 3) => &[
                        "Channel 1A: Practice 1A",
                        "does its display show the same name?",
                    ],
                    StepId(2, 5) => &["Read 299 of 299 entries"],
                    StepId(2, 6) => &["Editor Get: 5B", "global values answered"],
                    StepId(3, 1) => &["Variax 300/600", "firmware 3.10 (verified device)"],
                    StepId(3, 2) => &["Preset name: \"Spank 1\""],
                    StepId(3, 3) => &["Progress 35 of 35 chunks", "checksum OK"],
                    StepId(3, 4) => &["Read 60 of 60 presets", "Back on Spank 1"],
                    StepId(3, 5) => &["Read Spank 2: matches the backup", "Back on Spank 1"],
                    StepId(4, 1) => &[
                        "Stomp off to on: read back as sent",
                        "The whole tone matches the start",
                    ],
                    StepId(4, 2) => &["Sent and read back: all 1,092 words identical"],
                    StepId(4, 3) => &[
                        "Practice test model (drop D): you answered Yes",
                        "Back on Spank 1",
                    ],
                    StepId(4, 4 | 5) => {
                        &["working buffer was verified", "original program verified"]
                    }
                    StepId(4, 6) => &["whole tone read back as sent"],
                    StepId(4, 7) => &["verified the return to 5B"],
                    StepId(4, 8) => &["The tuner is off"],
                    StepId(5, 1) => &["Saved and verified Spank 1"],
                    StepId(5, 2) => &["Saved and verified channel 5B"],
                    StepId(5, 3) => &[
                        "Saved and verified the changed output trim",
                        "original backup verified",
                    ],
                    StepId(5, 4) => &[
                        "Synced and verified Spank 1",
                        "Synced and verified Spank 2",
                        "Back on Spank 1",
                    ],
                    StepId(6, 1) => &[
                        "Disconnect detected",
                        "reported that the guitar reconnected",
                        "VDI restored. The guitar's identity matches step 3.1 (firmware 3.10).",
                    ],
                    StepId(6, 2) => &[
                        "the PODxt Live tone is the starting tone",
                        "the Variax plays the starting program",
                        "snd-usb-variax",
                    ],
                    StepId(7, 1) => &["Spank 2 plays the program", "Back on Spank 1"],
                    StepId(7, 2) => &["volume 0-127, tone 0-127"],
                    StepId(7, 3) => &["Practice 5C", "Back on channel 5B"],
                    StepId(7, 4) => &["Pedal range seen: 0-127"],
                    StepId(7, 5) => &["Drive (CC 13): 0-127", "Channel Volume (CC 17): 0-127"],
                    StepId(8, 1) => &[
                        "Audio system: ALSA",
                        "PODxt Live audio device: card 1",
                        "No audio stream was opened",
                    ],
                    other => panic!("Missing SEE expectation for {other}"),
                };
                for value in expected {
                    assert!(
                        texts.iter().any(|line| line.contains(value)),
                        "{} should show {value}: {texts:?}",
                        record.id
                    );
                }
                for line in &texts {
                    assert!(
                        !line.contains("0x") && !line.to_ascii_lowercase().contains("f0 00 01 0c")
                    );
                    assert!(!line.split_whitespace().any(
                        |word| word.len() >= 64 && word.bytes().all(|b| b.is_ascii_hexdigit())
                    ));
                }
                assert!(texts.iter().any(|s| s == "Continue"));
                assert!(texts.iter().any(|s| s == "Stop"));
                assert!(from_runner.try_recv().is_err());
                assert!(replies.try_recv().is_err());
                frame(&ctx, &mut wizard, vec![key(Key::Enter)]);
                replies
                    .try_recv()
                    .expect("Enter continues from the focused button")
            }
            other if !other.needs_reply() => continue,
            other => panic!("Unexpected prompt awaiting a reply: {other:?}"),
        };
        to_runner.send(reply).unwrap();
    }
    drop(to_runner);
    let (passed, elapsed) = worker.join().unwrap();
    assert!(elapsed >= guided_delay);
    assert_eq!((passed, cards, results), (35, 35, 35));
}

#[test]
fn different_opens_a_field_and_enter_records_exact_text() {
    let ctx = egui::Context::default();
    let mut record = StepRecord::new(StepId(2, 2));
    record.started = Some("practice".into());
    record.outcome = Outcome::Passed;
    record.look_at("Compare the display.", "Practice 5B");
    let (mut wizard, replies) = wired(Prompt::Finished(Box::new(record)));
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 800.0))),
            ..Default::default()
        },
        |ctx| wizard.show(ctx),
    );
    let position = labels(&output)
        .into_iter()
        .find(|(label, _)| label == "Different…")
        .unwrap()
        .1;
    let pointer = |pressed| Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(
        &ctx,
        &mut wizard,
        vec![Event::PointerMoved(position), pointer(true)],
    );
    frame(&ctx, &mut wizard, vec![pointer(false)]);
    let texts = frame(&ctx, &mut wizard, Vec::new());
    assert!(
        texts
            .iter()
            .any(|s| s == "Type exactly what the device shows.")
    );
    assert!(replies.try_recv().is_err());
    frame(
        &ctx,
        &mut wizard,
        vec![Event::Text("  A different tone  ".into())],
    );
    frame(&ctx, &mut wizard, vec![key(Key::Enter)]);
    assert!(
        matches!(replies.try_recv(), Ok(Reply::Different(value)) if value == "  A different tone  ")
    );
}

#[test]
fn running_shows_latest_action_count_time_and_playing_notice() {
    let ctx = egui::Context::default();
    let (tx, rx) = channel();
    let mut wizard = Wizard {
        mode: Some(Mode::Practice),
        running: Some(StepId(3, 3)),
        step_start: Some(Instant::now() - Duration::from_secs(10)),
        from_runner: Some(rx),
        ..Wizard::default()
    };
    tx.send(Prompt::Progress(Progress::action(
        "Checking the connection",
    )))
    .unwrap();
    tx.send(Prompt::Progress(Progress::counted(
        "Reading words 384–415",
        12,
        35,
        "reads",
    )))
    .unwrap();
    let texts = frame(&ctx, &mut wizard, Vec::new());
    assert!(texts.iter().any(|line| line == "Reading words 384–415"));
    assert!(texts.iter().any(|line| line == "12 of 35 reads"));
    assert!(texts.iter().any(|line| line.starts_with("Elapsed ")));
    assert!(texts.iter().any(|line| line == CONTROLS_NOTICE));
    assert!(!texts.iter().any(|line| line == "Checking the connection"));
    let updated = Instant::now();
    wizard.activity = Some((
        Progress::waiting(
            "Waiting for the guitar to confirm, up to 15 s",
            Duration::from_secs(15),
            false,
        ),
        updated,
    ));
    for (elapsed, label) in [(0, "15 s left"), (9, "6 s left"), (15, "0 s left")] {
        let output = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                wizard.live_progress_at(ui, updated + Duration::from_secs(elapsed));
            });
        });
        assert!(
            labels(&output).iter().any(|(text, _)| text == label),
            "{label}"
        );
    }
}

#[test]
fn mockups_render_headless() {
    let states = mockup_states();
    assert_eq!(states.len(), 14);
    let (_, mut wizard) = mockup_states().into_iter().nth(2).unwrap();
    let image = raster::render(400, 300, |ctx| wizard.show(ctx));
    let mut colours = std::collections::BTreeSet::new();
    for pixel in image.pixels() {
        colours.insert(pixel.0);
    }
    assert!(
        colours.len() > 20,
        "text and panels were drawn: {}",
        colours.len()
    );
}

#[test]
fn laptop_mockups_show_run_position_and_catalogue_id_at_both_scales() {
    let (width, height) = LAPTOP_MOCKUP_SIZE;
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(width as f32, height as f32));
    for (_, scale) in LAPTOP_MOCKUP_SCALES {
        for ((name, mut wizard), (ordinal, id, control)) in
            laptop_mockup_states().into_iter().zip([
                ("Step 16 of 35", "Catalogue ID 6.1", "Start  (R)"),
                ("Step 16 of 35", "Catalogue ID 6.1", "OK"),
                ("Step 21 of 35", "Catalogue ID 7.4", "Start  (R)"),
                ("Step 29 of 35", "Catalogue ID 4.7", "Run  (R)"),
            ])
        {
            let ctx = egui::Context::default();
            apply_theme(&ctx);
            let mut input = RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .native_pixels_per_point = Some(scale as f32);
            let output = ctx.run(input, |ctx| wizard.show(ctx));
            assert_eq!(output.pixels_per_point, scale as f32);
            let labels = labels(&output);
            for expected in [ordinal, id, control] {
                assert!(
                    labels
                        .iter()
                        .any(|(text, position)| text == expected && screen.contains(*position)),
                    "{name} at {scale}× should show {expected:?}"
                );
            }
        }
    }
    let (_, mut wizard) = laptop_mockup_states().into_iter().next().unwrap();
    let retina = raster::render_scaled(width, height, 2, |ctx| {
        assert_eq!(ctx.pixels_per_point(), 2.0);
        wizard.show(ctx);
    });
    assert_eq!(retina.dimensions(), (width * 2, height * 2));
}

#[test]
fn clicking_a_finished_step_replays_it_without_continuing_the_active_result() {
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(1180.0, 800.0));
    let mut current = StepRecord::new(StepId(2, 3));
    current.started = Some("practice".into());
    current.outcome = Outcome::Passed;
    current.show("Channel 1A: Practice 1A");
    let (mut wizard, rx) = wired(Prompt::Finished(Box::new(current.clone())));
    let mut previous = StepRecord::new(StepId(2, 1));
    previous.outcome = Outcome::Passed;
    previous.show("PODxt Live, firmware 3.01 (verified device)");
    wizard.progress.extend([previous, current]);
    let output = ctx.run(
        RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        },
        |ctx| wizard.show(ctx),
    );
    let position = labels(&output)
        .into_iter()
        .find(|(label, _)| label.starts_with("2.1 PODxt Live identity"))
        .unwrap()
        .1;
    let pointer = |pressed| Event::PointerButton {
        pos: position,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    frame(
        &ctx,
        &mut wizard,
        vec![Event::PointerMoved(position), pointer(true)],
    );
    let replay = frame(&ctx, &mut wizard, vec![pointer(false)]);
    assert!(
        replay
            .iter()
            .any(|line| line == "PODxt Live, firmware 3.01 (verified device)")
    );
    assert!(replay.iter().any(|line| line == "Return to current step"));
    assert!(rx.try_recv().is_err());

    frame(&ctx, &mut wizard, vec![key(Key::Enter)]);
    let resumed = frame(&ctx, &mut wizard, Vec::new());
    assert!(resumed.iter().any(|line| line == "Channel 1A: Practice 1A"));
    assert!(rx.try_recv().is_err());
}

#[test]
fn stop_at_a_result_offers_the_guard_without_offering_another_step() {
    let (to_ui, from_runner) = channel();
    let (to_runner, from_ui) = channel();
    let worker = std::thread::spawn(move || {
        let mut op = ChannelOperator {
            to_ui,
            from_ui,
            stop: Arc::new(AtomicBool::new(false)),
            repaint: None,
        };
        run_practice(
            Faults::default(),
            &mut op,
            &mut NoReport,
            RunData::default(),
        )
        .0
    });
    let next = || {
        from_runner
            .recv()
            .expect("practice runner sends the next prompt")
    };
    assert!(matches!(next(), Prompt::Start(_)));
    to_runner.send(Reply::Accept(true)).unwrap();
    assert!(matches!(next(), Prompt::Card(StepId(0, 1), ..)));
    to_runner.send(Reply::Decision(Decision::Run)).unwrap();
    assert!(matches!(next(), Prompt::Progress(progress) if !progress.action.is_empty()));
    assert!(matches!(next(), Prompt::Finished(record) if record.id == StepId(0, 1)));
    to_runner.send(Reply::Continue(false)).unwrap();
    assert!(matches!(next(), Prompt::Card(catalogue::RIG_GUARD, ..)));
    to_runner.send(Reply::Decision(Decision::Skip)).unwrap();
    let run = worker.join().unwrap();
    assert_eq!(run.stopped_at, Some(StepId(0, 1)));
    assert_eq!(run.step(StepId(0, 2)).outcome, Outcome::NotReached);
}

#[test]
fn backspace_reviews_previous_results_and_c_resumes_the_same_card() {
    let ctx = egui::Context::default();
    let pending = StepId(2, 2);
    let (mut wizard, replies) = wired(Prompt::Card(pending, Mode::Practice, None));
    wizard.start_accepted = Some(catalogue::START_NOTICE);
    for id in [StepId(2, 1), StepId(1, 2)] {
        let mut record = StepRecord::new(id);
        record.outcome = Outcome::Passed;
        wizard.progress.push(record);
    }
    frame(&ctx, &mut wizard, Vec::new());
    let lines = frame(&ctx, &mut wizard, vec![key(Key::Backspace)]);
    assert!(lines.iter().any(|line| line == "Step 7 of 37"));
    assert!(lines.iter().any(|line| line == "Catalogue ID 1.2"));
    assert!(lines.iter().any(|line| line == "Result"));
    assert!(lines.iter().any(|line| line == "Return to current step"));
    assert!(!lines.iter().any(|line| line == "Run  (R)"));
    frame(&ctx, &mut wizard, Vec::new());
    let lines = frame(&ctx, &mut wizard, vec![key(Key::Backspace)]);
    assert!(lines.iter().any(|line| line == "Step 5 of 37"));
    assert!(lines.iter().any(|line| line == "Catalogue ID 2.1"));
    frame(&ctx, &mut wizard, vec![key(Key::R)]);
    assert!(replies.try_recv().is_err());
    frame(&ctx, &mut wizard, vec![key(Key::C)]);
    let lines = frame(&ctx, &mut wizard, Vec::new());
    assert!(lines.iter().any(|line| line == "Step 8 of 37"));
    assert!(lines.iter().any(|line| line == "Catalogue ID 2.2"));
    assert!(matches!(wizard.prompt, Some(Prompt::Card(id, ..)) if id == pending));
    assert_eq!(wizard.progress.len(), 2);
    assert!(wizard.progress.iter().all(|record| record.outcome.passed()));
    assert!(replies.try_recv().is_err());
}

#[test]
fn first_card_back_shows_the_already_accepted_start_notice() {
    let ctx = egui::Context::default();
    let (mut wizard, replies) = wired(Prompt::Start(catalogue::START_NOTICE.into()));
    wizard.reply(Reply::Accept(true));
    assert!(matches!(replies.try_recv(), Ok(Reply::Accept(true))));
    wizard.prompt = Some(Prompt::Card(StepId(0, 1), Mode::Practice, None));
    frame(&ctx, &mut wizard, Vec::new());
    let lines = frame(&ctx, &mut wizard, vec![key(Key::Backspace)]);
    assert!(lines.iter().any(|line| line == "Start notice (accepted)"));
    assert!(
        !lines
            .iter()
            .any(|line| line == "Accept and continue" || line == "Quit")
    );
    assert!(replies.try_recv().is_err());
}

#[test]
fn review_shows_the_start_notice_accepted_for_this_mac() {
    let ctx = egui::Context::default();
    let (mut wizard, _) = wired(Prompt::Start(catalogue::MAC_START_NOTICE.into()));
    wizard.reply(Reply::Accept(true));
    wizard.prompt = Some(Prompt::Card(StepId(0, 1), Mode::Real, None));
    wizard.go_back();
    assert_eq!(wizard.reviewing, Some(Review::Start));
    let texts = frame(&ctx, &mut wizard, Vec::new());
    assert!(
        texts
            .iter()
            .any(|line| line.contains("Rig Check can check this Mac"))
    );
}

#[test]
fn technical_details_are_hidden_until_opened() {
    let ctx = egui::Context::default();
    let mut record = StepRecord::new(StepId(4, 2));
    record.started = Some("practice".into());
    record.difference(
        super::super::differences::Severity::Stop,
        "2 words differ",
        vec!["word 2 · expected 0x010000 · actual 0x010001 · XOR 0x000001 · 1 bit".into()],
    );
    let (mut wizard, _) = wired(Prompt::Finished(Box::new(record)));
    let hidden = frame(&ctx, &mut wizard, Vec::new());
    assert!(hidden.iter().any(|text| text == "Show technical details"));
    assert!(!hidden.iter().any(|text| text.contains("XOR")));
    wizard.technical = true;
    let shown = frame(&ctx, &mut wizard, Vec::new());
    assert!(shown.iter().any(|text| text.contains("XOR 0x000001")));
}

#[test]
fn startup_opens_saved_reports_without_starting_a_run() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("Application Support");
    std::fs::create_dir(&legacy).unwrap();
    let path = legacy.join("rig-check-invalid.txt");
    std::fs::write(&path, "Incomplete report").unwrap();
    std::fs::write(path.with_extension("state"), "Incomplete checkpoint").unwrap();
    let mut app = App::new(
        dir.path().join("Documents/Rig Check"),
        Some(legacy),
        Some(Mode::Real),
        |_, _, _| panic!("preview must not start the runner"),
    );
    let ctx = egui::Context::default();
    let text = frame(&ctx, &mut app.wizard, Vec::new());
    for label in [
        "Start new",
        "Open last report",
        "Choose report…",
        "Browse Application Support…",
    ] {
        assert!(text.iter().any(|line| line == label), "{text:?}");
    }
    app.wizard.launch.as_mut().unwrap().open(path);
    let text = frame(&ctx, &mut app.wizard, Vec::new());
    assert!(
        text.iter()
            .any(|line| line.contains("No complete Rig Check report"))
    );
    assert!(!text.iter().any(|line| line == "Resume…"));
    assert_eq!(app.wizard.mode, None);
}

#[test]
fn blocked_resume_offers_safe_report_focus_and_a_fresh_start_without_a_plan() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("rig-check-source.txt");
    std::fs::write(&source, "Synthetic source report").unwrap();
    let (started, starts) = channel();
    let mut app = App::new(dir.path().to_owned(), None, None, move |mode, plan, _| {
        started.send((mode, plan.is_none())).unwrap();
    });
    let ctx = egui::Context::default();
    let blocked_attempt = || {
        let mut run = RunRecord::new(Mode::Practice, "synthetic".into());
        run.finished = true;
        run.resume_cleanup = Some(ResumeCleanup::Released);
        run.blocked_resume = Some(BlockedResume {
            target: StepId(3, 3),
            check: StepId(2, 2),
            reason: "The working tone differs.".into(),
            details: vec![
                "PODxt Live channel · expected 5B · observed 5B".into(),
                "byte 159 (linked Variax model (CC 120)) · expected 0x14 · actual 0x15".into(),
            ],
        });
        Prompt::Done {
            run: Box::new(run),
            report: String::new(),
            path: "synthetic-attempt.txt".into(),
        }
    };
    let show_blocked = |wizard: &mut Wizard| {
        let (to_ui, from_runner) = channel();
        wizard.mode = Some(Mode::Practice);
        wizard.focus_armed = false;
        wizard.from_runner = Some(from_runner);
        to_ui.send(blocked_attempt()).unwrap();
    };
    app.wizard.launch.as_mut().unwrap().open(source.clone());
    show_blocked(&mut app.wizard);
    let text = frame(&ctx, &mut app.wizard, Vec::new());
    for expected in [
        "Resume blocked",
        "Selected target 3.3",
        "Blocking check 2.2",
        "expected 5B · observed 5B",
        "byte 159 (linked Variax model (CC 120))",
        "Open report",
        "Start fresh",
    ] {
        assert!(
            text.iter().any(|label| label.contains(expected)),
            "{text:?}"
        );
    }
    assert!(!app.wizard.can_review());
    frame(&ctx, &mut app.wizard, vec![key(Key::Enter)]);
    app.start_if_ready(&ctx);
    assert_eq!(app.wizard.mode, None);
    assert!(matches!(
        starts.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    let launch = app.wizard.launch.as_ref().unwrap();
    assert_eq!(launch.report.as_ref(), Some(&source));
    assert!(
        launch
            .error
            .as_ref()
            .unwrap()
            .contains("No complete Rig Check report")
    );

    show_blocked(&mut app.wizard);
    frame(&ctx, &mut app.wizard, Vec::new());
    frame(&ctx, &mut app.wizard, vec![key(Key::Tab)]);
    frame(&ctx, &mut app.wizard, vec![key(Key::Enter)]);
    app.start_if_ready(&ctx);
    assert_eq!(
        starts.recv_timeout(Duration::from_secs(1)).unwrap(),
        (Mode::Practice, true)
    );
}

#[test]
fn failed_release_opens_only_the_attempt_report_and_never_restarts() {
    let dir = tempfile::tempdir().unwrap();
    let (starts, started) = channel();
    let mut app = App::new(dir.path().to_owned(), None, None, move |_, _, _| {
        starts.send(()).unwrap();
    });
    let ctx = egui::Context::default();
    let mut run = RunRecord::new(Mode::Practice, "synthetic".into());
    run.finished = true;
    run.blocked_resume = Some(BlockedResume {
        target: StepId(3, 3),
        check: StepId(2, 2),
        reason: "The working tone differs.".into(),
        details: Vec::new(),
    });
    let error = "PODxt Live interface 0 release failed: simulated USB release error";
    run.resume_cleanup = Some(ResumeCleanup::Failed(error.into()));
    let (to_runner, replies) = channel();
    let (to_ui, from_runner) = channel();
    app.wizard.mode = Some(Mode::Practice);
    app.wizard.to_runner = Some(to_runner);
    app.wizard.from_runner = Some(from_runner);
    app.wizard.focus_armed = false;
    to_ui
        .send(Prompt::Done {
            run: Box::new(run),
            report: "Synthetic failed attempt report".into(),
            path: "synthetic-attempt.txt".into(),
        })
        .unwrap();

    let labels = frame(&ctx, &mut app.wizard, Vec::new());
    for expected in [error, VERIFY_USB_CLEANUP, "Open report"] {
        assert!(
            labels.iter().any(|text| text.contains(expected)),
            "{labels:?}"
        );
    }
    assert!(!labels.iter().any(|text| text.contains("Start fresh")));
    frame(&ctx, &mut app.wizard, vec![key(Key::Enter)]);
    let labels = frame(&ctx, &mut app.wizard, Vec::new());
    assert!(
        labels
            .iter()
            .any(|text| text == "Synthetic failed attempt report")
    );
    assert!(!labels.iter().any(|text| text.contains("Start fresh")));
    assert!(!labels.iter().any(|text| text.contains("Resume…")));
    assert!(matches!(app.wizard.prompt, Some(Prompt::Done { .. })));
    app.start_if_ready(&ctx);
    assert!(matches!(
        started.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    assert!(matches!(
        replies.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
}
