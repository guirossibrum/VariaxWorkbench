use super::{BODY, EditorServices, PICKUP2, Path, PickRequest, VariaxEditor, egui, tabs};
use variax_workbench::body::{BODY_DEFINITION_BYTES, BodyDefinition};

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

fn positions(folder: &Path, body: u32) -> Result<Vec<Position>, String> {
    let resource = resource(body).ok_or("This body has no standard pickup positions.")?;
    let path = folder.join(resource);
    let bytes = tabs::open_candidate(&path, BODY_DEFINITION_BYTES as u64)?;
    let body = BodyDefinition::parse(&bytes)?;
    // The original JNI adapter sets Bridge/Neck true for a loaded body and
    // uses the resource's presence flag for Middle. The UI multiplies by 45.
    let mut positions = Vec::new();
    for (index, (label, defaults)) in ["Bridge", "Middle", "Neck"]
        .into_iter()
        .zip(body.pickup_defaults)
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
    pub(super) fn standard_pickup_positions(
        &mut self,
        ui: &mut egui::Ui,
        slot: u32,
        services: &mut EditorServices<'_>,
    ) {
        let available = self.can_edit(slot, Some(0x200000))
            && self.can_edit(slot, Some(0x200001))
            && self.model_id(BODY).and_then(resource).is_some();
        let response = ui
            .add_enabled(available, egui::Button::new("Standard positions…"))
            .on_hover_text("Load a standard pickup position and angle for this body.");
        #[cfg(test)]
        self.pickup_controls
            .insert((slot, "standard"), response.rect);
        if response.clicked() {
            if self.body_definitions.is_none() {
                self.choose_body_definitions(ui.ctx(), services, Some(slot));
            } else {
                self.open_pickup_positions(slot);
            }
        }
    }

    pub(super) fn choose_body_definitions(
        &mut self,
        ctx: &egui::Context,
        services: &mut EditorServices<'_>,
        pickup: Option<u32>,
    ) {
        services.request_picker::<Self>(ctx, PickRequest::Folder, move |editor, path, _| {
            let Some(path) = path else { return };
            if let Err(error) = editor.remember_body_definitions(&path) {
                editor.error = Some(error);
                return;
            }
            if let Some(slot) = pickup {
                editor.open_pickup_positions(slot);
            }
        });
    }

    fn open_pickup_positions(&mut self, slot: u32) {
        let (Some(folder), Some(body)) = (self.body_definitions.as_deref(), self.model_id(BODY))
        else {
            return;
        };
        match positions(folder, body) {
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
            Err(error) => {
                self.error = Some(format!(
                    "Could not load Workbench body definitions: {error}"
                ))
            }
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
            egui::ComboBox::from_id_salt("standard-pickup-position")
                .selected_text(dialog.positions[dialog.selected].label)
                .show_ui(ui, |ui| {
                    for (index, option) in dialog.positions.iter().enumerate() {
                        ui.selectable_value(&mut dialog.selected, index, option.label);
                    }
                });
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
    use super::super::{PICKUP1, Tab};
    use super::*;
    use crate::editor::{
        FakeDialogs, Workbench,
        tests::{app, click_prompt},
    };
    use std::fs;

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
        app.poll_picker(ctx);
    }

    #[test]
    fn standard_position_buttons_load_private_definitions_and_group_angle_and_position() {
        let (ctx, mut app) = app();
        app.variax_mut().open_example();
        app.variax_mut().tab = Tab::Pickups;
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("strat.bin");
        let bytes = definition(true);
        fs::write(&file, &bytes).unwrap();
        app.dialogs = Box::new(FakeDialogs {
            folder: [Some(directory.path().to_owned())].into(),
            ..Default::default()
        });
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
}
