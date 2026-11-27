use super::{BODY, PICKUP1, PICKUP2, Tab, VariaxEditor, egui};
use std::sync::Arc;
use variax_workbench::ui::{
    assets::UiImage,
    body_canvas::{
        BodyCanvas, BodyEvent, DragKind, PickupPlacement, PickupSprite, outline_rect,
        paint_body_outline, placement_for_body, wireframe_path,
    },
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
    pub(super) fn body_outline(&mut self, ui: &mut egui::Ui) {
        let Some(model_id) = self.model_id(BODY) else {
            return;
        };
        let path = wireframe_path(model_id);
        let image = match self.texture(ui.ctx(), path) {
            Some(texture) => UiImage {
                name: path.into(),
                pixels: texture.size(),
                texture: Some(texture),
                missing: false,
            },
            None => UiImage::missing("Body outline unavailable"),
        };
        let extent = outline_rect(egui::Pos2::ZERO, &image, 1.0).max.to_vec2();
        let (panel, _) = ui.allocate_exact_size(extent, egui::Sense::hover());
        paint_body_outline(ui, &image, panel.min, 1.0);
    }

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

    pub(super) fn body_placement(&self) -> PickupPlacement {
        self.model_id(BODY)
            .map(placement_for_body)
            .unwrap_or_default()
    }

    pub(super) fn guitar(&mut self, ui: &mut egui::Ui, height: f32, editable: bool) {
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
        let placement = self.body_placement();
        let canvas = ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                BodyCanvas::new(&body, &sprites)
                    .placement(placement)
                    .alphas(&alphas)
                    .interactive(editable && ui.is_enabled())
                    .show(ui)
            },
        );
        let output = canvas.inner;
        #[cfg(test)]
        {
            self.body_art_rect = Some(output.art_rect);
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
    use variax_workbench::ui::body_canvas::{
        POSITION_MAX, POSITION_MIN, pickup_limits, rotate_degrees,
    };

    #[test]
    fn pickup_drag_matches_workbench_175() {
        // Workbench 1.75 build 121 results for synthetic source dimensions.
        // The retained Windows and macOS headless runs agree bit for bit.
        const LESTER: u32 = 0x0310_0004;
        const FIREBIRD: u32 = 0x0310_0009;
        const ORIGIN: f32 = 4.8;
        let cases = [
            (LESTER, [26, 93], 100.0, 0x40e3_df23),
            (LESTER, [110, 30], 100.0, 0x40e7_bada),
            (FIREBIRD, [110, 30], -100.0, 0x3fca_5863),
            (LESTER, [26, 93], 198.0, 0x4116_5433),
            (FIREBIRD, [110, 30], 147.0, 0x4118_0000),
            (FIREBIRD, [26, 92], 100.0, 0x40f9_b3c6),
            (FIREBIRD, [110, 30], -147.0, 0x3dcc_cccd),
        ];
        for (body_id, pixels, dx, original_bits) in cases {
            let placement = placement_for_body(body_id);
            let body = UiImage::missing("Body");
            let mut pickup = UiImage::missing("Grabbed pickup");
            pickup.pixels = pixels;
            let mut other = UiImage::missing("Other pickup");
            other.pixels = [120, 120];
            let pickups = [
                PickupSprite {
                    position_inches: ORIGIN,
                    angle_degrees: 0.0,
                    enabled: true,
                    selected: false,
                    image: &pickup,
                },
                PickupSprite {
                    position_inches: POSITION_MIN,
                    angle_degrees: 0.0,
                    enabled: true,
                    selected: true,
                    image: &other,
                },
            ];
            for scale in [1.0, 0.5] {
                let ctx = egui::Context::default();
                let frame = |events| {
                    let mut output = None;
                    let _ = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                placement.art * scale,
                            )),
                            events,
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().frame(egui::Frame::NONE).show(
                                ctx,
                                |ui| {
                                    output = Some(
                                        BodyCanvas::new(&body, &pickups)
                                            .placement(placement)
                                            .show(ui),
                                    );
                                },
                            );
                        },
                    );
                    output.unwrap()
                };
                let art_rect = frame(vec![]).art_rect;
                assert_eq!(placement.screen_scale(art_rect), scale);
                let start = placement.screen_point(art_rect, ORIGIN);
                let press = frame(vec![
                    egui::Event::PointerMoved(start),
                    egui::Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ]);
                assert_eq!(press.events, [BodyEvent::PickupClicked { index: 0 }]);

                let end = start + egui::vec2(dx * scale, 0.0);
                let drag = frame(vec![egui::Event::PointerMoved(end)]);
                assert_eq!(
                    drag.events,
                    [
                        BodyEvent::DragStarted {
                            index: 0,
                            kind: DragKind::Move,
                        },
                        BodyEvent::Moved {
                            index: 0,
                            inches: f32::from_bits(original_bits),
                        },
                    ],
                    "{body_id:#x} {pixels:?}: {dx:+} px at scale {scale}",
                );
            }
        }
    }

    #[test]
    fn catalog_bodies_use_their_pickup_placement_for_rendering_and_dragging() {
        const HEIGHT: f32 = 220.0;
        const DRAG_ART_PIXELS: f32 = 100.0;
        let ctx = egui::Context::default();
        let mut app = Workbench::with_context(&ctx, None, PathBuf::new(), Some("pickups"));
        app.variax_mut().install_pickup_fixture(&ctx);
        let pickup_texture = app.variax().textures["pickups/strat.png"].handle.id();
        let frame = |app: &mut Workbench, events| {
            let mut stage = egui::Rect::NOTHING;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(750.0, 400.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        stage = egui::Rect::from_min_size(
                            ui.next_widget_position(),
                            egui::vec2(ui.available_width(), HEIGHT),
                        );
                        app.variax_mut().guitar(ui, HEIGHT, true);
                    });
                },
            );
            (stage, output.shapes)
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };

        let ids: Vec<_> = app
            .variax()
            .catalog
            .iter()
            .filter(|model| model.kind == "body")
            .map(|model| model.id)
            .collect();
        let position = app.variax().float(PICKUP1, 0x200000).unwrap();
        let second_position = app.variax().float(PICKUP2, 0x200000).unwrap();
        for id in ids {
            app.variax_mut()
                .change(|preset| preset.select_model(BODY, id));
            let (stage, shapes) = frame(&mut app, vec![]);
            let mut centers: Vec<_> = shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) if mesh.texture_id == pickup_texture => {
                        Some(mesh.calc_bounds().center())
                    }
                    _ => None,
                })
                .collect();
            if variax_workbench::edit::is_acoustic(id) {
                assert!(centers.is_empty(), "{id:#x} painted pickups");
                continue;
            }
            let placement = placement_for_body(id);
            let scale = HEIGHT / placement.art.y;
            let art_rect = egui::Rect::from_center_size(stage.center(), placement.art * scale);
            let start = placement.screen_point(art_rect, position);
            let second = placement.screen_point(art_rect, second_position);
            centers.sort_by(|a, b| a.x.total_cmp(&b.x));
            assert_eq!(centers.len(), 2, "{id:#x}");
            assert!(
                centers[0].distance(start) < 1e-3,
                "{id:#x}: {centers:?} vs {start}"
            );
            assert!(
                centers[1].distance(second) < 1e-3,
                "{id:#x}: {centers:?} vs {second}"
            );

            let end = start + egui::vec2(DRAG_ART_PIXELS * scale, 0.0);
            frame(
                &mut app,
                vec![egui::Event::PointerMoved(start), button(start, true)],
            );
            frame(&mut app, vec![egui::Event::PointerMoved(end)]);
            frame(&mut app, vec![button(end, false)]);
            let limits = pickup_limits(id);
            // The 26×92 test sprite is padded to a 105px square.
            let divisor = (limits.right - limits.left - 105) as f32;
            let span = POSITION_MAX - POSITION_MIN;
            let normalized = (position - POSITION_MIN) / span + DRAG_ART_PIXELS / divisor;
            let expected = normalized.clamp(0.0, 1.0) * span + POSITION_MIN;
            let actual = app.variax().float(PICKUP1, 0x200000).unwrap();
            assert!(
                (actual - expected).abs() < 1e-4,
                "{id:#x}: {actual} vs {expected}"
            );
            app.variax_mut().undo();
            assert_eq!(
                app.variax().float(PICKUP1, 0x200000),
                Some(position),
                "{id:#x}"
            );
        }
    }

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
