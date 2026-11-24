//! Guitar body with pickup sprites.
//!
//! The caller owns each pickup's position, angle, and on/off state. This
//! canvas paints the body and reports gestures. Position is inches along the
//! approved map, angle is degrees, and a drag does not write either value.
//!
//! #189 uses the move circle, the rotate margin, Ctrl, double-click, and the
//! body and pickup clicks. #190 passes a per-body [`PickupPlacement`]. #191
//! passes the wireframe through [`BodyCanvas::outline`].

use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Vec2, vec2};

use super::assets::{self, UiImage};
use super::theme::{AMBER, BG, LINE, RADIUS_PANEL};

/// Spank body art, and the art size the default placement uses.
pub const DEFAULT_ART: Vec2 = vec2(426.0, 412.0);
/// Horizontal pixel of 0.1 inches on [`DEFAULT_ART`].
pub const DEFAULT_X_AT_MIN: f32 = 68.0;
/// Pixel span from 0.1 inches to 9.5 inches on [`DEFAULT_ART`].
pub const DEFAULT_X_SPAN: f32 = 318.0;
/// Vertical center of every pickup on [`DEFAULT_ART`].
pub const DEFAULT_Y: f32 = 215.0;
pub const POSITION_MIN: f32 = 0.1;
pub const POSITION_MAX: f32 = 9.5;
pub const ANGLE_MIN: f32 = -45.0;
pub const ANGLE_MAX: f32 = 45.0;
/// Radius of the move circle, as a fraction of the sprite height.
///
/// Workbench's pickup control moves inside about a quarter of the image
/// height and rotates outside that circle.
pub const MOVE_RADIUS_OF_HEIGHT: f32 = 0.25;
/// One percent of the −45°…+45° range, per screen pixel. Up increases the angle.
pub const ROTATE_DEGREES_PER_PIXEL: f32 = 0.9;
/// Ctrl uses this fraction of [`ROTATE_DEGREES_PER_PIXEL`].
pub const FINE_ROTATE: f32 = 0.1;
/// Sprite width, as a fraction of the art width, when the image has no pixels.
///
/// 120 / 426 is the Spank pickup on the Spank body.
pub const FALLBACK_SPRITE_WIDTH: f32 = 120.0 / 426.0;

/// Where inches land on one body's artwork.
///
/// The default is the approved Spank map: `68 + 318 * (inches − 0.1) / 9.4`
/// at `y = 215`. #190 substitutes the range for another body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickupPlacement {
    pub art: Vec2,
    pub x_min: f32,
    pub x_max: f32,
    pub y: f32,
}

impl Default for PickupPlacement {
    fn default() -> Self {
        Self {
            art: DEFAULT_ART,
            x_min: DEFAULT_X_AT_MIN,
            x_max: DEFAULT_X_AT_MIN + DEFAULT_X_SPAN,
            y: DEFAULT_Y,
        }
    }
}

impl PickupPlacement {
    /// Art-pixel center for `inches`, clamped to 0.1…9.5.
    pub fn position_to_art(self, inches: f32) -> Pos2 {
        let inches = if inches.is_finite() {
            inches.clamp(POSITION_MIN, POSITION_MAX)
        } else {
            POSITION_MIN
        };
        let span = POSITION_MAX - POSITION_MIN;
        let t = (inches - POSITION_MIN) / span;
        Pos2::new(self.x_min + (self.x_max - self.x_min) * t, self.y)
    }

    /// Inverse of [`Self::position_to_art`] for a horizontal art pixel.
    pub fn art_x_to_position(self, x: f32) -> f32 {
        let span = self.x_max - self.x_min;
        if !span.is_finite() || span == 0.0 || !x.is_finite() {
            return POSITION_MIN;
        }
        let t = (x - self.x_min) / span;
        (POSITION_MIN + t * (POSITION_MAX - POSITION_MIN)).clamp(POSITION_MIN, POSITION_MAX)
    }

    pub fn screen_scale(self, art_rect: Rect) -> f32 {
        if self.art.x <= f32::EPSILON {
            1.0
        } else {
            art_rect.width() / self.art.x
        }
    }

    /// Screen center of `inches` inside the painted art rect.
    pub fn screen_point(self, art_rect: Rect, inches: f32) -> Pos2 {
        art_rect.min + self.position_to_art(inches).to_vec2() * self.screen_scale(art_rect)
    }
}

/// New position after a horizontal drag of `art_dx` art pixels.
pub fn move_inches(origin: f32, art_dx: f32, placement: PickupPlacement) -> f32 {
    let span = placement.x_max - placement.x_min;
    if !origin.is_finite() || !art_dx.is_finite() || !span.is_finite() || span == 0.0 {
        return origin.clamp(POSITION_MIN, POSITION_MAX);
    }
    (origin + art_dx / span * (POSITION_MAX - POSITION_MIN)).clamp(POSITION_MIN, POSITION_MAX)
}

/// New angle after a vertical drag. `screen_dy` is positive downward.
pub fn rotate_degrees(origin: f32, screen_dy: f32, fine: bool) -> f32 {
    if !origin.is_finite() || !screen_dy.is_finite() {
        return ANGLE_MIN;
    }
    let per_pixel = if fine {
        ROTATE_DEGREES_PER_PIXEL * FINE_ROTATE
    } else {
        ROTATE_DEGREES_PER_PIXEL
    };
    (origin - screen_dy * per_pixel).clamp(ANGLE_MIN, ANGLE_MAX)
}

/// Sprite size in art pixels. A missing image uses the Spank fraction of `art`.
pub fn sprite_art_size(pixels: [usize; 2], art: Vec2) -> Vec2 {
    if pixels[0] == 0 || pixels[1] == 0 {
        let width = art.x * FALLBACK_SPRITE_WIDTH;
        return vec2(width, width);
    }
    vec2(pixels[0] as f32, pixels[1] as f32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupZone {
    Move,
    Rotate,
}

/// `local` is the pointer relative to the sprite center, in the same units as
/// `sprite`, after the sprite's rotation has been removed.
pub fn zone_at(local: Vec2, sprite: Vec2) -> Option<PickupZone> {
    if sprite.x <= 0.0 || sprite.y <= 0.0 || !local.x.is_finite() || !local.y.is_finite() {
        return None;
    }
    let radius = sprite.y * MOVE_RADIUS_OF_HEIGHT;
    if local.length() <= radius {
        Some(PickupZone::Move)
    } else if local.x.abs() <= sprite.x * 0.5 && local.y.abs() <= sprite.y * 0.5 {
        Some(PickupZone::Rotate)
    } else {
        None
    }
}

/// One painted pickup. The last entry is the topmost, so the canvas passes
/// selected sprites last.
#[derive(Clone, Copy, Debug)]
pub struct SpriteHit {
    pub index: usize,
    pub center: Pos2,
    pub sprite: Vec2,
    pub angle_degrees: f32,
    pub selected: bool,
}

/// Topmost sprite under `pointer`. `paint_order` is back to front.
pub fn hit_sprite(pointer: Pos2, paint_order: &[SpriteHit]) -> Option<(usize, PickupZone)> {
    if !pointer.x.is_finite() || !pointer.y.is_finite() {
        return None;
    }
    for sprite in paint_order.iter().rev() {
        if !sprite.angle_degrees.is_finite() {
            continue;
        }
        let local = egui::emath::Rot2::from_angle(-sprite.angle_degrees.to_radians())
            * (pointer - sprite.center);
        if let Some(zone) = zone_at(local, sprite.sprite) {
            return Some((sprite.index, zone));
        }
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragKind {
    Move,
    Rotate,
}

/// A gesture. The value is the proposed position or angle, already clamped.
/// The caller stores it, including grouping one drag into one undo step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyEvent {
    DragStarted {
        index: usize,
        kind: DragKind,
    },
    Moved {
        index: usize,
        inches: f32,
    },
    Rotated {
        index: usize,
        degrees: f32,
    },
    DragFinished {
        index: usize,
        kind: DragKind,
    },
    /// A click on a pickup. The caller selects it and shows Pickups.
    PickupClicked {
        index: usize,
    },
    /// A double-click. The caller toggles that pickup and keeps one enabled.
    PickupToggled {
        index: usize,
    },
    /// A click on the body, away from every pickup. The caller shows Body type.
    BodyClicked,
}

pub struct PickupSprite<'a> {
    pub position_inches: f32,
    pub angle_degrees: f32,
    pub enabled: bool,
    pub selected: bool,
    pub image: &'a UiImage,
}

pub struct BodyCanvas<'a> {
    body: &'a UiImage,
    outline: Option<&'a UiImage>,
    pickups: &'a [PickupSprite<'a>],
    placement: PickupPlacement,
    interactive: bool,
}

impl<'a> BodyCanvas<'a> {
    pub fn new(body: &'a UiImage, pickups: &'a [PickupSprite<'a>]) -> Self {
        Self {
            body,
            outline: None,
            pickups,
            placement: PickupPlacement::default(),
            interactive: true,
        }
    }

    /// Draw a body-tab wireframe over the photo. A missing file paints its
    /// name, which is the #191 placeholder.
    pub fn outline(mut self, outline: Option<&'a UiImage>) -> Self {
        self.outline = outline;
        self
    }

    pub fn placement(mut self, placement: PickupPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> BodyOutput {
        let placement = self.placement;
        let art = placement.art.max(vec2(1.0, 1.0));
        let available = ui.available_size();
        let scale = (available.x / art.x)
            .min(available.y / art.y)
            .clamp(0.0, 64.0);
        let size = art * scale;
        let (stage, _) = ui.allocate_exact_size(
            vec2(available.x.max(size.x), size.y.max(1.0)),
            Sense::hover(),
        );
        let art_rect = Rect::from_center_size(stage.center(), size);
        paint_well(ui, stage);
        paint_image(ui, self.body, art_rect);
        if let Some(outline) = self.outline {
            paint_image(ui, outline, art_rect);
        }

        let paint_order = paint_order(self.pickups);
        let scale = placement.screen_scale(art_rect);
        let placed: Vec<SpriteHit> = paint_order
            .iter()
            .copied()
            .map(|index| placed_sprite(self.pickups, index, placement, art_rect, scale))
            .collect();
        for sprite in &placed {
            paint_pickup(
                ui,
                self.pickups[sprite.index].image,
                sprite,
                self.pickups[sprite.index].enabled,
            );
        }

        let id = ui.id().with("variax-body-canvas");
        let sense = if self.interactive {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let response = ui.interact(art_rect, id, sense);
        let mut events = Vec::new();
        if self.interactive {
            interact(
                ui,
                &Gesture {
                    response: &response,
                    id,
                    art_rect,
                    placed: &placed,
                    pickups: self.pickups,
                    placement,
                    scale,
                },
                &mut events,
            );
        } else if let Some(drag) = ui.data(|data| data.get_temp::<ActiveDrag>(id)) {
            ui.data_mut(|data| data.remove::<ActiveDrag>(id));
            events.push(BodyEvent::DragFinished {
                index: drag.index,
                kind: drag.kind,
            });
        }
        BodyOutput {
            response,
            events,
            art_rect,
        }
    }
}

pub struct BodyOutput {
    pub response: egui::Response,
    pub events: Vec<BodyEvent>,
    pub art_rect: Rect,
}

#[derive(Clone, Copy)]
struct ActiveDrag {
    index: usize,
    kind: DragKind,
    last: Pos2,
    inches: f32,
    degrees: f32,
    scale: f32,
    placement: PickupPlacement,
}

fn paint_order(pickups: &[PickupSprite<'_>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..pickups.len())
        .filter(|&index| {
            let pickup = &pickups[index];
            pickup.position_inches.is_finite() && pickup.angle_degrees.is_finite()
        })
        .collect();
    order.sort_by_key(|&index| pickups[index].selected);
    order
}

fn placed_sprite(
    pickups: &[PickupSprite<'_>],
    index: usize,
    placement: PickupPlacement,
    art_rect: Rect,
    scale: f32,
) -> SpriteHit {
    let pickup = &pickups[index];
    SpriteHit {
        index,
        center: placement.screen_point(art_rect, pickup.position_inches),
        sprite: sprite_art_size(pickup.image.pixels, placement.art) * scale,
        angle_degrees: pickup.angle_degrees,
        selected: pickup.selected,
    }
}

fn paint_well(ui: &egui::Ui, stage: Rect) {
    let painter = ui.painter();
    painter.rect_filled(stage, RADIUS_PANEL, BG);
    painter.rect_stroke(
        stage,
        RADIUS_PANEL,
        Stroke::new(1.0_f32, LINE),
        egui::StrokeKind::Inside,
    );
}

fn paint_image(ui: &egui::Ui, image: &UiImage, rect: Rect) {
    let Some(texture) = image.texture.as_ref().filter(|_| !image.missing) else {
        assets::paint(ui, image, rect);
        return;
    };
    ui.painter().image(
        texture.id(),
        rect,
        Rect::from_min_max(Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn paint_pickup(ui: &egui::Ui, image: &UiImage, sprite: &SpriteHit, enabled: bool) {
    let painter = ui.painter();
    let half = sprite.sprite * 0.5;
    let rotation = egui::emath::Rot2::from_angle(sprite.angle_degrees.to_radians());
    let corners = [
        vec2(-half.x, -half.y),
        vec2(half.x, -half.y),
        vec2(half.x, half.y),
        vec2(-half.x, half.y),
    ];
    if let Some(texture) = image.texture.as_ref().filter(|_| !image.missing) {
        let color = Color32::from_white_alpha(if enabled { 255 } else { 102 });
        let uvs = [
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
        ];
        let mut mesh = egui::Mesh::with_texture(texture.id());
        for (corner, uv) in corners.into_iter().zip(uvs) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: sprite.center + rotation * corner,
                uv,
                color,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    } else {
        assets::paint(
            ui,
            image,
            Rect::from_center_size(sprite.center, sprite.sprite),
        );
    }
    if sprite.selected {
        let stroke = Stroke::new(3.0_f32, AMBER);
        let points: Vec<Pos2> = corners
            .into_iter()
            .map(|corner| sprite.center + rotation * corner)
            .collect();
        for index in 0..4 {
            painter.line_segment([points[index], points[(index + 1) % 4]], stroke);
        }
    }
}

#[derive(Clone, Copy)]
struct Gesture<'a> {
    response: &'a egui::Response,
    id: egui::Id,
    art_rect: Rect,
    placed: &'a [SpriteHit],
    pickups: &'a [PickupSprite<'a>],
    placement: PickupPlacement,
    scale: f32,
}

fn interact(ui: &mut egui::Ui, gesture: &Gesture<'_>, events: &mut Vec<BodyEvent>) {
    let Gesture {
        response,
        id,
        art_rect,
        placed,
        pickups,
        placement,
        scale,
    } = *gesture;
    let fine = ui.input(|input| input.modifiers.ctrl);
    let pointer = response.interact_pointer_pos();
    let hover = pointer.and_then(|pos| hit_sprite(pos, placed));
    let dragging = ui.data(|data| data.get_temp::<ActiveDrag>(id));
    if let Some(kind) = dragging
        .map(|drag| drag.kind)
        .or(hover.map(|(_, zone)| match zone {
            PickupZone::Move => DragKind::Move,
            PickupZone::Rotate => DragKind::Rotate,
        }))
    {
        let cursor = match kind {
            DragKind::Move => CursorIcon::ResizeHorizontal,
            DragKind::Rotate => CursorIcon::ResizeVertical,
        };
        ui.ctx().set_cursor_icon(cursor);
    }
    if let Some((_, zone)) = hover {
        let _ = response.clone().on_hover_text(match zone {
            PickupZone::Move => {
                "Drag horizontally to move this pickup. Double-click to turn it on or off."
            }
            PickupZone::Rotate => {
                "Drag vertically to rotate this pickup. Ctrl drags in finer steps. \
                 Double-click to turn it on or off."
            }
        });
    }

    if let Some(mut drag) = dragging {
        if response.dragged()
            && let Some(now) = pointer
        {
            let delta = now - drag.last;
            drag.last = now;
            if let Some(event) = step_drag(&mut drag, delta, fine) {
                events.push(event);
            }
            ui.data_mut(|data| data.insert_temp(id, drag));
        }
        if response.drag_stopped() {
            ui.data_mut(|data| data.remove::<ActiveDrag>(id));
            events.push(BodyEvent::DragFinished {
                index: drag.index,
                kind: drag.kind,
            });
        }
        return;
    }

    if response.drag_started() {
        let origin = ui.input(|input| input.pointer.press_origin()).or(pointer);
        if let Some(origin) = origin
            && let Some((index, zone)) = hit_sprite(origin, placed)
            && let Some(now) = pointer
        {
            let kind = match zone {
                PickupZone::Move => DragKind::Move,
                PickupZone::Rotate => DragKind::Rotate,
            };
            let mut drag = ActiveDrag {
                index,
                kind,
                last: origin,
                inches: pickups[index].position_inches,
                degrees: pickups[index].angle_degrees,
                scale,
                placement,
            };
            events.push(BodyEvent::DragStarted { index, kind });
            if let Some(event) = step_drag(&mut drag, now - origin, fine) {
                events.push(event);
            }
            drag.last = now;
            ui.data_mut(|data| data.insert_temp(id, drag));
        }
        return;
    }

    // A double-click anywhere on this canvas is still one widget click. Only a
    // double-click that lands on a pickup toggles it. A second click on the
    // body, even inside the double-click window, is a body click.
    let pos = pointer.or(ui.input(|input| input.pointer.press_origin()));
    if response.double_clicked()
        && let Some(pos) = pos
        && let Some((index, _)) = hit_sprite(pos, placed)
    {
        events.push(BodyEvent::PickupToggled { index });
    } else if response.clicked()
        && let Some(pos) = pos
    {
        if let Some((index, _)) = hit_sprite(pos, placed) {
            events.push(BodyEvent::PickupClicked { index });
        } else if art_rect.contains(pos) {
            events.push(BodyEvent::BodyClicked);
        }
    }
}

fn step_drag(drag: &mut ActiveDrag, delta: Vec2, fine: bool) -> Option<BodyEvent> {
    match drag.kind {
        DragKind::Move => {
            let art_dx = if drag.scale.abs() <= f32::EPSILON {
                0.0
            } else {
                delta.x / drag.scale
            };
            let inches = move_inches(drag.inches, art_dx, drag.placement);
            if (inches - drag.inches).abs() <= 1.0e-4 {
                None
            } else {
                drag.inches = inches;
                Some(BodyEvent::Moved {
                    index: drag.index,
                    inches,
                })
            }
        }
        DragKind::Rotate => {
            let degrees = rotate_degrees(drag.degrees, delta.y, fine);
            if (degrees - drag.degrees).abs() <= 1.0e-4 {
                None
            } else {
                drag.degrees = degrees;
                Some(BodyEvent::Rotated {
                    index: drag.index,
                    degrees,
                })
            }
        }
    }
}
