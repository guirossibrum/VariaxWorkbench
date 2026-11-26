use eframe::egui::{self, Color32, Modifiers, Pos2, Rect, Vec2};

use super::assets::{self, UiImage};
use super::body_canvas::{
    self, BodyCanvas, BodyEvent, DragKind, PickupLimits, PickupPlacement, PickupSprite, PickupZone,
    SpriteHit, outline_rect, paint_body_outline, wireframe_path,
};
use super::chrome::{self, DialogButton, DialogEvent, Family, LinkState};
use super::controls::{Selector, Switch};
use super::editor_layout::{self, EditorRegions};
use super::gallery::GalleryState;
use super::knob::{self, Knob};
use super::theme::{
    self, AMBER, BG, DANGER, DIM, FOCUS, HD, INK, LINE, MUTED, OK, PANEL, PODXT, RAISED, TEXT,
    VARIAX,
};
use super::tuning_panel::{self, TuningEvent, TuningPanel, TuningRow, TwelveStringRow};

fn texts(output: &egui::FullOutput) -> Vec<String> {
    fn walk(shape: &egui::Shape, labels: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => labels.push(text.galley.text().to_owned()),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, labels)),
            _ => {}
        }
    }
    let mut labels = Vec::new();
    output
        .shapes
        .iter()
        .for_each(|shape| walk(&shape.shape, &mut labels));
    labels
}

fn shape_has_amber_stroke(shape: &egui::Shape) -> bool {
    match shape {
        egui::Shape::Noop
        | egui::Shape::Text(_)
        | egui::Shape::Mesh(_)
        | egui::Shape::Callback(_) => false,
        egui::Shape::Vec(shapes) => shapes.iter().any(shape_has_amber_stroke),
        egui::Shape::Circle(shape) => shape.stroke.color == AMBER,
        egui::Shape::Ellipse(shape) => shape.stroke.color == AMBER,
        egui::Shape::LineSegment { stroke, .. } => stroke.color == AMBER,
        egui::Shape::Rect(shape) => shape.stroke.color == AMBER,
        egui::Shape::Path(shape) => path_stroke_is_amber(&shape.stroke),
        egui::Shape::QuadraticBezier(shape) => path_stroke_is_amber(&shape.stroke),
        egui::Shape::CubicBezier(shape) => path_stroke_is_amber(&shape.stroke),
    }
}

fn path_stroke_is_amber(stroke: &egui::epaint::PathStroke) -> bool {
    stroke.width > 0.0
        && matches!(
            stroke.color,
            egui::epaint::ColorMode::Solid(color) if color == AMBER
        )
}

fn label_center(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
    fn walk(shape: &egui::Shape, label: &str) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.size() * 0.5)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| walk(shape, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| walk(&shape.shape, label))
}

struct Harness {
    ctx: egui::Context,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        Self { ctx, time: 1.0 }
    }

    fn run(
        &mut self,
        events: Vec<egui::Event>,
        modifiers: Modifiers,
        mut draw: impl FnMut(&egui::Context, &mut egui::Ui),
    ) -> egui::FullOutput {
        let time = self.time;
        self.time += 0.05;
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 720.0))),
                events,
                time: Some(time),
                modifiers,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| draw(ctx, ui));
            },
        )
    }

    fn run_screen(
        &mut self,
        screen: Vec2,
        pixels_per_point: f32,
        events: Vec<egui::Event>,
        mut draw: impl FnMut(&egui::Context, &mut egui::Ui),
    ) -> egui::FullOutput {
        let time = self.time;
        self.time += 0.05;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
            events,
            time: Some(time),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(pixels_per_point);
        self.ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| draw(ctx, ui));
        })
    }
}

fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

fn click_widget(harness: &mut Harness, pos: Pos2, mut draw: impl FnMut(&mut egui::Ui)) {
    harness.run(
        vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        Default::default(),
        |_, ui| draw(ui),
    );
    harness.run(vec![pointer(pos, false)], Default::default(), |_, ui| {
        draw(ui)
    });
}

#[test]
fn theme_tokens_match_the_approved_hex_values() {
    assert_eq!(BG, Color32::from_rgb(0x10, 0x11, 0x14));
    assert_eq!(RAISED, Color32::from_rgb(0x22, 0x26, 0x2c));
    assert_eq!(LINE, Color32::from_rgb(0x3a, 0x40, 0x48));
    assert_eq!(TEXT, Color32::from_rgb(0xf4, 0xf1, 0xea));
    assert_eq!(MUTED, Color32::from_rgb(0xc4, 0xbf, 0xb4));
    assert_eq!(DIM, Color32::from_rgb(0x9a, 0x94, 0x88));
    assert_eq!(AMBER, Color32::from_rgb(0xf0, 0xa2, 0x02));
    assert_eq!(FOCUS, Color32::from_rgb(0xff, 0xc1, 0x4d));
    assert_eq!(DANGER, Color32::from_rgb(0xff, 0xb4, 0xa8));
    assert_eq!(OK, Color32::from_rgb(0xb6, 0xe3, 0xa2));
    assert_eq!(VARIAX, Color32::from_rgb(0xe0, 0xb1, 0x5a));
    assert_eq!(PODXT, Color32::from_rgb(0xe4, 0x57, 0x2e));
    assert_eq!(HD, Color32::from_rgb(0x8a, 0xa0, 0xb4));
    assert_eq!(INK, Color32::from_rgb(0x1c, 0x14, 0x0c));
    assert_eq!(PANEL, Color32::from_rgb(0x1a, 0x1d, 0x22));
    assert_eq!(Family::Variax.accent(), VARIAX);
    assert_eq!(Family::Podxt.accent(), PODXT);
    assert_eq!(Family::Hd.accent(), HD);
    let ctx = egui::Context::default();
    theme::apply(&ctx);
}

#[test]
fn connection_phrases_match_the_specification() {
    assert_eq!(LinkState::NoDevice.chip(), "No device");
    assert_eq!(
        LinkState::NoDevice.tab_title("Variax · Custom 1"),
        "Variax · Custom 1"
    );
    assert_eq!(
        LinkState::IdentityPending.chip(),
        "USB detected · identity pending"
    );
    assert_eq!(
        LinkState::IdentityPending.tab_title("PODxt Live"),
        "PODxt Live · identity pending"
    );
    assert_eq!(
        LinkState::LinkedBridge.chip(),
        "USB bridge detected · guitar identity pending"
    );
    assert_eq!(
        LinkState::LinkedBridge.tab_title("Spank"),
        "Spank · identity pending"
    );
    assert_eq!(
        LinkState::Uncertain.chip(),
        "USB presence could not be verified"
    );
    assert_eq!(
        LinkState::Uncertain.tab_title("PODxt Live"),
        "PODxt Live · presence unknown"
    );
    assert_eq!(
        LinkState::Disconnected.chip(),
        "Disconnected · editing the open file"
    );
    assert_eq!(
        LinkState::Disconnected.tab_title("PODxt Live"),
        "PODxt Live · disconnected"
    );
    assert_eq!(
        LinkState::HdUnsupported.chip(),
        "Live connection not supported yet"
    );
    assert_eq!(
        LinkState::HdUnsupported.tab_title("POD HD500X"),
        "POD HD500X"
    );
    assert_eq!(knob::format_value(50.0, 0, "%"), "50%");
}

#[test]
fn wide_rack_slot_is_larger_than_the_stomp_slot() {
    let wide = [800, 210];
    let stomp = [174, 171];
    assert!(chrome::chain_slot(wide).x > chrome::chain_slot(stomp).x);
    let rack_fit = assets::fitted_size(wide, chrome::chain_slot(wide));
    let squeezed = assets::fitted_size(wide, chrome::chain_slot(stomp));
    assert!(rack_fit.y > squeezed.y);
}

#[test]
fn knob_drag_up_increases_and_shift_is_finer() {
    fn drag(shift: bool) -> f64 {
        let mut harness = Harness::new();
        let mut value = 50.0;
        let mut rect = Rect::NOTHING;
        harness.run(vec![], Default::default(), |_, ui| {
            rect = Knob::percent("Drive", &mut value).show(ui).response.rect;
        });
        let start = rect.center();
        let end = start + egui::vec2(0.0, -24.0);
        harness.run(
            vec![egui::Event::PointerMoved(start), pointer(start, true)],
            Default::default(),
            |_, ui| {
                let _ = Knob::percent("Drive", &mut value).show(ui);
            },
        );
        harness.run(
            vec![egui::Event::PointerMoved(end)],
            Modifiers {
                shift,
                ..Default::default()
            },
            |_, ui| {
                let _ = Knob::percent("Drive", &mut value).show(ui);
            },
        );
        value
    }
    let coarse = drag(false);
    let fine = drag(true);
    assert!(coarse > 60.0, "coarse {coarse}");
    assert!(fine > 50.0 && fine < coarse, "fine {fine} coarse {coarse}");
}

#[test]
fn disabled_knob_drag_does_not_change_the_value() {
    let mut harness = Harness::new();
    let mut value = 50.0;
    let mut rect = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        rect = Knob::percent("Drive", &mut value)
            .enabled(false)
            .show(ui)
            .response
            .rect;
    });
    let start = rect.center();
    let end = start + egui::vec2(0.0, -40.0);
    harness.run(
        vec![egui::Event::PointerMoved(start), pointer(start, true)],
        Default::default(),
        |_, ui| {
            let _ = Knob::percent("Drive", &mut value).enabled(false).show(ui);
        },
    );
    harness.run(
        vec![egui::Event::PointerMoved(end)],
        Default::default(),
        |_, ui| {
            let _ = Knob::percent("Drive", &mut value).enabled(false).show(ui);
        },
    );
    assert_eq!(value, 50.0);
}

#[test]
fn enter_edits_a_knob_as_text_and_escape_cancels() {
    fn open(harness: &mut Harness, value: &mut f64) {
        let mut rect = Rect::NOTHING;
        harness.run(vec![], Default::default(), |_, ui| {
            rect = Knob::percent("Drive", value).show(ui).response.rect;
        });
        let pos = rect.center();
        click_widget(harness, pos, |ui| {
            let _ = Knob::percent("Drive", value).show(ui);
        });
        harness.run(vec![key(egui::Key::Enter)], Default::default(), |_, ui| {
            let _ = Knob::percent("Drive", value).show(ui);
        });
    }

    let mut harness = Harness::new();
    let mut value = 50.0;
    open(&mut harness, &mut value);
    assert_eq!(value, 50.0);
    harness.run(
        vec![egui::Event::Text("42".into())],
        Default::default(),
        |_, ui| {
            let _ = Knob::percent("Drive", &mut value).show(ui);
        },
    );
    harness.run(vec![key(egui::Key::Enter)], Default::default(), |_, ui| {
        let _ = Knob::percent("Drive", &mut value).show(ui);
    });
    assert_eq!(value, 42.0);

    let mut harness = Harness::new();
    let mut value = 50.0;
    open(&mut harness, &mut value);
    harness.run(vec![key(egui::Key::Escape)], Default::default(), |_, ui| {
        let _ = Knob::percent("Drive", &mut value).show(ui);
    });
    harness.run(
        vec![egui::Event::Text("42".into())],
        Default::default(),
        |_, ui| {
            let _ = Knob::percent("Drive", &mut value).show(ui);
        },
    );
    harness.run(vec![key(egui::Key::Enter)], Default::default(), |_, ui| {
        let _ = Knob::percent("Drive", &mut value).show(ui);
    });
    assert_eq!(value, 50.0);
}

#[test]
fn double_click_edits_a_knob_as_text() {
    let mut harness = Harness::new();
    let mut value = 50.0;
    let mut rect = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        rect = Knob::percent("Drive", &mut value).show(ui).response.rect;
    });
    let pos = rect.center();
    click_widget(&mut harness, pos, |ui| {
        let _ = Knob::percent("Drive", &mut value).show(ui);
    });
    click_widget(&mut harness, pos, |ui| {
        let _ = Knob::percent("Drive", &mut value).show(ui);
    });
    harness.run(
        vec![egui::Event::Text("18".into())],
        Default::default(),
        |_, ui| {
            let _ = Knob::percent("Drive", &mut value).show(ui);
        },
    );
    harness.run(vec![key(egui::Key::Enter)], Default::default(), |_, ui| {
        let _ = Knob::percent("Drive", &mut value).show(ui);
    });
    assert_eq!(value, 18.0);
}

#[test]
fn switch_toggles_and_a_disabled_switch_does_not() {
    let mut harness = Harness::new();
    let mut on = false;
    let mut rect = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        rect = ui.add(Switch::new(&mut on)).rect;
    });
    click_widget(&mut harness, rect.center(), |ui| {
        ui.add(Switch::new(&mut on));
    });
    assert!(on);
    let mut rect = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        rect = ui.add(Switch::new(&mut on).enabled(false)).rect;
    });
    click_widget(&mut harness, rect.center(), |ui| {
        ui.add(Switch::new(&mut on).enabled(false));
    });
    assert!(on);
}

#[test]
fn selector_shows_the_empty_name_and_keeps_it_when_disabled() {
    let mut harness = Harness::new();
    let mut selected = None;
    let output = harness.run(vec![], Default::default(), |_, ui| {
        ui.add(Selector::new(
            ui.id().with("amp"),
            &mut selected,
            &["Blackface Lux"],
            "No amp model",
        ));
    });
    assert!(texts(&output).iter().any(|label| label == "No amp model"));
    selected = Some(0);
    let mut rect = Rect::NOTHING;
    let output = harness.run(vec![], Default::default(), |_, ui| {
        rect = ui
            .add(
                Selector::new(
                    ui.id().with("amp"),
                    &mut selected,
                    &["Blackface Lux"],
                    "No amp model",
                )
                .enabled(false),
            )
            .rect;
    });
    assert!(texts(&output).iter().any(|label| label == "Blackface Lux"));
    click_widget(&mut harness, rect.center(), |ui| {
        ui.add(
            Selector::new(
                ui.id().with("amp"),
                &mut selected,
                &["Blackface Lux"],
                "No amp model",
            )
            .enabled(false),
        );
    });
    assert_eq!(selected, Some(0));
}

#[test]
fn dialog_focuses_the_safe_button_escape_cancels_and_enter_confirms() {
    let buttons = [
        DialogButton {
            label: "Yes, scan",
            safe: false,
        },
        DialogButton {
            label: "No",
            safe: true,
        },
    ];
    let mut harness = Harness::new();
    let mut open = true;
    let mut focused = false;
    harness.run(vec![], Default::default(), |ctx, _| {
        let _ = chrome::dialog(
            ctx,
            &mut open,
            "Detect devices",
            "Scan cached USB information?",
            &buttons,
        );
    });
    harness.run(vec![], Default::default(), |ctx, _| {
        focused = chrome::dialog(
            ctx,
            &mut open,
            "Detect devices",
            "Scan cached USB information?",
            &buttons,
        )
        .safe_focused;
    });
    assert!(focused);
    assert!(open);
    let mut event = None;
    harness.run(
        vec![key(egui::Key::Escape)],
        Default::default(),
        |ctx, _| {
            event = chrome::dialog(
                ctx,
                &mut open,
                "Detect devices",
                "Scan cached USB information?",
                &buttons,
            )
            .event;
        },
    );
    assert_eq!(event, Some(DialogEvent::Dismissed));
    assert!(!open);

    let mut harness = Harness::new();
    let mut open = true;
    harness.run(vec![], Default::default(), |ctx, _| {
        let _ = chrome::dialog(
            ctx,
            &mut open,
            "Detect devices",
            "Scan cached USB information?",
            &buttons,
        );
    });
    harness.run(vec![], Default::default(), |ctx, _| {
        let _ = chrome::dialog(
            ctx,
            &mut open,
            "Detect devices",
            "Scan cached USB information?",
            &buttons,
        );
    });
    let mut event = None;
    harness.run(vec![key(egui::Key::Enter)], Default::default(), |ctx, _| {
        event = chrome::dialog(
            ctx,
            &mut open,
            "Detect devices",
            "Scan cached USB information?",
            &buttons,
        )
        .event;
    });
    assert_eq!(event, Some(DialogEvent::Pressed(1)));
    assert!(!open);
}

#[test]
fn section_heading_sits_on_the_inner_margin() {
    let mut harness = Harness::new();
    let mut panel = None;
    harness.run(vec![], Default::default(), |_, ui| {
        panel = Some(chrome::section_panel(ui, "Amp", |ui| {
            ui.label("Blackface Lux");
        }));
    });
    let panel = panel.unwrap();
    let gap = panel.heading.top() - panel.outer.top();
    // The 1px frame stroke sits outside the 16px margin. There is no spacer above the heading.
    assert_eq!(gap, theme::SPACE_16 + 1.0);
}

#[test]
fn missing_art_paints_its_name_and_a_png_loads() {
    let directory = tempfile::tempdir().unwrap();
    let image = image::RgbaImage::from_pixel(2, 2, image::Rgba([10, 20, 30, 255]));
    image.save(directory.path().join("mark.png")).unwrap();
    let ctx = egui::Context::default();
    let loaded = assets::load(&ctx, directory.path(), "mark.png");
    assert!(!loaded.missing);
    assert_eq!(loaded.pixels, [2, 2]);
    assert!(assets::load(&ctx, directory.path(), "../mark.png").missing);
    assert!(assets::load(&ctx, directory.path(), "/tmp/mark.png").missing);
    assert!(assets::load(&ctx, directory.path(), "").missing);
    let missing = UiImage::missing("pod-farm/missing.png");
    let mut harness = Harness::new();
    let output = harness.run(vec![], Default::default(), |_, ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(120.0, 48.0), egui::Sense::hover());
        assets::paint(ui, &missing, rect);
    });
    assert!(
        texts(&output)
            .iter()
            .any(|label| label.contains("missing.png"))
    );
}

#[test]
fn gallery_names_every_component() {
    let mut harness = Harness::new();
    let mut state = GalleryState::default();
    let output = harness.run(vec![], Default::default(), |ctx, _| state.show(ctx));
    let labels = texts(&output);
    for name in [
        "Knob",
        "Switch",
        "Selector",
        "Block",
        "LED",
        "Display",
        "Tab",
        "Toolbar",
        "Status",
        "Dialog",
        "Editor",
        "Body",
        "Pickup 1",
        "Pickup 2",
        "Tuning",
        "Unsaved",
        "No device",
        "No amp model",
    ] {
        assert!(
            labels.iter().any(|label| label.contains(name)),
            "missing {name}"
        );
    }
}

#[test]
fn selected_pickup_paints_no_box_on_the_body() {
    let mut harness = Harness::new();
    let body = missing_image("variax/spank-body.png", [426, 412]);
    let pickup = missing_image("variax/spank-pickup.png", [120, 120]);
    let output = harness.run(vec![], Default::default(), |_, ui| {
        ui.allocate_ui(egui::vec2(360.0, 280.0), |ui| {
            let _ = show_body(
                ui,
                &body,
                &pickup,
                &[
                    PickupView {
                        inches: 1.8,
                        degrees: 8.7,
                        enabled: true,
                        selected: true,
                    },
                    PickupView {
                        inches: 3.88,
                        degrees: 0.0,
                        enabled: true,
                        selected: false,
                    },
                ],
                true,
                None,
            );
        });
    });
    assert!(
        output
            .shapes
            .iter()
            .all(|shape| !shape_has_amber_stroke(&shape.shape)),
        "a selected pickup painted an amber stroke on the body"
    );
}

#[test]
fn gallery_pickup_controls_change_which_sprite_is_in_front() {
    let mut harness = Harness::new();
    let mut state = GalleryState::default();
    assert_eq!(state.selected_pickup, 0);
    let output = harness.run(vec![], Default::default(), |ctx, _| state.show(ctx));
    let center = label_center(&output, "Pickup 2").expect("Pickup 2 control");
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |ctx, _| state.show(ctx),
    );
    harness.run(
        vec![pointer(center, false)],
        Default::default(),
        |ctx, _| {
            state.show(ctx);
        },
    );
    assert_eq!(state.selected_pickup, 1);
}

fn missing_image(name: &str, pixels: [usize; 2]) -> UiImage {
    let mut image = UiImage::missing(name);
    image.pixels = pixels;
    image
}

struct PickupView {
    inches: f32,
    degrees: f32,
    enabled: bool,
    selected: bool,
}

fn show_body(
    ui: &mut egui::Ui,
    body: &UiImage,
    pickup: &UiImage,
    views: &[PickupView],
    interactive: bool,
    outline: Option<&UiImage>,
) -> super::body_canvas::BodyOutput {
    show_body_alphas(ui, body, pickup, views, interactive, outline, &[])
}

fn show_body_alphas(
    ui: &mut egui::Ui,
    body: &UiImage,
    pickup: &UiImage,
    views: &[PickupView],
    interactive: bool,
    outline: Option<&UiImage>,
    alphas: &[Option<&[u8]>],
) -> super::body_canvas::BodyOutput {
    let sprites: Vec<PickupSprite<'_>> = views
        .iter()
        .map(|view| PickupSprite {
            position_inches: view.inches,
            angle_degrees: view.degrees,
            enabled: view.enabled,
            selected: view.selected,
            image: pickup,
        })
        .collect();
    let _ = outline;
    BodyCanvas::new(body, &sprites)
        .alphas(alphas)
        .interactive(interactive)
        .show(ui)
}

fn open_rows(labels: &[String; 6]) -> Vec<TuningRow<'_>> {
    const NAMES: [&str; 6] = ["High E", "B", "G", "D", "A", "Low E"];
    const NOTES: [&str; 6] = ["E", "B", "G", "D", "A", "E"];
    (0..6)
        .map(|row| {
            let id = 5 - row;
            TuningRow {
                id,
                name: NAMES[row],
                shift: 0,
                shift_label: &labels[id],
                note: NOTES[row],
            }
        })
        .collect()
}

fn show_tuning(
    ui: &mut egui::Ui,
    rows: &[TuningRow<'_>],
    interactive: bool,
    lock_enabled: bool,
    transpose: bool,
) -> super::tuning_panel::TuningOutput {
    TuningPanel::new(rows)
        .interactive(interactive)
        .lock(lock_enabled, false)
        .transpose_enabled(transpose)
        .show(ui)
}

#[test]
fn pickup_placement_and_hit_testing_follow_the_approved_map() {
    let placement = PickupPlacement::default();
    assert_eq!(placement.position_to_art(0.1).x, placement.x_min);
    assert_eq!(placement.position_to_art(9.5).x, placement.x_max);
    // `(int)(318 * 1.7 / 9.4)` is 57, so 1.8 inches paints at 125, not 125.51.
    assert_eq!(placement.position_to_art(1.8).x, 125.0);
    let continuous = 68.0_f32 + 318.0 * (1.8 - 0.1) / (9.5 - 0.1);
    assert!((continuous - 125.5106).abs() < 1.0e-3, "{continuous}");
    assert!((placement.art_x_to_position(continuous) - 1.8).abs() < 1.0e-4);

    let lester = PickupPlacement {
        art: egui::vec2(426.0, 412.0),
        x_min: -8.0,
        x_max: 503.0,
        y: 100.0,
    };
    assert_eq!(lester.position_to_art(0.1).x, -8.0);
    assert_eq!(lester.position_to_art(9.5).x, 503.0);
    assert_eq!(body_canvas::move_inches(9.5, 40.0, placement), 9.5);
    assert!((body_canvas::rotate_degrees(0.0, -20.0, false) - 18.0).abs() < 0.01);
    assert!((body_canvas::rotate_degrees(0.0, -20.0, true) - 1.8).abs() < 0.01);
    assert_eq!(body_canvas::rotate_degrees(0.0, -80.0, false), 45.0);

    let sprite = egui::vec2(120.0, 120.0);
    assert_eq!(
        body_canvas::zone_at(Vec2::ZERO, sprite),
        Some(PickupZone::Move)
    );
    assert_eq!(
        body_canvas::zone_at(egui::vec2(0.0, 40.0), sprite),
        Some(PickupZone::Rotate)
    );
    assert_eq!(body_canvas::zone_at(egui::vec2(0.0, 70.0), sprite), None);

    let center = Pos2::new(80.0, 90.0);
    let order = [
        SpriteHit {
            index: 0,
            center,
            sprite,
            angle_degrees: 0.0,
            selected: false,
        },
        SpriteHit {
            index: 1,
            center,
            sprite,
            angle_degrees: 0.0,
            selected: true,
        },
    ];
    assert_eq!(
        body_canvas::hit_sprite(center, &order),
        Some((1, PickupZone::Move))
    );
    let turned = SpriteHit {
        index: 4,
        center: Pos2::new(200.0, 200.0),
        sprite,
        angle_degrees: 90.0,
        selected: false,
    };
    assert_eq!(
        body_canvas::hit_sprite(Pos2::new(240.0, 200.0), &[turned]),
        Some((4, PickupZone::Rotate))
    );
    assert_eq!(
        body_canvas::hit_sprite(Pos2::new(270.0, 200.0), &[turned]),
        None
    );

    let neck = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(410.0, 200.0));
    for semitone in -12..=12 {
        assert_eq!(
            tuning_panel::semitone_at(neck, tuning_panel::scale_x(neck, semitone)),
            semitone
        );
    }
    let marker = tuning_panel::marker_center(neck, 40.0, 0);
    assert!(tuning_panel::marker_hit(
        marker + egui::vec2(16.0, 0.0),
        marker
    ));
    assert!(!tuning_panel::marker_hit(
        marker + egui::vec2(16.1, 0.0),
        marker
    ));
}

#[test]
fn workbench_body_limits_place_and_drag_per_body() {
    // `UpdateBodyImage` bounds. Ids absent from the switch stay 20…458.
    const RANGES: &[(u32, i32, i32)] = &[
        (0x0310_0000, -5, 458),
        (0x0310_0001, -5, 458),
        (0x0310_0002, -5, 458),
        (0x0310_0003, -15, 483),
        (0x0310_0004, -8, 503),
        (0x0310_0005, -10, 488),
        (0x0310_0006, -25, 547),
        (0x0310_0007, -74, 546),
        (0x0310_0008, -28, 457),
        (0x0310_0009, 40, 458),
        (0x0310_000A, 8, 499),
        (0x0310_000B, 0, 458),
        (0x0310_000C, -5, 458),
        (0x0310_000D, -5, 458),
        (0x0310_000E, 17, 503),
        (0x0310_000F, 10, 458),
        (0x0310_0010, 0, 477),
        (0x0310_0011, 8, 428),
        (0x0310_0018, 27, 415),
        (0x0310_0019, 20, 458),
    ];
    for &(id, left, right) in RANGES {
        assert_eq!(
            body_canvas::pickup_limits(id),
            PickupLimits { left, right },
            "{id:#x}"
        );
        let placement = body_canvas::placement_for_body(id);
        let x_at_min = (left - 12 + 60) as f32;
        let travel = (right - left - 120) as f32;
        assert_eq!(placement.position_to_art(0.1), egui::pos2(x_at_min, 215.0));
        assert_eq!(
            placement.position_to_art(9.5),
            egui::pos2(x_at_min + travel, 215.0)
        );
    }
    assert_eq!(
        body_canvas::pickup_limits(0x0310_0017),
        PickupLimits {
            left: 20,
            right: 458
        }
    );
    assert_eq!(
        body_canvas::placement_for_body(0),
        PickupPlacement::default()
    );
    assert_ne!(
        body_canvas::placement_for_body(0x0310_0003).x_min,
        PickupPlacement::default().x_min
    );

    let lester = body_canvas::placement_for_body(0x0310_0004);
    assert_eq!(lester.position_to_art(0.1).x, 40.0);
    assert_eq!(lester.position_to_art(9.5).x, 431.0);
    assert_eq!(lester.position_to_art(1.8).x, 110.0);
    let firebird = body_canvas::placement_for_body(0x0310_0009);
    let lester_step = body_canvas::move_inches(1.8, 100.0, lester);
    let firebird_step = body_canvas::move_inches(1.8, 100.0, firebird);
    assert!(
        (lester_step - firebird_step).abs() > 0.1,
        "{lester_step} vs {firebird_step}"
    );

    let body = missing_image("variax/lester.png", [426, 412]);
    let pickup = missing_image("variax/lester-pickup.png", [120, 120]);
    let mut harness = Harness::new();
    let show = |harness: &mut Harness, events| {
        let mut reported = Vec::new();
        let mut art = Rect::NOTHING;
        harness.run(events, Default::default(), |_, ui| {
            let sprites = [PickupSprite {
                position_inches: 1.8,
                angle_degrees: 0.0,
                enabled: true,
                selected: true,
                image: &pickup,
            }];
            let output = BodyCanvas::new(&body, &sprites).placement(lester).show(ui);
            reported = output.events;
            art = output.art_rect;
        });
        (reported, art)
    };
    let (_, art) = show(&mut harness, vec![]);
    let scale = lester.screen_scale(art);
    let center = lester.screen_point(art, 1.8);
    let (pressed, _) = show(
        &mut harness,
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
    );
    assert_eq!(pressed, vec![BodyEvent::PickupClicked { index: 0 }]);
    let (dragged, _) = show(
        &mut harness,
        vec![egui::Event::PointerMoved(center + egui::vec2(100.0, 0.0))],
    );
    let inches = dragged.iter().find_map(|event| match event {
        BodyEvent::Moved { index: 0, inches } => Some(*inches),
        _ => None,
    });
    let expected = body_canvas::move_inches(1.8, 100.0 / scale, lester);
    let on_default = body_canvas::move_inches(1.8, 100.0 / scale, PickupPlacement::default());
    let inches = inches.expect("lester drag");
    assert!((inches - expected).abs() < 1.0e-3, "{inches} vs {expected}");
    assert!(
        (inches - on_default).abs() > 0.05,
        "{inches} vs {on_default}"
    );
}

#[test]
fn body_canvas_drag_reports_move_rotate_and_clamp() {
    let body = missing_image("variax/spank-body.png", [426, 412]);
    let pickup = missing_image("variax/spank-pickup.png", [120, 120]);
    let views = [PickupView {
        inches: 1.8,
        degrees: 0.0,
        enabled: true,
        selected: true,
    }];
    let placement = PickupPlacement::default();
    let mut harness = Harness::new();
    let mut art = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &views, true, None).art_rect;
    });
    let scale = placement.screen_scale(art);
    let center = placement.screen_point(art, 1.8);
    let end = center + egui::vec2(30.0, 0.0);
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |_, ui| {
            let _ = show_body(ui, &body, &pickup, &views, true, None);
        },
    );
    let mut inches = None;
    harness.run(
        vec![egui::Event::PointerMoved(end)],
        Default::default(),
        |_, ui| {
            let output = show_body(ui, &body, &pickup, &views, true, None);
            assert!(output.events.iter().any(|event| matches!(
                event,
                BodyEvent::DragStarted {
                    index: 0,
                    kind: DragKind::Move
                }
            )));
            inches = output.events.iter().find_map(|event| match event {
                BodyEvent::Moved { index: 0, inches } => Some(*inches),
                _ => None,
            });
        },
    );
    let inches = inches.expect("move");
    let expected = body_canvas::move_inches(1.8, 30.0 / scale, placement);
    assert!((inches - expected).abs() < 1.0e-3, "{inches} vs {expected}");
    assert!(inches > 1.8);
    let mut finished = false;
    harness.run(vec![pointer(end, false)], Default::default(), |_, ui| {
        finished = show_body(ui, &body, &pickup, &views, true, None)
            .events
            .iter()
            .any(|event| {
                matches!(
                    event,
                    BodyEvent::DragFinished {
                        index: 0,
                        kind: DragKind::Move
                    }
                )
            });
    });
    assert!(finished);

    let sprite = body_canvas::sprite_art_size(pickup.pixels, placement.art) * scale;
    let rotate_at = center + egui::vec2(0.0, sprite.y * 0.4);
    let rotate = |fine: bool, dy: f32| -> f32 {
        let mut harness = Harness::new();
        let up = rotate_at + egui::vec2(0.0, dy);
        harness.run(vec![], Default::default(), |_, ui| {
            let _ = show_body(ui, &body, &pickup, &views, true, None);
        });
        harness.run(
            vec![
                egui::Event::PointerMoved(rotate_at),
                pointer(rotate_at, true),
            ],
            Default::default(),
            |_, ui| {
                let _ = show_body(ui, &body, &pickup, &views, true, None);
            },
        );
        let modifiers = Modifiers {
            ctrl: fine,
            ..Default::default()
        };
        let mut degrees = None;
        harness.run(vec![egui::Event::PointerMoved(up)], modifiers, |_, ui| {
            let output = show_body(ui, &body, &pickup, &views, true, None);
            assert!(output.events.iter().any(|event| matches!(
                event,
                BodyEvent::DragStarted {
                    index: 0,
                    kind: DragKind::Rotate
                }
            )));
            degrees = output.events.iter().find_map(|event| match event {
                BodyEvent::Rotated { index: 0, degrees } => Some(*degrees),
                _ => None,
            });
        });
        if fine {
            assert!(
                degrees.is_none(),
                "a Ctrl-held first sample added {degrees:?}"
            );
            let further = up + egui::vec2(0.0, dy);
            harness.run(
                vec![egui::Event::PointerMoved(further)],
                modifiers,
                |_, ui| {
                    let output = show_body(ui, &body, &pickup, &views, true, None);
                    degrees = output.events.iter().find_map(|event| match event {
                        BodyEvent::Rotated { index: 0, degrees } => Some(*degrees),
                        _ => None,
                    });
                },
            );
        }
        degrees.expect("rotate")
    };
    let coarse = rotate(false, -20.0);
    let fine = rotate(true, -20.0);
    let clamped = rotate(false, -80.0);
    assert!((coarse - 18.0).abs() < 0.05, "{coarse}");
    assert!((fine - 1.8).abs() < 0.05, "{fine}");
    assert_eq!(clamped, 45.0);

    let capped = [PickupView {
        inches: 9.5,
        degrees: 0.0,
        enabled: true,
        selected: true,
    }];
    let mut harness = Harness::new();
    let mut art = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &capped, true, None).art_rect;
    });
    let at_end = placement.screen_point(art, 9.5);
    let past = at_end + egui::vec2(40.0, 0.0);
    harness.run(
        vec![egui::Event::PointerMoved(at_end), pointer(at_end, true)],
        Default::default(),
        |_, ui| {
            let _ = show_body(ui, &body, &pickup, &capped, true, None);
        },
    );
    harness.run(
        vec![egui::Event::PointerMoved(past)],
        Default::default(),
        |_, ui| {
            let events = show_body(ui, &body, &pickup, &capped, true, None).events;
            assert!(events.iter().any(|event| matches!(
                event,
                BodyEvent::DragStarted {
                    index: 0,
                    kind: DragKind::Move
                }
            )));
            assert!(
                events
                    .iter()
                    .all(|event| !matches!(event, BodyEvent::Moved { .. }))
            );
        },
    );
}

#[test]
fn body_canvas_clicks_select_toggle_and_can_be_inert() {
    let body = missing_image("variax/spank-body.png", [426, 412]);
    let pickup = missing_image("variax/spank-pickup.png", [120, 120]);
    let overlapping = [
        PickupView {
            inches: 3.0,
            degrees: 0.0,
            enabled: true,
            selected: false,
        },
        PickupView {
            inches: 3.0,
            degrees: 0.0,
            enabled: true,
            selected: true,
        },
    ];
    let placement = PickupPlacement::default();
    let mut harness = Harness::new();
    let mut art = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &overlapping, true, None).art_rect;
    });
    let center = placement.screen_point(art, 3.0);
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |_, ui| {
            let events = show_body(ui, &body, &pickup, &overlapping, true, None).events;
            assert_eq!(events, vec![BodyEvent::PickupClicked { index: 1 }]);
        },
    );
    harness.run(vec![pointer(center, false)], Default::default(), |_, ui| {
        let events = show_body(ui, &body, &pickup, &overlapping, true, None).events;
        assert!(events.is_empty());
    });

    let away = art.left_top() + egui::vec2(2.0, 2.0);
    harness.run(
        vec![egui::Event::PointerMoved(away), pointer(away, true)],
        Default::default(),
        |_, ui| {
            let events = show_body(ui, &body, &pickup, &overlapping, true, None).events;
            assert_eq!(events, vec![BodyEvent::BodyClicked]);
        },
    );
    harness.run(vec![pointer(away, false)], Default::default(), |_, ui| {
        let events = show_body(ui, &body, &pickup, &overlapping, true, None).events;
        assert!(events.is_empty());
    });

    let one = [PickupView {
        inches: 3.0,
        degrees: 0.0,
        enabled: true,
        selected: true,
    }];
    let mut harness = Harness::new();
    let mut art = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &one, true, None).art_rect;
    });
    let center = placement.screen_point(art, 3.0);
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |_, ui| {
            let events = show_body(ui, &body, &pickup, &one, true, None).events;
            assert_eq!(events, vec![BodyEvent::PickupClicked { index: 0 }]);
        },
    );
    harness.run(vec![pointer(center, false)], Default::default(), |_, ui| {
        assert!(
            show_body(ui, &body, &pickup, &one, true, None)
                .events
                .is_empty()
        );
    });
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |_, ui| {
            let events = show_body(ui, &body, &pickup, &one, true, None).events;
            assert_eq!(events, vec![BodyEvent::PickupClicked { index: 0 }]);
        },
    );
    harness.run(vec![pointer(center, false)], Default::default(), |_, ui| {
        let events = show_body(ui, &body, &pickup, &one, true, None).events;
        assert_eq!(events, vec![BodyEvent::PickupToggled { index: 0 }]);
    });

    let mut harness = Harness::new();
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &one, false, None).art_rect;
    });
    let center = placement.screen_point(art, 3.0);
    let end = center + egui::vec2(30.0, 0.0);
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |_, ui| {
            assert!(
                show_body(ui, &body, &pickup, &one, false, None)
                    .events
                    .is_empty()
            );
        },
    );
    harness.run(
        vec![egui::Event::PointerMoved(end)],
        Default::default(),
        |_, ui| {
            assert!(
                show_body(ui, &body, &pickup, &one, false, None)
                    .events
                    .is_empty()
            );
        },
    );

    let mut harness = Harness::new();
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &one, true, None).art_rect;
    });
    let center = placement.screen_point(art, 3.0);
    let end = center + egui::vec2(30.0, 0.0);
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |_, ui| {
            let _ = show_body(ui, &body, &pickup, &one, true, None);
        },
    );
    harness.run(
        vec![egui::Event::PointerMoved(end)],
        Default::default(),
        |_, ui| {
            assert!(
                show_body(ui, &body, &pickup, &one, true, None)
                    .events
                    .iter()
                    .any(|event| matches!(event, BodyEvent::DragStarted { .. }))
            );
        },
    );
    harness.run(vec![], Default::default(), |_, ui| {
        let events = show_body(ui, &body, &pickup, &one, false, None).events;
        assert_eq!(
            events,
            vec![BodyEvent::DragFinished {
                index: 0,
                kind: DragKind::Move
            }]
        );
    });
}

fn solid_bytes(alpha: u8) -> Vec<u8> {
    [255, 255, 255, alpha].repeat(40 * 40)
}

#[test]
fn pickup_hit_uses_alpha_above_204_and_the_padded_move_circle() {
    let sprite = egui::vec2(120.0, 120.0);
    // Padded side is (int)(120 * 1.15) = 138. Radius is (138 / 2) * 0.5 = 34.5.
    assert_eq!(
        body_canvas::zone_at(egui::vec2(32.0, 0.0), sprite),
        Some(PickupZone::Move)
    );
    assert_eq!(
        body_canvas::zone_at(egui::vec2(36.0, 0.0), sprite),
        Some(PickupZone::Rotate)
    );

    let body = missing_image("variax/spank-body.png", [426, 412]);
    let pickup = missing_image("alpha-pickup", [40, 40]);
    let views = [PickupView {
        inches: 3.0,
        degrees: 0.0,
        enabled: true,
        selected: true,
    }];
    let press_center = |bytes: &[u8]| -> Vec<BodyEvent> {
        let alphas = [Some(bytes)];
        let mut harness = Harness::new();
        let mut art = Rect::NOTHING;
        harness.run(vec![], Default::default(), |_, ui| {
            art = show_body_alphas(ui, &body, &pickup, &views, true, None, &alphas).art_rect;
        });
        let center = PickupPlacement::default().screen_point(art, 3.0);
        let mut events = Vec::new();
        harness.run(
            vec![egui::Event::PointerMoved(center), pointer(center, true)],
            Default::default(),
            |_, ui| {
                events = show_body_alphas(ui, &body, &pickup, &views, true, None, &alphas).events;
            },
        );
        events
    };
    let miss = solid_bytes(204);
    let hit = solid_bytes(205);
    assert_eq!(press_center(&miss), vec![BodyEvent::BodyClicked]);
    assert_eq!(
        press_center(&hit),
        vec![BodyEvent::PickupClicked { index: 0 }]
    );

    let mut split = vec![0; 40 * 40 * 4];
    for y in 16..24 {
        for x in 16..24 {
            let index = (y * 40 + x) * 4;
            split[index..index + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    let alphas = [Some(split.as_slice())];
    let mut harness = Harness::new();
    let mut art = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body_alphas(ui, &body, &pickup, &views, true, None, &alphas).art_rect;
    });
    let placement = PickupPlacement::default();
    let center = placement.screen_point(art, 3.0);
    let scale = placement.screen_scale(art);
    // 14 image pixels is inside the 40px rectangle and outside the opaque block.
    let outside = center + egui::vec2(14.0 * scale, 0.0);
    harness.run(
        vec![egui::Event::PointerMoved(outside), pointer(outside, true)],
        Default::default(),
        |_, ui| {
            assert_eq!(
                show_body_alphas(ui, &body, &pickup, &views, true, None, &alphas).events,
                vec![BodyEvent::BodyClicked]
            );
        },
    );
    harness.run(
        vec![pointer(outside, false)],
        Default::default(),
        |_, ui| {
            let _ = show_body_alphas(ui, &body, &pickup, &views, true, None, &alphas);
        },
    );
    harness.run(
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
        |_, ui| {
            assert_eq!(
                show_body_alphas(ui, &body, &pickup, &views, true, None, &alphas).events,
                vec![BodyEvent::PickupClicked { index: 0 }]
            );
        },
    );
}

#[test]
fn body_canvas_drag_tracks_the_press_point_and_rebases_rotation() {
    let body = missing_image("variax/spank-body.png", [426, 412]);
    let pickup = missing_image("variax/spank-pickup.png", [120, 120]);
    let views = [PickupView {
        inches: 1.8,
        degrees: 0.0,
        enabled: true,
        selected: true,
    }];
    let placement = PickupPlacement::default();
    let mut harness = Harness::new();
    let mut art = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &views, true, None).art_rect;
    });
    let scale = placement.screen_scale(art);
    let center = placement.screen_point(art, 1.8);
    let show = |harness: &mut Harness, events, modifiers: Modifiers| -> Vec<BodyEvent> {
        let mut reported = Vec::new();
        harness.run(events, modifiers, |_, ui| {
            reported = show_body(ui, &body, &pickup, &views, true, None).events;
        });
        reported
    };
    let press = show(
        &mut harness,
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
        Default::default(),
    );
    assert_eq!(press, vec![BodyEvent::PickupClicked { index: 0 }]);

    let far = center + egui::vec2(800.0, 0.0);
    let capped = show(
        &mut harness,
        vec![egui::Event::PointerMoved(far)],
        Default::default(),
    );
    assert!(capped.iter().any(|event| matches!(
        event,
        BodyEvent::DragStarted {
            index: 0,
            kind: DragKind::Move
        }
    )));
    assert!(capped.iter().all(|event| {
        !matches!(
            event,
            BodyEvent::PickupClicked { .. } | BodyEvent::Rotated { .. }
        )
    }));
    let capped_inches = capped.iter().find_map(|event| match event {
        BodyEvent::Moved { index: 0, inches } => Some(*inches),
        _ => None,
    });
    assert_eq!(capped_inches, Some(9.5));

    let mid = center + egui::vec2(200.0, 0.0);
    let returned_part = show(
        &mut harness,
        vec![egui::Event::PointerMoved(mid)],
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    let mid_inches = returned_part.iter().find_map(|event| match event {
        BodyEvent::Moved { index: 0, inches } => Some(*inches),
        _ => None,
    });
    let expected_mid = body_canvas::move_inches(1.8, 200.0 / scale, placement);
    let mid_inches = mid_inches.expect("mid move");
    assert!(
        (mid_inches - expected_mid).abs() < 1.0e-3,
        "{mid_inches} vs {expected_mid}"
    );
    assert!(returned_part.iter().all(|event| !matches!(
        event,
        BodyEvent::Rotated { .. } | BodyEvent::PickupClicked { .. }
    )));

    let home = show(
        &mut harness,
        vec![egui::Event::PointerMoved(center)],
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    let home_inches = home.iter().find_map(|event| match event {
        BodyEvent::Moved { index: 0, inches } => Some(*inches),
        _ => None,
    });
    assert_eq!(home_inches, Some(1.8));

    let sprite = body_canvas::sprite_art_size(pickup.pixels, placement.art) * scale;
    let rotate_at = center + egui::vec2(0.0, sprite.y * 0.4);
    let mut harness = Harness::new();
    harness.run(vec![], Default::default(), |_, ui| {
        let _ = show_body(ui, &body, &pickup, &views, true, None);
    });
    let rotate = |harness: &mut Harness, events, modifiers: Modifiers| -> Vec<BodyEvent> {
        let mut reported = Vec::new();
        harness.run(events, modifiers, |_, ui| {
            reported = show_body(ui, &body, &pickup, &views, true, None).events;
        });
        reported
    };
    rotate(
        &mut harness,
        vec![
            egui::Event::PointerMoved(rotate_at),
            pointer(rotate_at, true),
        ],
        Default::default(),
    );
    let up = rotate_at + egui::vec2(0.0, -80.0);
    let clamped = rotate(
        &mut harness,
        vec![egui::Event::PointerMoved(up)],
        Default::default(),
    );
    let clamped_degrees = clamped.iter().find_map(|event| match event {
        BodyEvent::Rotated { index: 0, degrees } => Some(*degrees),
        _ => None,
    });
    assert_eq!(clamped_degrees, Some(45.0));
    assert!(
        clamped
            .iter()
            .all(|event| !matches!(event, BodyEvent::Moved { .. }))
    );

    let held = rotate(
        &mut harness,
        vec![egui::Event::PointerMoved(up)],
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    assert!(
        held.iter()
            .all(|event| !matches!(event, BodyEvent::Rotated { .. } | BodyEvent::Moved { .. }))
    );

    let eased = rotate_at + egui::vec2(0.0, -60.0);
    let rebased = rotate(
        &mut harness,
        vec![egui::Event::PointerMoved(eased)],
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    let degrees = rebased
        .iter()
        .find_map(|event| match event {
            BodyEvent::Rotated { index: 0, degrees } => Some(*degrees),
            _ => None,
        })
        .expect("rebased rotation");
    assert!((degrees - 43.2).abs() < 0.05, "{degrees}");
    assert!(
        rebased
            .iter()
            .all(|event| !matches!(event, BodyEvent::Moved { .. }))
    );

    let mut harness = Harness::new();
    harness.run(vec![], Default::default(), |_, ui| {
        let _ = show_body(ui, &body, &pickup, &views, true, None);
    });
    rotate(
        &mut harness,
        vec![
            egui::Event::PointerMoved(rotate_at),
            pointer(rotate_at, true),
        ],
        Default::default(),
    );
    rotate(
        &mut harness,
        vec![egui::Event::PointerMoved(up)],
        Default::default(),
    );
    let back = rotate(
        &mut harness,
        vec![egui::Event::PointerMoved(rotate_at)],
        Default::default(),
    );
    let back_degrees = back.iter().find_map(|event| match event {
        BodyEvent::Rotated { index: 0, degrees } => Some(*degrees),
        _ => None,
    });
    assert_eq!(back_degrees, Some(0.0));
}

#[test]
fn ctrl_held_first_rotation_sample_rebases_without_adding_angle() {
    let body = missing_image("variax/spank-body.png", [426, 412]);
    let pickup = missing_image("variax/spank-pickup.png", [120, 120]);
    let views = [PickupView {
        inches: 1.8,
        degrees: 8.7,
        enabled: true,
        selected: true,
    }];
    let placement = PickupPlacement::default();
    let mut harness = Harness::new();
    let mut art = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        art = show_body(ui, &body, &pickup, &views, true, None).art_rect;
    });
    let scale = placement.screen_scale(art);
    let center = placement.screen_point(art, 1.8);
    let sprite = body_canvas::sprite_art_size(pickup.pixels, placement.art) * scale;
    let rotate_at = center + egui::vec2(0.0, sprite.y * 0.4);
    let ctrl = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let show = |harness: &mut Harness, events| -> Vec<BodyEvent> {
        let mut reported = Vec::new();
        harness.run(events, ctrl, |_, ui| {
            reported = show_body(ui, &body, &pickup, &views, true, None).events;
        });
        reported
    };
    show(
        &mut harness,
        vec![
            egui::Event::PointerMoved(rotate_at),
            pointer(rotate_at, true),
        ],
    );
    let first_at = rotate_at + egui::vec2(0.0, -8.0);
    let first = show(&mut harness, vec![egui::Event::PointerMoved(first_at)]);
    assert!(first.iter().any(|event| matches!(
        event,
        BodyEvent::DragStarted {
            index: 0,
            kind: DragKind::Rotate
        }
    )));
    assert!(
        first
            .iter()
            .all(|event| !matches!(event, BodyEvent::Rotated { .. }))
    );
    let second_at = first_at + egui::vec2(0.0, -8.0);
    let second = show(&mut harness, vec![egui::Event::PointerMoved(second_at)]);
    let degrees = second
        .iter()
        .find_map(|event| match event {
            BodyEvent::Rotated { index: 0, degrees } => Some(*degrees),
            _ => None,
        })
        .expect("fine rotation after the rebase");
    let expected = body_canvas::rotate_degrees(8.7, -8.0, true);
    assert!(
        (degrees - expected).abs() < 1.0e-4,
        "{degrees} vs {expected}"
    );
    let released = show(&mut harness, vec![pointer(second_at, false)]);
    assert!(released.iter().any(|event| matches!(
        event,
        BodyEvent::DragFinished {
            index: 0,
            kind: DragKind::Rotate
        }
    )));
    assert!(
        released
            .iter()
            .all(|event| !matches!(event, BodyEvent::Rotated { .. }))
    );

    let mut harness = Harness::new();
    harness.run(vec![], Default::default(), |_, ui| {
        let _ = show_body(ui, &body, &pickup, &views, true, None);
    });
    let move_show = |harness: &mut Harness, events| -> Vec<BodyEvent> {
        let mut reported = Vec::new();
        harness.run(events, ctrl, |_, ui| {
            reported = show_body(ui, &body, &pickup, &views, true, None).events;
        });
        reported
    };
    move_show(
        &mut harness,
        vec![egui::Event::PointerMoved(center), pointer(center, true)],
    );
    let moved_to = center + egui::vec2(30.0, 0.0);
    let moved = move_show(&mut harness, vec![egui::Event::PointerMoved(moved_to)]);
    let inches = moved.iter().find_map(|event| match event {
        BodyEvent::Moved { index: 0, inches } => Some(*inches),
        _ => None,
    });
    let expected_inches = body_canvas::move_inches(1.8, 30.0 / scale, placement);
    let inches = inches.expect("ctrl-held move");
    assert!(
        (inches - expected_inches).abs() < 1.0e-3,
        "{inches} vs {expected_inches}"
    );
    assert!(
        moved
            .iter()
            .all(|event| !matches!(event, BodyEvent::Rotated { .. }))
    );
}

#[test]
fn body_outline_paints_only_when_requested() {
    let body = missing_image("variax/spank-body.png", [426, 412]);
    let outline = UiImage::missing("variax/outline.png");
    let none: &[PickupSprite] = &[];
    let mut harness = Harness::new();
    let mut art_rect = Rect::NOTHING;
    let mut panel_min = Pos2::ZERO;
    let with_outline = harness.run(vec![], Default::default(), |_, ui| {
        let slot = outline_rect(Pos2::ZERO, &outline, 1.0).size();
        let (space, _) = ui.allocate_exact_size(slot, egui::Sense::hover());
        panel_min = space.min - egui::vec2(body_canvas::OUTLINE_X, body_canvas::OUTLINE_Y);
        paint_body_outline(ui, &outline, panel_min, 1.0);
        art_rect = BodyCanvas::new(&body, none).show(ui).art_rect;
    });
    let center = label_center(&with_outline, "variax/outline.png").expect("outline name");
    let slot = outline_rect(panel_min, &outline, 1.0);
    assert_eq!(slot.size(), egui::vec2(238.0, 148.0));
    assert!((center - slot.center()).length() < 1.0);
    assert!(!art_rect.contains(center));

    let photo_only = harness.run(vec![], Default::default(), |_, ui| {
        let _ = BodyCanvas::new(&body, none).show(ui);
    });
    let labels = texts(&photo_only);
    assert!(labels.iter().all(|label| !label.contains("outline.png")));
    assert!(labels.iter().any(|label| label.contains("spank-body.png")));
}

#[test]
fn body_wireframe_uses_the_file_size_and_workbench_paths() {
    let missing = UiImage::missing("bodies/wireframe/strat.png");
    assert_eq!(
        outline_rect(Pos2::new(10.0, 20.0), &missing, 1.0),
        Rect::from_min_size(Pos2::new(54.0, 23.0), egui::vec2(238.0, 148.0))
    );
    for scale in [0.0, -2.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            outline_rect(Pos2::new(10.0, 20.0), &missing, scale),
            outline_rect(Pos2::new(10.0, 20.0), &missing, 1.0)
        );
    }
    let mut native = UiImage::missing("bodies/wireframe/lespaul.png");
    native.missing = false;
    native.pixels = [90, 140];
    assert_eq!(
        outline_rect(Pos2::ZERO, &native, 1.0),
        Rect::from_min_size(Pos2::new(44.0, 3.0), egui::vec2(90.0, 140.0))
    );
    assert_eq!(
        outline_rect(Pos2::new(100.0, 200.0), &native, 2.0),
        Rect::from_min_size(Pos2::new(188.0, 206.0), egui::vec2(180.0, 280.0))
    );

    let rows = [
        (0x0310_0000, "bodies/wireframe/tele.png"),
        (0x0310_0001, "bodies/wireframe/tele.png"),
        (0x0310_0002, "bodies/wireframe/thinline.png"),
        (0x0310_0003, "bodies/wireframe/strat.png"),
        (0x0310_0004, "bodies/wireframe/lespaul.png"),
        (0x0310_0005, "bodies/wireframe/lespaul.png"),
        (0x0310_0006, "bodies/wireframe/lespaul.png"),
        (0x0310_0007, "bodies/wireframe/juniorsinglecut.png"),
        (0x0310_0008, "bodies/wireframe/lpspecial.png"),
        (0x0310_0009, "bodies/wireframe/firebird.png"),
        (0x0310_000A, "bodies/wireframe/gretsch6120.png"),
        (0x0310_000B, "bodies/wireframe/sparklejet.png"),
        (0x0310_000C, "bodies/wireframe/ric6.png"),
        (0x0310_000D, "bodies/wireframe/ric12.png"),
        (0x0310_000E, "bodies/wireframe/es335.png"),
        (0x0310_000F, "bodies/wireframe/casino.png"),
        (0x0310_0010, "bodies/wireframe/es175.png"),
        (0x0310_0011, "bodies/wireframe/super400.png"),
        (0x0310_0012, "bodies/wireframe/martind28.png"),
        (0x0310_0013, "bodies/wireframe/martind12_28.png"),
        (0x0310_0014, "bodies/wireframe/martin018.png"),
        (0x0310_0015, "bodies/wireframe/guildf212.png"),
        (0x0310_0016, "bodies/wireframe/j200.png"),
        (0x0310_0017, "bodies/wireframe/dobro.png"),
        (0x0310_0018, "bodies/wireframe/sitar.png"),
        (0x0310_0019, "bodies/wireframe/danelectro.png"),
        (0x0310_001A, "bodies/wireframe/banjo.png"),
        (0x0310_001B, "bodies/wireframe/tricone.png"),
    ];
    for (id, path) in rows {
        assert_eq!(wireframe_path(id), path, "{id:#x}");
    }
    for id in [0, 0x0310_001C, u32::MAX] {
        assert_eq!(wireframe_path(id), "bodies/wireframe/strat.png");
    }
}

#[test]
fn tuning_panel_reports_neck_drags_and_the_table_controls() {
    let labels = std::array::from_fn(|_| "0 semitones".to_owned());
    let mut harness = Harness::new();
    let mut neck = Rect::NOTHING;
    let mut row_y = 0.0;
    let mut enable = Rect::NOTHING;
    let mut presets = Rect::NOTHING;
    let mut lock = Rect::NOTHING;
    let mut reset = Rect::NOTHING;
    let mut flat = Rect::NOTHING;
    let mut sharp = Rect::NOTHING;
    let drawn = harness.run(vec![], Default::default(), |_, ui| {
        let rows = open_rows(&labels);
        let output = show_tuning(ui, &rows, true, false, true);
        neck = output.neck;
        row_y = output.rows.iter().find(|row| row.id == 5).unwrap().y;
        enable = output.enable;
        presets = output.presets;
        lock = output.lock;
        reset = output.reset;
        flat = output.flat;
        sharp = output.sharp;
    });
    let labels_drawn = texts(&drawn);
    for name in [
        "String",
        "Shift",
        "Note",
        "High E",
        "Low E",
        "0 semitones",
        "Lock",
        "Reset",
        "\u{2212}12",
        "+12",
        "\u{266d}",
        "\u{266f}",
    ] {
        assert!(
            labels_drawn.iter().any(|label| label.contains(name)),
            "missing {name}"
        );
    }

    let start = tuning_panel::marker_center(neck, row_y, 0);
    let end = Pos2::new(tuning_panel::scale_x(neck, 5), row_y);
    harness.run(
        vec![egui::Event::PointerMoved(start), pointer(start, true)],
        Default::default(),
        |_, ui| {
            let rows = open_rows(&labels);
            let _ = show_tuning(ui, &rows, true, false, true);
        },
    );
    harness.run(
        vec![egui::Event::PointerMoved(end)],
        Default::default(),
        |_, ui| {
            let rows = open_rows(&labels);
            assert_eq!(
                show_tuning(ui, &rows, true, false, true).events,
                vec![
                    TuningEvent::DragStarted { id: 5 },
                    TuningEvent::Shift {
                        id: 5,
                        semitones: 5
                    },
                ]
            );
        },
    );
    harness.run(vec![pointer(end, false)], Default::default(), |_, ui| {
        let rows = open_rows(&labels);
        assert_eq!(
            show_tuning(ui, &rows, true, false, true).events,
            vec![TuningEvent::DragFinished { id: 5 }]
        );
    });

    let at_seven = Pos2::new(tuning_panel::scale_x(neck, 7), row_y);
    let mut seen = Vec::new();
    click_at(&mut harness, at_seven, |ui| {
        let rows = open_rows(&labels);
        seen = show_tuning(ui, &rows, true, false, true).events;
    });
    assert_eq!(
        seen,
        vec![TuningEvent::Shift {
            id: 5,
            semitones: 7
        }]
    );

    let clicks = [
        (enable, TuningEvent::Enable),
        (presets, TuningEvent::Presets),
        (reset, TuningEvent::Reset),
        (flat, TuningEvent::FlattenAll),
        (sharp, TuningEvent::SharpenAll),
    ];
    for (rect, expected) in clicks {
        click_at(&mut harness, rect.center(), |ui| {
            let rows = open_rows(&labels);
            seen = show_tuning(ui, &rows, true, false, true).events;
        });
        assert_eq!(seen, vec![expected]);
    }
    click_at(&mut harness, lock.center(), |ui| {
        let rows = open_rows(&labels);
        seen = show_tuning(ui, &rows, true, false, true).events;
    });
    assert!(
        seen.iter()
            .all(|event| !matches!(event, TuningEvent::Lock(_)))
    );

    let mut harness = Harness::new();
    let mut lock = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        let rows = open_rows(&labels);
        lock = show_tuning(ui, &rows, true, true, true).lock;
    });
    let mut seen = Vec::new();
    click_at(&mut harness, lock.center(), |ui| {
        let rows = open_rows(&labels);
        seen = show_tuning(ui, &rows, true, true, true).events;
    });
    assert_eq!(seen, vec![TuningEvent::Lock(true)]);

    let mut harness = Harness::new();
    let mut sharp = Rect::NOTHING;
    harness.run(vec![], Default::default(), |_, ui| {
        let rows = open_rows(&labels);
        sharp = show_tuning(ui, &rows, true, false, false).sharp;
    });
    let mut seen = Vec::new();
    click_at(&mut harness, sharp.center(), |ui| {
        let rows = open_rows(&labels);
        seen = show_tuning(ui, &rows, true, false, false).events;
    });
    assert!(seen.is_empty());

    let mut harness = Harness::new();
    let mut neck = Rect::NOTHING;
    let mut row_y = 0.0;
    harness.run(vec![], Default::default(), |_, ui| {
        let rows = open_rows(&labels);
        let output = show_tuning(ui, &rows, false, false, true);
        neck = output.neck;
        row_y = output.rows.iter().find(|row| row.id == 5).unwrap().y;
    });
    let start = tuning_panel::marker_center(neck, row_y, 0);
    let end = Pos2::new(tuning_panel::scale_x(neck, 5), row_y);
    harness.run(
        vec![egui::Event::PointerMoved(start), pointer(start, true)],
        Default::default(),
        |_, ui| {
            let rows = open_rows(&labels);
            assert!(show_tuning(ui, &rows, false, false, true).events.is_empty());
        },
    );
    harness.run(
        vec![egui::Event::PointerMoved(end)],
        Default::default(),
        |_, ui| {
            let rows = open_rows(&labels);
            assert!(show_tuning(ui, &rows, false, false, true).events.is_empty());
        },
    );
}

fn click_at(harness: &mut Harness, pos: Pos2, mut draw: impl FnMut(&mut egui::Ui)) {
    harness.run(
        vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        Default::default(),
        |_, ui| draw(ui),
    );
    harness.run(vec![pointer(pos, false)], Default::default(), |_, ui| {
        draw(ui);
    });
}

fn type_tuning_value(
    harness: &mut Harness,
    pos: Pos2,
    value: &str,
    mut draw: impl FnMut(&mut egui::Ui),
) {
    click_widget(harness, pos, &mut draw);
    harness.run(vec![], Default::default(), |_, ui| draw(ui));
    harness.run(
        vec![egui::Event::Text(value.into())],
        Default::default(),
        |_, ui| draw(ui),
    );
    harness.run(vec![key(egui::Key::Enter)], Default::default(), |_, ui| {
        draw(ui)
    });
    harness.run(
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        }],
        Default::default(),
        |_, ui| draw(ui),
    );
}

#[test]
fn tuning_shift_cells_accept_typed_semitones_and_validate_entry() {
    let labels = std::array::from_fn(|_| "0 semitones".to_owned());
    let rows = open_rows(&labels);
    for (input, expected) in [
        ("−3 semitones", Some(-3)),
        ("21", Some(12)),
        ("1.7", Some(2)),
        ("NaN", None),
        ("invalid", None),
    ] {
        let mut harness = Harness::new();
        let mut cell = Rect::NOTHING;
        harness.run(vec![], Default::default(), |_, ui| {
            cell = TuningPanel::new(&rows).show(ui).rows[0].shift_cell;
        });
        let mut seen = Vec::new();
        type_tuning_value(&mut harness, cell.center(), input, |ui| {
            seen.extend(TuningPanel::new(&rows).show(ui).events);
        });
        assert_eq!(
            seen,
            expected
                .map(|semitones| TuningEvent::Shift { id: 5, semitones })
                .into_iter()
                .collect::<Vec<_>>(),
            "{input}"
        );
        assert_eq!(rows[0].shift, 0, "the caller applies events");
    }
}

#[test]
fn tuning_twelve_string_mode_shows_optional_values_and_reports_edits() {
    let labels = std::array::from_fn(|_| "0 semitones".to_owned());
    let rows = open_rows(&labels);
    let values = [TwelveStringRow {
        id: 5,
        mix: Some(0.5),
        detune: Some(3.25),
    }];
    let mut harness = Harness::new();
    let absent = harness.run(vec![], Default::default(), |_, ui| {
        let output = TuningPanel::new(&rows).show(ui);
        assert!(
            output
                .rows
                .iter()
                .all(|row| row.mix_cell.is_none() && row.detune_cell.is_none())
        );
    });
    assert!(
        !texts(&absent)
            .iter()
            .any(|line| line == "Mix" || line == "Detune")
    );

    let mut mix = Rect::NOTHING;
    let mut detune = Rect::NOTHING;
    let mut missing = Rect::NOTHING;
    let visible = harness.run(vec![], Default::default(), |_, ui| {
        let output = TuningPanel::new(&rows).twelve_string(&values).show(ui);
        mix = output.rows[0].mix_cell.unwrap();
        detune = output.rows[0].detune_cell.unwrap();
        missing = output.rows[1].mix_cell.unwrap();
        assert!(detune.right() < output.neck.left());
    });
    let labels = texts(&visible);
    for name in ["Mix", "Detune", "50%", "+3 cents", "—"] {
        assert!(labels.iter().any(|line| line == name), "{name}: {labels:?}");
    }

    let mut output = Vec::new();
    type_tuning_value(&mut harness, missing.center(), "77", |ui| {
        output.extend(
            TuningPanel::new(&rows)
                .twelve_string(&values)
                .show(ui)
                .events,
        );
    });
    assert!(output.is_empty());
    type_tuning_value(&mut harness, mix.center(), "75%", |ui| {
        output.extend(
            TuningPanel::new(&rows)
                .twelve_string(&values)
                .show(ui)
                .events,
        );
    });
    assert_eq!(
        output,
        [TuningEvent::Mix {
            id: 5,
            fraction: 0.75
        }]
    );

    click_widget(&mut harness, detune.center(), |ui| {
        TuningPanel::new(&rows).twelve_string(&values).show(ui);
    });
    let editing = harness.run(vec![], Default::default(), |_, ui| {
        TuningPanel::new(&rows).twelve_string(&values).show(ui);
    });
    let editing_labels = texts(&editing);
    assert!(
        editing_labels.iter().any(|line| line == "+3.25 cents"),
        "{editing_labels:?}"
    );
    let typed = harness.run(
        vec![egui::Event::Text("-7.25 cents".into())],
        Default::default(),
        |_, ui| {
            output.extend(
                TuningPanel::new(&rows)
                    .twelve_string(&values)
                    .show(ui)
                    .events,
            );
        },
    );
    assert!(
        texts(&typed).iter().any(|line| line == "-7.25 cents"),
        "{:?}",
        texts(&typed)
    );
    assert_eq!(output.len(), 1);
    harness.run(vec![key(egui::Key::Enter)], Default::default(), |_, ui| {
        output.extend(
            TuningPanel::new(&rows)
                .twelve_string(&values)
                .show(ui)
                .events,
        );
    });
    assert_eq!(
        output[1],
        TuningEvent::Detune {
            id: 5,
            cents: -7.25
        }
    );
}

#[test]
fn document_tab_reserves_a_gap_for_the_unsaved_marker() {
    let mut harness = Harness::new();
    let title =
        "Variax Workbench · Stratocaster with a very long filename and a long version number.vwm";
    let output = harness.run(vec![], Default::default(), |_, ui| {
        chrome::DocumentTab {
            title,
            family: Family::Variax,
            active: true,
            unsaved: true,
        }
        .show(ui);
    });
    let mut parts = output.shapes.iter().filter_map(|shape| match &shape.shape {
        egui::Shape::Text(text)
            if text.galley.text() == title || text.galley.text() == "Unsaved" =>
        {
            Some(text)
        }
        _ => None,
    });
    let marker = parts.next().unwrap();
    let file = parts.next().unwrap();
    assert_eq!(marker.galley.text(), "Unsaved");
    assert_eq!(file.galley.text(), title);
    assert!(file.galley.elided);
    assert!(file.pos.x + file.galley.size().x + 6.0 <= marker.pos.x);
}

#[test]
fn fitted_stage_keeps_aspect_and_uses_the_tighter_side() {
    let art = body_canvas::DEFAULT_ART;
    let height_limited = body_canvas::fitted_stage(art, egui::vec2(800.0, 200.0));
    assert!((height_limited.y - 200.0).abs() < 1.0e-3);
    assert!(height_limited.x < 800.0);
    assert!((height_limited.x / height_limited.y - art.x / art.y).abs() < 1.0e-3);

    let width_limited = body_canvas::fitted_stage(art, egui::vec2(100.0, 400.0));
    assert!((width_limited.x - 100.0).abs() < 1.0e-3);
    assert!((width_limited.x / width_limited.y - art.x / art.y).abs() < 1.0e-3);

    assert_eq!(
        body_canvas::fitted_stage(art, egui::vec2(100_000.0, 100_000.0)),
        art * 64.0
    );
    assert_eq!(
        body_canvas::fitted_stage(egui::vec2(0.0, 10.0), egui::vec2(50.0, 50.0)),
        egui::vec2(5.0, 50.0)
    );
}

fn assert_aspect(art: Vec2, size: Vec2) {
    assert!(
        (size.x / size.y - art.x / art.y).abs() < 1.0e-3,
        "art {art:?} drew {size:?}"
    );
}

fn assert_inside(outer: Rect, inner: Rect) {
    assert!(
        inner.min.x >= outer.min.x - 0.05
            && inner.min.y >= outer.min.y - 0.05
            && inner.max.x <= outer.max.x + 0.05
            && inner.max.y <= outer.max.y + 0.05,
        "{inner:?} outside {outer:?}"
    );
}

fn assert_regions_fit(workspace: Rect, art: Vec2, regions: EditorRegions) {
    assert_inside(workspace, regions.body);
    assert_inside(workspace, regions.inspector);
    assert_inside(workspace, regions.tuning);
    assert_aspect(art, regions.body_art);
    assert!(
        (regions.body.size() - regions.body_art).length() < 1.0e-3,
        "{regions:?}"
    );
    assert!(regions.inspector.left() >= regions.body.right() - 0.1);
    assert!(regions.tuning.width() <= workspace.width() + 0.1);
    let (_, tune_max) = editor_layout::tuning_width_bounds(regions.twelve_string_width > 0.0);
    assert!(
        regions.tuning.width() <= tune_max + 0.1,
        "tuning {} cap {tune_max}",
        regions.tuning.width()
    );
}

#[test]
fn editor_regions_keep_a_useful_body_and_a_contained_neck_on_the_laptop() {
    let art = body_canvas::DEFAULT_ART;
    let workspace = editor_layout::laptop_workspace();
    assert!((workspace.width() - 1120.0).abs() < 1.0e-3);
    assert!((workspace.height() - 527.0).abs() < 1.0e-3);
    assert!((editor_layout::SHELL_FOOTER_HEIGHT - 29.0).abs() < 1.0e-3);

    let collapsed = editor_layout::editor_regions(workspace, art, false);
    assert_eq!(collapsed.twelve_string_width, 0.0);
    assert_regions_fit(workspace, art, collapsed);
    let body_scale = collapsed.body_art.x / art.x;
    assert!(
        body_scale >= 1.0,
        "laptop body scale {body_scale}, well {:?}",
        collapsed.body
    );
    assert!(
        collapsed.body.width() >= workspace.width() * 0.45,
        "body {} workspace {}",
        collapsed.body.width(),
        workspace.width()
    );
    assert!(collapsed.body.height() + 0.1 >= workspace.height());
    assert!(collapsed.tuning.left() >= collapsed.body.right());
    assert!(collapsed.tuning.width() <= workspace.width() - 400.0);
    let neck = collapsed.tuning.width()
        - editor_layout::TUNING_TABLE_WIDTH
        - editor_layout::TUNING_CHROME_WIDTH;
    assert!(
        (editor_layout::NECK_MIN..=editor_layout::NECK_MAX).contains(&neck),
        "neck {neck}"
    );
    assert!(collapsed.inspector.width() >= editor_layout::INSPECTOR_MIN_WIDTH);

    let open = editor_layout::editor_regions(workspace, art, true);
    assert_eq!(
        open.twelve_string_width,
        editor_layout::TWELVE_STRING_COLUMN
    );
    assert_regions_fit(workspace, art, open);
    let open_scale = open.body_art.x / art.x;
    assert!(open_scale >= editor_layout::SIDE_MIN_SCALE, "{open_scale}");
    let open_neck = open.tuning.width()
        - open.twelve_string_width
        - editor_layout::TUNING_TABLE_WIDTH
        - editor_layout::TUNING_CHROME_WIDTH;
    assert!(
        open_neck + 0.1 >= editor_layout::NECK_MIN,
        "twelve-string neck {open_neck}"
    );
    assert!(open.tuning.left() >= open.body.right());

    let wide = editor_layout::editor_regions(
        Rect::from_min_size(Pos2::ZERO, egui::vec2(1680.0, 860.0)),
        art,
        false,
    );
    assert_regions_fit(
        Rect::from_min_size(Pos2::ZERO, egui::vec2(1680.0, 860.0)),
        art,
        wide,
    );
    let (_, wide_cap) = editor_layout::tuning_width_bounds(false);
    assert!((wide.tuning.width() - wide_cap).abs() < 0.1);

    let narrow = editor_layout::editor_regions(
        Rect::from_min_size(Pos2::ZERO, egui::vec2(720.0, 400.0)),
        art,
        false,
    );
    assert_regions_fit(
        Rect::from_min_size(Pos2::ZERO, egui::vec2(720.0, 400.0)),
        art,
        narrow,
    );
    assert_eq!(narrow.twelve_string_width, 0.0);
    assert!(narrow.tuning.top() >= narrow.body.bottom() - 0.1);

    let again = editor_layout::editor_regions(workspace, art, false);
    assert_eq!(collapsed, again);
    let broken = editor_layout::editor_regions(
        Rect::from_min_size(Pos2::ZERO, egui::vec2(f32::NAN, -20.0)),
        egui::vec2(f32::INFINITY, 0.0),
        false,
    );
    assert!(broken.body.width().is_finite() && broken.tuning.height().is_finite());
}

#[test]
fn stacked_twelve_string_body_uses_the_short_960_workspace() {
    let art = body_canvas::DEFAULT_ART;
    // Lane 3 measured this workspace under a 960×680 window with the
    // twelve-string bank selected: tuning width 944 and an even split of
    // 140.5, so the page is 944×297. The 426×412 photo fitted to 145×141.
    let workspace = Rect::from_min_size(Pos2::ZERO, egui::vec2(944.0, 297.0));
    let regions = editor_layout::editor_regions(workspace, art, true);
    assert_regions_fit(workspace, art, regions);
    assert_eq!(
        regions.twelve_string_width,
        editor_layout::TWELVE_STRING_COLUMN
    );
    assert!(regions.tuning.top() + 0.1 >= regions.body.bottom());
    let scale = regions.body_art.x / art.x;
    assert!(
        (regions.body_art.x - 180.1).abs() < 1.0 && (regions.body_art.y - 174.2).abs() < 1.0,
        "narrow twelve-string art {:?} scale {scale}",
        regions.body_art
    );
    assert!(scale > 0.40, "narrow twelve-string scale {scale}");
    assert!(
        art.y * editor_layout::SIDE_MIN_SCALE > workspace.height(),
        "a 0.75-scale body would fit a {}-point workspace",
        workspace.height()
    );
    let laptop = editor_layout::editor_regions(editor_layout::laptop_workspace(), art, true);
    assert!(laptop.tuning.left() + 0.1 >= laptop.body.right());
}

#[test]
fn editor_regions_preserve_pickup_mapping_and_hit_testing_at_both_densities() {
    let art = body_canvas::DEFAULT_ART;
    let placement = PickupPlacement::default();
    let body = missing_image("variax/spank-body.png", [426, 412]);
    let pickup = missing_image("variax/spank-pickup.png", [120, 120]);
    let workspace = editor_layout::laptop_workspace();
    let regions = editor_layout::editor_regions(workspace, art, false);
    let mut centers = Vec::new();
    let mut arts = Vec::new();
    for pixels_per_point in [1.0_f32, 2.0] {
        let mut harness = Harness::new();
        let mut art_rect = Rect::NOTHING;
        let screen = workspace.size();
        harness.run_screen(screen, pixels_per_point, vec![], |_, ui| {
            let laid = editor_layout::editor_regions(ui.max_rect(), art, false);
            assert_eq!(laid.body.size(), regions.body.size());
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("body", pixels_per_point.to_bits()))
                    .max_rect(laid.body),
            );
            let sprites = [PickupSprite {
                position_inches: 1.8,
                angle_degrees: 8.7,
                enabled: true,
                selected: true,
                image: &pickup,
            }];
            art_rect = BodyCanvas::new(&body, &sprites)
                .placement(placement)
                .show(&mut child)
                .art_rect;
        });
        assert!(
            (harness.ctx.pixels_per_point() - pixels_per_point).abs() < 0.01,
            "{}",
            harness.ctx.pixels_per_point()
        );
        assert_aspect(art, art_rect.size());
        assert!(
            (art_rect.width() - regions.body.width()).abs() < 1.0,
            "ppp {pixels_per_point} art {} well {}",
            art_rect.width(),
            regions.body.width()
        );
        assert!(
            (art_rect.height() - regions.body.height()).abs() < 1.0,
            "ppp {pixels_per_point} art height {}",
            art_rect.height()
        );
        let scale = placement.screen_scale(art_rect);
        assert!((scale - art_rect.width() / placement.art.x).abs() < 1.0e-3);
        assert!((art_rect.height() / placement.art.y - scale).abs() < 1.0e-2);
        let center = placement.screen_point(art_rect, 1.8);
        let mapped = art_rect.min + placement.position_to_art(1.8).to_vec2() * scale;
        assert!((center - mapped).length() < 0.05, "{center} vs {mapped}");
        let mut reported = Vec::new();
        harness.run_screen(
            screen,
            pixels_per_point,
            vec![egui::Event::PointerMoved(center), pointer(center, true)],
            |_, ui| {
                let laid = editor_layout::editor_regions(ui.max_rect(), art, false);
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("body", pixels_per_point.to_bits()))
                        .max_rect(laid.body),
                );
                let sprites = [PickupSprite {
                    position_inches: 1.8,
                    angle_degrees: 8.7,
                    enabled: true,
                    selected: true,
                    image: &pickup,
                }];
                reported = BodyCanvas::new(&body, &sprites)
                    .placement(placement)
                    .show(&mut child)
                    .events;
            },
        );
        assert!(
            reported
                .iter()
                .any(|event| matches!(event, BodyEvent::PickupClicked { index: 0 })),
            "ppp {pixels_per_point} events {reported:?}"
        );
        centers.push(center);
        arts.push(art_rect);
    }
    assert!((arts[0].size() - arts[1].size()).length() < 0.5);
    assert!((centers[0] - centers[1]).length() < 0.5);
}

#[test]
fn editor_regions_omit_the_unavailable_twelve_string_column() {
    let art = body_canvas::DEFAULT_ART;
    let workspace = editor_layout::laptop_workspace();
    for (twelve, pixels_per_point) in [(false, 1.0_f32), (false, 2.0), (true, 1.0), (true, 2.0)] {
        let regions = editor_layout::editor_regions(workspace, art, twelve);
        let mut harness = Harness::new();
        let mut neck = Rect::NOTHING;
        let rows = [TuningRow {
            id: 0,
            name: "Low E",
            shift: 0,
            shift_label: "0 semitones",
            note: "E",
        }];
        let output = harness.run_screen(workspace.size(), pixels_per_point, vec![], |_, ui| {
            let laid = editor_layout::editor_regions(ui.max_rect(), art, twelve);
            assert_eq!(laid.twelve_string_width, regions.twelve_string_width);
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("tuning", twelve, pixels_per_point.to_bits()))
                    .max_rect(laid.tuning),
            );
            child.horizontal_top(|ui| {
                if laid.twelve_string_width > 0.0 {
                    ui.add_space(laid.twelve_string_width);
                }
                neck = TuningPanel::new(&rows).show(ui).neck;
            });
        });
        assert!((output.pixels_per_point - pixels_per_point).abs() < 0.01);
        let labels = texts(&output);
        assert!(
            labels.iter().all(|label| label != "No 12-String available"),
            "{labels:?}"
        );
        assert!(labels.iter().any(|label| label == "Low E"));
        assert!(labels.iter().any(|label| label == "Enable"));
        assert!(
            regions.tuning.contains_rect(neck.shrink(1.0)),
            "ppp {pixels_per_point} twelve {twelve} neck {neck:?} band {:?}",
            regions.tuning
        );
        assert!(
            neck.width() <= editor_layout::NECK_MAX + 1.0,
            "neck {}",
            neck.width()
        );
        let from_band = neck.left() - regions.tuning.left();
        let table_and_frame =
            editor_layout::TUNING_TABLE_WIDTH + editor_layout::TUNING_CHROME_WIDTH;
        if twelve {
            assert!(
                from_band >= regions.twelve_string_width + editor_layout::TUNING_TABLE_WIDTH - 24.0,
                "twelve-string neck starts at {from_band}"
            );
        } else {
            assert!(
                (table_and_frame - 24.0..=table_and_frame + 24.0).contains(&from_band),
                "collapsed neck starts at {from_band}, not after a 204-point column"
            );
        }
        let enable = label_center(&output, "Enable").expect("Enable");
        let mut clicked = Vec::new();
        harness.run_screen(
            workspace.size(),
            pixels_per_point,
            vec![
                egui::Event::PointerMoved(enable),
                pointer(enable, true),
                pointer(enable, false),
            ],
            |_, ui| {
                let laid = editor_layout::editor_regions(ui.max_rect(), art, twelve);
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt(("tuning", twelve, pixels_per_point.to_bits()))
                        .max_rect(laid.tuning),
                );
                child.horizontal_top(|ui| {
                    if laid.twelve_string_width > 0.0 {
                        ui.add_space(laid.twelve_string_width);
                    }
                    clicked = TuningPanel::new(&rows).show(ui).events;
                });
            },
        );
        assert!(
            clicked.contains(&TuningEvent::Enable),
            "ppp {pixels_per_point} twelve {twelve} {clicked:?}"
        );
    }
}
