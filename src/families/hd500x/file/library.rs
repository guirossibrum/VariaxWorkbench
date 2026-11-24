use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredHdFile {
    file: HdEditFile,
    saved: Vec<u8>,
}

/// Shared in-memory library for statically framed HD Edit files. Imported
/// containers and their source bytes are retained independently; extracting a
/// child creates a separate preset entry and never rewrites its parent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HdEditorLibrary {
    files: Vec<StoredHdFile>,
}

impl HdEditorLibrary {
    /// Applies an admitted type-9 choice to an extracted standalone H5EP.
    /// H5ES/H5EB parent bytes remain independently preserved and unchanged.
    pub fn set_type9_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        choice: HdType9Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_hd_type9_choice(effect_slot, choice),
            _ => Err(ToneError::format(
                "HD type-9 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the exact admitted type-8 choice to an extracted standalone
    /// H5EP. H5ES/H5EB parent bytes remain independently preserved.
    pub fn set_type8_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        choice: HdType8Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_hd_type8_choice(effect_slot, choice),
            _ => Err(ToneError::format(
                "HD type-8 choice requires an extracted H5EP preset",
            )),
        }
    }

    pub fn set_type10_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        choice: HdType10Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_hd_type10_choice(effect_slot, choice),
            _ => Err(ToneError::format(
                "HD type-10 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies one explicitly keyed type-11 choice to an extracted H5EP.
    /// Same-model controls remain independent and parent containers unchanged.
    pub fn set_type11_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        control: HdType11Control,
        choice: HdType11Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_hd_type11_choice(effect_slot, control, choice),
            _ => Err(ToneError::format(
                "HD type-11 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the one admitted type-12 choice to an extracted H5EP only.
    pub fn set_type12_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        control: HdType12Control,
        choice: HdType12Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_hd_type12_choice(effect_slot, control, choice),
            _ => Err(ToneError::format(
                "HD type-12 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the single static-derived type-0x0D four-choice value to an
    /// extracted H5EP only; enclosing H5ES/H5EB bytes remain independent.
    pub fn set_type13_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        control: HdType13Control,
        choice: HdType13Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_hd_type13_choice(effect_slot, control, choice),
            _ => Err(ToneError::format(
                "HD type-13 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the single static-derived descriptor type-0x13 (decimal 19)
    /// eight-choice candidate to an extracted H5EP only.
    pub fn set_type19_choice(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        control: HdType19Control,
        choice: HdType19Choice,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_hd_type19_choice(effect_slot, control, choice),
            _ => Err(ToneError::format(
                "HD descriptor type-0x13 choice requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies an enable flag to one standalone H5EP item. Extract a child
    /// first for H5ES/H5EB; the parents remain untouched in the library.
    pub fn set_effect_enabled(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => preset.set_effect_enabled(effect_slot, enabled),
            _ => Err(ToneError::format(
                "HD effect enable requires an extracted H5EP preset",
            )),
        }
    }

    pub fn import_bytes(&mut self, bytes: Vec<u8>) -> Result<usize, ToneError> {
        let file = HdEditFile::parse(bytes)?;
        let saved = file.as_bytes().to_vec();
        self.files.push(StoredHdFile { file, saved });
        Ok(self.files.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn file(&self, index: usize) -> Option<&HdEditFile> {
        self.files.get(index).map(|entry| &entry.file)
    }

    pub fn file_mut(&mut self, index: usize) -> Option<&mut HdEditFile> {
        self.files.get_mut(index).map(|entry| &mut entry.file)
    }

    pub fn extract_preset(
        &mut self,
        index: usize,
        setlist: usize,
        preset: usize,
    ) -> Result<usize, ToneError> {
        let child = match self.file(index) {
            Some(HdEditFile::H5ep(_)) if setlist == 0 && preset == 0 => {
                return Ok(index);
            }
            Some(HdEditFile::H5es(file)) if setlist == 0 => file.preset_bytes(preset),
            Some(HdEditFile::H5eb(file)) => file.setlist_bytes(setlist).and_then(|bytes| {
                let start = H5ES_HEADER_BYTES + preset.checked_mul(H5EP_MIN_BYTES)?;
                bytes.get(start..start.checked_add(H5EP_MIN_BYTES)?)
            }),
            _ => None,
        }
        .ok_or_else(|| ToneError::format("setlist or preset index is outside the file"))?
        .to_vec();
        let file = HdEditFile::H5ep(OpaqueH5epPreset::parse(child)?);
        let saved = file.as_bytes().to_vec();
        self.files.push(StoredHdFile { file, saved });
        Ok(self.files.len() - 1)
    }

    /// Applies the one admitted model-default replacement to a standalone
    /// H5EP item. For H5ES/H5EB parents, extract a child first; their bytes are
    /// retained unchanged as separate library items.
    pub fn replace_octave_fuzz_with_vintage_pre_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_vintage_pre_defaults(effect_slot)
            }
            _ => Err(ToneError::format(
                "Vintage Pre replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the bounded Screamer defaults replacement to a standalone H5EP
    /// item. Extract H5ES/H5EB children first; their parents remain untouched.
    pub fn replace_octave_fuzz_with_screamer_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_screamer_defaults(effect_slot)
            }
            _ => Err(ToneError::format(
                "Screamer replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the bounded Sub Octave Fuzz defaults replacement to a
    /// standalone H5EP item. Extract H5ES/H5EB children first; parents remain
    /// untouched.
    pub fn replace_octave_fuzz_with_sub_octave_fuzz_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_sub_octave_fuzz_defaults(effect_slot)
            }
            _ => Err(ToneError::format(
                "Sub Octave Fuzz replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies the bounded Noise Gate defaults replacement to a standalone
    /// H5EP item. Extract H5ES/H5EB children first; parents remain untouched.
    pub fn replace_octave_fuzz_with_noise_gate_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_noise_gate_defaults(effect_slot)
            }
            _ => Err(ToneError::format(
                "Noise Gate replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies one admitted closed-set distortion/fuzz default to an extracted
    /// H5EP item. H5ES/H5EB parents remain separate and unchanged.
    pub fn replace_octave_fuzz_with_admitted_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        target: HdDistortionDefault,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_admitted_defaults(effect_slot, target)
            }
            _ => Err(ToneError::format(
                "admitted defaults replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies one admitted closed-set FX default to an extracted H5EP item.
    /// H5ES/H5EB parents remain separate and unchanged.
    pub fn replace_octave_fuzz_with_admitted_dynamics_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        target: HdDynamicsDefault,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_admitted_dynamics_defaults(effect_slot, target)
            }
            _ => Err(ToneError::format(
                "admitted FX defaults replacement requires an extracted H5EP preset",
            )),
        }
    }

    /// Applies one admitted closed-set reverb default to an extracted H5EP
    /// item. H5ES/H5EB parents remain separate and unchanged.
    pub fn replace_octave_fuzz_with_admitted_reverb_defaults(
        &mut self,
        index: usize,
        effect_slot: HdFxSlot,
        target: HdReverbDefault,
    ) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        match &mut entry.file {
            HdEditFile::H5ep(preset) => {
                preset.replace_octave_fuzz_with_admitted_reverb_defaults(effect_slot, target)
            }
            _ => Err(ToneError::format(
                "admitted reverb defaults replacement requires an extracted H5EP preset",
            )),
        }
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.files
            .get(index)
            .is_some_and(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn any_dirty(&self) -> bool {
        self.files
            .iter()
            .any(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn dirty_count(&self) -> usize {
        self.files
            .iter()
            .filter(|entry| entry.file.as_bytes() != entry.saved)
            .count()
    }

    pub fn first_dirty(&self) -> Option<usize> {
        self.files
            .iter()
            .position(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn note_saved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        entry.saved = entry.file.as_bytes().to_vec();
        Ok(())
    }

    pub fn revert_unsaved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        if entry.file.as_bytes() != entry.saved {
            entry.file = HdEditFile::parse(entry.saved.clone())?;
        }
        Ok(())
    }
}
