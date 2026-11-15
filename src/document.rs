//! Shared offline editor state and atomic creation of new preset files.
use crate::{
    bundle::{Bank, BundleFile},
    preset::PresetFile,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn save_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(bytes).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary
        .persist_noclobber(path)
        .map_err(|e| format!("{}: {}", path.display(), e.error))?;
    Ok(())
}

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
}

pub struct EditorDocument {
    state: DocumentState,
    undo: Vec<DocumentState>,
    redo: Vec<DocumentState>,
    saved: Vec<u8>,
    group_base: Option<DocumentState>,
    pub source: PathBuf,
}

impl EditorDocument {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if matches!(bytes.get(8..12), Some(b"L6BA" | b"L6CO")) {
            Self::from_bundle(BundleFile::parse(bytes)?, path.to_owned())
        } else {
            Ok(Self::from_preset(
                PresetFile::parse(bytes)?,
                path.to_owned(),
            ))
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
            saved: state.bytes().to_vec(),
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

    pub fn is_dirty(&self) -> bool {
        self.as_bytes() != self.saved
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
        self.saved = self.as_bytes().to_vec();
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
