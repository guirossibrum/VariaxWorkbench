//! Load committed UI art from `assets/ui`. A missing file draws a placeholder.

use std::path::{Component, Path, PathBuf};

use eframe::egui::{self, Color32, ColorImage, Rect, TextureHandle, TextureOptions, Vec2, vec2};

use super::theme::{COMPACT, DIM, LINE, RADIUS_CONTROL, RAISED};

/// A decoded image, or a named placeholder when the file is absent.
pub struct UiImage {
    pub name: String,
    pub pixels: [usize; 2],
    pub texture: Option<TextureHandle>,
    pub missing: bool,
}

impl UiImage {
    pub fn missing(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pixels: [0, 0],
            texture: None,
            missing: true,
        }
    }
}

/// `CARGO_MANIFEST_DIR/assets/ui`. Present after the design art is on this branch.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ui")
}

/// Load `relative` from `root`. `..` and absolute paths are treated as missing.
pub fn load(ctx: &egui::Context, root: &Path, relative: &str) -> UiImage {
    let Some(path) = contained(root, relative) else {
        return UiImage::missing(relative);
    };
    let Ok(image) = image::open(&path) else {
        return UiImage::missing(relative);
    };
    let rgba = image.to_rgba8();
    let pixels = [rgba.width() as usize, rgba.height() as usize];
    if pixels[0] == 0 || pixels[1] == 0 {
        return UiImage::missing(relative);
    }
    let color = ColorImage::from_rgba_unmultiplied(pixels, rgba.as_raw());
    let texture = ctx.load_texture(relative, color, TextureOptions::LINEAR);
    UiImage {
        name: relative.to_owned(),
        pixels,
        texture: Some(texture),
        missing: false,
    }
}

fn contained(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() {
        return None;
    }
    let relative_path = Path::new(relative);
    let escapes = relative_path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if escapes {
        return None;
    }
    Some(root.join(relative_path))
}

/// Draw the image inside `bounds`, keeping its aspect. A missing image is a
/// raised plate with its path, so a gap in `assets/ui` does not panic.
pub fn paint(ui: &egui::Ui, image: &UiImage, bounds: Rect) {
    paint_tinted(ui, image, bounds, Color32::WHITE);
}

/// Like [`paint`], with a tint. A bypassed chain block passes a grey tint.
pub fn paint_tinted(ui: &egui::Ui, image: &UiImage, bounds: Rect, tint: Color32) {
    let painter = ui.painter();
    let Some(texture) = image.texture.as_ref().filter(|_| !image.missing) else {
        painter.rect_filled(bounds, RADIUS_CONTROL, RAISED);
        painter.rect_stroke(
            bounds,
            RADIUS_CONTROL,
            egui::Stroke::new(1.0_f32, LINE),
            egui::StrokeKind::Inside,
        );
        painter.text(
            bounds.center(),
            egui::Align2::CENTER_CENTER,
            image.name.as_str(),
            egui::FontId::proportional(COMPACT),
            DIM,
        );
        return;
    };
    let fitted = fitted_size(image.pixels, bounds.size());
    let rect = Rect::from_center_size(bounds.center(), fitted);
    painter.image(
        texture.id(),
        rect,
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        tint,
    );
}

pub fn fitted_size(pixels: [usize; 2], bounds: Vec2) -> Vec2 {
    if pixels[0] == 0 || pixels[1] == 0 || bounds.x <= 0.0 || bounds.y <= 0.0 {
        return vec2(0.0, 0.0);
    }
    let aspect = pixels[0] as f32 / pixels[1] as f32;
    let bounds_aspect = bounds.x / bounds.y;
    if aspect > bounds_aspect {
        vec2(bounds.x, bounds.x / aspect)
    } else {
        vec2(bounds.y * aspect, bounds.y)
    }
}
