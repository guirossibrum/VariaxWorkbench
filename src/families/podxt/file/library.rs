use super::*;
use crate::shared::file::read_bounded_file;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StoredTone {
    pub(super) tone: PodTone,
    /// Bytes last imported or successfully saved. Compared with the applied tone.
    pub(super) saved: Vec<u8>,
}

/// In-memory list of individual tone files. This is not a bank format.
///
/// Each tone keeps the bytes last imported or successfully saved. An applied
/// edit that differs from that baseline is unsaved. Saving or discarding one
/// tone does not change another tone's baseline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    pub(super) tones: Vec<StoredTone>,
}

impl Library {
    pub fn clear(&mut self) {
        self.tones.clear();
    }

    pub fn import_bytes(&mut self, bytes: Vec<u8>) -> Result<usize, ToneError> {
        let tone = PodTone::parse(bytes)?;
        let saved = tone.as_bytes().to_vec();
        self.tones.push(StoredTone { tone, saved });
        Ok(self.tones.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.tones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tones.is_empty()
    }

    pub fn tone(&self, index: usize) -> Option<&PodTone> {
        self.tones.get(index).map(|entry| &entry.tone)
    }

    pub fn set_model_enabled(
        &mut self,
        index: usize,
        control: PodModelEnable,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_model_enabled(control, enabled)
    }

    pub fn set_amp_model_choice(
        &mut self,
        index: usize,
        choice: PodAmpModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_amp_model_choice(choice)
    }

    pub fn set_delay_model_choice(
        &mut self,
        index: usize,
        choice: PodDelayModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_model_choice(choice)
    }

    pub fn set_mod_model_choice(
        &mut self,
        index: usize,
        choice: PodModModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_model_choice(choice)
    }

    pub fn set_stomp_model_choice(
        &mut self,
        index: usize,
        choice: PodStompModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_model_choice(choice)
    }

    pub fn set_reverb_model_choice(
        &mut self,
        index: usize,
        choice: PodReverbModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_reverb_model_choice(choice)
    }

    pub fn set_cab_model_choice(
        &mut self,
        index: usize,
        choice: PodCabModelChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_cab_model_choice(choice)
    }
    pub fn set_mic_choice(&mut self, index: usize, choice: PodMicChoice) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mic_choice(choice)
    }

    pub fn set_amp_drive_fraction(&mut self, index: usize, value: f32) -> Result<(), ToneError> {
        self.set_amp_control_fraction(index, AmpControl::Drive, value)
    }

    pub fn set_amp_control_fraction(
        &mut self,
        index: usize,
        control: AmpControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_amp_control_fraction(control, value)
    }

    pub fn set_stomp_drive_fraction(&mut self, index: usize, value: f32) -> Result<(), ToneError> {
        self.set_stomp_control_fraction(index, StompControl::Drive, value)
    }

    pub fn set_stomp_control_fraction(
        &mut self,
        index: usize,
        control: StompControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_control_fraction(control, value)
    }

    pub fn stomp_whammy_knob_position(
        &self,
        index: usize,
        control: StompWhammyKnob,
    ) -> Result<f32, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_whammy_knob_position(control)
    }

    pub fn set_stomp_whammy_knob_position(
        &mut self,
        index: usize,
        control: StompWhammyKnob,
        position: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_whammy_knob_position(control, position)
    }

    pub fn stomp_wave_selection(&self, index: usize) -> Result<StompWaveValue, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_wave_selection()
    }

    pub fn set_stomp_wave_selection(
        &mut self,
        index: usize,
        selection: StompWaveSelection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_wave_selection(selection)
    }

    pub fn stomp_harmony_1m335_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1M335Selection>, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_harmony_1m335_selection()
    }
    pub fn set_stomp_harmony_1m335_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1M335Selection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_harmony_1m335_selection(choice)
    }
    pub fn stomp_harmony_1457_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1457Selection>, ToneError> {
        self.tones
            .get(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?
            .tone
            .stomp_harmony_1457_selection()
    }
    pub fn set_stomp_harmony_1457_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1457Selection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_stomp_harmony_1457_selection(choice)
    }

    pub fn set_mod_control_fraction(
        &mut self,
        index: usize,
        control: ModControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_control_fraction(control, value)
    }

    pub fn set_reverb_control_fraction(
        &mut self,
        index: usize,
        control: ReverbControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_reverb_control_fraction(control, value)
    }

    pub fn set_delay_control_fraction(
        &mut self,
        index: usize,
        control: DelayControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_control_fraction(control, value)
    }

    pub fn set_delay_note_choice(
        &mut self,
        index: usize,
        choice: DelayNoteChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_note_choice(choice)
    }

    pub fn set_mod_note_choice(
        &mut self,
        index: usize,
        choice: ModNoteChoice,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_note_choice(choice)
    }

    pub fn set_mod_speed_hz(&mut self, index: usize, hz: f32) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_mod_speed_hz(hz)
    }

    pub fn set_delay_time_ms(&mut self, index: usize, milliseconds: f32) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_time_ms(milliseconds)
    }

    pub fn set_delay_heads_selection(
        &mut self,
        index: usize,
        selection: DelayHeadsSelection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_heads_selection(selection)
    }

    pub fn set_delay_bits_selection(
        &mut self,
        index: usize,
        selection: DelayBitsSelection,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_delay_bits_selection(selection)
    }

    pub fn set_podxt_eq_band(
        &mut self,
        index: usize,
        band: PodxtEqBand,
        frequency_hz: f32,
        gain_db: f32,
    ) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.set_podxt_eq_band(band, frequency_hz, gain_db)
    }

    pub fn reset_podxt_eq(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.tone.reset_podxt_eq()
    }

    pub fn replace(&mut self, index: usize, bytes: Vec<u8>) -> Result<(), ToneError> {
        let tone = PodTone::parse(bytes)?;
        let slot = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        slot.tone = tone;
        Ok(())
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.tones
            .get(index)
            .is_some_and(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    pub fn any_dirty(&self) -> bool {
        self.tones
            .iter()
            .any(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    pub fn dirty_count(&self) -> usize {
        self.tones
            .iter()
            .filter(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
            .count()
    }

    pub fn first_dirty(&self) -> Option<usize> {
        self.tones
            .iter()
            .position(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    /// Record a successful save of this tone only.
    pub fn note_saved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.saved = entry.tone.as_bytes().to_vec();
        Ok(())
    }

    /// Restore one tone to its imported or last-saved bytes.
    pub fn revert_unsaved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        if entry.tone.as_bytes() != entry.saved.as_slice() {
            entry.tone = PodTone::parse(entry.saved.clone())?;
        }
        Ok(())
    }
}

/// Reads only after `facts` names a regular file at or under the ceiling.
///
/// A pipe, socket, or device node is refused first, including when its
/// reported length is zero. `read` is not called in that case.
pub fn read_regular_tone<R: Read>(
    facts: std::io::Result<PathFacts>,
    read: impl FnOnce() -> std::io::Result<R>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "tone input must be a regular file. A pipe, socket, or device node is not opened.",
        ));
    }
    if facts.len > TONE_BYTES_CEILING {
        return Err(ToneError::format(
            "tone file exceeds the 1 MiB offline librarian ceiling",
        ));
    }
    read_bounded_file(
        read().map_err(|error| ToneError::format(error.to_string()))?,
        TONE_BYTES_CEILING,
        "tone file exceeds the 1 MiB offline librarian ceiling",
    )
}
