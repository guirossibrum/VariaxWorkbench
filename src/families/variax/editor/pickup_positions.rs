use super::{BODY, EditorServices, PICKUP2, Path, VariaxEditor, egui, tabs};
use variax_workbench::{
    body::{BODY_DEFINITION_BYTES, BodyDefinition},
    edit::model_allowed,
    host::ModelLibrary,
};

fn resource(body: u32) -> Option<&'static str> {
    // Workbench 1.75 coefeng/data/definitions.xml, in asset-ID order.
    const FILES: [&str; 28] = [
        "tele_custom.bin",
        "tele.bin",
        "tele_thinline.bin",
        "strat.bin",
        "lespaul.bin",
        "lespaul_goldtop.bin",
        "lespaul_custom.bin",
        "lespaul_junior.bin",
        "lespaul_special.bin",
        "firebird.bin",
        "gretsch_6120.bin",
        "gretsch_duojet.bin",
        "rick_6.bin",
        "rick_12.bin",
        "es335.bin",
        "casino.bin",
        "es175.bin",
        "super400.bin",
        "martin_d28.bin",
        "martin_d12_28.bin",
        "martin_018.bin",
        "guild_f212.bin",
        "gibson_j200.bin",
        "dobro.bin",
        "sitar.bin",
        "danelectro.bin",
        "banjo.bin",
        "tricone.bin",
    ];
    FILES.get(body.checked_sub(0x0310_0000)? as usize).copied()
}

#[derive(Clone, Copy)]
struct Position {
    label: &'static str,
    inches: f32,
    degrees: f32,
}

fn override_definition(folder: &Path, body: u32) -> Result<BodyDefinition, String> {
    let resource = resource(body).ok_or("This body has no standard pickup positions.")?;
    let path = folder.join(resource);
    let bytes = tabs::open_candidate(&path, BODY_DEFINITION_BYTES as u64)?;
    BodyDefinition::parse(&bytes)
}

fn positions(body: &BodyDefinition) -> Result<Vec<Position>, String> {
    // The original JNI adapter sets Bridge/Neck true for a loaded body and
    // uses the resource's presence flag for Middle. The UI multiplies by 45.
    let mut positions = Vec::new();
    for (index, (label, defaults)) in ["Bridge", "Middle", "Neck"]
        .into_iter()
        .zip(&body.pickup_defaults)
        .enumerate()
    {
        if index == 1 && !body.has_middle {
            continue;
        }
        let degrees = defaults.angle * 45.0;
        if !defaults.position.is_finite() || !degrees.is_finite() {
            return Err("Standard pickup positions contain a nonfinite value.".into());
        }
        positions.push(Position {
            label,
            inches: defaults.position,
            degrees,
        });
    }
    Ok(positions)
}

pub(super) struct PickupPositionDialog {
    slot: u32,
    body: u32,
    positions: Vec<Position>,
    selected: usize,
}

impl VariaxEditor {
    pub(super) fn standard_pickup_positions(&mut self, ui: &mut egui::Ui, slot: u32) {
        let available = self.can_edit(slot, Some(0x200000))
            && self.can_edit(slot, Some(0x200001))
            && self.float(slot, 0x200000).is_some()
            && self.float(slot, 0x200001).is_some()
            && self
                .model_id(BODY)
                .is_some_and(|body| model_allowed(BODY, body));
        let response = ui
            .add_enabled(available, egui::Button::new("Standard positions…"))
            .on_hover_text("Load a standard pickup position and angle for this body.");
        #[cfg(test)]
        self.pickup_controls
            .insert((slot, "standard"), response.rect);
        if response.clicked() {
            self.open_pickup_positions(slot);
        }
    }

    fn open_pickup_positions(&mut self, slot: u32) {
        let Some(body) = self.model_id(BODY) else {
            return;
        };
        let choices = if let Some(folder) = self.body_definitions.as_deref() {
            override_definition(folder, body)
                .and_then(|definition| positions(&definition))
                .map_err(|error| format!("Could not load Workbench body definitions: {error}"))
        } else {
            ModelLibrary::built_in()
                .and_then(|models| {
                    models
                        .bodies
                        .get(&body)
                        .ok_or_else(|| "This body has no standard pickup positions.".into())
                })
                .and_then(positions)
        };
        match choices {
            Ok(positions) => {
                self.error = None;
                self.pickup_position_dialog = Some(PickupPositionDialog {
                    slot,
                    body,
                    selected: if slot == PICKUP2 {
                        positions.len() - 1
                    } else {
                        0
                    },
                    positions,
                });
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn pickup_position_dialog(
        &mut self,
        ctx: &egui::Context,
        _services: &mut EditorServices<'_>,
    ) {
        let Some(mut dialog) = self.pickup_position_dialog.take() else {
            return;
        };
        let response = egui::Modal::new(egui::Id::new("variax-pickup-positions")).show(ctx, |ui| {
            ui.heading("Move Pickup to Standard Positions");
            let _choices = egui::ComboBox::from_id_salt("standard-pickup-position")
                .selected_text(dialog.positions[dialog.selected].label)
                .show_ui(ui, |ui| {
                    for (index, option) in dialog.positions.iter().enumerate() {
                        let _response =
                            ui.selectable_value(&mut dialog.selected, index, option.label);
                        #[cfg(test)]
                        _services
                            .prompt_buttons
                            .insert(option.label, _response.rect);
                    }
                });
            #[cfg(test)]
            _services
                .prompt_buttons
                .insert("pickup-position-choices", _choices.response.rect);
            ui.horizontal(|ui| {
                let ok = ui.button("OK");
                let cancel = ui.button("Cancel");
                #[cfg(test)]
                {
                    _services
                        .prompt_buttons
                        .insert("confirm-pickup-position", ok.rect);
                    _services
                        .prompt_buttons
                        .insert("cancel-pickup-position", cancel.rect);
                }
                (ok.clicked(), cancel.clicked())
            })
            .inner
        });
        let (confirm, cancel) = response.inner;
        if confirm {
            if self.model_id(BODY) != Some(dialog.body) {
                self.error =
                    Some("The selected body changed. Choose a pickup position again.".into());
                return;
            }
            let position = dialog.positions[dialog.selected];
            self.change(|preset| {
                preset.set_parameter(dialog.slot, 0x200001, 1, position.degrees.to_bits())?;
                preset.set_parameter(dialog.slot, 0x200000, 1, position.inches.to_bits())
            });
        } else if !cancel && !response.should_close() {
            self.pickup_position_dialog = Some(dialog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{EditorDefaults, EditorDocument, FAMILY, PICKUP1, PresetFile};
    use super::*;
    use crate::editor::{Workbench, tests::click_prompt};
    use std::{fs, time::Instant};

    fn startup(preferences: &Path) -> (egui::Context, Workbench) {
        let ctx = egui::Context::default();
        let mut app = Workbench::new(
            &ctx,
            EditorDefaults {
                artwork: preferences.with_file_name("artwork"),
                preferences: Some(preferences.to_owned()),
                startup_panel: None,
            },
        );
        app.theme_polled = Instant::now();
        app.new_tab(&FAMILY);
        app.variax_mut().open_example();
        (ctx, app)
    }

    fn definition(has_middle: bool) -> Vec<u8> {
        let mut bytes = vec![0; BODY_DEFINITION_BYTES];
        bytes[0] = u8::from(has_middle);
        for (index, (position, angle)) in [(2.25_f32, 0.25_f32), (4.5, -0.5), (7.75, 0.0)]
            .into_iter()
            .enumerate()
        {
            let field = 1 + index * 32;
            bytes[field..field + 4].copy_from_slice(&position.to_be_bytes());
            bytes[field + 4..field + 8].copy_from_slice(&angle.to_be_bytes());
        }
        bytes
    }

    fn frame(ctx: &egui::Context, app: &mut Workbench, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440.0, 1000.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.render(ctx),
        );
    }

    fn open(ctx: &egui::Context, app: &mut Workbench, pickup: u32) {
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        let pos = app.variax().pickup_controls[&(pickup, "standard")].center();
        click(ctx, app, pos);
        app.poll_picker(ctx);
    }

    fn click(ctx: &egui::Context, app: &mut Workbench, pos: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }

    fn choose_second_pickup_position(
        ctx: &egui::Context,
        app: &mut Workbench,
        label: &'static str,
    ) {
        open(ctx, app, PICKUP2);
        let dialog = app.variax().pickup_position_dialog.as_ref().unwrap();
        assert_eq!(dialog.positions[dialog.selected].label, "Neck");
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        let combo = app.pending_buttons["pickup-position-choices"].center();
        click(ctx, app, combo);
        frame(ctx, app, vec![]);
        frame(ctx, app, vec![]);
        let option = app.pending_buttons[label].center();
        click(ctx, app, option);
        let dialog = app.variax().pickup_position_dialog.as_ref().unwrap();
        assert_eq!(dialog.positions[dialog.selected].label, label);
    }

    fn write_private_definition(directory: &Path) -> std::path::PathBuf {
        let folder = directory.join("override");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("strat.bin"), definition(true)).unwrap();
        folder
    }

    #[test]
    fn clean_startup_uses_bundled_positions_without_a_folder_picker() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = directory.path().join("editor-preferences.txt");
        let (ctx, mut app) = startup(&preferences);
        assert!(app.variax().body_definitions.is_none());
        assert_eq!(app.variax().model_id(BODY), Some(0x0310_0003));
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();

        open(&ctx, &mut app, PICKUP1);
        let dialog = app.variax().pickup_position_dialog.as_ref().unwrap();
        assert_eq!(
            dialog
                .positions
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            ["Bridge", "Middle", "Neck"]
        );
        assert_eq!(dialog.selected, 0);
        click_prompt(&ctx, &mut app, "confirm-pickup-position");
        assert!((app.variax().float(PICKUP1, 0x200000).unwrap() - 1.803_116).abs() < 1e-6);
        assert!((app.variax().float(PICKUP1, 0x200001).unwrap() - 8.775).abs() < 1e-6);
        assert!(app.variax().document.as_ref().unwrap().can_undo());
        app.variax_mut().undo();
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.as_bytes(), before);
        assert!(!document.can_undo());

        app.variax_mut()
            .change(|preset| preset.select_model(BODY, 0x0310_0001));
        open(&ctx, &mut app, PICKUP2);
        let dialog = app.variax().pickup_position_dialog.as_ref().unwrap();
        assert_eq!(
            dialog
                .positions
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            ["Bridge", "Neck"]
        );
        assert_eq!(dialog.selected, 1);
        click_prompt(&ctx, &mut app, "cancel-pickup-position");
    }

    #[test]
    fn acoustic_body_refuses_standard_positions_for_both_pickups() {
        let directory = tempfile::tempdir().unwrap();
        let (ctx, mut app) = startup(&directory.path().join("editor-preferences.txt"));
        app.variax_mut()
            .change(|preset| preset.select_model(BODY, 0x0310_0012));
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();

        for pickup in [PICKUP1, PICKUP2] {
            open(&ctx, &mut app, pickup);
            assert!(app.variax().pickup_position_dialog.is_none());
        }
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    }

    #[test]
    fn missing_pickup_slot_refuses_standard_positions() {
        let directory = tempfile::tempdir().unwrap();
        let (ctx, mut app) = startup(&directory.path().join("editor-preferences.txt"));
        let mut bytes =
            include_bytes!("../../../../tests/fixtures/synthetic-host-preset.vwm").to_vec();
        let pattern = [0x0311_0001_u32.to_be_bytes(), PICKUP1.to_be_bytes()].concat();
        let model = bytes
            .windows(pattern.len())
            .position(|window| window == pattern)
            .unwrap();
        bytes[model + 4..model + 8].copy_from_slice(&0x0201_0101_u32.to_be_bytes());
        app.variax_mut().document = Some(EditorDocument::from_preset(
            PresetFile::parse(bytes.clone()).unwrap(),
            "missing-pickup.vwm".into(),
        ));
        assert_eq!(app.variax().model_id(PICKUP1), None);

        open(&ctx, &mut app, PICKUP1);
        assert!(app.variax().pickup_position_dialog.is_none());
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), bytes);
    }

    #[test]
    fn remembered_override_loads_private_definitions_and_groups_angle_and_position() {
        let directory = tempfile::tempdir().unwrap();
        let preferences = directory.path().join("editor-preferences.txt");
        let override_folder = write_private_definition(directory.path());
        let file = override_folder.join("strat.bin");
        let bytes = definition(true);
        let (_, mut app) = startup(&preferences);
        app.variax_mut()
            .remember_body_definitions(&override_folder)
            .unwrap();
        drop(app);
        let (ctx, mut app) = startup(&preferences);
        assert_eq!(
            app.variax().body_definitions.as_deref(),
            Some(override_folder.as_path())
        );
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        open(&ctx, &mut app, PICKUP1);
        let dialog = app.variax_mut().pickup_position_dialog.as_mut().unwrap();
        assert_eq!(
            dialog
                .positions
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            ["Bridge", "Middle", "Neck"]
        );
        assert_eq!(dialog.selected, 0);
        dialog.selected = 1;
        click_prompt(&ctx, &mut app, "confirm-pickup-position");
        assert_eq!(app.variax().float(PICKUP1, 0x200000), Some(4.5));
        assert_eq!(app.variax().float(PICKUP1, 0x200001), Some(-22.5));
        app.variax_mut().undo();
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.as_bytes(), before);
        assert!(!document.can_undo());
        assert_eq!(fs::read(&file).unwrap(), bytes);

        fs::write(&file, definition(false)).unwrap();
        open(&ctx, &mut app, PICKUP2);
        let dialog = app.variax().pickup_position_dialog.as_ref().unwrap();
        assert_eq!(
            dialog
                .positions
                .iter()
                .map(|item| item.label)
                .collect::<Vec<_>>(),
            ["Bridge", "Neck"]
        );
        assert_eq!(dialog.selected, 1);
        click_prompt(&ctx, &mut app, "cancel-pickup-position");
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);

        fs::write(&file, b"truncated").unwrap();
        open(&ctx, &mut app, PICKUP1);
        assert!(app.variax().pickup_position_dialog.is_none());
        assert!(
            app.variax()
                .error
                .as_deref()
                .unwrap()
                .contains("body definition")
        );
        assert_eq!(app.variax().document.as_ref().unwrap().as_bytes(), before);
    }

    #[test]
    fn second_pickup_dialog_applies_one_edit_and_cancel_preserves_history() {
        fn document(app: &Workbench) -> &EditorDocument {
            app.variax().document.as_ref().unwrap()
        }

        let directory = tempfile::tempdir().unwrap();
        let folder = write_private_definition(directory.path());
        let (ctx, mut app) = startup(&directory.path().join("editor-preferences.txt"));
        app.variax_mut().remember_body_definitions(&folder).unwrap();
        let original = document(&app).as_bytes().to_vec();
        app.variax_mut()
            .change(|preset| preset.rename("Prior edit"));
        let before = document(&app).as_bytes().to_vec();
        assert_ne!(before, original);
        assert_ne!(app.variax().float(PICKUP2, 0x200000), Some(4.5));
        assert_ne!(app.variax().float(PICKUP2, 0x200001), Some(-22.5));
        let first_pickup = document(&app).preset().model(PICKUP1).unwrap().clone();
        let mut expected = document(&app).preset().clone();
        expected
            .set_parameter(PICKUP2, 0x200000, 1, 4.5_f32.to_bits())
            .unwrap();
        expected
            .set_parameter(PICKUP2, 0x200001, 1, (-22.5_f32).to_bits())
            .unwrap();

        choose_second_pickup_position(&ctx, &mut app, "Middle");
        assert_eq!(document(&app).as_bytes(), before);
        click_prompt(&ctx, &mut app, "confirm-pickup-position");
        assert!(app.variax().pickup_position_dialog.is_none());
        assert_eq!(app.variax().float(PICKUP2, 0x200000), Some(4.5));
        assert_eq!(app.variax().float(PICKUP2, 0x200001), Some(-22.5));
        assert_eq!(document(&app).preset().model(PICKUP1), Some(&first_pickup));
        assert!(
            document(&app).preset().as_bytes() == expected.as_bytes(),
            "Only the second pickup's position and angle should change"
        );
        let applied = document(&app).as_bytes().to_vec();

        app.variax_mut().undo();
        assert_eq!(document(&app).as_bytes(), before);
        assert!(document(&app).can_undo());
        assert!(document(&app).can_redo());

        choose_second_pickup_position(&ctx, &mut app, "Bridge");
        click_prompt(&ctx, &mut app, "cancel-pickup-position");
        assert!(app.variax().pickup_position_dialog.is_none());
        assert_eq!(document(&app).as_bytes(), before);
        assert!(document(&app).can_undo());
        assert!(document(&app).can_redo());

        app.variax_mut().undo();
        assert_eq!(document(&app).as_bytes(), original);
        assert!(!document(&app).can_undo());
        app.variax_mut().redo();
        assert_eq!(document(&app).as_bytes(), before);
        app.variax_mut().redo();
        assert_eq!(document(&app).as_bytes(), applied);
        assert!(!document(&app).can_redo());
    }
}
