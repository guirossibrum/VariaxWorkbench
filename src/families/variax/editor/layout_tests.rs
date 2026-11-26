use super::{BODY, PICKUP1, TUNING, Tab, egui, tuning_presets::DETUNE};
use crate::editor::{EditorDefaults, Workbench, registry, tests::app};
use variax_workbench::ui::{
    body_canvas::move_inches,
    editor_layout::{
        NECK_MAX, SIDE_MIN_SCALE, TUNING_CHROME_WIDTH, TUNING_TABLE_WIDTH, TWELVE_STRING_COLUMN,
    },
};

const SIZES: [egui::Vec2; 2] = [egui::vec2(1120.0, 760.0), egui::vec2(960.0, 680.0)];

fn render(
    app: &mut Workbench,
    ctx: &egui::Context,
    size: egui::Vec2,
    pixels_per_point: f32,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        modifiers,
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .native_pixels_per_point = Some(pixels_per_point);
    ctx.run(input, |ctx| app.render(ctx))
}

fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn text_rect(output: &egui::FullOutput, expected: &str) -> Option<egui::Rect> {
    fn bounds(shape: &egui::Shape, expected: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == expected => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| bounds(shape, expected)),
            _ => None,
        }
    }

    output.shapes.iter().find_map(|shape| {
        let rect = bounds(&shape.shape, expected)?.intersect(shape.clip_rect);
        rect.is_positive().then_some(rect)
    })
}

fn assert_footer(output: &egui::FullOutput, size: egui::Vec2, tuning: egui::Rect) {
    let footer =
        text_rect(output, &format!("Build {}", crate::BUILD_NUMBER)).expect("build footer");
    assert!(
        footer.top() >= tuning.bottom() - 1.0,
        "footer {footer:?} vs tuning {tuning:?}"
    );
    assert!(footer.right() > size.x - 30.0 && footer.right() <= size.x);
    assert!(footer.bottom() <= size.y);
}

#[test]
fn scrolled_tuning_labels_cannot_paint_in_the_footer() {
    let ctx = egui::Context::default();
    let mut app = Workbench::new(
        &ctx,
        EditorDefaults {
            artwork: Default::default(),
            preferences: None,
            startup_panel: None,
        },
    );
    app.theme_polled = std::time::Instant::now();
    app.new_tab(registry::LEGACY_STARTUP);
    let size = SIZES[0];
    let output = render(&mut app, &ctx, size, 1.0, vec![], egui::Modifiers::NONE);
    let footer = egui::containers::panel::PanelState::load(&ctx, egui::Id::new("status"))
        .unwrap()
        .rect;
    let tuning = app.variax().workspace_tuning.unwrap();
    assert!(tuning.bottom() <= footer.top());
    assert_footer(&output, size, tuning);
    for label in ["-12", "-7", "-5", "0", "+5", "+7", "+12"] {
        if let Some(bounds) = text_rect(&output, label) {
            assert!(
                bounds.bottom() <= footer.top(),
                "{label}: {bounds:?} vs {footer:?}"
            );
        }
    }
}

#[test]
fn normal_editor_fits_body_and_tuning_and_retains_pickup_drag_and_keyboard_undo() {
    for size in SIZES {
        for pixels_per_point in [1.0, 2.0] {
            let (ctx, mut app) = app();
            app.clear_window_controls = true;
            app.variax_mut().open_example();
            app.variax_mut()
                .change(|preset| preset.select_model(BODY, 0x0310_0003));
            app.variax_mut().install_pickup_fixture(&ctx);
            app.variax_mut().tab = Tab::Body;
            let mut frame = |events, modifiers| {
                render(&mut app, &ctx, size, pixels_per_point, events, modifiers)
            };
            frame(vec![], egui::Modifiers::NONE);
            let output = frame(vec![], egui::Modifiers::NONE);
            assert!((output.pixels_per_point - pixels_per_point).abs() < 0.01);

            let editor = app.variax();
            let (well, inspector) = editor.workspace_cards.unwrap();
            let tuning = editor.workspace_tuning.unwrap();
            let art = editor.body_art_rect.unwrap();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            for region in [well, inspector, tuning] {
                assert!(screen.contains_rect(region), "{size:?}: {region:?}");
            }
            let placement = editor.body_placement();
            let scale = placement.screen_scale(art);
            assert!((art.size() - placement.art * scale).length() < 1.0);
            assert!((well.size() - art.size()).length() < 1.0);
            assert!(well.right() <= inspector.left());
            assert!(
                scale >= if size.x == 1120.0 { 1.0 } else { 0.8 },
                "{size:?}: art {art:?}, tuning {tuning:?}"
            );
            let neck = editor.tuning_widgets["Neck"];
            assert!(neck.width() <= NECK_MAX + 1.0 && neck.right() <= tuning.right() + 1.0);
            assert!(neck.left() - tuning.left() < TUNING_TABLE_WIDTH + TUNING_CHROME_WIDTH + 12.0);
            assert!(text_rect(&output, "No 12-String available").is_none());
            assert!(!editor.tuning_widgets.contains_key("Low E mix"));
            assert!(!editor.tuning_widgets.contains_key("Low E detune"));
            assert!(text_rect(&output, "Pickups").is_some());
            assert_footer(&output, size, tuning);

            let pos = editor.float(PICKUP1, 0x200000).unwrap();
            let center = placement.screen_point(art, pos);
            assert!(editor.pickup_centers[&PICKUP1].distance(center) < 0.1);
            let before = editor.document.as_ref().unwrap().as_bytes().to_vec();
            let end = center + egui::vec2(36.0 * scale, 0.0);
            let mut frame = |events, modifiers| {
                render(&mut app, &ctx, size, pixels_per_point, events, modifiers)
            };
            frame(
                vec![egui::Event::PointerMoved(center), pointer(center, true)],
                egui::Modifiers::NONE,
            );
            frame(vec![egui::Event::PointerMoved(end)], egui::Modifiers::NONE);
            frame(vec![pointer(end, false)], egui::Modifiers::NONE);
            let editor = app.variax();
            assert_eq!(editor.tab, Tab::Pickups);
            let actual = editor.float(PICKUP1, 0x200000).unwrap();
            assert!((actual - move_inches(pos, 36.0, placement)).abs() < 1e-4);
            let command = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            render(
                &mut app,
                &ctx,
                size,
                pixels_per_point,
                vec![egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: command,
                }],
                command,
            );
            assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
        }
    }
}

#[test]
fn twelve_string_mix_and_detune_stay_in_the_tuning_band_at_both_densities() {
    for size in SIZES {
        for pixels_per_point in [1.0, 2.0] {
            let (ctx, mut app) = app();
            app.clear_window_controls = true;
            app.variax_mut().open_example();
            app.variax_mut()
                .document
                .as_mut()
                .unwrap()
                .select(0, 4)
                .unwrap();
            app.variax_mut()
                .change(|preset| preset.set_parameter(TUNING, DETUNE, 1, 12.0_f32.to_bits()));
            assert!(app.variax().has_twelve_string());
            let frame = |app: &mut Workbench, events| {
                render(
                    app,
                    &ctx,
                    size,
                    pixels_per_point,
                    events,
                    egui::Modifiers::NONE,
                )
            };
            frame(&mut app, vec![]);
            let output = frame(&mut app, vec![]);
            let editor = app.variax();
            let tuning = editor.workspace_tuning.unwrap();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            assert!(screen.contains_rect(tuning), "{size:?}: {tuning:?}");
            let placement = editor.body_placement();
            let art = editor.body_art_rect.unwrap();
            let scale = placement.screen_scale(art);
            assert!((art.size() - placement.art * scale).length() < 1.0);
            if size.x == 1120.0 {
                assert!(scale >= SIDE_MIN_SCALE, "{size:?}: {art:?}");
            }
            let neck = editor.tuning_widgets["Neck"];
            assert!(neck.left() - tuning.left() >= TWELVE_STRING_COLUMN + TUNING_TABLE_WIDTH);
            assert!(neck.width() <= NECK_MAX + 1.0 && neck.right() <= tuning.right() + 1.0);
            assert!(text_rect(&output, "Mix").is_some());
            assert!(text_rect(&output, "Detune").is_some());
            assert_footer(&output, size, tuning);

            let reset = app.variax().tuning_widgets["Zero detune"].center();
            if !tuning.contains(reset) {
                frame(
                    &mut app,
                    vec![
                        egui::Event::PointerMoved(tuning.center()),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -400.0),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                for _ in 0..20 {
                    frame(&mut app, vec![]);
                }
            }
            let widgets = &app.variax().tuning_widgets;
            for label in ["Low E mix", "Low E detune", "Zero detune"] {
                assert!(
                    tuning.contains(widgets[label].center()),
                    "{size:?} at {pixels_per_point}: {label} {:?} out of {tuning:?}",
                    widgets[label]
                );
            }
            let reset = widgets["Zero detune"].center();
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(reset), pointer(reset, true)],
            );
            frame(&mut app, vec![pointer(reset, false)]);
            assert_eq!(app.variax().float(TUNING, DETUNE), Some(0.0));
        }
    }
}
