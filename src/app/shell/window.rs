//! Geometry preferences for the native Workbench window.

use eframe::egui;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Geometry {
    size: egui::Vec2,
    position: Option<egui::Pos2>,
}

impl Geometry {
    fn parse(text: &str) -> Option<Self> {
        let mut values = text.split_whitespace();
        if values.next()? != "1" {
            return None;
        }
        let width = values.next()?.parse().ok()?;
        let height = values.next()?.parse().ok()?;
        let position = match values.next() {
            None => None,
            Some(x) => Some(egui::pos2(x.parse().ok()?, values.next()?.parse().ok()?)),
        };
        if values.next().is_some() {
            return None;
        }
        let size = egui::vec2(width, height);
        let valid_size = (1.0..=32767.0).contains(&size.x) && (1.0..=32767.0).contains(&size.y);
        if !valid_size || position.is_some_and(|point| !point.is_finite()) {
            return None;
        }
        Some(Self { size, position })
    }

    fn from_viewport(info: &egui::ViewportInfo, viewport_size: egui::Vec2) -> Option<Self> {
        if [info.minimized, info.maximized, info.fullscreen].contains(&Some(true)) {
            return None;
        }
        // Wayland may not report window coordinates, even for the inner rect.
        let size = info.inner_rect.map_or(viewport_size, |rect| rect.size());
        if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
            return None;
        }
        let position = info
            .outer_rect
            .map(|rect| rect.min)
            .filter(|point| point.is_finite());
        Some(Self { size, position })
    }
}

pub(super) struct WindowPreferences {
    path: PathBuf,
    saved: Option<Geometry>,
    latest: Option<Geometry>,
}

impl WindowPreferences {
    pub(super) fn load(path: PathBuf) -> Self {
        let saved = fs::read_to_string(&path)
            .ok()
            .and_then(|text| Geometry::parse(&text));
        Self {
            path,
            saved,
            latest: None,
        }
    }

    pub(super) fn apply(&self, mut viewport: egui::ViewportBuilder) -> egui::ViewportBuilder {
        if let Some(geometry) = self.saved {
            viewport = viewport.with_inner_size(geometry.size);
            if let Some(position) = geometry.position {
                viewport = viewport.with_position(position);
            }
        }
        viewport
    }

    pub(super) fn observe(&mut self, info: &egui::ViewportInfo, viewport_size: egui::Vec2) {
        if let Some(current) = Geometry::from_viewport(info, viewport_size) {
            self.latest = Some(current);
        }
    }

    pub(super) fn save(&self) -> std::io::Result<()> {
        let Some(geometry) = self.latest.filter(|latest| Some(*latest) != self.saved) else {
            return Ok(());
        };
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        write!(temporary, "1 {} {}", geometry.size.x, geometry.size.y)?;
        if let Some(position) = geometry.position {
            write!(temporary, " {} {}", position.x, position.y)?;
        }
        writeln!(temporary)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restores_window_geometry_and_atomic_replacement_uses_last_normal_frame() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings").join("window.txt");
        let mut prefs = WindowPreferences::load(path.clone());
        let size = egui::vec2(1280.0, 800.0);
        let origin = egui::pos2(-120.0, 50.0);
        prefs.observe(
            &egui::ViewportInfo {
                outer_rect: Some(egui::Rect::from_min_size(origin, size)),
                ..Default::default()
            },
            size,
        );
        prefs.save().unwrap();

        let mut prefs = WindowPreferences::load(path.clone());
        let initial = prefs.apply(egui::ViewportBuilder::default());
        assert_eq!(initial.inner_size, Some(size));
        assert_eq!(initial.position, Some(origin));

        let next_size = egui::vec2(1300.0, 820.0);
        prefs.observe(&egui::ViewportInfo::default(), next_size);
        prefs.observe(
            &egui::ViewportInfo {
                maximized: Some(true),
                ..Default::default()
            },
            egui::vec2(3840.0, 2160.0),
        );
        prefs.save().unwrap();
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        let next = WindowPreferences::load(path).apply(egui::ViewportBuilder::default());
        assert_eq!(next.inner_size, Some(next_size));
        assert_eq!(next.position, None);
    }

    #[test]
    fn malformed_geometry_uses_startup_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("window.txt");
        fs::write(&path, "1 NaN 900 10 20").unwrap();
        let initial = WindowPreferences::load(path)
            .apply(egui::ViewportBuilder::default().with_inner_size([1120.0, 760.0]));
        assert_eq!(initial.inner_size, Some(egui::vec2(1120.0, 760.0)));
        assert_eq!(initial.position, None);
    }
}
