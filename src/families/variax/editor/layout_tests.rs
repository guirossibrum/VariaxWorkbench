use super::{BODY, PICKUP1, SYSTEM, TUNING, Tab, egui, tuning_presets::DETUNE};
use crate::editor::{EditorDefaults, Workbench, registry, tests::app};
use variax_workbench::preset::{PresetText, legacy_text_issue};
use variax_workbench::ui::{
    body_canvas::{PICKUP_SLOT_PX, POSITION_MAX, POSITION_MIN},
    editor_layout::{COLUMN_GAP, TUNING_CHROME_WIDTH, TUNING_TABLE_WIDTH, TWELVE_STRING_COLUMN},
    tuning_panel::scale_x,
};

const SIZES: [egui::Vec2; 2] = [egui::vec2(1120.0, 760.0), egui::vec2(960.0, 680.0)];
const OUTLINE_TEST_SIZE: egui::Vec2 = egui::vec2(1120.0, 900.0);

fn bottom_band_sizes() -> impl Iterator<Item = egui::Vec2> {
    SIZES.into_iter().chain([egui::vec2(1440.0, 900.0)])
}

fn density_and_route() -> impl Iterator<Item = (f32, bool)> {
    [1.0, 2.0].into_iter().flat_map(|density| {
        [false, true]
            .into_iter()
            .filter(|&connected| !connected || cfg!(feature = "usb"))
            .map(move |connected| (density, connected))
    })
}

fn set_verified_route(app: &mut Workbench, connected: bool) {
    #[cfg(feature = "usb")]
    if connected {
        let index = app.tabs.index(app.tabs.active.unwrap()).unwrap();
        app.tabs.entries[index].device = Some(super::refresh::tests::verified_tab());
    }
    #[cfg(not(feature = "usb"))]
    let _ = (app, connected);
}

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
    output.shapes.iter().find_map(|shape| {
        let rect = text_bounds(&shape.shape, expected)?.intersect(shape.clip_rect);
        rect.is_positive().then_some(rect)
    })
}

fn text_bounds(shape: &egui::Shape, expected: &str) -> Option<egui::Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == expected => {
            Some(text.galley.rect.translate(text.pos.to_vec2()))
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_bounds(shape, expected)),
        _ => None,
    }
}

#[cfg(feature = "usb")]
fn glyph_bounds(shape: &egui::Shape, expected: &str) -> Option<egui::Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == expected => {
            Some(text.galley.mesh_bounds.translate(text.pos.to_vec2()))
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| glyph_bounds(shape, expected)),
        _ => None,
    }
}

#[cfg(feature = "usb")]
#[test]
fn compact_toolbar_keeps_route_in_footer_and_device_actions_keyboard_reachable() {
    const ROUTE: &str = "Guitar: Variax 600 3.10 via PODxt Live";
    const ACTIONS: &[&str] = &[
        "Open…",
        "Save",
        "Save As…",
        "New offline tab",
        "Detect devices",
        "New Variax file",
        "Guitar actions",
        "Tools",
        "Help",
    ];
    let build = format!("Build {}", crate::BUILD_NUMBER);

    for (size, mac_inset) in SIZES
        .into_iter()
        .map(|size| (size, true))
        .chain([(egui::vec2(1301.0, 974.0), false)])
    {
        for density in [1.0, 2.0] {
            let (ctx, mut app) = chime_editor(true);
            app.clear_window_controls = mac_inset;
            let none = egui::Modifiers::NONE;
            let frame =
                |app: &mut Workbench, events| render(app, &ctx, size, density, events, none);
            frame(&mut app, vec![]);
            let output = frame(&mut app, vec![]);
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let full_bounds = |label: &str| {
                let (rect, clip) = output
                    .shapes
                    .iter()
                    .find_map(|shape| Some((glyph_bounds(&shape.shape, label)?, shape.clip_rect)))
                    .unwrap_or_else(|| panic!("{size:?} at {density}x misses {label:?}"));
                let visible = clip.intersect(viewport);
                assert!(
                    visible.expand(0.5).contains_rect(rect),
                    "{size:?} at {density}x clips {label:?} {rect:?} to {visible:?}"
                );
                rect
            };
            let panel = |id| {
                egui::containers::panel::PanelState::load(&ctx, egui::Id::new(id))
                    .unwrap()
                    .rect
            };
            let header = panel("header");
            let footer = panel("status");
            assert!(
                header.height() <= 80.0,
                "{size:?} at {density}x: {header:?}"
            );
            let editor = app.variax();
            let (body, inspector) = editor.workspace_cards.unwrap();
            let band = editor.workspace_tuning.unwrap();
            assert!(
                body.top() <= size.y * 0.4,
                "{size:?} at {density}x: {body:?}"
            );
            assert!(
                inspector.height() > 200.0,
                "{size:?} at {density}x: {inspector:?}"
            );
            assert!((band.height() - size.y * 0.225).abs() < 0.01);
            for &label in ACTIONS {
                assert!(header.contains_rect(full_bounds(label)), "{label:?}");
            }
            let route = full_bounds(ROUTE);
            let status = full_bounds("Opened the built-in example banks.");
            let build = full_bounds(&build);
            assert!(footer.contains_rect(route));
            assert!(footer.contains_rect(status));
            assert!(
                !route.intersects(status),
                "{size:?} at {density}x: route {route:?}, status {status:?}"
            );
            assert!(status.max.x < build.min.x && route.max.x < build.min.x);
            assert!(build.max.y >= size.y - 10.0);

            let tap = |app: &mut Workbench, key| {
                frame(app, vec![tuning_key(key, true, none)]);
                frame(app, vec![tuning_key(key, false, none)])
            };
            let focus = |app: &mut Workbench, target: egui::Rect, exact: bool| {
                for _ in 0..100 {
                    let focused = ctx
                        .memory(|memory| memory.focused())
                        .and_then(|id| ctx.read_response(id));
                    if focused.is_some_and(|response| {
                        (response.rect == target || !exact && response.rect.contains_rect(target))
                            && Some(response.layer_id) == ctx.layer_id_at(target.center())
                    }) {
                        return;
                    }
                    tap(app, egui::Key::Tab);
                }
                panic!("{size:?} at {density}x cannot Tab to {target:?}");
            };

            focus(&mut app, full_bounds("New Variax file"), false);
            tap(&mut app, egui::Key::Enter);
            assert!(text_rect(&frame(&mut app, vec![]), "New Model…").is_some());
            tap(&mut app, egui::Key::Escape);

            focus(&mut app, full_bounds("Guitar actions"), false);
            tap(&mut app, egui::Key::Enter);
            let guitar = frame(&mut app, vec![]);
            for label in [
                "Download from guitar…",
                "Send Edit Buffer",
                "Get Edit Buffer",
                "String Volume Setup…",
            ] {
                assert!(text_rect(&output, label).is_none());
                assert!(text_rect(&guitar, label).is_some(), "{label}");
            }
            tap(&mut app, egui::Key::Escape);

            focus(&mut app, full_bounds("Tools"), false);
            tap(&mut app, egui::Key::Enter);
            let tools = frame(&mut app, vec![]);
            for label in ["Device check…", "Test recording…"] {
                assert!(text_rect(&output, label).is_none());
                assert!(text_rect(&tools, label).is_some(), "{label}");
            }
            let device_check = app.device_check_buttons["open"];
            focus(&mut app, device_check, true);
            tap(&mut app, egui::Key::Enter);
            assert!(app.device_check_open);
        }
    }
}

#[test]
fn inspector_exposes_initial_and_final_controls_at_both_densities() {
    const PHASE: &str = "Reverse pickup 2 phase";
    const AUTO: &str = "Auto-select Tone/Volume defaults when selecting pickup types";

    for size in SIZES {
        for density in [1.0, 2.0] {
            let (ctx, mut app) = app();
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.clear_window_controls = true;
            let editor = app.variax_mut();
            editor.open_example();
            editor.document.as_mut().unwrap().select(0, 1).unwrap();
            #[cfg(feature = "usb")]
            {
                let index = app.tabs.index(app.tabs.active.unwrap()).unwrap();
                app.tabs.entries[index].device = Some(super::refresh::tests::verified_tab());
            }
            let frame = |app: &mut Workbench, events| {
                render(
                    &mut *app,
                    &ctx,
                    size,
                    density,
                    events,
                    egui::Modifiers::NONE,
                )
            };
            frame(&mut app, vec![]);
            let output = frame(&mut app, vec![]);
            let (body, inspector) = app.variax().workspace_cards.unwrap();
            let tuning = app.variax().workspace_tuning;
            let visible_bounds = |output: &egui::FullOutput, label: &str| {
                output.shapes.iter().find_map(|shape| {
                    let rect = text_bounds(&shape.shape, label)?;
                    inspector
                        .intersect(shape.clip_rect)
                        .expand(0.5)
                        .contains_rect(rect)
                        .then_some(rect)
                })
            };
            let bounds = |output: &egui::FullOutput, label: &str| {
                visible_bounds(output, label)
                    .unwrap_or_else(|| panic!("{size:?} at {density}x clips {label:?}"))
            };
            let click = |app: &mut Workbench, position| {
                frame(app, vec![egui::Event::PointerMoved(position)]);
                frame(app, vec![pointer(position, true)]);
                frame(app, vec![pointer(position, false)]);
                frame(app, vec![])
            };
            let scroll = |app: &mut Workbench, unit, delta| {
                frame(
                    app,
                    vec![
                        egui::Event::PointerMoved(egui::pos2(
                            inspector.center().x,
                            inspector.bottom() - 16.0,
                        )),
                        egui::Event::MouseWheel {
                            unit,
                            delta: egui::vec2(0.0, delta),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                frame(app, vec![])
            };

            // Reveal controls that start below the inspector's visible rows.
            let reveal = |app: &mut Workbench, mut output: egui::FullOutput, label: &str| {
                for _ in 0..40 {
                    if visible_bounds(&output, label).is_some() {
                        return output;
                    }
                    output = scroll(app, egui::MouseWheelUnit::Point, -4.0);
                }
                bounds(&output, label);
                output
            };

            let output = click(&mut app, bounds(&output, "Pickups").center());
            bounds(&output, "Pickup 2");
            bounds(&output, "Enabled");
            if size.x == 1120.0 {
                reveal(&mut app, output, "Position");
            }
            let phase_before = app.variax().integer(super::PICKUP2, 0x200002).unwrap();
            let output = scroll(&mut app, egui::MouseWheelUnit::Page, -2.0);
            click(&mut app, bounds(&output, PHASE).center());
            let phase_after = app.variax().integer(super::PICKUP2, 0x200002).unwrap();
            assert_ne!(phase_before, phase_after);
            for (shift, expected) in [(false, phase_before), (true, phase_after)] {
                let modifiers = egui::Modifiers {
                    ctrl: true,
                    command: true,
                    shift,
                    ..Default::default()
                };
                render(
                    &mut app,
                    &ctx,
                    size,
                    density,
                    vec![egui::Event::Key {
                        key: egui::Key::Z,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    modifiers,
                );
                assert_eq!(
                    app.variax().integer(super::PICKUP2, 0x200002),
                    Some(expected)
                );
                frame(&mut app, vec![]);
            }

            let output = scroll(&mut app, egui::MouseWheelUnit::Page, 2.0);
            bounds(&output, "Pickup 1");
            let output = click(&mut app, bounds(&output, "Controls").center());
            bounds(&output, "Volume knob");
            if size.x == 1120.0 {
                let output = reveal(&mut app, output, "500k Ω");
                reveal(&mut app, output, "Audio");
            }
            let auto_before = app.variax().auto_select_controls;
            let output = scroll(&mut app, egui::MouseWheelUnit::Page, -2.0);
            click(&mut app, bounds(&output, AUTO).center());
            assert_ne!(app.variax().auto_select_controls, auto_before);
            let output = scroll(&mut app, egui::MouseWheelUnit::Page, 2.0);
            bounds(&output, "Volume knob");
            assert_eq!(app.variax().workspace_cards, Some((body, inspector)));
            assert_eq!(app.variax().workspace_tuning, tuning);
        }
    }
}

#[test]
fn populated_bank_slots_show_names_and_keep_their_controls_at_both_densities() {
    let artwork = tempfile::tempdir().unwrap();
    let directory = artwork.path().join("bodies/buttons");
    std::fs::create_dir_all(&directory).unwrap();
    image::RgbaImage::from_pixel(27, 32, image::Rgba([200, 220, 255, 255]))
        .save(directory.join("strat.png"))
        .unwrap();
    let long_name = "Écho WWWWWWWWWWWWWWWWWWWW";
    let names = [
        "T-Model example",
        long_name,
        "Lester example",
        "R-Billy example",
        "Twelve-string example",
    ];
    for size in SIZES {
        for pixels_per_point in [1.0, 2.0] {
            let (ctx, mut app) = app();
            app.clear_window_controls = true;
            let editor = app.variax_mut();
            editor.open_example();
            editor.choose_artwork(artwork.path().into());
            let document = editor.document.as_mut().unwrap();
            document.select(0, 1).unwrap();
            document
                .apply(|preset| preset.set_stored_name(long_name))
                .unwrap();
            document.select(0, 0).unwrap();
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
            assert_eq!(output.pixels_per_point, pixels_per_point);
            let editor = app.variax();
            assert_eq!(editor.bank_slots.len(), names.len());
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let mut previous = editor.bank_dropdown.unwrap();
            for (index, name) in names.into_iter().enumerate() {
                let slot = editor.bank_slots[&index];
                assert!(screen.contains_rect(slot), "{size:?}: {slot:?}");
                assert!(
                    previous.right() <= slot.left() && slot.y_range().contains(previous.center().y),
                    "{size:?} at {pixels_per_point}: slot {} {slot:?} after {previous:?}",
                    index + 1
                );
                previous = slot;
                let number = (index + 1).to_string();
                let label = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.text() == name => {
                            Some((text, shape.clip_rect))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("{size:?} at {pixels_per_point}: missing {name:?}"));
                let bounds = label.0.galley.rect.translate(label.0.pos.to_vec2());
                assert!(
                    slot.intersect(label.1).contains_rect(bounds),
                    "{name:?}: {bounds:?} in {slot:?}"
                );
                assert!(bounds.is_positive());
                assert_eq!(label.0.galley.elided, name == long_name, "{name:?}");
                for marker in [Some(number.as_str()), (name == long_name).then_some("!")]
                    .into_iter()
                    .flatten()
                {
                    assert!(
                        output.shapes.iter().any(|shape| {
                            text_bounds(&shape.shape, marker).is_some_and(|marker| {
                                slot.intersect(shape.clip_rect).contains_rect(marker)
                                    && marker.right() < bounds.left()
                            })
                        }),
                        "missing {marker:?} in slot {}",
                        index + 1
                    );
                }
            }
            let unsaved = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.text() == "Unsaved" => {
                        let bounds = text.visual_bounding_rect();
                        (bounds.top() >= previous.top()).then_some((bounds, shape.clip_rect))
                    }
                    _ => None,
                })
                .expect("bank header Unsaved indicator");
            assert!(
                previous.right() <= unsaved.0.left()
                    && previous.y_range().contains(unsaved.0.center().y)
                    && screen.intersect(unsaved.1).contains_rect(unsaved.0),
                "{size:?} at {pixels_per_point}: Unsaved {unsaved:?} should fit after the last slot {previous:?}"
            );
            let target = editor.bank_slots[&1];
            let texture = editor.textures["bodies/buttons/strat.png"].handle.id();
            assert!(output.shapes.iter().any(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                    target
                        .intersect(shape.clip_rect)
                        .contains_rect(mesh.calc_bounds())
                }
                _ => false,
            }));

            let name_position = egui::pos2(target.right() - 15.0, target.center().y);
            frame(
                &mut app,
                vec![
                    egui::Event::PointerMoved(name_position),
                    pointer(name_position, true),
                ],
            );
            let output = frame(&mut app, vec![pointer(name_position, false)]);
            assert_eq!(
                app.variax().document.as_ref().unwrap().selection(),
                Some((0, 1))
            );
            let announcement = format!(
                "Slot 2 · {long_name} · differs from the last saved file · modified since the library was opened"
            );
            assert!(
                output.platform_output.events.iter().any(|event| {
                    matches!(event, egui::output::OutputEvent::Clicked(info)
                    if info.selected == Some(false) && info.label.as_deref() == Some(&announcement))
                }),
                "slot activation must announce the full name and dirty state"
            );
            let output = frame(&mut app, vec![]);
            assert!(
                output.shapes.iter().any(|shape| match &shape.shape {
                    egui::Shape::Rect(rect) if rect.rect == target => {
                        rect.fill == egui::Color32::from_rgb(60, 39, 18)
                    }
                    _ => false,
                }),
                "the clicked slot must be highlighted"
            );
            let editor = app.variax();
            println!(
                "{size:?} at {pixels_per_point} px/point: dropdown {:?}, slots 1–5 {:?}",
                editor.bank_dropdown.unwrap(),
                editor.bank_slots[&0].union(editor.bank_slots[&4])
            );
        }
    }
}

#[test]
fn bank_selection_and_file_actions_share_a_compact_header_at_supported_sizes() {
    use super::LibraryOrigin;
    use std::time::UNIX_EPOCH;

    let library = tempfile::tempdir().unwrap();
    let path = library.path().join("selection.vwb");
    let (_, mut seed) = app();
    seed.variax_mut().open_example();
    seed.variax_mut()
        .document
        .as_mut()
        .unwrap()
        .save_copy(&path)
        .unwrap();
    for size in SIZES.into_iter().chain([egui::vec2(1440.0, 900.0)]) {
        for density in [1.0, 2.0] {
            let (ctx, mut app) = app();
            app.clear_window_controls = true;
            let editor = app.variax_mut();
            editor.load(&path);
            editor.document.as_mut().unwrap().select(0, 1).unwrap();
            editor.library_origin = Some(LibraryOrigin::Download(UNIX_EPOCH));
            editor.change(|preset| preset.rename("Edited Spank"));
            let origin = editor.library_label().unwrap();
            let frame = |app: &mut Workbench, events| {
                render(app, &ctx, size, density, events, egui::Modifiers::NONE)
            };
            frame(&mut app, vec![]);
            let output = frame(&mut app, vec![]);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let visible = |label: &str| {
                let (bounds, clip) = output
                    .shapes
                    .iter()
                    .find_map(|shape| Some((text_bounds(&shape.shape, label)?, shape.clip_rect)))
                    .unwrap_or_else(|| panic!("{size:?} at {density}x: missing {label:?}"));
                assert!(
                    screen.intersect(clip).contains_rect(bounds),
                    "{label}: {bounds:?}"
                );
                bounds
            };
            let model_actions = visible("Model actions");
            let library_actions = visible("Library actions");
            assert!(model_actions.y_range().contains(library_actions.center().y));
            let editor = app.variax();
            let slot_top = editor.bank_slots[&0].top();
            assert!(model_actions.bottom() - slot_top <= 100.0);
            assert!(
                visible("Modified · not stored on guitar").bottom()
                    <= editor.bank_slots[&4].bottom()
            );
            assert!(visible("Unsaved").bottom() <= editor.bank_slots[&4].bottom());
            for label in ["Model", "Author", "Notes", "Undo", "Redo", "Output trim"] {
                let bounds = visible(label);
                assert!(bounds.top() > slot_top && bounds.bottom() < model_actions.top());
            }

            let second_slot = editor.bank_slots[&1];
            let model_menu = editor.slot_file_buttons["slot-transfers"];
            let library_menu = editor.slot_file_buttons["library-actions"];
            let tap = |app: &mut Workbench, key| {
                frame(app, vec![tuning_key(key, true, egui::Modifiers::NONE)]);
                frame(app, vec![tuning_key(key, false, egui::Modifiers::NONE)])
            };
            let tab_to = |app: &mut Workbench, target| {
                for _ in 0..100 {
                    tap(app, egui::Key::Tab);
                    if ctx
                        .memory(|memory| memory.focused())
                        .and_then(|id| ctx.read_response(id))
                        .is_some_and(|response| response.rect == target)
                    {
                        return;
                    }
                }
                panic!("{size:?} at {density}x: cannot tab to {target:?}");
            };
            // Change from the selected model and back through the named slot cards.
            let first_slot = editor.bank_slots[&0];
            tab_to(&mut app, first_slot);
            tap(&mut app, egui::Key::Enter);
            assert_eq!(
                app.variax().document.as_ref().unwrap().selection(),
                Some((0, 0))
            );
            tab_to(&mut app, second_slot);
            tap(&mut app, egui::Key::Enter);
            assert_eq!(
                app.variax().document.as_ref().unwrap().selection(),
                Some((0, 1))
            );
            tab_to(&mut app, model_menu);
            tap(&mut app, egui::Key::Enter);
            let opened = frame(&mut app, vec![]);
            for label in [
                "Copy model",
                "Save model as…",
                "Replace slot with model file…",
            ] {
                assert!(text_rect(&opened, label).is_some(), "{label}");
            }
            tap(&mut app, egui::Key::Escape);
            assert!(text_rect(&frame(&mut app, vec![]), "Copy model").is_none());

            tab_to(&mut app, library_menu);
            tap(&mut app, egui::Key::Enter);
            let opened = frame(&mut app, vec![]);
            for label in [
                "Rename bank…",
                "Export HTML Model List…",
                "Copy bank",
                &origin,
            ] {
                assert!(text_rect(&opened, label).is_some(), "{label}");
            }
            tap(&mut app, egui::Key::Escape);
            assert!(text_rect(&frame(&mut app, vec![]), &origin).is_none());
        }
    }
}

#[test]
fn bank_storage_notice_distinguishes_local_and_detached_guitar_libraries() {
    use super::LibraryOrigin;
    use std::{fs, time::UNIX_EPOCH};

    const STORAGE_NOTICE: &str = "Modified · not stored on guitar";
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("local.vwb");
    let (_, mut seed) = app();
    seed.variax_mut().open_example();
    let original = seed.variax().document.as_ref().unwrap().as_bytes().to_vec();
    fs::write(&path, &original).unwrap();

    for size in SIZES {
        for density in [1.0, 2.0] {
            for guitar_library in [false, true] {
                let (ctx, mut app) = app();
                app.clear_window_controls = true;
                let editor = app.variax_mut();
                editor.load(&path);
                assert!(editor.library_origin.is_none());
                assert!(editor.document.as_ref().unwrap().selection().is_some());
                if guitar_library {
                    #[cfg(feature = "usb")]
                    {
                        let mut tab = super::refresh::tests::verified_tab();
                        editor.library_origin = Some(LibraryOrigin::AttachedDownload(
                            UNIX_EPOCH,
                            tab.device.key.clone(),
                        ));
                        tab.connection = crate::editor::tabs::Connection::Disconnected;
                        let index = app.tabs.index(app.tabs.active.unwrap()).unwrap();
                        app.tabs.entries[index].device = Some(tab);
                    }
                    #[cfg(not(feature = "usb"))]
                    {
                        editor.library_origin = Some(LibraryOrigin::Download(UNIX_EPOCH));
                    }
                }
                let frame = |app: &mut Workbench| {
                    render(app, &ctx, size, density, vec![], egui::Modifiers::NONE)
                };

                app.variax_mut()
                    .change(|preset| preset.rename("Local edit"));
                let editor = app.variax();
                let document = editor.document.as_ref().unwrap();
                let (bank, slot) = document.selection().unwrap();
                assert!(document.slot_is_unsent(bank, slot));

                frame(&mut app);
                let output = frame(&mut app);
                let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
                let notice = |output: &egui::FullOutput, label| {
                    output.shapes.iter().find_map(|shape| {
                        Some((text_bounds(&shape.shape, label)?, shape.clip_rect))
                    })
                };
                let (bounds, clip) = notice(&output, "Unsaved").expect("unsaved file");
                assert!(screen.intersect(clip).contains_rect(bounds));
                if guitar_library {
                    let (bounds, clip) =
                        notice(&output, STORAGE_NOTICE).expect("pending guitar storage");
                    assert!(screen.intersect(clip).contains_rect(bounds));
                    let last_slot = app.variax().bank_slots[&4];
                    assert!(last_slot.right() <= bounds.left());
                    assert!(last_slot.y_range().contains(bounds.center().y));
                    #[cfg(feature = "usb")]
                    assert!(text_rect(&output, "Guitar: Disconnected · offline editing").is_some());
                } else {
                    assert!(text_rect(&output, "Guitar offline").is_some());
                    assert!(
                        notice(&output, STORAGE_NOTICE).is_none(),
                        "{size:?} at {density}x: a local edit claimed pending guitar storage"
                    );
                }

                app.variax_mut().undo();
                let output = frame(&mut app);
                assert!(text_rect(&output, "Unsaved").is_none());
                assert!(notice(&output, STORAGE_NOTICE).is_none());
                assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
                assert_eq!(fs::read(&path).unwrap(), original);
            }
        }
    }
}

#[test]
fn imported_nonfinite_trim_fits_and_resets_only_on_request() {
    use std::fs;

    const TRIM: u32 = 0x200000;
    const RESET: &str = "Reset to minimum";
    // System model header through the float parameter's type.
    const TRIM_RECORD: &[u8] = b"\x02\x01\0\0\0\0\0\x01PARM\0\0\0\x0c\x3f\x20\0\0\0\0\0\x01";
    let seed = include_bytes!("../../../../tests/fixtures/synthetic-host-preset.vwm");
    let offset = seed
        .windows(TRIM_RECORD.len())
        .position(|bytes| bytes == TRIM_RECORD)
        .unwrap()
        + TRIM_RECORD.len();
    let temp = tempfile::tempdir().unwrap();

    for (bits, shown) in [
        (0x7fc0_0123_u32, "NaN"),
        (f32::INFINITY.to_bits(), "inf"),
        (f32::NEG_INFINITY.to_bits(), "-inf"),
    ] {
        let mut original = seed.to_vec();
        original[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
        let path = temp.path().join(format!("{bits:08x}.vwm"));
        fs::write(&path, &original).unwrap();
        let caption = format!("Output trim: {shown} (stored)");
        let mut reset = original.clone();
        reset[offset..offset + 4].copy_from_slice(&(-20.0_f32).to_be_bytes());

        for size in SIZES {
            for density in [1.0, 2.0] {
                let (ctx, mut app) = app();
                app.clear_window_controls = true;
                app.variax_mut().load(&path);
                let frame = |app: &mut Workbench, events| {
                    render(app, &ctx, size, density, events, egui::Modifiers::NONE)
                };
                let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
                let visible = |output: &egui::FullOutput, label: &str| {
                    let (bounds, clip) = output
                        .shapes
                        .iter()
                        .find_map(|shape| {
                            Some((text_bounds(&shape.shape, label)?, shape.clip_rect))
                        })
                        .unwrap_or_else(|| panic!("{size:?} at {density}x lacks {label:?}"));
                    assert!(
                        screen.intersect(clip).contains_rect(bounds),
                        "{size:?} at {density}x clips {label:?} {bounds:?} to {clip:?}"
                    );
                    bounds.center()
                };
                let click = |app: &mut Workbench, pos| {
                    frame(
                        app,
                        vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
                    );
                    frame(app, vec![pointer(pos, false)]);
                    frame(app, vec![])
                };
                let bytes =
                    |app: &Workbench| app.variax().document.as_ref().unwrap().as_bytes().to_vec();

                frame(&mut app, vec![]);
                let output = frame(&mut app, vec![]);
                assert_eq!(output.pixels_per_point, density);
                assert!(!app.variax().dirty());
                assert_eq!(app.variax().float(SYSTEM, TRIM).unwrap().to_bits(), bits);
                assert_eq!(bytes(&app), original);
                visible(&output, &caption);
                let output = click(&mut app, visible(&output, RESET));
                assert_eq!(bytes(&app), reset);
                assert!(app.variax().dirty());
                assert!(text_rect(&output, RESET).is_none());

                let output = click(&mut app, visible(&output, "Undo"));
                assert_eq!(bytes(&app), original);
                assert!(!app.variax().dirty());
                visible(&output, &caption);
                visible(&output, RESET);
                click(&mut app, visible(&output, "Redo"));
                assert_eq!(bytes(&app), reset);
                assert_eq!(fs::read(&path).unwrap(), original);
            }
        }
    }
}

#[test]
fn twelve_string_metadata_header_fits_and_accepts_input_at_both_densities() {
    let name = "Twelve-string example";
    let notice = format!("Model: {}", legacy_text_issue(name, true).unwrap());
    for size in SIZES {
        for pixels_per_point in [1.0, 2.0] {
            let (ctx, mut app) = app();
            app.clear_window_controls = true;
            let editor = app.variax_mut();
            editor.open_example();
            editor.document.as_mut().unwrap().select(0, 4).unwrap();
            assert!(editor.has_twelve_string());
            assert_eq!(editor.document.as_ref().unwrap().preset().name(), name);
            editor.change(|preset| preset.set_metadata(PresetText::Author, "Fixture author"));
            editor.change(|preset| preset.set_metadata(PresetText::Notes, "Fixture notes"));

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
            let click = |app: &mut Workbench, position| {
                frame(
                    app,
                    vec![egui::Event::PointerMoved(position), pointer(position, true)],
                );
                frame(app, vec![pointer(position, false)]);
                frame(app, vec![])
            };
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let visible = |output: &egui::FullOutput, text: &str| {
                let (bounds, clip) = output
                    .shapes
                    .iter()
                    .find_map(|shape| Some((text_bounds(&shape.shape, text)?, shape.clip_rect)))
                    .unwrap_or_else(|| panic!("{size:?} at {pixels_per_point}: missing {text:?}"));
                assert!(
                    screen.intersect(clip).contains_rect(bounds),
                    "{size:?} at {pixels_per_point}: {text:?} {bounds:?} clipped to {clip:?}",
                );
                bounds
            };

            frame(&mut app, vec![]);
            let output = frame(&mut app, vec![]);
            assert_eq!(output.pixels_per_point, pixels_per_point);
            for text in [
                &notice,
                "Output trim",
                "Model",
                "Author",
                "Notes",
                "Undo",
                "Redo",
            ] {
                visible(&output, text);
            }
            let trim_value = visible(&output, "0.00dB");
            // The slider track precedes the numeric field on the same line.
            let track = trim_value.center() - egui::vec2(44.0, 0.0);
            click(&mut app, track);
            let changed_trim = app.variax().float(SYSTEM, 0x200000).unwrap();
            assert_ne!(changed_trim, 0.0);
            let output = frame(&mut app, vec![]);
            let output = click(&mut app, visible(&output, "Undo").center());
            assert_eq!(app.variax().float(SYSTEM, 0x200000), Some(0.0));
            let output = click(&mut app, visible(&output, "Redo").center());
            assert_eq!(app.variax().float(SYSTEM, 0x200000), Some(changed_trim));
            let output = click(&mut app, visible(&output, "Undo").center());

            let enter = |app: &mut Workbench, text: &str| {
                frame(
                    app,
                    vec![
                        egui::Event::Text(text.into()),
                        egui::Event::Key {
                            key: egui::Key::Enter,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            };
            click(&mut app, visible(&output, "0.00dB").center());
            enter(&mut app, "+1.50dB");
            assert_eq!(app.variax().float(SYSTEM, 0x200000), Some(1.5));

            for (field, original, input) in [
                (PresetText::Author, "Fixture author", "A"),
                (PresetText::Notes, "Fixture notes", "N"),
            ] {
                let output = frame(&mut app, vec![]);
                click(&mut app, visible(&output, original).center());
                frame(&mut app, vec![egui::Event::Text(input.into())]);
                let preset = app.variax().document.as_ref().unwrap().preset();
                assert_ne!(preset.metadata(field).unwrap(), original);
            }

            let model = ctx
                .read_response(egui::Id::new("variax-model-name"))
                .unwrap();
            assert!(screen.contains_rect(model.rect));
            assert_eq!(model.interact_rect, model.rect);
            click(&mut app, model.rect.center());
            assert!(ctx.memory(|memory| memory.has_focus(model.id)));
        }
    }
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

fn assert_bottom_band(
    app: &Workbench,
    ctx: &egui::Context,
    output: &egui::FullOutput,
    size: egui::Vec2,
) {
    let editor = app.variax();
    let (body, inspector) = editor.workspace_cards.unwrap();
    let tuning = editor.workspace_tuning.unwrap();
    let margin = egui::Frame::central_panel(&ctx.style()).total_margin();
    let footer = egui::containers::panel::PanelState::load(ctx, egui::Id::new("status"))
        .unwrap()
        .rect;
    let screen = ctx.content_rect();
    assert_eq!(body.left(), tuning.left());
    assert_eq!(body.top(), inspector.top());
    assert!(body.right() <= inspector.left());
    assert!(body.bottom() <= inspector.bottom() + 0.01);
    assert_eq!(inspector.right(), tuning.right());
    assert!((tuning.top() - inspector.bottom() - COLUMN_GAP).abs() < 0.01);
    assert!((tuning.left() - screen.left() - margin.left).abs() < 0.5);
    assert!((screen.right() - tuning.right() - margin.right).abs() < 0.5);
    assert!((footer.top() - tuning.bottom() - margin.bottom).abs() < 0.5);
    let expected = (size.y * 0.225).min((tuning.bottom() - body.top() - COLUMN_GAP) / 2.0);
    assert!(
        (tuning.height() - expected).abs() < 0.01,
        "{size:?}: {tuning:?}"
    );
    assert_footer(output, size, tuning);
}

fn assert_neck_uses_remaining_width(tuning: egui::Rect, neck: egui::Rect, twelve_width: f32) {
    let expected_left = tuning.left() + twelve_width + TUNING_TABLE_WIDTH + TUNING_CHROME_WIDTH;
    assert!((neck.left() - expected_left).abs() < 12.0, "{neck:?}");
    // The right inset includes the panel margin and room for a scrollbar.
    assert!(
        (0.0..TUNING_CHROME_WIDTH).contains(&(tuning.right() - neck.right())),
        "neck {neck:?} in tuning band {tuning:?}"
    );
}

fn body_tab_app(artwork: &std::path::Path) -> (egui::Context, Workbench) {
    let ctx = egui::Context::default();
    let mut app = Workbench::with_context(&ctx, None, artwork.to_owned(), None);
    app.theme_polled = std::time::Instant::now();
    app.clear_window_controls = true;
    let editor = app.variax_mut();
    editor.open_example();
    editor.tab = Tab::Body;
    (ctx, app)
}

fn render_body_tab(app: &mut Workbench, ctx: &egui::Context) -> egui::FullOutput {
    render(
        app,
        ctx,
        OUTLINE_TEST_SIZE,
        1.0,
        vec![],
        egui::Modifiers::NONE,
    )
}

#[test]
fn editor_paints_bundled_body_pickups_and_outline_without_an_artwork_folder() {
    let install = tempfile::tempdir().unwrap();
    let artwork = install.path().join("absent-source/assets/ui");
    let (ctx, mut app) = body_tab_app(&artwork);

    for (body, pickup, expected) in [
        (
            0x0310_0000,
            0x0311_0001,
            [
                ("bodies/telecustom.png", [426, 412]),
                ("pickups/telebridge.png", [120, 120]),
                ("bodies/wireframe/tele.png", [238, 148]),
            ],
        ),
        (
            0x0310_0001,
            0x0311_0000,
            [
                ("bodies/tele.png", [426, 412]),
                ("pickups/teleneck.png", [120, 120]),
                ("bodies/wireframe/tele.png", [238, 148]),
            ],
        ),
        (
            0x0310_0004,
            0x0311_0009,
            [
                ("bodies/lester.png", [426, 412]),
                ("pickups/chromep90.png", [56, 130]),
                ("bodies/wireframe/lespaul.png", [238, 148]),
            ],
        ),
    ] {
        app.variax_mut().change(|preset| {
            preset.select_model(BODY, body)?;
            preset.select_model(PICKUP1, pickup)
        });
        render_body_tab(&mut app, &ctx);
        let output = render_body_tab(&mut app, &ctx);
        let editor = app.variax();
        let (photo, inspector) = editor.workspace_cards.unwrap();
        for (key, pixels) in expected {
            let texture = &editor.textures[key].handle;
            assert_eq!(texture.size(), pixels, "{key}");
            let mesh = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture.id() => {
                    Some(mesh.calc_bounds())
                }
                _ => None,
            });
            let bounds = mesh.expect(key);
            if key.starts_with("bodies/wireframe/") {
                assert!(inspector.contains_rect(bounds), "{key}: {bounds:?}");
                assert_eq!(bounds.size(), egui::vec2(238.0, 148.0));
            } else {
                assert!(photo.contains(bounds.center()), "{key}: {bounds:?}");
            }
        }
        assert!(text_rect(&output, "Guitar artwork unavailable").is_none());
        assert!(text_rect(&output, "Body outline unavailable").is_none());
    }
    assert!(!artwork.exists());
}

const LESTER_OUTLINE: &str = "bodies/wireframe/lespaul.png";

fn body_tab_with_unavailable_outline(artwork: &std::path::Path) -> (egui::Context, Workbench) {
    let (ctx, mut app) = body_tab_app(artwork);
    let editor = app.variax_mut();
    editor.change(|preset| preset.select_model(BODY, 0x0310_0004));
    // All normal outlines are bundled. Inject a cached miss to exercise the
    // unavailable-art UI without changing the selected model or the bundle.
    editor.missing_textures.insert(LESTER_OUTLINE.into());
    (ctx, app)
}

#[test]
fn body_tab_paints_the_selected_models_private_outline_at_native_size() {
    let artwork = tempfile::tempdir().unwrap();
    let directory = artwork.path().join("bodies/wireframe");
    std::fs::create_dir_all(&directory).unwrap();
    let (ctx, mut app) = body_tab_app(artwork.path());
    let outline_bounds = |output: &egui::FullOutput, texture| {
        output.shapes.iter().find_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                let bounds = mesh.calc_bounds();
                assert!(shape.clip_rect.contains_rect(bounds));
                Some(bounds)
            }
            _ => None,
        })
    };

    let mut previous_texture = None;
    for (model, file, width, height) in [
        (0x0310_0003, "strat.png", 117, 61),
        (0x0310_0002, "thinline.png", 138, 70),
    ] {
        image::RgbaImage::from_pixel(width, height, image::Rgba([200, 220, 255, 255]))
            .save(directory.join(file))
            .unwrap();
        app.variax_mut()
            .change(|preset| preset.select_model(BODY, model));
        render_body_tab(&mut app, &ctx);
        let output = render_body_tab(&mut app, &ctx);
        let editor = app.variax();
        let path = format!("bodies/wireframe/{file}");
        let texture = editor.textures[&path].handle.id();
        let bounds = outline_bounds(&output, texture).expect("selected outline");
        assert_eq!(bounds.size(), egui::vec2(width as f32, height as f32));
        let (photo, inspector) = editor.workspace_cards.unwrap();
        assert!(inspector.contains_rect(bounds));
        assert!(!photo.intersects(bounds));
        assert!(text_rect(&output, &path).is_none());
        if let Some(previous) = previous_texture {
            assert!(outline_bounds(&output, previous).is_none());
        }
        previous_texture = Some(texture);
    }

    app.variax_mut().tab = Tab::Pickups;
    let output = render_body_tab(&mut app, &ctx);
    assert!(outline_bounds(&output, previous_texture.unwrap()).is_none());
}

#[test]
fn body_tab_keeps_the_outline_slot_when_art_is_unavailable() {
    const NOTICE: &str = "Body outline unavailable";
    let artwork = tempfile::tempdir().unwrap();
    let (ctx, mut app) = body_tab_with_unavailable_outline(artwork.path());
    render_body_tab(&mut app, &ctx);
    let output = render_body_tab(&mut app, &ctx);
    let label = text_rect(&output, NOTICE).expect("placeholder before scrolling");
    assert!(text_rect(&output, LESTER_OUTLINE).is_none());
    let (photo, inspector) = app.variax().workspace_cards.unwrap();
    render(
        &mut app,
        &ctx,
        OUTLINE_TEST_SIZE,
        1.0,
        vec![
            egui::Event::PointerMoved(label.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -400.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        egui::Modifiers::NONE,
    );
    for _ in 0..20 {
        render_body_tab(&mut app, &ctx);
    }
    let output = render_body_tab(&mut app, &ctx);
    let label = text_rect(&output, NOTICE).expect("placeholder after scrolling");
    let slot = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.rect.size() == egui::vec2(238.0, 148.0)
                    && rect.rect.contains_rect(label) =>
            {
                assert!(
                    shape.clip_rect.contains_rect(rect.rect),
                    "outline {:?} vs clip {:?}",
                    rect.rect,
                    shape.clip_rect
                );
                Some(rect.rect)
            }
            _ => None,
        })
        .expect("original missing-outline slot");
    assert!(inspector.contains_rect(slot));
    assert!(!photo.intersects(slot));

    app.variax_mut().tab = Tab::Controls;
    assert!(text_rect(&render_body_tab(&mut app, &ctx), NOTICE).is_none());
}

#[test]
fn body_outline_notice_and_loaded_art_fit_the_inspector_at_both_densities() {
    const NOTICE: &str = "Body outline unavailable";
    let missing = tempfile::tempdir().unwrap();
    let loaded = tempfile::tempdir().unwrap();
    let fixture = loaded.path().join(LESTER_OUTLINE);
    std::fs::create_dir_all(fixture.parent().unwrap()).unwrap();
    image::RgbaImage::from_pixel(117, 61, image::Rgba([200, 220, 255, 255]))
        .save(fixture)
        .unwrap();

    for size in SIZES {
        for density in [1.0, 2.0] {
            let (ctx, mut app) = body_tab_with_unavailable_outline(missing.path());
            let frame = |app: &mut Workbench, events| {
                render(app, &ctx, size, density, events, egui::Modifiers::NONE)
            };
            frame(&mut app, vec![]);
            let (photo, inspector) = app.variax().workspace_cards.unwrap();
            let scroll = |app: &mut Workbench| {
                frame(
                    app,
                    vec![
                        egui::Event::PointerMoved(egui::pos2(
                            inspector.center().x,
                            inspector.bottom() - 16.0,
                        )),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, -4.0),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                frame(app, vec![])
            };
            let label_fits = |output: &egui::FullOutput| {
                assert!(
                    output
                        .shapes
                        .iter()
                        .all(|shape| text_bounds(&shape.shape, LESTER_OUTLINE).is_none())
                );
                output.shapes.iter().any(|shape| {
                    text_bounds(&shape.shape, NOTICE).is_some_and(|bounds| {
                        inspector.intersect(shape.clip_rect).contains_rect(bounds)
                            && !photo.intersects(bounds)
                    })
                })
            };
            let mut output = frame(&mut app, vec![]);
            for _ in 0..40 {
                if label_fits(&output) {
                    break;
                }
                output = scroll(&mut app);
            }
            assert!(
                label_fits(&output),
                "{size:?} at {density}x clips {NOTICE:?}"
            );
            app.variax_mut().choose_artwork(loaded.path().into());
            assert!(!app.variax().missing_textures.contains(LESTER_OUTLINE));
            frame(&mut app, vec![]);
            let output = frame(&mut app, vec![]);
            assert_eq!(app.variax().workspace_cards, Some((photo, inspector)));
            assert!(text_rect(&output, NOTICE).is_none());
            let texture = app.variax().textures[LESTER_OUTLINE].handle.id();
            let outline = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) if mesh.texture_id == texture => {
                    let bounds = mesh.calc_bounds();
                    let visible = inspector.intersect(shape.clip_rect).intersect(bounds);
                    assert!(
                        visible.is_positive() && visible.x_range() == bounds.x_range(),
                        "{size:?} at {density}x shows {visible:?} of {bounds:?}"
                    );
                    Some(bounds)
                }
                _ => None,
            });
            let outline = outline.expect("loaded wireframe mesh");
            assert_eq!(outline.size(), egui::vec2(117.0, 61.0));
            assert!(!photo.intersects(outline));
        }
    }
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
    for size in bottom_band_sizes() {
        for (pixels_per_point, connected) in density_and_route() {
            let (ctx, mut app) = app();
            app.clear_window_controls = true;
            set_verified_route(&mut app, connected);
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
            assert_bottom_band(&app, &ctx, &output, size);

            let editor = app.variax();
            assert_eq!(editor.connection_status.is_some(), connected);
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
            assert!((art.height() - inspector.height()).abs() < 1.0);
            let neck = editor.tuning_widgets["Neck"];
            assert_neck_uses_remaining_width(tuning, neck, 0.0);
            assert!(text_rect(&output, "No 12-String available").is_none());
            assert!(!editor.tuning_widgets.contains_key("Low E mix"));
            assert!(!editor.tuning_widgets.contains_key("Low E detune"));
            assert!(text_rect(&output, "Pickups").is_some());

            let pos = editor.float(PICKUP1, 0x200000).unwrap();
            let center = placement.screen_point(art, pos);
            assert!(editor.pickup_centers[&PICKUP1].distance(center) < 0.1);
            let before = editor.document.as_ref().unwrap().as_bytes().to_vec();
            let [width, height] = editor.textures["pickups/strat.png"].handle.size();
            let padded_width = (width.max(height) as f64 * 1.15) as usize;
            let component_width = placement.x_max - placement.x_min + PICKUP_SLOT_PX as f32;
            let divisor = component_width - padded_width as f32;
            let span = POSITION_MAX - POSITION_MIN;
            let drag_art_pixels = 36.0;
            let normalized = (pos - POSITION_MIN) / span + drag_art_pixels / divisor;
            let expected = normalized.clamp(0.0, 1.0) * span + POSITION_MIN;
            let end = center + egui::vec2(drag_art_pixels * scale, 0.0);
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
            assert!((actual - expected).abs() < 1e-4, "{actual} vs {expected}");
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
    for size in bottom_band_sizes() {
        for (pixels_per_point, connected) in density_and_route() {
            let (ctx, mut app) = chime_editor(connected);
            app.variax_mut().set_tuning([2, 0, 0, 0, 0, 0], true);
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
            assert_eq!(output.pixels_per_point, pixels_per_point);
            assert_bottom_band(&app, &ctx, &output, size);
            let editor = app.variax();
            assert_eq!(editor.connection_status.is_some(), connected);
            let tuning = editor.workspace_tuning.unwrap();
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            assert!(screen.contains_rect(tuning), "{size:?}: {tuning:?}");
            let placement = editor.body_placement();
            let art = editor.body_art_rect.unwrap();
            let scale = placement.screen_scale(art);
            assert!((art.size() - placement.art * scale).length() < 1.0);
            let neck = editor.tuning_widgets["Neck"];
            assert_neck_uses_remaining_width(tuning, neck, TWELVE_STRING_COLUMN);

            let regions = app.variax().workspace_cards;
            let scroll = |app: &mut Workbench, delta| {
                frame(
                    app,
                    vec![
                        egui::Event::PointerMoved(tuning.center()),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: egui::vec2(0.0, delta),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
                for _ in 0..25 {
                    frame(app, vec![]);
                }
                frame(app, vec![egui::Event::PointerGone])
            };

            let headings = if text_rect(&output, "Mix").is_some() {
                output
            } else {
                scroll(&mut app, -36.0)
            };
            for label in ["Mix", "Detune"] {
                assert!(
                    text_rect(&headings, label).is_some_and(|rect| tuning.contains_rect(rect)),
                    "{size:?} at {pixels_per_point}x, connected={connected}: {label} is clipped"
                );
            }

            let low_mix = app.variax().tuning_widgets["Low E mix"];
            scroll(&mut app, tuning.center().y - low_mix.center().y);
            for label in ["Low E shift", "Low E mix", "Low E detune"] {
                assert!(
                    tuning.contains_rect(app.variax().tuning_widgets[label]),
                    "{size:?} at {pixels_per_point}x: {label} outside {tuning:?}"
                );
            }

            let original = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
            let widgets = &app.variax().tuning_widgets;
            let semitone = egui::pos2(
                scale_x(widgets["Neck"], 7),
                widgets["Low E shift"].center().y,
            );
            assert!(tuning.contains(semitone));
            frame(&mut app, vec![egui::Event::PointerMoved(semitone)]);
            frame(&mut app, vec![pointer(semitone, true)]);
            frame(&mut app, vec![pointer(semitone, false)]);
            assert_eq!(
                app.variax().integer(TUNING, super::tuning_presets::PITCH),
                Some(7)
            );
            app.variax_mut().undo();
            assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);

            let output = scroll(&mut app, -700.0);
            assert_eq!(app.variax().workspace_cards, regions);
            assert_eq!(app.variax().workspace_tuning, Some(tuning));
            assert_footer(&output, size, tuning);
            for label in ["Lock", "Reset", "Zero detune", "−12", "+12"] {
                let (bounds, clip) = output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        text_bounds(&shape.shape, label).map(|bounds| (bounds, shape.clip_rect))
                    })
                    .unwrap_or_else(|| panic!("{size:?} at {pixels_per_point}x: missing {label}"));
                assert!(
                    tuning.intersect(clip).contains_rect(bounds),
                    "{size:?} at {pixels_per_point}x: {label} clipped to {clip:?}"
                );
            }

            for action in ["Lock", "Reset", "Zero detune"] {
                let center = app.variax().tuning_widgets[action].center();
                assert!(tuning.contains(center));
                frame(&mut app, vec![egui::Event::PointerMoved(center)]);
                frame(&mut app, vec![pointer(center, true)]);
                frame(&mut app, vec![pointer(center, false)]);
                match action {
                    "Lock" => {
                        assert!(app.variax().link_tuning_mix);
                        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
                    }
                    "Reset" => {
                        for id in super::tuning_presets::PITCH..super::tuning_presets::PITCH + 6 {
                            assert_eq!(app.variax().integer(TUNING, id), Some(0));
                        }
                        assert_eq!(app.variax().float(TUNING, DETUNE), Some(12.0));
                    }
                    "Zero detune" => assert_eq!(app.variax().float(TUNING, DETUNE), Some(0.0)),
                    _ => unreachable!(),
                }
            }
            let command = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            for _ in 0..2 {
                render(
                    &mut app,
                    &ctx,
                    size,
                    pixels_per_point,
                    vec![tuning_key(egui::Key::Z, true, command)],
                    command,
                );
                assert_eq!(app.variax().float(TUNING, DETUNE), Some(12.0));
                frame(&mut app, vec![tuning_key(egui::Key::Z, false, command)]);
            }
            assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), original);
        }
    }
}

fn chime_editor(connected: bool) -> (egui::Context, Workbench) {
    let (ctx, mut app) = app();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.clear_window_controls = true;
    let editor = app.variax_mut();
    editor.open_example();
    editor.document.as_mut().unwrap().select(0, 4).unwrap();
    editor.change(|preset| preset.set_parameter(TUNING, DETUNE, 1, 12.0_f32.to_bits()));
    set_verified_route(&mut app, connected);
    (ctx, app)
}

fn tuning_key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

#[test]
fn tab_reveals_family_owned_twelve_string_controls() {
    let none = egui::Modifiers::NONE;
    for size in bottom_band_sizes() {
        for (density, connected) in density_and_route() {
            let (ctx, mut app) = chime_editor(connected);
            let frame = |app: &mut Workbench, events, modifiers| {
                render(app, &ctx, size, density, events, modifiers)
            };
            for _ in 0..4 {
                frame(&mut app, vec![], none);
            }
            let tuning = app.variax().workspace_tuning.unwrap();
            let focus = |app: &Workbench, label| {
                let id = ctx.memory(|memory| memory.focused())?;
                let response = ctx.read_response(id)?;
                (response.rect == app.variax().tuning_widgets[label]).then_some(response)
            };
            let tap = |app: &mut Workbench, key, modifiers| {
                for pressed in [true, false] {
                    frame(app, vec![tuning_key(key, pressed, modifiers)], modifiers);
                }
                for _ in 0..4 {
                    frame(app, vec![], none);
                }
            };
            for label in ["Low E mix", "Low E detune", "Zero detune"] {
                for _ in 0..120 {
                    tap(&mut app, egui::Key::Tab, none);
                    if focus(&app, label).is_some() {
                        break;
                    }
                }
                let response = focus(&app, label)
                    .unwrap_or_else(|| panic!("{size:?} at {density}x: Tab missed {label}"));
                assert!(
                    tuning.contains_rect(response.rect) && response.interact_rect == response.rect,
                    "{size:?} at {density}x: Tab focused clipped {label} at {:?}",
                    response.rect
                );
            }
            tap(&mut app, egui::Key::Enter, none);
            assert_eq!(app.variax().float(TUNING, DETUNE), Some(0.0));

            let shift = egui::Modifiers::SHIFT;
            tap(&mut app, egui::Key::Tab, shift);
            let response = focus(&app, "Low E detune").expect("Shift+Tab returns to Low E detune");
            assert!(tuning.contains_rect(response.rect));
        }
    }
}

#[test]
fn factory_selector_stays_readable_and_preserves_the_current_library_on_laptops() {
    use super::startup_library::{LibraryOrigin, record_unchecked_archive};
    use std::{fs, time::UNIX_EPOCH};
    use variax_workbench::bundle::BundleFile;

    let temp = tempfile::tempdir().unwrap();
    let preferences = temp.path().join("editor-preferences");
    let archive = temp.path().join("factory.vxf");
    let archived_fixture = b"synthetic factory source";
    fs::write(&archive, archived_fixture).unwrap();
    record_unchecked_archive(&preferences, &archive);
    let factory = BundleFile::new_default_bundle().unwrap();
    let path = temp.path().join("user.vwb");
    fs::write(&path, factory.as_bytes()).unwrap();
    let heading = "Factory selector reference · T-Model";
    let order = "Variax 3.10 factory settings · bridge to neck";

    for size in SIZES {
        for density in [1.0, 2.0] {
            let (ctx, mut app) = app();
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.clear_window_controls = true;
            let editor = app.variax_mut();
            editor.preferences_path = Some(preferences.clone());
            editor.adopt_document(super::EditorDocument::open(&path).unwrap(), Some(&path));
            editor.library_origin = Some(LibraryOrigin::Download(UNIX_EPOCH));
            let document = editor.document.as_mut().unwrap();
            document.select(1, 2).unwrap();
            document.rename_selected_bank("My Bank").unwrap();
            document.apply(|preset| preset.rename("My Preset")).unwrap();
            document.apply(|preset| preset.rename("Redo this")).unwrap();
            document.undo();
            assert!(document.can_undo() && document.can_redo());
            let bytes = document.as_bytes().to_vec();
            let library_label = editor.library_label();
            #[cfg(feature = "usb")]
            let observer_state = (
                editor.follow.active(),
                editor.follow.tuning_label(),
                editor.live_edit.enabled(),
            );

            let frame = |app: &mut Workbench, events| {
                render(app, &ctx, size, density, events, egui::Modifiers::NONE)
            };
            frame(&mut app, vec![]);
            let collapsed = frame(&mut app, vec![]);
            let editor = app.variax();
            let connection_label = editor.connection_status.clone();
            let closed_cards = editor.workspace_cards.unwrap();
            let closed_tuning = editor.workspace_tuning.unwrap();
            let trigger_text = text_rect(&collapsed, heading).expect("closed selector trigger");
            assert!(editor.bank_slots[&4].bottom() <= trigger_text.top());
            let toggle = trigger_text.center();
            assert!(text_rect(&collapsed, order).is_none());
            assert!(text_rect(&collapsed, "Editing bank: My Bank").is_some());
            assert!(text_rect(&collapsed, "My Preset").is_some());

            frame(
                &mut app,
                vec![egui::Event::PointerMoved(toggle), pointer(toggle, true)],
            );
            app.variax_mut().seed_factory_selector(&archive, &factory);
            frame(&mut app, vec![pointer(toggle, false)]);
            let output = frame(&mut app, vec![]);
            assert_eq!(output.pixels_per_point, density);
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
            let trigger_id = ctx
                .memory(|memory| memory.focused())
                .expect("trigger focus");
            let trigger = ctx.read_response(trigger_id).unwrap().rect;
            assert!(trigger.contains(toggle));
            let mut preceding = text_rect(&output, order).expect("selector order caption");
            assert!(preceding.top() >= trigger.bottom());
            // The popup can slide left when its content would cross the window edge.
            assert!(preceding.left() <= trigger.left() + 12.0);
            assert!(screen.contains_rect(preceding));
            for position in 1..=5 {
                let label = format!("{position} · Spank · Spank Single Coil Bridge");
                let row = output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        let bounds = text_bounds(&shape.shape, &label)?;
                        assert!(
                            screen.intersect(shape.clip_rect).contains_rect(bounds),
                            "{size:?} at {density}: clipped row {label}"
                        );
                        Some(bounds)
                    })
                    .unwrap_or_else(|| panic!("{size:?} at {density}: no row {label}"));
                assert!(
                    row.top() >= preceding.bottom(),
                    "{row:?} after {preceding:?}"
                );
                preceding = row;
            }
            assert!(text_rect(&output, "Editing bank: My Bank").is_some());
            assert!(text_rect(&output, "My Preset").is_some());
            assert_eq!(app.variax().workspace_cards, Some(closed_cards));
            assert_eq!(app.variax().workspace_tuning, Some(closed_tuning));
            assert!(text_rect(&output, "6 · Spank · Spank Single Coil Bridge").is_none());

            let key = |key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            frame(&mut app, vec![key(egui::Key::Escape)]);
            let closed = frame(&mut app, vec![]);
            assert!(text_rect(&closed, order).is_none());
            assert!(ctx.memory(|memory| memory.has_focus(trigger_id)));

            // Reopening requires verification even when the path and file stamp stayed the same.
            frame(&mut app, vec![key(egui::Key::Enter)]);
            let unverified = frame(&mut app, vec![]);
            assert!(text_rect(&unverified, order).is_none());
            assert!(
                text_rect(
                    &unverified,
                    "Choose an available, verified Variax 3.10 firmware file in Preferences."
                )
                .is_some()
            );
            app.variax_mut().seed_factory_selector(&archive, &factory);
            let reopened = frame(&mut app, vec![]);
            assert!(text_rect(&reopened, order).is_some());

            let row = preceding.center();
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(row), pointer(row, true)],
            );
            frame(&mut app, vec![pointer(row, false)]);
            assert!(text_rect(&frame(&mut app, vec![]), order).is_some());

            let outside = egui::pos2(size.x - 12.0, trigger.center().y);
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(outside), pointer(outside, true)],
            );
            frame(&mut app, vec![pointer(outside, false)]);
            assert!(text_rect(&frame(&mut app, vec![]), order).is_none());
            assert_eq!(app.variax().workspace_cards, Some(closed_cards));
            assert_eq!(app.variax().workspace_tuning, Some(closed_tuning));
            let editor = app.variax_mut();
            assert_eq!(editor.library_label(), library_label);
            assert_eq!(editor.source_path.as_deref(), Some(path.as_path()));
            assert_eq!(editor.connection_status, connection_label);
            #[cfg(feature = "usb")]
            assert_eq!(
                (
                    editor.follow.active(),
                    editor.follow.tuning_label(),
                    editor.live_edit.enabled(),
                ),
                observer_state
            );
            let document = editor.document.as_mut().unwrap();
            assert_eq!(document.source, path);
            assert_eq!(document.selection(), Some((1, 2)));
            assert_eq!(document.selected_bank().unwrap().1.name, "My Bank");
            assert_eq!(document.preset().name(), "My Preset");
            assert_eq!(document.as_bytes(), bytes);
            assert!(document.is_dirty());
            document.undo();
            assert_eq!(document.preset().name(), "Default Patch");
            document.redo();
            assert_eq!(document.as_bytes(), bytes);
            document.redo();
            assert_eq!(document.preset().name(), "Redo this");
            assert_eq!(fs::read(&path).unwrap(), factory.as_bytes());
            assert_eq!(fs::read(&archive).unwrap(), archived_fixture);
        }
    }
}
