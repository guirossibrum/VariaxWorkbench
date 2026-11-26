use super::{BODY, PICKUP1, PICKUP2, Tab, VariaxEditor, egui};
use std::sync::Arc;
use variax_workbench::ui::{
    assets::UiImage,
    body_canvas::{BodyCanvas, BodyEvent, DragKind, PickupSprite},
};

const SPANK_BODY: &[u8] = include_bytes!("../../../../assets/ui/variax/spank-body.png");
const SPANK_PICKUP: &[u8] = include_bytes!("../../../../assets/ui/variax/spank-pickup.png");

#[derive(Clone)]
pub(in crate::editor) struct ArtworkTexture {
    pub(super) handle: egui::TextureHandle,
    rgba: Option<Arc<[u8]>>,
}

impl ArtworkTexture {
    pub(super) fn new(
        ctx: &egui::Context,
        key: &str,
        pixels: image::RgbaImage,
        retain_rgba: bool,
    ) -> Self {
        let size = [pixels.width() as usize, pixels.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_raw());
        Self {
            handle: ctx.load_texture(key, color, egui::TextureOptions::LINEAR),
            rgba: retain_rgba.then(|| Arc::from(pixels.into_raw())),
        }
    }
}

#[derive(Default)]
pub(super) struct BundledArtwork {
    body: Option<ArtworkTexture>,
    pickup: Option<ArtworkTexture>,
}

impl VariaxEditor {
    fn canvas_image(
        &mut self,
        ctx: &egui::Context,
        slot: u32,
    ) -> Option<(UiImage, Option<Arc<[u8]>>)> {
        let model_id = self.model_id(slot)?;
        let name = self
            .catalog
            .iter()
            .find(|art| art.id == model_id)?
            .image
            .clone();
        let artwork = self.artwork_texture(ctx, &name).or_else(|| {
            let (cache, key, bytes) = match name.as_str() {
                "bodies/strat.png" => (
                    &mut self.bundled_artwork.body,
                    "bundled-spank-body",
                    SPANK_BODY,
                ),
                "pickups/strat.png" => (
                    &mut self.bundled_artwork.pickup,
                    "bundled-spank-pickup",
                    SPANK_PICKUP,
                ),
                _ => return None,
            };
            if cache.is_none() {
                let pixels = image::load_from_memory(bytes).ok()?.into_rgba8();
                *cache = Some(ArtworkTexture::new(ctx, key, pixels, slot != BODY));
            }
            cache.clone()
        })?;
        let image = UiImage {
            name,
            pixels: artwork.handle.size(),
            texture: Some(artwork.handle),
            missing: false,
        };
        Some((image, artwork.rgba))
    }

    #[cfg(test)]
    pub(super) fn install_pickup_fixture(&mut self, ctx: &egui::Context) {
        let pixels = image::RgbaImage::from_pixel(26, 92, image::Rgba([255; 4]));
        let key = "pickups/strat.png";
        self.textures
            .insert(key.into(), ArtworkTexture::new(ctx, key, pixels, true));
    }

    pub(super) fn guitar(&mut self, ui: &mut egui::Ui, height: f32, editable: bool) -> egui::Rect {
        let body = self
            .canvas_image(ui.ctx(), BODY)
            .map(|(image, _)| image)
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
            .map(|(slot, (image, _))| PickupSprite {
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
        let alphas = images
            .iter()
            .map(|(_, (_, rgba))| rgba.as_deref())
            .collect::<Vec<_>>();
        let canvas = ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                BodyCanvas::new(&body, &sprites)
                    .alphas(&alphas)
                    .interactive(editable && ui.is_enabled())
                    .show(ui)
            },
        );
        let output = canvas.inner;
        #[cfg(test)]
        {
            use variax_workbench::ui::body_canvas::PickupPlacement;
            let placement = PickupPlacement::default();
            self.pickup_centers.clear();
            for ((slot, _), sprite) in images.iter().zip(&sprites) {
                self.pickup_centers.insert(
                    *slot,
                    placement.screen_point(output.art_rect, sprite.position_inches),
                );
            }
        }

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
                        && let Some(document) = &mut self.document
                    {
                        document.begin_gesture();
                        self.drag_origin = Some(slot);
                    }
                }
                BodyEvent::Moved { index, inches } => {
                    self.apply_body_drag(images[index].0, 0x200000, inches)
                }
                BodyEvent::Rotated { index, degrees } => {
                    self.apply_body_drag(images[index].0, 0x200001, degrees)
                }
                BodyEvent::DragFinished { .. } => self.finish_body_drag(),
                BodyEvent::BodyClicked => self.tab = Tab::Body,
                BodyEvent::PickupClicked { .. } => self.tab = Tab::Pickups,
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
        }
        canvas.response.rect
    }

    fn apply_body_drag(&mut self, slot: u32, parameter: u32, value: f32) {
        if self.drag_origin == Some(slot) && self.float(slot, parameter) != Some(value) {
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
    use variax_workbench::ui::body_canvas::{PickupPlacement, rotate_degrees};

    #[test]
    fn outer_pickup_drag_rotates_in_fine_steps_and_undoes_once() {
        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, PathBuf::new(), Some("pickups"));
        const HEIGHT: f32 = 220.0;
        app.variax_mut().install_pickup_fixture(&ctx);
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
        // Workbench rebases on the first Ctrl-held move.
        assert_eq!(app.variax().float(PICKUP1, 0x200001), Some(angle));
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(start - egui::vec2(0.0, 16.0))],
        );
        let rotated = app.variax().float(PICKUP1, 0x200001).unwrap();
        assert!((rotated - rotate_degrees(angle, -8.0, true)).abs() < 1e-5);
        frame(&mut app, vec![button(start - egui::vec2(0.0, 20.0), false)]);
        assert_eq!(app.variax().float(PICKUP1, 0x200001), Some(rotated));
        assert_eq!(app.variax().float(PICKUP1, 0x200000), Some(position));
        app.variax_mut().undo();
        let document = app.variax().document.as_ref().unwrap();
        assert_eq!(document.as_bytes(), before);
        assert!(!document.can_undo() && document.can_redo());
    }

    #[test]
    fn pickup_artwork_alpha_controls_press_selection_and_leaves_the_front_pickup_in_place() {
        const HEIGHT: f32 = 220.0;
        let ctx = egui::Context::default();
        let folder = tempfile::tempdir().unwrap();
        let directory = folder.path().join("pickups");
        std::fs::create_dir(&directory).unwrap();
        let pixels = image::RgbaImage::from_fn(26, 92, |_, y| {
            image::Rgba([255, 255, 255, if (38..54).contains(&y) { 204 } else { 255 }])
        });
        pixels.save(directory.join("strat.png")).unwrap();
        let mut app = Workbench::with_context(&ctx, None, folder.path().into(), None);
        let before = app.variax().document.as_ref().unwrap().as_bytes().to_vec();
        let mut time = 0.0;
        let mut frame = |app: &mut Workbench, events| {
            time += 0.5;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(750.0, 400.0),
                    )),
                    time: Some(time),
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
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };

        frame(&mut app, vec![]);
        frame(&mut app, vec![]);
        let center = app.variax().pickup_centers[&PICKUP2];
        app.variax_mut().tab = Tab::Controls;
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(center), button(center, true)],
        );
        assert_eq!(app.variax().tab, Tab::Body);
        frame(&mut app, vec![button(center, false)]);

        let scale = HEIGHT / PickupPlacement::default().art.y;
        let angle = app.variax().float(PICKUP2, 0x200001).unwrap();
        let opaque = center
            + egui::emath::Rot2::from_angle(angle.to_radians()) * egui::vec2(0.0, -35.0 * scale);
        app.variax_mut().tab = Tab::Controls;
        frame(
            &mut app,
            vec![egui::Event::PointerMoved(opaque), button(opaque, true)],
        );
        assert_eq!(app.variax().tab, Tab::Pickups);
        assert_eq!(app.variax().pickup_front, PICKUP1);
        frame(&mut app, vec![button(opaque, false)]);
        let document = app.variax().document.as_ref().unwrap();
        assert!(
            document.as_bytes() == before,
            "Panel selection changed preset bytes"
        );
        assert!(!document.can_undo());
    }
}
