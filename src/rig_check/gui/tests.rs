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
    let mut texts = Vec::new();
    fn walk(shape: &egui::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => texts.push(text.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, texts)),
            _ => {}
        }
    }
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
    let (mut wizard, rx) = wired(Prompt::Card(StepId(2, 1), Mode::Practice));
    let texts = frame(&ctx, &mut wizard, Vec::new());
    for expected in [
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
fn keyboard_shortcuts_run_and_stop() {
    let ctx = egui::Context::default();
    let (mut wizard, rx) = wired(Prompt::Card(StepId(2, 1), Mode::Practice));
    frame(&ctx, &mut wizard, Vec::new());
    frame(&ctx, &mut wizard, vec![key(Key::R)]);
    assert!(matches!(rx.try_recv(), Ok(Reply::Decision(Decision::Run))));
    assert_eq!(wizard.running, Some(StepId(2, 1)));
    let (mut wizard, rx) = wired(Prompt::Card(StepId(2, 1), Mode::Practice));
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
    let worker = std::thread::spawn(move || {
        let mut op = ChannelOperator {
            to_ui,
            from_ui,
            stop: op_stop,
            repaint: None,
        };
        let (run, _, _) = run_practice(
            Faults::default(),
            &mut op,
            &mut NoReport,
            RunData::default(),
        );
        run.count("passed")
    });
    // The test plays the person: accept, Run every card, type every
    // confirmation, answer Yes.
    let mut cards = 0;
    while let Ok(prompt) = from_runner.recv() {
        let reply = match prompt {
            Prompt::Start(_) => Reply::Accept(true),
            Prompt::Card(..) => {
                cards += 1;
                Reply::Decision(Decision::Run)
            }
            Prompt::Save { confirmation, .. } => Reply::Typed(Some(confirmation)),
            Prompt::NotSeen { .. } => Reply::Retry(Retry::Skip),
            Prompt::Ask(_) => Reply::Answer(Answer::Yes, String::new()),
            _ => continue,
        };
        to_runner.send(reply).unwrap();
    }
    let passed = worker.join().unwrap();
    assert_eq!(passed, 28);
    assert_eq!(cards, 28);
}

#[test]
fn mockups_render_headless() {
    let states = mockup_states();
    assert_eq!(states.len(), 10);
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
