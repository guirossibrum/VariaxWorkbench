//! Variax offline editor state and document history.
use crate::{
    bundle::{Bank, BundleFile},
    preset::PresetFile,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub use crate::shared::document::save_new;

#[derive(Clone)]
struct SelectedBundle {
    file: BundleFile,
    bank: usize,
    preset: usize,
}

#[derive(Clone)]
struct DocumentState {
    preset: PresetFile,
    bundle: Option<SelectedBundle>,
}
impl DocumentState {
    fn bytes(&self) -> &[u8] {
        self.bundle
            .as_ref()
            .map_or_else(|| self.preset.as_bytes(), |b| b.file.as_bytes())
    }

    fn slot_bytes(&self, bank: usize, preset: usize) -> Option<&[u8]> {
        self.bundle
            .as_ref()?
            .file
            .banks()
            .get(bank)?
            .presets()
            .get(preset)
            .map(PresetFile::as_bytes)
    }
}

pub struct EditorDocument {
    state: DocumentState,
    undo: Vec<DocumentState>,
    redo: Vec<DocumentState>,
    saved: DocumentState,
    group_base: Option<DocumentState>,
    pub source: PathBuf,
}

impl EditorDocument {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_file_bytes(bytes, path)
    }

    pub fn from_file_bytes(bytes: Vec<u8>, path: &Path) -> Result<Self, String> {
        if matches!(bytes.get(8..12), Some(b"L6BA" | b"L6CO")) {
            let file = BundleFile::parse(bytes)?;
            if file
                .banks()
                .iter()
                .flat_map(|bank| bank.presets())
                .any(PresetFile::is_podxt_live_target)
            {
                return Err(
                    "this bank contains a PODxt Live tone; the guitar editor does not open it"
                        .into(),
                );
            }
            Self::from_bundle(file, path.to_owned())
        } else {
            let preset = PresetFile::parse(bytes)?;
            if preset.is_podxt_live_target() {
                return Err(
                    "this file is a PODxt Live tone, not a Variax preset. Use the offline POD librarian"
                        .into(),
                );
            }
            Ok(Self::from_preset(preset, path.to_owned()))
        }
    }

    pub fn from_preset(preset: PresetFile, source: PathBuf) -> Self {
        Self::from_state(
            DocumentState {
                preset,
                bundle: None,
            },
            source,
        )
    }

    pub fn from_bundle(file: BundleFile, source: PathBuf) -> Result<Self, String> {
        let (bank, preset) = file
            .banks()
            .iter()
            .enumerate()
            .find_map(|(i, b)| b.presets().first().map(|p| (i, p.clone())))
            .ok_or("bank/collection contains no presets to edit; file is retained unchanged")?;
        Ok(Self::from_state(
            DocumentState {
                preset,
                bundle: Some(SelectedBundle {
                    file,
                    bank,
                    preset: 0,
                }),
            },
            source,
        ))
    }

    fn from_state(state: DocumentState, source: PathBuf) -> Self {
        Self {
            saved: state.clone(),
            state,
            undo: vec![],
            redo: vec![],
            group_base: None,
            source,
        }
    }

    pub fn preset(&self) -> &PresetFile {
        &self.state.preset
    }
    pub fn as_bytes(&self) -> &[u8] {
        self.state.bytes()
    }
    pub fn banks(&self) -> Option<&[Bank]> {
        self.state.bundle.as_ref().map(|b| b.file.banks())
    }
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.state.bundle.as_ref().map(|b| (b.bank, b.preset))
    }
    pub fn extension(&self) -> &'static str {
        match &self.state.bundle {
            None => "vwm",
            Some(b) if b.file.fixed => "vwb",
            Some(_) => "l6c",
        }
    }

    /// Browsing does not edit the file or clear redo. An invalid selection is
    /// rejected before even ending an active gesture.
    pub fn select(&mut self, bank: usize, preset: usize) -> Result<(), String> {
        let bundle = self
            .state
            .bundle
            .as_ref()
            .ok_or("document is an individual preset")?;
        let next = bundle
            .file
            .banks()
            .get(bank)
            .and_then(|b| b.presets().get(preset))
            .ok_or("bank/preset selection is out of range")?
            .clone();
        self.end_gesture();
        let bundle = self.state.bundle.as_mut().unwrap();
        bundle.bank = bank;
        bundle.preset = preset;
        self.state.preset = next;
        Ok(())
    }

    /// The current standalone model inside an open bank or collection.
    pub fn selected_slot_model(&self) -> Result<&PresetFile, String> {
        self.selection()
            .ok_or("Open a bank or collection and select a model slot.")?;
        validate_slot_model(self.preset())?;
        Ok(self.preset())
    }

    pub fn validate_slot_replacement(&self, replacement: &PresetFile) -> Result<(), String> {
        self.selected_slot_model()?;
        validate_slot_model(replacement)
    }

    /// Replace the selected model in one undo step, retaining the source bytes.
    pub fn replace_slot_model(&mut self, replacement: PresetFile) -> Result<(), String> {
        self.validate_slot_replacement(&replacement)?;
        self.end_gesture();
        self.apply(|preset| {
            *preset = replacement;
            Ok(())
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.as_bytes() != self.saved.bytes()
    }

    /// Compare one bundle slot with the last opened or successfully saved file.
    pub fn slot_is_dirty(&self, bank: usize, preset: usize) -> bool {
        self.state.slot_bytes(bank, preset) != self.saved.slot_bytes(bank, preset)
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn apply(
        &mut self,
        change: impl FnOnce(&mut PresetFile) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut next = self.state.clone();
        change(&mut next.preset)?;
        if next.preset.as_bytes() != self.state.preset.as_bytes() {
            if let Some(bundle) = &mut next.bundle {
                bundle
                    .file
                    .replace(bundle.bank, bundle.preset, &next.preset)?;
            }
            let previous = std::mem::replace(&mut self.state, next);
            if self.group_base.is_none() {
                self.undo.push(previous);
                self.redo.clear();
            }
        }
        Ok(())
    }

    pub fn undo(&mut self) {
        self.end_gesture();
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, previous));
        }
    }
    pub fn redo(&mut self) {
        self.end_gesture();
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, next));
        }
    }
    pub fn save_copy(&mut self, path: &Path) -> Result<(), String> {
        self.end_gesture();
        save_new(path, self.as_bytes())?;
        self.saved = self.state.clone();
        Ok(())
    }
    pub fn begin_gesture(&mut self) {
        if self.group_base.is_none() {
            self.group_base = Some(self.state.clone());
        }
    }
    pub fn end_gesture(&mut self) {
        if let Some(base) = self.group_base.take()
            && base.bytes() != self.as_bytes()
        {
            self.undo.push(base);
            self.redo.clear();
        }
    }
}

fn validate_slot_model(model: &PresetFile) -> Result<(), String> {
    if model.is_podxt_live_target() {
        return Err("A PODxt Live tone cannot be placed in a Variax model slot.".into());
    }
    // Workbench's L6ToneCompatibilityFilter treats these legacy electric IDs
    // as data-compatible. A fixed bundle replacement keeps the model's target ID.
    if !model.is_legacy_electric() {
        return Err("The model must target a Variax 300/600 or 500/700.".into());
    }
    if model.as_bytes().get(8..12) != Some(b"L6PA") {
        return Err("Choose a standalone Workbench model (.vwm).".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_bytes(device: u32, form: &[u8; 4]) -> Vec<u8> {
        let mut bytes = include_bytes!("../../../../tests/fixtures/editor-preset.vwm").to_vec();
        bytes[8..12].copy_from_slice(form);
        bytes[24..28].copy_from_slice(&device.to_be_bytes());
        bytes
    }

    #[test]
    fn model_slot_replacement_supports_fixed_banks_and_collections() {
        let replacement = PresetFile::parse(model_bytes(0x0007_0000, b"L6PA")).unwrap();
        for (path, bytes) in [
            (
                Path::new("bank.vwb"),
                include_bytes!("../../../../tests/fixtures/synthetic-bank.vwb").as_slice(),
            ),
            (
                Path::new("collection.l6c"),
                include_bytes!("../../../../tests/fixtures/synthetic-collection.l6c").as_slice(),
            ),
        ] {
            let mut document = EditorDocument::from_file_bytes(bytes.to_vec(), path).unwrap();
            let selected = document.selection().unwrap();
            assert_eq!(
                document.selected_slot_model().unwrap().device_id(),
                0x0007_0004
            );
            document.replace_slot_model(replacement.clone()).unwrap();
            assert!(document.slot_is_dirty(selected.0, selected.1));
            let current = document.as_bytes().to_vec();
            let reopened = BundleFile::parse(current.clone()).unwrap();
            assert_eq!(
                reopened.banks()[selected.0].presets()[selected.1].as_bytes(),
                replacement.as_bytes()
            );
            document.undo();
            assert_eq!(document.as_bytes(), bytes);
            assert!(!document.can_undo());
            document.redo();
            assert_eq!(document.as_bytes(), current);
        }
    }

    #[test]
    fn model_slot_validation_preserves_the_document_on_rejection_and_identical_replacement() {
        let bytes = include_bytes!("../../../../tests/fixtures/synthetic-bank.vwb");
        let mut document =
            EditorDocument::from_file_bytes(bytes.to_vec(), Path::new("bank.vwb")).unwrap();
        for (target, form, error) in [
            (0x0003_000a, b"L6PA", "PODxt Live"),
            (0x0007_0001, b"L6PA", "300/600 or 500/700"),
            (0x0007_0005, b"L6PA", "300/600 or 500/700"),
            (0x0007_0004, b"L6FS", "standalone Workbench model"),
            (0x0007_0004, b"L6AS", "standalone Workbench model"),
        ] {
            let replacement = PresetFile::parse(model_bytes(target, form)).unwrap();
            let failure = document.replace_slot_model(replacement).unwrap_err();
            assert!(failure.contains(error), "{failure}");
        }
        document
            .replace_slot_model(document.preset().clone())
            .unwrap();
        assert_eq!(document.as_bytes(), bytes);
        assert!(!document.can_undo());
        assert!(!document.is_dirty());

        let model = PresetFile::parse(model_bytes(0x0007_0004, b"L6PA")).unwrap();
        let mut single = EditorDocument::from_preset(model.clone(), PathBuf::from("single.vwm"));
        assert!(single.selected_slot_model().is_err());
        assert!(single.replace_slot_model(model).is_err());
        assert!(!single.can_undo());
    }

    #[test]
    fn guitar_document_rejects_a_podxt_live_tone_and_a_bank_that_contains_one() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pod.vwm");
        let mut bytes = include_bytes!("../../../../tests/fixtures/editor-preset.vwm").to_vec();
        bytes[24..28].copy_from_slice(&0x0003_000au32.to_be_bytes());
        std::fs::write(&path, &bytes).unwrap();
        match EditorDocument::open(&path) {
            Err(error) => assert!(error.contains("PODxt Live"), "{error}"),
            Ok(_) => panic!("opened a PODxt Live tone as a guitar document"),
        }
        let bank_path = directory.path().join("pod.vwb");
        let mut bank = include_bytes!("../../../../tests/fixtures/synthetic-bank.vwb").to_vec();
        let at = bank
            .windows(4)
            .position(|item| item == 0x0007_0004u32.to_be_bytes())
            .expect("bank fixture has a Variax 300/600 id");
        bank[at..at + 4].copy_from_slice(&0x0003_000au32.to_be_bytes());
        std::fs::write(&bank_path, bank).unwrap();
        match EditorDocument::open(&bank_path) {
            Err(error) => assert!(error.contains("PODxt Live"), "{error}"),
            Ok(_) => panic!("opened a bank that contains a PODxt Live tone"),
        }
    }
}
