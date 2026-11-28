use super::*;
use egui::{Event, Pos2, Rect, Shape, Vec2};

struct Harness {
    ctx: egui::Context,
    popup: TunerPopup,
    screen: Vec2,
    density: f32,
    now: Instant,
    action: Option<TunerAction>,
    stage: Option<egui::Response>,
    stage_clicks: usize,
    stage_escape: bool,
    focused: bool,
}

impl Harness {
    fn new(screen: Vec2) -> Self {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        Self {
            ctx,
            popup: TunerPopup::default(),
            screen,
            density: 1.0,
            now: Instant::now(),
            action: None,
            stage: None,
            stage_clicks: 0,
            stage_escape: false,
            focused: true,
        }
    }

    fn frame(&mut self, state: Option<TunerState<'_>>, events: Vec<Event>) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.screen)),
            events,
            focused: self.focused,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(self.density);
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let stage = ui.button("Stage action");
                self.stage_clicks += usize::from(stage.clicked());
                self.stage = Some(stage);
            });
            self.action = state.and_then(|state| self.popup.show_at(ctx, state, self.now));
            self.stage_escape = ctx.input(|input| input.key_pressed(Key::Escape));
        })
    }

    fn draw(&mut self, state: TunerState<'_>) -> egui::FullOutput {
        self.frame(Some(state), vec![])
    }

    fn open(&mut self, state: TunerState<'_>) -> egui::FullOutput {
        self.draw(state);
        self.draw(state)
    }

    fn key(&mut self, state: TunerState<'_>, key: Key, modifiers: Modifiers) -> egui::FullOutput {
        self.frame(
            Some(state),
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
        )
    }

    fn click(&mut self, state: TunerState<'_>, pos: Pos2) {
        self.frame(
            Some(state),
            vec![Event::PointerMoved(pos), pointer(pos, true)],
        );
        self.frame(Some(state), vec![pointer(pos, false)]);
    }

    fn window(&self) -> Rect {
        self.ctx
            .memory(|memory| memory.area_rect(window_id()))
            .unwrap()
    }

    fn focus_index(&self) -> Option<usize> {
        self.popup
            .controls
            .unwrap()
            .iter()
            .position(|id| self.ctx.memory(|m| m.has_focus(*id)))
    }
}

fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn reading(note: TunerNote<'_>, cents: Option<i16>, received_at: Instant) -> TunerState<'_> {
    TunerState::Listening {
        reading: Some(TunerReading {
            note,
            cents,
            received_at,
        }),
    }
}

fn visible_label(output: &egui::FullOutput, text: &str) -> Option<Rect> {
    fn label(shape: &Shape, clip: Rect, text: &str) -> Option<Rect> {
        match shape {
            Shape::Text(shape) if shape.galley.text() == text => {
                let rect = shape.galley.rect.translate(shape.pos.to_vec2());
                clip.contains_rect(rect).then_some(rect)
            }
            Shape::Vec(shapes) => shapes.iter().find_map(|shape| label(shape, clip, text)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| label(&shape.shape, shape.clip_rect, text))
}

fn assert_labels(output: &egui::FullOutput, labels: &[&str]) {
    for label in labels {
        assert!(
            visible_label(output, label).is_some(),
            "missing visible {label:?}: {:?}",
            output
                .shapes
                .iter()
                .filter_map(|entry| if let Shape::Text(text) = &entry.shape {
                    Some((text.galley.text(), text.pos, entry.clip_rect))
                } else {
                    None
                })
                .collect::<Vec<_>>()
        );
    }
}

fn needle(output: &egui::FullOutput) -> Option<[Pos2; 2]> {
    output.shapes.iter().find_map(|shape| match shape.shape {
        Shape::LineSegment { points, stroke } if stroke.width == 2.5 => Some(points),
        _ => None,
    })
}

#[test]
fn digital_renders_signed_cents_flats_and_both_in_tune_edges() {
    let mut gui = Harness::new(vec2(740.0, 460.0));
    for (cents, sign, status) in [
        (-12, "−12 cents", "◀ Flat"),
        (-2, "−2 cents", "In tune"),
        (0, "0 cents", "In tune"),
        (2, "+2 cents", "In tune"),
        (12, "+12 cents", "Sharp ▶"),
        (i16::MIN, "−32768 cents", "◀ Flat"),
        (i16::MAX, "+32767 cents", "Sharp ▶"),
    ] {
        let frame = gui.open(reading(TunerNote::Named("Bb"), Some(cents), gui.now));
        assert_labels(&frame, &["Bb", sign, status, "PODxt Live"]);
        assert!(needle(&frame).is_none());
    }
}

#[test]
fn analog_renders_a_sweeping_needle_and_clamps_only_the_drawing() {
    let mut gui = Harness::new(vec2(740.0, 460.0));
    let initial = reading(TunerNote::Named("E"), Some(-12), gui.now);
    gui.open(initial);
    let analog = gui.key(initial, Key::ArrowRight, Modifiers::NONE);
    assert_labels(&analog, &["E", "−50¢", "0", "+50¢", "◀ Flat"]);
    assert!(visible_label(&analog, "−12 cents").is_none());
    assert!(
        !gui.ctx
            .tessellate(analog.shapes, analog.pixels_per_point)
            .is_empty()
    );

    let mut tips = Vec::new();
    for cents in [-200, -50, -12, 0, 12, 50, 200] {
        let frame = gui.draw(reading(TunerNote::Named("E"), Some(cents), gui.now));
        let [pivot, tip] = needle(&frame).unwrap();
        assert!(tip.y < pivot.y);
        tips.push(tip.x);
    }
    assert_eq!(tips[0], tips[1]);
    assert!(tips[1..6].windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(tips[5], tips[6]);
    let raw = reading(TunerNote::Named("E"), Some(200), gui.now);
    let digital = gui.key(raw, Key::ArrowLeft, Modifiers::NONE);
    assert_labels(&digital, &["+200 cents"]);
    assert_eq!(
        Display::new(raw, gui.now).description(),
        "E, +200 cents, Sharp"
    );
}

#[test]
fn absent_invalid_future_and_stale_samples_never_keep_an_old_pitch() {
    let mut gui = Harness::new(vec2(740.0, 460.0));
    let now = gui.now;
    let fresh = reading(TunerNote::Named("A"), Some(0), now);
    let waiting = TunerState::Listening { reading: None };
    assert_labels(&gui.open(waiting), &["Pluck a string", "— cents"]);
    gui.key(fresh, Key::ArrowRight, Modifiers::NONE);
    let no_signal = gui.draw(reading(TunerNote::NoSignal, Some(0), now));
    assert_labels(&no_signal, &["—", "No signal", "Pluck one string"]);
    assert!(needle(&no_signal).is_none());
    let partial = gui.draw(reading(TunerNote::Named("Bb"), None, now));
    assert_labels(&partial, &["Bb", "Waiting for reading"]);
    assert!(needle(&partial).is_none());

    for bad in [
        reading(TunerNote::Missing, Some(0), now),
        reading(TunerNote::Named(""), Some(0), now),
        reading(TunerNote::Named("H"), Some(0), now),
        reading(
            TunerNote::Named("A"),
            Some(0),
            now + Duration::from_millis(1),
        ),
    ] {
        assert!(needle(&gui.draw(fresh)).is_some());
        let frame = gui.draw(bad);
        assert_labels(&frame, &["—", "Waiting for reading"]);
        assert!(needle(&frame).is_none());
        assert!(visible_label(&frame, "A").is_none());
    }

    gui.now += Duration::from_millis(999);
    let before_expiry = gui.draw(fresh);
    assert!(needle(&before_expiry).is_some());
    assert!(
        before_expiry.viewport_output[&egui::ViewportId::ROOT].repaint_delay
            <= Duration::from_millis(1)
    );
    gui.now += Duration::from_millis(1);
    let expired = gui.draw(fresh);
    assert_labels(&expired, &["—", "Waiting for reading"]);
    assert!(needle(&expired).is_none());
}

#[test]
fn keyboard_cycles_activates_and_returns_focus_after_stop() {
    let mut gui = Harness::new(vec2(740.0, 460.0));
    let starting = TunerState::Starting;
    assert_labels(&gui.open(starting), &["Starting tuner…"]);
    assert_eq!(gui.focus_index(), Some(0));
    gui.key(starting, Key::Tab, Modifiers::SHIFT);
    assert_eq!(gui.focus_index(), Some(1));
    gui.key(starting, Key::Tab, Modifiers::NONE);
    assert_eq!(gui.focus_index(), Some(0));
    gui.key(starting, Key::Tab, Modifiers::NONE);
    assert_eq!(gui.focus_index(), Some(1));
    gui.key(starting, Key::Space, Modifiers::NONE);
    assert_eq!(gui.action, None);
    assert_labels(&gui.draw(starting), &["−50¢"]);
    gui.key(starting, Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(gui.focus_index(), Some(0));
    gui.key(starting, Key::Escape, Modifiers::NONE);
    assert_eq!(gui.action, Some(TunerAction::Stop));
    assert!(!gui.stage_escape);

    let stopping = TunerState::Stopping;
    assert_labels(&gui.draw(stopping), &["Stopping tuner…", "— cents"]);
    gui.key(stopping, Key::Escape, Modifiers::NONE);
    assert_eq!(gui.action, None);
    assert!(!gui.stage_escape);

    gui.popup.restore_focus(gui.stage.as_ref().unwrap());
    gui.frame(None, vec![]);
    assert!(gui.stage.as_ref().unwrap().has_focus());
    gui.open(TunerState::Stopped);
    gui.key(TunerState::Stopped, Key::Escape, Modifiers::NONE);
    assert_eq!(gui.action, Some(TunerAction::Dismiss));
}

#[test]
fn nonmodal_window_moves_and_accepts_stage_and_popup_pointer_actions() {
    let mut gui = Harness::new(vec2(740.0, 460.0));
    let active = reading(TunerNote::Named("E"), Some(-12), gui.now);
    let frame = gui.open(active);
    let initial = gui.window();
    let title = visible_label(&frame, "Tuner").unwrap().center();
    gui.frame(
        Some(active),
        vec![Event::PointerMoved(title), pointer(title, true)],
    );
    let moved = title + vec2(50.0, 20.0);
    gui.frame(Some(active), vec![Event::PointerMoved(moved)]);
    gui.frame(Some(active), vec![pointer(moved, false)]);
    assert_eq!(
        gui.window().min - initial.min,
        vec2(50.0, 20.0),
        "initial {initial:?}, now {:?}, title {title:?}",
        gui.window()
    );

    gui.click(active, gui.stage.as_ref().unwrap().rect.center());
    assert_eq!(gui.stage_clicks, 1);
    assert_eq!(gui.action, None);
    gui.key(active, Key::Escape, Modifiers::NONE);
    assert_eq!(gui.action, None);
    assert!(gui.stage_escape);

    let frame = gui.draw(active);
    assert!(visible_label(&frame, "Stop tuner").is_none());
    let window = gui.window();
    let title_height = theme::TARGET + 24.0;
    let glyph = window.right_top() + vec2(-1.0 - title_height / 2.0, 1.0 + title_height / 2.0);
    gui.click(active, glyph);
    assert_eq!(gui.action, Some(TunerAction::Stop));
    gui.open(active);
    let frame = gui.draw(active);
    gui.click(active, visible_label(&frame, "Digital").unwrap().center());
    gui.focused = false;
    gui.key(active, Key::Escape, Modifiers::NONE);
    assert_eq!(gui.action, None);
    gui.focused = true;
    gui.click(active, visible_label(&frame, "Digital").unwrap().center());
    gui.key(active, Key::Escape, Modifiers::NONE);
    assert_eq!(gui.action, Some(TunerAction::Stop));
    gui.click(TunerState::Stopping, glyph);
    assert_eq!(gui.action, None);
}

#[test]
fn terminal_states_clear_readings_keep_errors_and_announce_state_changes_only() {
    let mut gui = Harness::new(vec2(740.0, 460.0));
    let active = reading(TunerNote::Named("E"), Some(-12), gui.now);
    gui.open(TunerState::Starting);
    let first = gui.draw(active);
    let events = |frame: egui::FullOutput| {
        frame
            .platform_output
            .events
            .into_iter()
            .filter_map(|event| match event {
                egui::output::OutputEvent::ValueChanged(info) => info.label,
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(events(first), ["Pitch detected"]);
    assert!(events(gui.draw(reading(TunerNote::Named("Ab"), Some(3), gui.now))).is_empty());
    gui.key(active, Key::ArrowRight, Modifiers::NONE);

    for (state, status) in [
        (TunerState::Stopping, "Stopping tuner…"),
        (TunerState::Stopped, "Tuner stopped"),
        (TunerState::Disconnected, "PODxt Live disconnected"),
        (
            TunerState::Error("Check the tuner on your PODxt Live"),
            "Tuner error",
        ),
    ] {
        let frame = gui.draw(state);
        assert_labels(&frame, &["—", status]);
        assert!(needle(&frame).is_none());
        if let TunerState::Error(detail) = state {
            assert_labels(&frame, &[detail]);
            gui.key(state, Key::Escape, Modifiers::NONE);
            assert_eq!(gui.action, Some(TunerAction::Dismiss));
        }
    }
}

#[test]
fn compact_viewport_keeps_title_views_and_status_visible_at_both_densities() {
    for density in [1.0, 2.0] {
        let mut gui = Harness::new(vec2(320.0, 210.0));
        gui.density = density;
        let active = reading(TunerNote::Named("E"), Some(-12), gui.now);
        let frame = gui.open(active);
        assert_labels(&frame, &["Tuner", "Digital", "Analog", "◀ Flat"]);
        assert!(
            Rect::from_min_size(Pos2::ZERO, gui.screen)
                .shrink(7.0)
                .contains_rect(gui.window()),
            "{:?}",
            gui.window()
        );
        assert!(visible_label(&frame, "−12 cents").is_none());
        let body = pos2(gui.window().center().x, gui.window().bottom() - 52.0);
        gui.frame(
            Some(active),
            vec![
                Event::PointerMoved(body),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -150.0),
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let mut scrolled = gui.draw(active);
        for _ in 0..12 {
            scrolled = gui.draw(active);
        }
        assert_labels(&scrolled, &["−12 cents"]);
        let frame = gui.key(active, Key::ArrowRight, Modifiers::NONE);
        assert_labels(&frame, &["Tuner", "Digital", "Analog", "◀ Flat"]);
        assert!(
            Rect::from_min_size(Pos2::ZERO, gui.screen)
                .shrink(7.0)
                .contains_rect(gui.window()),
            "{:?}",
            gui.window()
        );
        gui.key(active, Key::Escape, Modifiers::NONE);
        assert_eq!(gui.action, Some(TunerAction::Stop));

        gui.screen.y = 180.0;
        let frame = gui.open(TunerState::Stopping);
        assert_labels(&frame, &["Tuner", "Digital", "Analog", "Stopping tuner…"]);
        let frame = gui.draw(TunerState::Disconnected);
        assert_labels(&frame, &["PODxt Live disconnected"]);
    }
}

#[test]
fn view_preference_is_shared_and_persisted_without_session_actions() {
    let dir = tempfile::tempdir().unwrap();
    let preferences = dir.path().join("settings").join("editor-preferences.txt");
    let path = preferences.with_file_name("tuner-view.txt");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "unknown view").unwrap();

    let mut gui = Harness::new(vec2(740.0, 460.0));
    gui.popup = TunerPopup::with_preferences(Some(&preferences));
    let active = reading(TunerNote::Named("E"), Some(-12), gui.now);
    assert_labels(&gui.open(active), &["−12 cents"]);
    gui.key(active, Key::ArrowRight, Modifiers::NONE);
    assert_eq!(gui.action, None);
    assert_eq!(fs::read_to_string(&path).unwrap(), "analog\n");

    gui.popup = TunerPopup::with_preferences(Some(&preferences));
    assert_labels(&gui.open(active), &["−50¢"]);
    assert_eq!(gui.focus_index(), Some(1));
    let mut restarted = Harness::new(vec2(740.0, 460.0));
    restarted.popup = TunerPopup::with_preferences(Some(&preferences));
    assert_labels(&restarted.open(active), &["−50¢"]);

    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert_labels(
        &restarted.key(active, Key::ArrowLeft, Modifiers::NONE),
        &["−12 cents"],
    );
    assert_eq!(restarted.action, None);
    assert_labels(&restarted.draw(active), &["−12 cents"]);
}
