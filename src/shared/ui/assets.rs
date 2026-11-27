//! Load committed UI art from `assets/ui` or from the executable.

mod bundled_variax;

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use eframe::egui::{self, Color32, ColorImage, Rect, TextureHandle, TextureOptions, Vec2, vec2};
use image::{ImageError, ImageFormat};

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

/// Load `relative` from `root`, falling back to embedded shell art for missing files.
/// `..` and absolute paths are treated as missing.
pub fn load(ctx: &egui::Context, root: &Path, relative: &str) -> UiImage {
    let Some(path) = contained(root, relative) else {
        return UiImage::missing(relative);
    };
    let image = match image::open(&path) {
        Ok(image) => Some(image),
        Err(ImageError::IoError(error)) if error.kind() == ErrorKind::NotFound => {
            embedded_shell_png(relative)
                .and_then(|png| image::load_from_memory_with_format(png, ImageFormat::Png).ok())
        }
        Err(_) => None,
    };
    let Some(image) = image else {
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

/// Decode a Variax editor image. An optional local image takes precedence
/// over the embedded Workbench artwork. Pickup hit testing uses the
/// unmultiplied alpha from the returned pixels.
pub fn load_variax_rgba(root: &Path, relative: &str) -> Option<image::RgbaImage> {
    let path = contained(root, relative)?;
    let decoded = image::open(path).ok().or_else(|| {
        let png = bundled_variax::png(relative)?;
        image::load_from_memory_with_format(png, ImageFormat::Png).ok()
    })?;
    Some(decoded.into_rgba8())
}

fn embedded_shell_png(relative: &str) -> Option<&'static [u8]> {
    match relative {
        "shell/workbench-mark.png" => Some(include_bytes!(
            "../../../assets/ui/shell/workbench-mark.png"
        )),
        "shell/variax.png" => Some(include_bytes!("../../../assets/ui/shell/variax.png")),
        "shell/podxt-live.png" => Some(include_bytes!("../../../assets/ui/shell/podxt-live.png")),
        _ => None,
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
        paint_placeholder(painter, &image.name, bounds, bounds);
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

pub(super) fn paint_placeholder(
    painter: &egui::Painter,
    name: &str,
    bounds: Rect,
    label_bounds: Rect,
) {
    painter.rect_filled(bounds, RADIUS_CONTROL, RAISED);
    painter.rect_stroke(
        bounds,
        RADIUS_CONTROL,
        egui::Stroke::new(1.0_f32, LINE),
        egui::StrokeKind::Inside,
    );
    let inset = label_bounds.shrink(1.0);
    if !inset.is_positive() {
        return;
    }
    let font = egui::FontId::proportional(COMPACT);
    let mut galley = painter.layout_no_wrap(name.to_owned(), font.clone(), DIM);
    if galley.size().x > inset.width() {
        let mut job = egui::text::LayoutJob::simple(name.to_owned(), font, DIM, inset.width());
        job.halign = egui::Align::Center;
        galley = painter.layout_job(job);
    }
    let origin = label_bounds.center() - galley.rect.center().to_vec2();
    if label_bounds.contains_rect(galley.rect.translate(origin.to_vec2()))
        && inset.contains_rect(galley.mesh_bounds.translate(origin.to_vec2()))
    {
        painter
            .with_clip_rect(label_bounds)
            .galley(origin, galley, DIM);
    }
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
    use sha2::{Digest, Sha256};

    #[test]
    fn loads_exact_approved_shell_art_without_a_source_or_asset_directory() {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("absent-source");
        let root = source.join("assets/ui");
        assert!(!source.exists());
        let ctx = egui::Context::default();
        // SHA-256 values from assets/ui/SOURCES.md.
        for (name, size, digest) in [
            (
                "shell/workbench-mark.png",
                [128, 128],
                "c6605b76df17861cbff74eae4b446612218d7d2aed4ad18b7ef12f0120ee30c2",
            ),
            (
                "shell/variax.png",
                [64, 64],
                "e282971a7c40ca0ed8b0ed5e62d6cdaf8b36c49825cac59dba7f390b2fbe779e",
            ),
            (
                "shell/podxt-live.png",
                [64, 64],
                "223f8d8a9b02c42aaa1542e06283f63a646bf3e126537dcfd0e0bb35bf32ef48",
            ),
        ] {
            let artwork = load(&ctx, &root, name);
            assert_eq!(artwork.name, name);
            assert!(!artwork.missing, "{name}");
            assert_eq!(artwork.pixels, size, "{name}");
            assert!(artwork.texture.is_some(), "{name}");
            let png = embedded_shell_png(name).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(png)), digest, "{name}");
        }
        assert!(!root.exists());
    }

    #[test]
    fn bundled_variax_art_resolves_every_key_without_a_source_tree() {
        use std::collections::{BTreeMap, BTreeSet};

        let install = tempfile::tempdir().unwrap();
        let absent_override = install.path().join("user data/artwork");
        let provenance: BTreeMap<_, _> = include_str!("../../../assets/ui/SOURCES.md")
            .lines()
            .filter_map(|line| {
                let mut columns = line.strip_prefix("| `")?.split(" | ");
                let file = columns.next()?.strip_suffix('`')?;
                let digest = columns.last()?.strip_prefix('`')?.strip_suffix("` |")?;
                Some((file, digest))
            })
            .collect();
        let mut keys = BTreeSet::new();
        for &(key, file) in bundled_variax::KEYS {
            assert!(keys.insert(key), "duplicate {key}");
            let png = bundled_variax::png(key).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(png)),
                provenance[file],
                "{key}"
            );
            let rgba = load_variax_rgba(&absent_override, key).expect(key);
            let size = if key.starts_with("bodies/buttons/") {
                (42, 50)
            } else if key.starts_with("bodies/wireframe/") {
                (238, 148)
            } else if key.starts_with("bodies/") {
                (426, 412)
            } else if key == "pickups/chromep90.png" {
                (56, 130)
            } else {
                assert!(key.starts_with("pickups/"), "{key}");
                (120, 120)
            };
            assert_eq!(rgba.dimensions(), size, "{key}");
            if key.starts_with("pickups/") {
                assert!(rgba.pixels().any(|pixel| pixel[3] == 0), "{key}");
                assert!(rgba.pixels().any(|pixel| pixel[3] == 255), "{key}");
            }
        }

        let catalog: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../../assets/model-ui.json")).unwrap();
        let mut required = BTreeSet::new();
        for model in &catalog {
            required.insert(model["image"].as_str().unwrap());
            if model["kind"] == "body" {
                let id = u32::try_from(model["id"].as_u64().unwrap()).unwrap();
                required.insert(super::super::body_canvas::wireframe_path(id));
            }
        }
        assert_eq!(
            keys.iter()
                .copied()
                .filter(|key| !key.starts_with("bodies/buttons/"))
                .collect::<BTreeSet<_>>(),
            required
        );
        assert_eq!(keys.len(), 88);
        for key in [
            "bodies/telecustom.png",
            "bodies/tele.png",
            "bodies/lester.png",
            "bodies/ricky.png",
            "bodies/acoustic.png",
            "pickups/telebridge.png",
            "pickups/chromep90.png",
            "bodies/buttons/tele.png",
            "bodies/wireframe/tele.png",
        ] {
            assert!(keys.contains(key), "{key}");
        }
        assert!(!absent_override.exists());
    }

    #[test]
    fn variax_art_prefers_a_valid_override_and_checks_the_runtime_path() {
        let install = tempfile::tempdir().unwrap();
        let root = install.path().join("artwork");
        let key = "bodies/telecustom.png";
        let path = root.join(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let custom = image::RgbaImage::from_pixel(2, 3, image::Rgba([20, 40, 60, 128]));
        custom.save(&path).unwrap();
        assert_eq!(load_variax_rgba(&root, key), Some(custom));
        std::fs::write(&path, b"invalid PNG").unwrap();
        assert_eq!(
            load_variax_rgba(&root, key).unwrap().dimensions(),
            (426, 412)
        );

        for key in [
            "bodies/missing.png",
            "../artwork/bodies/telecustom.png",
            "bodies/../bodies/telecustom.png",
            "/bodies/telecustom.png",
            "",
        ] {
            assert!(load_variax_rgba(&root, key).is_none(), "{key}");
        }
    }

    #[test]
    fn unavailable_editor_art_and_invalid_shell_paths_stay_missing() {
        let scratch = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        for name in [
            "pod-farm/sine-chorus.png",
            "bodies/wireframe/strat.png",
            "shell/unlisted.png",
            "elsewhere/variax.png",
            "../shell/variax.png",
            "shell/../shell/variax.png",
            "/shell/variax.png",
            "",
        ] {
            let artwork = load(&ctx, scratch.path(), name);
            assert!(artwork.missing, "{name}");
            assert!(artwork.texture.is_none(), "{name}");
        }
    }

    #[test]
    fn loads_home_artwork_next_to_a_relocated_linux_executable() {
        let scratch = tempfile::tempdir().unwrap();
        let exe = scratch.path().join("variax-workbench-gui");
        assert!(linux_package_artwork(&exe).is_none());
        let artwork = scratch.path().join("assets/ui");
        std::fs::create_dir_all(artwork.join("shell")).unwrap();
        let photo = "shell/variax.png";
        image::RgbaImage::new(2, 3)
            .save(artwork.join(photo))
            .unwrap();

        assert_eq!(linux_package_artwork(&exe), Some(artwork.clone()));
        let ctx = egui::Context::default();
        assert_eq!(load(&ctx, &artwork, photo).pixels, [2, 3]);
        std::fs::write(artwork.join(photo), b"invalid PNG").unwrap();
        assert!(load(&ctx, &artwork, photo).missing);
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
        let root = macos_bundle_artwork(&exe).unwrap();
        assert_eq!(root, resources);
        let ctx = egui::Context::default();
        for name in [
            "shell/workbench-mark.png",
            "shell/variax.png",
            "shell/podxt-live.png",
        ] {
            image::RgbaImage::new(3, 2).save(root.join(name)).unwrap();
            let artwork = load(&ctx, &root, name);
            assert!(!artwork.missing, "{name}");
            assert_eq!(artwork.pixels, [3, 2], "{name}");
            assert!(artwork.texture.is_some(), "{name}");
        }
        std::fs::remove_file(resources.join("shell/workbench-mark.png")).unwrap();
        assert_eq!(macos_bundle_artwork(&exe).unwrap(), resources);
        assert_eq!(
            load(&ctx, &root, "shell/workbench-mark.png").pixels,
            [128, 128]
        );
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
