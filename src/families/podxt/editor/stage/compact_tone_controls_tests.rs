use super::*;
use egui::{Event, Key, Modifiers};
use pod_preset::PodMetadata;

const VIEWPORTS: [[f32; 2]; 3] = [[1301.0, 974.0], [1120.0, 760.0], [960.0, 680.0]];

struct StageGui {
    ctx: egui::Context,
    app: Workbench,
    screen: Rect,
    output: egui::FullOutput,
}

impl StageGui {
    fn new(size: [f32; 2], tones: Vec<PodTone>) -> Self {
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
        app.detection = None;
        app.new_tab(&super::super::FAMILY);
        let editor = app.podxt_mut();
        editor.table_view_in_test = true;
        let bundle =
            pod_preset::pod_bundle_from_tones(false, &[("Stage controls", tones)]).unwrap();
        editor.open_stage_document(bundle.as_bytes().to_vec());
        let mut gui = Self {
            ctx,
            app,
            screen: Rect::from_min_size(Pos2::ZERO, size.into()),
            output: egui::FullOutput::default(),
        };
        gui.frame(vec![]);
        gui.frame(vec![]);
        gui
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.output = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(self.screen),
                events,
                ..Default::default()
            },
            |ctx| self.app.render(ctx),
        );
    }

    fn label(&self, text: &str) -> Option<Rect> {
        label_rect(&self.output, text)
    }

    fn key(&mut self, key: Key) {
        let mut release = key_event(key);
        if let Event::Key { pressed, .. } = &mut release {
            *pressed = false;
        }
        self.frame(vec![key_event(key), release]);
    }

    fn click(&mut self, label: &str) {
        self.frame(vec![]);
        let pos = self
            .label(label)
            .unwrap_or_else(|| panic!("missing visible label {label:?}"))
            .center();
        self.frame(vec![Event::PointerMoved(pos), pointer(pos, true)]);
        self.frame(vec![pointer(pos, false)]);
        self.frame(vec![]);
    }

    fn scroll_to_header(&mut self) {
        self.frame(vec![
            Event::PointerMoved(egui::pos2(self.screen.right() - 35.0, 380.0)),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 1000.0),
                modifiers: Modifiers::NONE,
            },
        ]);
        for _ in 0..14 {
            self.frame(vec![]);
        }
    }

    fn focus(&mut self, label: &str) {
        for _ in 0..80 {
            self.frame(vec![]);
            if let Some(pos) = self.label(label).map(|label| label.center())
                && let Some(id) = self.ctx.memory(|memory| memory.focused())
                && self.ctx.read_response(id).is_some_and(|response| {
                    response.rect.contains(pos)
                        && response.sense.senses_click()
                        && Some(response.layer_id) == self.ctx.layer_id_at(pos)
                })
            {
                return;
            }
            self.key(Key::Tab);
        }
        panic!("could not reach {label:?} by Tab");
    }

    fn tone(&self, index: usize) -> &PodTone {
        self.app.podxt().pod_library.tone(index).unwrap()
    }

    fn bits(&self, index: usize, key: &str) -> Option<u32> {
        get(&self.tone(index).table_controls(0), key).unwrap().bits
    }
}

fn key_event(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
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

fn label_rect(output: &egui::FullOutput, text: &str) -> Option<Rect> {
    fn visit(shape: &egui::Shape, clip: Rect, text: &str) -> Option<Rect> {
        match shape {
            egui::Shape::Text(shape) if shape.galley.text() == text => {
                let bounds = shape.galley.rect.translate(shape.pos.to_vec2());
                clip.contains_rect(bounds).then_some(bounds)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| visit(shape, clip, text)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| visit(&shape.shape, shape.clip_rect, text))
}

fn linked_tone(name: &str) -> PodTone {
    let mut tone = PodTone::new_podxt_tone(name).unwrap();
    tone.set_table_parameter("VARIAX_FAMILY", 0).unwrap();
    tone.set_table_parameter("VARIAX_FAMILY_0_MODEL", 0)
        .unwrap();
    tone
}

#[test]
fn compact_tone_controls_are_visible_and_keyboard_reachable_at_three_sizes() {
    for size in VIEWPORTS {
        let mut gui = StageGui::new(size, vec![linked_tone("Compact tone")]);
        let title = gui.label("Compact tone").unwrap();
        let metadata = gui.label("Tone metadata").expect("file header metadata");
        let controls = [
            "Tempo",
            "Pedal assignment",
            "Tweak",
            "Variax family",
            "Linked model",
        ]
        .map(|label| {
            gui.label(label)
                .unwrap_or_else(|| panic!("{size:?}: {label}"))
        });
        assert!((title.center().y - metadata.center().y).abs() <= theme::TARGET);
        assert!(metadata.left() >= title.right());
        assert!(
            controls
                .iter()
                .all(|label| (label.top() - controls[0].top()).abs() <= 1.0)
        );
        assert!(gui.label("Tap").is_some());
        assert!(gui.label("Load amp defaults").is_none());
        assert!(gui.label("Model packs").is_none());
        assert!(gui.label("Song").is_none());

        gui.focus("Tap");
        gui.key(Key::Enter);
        assert!(
            gui.app.podxt().stage.tap.is_some(),
            "{size:?}: {:?}",
            gui.output.platform_output.events
        );
        assert!(!gui.app.podxt().pod_dirty());
        gui.focus("1-Wah  2-Vol");
        gui.key(Key::Enter);
        gui.frame(vec![]);
        assert!(gui.label("1-Tweak  2-Vol").is_some(), "{size:?}");
        gui.key(Key::Escape);
        gui.focus("Tone metadata");
        gui.key(Key::Enter);
        for _ in 0..16 {
            gui.frame(vec![]);
        }
        assert!(gui.label("Song").is_some(), "{size:?}");
        gui.focus("Tone metadata");
        gui.key(Key::Enter);
        for _ in 0..16 {
            gui.frame(vec![]);
        }
        assert!(gui.label("Tap").is_some(), "{size:?}");
    }
}

#[test]
fn file_strip_edits_and_metadata_stay_with_selected_tone_and_undo() {
    for size in VIEWPORTS {
        let mut first = linked_tone("First");
        first.set_metadata(PodMetadata::Song, "First song").unwrap();
        let mut second = linked_tone("Second");
        second
            .set_metadata(PodMetadata::Song, "Second song")
            .unwrap();
        let mut gui = StageGui::new(size, vec![first.clone(), second.clone()]);

        let tempo = gui.label("120.0 BPM").unwrap();
        gui.click("120.0 BPM");
        gui.frame(vec![Event::Text("143.5".into())]);
        gui.key(Key::Enter);
        let bpm = gui.bits(0, "TEMPO").map(f32::from_bits);
        assert_eq!(bpm, Some(143.5), "tempo control {tempo:?}");
        gui.click("1-Wah  2-Vol");
        assert!(
            gui.label("1-Tweak  2-Vol").is_some(),
            "pedal menu did not open"
        );
        gui.click("1-Tweak  2-Vol");
        assert_eq!(gui.bits(0, "PEDAL_ASSIGN"), Some(1));

        let rows = gui.tone(0).table_controls(0);
        for (key, name) in [
            ("TWEAK_PARAM", "Tweak"),
            ("VARIAX_FAMILY_0_MODEL", "Linked model"),
        ] {
            let row = get(&rows, key).unwrap();
            let old = choice(row).unwrap();
            let (bits, new) = row
                .choices
                .iter()
                .find(|(bits, _)| Some(*bits) != row.bits)
                .unwrap();
            assert!(gui.label(name).is_some());
            gui.click(old);
            gui.click(new);
            assert_eq!(gui.bits(0, key), Some(*bits));
        }
        gui.scroll_to_header();
        gui.click("Variax 300/500/700");
        gui.click("Variax Acoustic");
        assert_eq!(gui.bits(0, "VARIAX_FAMILY"), Some(1));
        assert_eq!(gui.tone(1).as_bytes(), second.as_bytes());

        gui.scroll_to_header();
        gui.focus("Tone metadata");
        gui.key(Key::Enter);
        for _ in 0..16 {
            gui.frame(vec![]);
        }
        assert!(
            gui.label("Song").is_some(),
            "{size:?}: metadata form did not open"
        );
        gui.click("First song");
        gui.frame(vec![
            Event::Key {
                key: Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
            Event::Text("Edited song".into()),
        ]);
        gui.focus("Apply metadata");
        gui.key(Key::Enter);
        assert_eq!(
            gui.tone(0).metadata(PodMetadata::Song).unwrap().as_deref(),
            Some("Edited song")
        );

        gui.app.podxt_mut().select_pod_index(1);
        gui.scroll_to_header();
        assert!(gui.label("Second").is_some());
        assert!(gui.label("120.0 BPM").is_some());
        assert!(gui.label("1-Wah  2-Vol").is_some());
        assert_eq!(gui.tone(1).as_bytes(), second.as_bytes());
        gui.app.podxt_mut().table_history(false);
        assert_eq!(gui.tone(1).as_bytes(), second.as_bytes());

        gui.app.podxt_mut().select_pod_index(0);
        gui.app.podxt_mut().table_history(false);
        assert_eq!(
            gui.tone(0).metadata(PodMetadata::Song).unwrap().as_deref(),
            Some("First song")
        );
        assert_eq!(gui.bits(0, "TEMPO").map(f32::from_bits), Some(143.5));
    }
}

#[test]
fn tap_does_not_reuse_a_different_tone_or_document_interval() {
    let a = linked_tone("First");
    let b = linked_tone("Second");
    let mut gui = StageGui::new(VIEWPORTS[2], vec![a.clone(), b]);
    let editor = gui.app.podxt_mut();
    editor.stage.tap = Some((
        editor.tone_control_scope(0),
        Instant::now() - std::time::Duration::from_millis(750),
    ));
    editor.select_pod_index(1);
    gui.frame(vec![]);
    gui.click("Tap");
    assert_eq!(gui.bits(1, "TEMPO").map(f32::from_bits), Some(120.0));

    let editor = gui.app.podxt_mut();
    editor.stage.tap = Some((
        editor.tone_control_scope(0),
        Instant::now() - std::time::Duration::from_millis(750),
    ));
    editor.open_stage_document(a.into_bytes());
    gui.frame(vec![]);
    gui.click("Tap");
    assert_eq!(gui.bits(0, "TEMPO").map(f32::from_bits), Some(120.0));

    let editor = gui.app.podxt_mut();
    editor.stage.tap = Some((
        editor.tone_control_scope(0),
        Instant::now() - std::time::Duration::from_millis(750),
    ));
    gui.click("Tap");
    let bpm = gui.bits(0, "TEMPO").map(f32::from_bits).unwrap();
    assert!((30.0..=80.0).contains(&bpm), "{bpm}");
    gui.app.podxt_mut().table_history(false);
    assert_eq!(gui.bits(0, "TEMPO").map(f32::from_bits), Some(120.0));
}

#[cfg(feature = "usb")]
#[test]
fn unavailable_playing_tone_shows_values_without_allowing_file_or_live_edits() {
    let playing = linked_tone("Playing tone");
    let rows = playing.table_controls(0);
    for size in VIEWPORTS {
        let mut editor = PodxtEditor::new();
        editor.table_view_in_test = true;
        editor.open_stage_document(linked_tone("File copy").into_bytes());
        let original = editor.pod_library.tone(0).unwrap().as_bytes().to_vec();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut render = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size.into())),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        editor.stage_header(ui, CURRENT_STAGE, &playing);
                        editor.stage_tone_strip(ui, CURRENT_STAGE, &rows);
                    });
                },
            )
        };
        let output = render(vec![]);
        assert!(label_rect(&output, "Playing tone").is_some());
        assert!(label_rect(&output, "Tone metadata").is_none());
        assert!(label_rect(&output, "Apply name").is_none());
        for label in ["Tap", "120.0 BPM", "1-Wah  2-Vol", "Variax 300/500/700"] {
            let pos = label_rect(&output, label)
                .unwrap_or_else(|| panic!("{label}"))
                .center();
            render(vec![Event::PointerMoved(pos), pointer(pos, true)]);
            render(vec![pointer(pos, false)]);
        }
        assert!(editor.stage.tap.is_none());
        assert!(!editor.live.has_staged_edits());
        assert_eq!(editor.pod_library.tone(0).unwrap().as_bytes(), original);
        assert!(
            get(&rows, "VARIAX_FAMILY")
                .unwrap()
                .live_control()
                .is_none()
        );
        assert!(linked_model_row(&rows).unwrap().live_control().is_none());
    }
}

#[test]
fn more_tone_settings_keeps_file_pack_choices_and_amp_defaults_together() {
    let mut gui = StageGui::new(VIEWPORTS[0], vec![linked_tone("Sets")]);
    let bytes = gui.tone(0).as_bytes().to_vec();
    gui.focus("More tone settings");
    gui.key(Key::Enter);
    for _ in 0..16 {
        gui.frame(vec![]);
    }
    assert!(gui.label("Load amp defaults").is_some());
    gui.click("Model packs");
    gui.click("FX Junkie");
    assert_eq!(gui.app.podxt().parameter_panel.packs, 4);
    assert_eq!(gui.tone(0).as_bytes(), bytes);
}

#[test]
fn tabbing_past_tempo_preserves_existing_file_precision() {
    let mut tone = linked_tone("Tempo precision");
    let exact = 123.456_f32.to_bits();
    tone.set_table_parameter("TEMPO", exact).unwrap();
    let mut gui = StageGui::new(VIEWPORTS[2], vec![tone.clone()]);
    gui.focus("Tap");
    assert_eq!(gui.bits(0, "TEMPO"), Some(exact));
    assert_eq!(gui.tone(0).as_bytes(), tone.as_bytes());
}

#[test]
fn drive_drag_emits_values_before_release_only_for_live_streaming() {
    let tone = PodTone::new_podxt_tone("Drive drag").unwrap();
    let rows = tone.table_controls(0);
    let drive = rows
        .iter()
        .find(|row| row.parameter.key == "DRIVE")
        .unwrap();
    let [min, max] = drive.display_range().unwrap();

    for streaming in [false, true] {
        let ctx = egui::Context::default();
        let mut value = (min + max) / 2.0;
        let mut frame = |events| {
            let mut result = None;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let output = Knob::percent("Drive", &mut value).range(min..=max).show(ui);
                        let bits = stage_drag_value(
                            &output.response,
                            output.changed,
                            streaming,
                            value,
                            drive,
                        );
                        result = Some((
                            output.response.rect.center(),
                            output.response.dragged(),
                            bits,
                        ));
                    });
                },
            );
            result.unwrap()
        };

        let (start, _, _) = frame(vec![]);
        frame(vec![Event::PointerMoved(start), pointer(start, true)]);
        let first = start - egui::vec2(0.0, 18.0);
        let second = start - egui::vec2(0.0, 36.0);
        let (_, first_held, first_bits) = frame(vec![Event::PointerMoved(first)]);
        let (_, second_held, second_bits) = frame(vec![Event::PointerMoved(second)]);
        assert!(first_held && second_held);
        if streaming {
            assert!(first_bits.is_some());
            assert!(second_bits.is_some());
            assert_ne!(first_bits, second_bits);
        } else {
            assert_eq!((first_bits, second_bits), (None, None));
            assert!(frame(vec![pointer(second, false)]).2.is_some());
        }
    }
}
