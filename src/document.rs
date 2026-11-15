//! Shared offline editor state and atomic creation of new preset files.
use crate::preset::PresetFile;
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

pub struct EditorDocument {
    preset: PresetFile,
    undo: Vec<PresetFile>,
    redo: Vec<PresetFile>,
    saved: Vec<u8>,
    group_base: Option<PresetFile>,
    pub source: PathBuf,
}

impl EditorDocument {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let preset = PresetFile::parse(bytes)?;
        Ok(Self::from_preset(preset, path.to_owned()))
    }

    pub fn from_preset(preset: PresetFile, source: PathBuf) -> Self {
        Self {
            saved: preset.as_bytes().to_vec(),
            preset,
            undo: vec![],
            redo: vec![],
            group_base: None,
            source,
        }
    }
    pub fn preset(&self) -> &PresetFile {
        &self.preset
    }
    pub fn is_dirty(&self) -> bool {
        self.preset.as_bytes() != self.saved
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
        let mut next = self.preset.clone();
        change(&mut next)?;
        if next.as_bytes() != self.preset.as_bytes() {
            let previous = std::mem::replace(&mut self.preset, next);
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
            self.redo
                .push(std::mem::replace(&mut self.preset, previous));
        }
    }
    pub fn redo(&mut self) {
        self.end_gesture();
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.preset, next));
        }
    }
    pub fn save_copy(&mut self, path: &Path) -> Result<(), String> {
        self.end_gesture();
        save_new(path, self.preset.as_bytes())?;
        self.saved = self.preset.as_bytes().to_vec();
        Ok(())
    }

    pub fn begin_gesture(&mut self) {
        if self.group_base.is_none() {
            self.group_base = Some(self.preset.clone());
        }
    }
    pub fn end_gesture(&mut self) {
        if let Some(base) = self.group_base.take()
            && base.as_bytes() != self.preset.as_bytes()
        {
            self.undo.push(base);
            self.redo.clear();
        }
    }
}
