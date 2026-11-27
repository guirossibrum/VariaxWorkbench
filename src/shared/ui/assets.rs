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

/// Artwork shipped with the executable, or the source tree for development.
pub fn workspace_root() -> PathBuf {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        #[cfg(target_os = "linux")]
        let package_artwork = linux_package_artwork;
        #[cfg(target_os = "macos")]
        let package_artwork = macos_bundle_artwork;

        static ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
        ROOT.get_or_init(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| package_artwork(&exe))
                .unwrap_or_else(source_artwork)
        })
        .clone()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    source_artwork()
}

fn source_artwork() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/ui")
}

#[cfg(any(target_os = "linux", test))]
fn linux_package_artwork(exe: &Path) -> Option<PathBuf> {
    let directory = exe.parent()?.join("assets/ui");
    directory.is_dir().then_some(directory)
}

#[cfg(any(target_os = "macos", test))]
fn macos_bundle_artwork(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?;
    let contents = bin.parent()?;
    let app = contents.parent()?;
    if bin.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app"
    {
        Some(contents.join("Resources/assets/ui"))
    } else {
        None
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_home_artwork_next_to_a_relocated_linux_executable() {
        let scratch = tempfile::tempdir().unwrap();
        let exe = scratch.path().join("variax-workbench-gui");
        assert!(linux_package_artwork(&exe).is_none());
        let artwork = scratch.path().join("assets/ui");
        std::fs::create_dir_all(artwork.join("shell")).unwrap();
        let photo = "shell/variax.png";
        std::fs::copy(source_artwork().join(photo), artwork.join(photo)).unwrap();

        assert_eq!(linux_package_artwork(&exe), Some(artwork.clone()));
        assert!(!load(&egui::Context::default(), &artwork, photo).missing);
    }

    #[test]
    fn loads_home_artwork_from_a_relocated_app_bundle() {
        let scratch = tempfile::tempdir().unwrap();
        let contents = scratch.path().join("Moved Variax Workbench.app/Contents");
        let exe = contents.join("MacOS/variax-workbench-gui");
        let resources = contents.join("Resources/assets/ui");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, []).unwrap();
        std::fs::create_dir_all(resources.join("shell")).unwrap();
        for name in [
            "shell/workbench-mark.png",
            "shell/variax.png",
            "shell/podxt-live.png",
        ] {
            std::fs::copy(source_artwork().join(name), resources.join(name)).unwrap();
        }

        let root = macos_bundle_artwork(&exe).unwrap();
        assert_eq!(root, resources);
        let ctx = egui::Context::default();
        for (name, size) in [
            ("shell/workbench-mark.png", [128, 128]),
            ("shell/variax.png", [64, 64]),
            ("shell/podxt-live.png", [64, 64]),
        ] {
            let artwork = load(&ctx, &root, name);
            assert!(!artwork.missing, "{name}");
            assert_eq!(artwork.pixels, size, "{name}");
            assert!(artwork.texture.is_some(), "{name}");
        }
        std::fs::remove_file(resources.join("shell/workbench-mark.png")).unwrap();
        assert_eq!(macos_bundle_artwork(&exe).unwrap(), resources);
        assert!(load(&ctx, &root, "shell/workbench-mark.png").missing);
    }

    #[test]
    fn standalone_development_executable_has_no_bundle_artwork() {
        for exe in [
            "/build/target/debug/variax-workbench-gui",
            "/build/target/Contents/MacOS/variax-workbench-gui",
            "/build/Moved Workbench.app/MacOS/variax-workbench-gui",
        ] {
            assert!(macos_bundle_artwork(Path::new(exe)).is_none(), "{exe}");
        }
    }
}
