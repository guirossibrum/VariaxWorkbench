use super::{BODY, EditorDocument, ModelArt, PICKUP1, PICKUP2, VariaxEditor, egui};
use line6_devices::family::variax::{BANKS, PRESETS_PER_BANK};
use std::path::{Path, PathBuf};
use variax_workbench::{
    bundle::BundleFile, edit::is_acoustic, preset::PresetFile, variax_library::LibraryPreferences,
};

const FIRST_MODEL_BANK: usize = 1;
const LAST_MODEL_BANK: usize = BANKS.len() - 2;

fn model_bank(document: &EditorDocument) -> Option<usize> {
    let (bank, _) = document.selection()?;
    // A full fixed Workbench bank file uses physical model knob order even
    // after its banks and presets are renamed. A collection has no such order.
    (document.extension() == "vwb"
        && (FIRST_MODEL_BANK..=LAST_MODEL_BANK).contains(&bank)
        && document.preset().is_legacy_electric()
        && document.banks().is_some_and(|banks| {
            banks.len() == BANKS.len()
                && banks
                    .iter()
                    .all(|bank| bank.fixed && bank.presets().len() == usize::from(PRESETS_PER_BANK))
        }))
    .then_some(bank)
}

type PositionLabels = Vec<Vec<String>>;

fn position_labels(bundle: &BundleFile, catalog: &[ModelArt]) -> PositionLabels {
    // The #240 factory bundle has already reversed the five flash slots
    // in each bank into physical selector order (bridge to neck).
    bundle.banks()[FIRST_MODEL_BANK..=LAST_MODEL_BANK]
        .iter()
        .map(|bank| {
            bank.presets()
                .iter()
                .map(|preset| model_label(preset, catalog))
                .collect()
        })
        .collect()
}

fn model_label(preset: &PresetFile, catalog: &[ModelArt]) -> String {
    let name = |id, kind, unknown| {
        catalog
            .iter()
            .find(|entry| entry.id == id && entry.kind == kind)
            .map_or(unknown, |entry| entry.name.as_str())
    };
    let body = preset.model(BODY);
    let body_name = body.map_or("Unknown body", |model| {
        name(model.id, "body", "Unknown body")
    });
    let mut label = body_name.to_owned();
    if !body.is_some_and(|model| is_acoustic(model.id)) {
        for pickup in [PICKUP1, PICKUP2]
            .into_iter()
            .filter_map(|slot| preset.model(slot).filter(|model| model.enabled))
        {
            label.push_str(" · ");
            label.push_str(name(pickup.id, "pickup", "Unknown pickup"));
        }
    }
    label
}

struct CachedArchive {
    path: PathBuf,
    bytes: Vec<u8>,
    labels: Option<PositionLabels>,
}

#[derive(Default)]
pub(super) struct FactorySelector {
    cache: Option<CachedArchive>,
}

impl FactorySelector {
    #[cfg(test)]
    fn seed(&mut self, archive: &Path, bundle: &BundleFile, catalog: &[ModelArt]) {
        self.cache = Some(CachedArchive {
            path: archive.to_owned(),
            bytes: super::startup_library::factory_archive_bytes(archive).unwrap(),
            labels: Some(position_labels(bundle, catalog)),
        });
    }

    fn positions(
        &mut self,
        preferences: &Path,
        bank: usize,
        read: impl FnOnce(&[u8]) -> Result<PositionLabels, String>,
    ) -> Option<&[String]> {
        // Metadata can stay identical after a same-size write. Compare the
        // bounded snapshot and derive labels from the same bytes we cache.
        let archive = LibraryPreferences::new(preferences)
            .factory_archive()
            .and_then(|path| {
                super::startup_library::factory_archive_bytes(&path)
                    .ok()
                    .map(|bytes| (path, bytes))
            });
        let Some((path, bytes)) = archive else {
            self.cache = None;
            return None;
        };
        if self
            .cache
            .as_ref()
            .is_none_or(|previous| previous.path != path || previous.bytes != bytes)
        {
            let labels = read(&bytes).ok();
            self.cache = Some(CachedArchive {
                path,
                bytes,
                labels,
            });
        }
        self.cache
            .as_ref()?
            .labels
            .as_ref()?
            .get(bank - FIRST_MODEL_BANK)
            .map(Vec::as_slice)
    }
}

impl VariaxEditor {
    #[cfg(test)]
    pub(super) fn seed_factory_selector(&mut self, archive: &Path, bundle: &BundleFile) {
        self.factory_selector.seed(archive, bundle, &self.catalog);
    }

    pub(super) fn factory_selector_reference(&mut self, ui: &mut egui::Ui) {
        let (Some(preferences), Some(bank)) = (
            self.preferences_path.as_deref(),
            self.document.as_ref().and_then(model_bank),
        ) else {
            self.factory_selector.cache = None;
            return;
        };
        let trigger = ui.button(format!("Factory selector reference · {}", BANKS[bank]));
        let popup = egui::Popup::from_toggle_button_response(&trigger)
            .gap(4.0)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
        let popup_id = popup.get_id();
        let dismiss_with_escape =
            popup.is_open() && ui.input(|input| input.key_pressed(egui::Key::Escape));
        popup.show(|ui| {
            ui.set_max_width(420.0);
            ui.style_mut().interaction.selectable_labels = false;
            let positions = self.factory_selector.positions(preferences, bank, |bytes| {
                super::startup_library::factory_bundle_from_bytes(bytes)
                    .map(|bundle| position_labels(&bundle, &self.catalog))
            });
            show_positions(ui, positions);
        });
        if trigger.clicked() || dismiss_with_escape {
            trigger.request_focus();
        }
        if !egui::Popup::is_id_open(ui.ctx(), popup_id) {
            // Check the fingerprint again every time the reference is opened.
            self.factory_selector.cache = None;
        }
    }
}

fn show_positions(ui: &mut egui::Ui, positions: Option<&[String]>) {
    let Some(positions) = positions else {
        ui.label("Choose an available, verified Variax 3.10 firmware file in Preferences.");
        return;
    };
    ui.label("Variax 3.10 factory settings · bridge to neck");
    for (index, label) in positions.iter().enumerate() {
        ui.label(format!("{} · {label}", index + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        load_catalog,
        startup_library::{factory_bundle_from_bytes, record_unchecked_archive},
    };
    use super::*;
    use crate::editor::FakeDialogs;
    use crate::editor::tests::app;
    use std::fs;

    #[test]
    fn factory_selector_covers_all_positions_and_uses_catalog_labels() {
        let catalog = load_catalog();
        let bundle = BundleFile::new_default_bundle().unwrap();
        let mut document = EditorDocument::from_bundle(bundle, PathBuf::new()).unwrap();
        let fixture_catalog: Vec<ModelArt> = catalog
            .into_iter()
            .filter(|entry| entry.kind == "pickup")
            .take(10)
            .collect();
        let mut seen = std::collections::HashSet::new();
        for (bank, primary) in (1..=10).zip(&fixture_catalog) {
            for (slot, secondary) in fixture_catalog.iter().take(5).enumerate() {
                document.select(bank, slot).unwrap();
                assert_eq!(model_bank(&document), Some(bank));
                document
                    .apply(|preset| {
                        preset.select_model(PICKUP1, primary.id)?;
                        preset.select_model(PICKUP2, secondary.id)?;
                        preset.set_enabled(PICKUP2, true)?;
                        preset.rename("User preset")?;
                        Ok(())
                    })
                    .unwrap();
            }
        }
        let factory = BundleFile::parse(document.as_bytes().to_vec()).unwrap();
        let labels = position_labels(&factory, &load_catalog());
        assert_eq!(labels.len(), 10);
        for (rows, primary) in labels.iter().zip(&fixture_catalog) {
            assert_eq!(rows.len(), 5);
            for (label, secondary) in rows.iter().zip(&fixture_catalog[..5]) {
                assert_eq!(
                    label,
                    &format!("Spank · {} · {}", primary.name, secondary.name)
                );
                assert!(seen.insert(label));
            }
        }
        assert_eq!(seen.len(), 50);
        assert!(document.select(12, 0).is_err());
        assert!(document.select(10, 5).is_err());
        assert_eq!(document.selection(), Some((10, 4)));

        for bank in [0, 11] {
            document.select(bank, 0).unwrap();
            assert_eq!(model_bank(&document), None);
        }
        let preset = document.preset().clone();
        let mut collection =
            EditorDocument::from_bundle(BundleFile::new_collection().unwrap(), PathBuf::new())
                .unwrap();
        collection.append_collection_model(preset.clone()).unwrap();
        assert_eq!(model_bank(&collection), None);
        assert_eq!(
            model_bank(&EditorDocument::from_preset(preset, PathBuf::new())),
            None
        );
    }

    #[test]
    fn factory_selector_omits_disabled_and_acoustic_pickups() {
        let catalog = load_catalog();
        let mut preset = PresetFile::default_legacy_model().unwrap();
        assert_eq!(
            model_label(&preset, &catalog),
            "Spank · Spank Single Coil Bridge"
        );
        let acoustic = catalog
            .iter()
            .find(|entry| entry.kind == "body" && is_acoustic(entry.id))
            .unwrap();
        preset.select_model(BODY, acoustic.id).unwrap();
        assert_eq!(model_label(&preset, &catalog), acoustic.name);
    }

    #[test]
    fn factory_selector_revalidates_missing_and_replaced_selection() {
        let temp = tempfile::tempdir().unwrap();
        let preferences = temp.path().join("editor-preferences");
        let archive = temp.path().join("picked.vxf");
        let load = |bytes: &[u8]| {
            factory_bundle_from_bytes(bytes).map(|bundle| position_labels(&bundle, &load_catalog()))
        };
        let mut selector = FactorySelector::default();
        assert!(
            selector
                .positions(&preferences, 1, |_| panic!("No archive was selected"))
                .is_none()
        );

        fs::write(&archive, b"synthetic accepted fixture").unwrap();
        record_unchecked_archive(&preferences, &archive);
        let catalog = load_catalog();
        let bundle = BundleFile::new_default_bundle().unwrap();
        let synthetic_load = |bytes: &[u8]| {
            assert_eq!(bytes, b"synthetic accepted fixture");
            Ok(position_labels(&bundle, &catalog))
        };
        assert_eq!(
            selector
                .positions(&preferences, 1, synthetic_load)
                .unwrap()
                .len(),
            5
        );
        assert!(
            selector
                .positions(&preferences, 10, |_| panic!(
                    "Reuse verified labels for this file"
                ))
                .is_some()
        );

        let (ctx, mut app) = app();
        app.variax_mut().preferences_path = Some(preferences.clone());
        let invalid_pick = temp.path().join("invalid.vxf");
        fs::write(&invalid_pick, b"invalid selection").unwrap();
        app.dialogs = Box::new(FakeDialogs {
            open: [Some(invalid_pick)].into(),
            ..Default::default()
        });
        app.test_action(|editor: &mut VariaxEditor, services| {
            editor.choose_factory_archive(&ctx, services)
        });
        app.poll_picker(&ctx);
        assert_eq!(
            LibraryPreferences::new(&preferences).factory_archive(),
            Some(archive.clone())
        );

        fs::remove_file(&archive).unwrap();
        assert!(
            selector
                .positions(&preferences, 1, |_| panic!("Selected archive is missing"))
                .is_none()
        );
        fs::write(&archive, b"invalid archive content     ").unwrap();
        assert!(selector.positions(&preferences, 1, load).is_none());
        fs::write(&archive, b"synthetic accepted fixture").unwrap();
        assert!(
            selector
                .positions(&preferences, 1, synthetic_load)
                .is_some()
        );
        let old_stamp = fs::metadata(&archive).unwrap();
        fs::write(&archive, b"invalid archive content   ").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&archive)
            .unwrap()
            .set_modified(old_stamp.modified().unwrap())
            .unwrap();
        assert_eq!(fs::metadata(&archive).unwrap().len(), old_stamp.len());
        assert!(selector.positions(&preferences, 1, load).is_none());
        assert_eq!(fs::read(&archive).unwrap(), b"invalid archive content   ");

        let replacement = temp.path().join("replacement.vxf");
        fs::write(&replacement, b"synthetic accepted fixture").unwrap();
        record_unchecked_archive(&preferences, &replacement);
        assert!(
            selector
                .positions(&preferences, 1, synthetic_load)
                .is_some()
        );
    }
}
