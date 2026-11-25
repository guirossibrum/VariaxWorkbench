use super::{BODY, PICKUP1, PICKUP2, Tab, VariaxEditor, egui};
use variax_workbench::ui::{
    assets::UiImage,
    body_canvas::{
        BodyCanvas, BodyEvent, DragKind, PickupPlacement, PickupSprite, move_inches, rotate_degrees,
    },
};

const SPANK_BODY: &[u8] = include_bytes!("../../../../assets/ui/variax/spank-body.png");
const SPANK_PICKUP: &[u8] = include_bytes!("../../../../assets/ui/variax/spank-pickup.png");

#[derive(Default)]
pub(super) struct BundledArtwork {
    body: Option<egui::TextureHandle>,
    pickup: Option<egui::TextureHandle>,
}

pub(super) struct BodyDrag {
    slot: u32,
    kind: DragKind,
    pointer: egui::Pos2,
    scale: f32,
}

impl VariaxEditor {
    fn canvas_image(&mut self, ctx: &egui::Context, slot: u32) -> Option<UiImage> {
        let model_id = self.model_id(slot)?;
        let name = self
            .catalog
            .iter()
            .find(|art| art.id == model_id)?
            .image
            .clone();
        let texture = self.texture(ctx, &name).or_else(|| {
            let (key, bytes) = match name.as_str() {
                "bodies/strat.png" => ("bundled-spank-body", SPANK_BODY),
                "pickups/strat.png" => ("bundled-spank-pickup", SPANK_PICKUP),
                _ => return None,
            };
            let cached = if slot == BODY {
                &mut self.bundled_artwork.body
            } else {
                &mut self.bundled_artwork.pickup
            };
            if let Some(texture) = cached {
                return Some(texture.clone());
            }
            let pixels = image::load_from_memory(bytes).ok()?.into_rgba8();
            // The archived Spank sprite is centered in a transparent square.
            // Trim that padding symmetrically so the canvas outlines the pickup.
            let pixels = if slot == BODY {
                pixels
            } else {
                image::imageops::crop_imm(&pixels, 37, 3, 46, 114).to_image()
            };
            let size = [pixels.width() as usize, pixels.height() as usize];
            let color = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_raw());
            let texture = ctx.load_texture(key, color, egui::TextureOptions::LINEAR);
            *cached = Some(texture.clone());
            Some(texture)
        })?;
        Some(UiImage {
            name,
            pixels: texture.size(),
            texture: Some(texture),
            missing: false,
        })
    }

    pub(super) fn guitar(&mut self, ui: &mut egui::Ui, height: f32, editable: bool) -> egui::Rect {
        let body = self
            .canvas_image(ui.ctx(), BODY)
            .unwrap_or_else(|| UiImage::missing("Guitar artwork unavailable"));
        let order = if self.pickup_front == PICKUP2 {
            [PICKUP1, PICKUP2]
        } else {
            [PICKUP2, PICKUP1]
        };
        let acoustic = self.acoustic_body();
        let images: Vec<_> = order
            .into_iter()
            .filter(|_| !acoustic)
            .filter_map(|slot| self.canvas_image(ui.ctx(), slot).map(|image| (slot, image)))
            .collect();
        let sprites: Vec<_> = images
            .iter()
            .map(|(slot, image)| PickupSprite {
                position_inches: self.float(*slot, 0x200000).unwrap_or(2.0),
                angle_degrees: self.float(*slot, 0x200001).unwrap_or(0.0),
                enabled: self
                    .document
                    .as_ref()
                    .and_then(|document| document.preset().model(*slot))
                    .is_some_and(|model| model.enabled),
                selected: self.tab == Tab::Pickups && self.pickup_front == *slot,
                image,
            })
            .collect();
        let placement = PickupPlacement::default();
        let canvas = ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                BodyCanvas::new(&body, &sprites)
                    .interactive(editable && ui.is_enabled())
                    .show(ui)
            },
        );
        let output = canvas.inner;
        #[cfg(test)]
        {
            self.pickup_centers.clear();
            for ((slot, _), sprite) in images.iter().zip(&sprites) {
                self.pickup_centers.insert(
                    *slot,
                    placement.screen_point(output.art_rect, sprite.position_inches),
                );
            }
        }

        let pointer = ui.input(|input| input.pointer.interact_pos());
        for event in output.events {
            match event {
                BodyEvent::DragStarted { index, kind } => {
                    let slot = images[index].0;
                    let parameter = match kind {
                        DragKind::Move => 0x200000,
                        DragKind::Rotate => 0x200001,
                    };
                    if self.can_edit(slot, Some(parameter))
                        && self.float(slot, parameter).is_some_and(f32::is_finite)
                        && let (Some(document), Some(pointer)) = (&mut self.document, pointer)
                    {
                        document.begin_gesture();
                        self.drag_origin = Some(BodyDrag {
                            slot,
                            kind,
                            pointer,
                            scale: placement.screen_scale(output.art_rect),
                        });
                    }
                }
                BodyEvent::Moved { index, inches } => {
                    self.apply_body_drag(images[index].0, 0x200000, inches)
                }
                BodyEvent::Rotated { index, degrees } => {
                    self.apply_body_drag(images[index].0, 0x200001, degrees)
                }
                BodyEvent::DragFinished { .. } => {
                    if editable
                        && ui.input(|input| {
                            input.pointer.button_released(egui::PointerButton::Primary)
                        })
                        && let (Some(drag), Some(pointer)) = (&self.drag_origin, pointer)
                    {
                        // A release can carry a final position without a drag update.
                        let delta = pointer - drag.pointer;
                        let slot = drag.slot;
                        let (id, value) = match drag.kind {
                            DragKind::Move => {
                                let origin = self.float(slot, 0x200000).unwrap();
                                (
                                    0x200000,
                                    move_inches(origin, delta.x / drag.scale, placement),
                                )
                            }
                            DragKind::Rotate => {
                                let origin = self.float(slot, 0x200001).unwrap();
                                let fine = ui.input(|input| input.modifiers.ctrl);
                                (0x200001, rotate_degrees(origin, delta.y, fine))
                            }
                        };
                        self.apply_body_drag(slot, id, value);
                    }
                    self.finish_body_drag();
                }
                BodyEvent::BodyClicked => self.tab = Tab::Body,
                BodyEvent::PickupClicked { index } => {
                    self.pickup_front = images[index].0;
                    self.tab = Tab::Pickups;
                }
                BodyEvent::PickupToggled { index } => {
                    let slot = images[index].0;
                    if self.can_edit(slot, None) {
                        self.set_pickup_enabled(slot, !sprites[index].enabled);
                    }
                    self.tab = Tab::Pickups;
                }
            }
        }
        if !ui.input(|input| input.pointer.primary_down()) {
            self.finish_body_drag();
        } else if let (Some(drag), Some(pointer)) = (&mut self.drag_origin, pointer) {
            drag.pointer = pointer;
        }
        canvas.response.rect
    }

    fn apply_body_drag(&mut self, slot: u32, parameter: u32, value: f32) {
        if self
            .drag_origin
            .as_ref()
            .is_some_and(|drag| drag.slot == slot)
            && self.float(slot, parameter) != Some(value)
        {
            self.change(|preset| preset.set_parameter(slot, parameter, 1, value.to_bits()));
        }
    }

    fn finish_body_drag(&mut self) {
        if self.drag_origin.take().is_some()
            && let Some(document) = &mut self.document
        {
            document.end_gesture();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::{PathBuf, Workbench};

    #[test]
    fn outer_pickup_drag_rotates_in_fine_steps_and_undoes_once() {
        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, PathBuf::new(), Some("pickups"));
        let artwork = ctx.load_texture(
            "rotation fixture",
            egui::ColorImage::filled([26, 92], egui::Color32::WHITE),
            egui::TextureOptions::LINEAR,
        );
        const HEIGHT: f32 = 220.0;
        app.variax_mut()
            .textures
            .insert("pickups/strat.png".into(), artwork);
        let frame = |app: &mut Workbench, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(750.0, 400.0),
                    )),
                    modifiers: egui::Modifiers::CTRL,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        app.variax_mut().guitar(ui, HEIGHT, true);
                    });
                },
            );
        };
        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let angle = app.variax().float(PICKUP1, 0x200001).unwrap();
        let position = app.variax().float(PICKUP1, 0x200000).unwrap();
        let center = app.variax().pickup_centers[&PICKUP1];
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        let radius = 0.38 * 92.0 * HEIGHT / PickupPlacement::default().art.y;
        let start =
            center + egui::emath::Rot2::from_angle(angle.to_radians()) * egui::vec2(0.0, -radius);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::CTRL,
        };
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(start), button(start, true)],
        );
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(start - egui::vec2(0.0, 8.0))],
        );
        frame(&mut app, vec![button(start - egui::vec2(0.0, 12.0), false)]);
        let rotated = app.variax().float(PICKUP1, 0x200001).unwrap();
        assert!((rotated - rotate_degrees(angle, -12.0, true)).abs() < 1e-5);
        assert_eq!(app.variax().float(PICKUP1, 0x200000), Some(position));
        app.variax_mut().undo();
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.as_bytes(), before);
        assert!(!document.can_undo() && document.can_redo());
    }
}
