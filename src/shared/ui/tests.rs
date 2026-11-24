use eframe::egui::{self, Color32, Modifiers, Pos2, Rect, Vec2};

use super::assets::{self, UiImage};
use super::chrome::{self, DialogButton, DialogEvent, Family, LinkState};
use super::controls::{Selector, Switch};
use super::gallery::GalleryState;
use super::knob::{self, Knob};
use super::theme::{
    self, AMBER, BG, DANGER, DIM, FOCUS, HD, INK, LINE, MUTED, OK, PANEL, PODXT, RAISED, TEXT,
    VARIAX,
};

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
