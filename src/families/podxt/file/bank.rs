use super::*;

/// Target-specific view of a Workbench bank/collection containing only PODxt
/// Live tones. The generic guitar document continues to reject such files.
#[derive(Clone, Debug)]
pub struct PodBundleLibrary {
    bytes: Vec<u8>,
    pub(super) saved: Vec<u8>,
    collection: bool,
    banks: Vec<PodBundleBank>,
}

#[derive(Clone, Debug)]
struct PodBundleBank {
    name: String,
    entry_ranges: Vec<Range<usize>>,
    children: Vec<PodBundleChild>,
    folder: bool,
    chunk_start: usize,
    data_length: usize,
}

#[derive(Clone, Debug)]
struct PodBundleChild {
    range: Range<usize>,
    tone: bool,
}

fn span_identities(spans: &[Range<usize>]) -> Vec<(usize, usize)> {
    let mut identities: Vec<_> = spans.iter().map(|span| (span.start, span.end)).collect();
    identities.sort_unstable();
    identities
}

impl PodBundleLibrary {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() as u64 > BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "POD bank exceeds the 16 MiB offline librarian ceiling",
            ));
        }
        if bytes.len() < 12 || &bytes[..4] != b"FORM" || !matches!(&bytes[8..12], b"L6BA" | b"L6CO")
        {
            return Err(ToneError::format(
                "expected Workbench FORM/L6BA or FORM/L6CO",
            ));
        }
        if preset::word(&bytes, 4) as usize != bytes.len() - 8 {
            return Err(ToneError::format(
                "bundle FORM length does not match the file",
            ));
        }
        let collection = &bytes[8..12] == b"L6CO";
        let mut banks = Vec::new();
        let mut tone_count = 0;
        let outer = preset::chunks(&bytes, 12..bytes.len()).map_err(ToneError::format)?;
        for bank in outer.iter().filter(|entry| {
            preset::container(&bytes, entry, b"BANK") || preset::container(&bytes, entry, b"FLDR")
        }) {
            let children = preset::chunks(&bytes, bank.data.start + 4..bank.data.end)
                .map_err(ToneError::format)?;
            let info = preset::unique(&children, b"BINF").map_err(ToneError::format)?;
            if info.data.len() < 68 {
                return Err(ToneError::format("truncated bank information"));
            }
            let units: Vec<_> = bytes[info.data.start + 4..info.data.start + 68]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|unit| u16::from_be_bytes(*unit))
                .take_while(|&unit| unit != 0)
                .collect();
            let name = String::from_utf16(&units)
                .map_err(|_| ToneError::format("invalid UTF-16 bank name"))?
                .trim_end()
                .to_owned();
            let mut entry_ranges = Vec::new();
            let mut child_spans = Vec::with_capacity(children.len());
            for child in &children {
                let payload_end = child.data.end;
                let full_end = payload_end + (child.data.len() & 1);
                let full_range = child.data.start - 8..full_end;
                let is_tone = preset::container(&bytes, child, b"L6PA")
                    || preset::container(&bytes, child, b"L6FS")
                    || preset::container(&bytes, child, b"L6AS");
                if is_tone {
                    let tone_range = child.data.start - 8..payload_end;
                    PodTone::parse(bytes[tone_range].to_vec())?;
                    tone_count += 1;
                    entry_ranges.push(full_range.clone());
                }
                child_spans.push(PodBundleChild {
                    range: full_range,
                    tone: is_tone,
                });
            }
            banks.push(PodBundleBank {
                name,
                entry_ranges,
                children: child_spans,
                folder: preset::container(&bytes, bank, b"FLDR"),
                chunk_start: bank.data.start - 8,
                data_length: bank.data.len(),
            });
        }
        if tone_count == 0 {
            return Err(ToneError::format("POD bank/collection contains no tones"));
        }
        let saved = bytes.clone();
        Ok(Self {
            bytes,
            saved,
            collection,
            banks,
        })
    }

    /// Membership edits are admitted only for existing folders in collections.
    pub fn membership_editable(&self, bank: usize) -> bool {
        self.collection && self.banks.get(bank).is_some_and(|bank| bank.folder)
    }

    /// Whether a bank is within the admitted fixed-bank tone reorder subset.
    /// This is an offline file operation, not a physical slot-layout claim.
    pub fn fixed_bank_reorder_editable(&self, bank: usize) -> bool {
        self.fixed_bank_tone_spans(bank).is_ok()
    }

    /// Move one existing PODxt Live tone to a final slot index in one fixed
    /// L6BA/BANK. Complete tone records are permuted only among their original
    /// token positions; opaque siblings and all enclosing bytes stay in place.
    pub fn reorder_fixed_bank_entry(
        &mut self,
        bank_index: usize,
        from: usize,
        to: usize,
    ) -> Result<usize, ToneError> {
        let mut tone_order = self.fixed_bank_tone_spans(bank_index)?;
        if from >= tone_order.len() || to >= tone_order.len() {
            return Err(ToneError::format("preset index is outside the fixed bank"));
        }
        if from == to {
            return Ok(self.flat_index(bank_index, to));
        }

        let original_ids = span_identities(&tone_order);
        let moved = tone_order.remove(from);
        tone_order.insert(to, moved);
        if span_identities(&tone_order) != original_ids {
            return Err(ToneError::format(
                "fixed-bank reorder must preserve every original tone exactly once",
            ));
        }

        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the file"))?;
        let mut next_tone = tone_order.iter();
        let mut replacement_children = Vec::with_capacity(bank.data_length - 4);
        for child in &bank.children {
            if child.tone {
                let range = next_tone.next().ok_or_else(|| {
                    ToneError::format("fixed-bank reorder has an invalid tone ordering")
                })?;
                replacement_children.extend_from_slice(&self.bytes[range.clone()]);
            } else {
                replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
            }
        }
        if next_tone.next().is_some() || replacement_children.len() != bank.data_length - 4 {
            return Err(ToneError::format(
                "fixed-bank reorder spans do not preserve the bank length",
            ));
        }

        let child_start = bank.chunk_start + 12;
        let child_end = bank.chunk_start + 8 + bank.data_length;
        if child_end.checked_sub(child_start) != Some(replacement_children.len()) {
            return Err(ToneError::format(
                "fixed-bank reorder has inconsistent child spans",
            ));
        }
        let mut candidate = self.bytes.clone();
        candidate[child_start..child_end].copy_from_slice(&replacement_children);

        // Parse the complete candidate before publishing any state. The saved
        // baseline is deliberately retained so moving back can become clean.
        let saved = self.saved.clone();
        let mut next = Self::parse(candidate)?;
        next.fixed_bank_tone_spans(bank_index)?;
        if next.preset_count(bank_index) != Some(tone_order.len()) {
            return Err(ToneError::format(
                "fixed-bank reorder changed the selected bank's tone count",
            ));
        }
        for (slot, expected) in tone_order.iter().enumerate() {
            let actual = &next.banks[bank_index].entry_ranges[slot];
            let expected_end =
                expected.start + 8 + preset::word(&self.bytes, expected.start + 4) as usize;
            if next.tone(bank_index, slot)?.as_bytes() != &self.bytes[expected.start..expected_end]
            {
                return Err(ToneError::format(
                    "fixed-bank reorder candidate did not preserve the requested tone order",
                ));
            }
            if actual.end - actual.start != expected.end - expected.start {
                return Err(ToneError::format(
                    "fixed-bank reorder changed a tone token span length",
                ));
            }
        }
        next.saved = saved;
        *self = next;
        Ok(self.flat_index(bank_index, to))
    }

    fn fixed_bank_tone_spans(&self, bank_index: usize) -> Result<Vec<Range<usize>>, ToneError> {
        if self.collection {
            return Err(ToneError::format(
                "fixed-bank reorder requires an L6BA root",
            ));
        }
        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the file"))?;
        if bank.folder {
            return Err(ToneError::format(
                "fixed-bank reorder requires an existing BANK child",
            ));
        }
        if bank.entry_ranges.is_empty() {
            return Err(ToneError::format(
                "fixed-bank reorder requires a nonempty bank",
            ));
        }
        for index in 0..bank.entry_ranges.len() {
            let tone = self.tone(bank_index, index)?;
            if tone.container() != ContainerKind::L6pa || tone.device_id() != PODXT_LIVE_TONE_ID {
                return Err(ToneError::format(
                    "fixed-bank reorder accepts only existing PODxt Live L6PA tones",
                ));
            }
        }
        Ok(bank.entry_ranges.clone())
    }

    /// Duplicate an existing tone byte-for-byte after its current tone slot.
    /// Returns the new flattened entry index.
    pub fn duplicate_entry(&mut self, bank: usize, preset: usize) -> Result<usize, ToneError> {
        let order = self.membership_order(bank)?;
        let source = order
            .get(preset)
            .cloned()
            .ok_or_else(|| ToneError::format("preset index is outside the folder"))?;
        self.apply_membership_order(bank, order, None, Some(source))?;
        Ok(self.flat_index(bank, preset + 1))
    }

    /// Remove one existing tone. The current library invariant requires at
    /// least one recognized POD tone to remain in the collection.
    pub fn remove_entry(&mut self, bank: usize, preset: usize) -> Result<usize, ToneError> {
        let mut order = self.membership_order(bank)?;
        if preset >= order.len() {
            return Err(ToneError::format("preset index is outside the folder"));
        }
        let old_flat_index = self.flat_index(bank, preset);
        let removed = order.remove(preset);
        if self.entry_count() == 1 {
            return Err(ToneError::format(
                "cannot remove the last POD tone from the collection",
            ));
        }
        self.apply_membership_order(bank, order, Some(removed), None)?;
        Ok(old_flat_index.min(self.entry_count() - 1))
    }

    /// Move an existing tone to a final tone-slot index in the same folder.
    /// Returns the new flattened entry index.
    pub fn reorder_entry(
        &mut self,
        bank: usize,
        from: usize,
        to: usize,
    ) -> Result<usize, ToneError> {
        let mut order = self.membership_order(bank)?;
        if from >= order.len() || to >= order.len() {
            return Err(ToneError::format("preset index is outside the folder"));
        }
        if from != to {
            let tone = order.remove(from);
            order.insert(to, tone);
            self.apply_membership_order(bank, order, None, None)?;
        }
        Ok(self.flat_index(bank, to))
    }

    fn membership_order(&self, bank_index: usize) -> Result<Vec<Range<usize>>, ToneError> {
        if !self.membership_editable(bank_index) {
            return Err(ToneError::format(
                "POD membership edits require an existing FLDR in an L6CO collection",
            ));
        }
        Ok(self.banks[bank_index]
            .children
            .iter()
            .filter(|child| child.tone)
            .map(|child| child.range.clone())
            .collect())
    }

    fn apply_membership_order(
        &mut self,
        bank_index: usize,
        tone_order: Vec<Range<usize>>,
        remove_tone: Option<Range<usize>>,
        duplicate_after: Option<Range<usize>>,
    ) -> Result<(), ToneError> {
        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the collection"))?;
        if !self.membership_editable(bank_index) {
            return Err(ToneError::format(
                "POD membership edits require an existing FLDR in an L6CO collection",
            ));
        }
        if tone_order.is_empty() && self.entry_count() == bank.entry_ranges.len() {
            return Err(ToneError::format(
                "cannot remove the last POD tone from the collection",
            ));
        }

        let child_length = bank.data_length - 4;
        let (new_child_length, new_file_length) = if let Some(removed) = &remove_tone {
            let delta = removed.len();
            (
                child_length
                    .checked_sub(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
                self.bytes
                    .len()
                    .checked_sub(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
            )
        } else if let Some(duplicate) = &duplicate_after {
            let delta = duplicate.len();
            (
                child_length
                    .checked_add(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
                self.bytes
                    .len()
                    .checked_add(delta)
                    .ok_or_else(|| ToneError::format("bundle size overflow"))?,
            )
        } else {
            (child_length, self.bytes.len())
        };
        if new_file_length as u64 > BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "POD bank exceeds the 16 MiB offline librarian ceiling",
            ));
        }
        let new_data_length = 4usize
            .checked_add(new_child_length)
            .ok_or_else(|| ToneError::format("bundle size overflow"))?;
        let bank_length = u32::try_from(new_data_length)
            .map_err(|_| ToneError::format("folder exceeds IFF size capacity"))?;
        let root_length = u32::try_from(new_file_length - 8)
            .map_err(|_| ToneError::format("bundle exceeds IFF size capacity"))?;

        let mut replacement_children = Vec::with_capacity(new_child_length);
        let mut next_tone = tone_order.iter();
        if let Some(removed) = remove_tone {
            for child in &bank.children {
                if child.tone && child.range == removed {
                    continue;
                }
                replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
            }
        } else if let Some(duplicate) = duplicate_after {
            for child in &bank.children {
                replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
                if child.tone && child.range == duplicate {
                    replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
                }
            }
        } else {
            for child in &bank.children {
                if child.tone {
                    let range = next_tone.next().ok_or_else(|| {
                        ToneError::format("membership edit has an invalid tone ordering")
                    })?;
                    replacement_children.extend_from_slice(&self.bytes[range.clone()]);
                } else {
                    replacement_children.extend_from_slice(&self.bytes[child.range.clone()]);
                }
            }
            if next_tone.next().is_some() {
                return Err(ToneError::format(
                    "membership edit has an invalid tone ordering",
                ));
            }
        }

        if replacement_children.len() != new_child_length {
            return Err(ToneError::format(
                "membership span lengths are inconsistent",
            ));
        }
        let child_start = bank.chunk_start + 12;
        let child_end = bank.chunk_start + 8 + bank.data_length;
        let mut bytes = self.bytes.clone();
        bytes.splice(child_start..child_end, replacement_children);
        bytes[bank.chunk_start + 4..bank.chunk_start + 8]
            .copy_from_slice(&bank_length.to_be_bytes());
        bytes[4..8].copy_from_slice(&root_length.to_be_bytes());
        let saved = self.saved.clone();
        let mut next = Self::parse(bytes)?;
        next.saved = saved;
        *self = next;
        Ok(())
    }

    fn flat_index(&self, bank_index: usize, preset_index: usize) -> usize {
        self.banks[..bank_index]
            .iter()
            .map(|bank| bank.entry_ranges.len())
            .sum::<usize>()
            + preset_index
    }

    pub fn bank_count(&self) -> usize {
        self.banks.len()
    }

    pub fn entry_count(&self) -> usize {
        self.banks.iter().map(|bank| bank.entry_ranges.len()).sum()
    }

    pub fn entry_location(&self, index: usize) -> Option<(usize, usize)> {
        let mut offset = index;
        for (bank_index, bank) in self.banks.iter().enumerate() {
            if offset < bank.entry_ranges.len() {
                return Some((bank_index, offset));
            }
            offset -= bank.entry_ranges.len();
        }
        None
    }

    pub fn preset_count(&self, bank: usize) -> Option<usize> {
        self.banks.get(bank).map(|entry| entry.entry_ranges.len())
    }

    pub fn bank_name(&self, bank: usize) -> Option<&str> {
        self.banks.get(bank).map(|entry| entry.name.as_str())
    }

    pub fn tone(&self, bank: usize, preset: usize) -> Result<PodTone, ToneError> {
        let range = self
            .banks
            .get(bank)
            .and_then(|entry| entry.entry_ranges.get(preset))
            .ok_or_else(|| ToneError::format("bank or preset index is outside the library"))?
            .clone();
        // Parse by the embedded FORM's own declared length so its parent's IFF
        // alignment byte never enters PodTone.
        let form_length = preset::word(&self.bytes, range.start + 4) as usize;
        let end = range.start + 8 + form_length;
        if end > range.end {
            return Err(ToneError::format("embedded tone exceeds its IFF chunk"));
        }
        PodTone::parse(self.bytes[range.start..end].to_vec())
    }

    pub fn edit_tone(
        &mut self,
        bank: usize,
        preset: usize,
        edit: impl FnOnce(&mut PodTone) -> Result<(), ToneError>,
    ) -> Result<(), ToneError> {
        let mut tone = self.tone(bank, preset)?;
        let original = tone.as_bytes().to_vec();
        edit(&mut tone)?;
        if tone.as_bytes() == original {
            return Ok(());
        }
        self.replace_tone(bank, preset, &tone)
    }

    pub fn edit_entry(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut PodTone) -> Result<(), ToneError>,
    ) -> Result<(), ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.edit_tone(bank, preset, edit)
    }

    pub fn set_delay_note_choice(
        &mut self,
        index: usize,
        choice: DelayNoteChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_delay_note_choice(choice))
    }

    pub fn set_mod_note_choice(
        &mut self,
        index: usize,
        choice: ModNoteChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mod_note_choice(choice))
    }

    pub fn set_mod_speed_hz(&mut self, index: usize, hz: f32) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mod_speed_hz(hz))
    }

    pub fn set_delay_time_ms(&mut self, index: usize, milliseconds: f32) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_delay_time_ms(milliseconds))
    }

    pub fn stomp_whammy_knob_position(
        &self,
        index: usize,
        control: StompWhammyKnob,
    ) -> Result<f32, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the bundle"))?;
        self.tone(bank, preset)?.stomp_whammy_knob_position(control)
    }

    pub fn set_stomp_whammy_knob_position(
        &mut self,
        index: usize,
        control: StompWhammyKnob,
        position: f32,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| {
            tone.set_stomp_whammy_knob_position(control, position)
        })
    }

    pub fn set_podxt_eq_band(
        &mut self,
        index: usize,
        band: PodxtEqBand,
        frequency_hz: f32,
        gain_db: f32,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| {
            tone.set_podxt_eq_band(band, frequency_hz, gain_db)
        })
    }

    pub fn reset_podxt_eq(&mut self, index: usize) -> Result<(), ToneError> {
        self.edit_entry(index, PodTone::reset_podxt_eq)
    }

    pub fn set_model_enabled(
        &mut self,
        index: usize,
        control: PodModelEnable,
        enabled: bool,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_model_enabled(control, enabled))
    }

    pub fn set_amp_model_choice(
        &mut self,
        index: usize,
        choice: PodAmpModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_amp_model_choice(choice))
    }

    pub fn set_delay_model_choice(
        &mut self,
        index: usize,
        choice: PodDelayModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_delay_model_choice(choice))
    }

    pub fn set_mod_model_choice(
        &mut self,
        index: usize,
        choice: PodModModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mod_model_choice(choice))
    }

    pub fn set_stomp_model_choice(
        &mut self,
        index: usize,
        choice: PodStompModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_model_choice(choice))
    }

    pub fn set_reverb_model_choice(
        &mut self,
        index: usize,
        choice: PodReverbModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_reverb_model_choice(choice))
    }

    pub fn set_cab_model_choice(
        &mut self,
        index: usize,
        choice: PodCabModelChoice,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_cab_model_choice(choice))
    }
    pub fn set_mic_choice(&mut self, index: usize, choice: PodMicChoice) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_mic_choice(choice))
    }

    pub fn stomp_wave_selection(&self, index: usize) -> Result<StompWaveValue, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.tone(bank, preset)?.stomp_wave_selection()
    }

    pub fn set_stomp_wave_selection(
        &mut self,
        index: usize,
        selection: StompWaveSelection,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_wave_selection(selection))
    }

    pub fn stomp_harmony_1m335_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1M335Selection>, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.tone(bank, preset)?.stomp_harmony_1m335_selection()
    }
    pub fn set_stomp_harmony_1m335_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1M335Selection,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_harmony_1m335_selection(choice))
    }
    pub fn stomp_harmony_1457_selection(
        &self,
        index: usize,
    ) -> Result<StompHarmonyValue<StompHarmony1457Selection>, ToneError> {
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.tone(bank, preset)?.stomp_harmony_1457_selection()
    }
    pub fn set_stomp_harmony_1457_selection(
        &mut self,
        index: usize,
        choice: StompHarmony1457Selection,
    ) -> Result<(), ToneError> {
        self.edit_entry(index, |tone| tone.set_stomp_harmony_1457_selection(choice))
    }

    pub fn replace_entry(&mut self, index: usize, bytes: Vec<u8>) -> Result<(), ToneError> {
        let replacement = PodTone::parse(bytes)?;
        let (bank, preset) = self
            .entry_location(index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?;
        self.replace_tone(bank, preset, &replacement)
    }

    fn replace_tone(
        &mut self,
        bank_index: usize,
        preset_index: usize,
        replacement: &PodTone,
    ) -> Result<(), ToneError> {
        let bank = self
            .banks
            .get(bank_index)
            .ok_or_else(|| ToneError::format("bank index is outside the library"))?;
        let range = bank
            .entry_ranges
            .get(preset_index)
            .ok_or_else(|| ToneError::format("preset index is outside the library"))?
            .clone();
        let old_span = range.end - range.start;
        let old_form_length = preset::word(&self.bytes, range.start + 4) as usize;
        let old_payload_length = 8 + old_form_length;
        let tone_bytes = replacement.as_bytes();
        let replacement_span = tone_bytes.len() + (tone_bytes.len() & 1);
        let new_bank_length = bank
            .data_length
            .checked_sub(old_span)
            .and_then(|length| length.checked_add(replacement_span))
            .ok_or_else(|| ToneError::format("bundle size overflow"))?;
        let new_file_length = self
            .bytes
            .len()
            .checked_sub(old_span)
            .and_then(|length| length.checked_add(replacement_span))
            .ok_or_else(|| ToneError::format("bundle size overflow"))?;
        if new_file_length as u64 > BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "POD bank exceeds the 16 MiB offline librarian ceiling",
            ));
        }
        let bank_length = u32::try_from(new_bank_length)
            .map_err(|_| ToneError::format("bank exceeds IFF size capacity"))?;
        let root_length = u32::try_from(new_file_length - 8)
            .map_err(|_| ToneError::format("bundle exceeds IFF size capacity"))?;
        let chunk_start = bank.chunk_start;
        let mut encoded = tone_bytes.to_vec();
        if tone_bytes.len() & 1 != 0 {
            let original_pad =
                (old_payload_length & 1 == 1).then(|| self.bytes[range.start + old_payload_length]);
            encoded.push(original_pad.unwrap_or(0));
        }
        let mut bytes = self.bytes.clone();
        bytes.splice(range, encoded);
        bytes[chunk_start + 4..chunk_start + 8].copy_from_slice(&bank_length.to_be_bytes());
        bytes[4..8].copy_from_slice(&root_length.to_be_bytes());
        let saved = self.saved.clone();
        let mut next = Self::parse(bytes)?;
        next.saved = saved;
        *self = next;
        Ok(())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn is_dirty(&self) -> bool {
        self.as_bytes() != self.saved
    }

    pub fn revert(&mut self) -> Result<(), ToneError> {
        if self.is_dirty() {
            let restored = Self::parse(self.saved.clone())?;
            *self = restored;
        }
        Ok(())
    }

    pub fn note_saved(&mut self) {
        self.saved = self.bytes.clone();
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, self.as_bytes()).map_err(ToneError::format)
    }
}

/// Reads a bounded Workbench POD bank after path and opened-handle checks.
///
/// The reader is limited to the application ceiling plus one sentinel byte,
/// including when the file grows after either metadata check.
pub fn read_regular_bundle<R: std::io::Read>(
    facts: std::io::Result<PathFacts>,
    open: impl FnOnce() -> std::io::Result<(PathFacts, R)>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "POD bank input must be a regular file. A pipe, socket, or device node is not opened.",
        ));
    }
    if facts.len > BUNDLE_BYTES_CEILING {
        return Err(ToneError::format(
            "POD bank exceeds the 16 MiB offline librarian ceiling",
        ));
    }
    let (opened_facts, reader) = open().map_err(|error| ToneError::format(error.to_string()))?;
    if !opened_facts.regular_file {
        return Err(ToneError::format(
            "POD bank input must be a regular file. A pipe, socket, or device node is not read.",
        ));
    }
    if opened_facts.len > BUNDLE_BYTES_CEILING {
        return Err(ToneError::format(
            "POD bank exceeds the 16 MiB offline librarian ceiling",
        ));
    }
    let mut bytes = Vec::with_capacity(opened_facts.len.min(BUNDLE_BYTES_CEILING) as usize);
    let mut bounded = std::io::Read::take(reader, BUNDLE_BYTES_CEILING + 1);
    std::io::Read::read_to_end(&mut bounded, &mut bytes)
        .map_err(|error| ToneError::format(error.to_string()))?;
    if bytes.len() as u64 > BUNDLE_BYTES_CEILING {
        return Err(ToneError::format(
            "POD bank exceeds the 16 MiB offline librarian ceiling",
        ));
    }
    Ok(bytes)
}
