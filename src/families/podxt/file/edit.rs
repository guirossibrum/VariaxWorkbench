use super::*;
use crate::pod_assets;

impl PodTone {
    /// Apply one admitted AMP choice to the existing AMP MINF model-id word.
    /// This file-only Apply path never creates a slot or rewrites PARMs.
    pub fn set_amp_model_choice(&mut self, choice: PodAmpModelChoice) -> Result<(), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "AMP model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "AMP model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "AMP model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }

        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| model.slot == AMP_SLOT)
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "AMP model selection requires exactly one existing model in slot {AMP_SLOT:08x}"
            )));
        };
        let model_index = *model_index;
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .ok_or_else(|| ToneError::format("AMP model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|minimum_end| minimum_end > range.end)
        {
            return Err(ToneError::format(
                "AMP model selection requires an existing MINF body of at least 12 bytes",
            ));
        }

        let requested = choice.model_id();
        let current = self.models[model_index].model_id;
        let legacy_choice = matches!(
            choice,
            PodAmpModelChoice::NoAmp | PodAmpModelChoice::Line6Clean
        );
        // GetAmpModelList returns its 107-entry AMP_MODELS initializer,
        // including No Amp. Only the two legacy
        // requests remain available when the stored current ID is unknown.
        if !legacy_choice && !PODXT_AMP_SOURCE_MODEL_IDS.contains(&current) {
            return Err(ToneError::format(
                "new AMP choices require a current model ID in the source-listed 107-ID catalog",
            ));
        }
        if self.models[model_index].model_id == requested {
            return Ok(());
        }
        let mut staged = self.bytes.clone();
        staged[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
        let verified = Self::parse(staged)?;
        let verified_amp: Vec<_> = verified
            .models
            .iter()
            .filter(|model| model.slot == AMP_SLOT)
            .collect();
        if verified.container != ContainerKind::L6pa
            || verified.device_id != PODXT_LIVE_TONE_ID
            || verified.device_version < PODXT_LIVE_HOST_MIN_VERSION
            || verified_amp.len() != 1
            || verified_amp[0].model_id != requested
        {
            return Err(ToneError::format(
                "staged AMP model selection did not reparse to the requested existing slot",
            ));
        }
        *self = verified;
        Ok(())
    }

    /// Return the Delay model ID when the tone supports model selection.
    pub fn editable_delay_model_id(&self) -> Result<u32, ToneError> {
        self.delay_model_selection().map(|(_, _, current)| current)
    }

    fn delay_model_selection(&self) -> Result<(usize, usize, u32), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "Delay model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "Delay model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "Delay model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }
        let delay_slots = [DELAY_SLOT, 0x0005_0004];
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| delay_slots.contains(&model.slot))
            .map(|(i, _)| i)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(
                "Delay model selection requires exactly one existing primary or fallback model",
            ));
        };
        let model_index = *model_index;
        let current = self.models[model_index].model_id;
        if !DELAY_CONTROL_MODEL_IDS.contains(&current) {
            return Err(ToneError::format(format!(
                "unknown current Delay model id {current:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .ok_or_else(|| ToneError::format("Delay model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "Delay model selection requires an existing MINF body of at least 12 bytes",
            ));
        }
        Ok((model_index, range.start, current))
    }

    /// Change only the existing Delay model ID in an individual L6PA tone.
    /// This application policy requires exactly one primary-or-fallback slot.
    pub fn set_delay_model_choice(&mut self, choice: PodDelayModelChoice) -> Result<(), ToneError> {
        let (model_index, offset, current) = self.delay_model_selection()?;
        let requested = choice.model_id();
        if current == requested {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        candidate[offset..offset + 4].copy_from_slice(&requested.to_be_bytes());
        let parsed = Self::parse(candidate)?;
        if parsed
            .models
            .get(model_index)
            .is_none_or(|model| model.model_id != requested)
        {
            return Err(ToneError::format(
                "Delay model candidate did not reparse to the requested model",
            ));
        }
        *self = parsed;
        Ok(())
    }

    /// Return the Mod model ID when the tone supports model selection.
    pub fn editable_mod_model_id(&self) -> Result<u32, ToneError> {
        self.mod_model_selection().map(|(_, _, current)| current)
    }

    fn mod_model_selection(&self) -> Result<(usize, usize, u32), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "Mod model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "Mod model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "Mod model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }

        let mod_slots = [MOD_SLOT, 0x0005_0003];
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| mod_slots.contains(&model.slot))
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(
                "Mod model selection requires exactly one existing primary or fallback model",
            ));
        };
        let model_index = *model_index;
        let current = self.models[model_index].model_id;
        if !MOD_CONTROL_MODEL_IDS.contains(&current) {
            return Err(ToneError::format(format!(
                "unknown current Mod model id {current:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .ok_or_else(|| ToneError::format("Mod model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "Mod model selection requires an existing MINF body of at least 12 bytes",
            ));
        }

        Ok((model_index, range.start, current))
    }

    /// Change only the existing Mod MINF model ID in an individual L6PA tone.
    /// This conservative offline policy admits only source-listed choices
    /// without a Pack 4 selector predicate and requires one existing slot.
    pub fn set_mod_model_choice(&mut self, choice: PodModModelChoice) -> Result<(), ToneError> {
        let (model_index, offset, current) = self.mod_model_selection()?;
        let requested = choice.model_id();
        if current == requested {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        candidate[offset..offset + 4].copy_from_slice(&requested.to_be_bytes());
        let parsed = Self::parse(candidate)?;
        if parsed
            .models
            .get(model_index)
            .is_none_or(|model| model.model_id != requested)
        {
            return Err(ToneError::format(
                "Mod model candidate did not reparse to the requested model",
            ));
        }
        *self = parsed;
        Ok(())
    }

    /// Return the parsed source interpretation of one applicable model flag.
    /// Noncanonical bytes remain readable and are reported with their raw byte.
    pub fn model_enable_state(
        &self,
        control: PodModelEnable,
    ) -> Result<ModelEnableState, ToneError> {
        let (model_index, offset) = self.model_enable_location(control)?;
        let raw = self.bytes[offset + 11];
        debug_assert_eq!(self.models[model_index].enabled, Some((raw as i8) > 0));
        Ok(match raw {
            0 => ModelEnableState::Canonical(false),
            1 => ModelEnableState::Canonical(true),
            raw => ModelEnableState::NonCanonical {
                raw,
                source_interpretation: (raw as i8) > 0,
            },
        })
    }

    /// Set one existing PODxt model's MINF enabled state without creating or
    /// replacing records. The only file-byte change is MINF body byte 11.
    pub fn set_model_enabled(
        &mut self,
        control: PodModelEnable,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let (model_index, offset) = self.model_enable_location(control)?;
        let raw = self.bytes[offset + 11];
        if raw != 0 && raw != 1 {
            return Err(ToneError::format(format!(
                "{} enable edit requires canonical MINF byte 0x00 or 0x01; found 0x{raw:02x}",
                control.label()
            )));
        }
        let next = u8::from(enabled);
        if raw == next {
            return Ok(());
        }
        self.bytes[offset + 11] = next;
        self.models[model_index].enabled = Some(enabled);
        Ok(())
    }

    fn model_enable_location(&self, control: PodModelEnable) -> Result<(usize, usize), ToneError> {
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "model-enable edit requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "model-enable edit requires the PODxt Live host profile minimum 3.00",
            ));
        }
        let slots = control.slots();
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| slots.contains(&model.slot))
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "{} enable edit requires exactly one existing model in its slot",
                control.label()
            )));
        };
        let model_index = *model_index;
        if let Some(expected) = control.expected_model_id()
            && self.models[model_index].model_id != expected
        {
            let slot = self.models[model_index].slot;
            return Err(ToneError::format(format!(
                "EQ enable edit requires model {expected:08x} in slot {slot:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .ok_or_else(|| ToneError::format("model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|minimum_end| minimum_end > range.end)
        {
            return Err(ToneError::format(format!(
                "{} enable edit requires an existing MINF body of at least 12 bytes",
                control.label()
            )));
        }
        Ok((model_index, range.start))
    }

    /// Amp model id when slot `0x00030000` is present.
    pub fn amp_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == AMP_SLOT)
            .map(|model| model.model_id)
    }

    /// Stomp model id when slot `0x00020003` is present.
    pub fn stomp_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
    }

    /// Apply one admitted Stomp choice to the existing slot's MINF model ID.
    /// This conservative file-only operation changes no parameters or other bytes.
    pub fn set_stomp_model_choice(&mut self, choice: PodStompModelChoice) -> Result<(), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "Stomp model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "Stomp model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "Stomp model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }

        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| model.slot == STOMP_SLOT)
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "Stomp model selection requires exactly one existing model in slot {STOMP_SLOT:08x}"
            )));
        };
        let model_index = *model_index;
        let current = self.models[model_index].model_id;
        if !STOMP_CONTROL_MODEL_IDS.contains(&current) {
            return Err(ToneError::format(format!(
                "unknown current Stomp model id {current:08x}"
            )));
        }
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .cloned()
            .ok_or_else(|| ToneError::format("Stomp model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "Stomp model selection requires an existing MINF body of at least 12 bytes",
            ));
        }

        let requested = choice.model_id();
        if current == requested {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        candidate[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
        let parsed = Self::parse(candidate)?;
        if parsed
            .models
            .get(model_index)
            .is_none_or(|model| model.model_id != requested || model.slot != STOMP_SLOT)
        {
            return Err(ToneError::format(
                "Stomp model candidate did not reparse to the requested model",
            ));
        }
        *self = parsed;
        Ok(())
    }

    /// Mod model id when the primary Mod slot `0x00020004` is present.
    pub fn mod_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == MOD_SLOT)
            .map(|model| model.model_id)
    }

    pub fn reverb_model_id(&self) -> Option<u32> {
        let matches: Vec<_> = self
            .models
            .iter()
            .filter(|model| matches!(model.slot, REVERB_SLOT | REVERB_FALLBACK_SLOT))
            .collect();
        (matches.len() == 1).then(|| matches[0].model_id)
    }

    /// Number of persisted primary and fallback Reverb model records.
    pub fn reverb_model_slot_count(&self) -> usize {
        self.models
            .iter()
            .filter(|model| matches!(model.slot, REVERB_SLOT | REVERB_FALLBACK_SLOT))
            .count()
    }

    /// Change only the MINF ID word of one existing primary or fallback Reverb.
    /// The input must satisfy the conservative PODxt profile and closed ID set.
    pub fn set_reverb_model_choice(
        &mut self,
        choice: PodReverbModelChoice,
    ) -> Result<(), ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "Reverb model selection requires a FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "Reverb model selection requires the PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "Reverb model selection requires the PODxt Live host profile minimum 3.00",
            ));
        }
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| matches!(model.slot, REVERB_SLOT | REVERB_FALLBACK_SLOT))
            .map(|(index, _)| index)
            .collect();
        let [model_index] = matches.as_slice() else {
            return Err(ToneError::format(
                "Reverb model selection requires exactly one existing primary or fallback model",
            ));
        };
        let model_index = *model_index;
        let model = &self.models[model_index];
        if !REVERB_CONTROL_MODEL_IDS.contains(&model.model_id) {
            return Err(ToneError::format(format!(
                "unknown current Reverb model id {:08x}",
                model.model_id
            )));
        }
        let slot = model.slot;
        let range = self
            .model_minfo_ranges
            .get(model_index)
            .cloned()
            .ok_or_else(|| ToneError::format("Reverb model MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "Reverb model selection requires an existing MINF body of at least 12 bytes",
            ));
        }
        let requested = choice.model_id();
        if model.model_id == requested {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        candidate[range.start..range.start + 4].copy_from_slice(&requested.to_be_bytes());
        let parsed = Self::parse(candidate)?;
        if parsed
            .models
            .get(model_index)
            .is_none_or(|model| model.model_id != requested || model.slot != slot)
        {
            return Err(ToneError::format(
                "Reverb model candidate did not reparse to the requested model",
            ));
        }
        *self = parsed;
        Ok(())
    }

    pub fn delay_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .map(|model| model.model_id)
    }

    /// Asset name for the amp model id, when the PODxt Live table has one.
    pub fn amp_asset_name(&self) -> Option<&'static str> {
        self.amp_model_id().and_then(pod_assets::asset_name)
    }

    /// Read one admitted Line6Edit host-scale amp control value (0..=1).
    /// This is not a claim about the value accepted by physical hardware.
    pub fn amp_control_fraction(&self, control: AmpControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.amp_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Amp {}", control.label()),
        )
    }

    /// Read the admitted PODxt Live Stomp Drive host-scale value (0..=1).
    /// This is not a claim about physical-device acceptance or effect sound.
    pub fn stomp_drive_fraction(&self) -> Result<f32, ToneError> {
        self.stomp_control_fraction(StompControl::Drive)
    }

    /// Read a source-backed Stomp host-scale value (0..=1) for the admitted model.
    pub fn stomp_control_fraction(&self, control: StompControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.stomp_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Stomp {}", control.label()),
        )
    }

    /// Read a source-backed Whammy knob position without quantizing or
    /// normalizing the imported word. The definition range is `0..=1`.
    pub fn stomp_whammy_knob_position(&self, control: StompWhammyKnob) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.stomp_whammy_knob_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Stomp Whammy {} knob position", control.label()),
        )
    }

    /// Read the existing Stomp SYNWAVE value without snapping unknown raw words.
    pub fn stomp_wave_selection(&self) -> Result<StompWaveValue, ToneError> {
        let (model_index, parameter_index) = self.stomp_wave_location()?;
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(
                "Stomp SYNWAVE target is not a float PARM",
            ));
        };
        Ok(StompWaveSelection::from_float_bits(*bits)
            .map(StompWaveValue::Known)
            .unwrap_or(StompWaveValue::Unknown { raw_bits: *bits }))
    }

    pub fn stomp_harmony_1m335_selection(
        &self,
    ) -> Result<StompHarmonyValue<StompHarmony1M335Selection>, ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1M335)?;
        let bits =
            self.delay_association_bits_at(model_index, parameter_index, "Stomp HARMONY_1M335")?;
        Ok(StompHarmony1M335Selection::from_float_bits(bits)
            .map(StompHarmonyValue::Known)
            .unwrap_or(StompHarmonyValue::Unknown { raw_bits: bits }))
    }

    pub fn stomp_harmony_1457_selection(
        &self,
    ) -> Result<StompHarmonyValue<StompHarmony1457Selection>, ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1457)?;
        let bits =
            self.delay_association_bits_at(model_index, parameter_index, "Stomp HARMONY_1457")?;
        Ok(StompHarmony1457Selection::from_float_bits(bits)
            .map(StompHarmonyValue::Known)
            .unwrap_or(StompHarmonyValue::Unknown { raw_bits: bits }))
    }

    /// Read one source-backed Mod percent value (host scale 0..=1).
    pub fn mod_control_fraction(&self, control: ModControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.mod_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Mod {}", control.label()),
        )
    }

    pub fn reverb_control_fraction(&self, control: ReverbControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.reverb_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Reverb {}", control.label()),
        )
    }

    pub fn delay_control_fraction(&self, control: DelayControl) -> Result<f32, ToneError> {
        let (model_index, parameter_index) = self.delay_control_location(control)?;
        self.float_control_fraction_at(
            model_index,
            parameter_index,
            &format!("Delay {}", control.label()),
        )
    }

    /// Read the finite HEADS association selection, preserving unrecognized
    /// float bits as `Unknown` without snapping them to a displayed choice.
    pub fn delay_heads_selection(
        &self,
    ) -> Result<DelayAssociationValue<DelayHeadsSelection>, ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_0005, 0x0005_0004, 0x0010_0002, "Delay Heads")?;
        let bits = self.delay_association_bits_at(model_index, parameter_index, "Delay Heads")?;
        Ok(DelayHeadsSelection::from_float_bits(bits)
            .map(DelayAssociationValue::Known)
            .unwrap_or(DelayAssociationValue::Unknown { raw_bits: bits }))
    }

    /// Read the finite BITS association selection, preserving unrecognized
    /// float bits as `Unknown` without snapping them to a displayed choice.
    pub fn delay_bits_selection(
        &self,
    ) -> Result<DelayAssociationValue<DelayBitsSelection>, ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_000d, 0x0005_0005, 0x0010_0003, "Delay Bits")?;
        let bits = self.delay_association_bits_at(model_index, parameter_index, "Delay Bits")?;
        Ok(DelayBitsSelection::from_float_bits(bits)
            .map(DelayAssociationValue::Known)
            .unwrap_or(DelayAssociationValue::Unknown { raw_bits: bits }))
    }

    /// Return the stored normalized frequency/gain words without snapping or
    /// rejecting finite values outside the source definition range.
    pub fn podxt_eq_band_raw_values(&self, band: PodxtEqBand) -> Result<(f32, f32), ToneError> {
        let (frequency_id, gain_id) = band.parameter_ids();
        let (model_index, frequency_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            frequency_id,
            &format!("EQ band {} frequency", band.label()),
        )?;
        let (_, gain_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            gain_id,
            &format!("EQ band {} gain", band.label()),
        )?;
        let read = |index: usize, label: &str| match self.models[model_index].parameters[index] {
            ModelParameter::Recorded {
                value: ParameterValue::Float { bits },
                ..
            } => Ok(f32::from_bits(bits)),
            _ => Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            ))),
        };
        Ok((
            read(frequency_index, "EQ frequency")?,
            read(gain_index, "EQ gain")?,
        ))
    }

    /// Source-style EQ display values for normalized words in the definition
    /// range. The frequency display uses the 127-step read conversion.
    pub fn podxt_eq_band_display_values(&self, band: PodxtEqBand) -> Result<(f32, f32), ToneError> {
        let (frequency, gain) = self.podxt_eq_band_raw_values(band)?;
        if !frequency.is_finite()
            || !(0.0..=1.0).contains(&frequency)
            || !gain.is_finite()
            || !(0.0..=1.0).contains(&gain)
        {
            return Err(ToneError::format(
                "EQ stored value is outside the finite definition range 0..=1",
            ));
        }
        let table = &PODXT_EQ_FREQUENCY_TABLES[band.index()];
        // Line6Edit promotes the stored float to double before forming the
        // 127-step controller key, then handles both endpoints explicitly.
        let key = (f64::from(frequency) * 127.0 + 0.5) as i32;
        let frequency_hz = if key <= 0 {
            table[0]
        } else if key >= 127 {
            table[8]
        } else {
            let point = (key >> 4) as usize;
            let fraction = (key & 0x0f) as f64 / 16.0;
            table[point] + (table[point + 1] - table[point]) * fraction
        } as f32;
        let width = PODXT_EQ_GAIN_MAX - PODXT_EQ_GAIN_MIN;
        let gain_db = if gain <= 0.0 {
            PODXT_EQ_GAIN_MIN
        } else if gain >= 1.0 {
            PODXT_EQ_GAIN_MAX
        } else {
            (f64::from(gain) * f64::from(width) + f64::from(PODXT_EQ_GAIN_MIN)) as f32
        };
        Ok((frequency_hz, gain_db))
    }

    /// Apply the original four-band EQ graph callback to one existing band.
    /// Inputs are graph display values in Hz and dB (both received as float32).
    /// The frequency and gain value words are committed together after every
    /// target, current value, and input has passed validation.
    pub fn set_podxt_eq_band(
        &mut self,
        band: PodxtEqBand,
        frequency_hz: f32,
        gain_db: f32,
    ) -> Result<(), ToneError> {
        let label = format!("EQ band {}", band.label());
        let (frequency_id, gain_id) = band.parameter_ids();
        let (model_index, frequency_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            frequency_id,
            &format!("{label} frequency"),
        )?;
        let (_, gain_index) = self.control_location(
            PODXT_EQ_SLOT,
            &[PODXT_EQ_MODEL_ID],
            gain_id,
            &format!("{label} gain"),
        )?;
        let current = |index: usize, control: &str| -> Result<(), ToneError> {
            let bits = match self.models[model_index].parameters[index] {
                ModelParameter::Recorded {
                    value: ParameterValue::Float { bits },
                    ..
                } => bits,
                _ => {
                    return Err(ToneError::format(format!(
                        "{label} {control} target is not a float PARM"
                    )));
                }
            };
            let value = f32::from_bits(bits);
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(ToneError::format(format!(
                    "existing {label} {control} value is outside the finite definition range 0..=1"
                )));
            }
            Ok(())
        };
        current(frequency_index, "frequency")?;
        current(gain_index, "gain")?;
        let table = &PODXT_EQ_FREQUENCY_TABLES[band.index()];
        if !frequency_hz.is_finite()
            || frequency_hz < table[0] as f32
            || frequency_hz > table[8] as f32
        {
            return Err(ToneError::format(format!(
                "{label} frequency must be finite and within {}..{} Hz",
                table[0], table[8]
            )));
        }
        if !gain_db.is_finite() || gain_db < PODXT_EQ_GAIN_MIN || gain_db > PODXT_EQ_GAIN_MAX {
            return Err(ToneError::format(format!(
                "{label} gain must be finite and within -12.8..12.6 dB"
            )));
        }
        let frequency = frequency_hz as f64;
        let segment = (0..8).find(|&i| frequency <= table[i + 1]).unwrap_or(7);
        let normalized_frequency = (((frequency - table[segment])
            / (table[segment + 1] - table[segment])
            + segment as f64)
            / 8.0) as f32;
        let gain_width = PODXT_EQ_GAIN_MAX - PODXT_EQ_GAIN_MIN;
        let normalized_gain = if gain_db <= PODXT_EQ_GAIN_MIN {
            0.0
        } else if gain_db >= PODXT_EQ_GAIN_MAX {
            1.0
        } else {
            ((gain_db as f64 - PODXT_EQ_GAIN_MIN as f64) / gain_width as f64) as f32
        };
        let frequency_bits = normalized_frequency.to_bits();
        let gain_bits = normalized_gain.to_bits();
        let frequency_offset = self.parameter_value_offsets[model_index][frequency_index]
            .ok_or_else(|| {
                ToneError::format(format!("{label} frequency has no unique raw value word"))
            })?;
        let gain_offset =
            self.parameter_value_offsets[model_index][gain_index].ok_or_else(|| {
                ToneError::format(format!("{label} gain has no unique raw value word"))
            })?;

        self.bytes[frequency_offset..frequency_offset + 4]
            .copy_from_slice(&frequency_bits.to_be_bytes());
        self.bytes[gain_offset..gain_offset + 4].copy_from_slice(&gain_bits.to_be_bytes());
        for (index, bits) in [(frequency_index, frequency_bits), (gain_index, gain_bits)] {
            if let ModelParameter::Recorded {
                value: ParameterValue::Float { bits: stored },
                ..
            } = &mut self.models[model_index].parameters[index]
            {
                *stored = bits;
            }
        }
        Ok(())
    }

    pub(super) fn podxt_eq_reset_targets(&self) -> Result<[(usize, u32); 8], ToneError> {
        if self.container != ContainerKind::L6pa {
            return Err(ToneError::format(
                "EQ reset requires an individual FORM/L6PA PODxt Live tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "EQ reset requires PODxt Live target 0x0003000a",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "EQ reset requires the PODxt Live host profile minimum 3.00",
            ));
        }

        let slot_models: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter_map(|(index, model)| (model.slot == PODXT_EQ_SLOT).then_some(index))
            .collect();
        let model_ids: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter_map(|(index, model)| (model.model_id == PODXT_EQ_MODEL_ID).then_some(index))
            .collect();
        let [model_index] = slot_models.as_slice() else {
            return Err(ToneError::format(
                "EQ reset requires exactly one existing fixed EQ slot",
            ));
        };
        if model_ids.as_slice() != [*model_index]
            || self.models[*model_index].model_id != PODXT_EQ_MODEL_ID
        {
            return Err(ToneError::format(
                "EQ reset requires exactly one EQ model 0x020c0002 in slot 0x00030004",
            ));
        }
        if self.model_minfo_ranges[*model_index].len() < 12 {
            return Err(ToneError::format(
                "EQ reset requires a parseable MINF body of at least 12 bytes",
            ));
        }
        if self.models[*model_index]
            .parameters
            .iter()
            .any(|parameter| matches!(parameter, ModelParameter::Opaque { .. }))
        {
            return Err(ToneError::format(
                "opaque or malformed PARM in EQ model makes reset ambiguous",
            ));
        }

        let mut targets = [(0usize, 0u32); 8];
        for (index, parameter_id) in (0x0010_0000..=0x0010_0007).enumerate() {
            let matches: Vec<_> = self.opaque_parameter_id_hints[*model_index]
                .iter()
                .enumerate()
                .filter_map(|(parameter_index, hint)| {
                    (*hint == Some(parameter_id)).then_some(parameter_index)
                })
                .collect();
            let [parameter_index] = matches.as_slice() else {
                return Err(ToneError::format(format!(
                    "EQ reset requires exactly one existing PARM 0x{parameter_id:08x}"
                )));
            };
            let parameter_index = *parameter_index;
            let ModelParameter::Recorded {
                prefix: 0x3f,
                id,
                value: ParameterValue::Float { bits },
            } = &self.models[*model_index].parameters[parameter_index]
            else {
                return Err(ToneError::format(format!(
                    "EQ reset PARM 0x{parameter_id:08x} must be canonical 0x3f/type-1 float"
                )));
            };
            if *id != parameter_id {
                return Err(ToneError::format(format!(
                    "EQ reset PARM 0x{parameter_id:08x} has an ambiguous raw identifier"
                )));
            }
            let value = f32::from_bits(*bits);
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(ToneError::format(format!(
                    "existing EQ PARM 0x{parameter_id:08x} is outside finite range 0..=1"
                )));
            }
            let value_offset = self.parameter_value_offsets[*model_index][parameter_index]
                .ok_or_else(|| {
                    ToneError::format(format!(
                        "EQ reset PARM 0x{parameter_id:08x} has no unique raw value word"
                    ))
                })?;
            let value_end = value_offset
                .checked_add(4)
                .ok_or_else(|| ToneError::format("EQ reset PARM raw value offset overflows"))?;
            if value_end > self.bytes.len()
                || self.bytes.get(value_offset..value_end) != Some(bits.to_be_bytes().as_slice())
            {
                return Err(ToneError::format(format!(
                    "EQ reset PARM 0x{parameter_id:08x} has an invalid raw value offset"
                )));
            }
            targets[index] = (value_offset, *bits);
        }
        Ok(targets)
    }

    /// Stable bytes for the current validated EQ model, used to invalidate a
    /// staged reset if that model or any of its opaque records changes.
    pub fn podxt_eq_reset_context(&self) -> Result<Vec<u8>, ToneError> {
        self.podxt_eq_reset_targets()?;
        let model_index = self
            .models
            .iter()
            .position(|model| model.slot == PODXT_EQ_SLOT)
            .ok_or_else(|| ToneError::format("EQ reset target disappeared"))?;
        let range = self.model_chunk_ranges[model_index].clone();
        self.bytes
            .get(range)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| ToneError::format("EQ reset model span is invalid"))
    }

    /// Atomically reset the four-band EQ in an existing PODxt Live L6PA tone.
    /// This is an offline file operation; it never sends an edit buffer.
    pub fn reset_podxt_eq(&mut self) -> Result<(), ToneError> {
        let targets = self.podxt_eq_reset_targets()?;
        let original = self.bytes.clone();
        let mut candidate_bytes = original.clone();
        for ((offset, _), word) in targets.iter().zip(PODXT_EQ_RESET_WORDS) {
            candidate_bytes[*offset..*offset + 4].copy_from_slice(&word.to_be_bytes());
        }
        if candidate_bytes == original {
            return Ok(());
        }

        let candidate = PodTone::parse(candidate_bytes)?;
        let candidate_targets = candidate.podxt_eq_reset_targets()?;
        if candidate_targets
            .iter()
            .zip(PODXT_EQ_RESET_WORDS)
            .any(|((_, actual), expected)| *actual != expected)
        {
            return Err(ToneError::format(
                "EQ reset candidate did not preserve all eight requested value words",
            ));
        }
        let allowed: Vec<_> = targets
            .iter()
            .flat_map(|(offset, _)| *offset..*offset + 4)
            .collect();
        if original
            .iter()
            .zip(candidate.as_bytes())
            .enumerate()
            .any(|(offset, (before, after))| before != after && !allowed.contains(&offset))
        {
            return Err(ToneError::format(
                "EQ reset candidate changed bytes outside the eight selected value words",
            ));
        }
        *self = candidate;
        Ok(())
    }

    /// Compatibility convenience for callers that only use Drive.
    pub fn amp_drive_fraction(&self) -> Result<f32, ToneError> {
        self.amp_control_fraction(AmpControl::Drive)
    }

    /// Change only the existing raw value word for the selected amp control.
    pub fn set_amp_control_fraction(
        &mut self,
        control: AmpControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Amp {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.amp_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    /// Change only the existing raw Stomp Drive value word.
    pub fn set_stomp_drive_fraction(&mut self, value: f32) -> Result<(), ToneError> {
        self.set_stomp_control_fraction(StompControl::Drive, value)
    }

    /// Change only the existing raw value word for an admitted Stomp control.
    pub fn set_stomp_control_fraction(
        &mut self,
        control: StompControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Stomp {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.stomp_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    /// Apply a finite normalized editor knob position through the original
    /// WHAMMY_INTERVAL float32 truncating quantizer. This is not a display-value
    /// parser and does not accept raw stored values.
    pub fn set_stomp_whammy_knob_position(
        &mut self,
        control: StompWhammyKnob,
        position: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Stomp Whammy {} knob position", control.label());
        Self::validate_host_fraction(position, &label)?;
        let (model_index, parameter_index) = self.stomp_whammy_knob_location(control)?;
        let current = match &self.models[model_index].parameters[parameter_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Float { bits },
                ..
            } => f32::from_bits(*bits),
            _ => {
                return Err(ToneError::format(format!(
                    "{label} target is not a float PARM"
                )));
            }
        };
        if !current.is_finite() || !(0.0..=1.0).contains(&current) {
            return Err(ToneError::format(format!(
                "existing {label} value is outside the finite definition range 0..=1"
            )));
        }
        let scaled = position * WHAMMY_INTERVAL_RESOLUTION;
        let steps = scaled as i32;
        let stored = (steps as f32) * WHAMMY_INTERVAL_RESOLUTION_RECIPROCAL;
        self.set_float_control_fraction_at(model_index, parameter_index, stored, &label)
    }

    /// Replace only the existing Stomp SYNWAVE value word with a typed choice.
    /// Unknown finite values may be replaced; non-finite stored values refuse mutation.
    pub fn set_stomp_wave_selection(
        &mut self,
        selection: StompWaveSelection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) = self.stomp_wave_location()?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Stomp SYNWAVE",
        )
    }

    pub fn set_stomp_harmony_1m335_selection(
        &mut self,
        selection: StompHarmony1M335Selection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1M335)?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Stomp HARMONY_1M335",
        )
    }

    pub fn set_stomp_harmony_1457_selection(
        &mut self,
        selection: StompHarmony1457Selection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.stomp_harmony_location(StompHarmonyControl::Harmony1457)?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Stomp HARMONY_1457",
        )
    }

    /// Change only the existing raw value word for a source-mapped Mod percent control.
    pub fn set_mod_control_fraction(
        &mut self,
        control: ModControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Mod {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.mod_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    pub fn set_reverb_control_fraction(
        &mut self,
        control: ReverbControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Reverb {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.reverb_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    pub fn set_delay_control_fraction(
        &mut self,
        control: DelayControl,
        value: f32,
    ) -> Result<(), ToneError> {
        let label = format!("Delay {}", control.label());
        Self::validate_host_fraction(value, &label)?;
        let (model_index, parameter_index) = self.delay_control_location(control)?;
        self.set_float_control_fraction_at(model_index, parameter_index, value, &label)
    }

    /// Read the stored Delay NOTE without normalizing unknown imported ordinals.
    pub fn delay_note_value(&self) -> Result<DelayNoteValue, ToneError> {
        let model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay NOTE",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0020_0000, false, "Delay NOTE")?;
        let bits = match self.models[model].parameters[parameter] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => {
                return Err(ToneError::format(
                    "Delay NOTE target is not an integer PARM",
                ));
            }
        };
        Ok(DelayNoteChoice::from_value(bits)
            .map(DelayNoteValue::Known)
            .unwrap_or(DelayNoteValue::Unknown { raw_bits: bits }))
    }

    /// Display the normalized Delay TIME as milliseconds for UI staging.
    pub fn delay_time_ms(&self) -> Result<f32, ToneError> {
        let model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay TIME",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0010_0000, true, "Delay TIME")?;
        let raw = self.float_parameter_bits(model, parameter, "Delay TIME")?;
        if !raw.is_finite() || !(0.0..=1.0).contains(&raw) {
            return Err(ToneError::format(
                "stored Delay TIME is outside finite raw range 0..=1",
            ));
        }
        let scale: f32 = 2000.0 - 20.0;
        Ok(20.0 + raw * scale)
    }

    pub fn mod_note_value(&self) -> Result<ModNoteValue, ToneError> {
        let model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod NOTE",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0020_0000, false, "Mod NOTE")?;
        let bits = match self.models[model].parameters[parameter] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        Ok(DelayNoteChoice::from_value(bits)
            .map(ModNoteValue::Known)
            .unwrap_or(ModNoteValue::Unknown { raw_bits: bits }))
    }

    pub fn mod_speed_hz(&self) -> Result<f32, ToneError> {
        let model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod SPEED",
        )?;
        let (parameter, _) =
            self.unique_control_parameter(model, 0x0010_0000, true, "Mod SPEED")?;
        let raw = self.float_parameter_bits(model, parameter, "Mod SPEED")?;
        if !raw.is_finite() || !(0.0..=1.0).contains(&raw) {
            return Err(ToneError::format(
                "stored Mod SPEED is outside finite raw range 0..=1",
            ));
        }
        Ok(0.1 + raw * (15.0_f32 - 0.1_f32))
    }

    /// Writes an existing Mod NOTE record. Positive choices recalculate SPEED
    /// from the source System Tempo; Off changes only NOTE. Delay NOTE must be
    /// Off because the original shared callback also recalculates Delay TIME.
    pub fn set_mod_note_choice(&mut self, choice: ModNoteChoice) -> Result<(), ToneError> {
        self.require_control_edit_profile("Mod NOTE")?;
        let mod_model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod NOTE",
        )?;
        let (note_index, note_offset) =
            self.unique_control_parameter(mod_model, 0x0020_0000, false, "Mod NOTE")?;
        let old_note = match self.models[mod_model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        if ModNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Mod NOTE is outside 0..=13"));
        }

        let delay_model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay NOTE cross-effect guard",
        )?;
        let (delay_note_index, _) = self.unique_control_parameter(
            delay_model,
            0x0020_0000,
            false,
            "Delay NOTE cross-effect guard",
        )?;
        let delay_note = match self.models[delay_model].parameters[delay_note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        if delay_note != 0 {
            return Err(ToneError::format(
                "Mod NOTE edit requires Delay NOTE Off to avoid a coupled Delay TIME write",
            ));
        }

        let speed_target = if choice != ModNoteChoice::Off {
            Some(self.unique_control_parameter(mod_model, 0x0010_0000, true, "Mod SPEED")?)
        } else {
            None
        };
        let speed_bits = if let Some((speed_index, _)) = speed_target {
            let old_speed = self.float_parameter_bits(mod_model, speed_index, "Mod SPEED")?;
            if !old_speed.is_finite() || !(0.0..=1.0).contains(&old_speed) {
                return Err(ToneError::format(
                    "stored Mod SPEED is outside finite raw range 0..=1",
                ));
            }
            let system_model =
                self.unique_control_model(&[0x0001_0000], &[0x3e70_0000], "System Tempo context")?;
            let (tempo_index, _) = self.unique_control_parameter(
                system_model,
                0x0020_0001,
                true,
                "System Tempo context",
            )?;
            let tempo =
                self.float_parameter_bits(system_model, tempo_index, "System Tempo context")?;
            if !tempo.is_finite() || !(30.0..=240.0).contains(&tempo) {
                return Err(ToneError::format(
                    "stored Tempo is outside finite BPM range 30..=240",
                ));
            }
            let multiplier = choice.multiplier().expect("positive Mod NOTE choice");
            let q = multiplier * 4.0_f32;
            let displayed_hz = (tempo / (q * 60.0_f32)).clamp(0.1, 15.0);
            Some(source_range_raw(displayed_hz, 0.1_f32, 15.0_f32).to_bits())
        } else {
            None
        };

        let current_speed_bits = speed_target
            .map(|(index, _)| self.float_parameter_bits(mod_model, index, "Mod SPEED"))
            .transpose()?
            .map(f32::to_bits);
        if old_note == choice.value() && speed_bits == current_speed_bits {
            return Ok(());
        }
        self.write_parameter_bits(mod_model, note_index, note_offset, choice.value());
        if let (Some(bits), Some((index, offset))) = (speed_bits, speed_target) {
            self.write_parameter_bits(mod_model, index, offset, bits);
        }
        Ok(())
    }

    /// Writes displayed Mod SPEED in Hz through the original range format and
    /// applies the source manual-speed event, which clears a positive NOTE.
    pub fn set_mod_speed_hz(&mut self, hz: f32) -> Result<(), ToneError> {
        self.require_control_edit_profile("Mod SPEED")?;
        if !hz.is_finite() || !(0.1..=15.0).contains(&hz) {
            return Err(ToneError::format(
                "Mod SPEED edit requires finite displayed Hz in 0.1..=15",
            ));
        }
        let model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod SPEED",
        )?;
        let (speed_index, speed_offset) =
            self.unique_control_parameter(model, 0x0010_0000, true, "Mod SPEED")?;
        let old_speed = self.float_parameter_bits(model, speed_index, "Mod SPEED")?;
        if !old_speed.is_finite() || !(0.0..=1.0).contains(&old_speed) {
            return Err(ToneError::format(
                "stored Mod SPEED is outside finite raw range 0..=1",
            ));
        }
        let (note_index, note_offset) =
            self.unique_control_parameter(model, 0x0020_0000, false, "Mod NOTE")?;
        let old_note = match self.models[model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => unreachable!("unique_control_parameter checked integer type"),
        };
        if ModNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Mod NOTE is outside 0..=13"));
        }
        let new_speed = source_range_raw(hz, 0.1_f32, 15.0_f32).to_bits();
        if old_speed.to_bits() == new_speed && old_note == 0 {
            return Ok(());
        }
        self.write_parameter_bits(model, speed_index, speed_offset, new_speed);
        if old_note != 0 {
            self.write_parameter_bits(model, note_index, note_offset, 0);
        }
        Ok(())
    }

    /// Apply a typed Delay NOTE choice to an existing target. Positive choices
    /// also update TIME using the original f32 NOTE formula and source range
    /// conversion. Off changes NOTE only. This operation never edits Mod.
    pub fn set_delay_note_choice(&mut self, choice: DelayNoteChoice) -> Result<(), ToneError> {
        self.require_control_edit_profile("Delay NOTE")?;
        let delay_model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay NOTE",
        )?;
        let (note_index, note_offset) =
            self.unique_control_parameter(delay_model, 0x0020_0000, false, "Delay NOTE")?;
        let old_note = match self.models[delay_model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => {
                return Err(ToneError::format(
                    "Delay NOTE target is not an integer PARM",
                ));
            }
        };
        if DelayNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Delay NOTE is outside 0..=13"));
        }

        let mod_model = self.unique_control_model(
            &[MOD_SLOT, 0x0005_0003],
            &MOD_CONTROL_MODEL_IDS,
            "Mod NOTE context",
        )?;
        let (mod_note_index, _) =
            self.unique_control_parameter(mod_model, 0x0020_0000, false, "Mod NOTE context")?;
        let mod_note = match self.models[mod_model].parameters[mod_note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => return Err(ToneError::format("Mod NOTE context is not an integer PARM")),
        };
        if mod_note != 0 {
            return Err(ToneError::format(
                "Delay NOTE edit requires Mod NOTE Off to avoid the coupled Mod SPEED write",
            ));
        }

        let delay_time_target = if choice != DelayNoteChoice::Off {
            Some(self.unique_control_parameter(delay_model, 0x0010_0000, true, "Delay TIME")?)
        } else {
            None
        };
        let derived_bits = if let Some((time_index, _)) = delay_time_target {
            let old_time = self.float_parameter_bits(delay_model, time_index, "Delay TIME")?;
            if !old_time.is_finite() || !(0.0..=1.0).contains(&old_time) {
                return Err(ToneError::format(
                    "stored Delay TIME is outside finite raw range 0..=1",
                ));
            }
            let system_model =
                self.unique_control_model(&[0x0001_0000], &[0x3e70_0000], "System Tempo context")?;
            let (tempo_index, _) = self.unique_control_parameter(
                system_model,
                0x0020_0001,
                true,
                "System Tempo context",
            )?;
            let tempo =
                self.float_parameter_bits(system_model, tempo_index, "System Tempo context")?;
            if !tempo.is_finite() || !(30.0..=240.0).contains(&tempo) {
                return Err(ToneError::format(
                    "stored Tempo is outside finite BPM range 30..=240",
                ));
            }
            Some(delay_note_time_raw(choice, tempo))
        } else {
            None
        };

        if old_note == choice.value()
            && derived_bits.is_none_or(|bits| {
                self.float_parameter_bits(delay_model, delay_time_target.unwrap().0, "Delay TIME")
                    .is_ok_and(|old| old.to_bits() == bits)
            })
        {
            return Ok(());
        }
        self.write_parameter_bits(delay_model, note_index, note_offset, choice.value());
        if let (Some(bits), Some((time_index, offset))) = (derived_bits, delay_time_target) {
            self.write_parameter_bits(delay_model, time_index, offset, bits);
        }
        Ok(())
    }

    /// Apply a displayed Delay TIME in milliseconds. The explicit TIME event
    /// clears a positive NOTE; it deliberately has no Mod or Tempo dependency.
    pub fn set_delay_time_ms(&mut self, milliseconds: f32) -> Result<(), ToneError> {
        self.require_control_edit_profile("Delay TIME")?;
        if !milliseconds.is_finite() || !(20.0..=2000.0).contains(&milliseconds) {
            return Err(ToneError::format(
                "Delay TIME edit requires finite displayed milliseconds in 20..=2000",
            ));
        }
        let delay_model = self.unique_control_model(
            &[DELAY_SLOT, 0x0005_0004],
            &DELAY_CONTROL_MODEL_IDS,
            "Delay TIME",
        )?;
        let (time_index, time_offset) =
            self.unique_control_parameter(delay_model, 0x0010_0000, true, "Delay TIME")?;
        let old_time = self.float_parameter_bits(delay_model, time_index, "Delay TIME")?;
        if !old_time.is_finite() || !(0.0..=1.0).contains(&old_time) {
            return Err(ToneError::format(
                "stored Delay TIME is outside finite raw range 0..=1",
            ));
        }
        let (note_index, note_offset) =
            self.unique_control_parameter(delay_model, 0x0020_0000, false, "Delay NOTE")?;
        let old_note = match self.models[delay_model].parameters[note_index] {
            ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } => bits,
            _ => {
                return Err(ToneError::format(
                    "Delay NOTE target is not an integer PARM",
                ));
            }
        };
        if DelayNoteChoice::from_value(old_note).is_none() {
            return Err(ToneError::format("stored Delay NOTE is outside 0..=13"));
        }
        let new_time = delay_time_raw(milliseconds);
        if old_time.to_bits() == new_time && old_note == 0 {
            return Ok(());
        }
        self.write_parameter_bits(delay_model, time_index, time_offset, new_time);
        if old_note != 0 {
            self.write_parameter_bits(delay_model, note_index, note_offset, 0);
        }
        Ok(())
    }

    fn require_control_edit_profile(&self, label: &str) -> Result<(), ToneError> {
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(format!(
                "{label} requires a PODxt Live PINF target"
            )));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(format!(
                "{label} requires the PODxt Live host profile minimum 3.00"
            )));
        }
        Ok(())
    }

    fn unique_control_model(
        &self,
        slots: &[u32],
        model_ids: &[u32],
        label: &str,
    ) -> Result<usize, ToneError> {
        self.require_control_edit_profile(label)?;
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, model)| slots.contains(&model.slot))
            .map(|(index, _)| index)
            .collect();
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "{label} requires exactly one model across slots {slots:02x?}"
            )));
        };
        let index = *index;
        if !model_ids.contains(&self.models[index].model_id) {
            return Err(ToneError::format(format!(
                "{label} model is outside the source-backed host profile"
            )));
        }
        if self.models[index]
            .parameters
            .iter()
            .any(|parameter| matches!(parameter, ModelParameter::Opaque { .. }))
        {
            return Err(ToneError::format(format!(
                "opaque PARM in {label} model makes control context ambiguous"
            )));
        }
        Ok(index)
    }

    fn unique_control_parameter(
        &self,
        model: usize,
        id: u32,
        float: bool,
        label: &str,
    ) -> Result<(usize, usize), ToneError> {
        let matches: Vec<_> = self.models[model]
            .parameters
            .iter()
            .enumerate()
            .filter_map(|(index, parameter)| {
                matches!(parameter, ModelParameter::Recorded { id: found, .. } if *found == id)
                    .then_some(index)
            })
            .collect();
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(format!(
                "{label} requires exactly one PARM 0x{id:06x}"
            )));
        };
        let index = *index;
        let ModelParameter::Recorded { prefix, value, .. } = &self.models[model].parameters[index]
        else {
            unreachable!()
        };
        if *prefix != 0x3f {
            return Err(ToneError::format(format!(
                "{label} target prefix is not canonical 0x3f"
            )));
        }
        let type_ok = if float {
            matches!(value, ParameterValue::Float { .. })
        } else {
            matches!(value, ParameterValue::Int { .. })
        };
        if !type_ok {
            return Err(ToneError::format(format!(
                "{label} target has an unsupported value type"
            )));
        }
        let offset = self
            .parameter_value_offsets
            .get(model)
            .and_then(|offsets| offsets.get(index))
            .copied()
            .flatten()
            .ok_or_else(|| {
                ToneError::format(format!("{label} target has no unique raw value word"))
            })?;
        Ok((index, offset))
    }

    fn float_parameter_bits(
        &self,
        model: usize,
        parameter: usize,
        label: &str,
    ) -> Result<f32, ToneError> {
        match self.models[model].parameters[parameter] {
            ModelParameter::Recorded {
                value: ParameterValue::Float { bits },
                ..
            } => Ok(f32::from_bits(bits)),
            _ => Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            ))),
        }
    }

    fn write_parameter_bits(&mut self, model: usize, parameter: usize, offset: usize, bits: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
        if let ModelParameter::Recorded { value, .. } =
            &mut self.models[model].parameters[parameter]
        {
            match value {
                ParameterValue::Int { bits: stored } | ParameterValue::Float { bits: stored } => {
                    *stored = bits
                }
                ParameterValue::Other { .. } => unreachable!("validated parameter type"),
            }
        }
    }

    /// Write only the existing HEADS PARM value word using a finite registered
    /// choice. This discrete operation intentionally does not use the percent
    /// scale setter.
    pub fn set_delay_heads_selection(
        &mut self,
        selection: DelayHeadsSelection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_0005, 0x0005_0004, 0x0010_0002, "Delay Heads")?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Delay Heads",
        )
    }

    /// Write only the existing BITS PARM value word using a finite registered
    /// choice. This discrete operation intentionally does not use the percent
    /// scale setter.
    pub fn set_delay_bits_selection(
        &mut self,
        selection: DelayBitsSelection,
    ) -> Result<(), ToneError> {
        let (model_index, parameter_index) =
            self.delay_association_location(0x0202_000d, 0x0005_0005, 0x0010_0003, "Delay Bits")?;
        self.set_association_bits_at(
            model_index,
            parameter_index,
            selection.float_bits(),
            "Delay Bits",
        )
    }

    /// Compatibility convenience for callers that only use Drive.
    pub fn set_amp_drive_fraction(&mut self, value: f32) -> Result<(), ToneError> {
        self.set_amp_control_fraction(AmpControl::Drive, value)
    }

    pub(super) fn amp_control_location(
        &self,
        control: AmpControl,
    ) -> Result<(usize, usize), ToneError> {
        self.control_location(
            AMP_SLOT,
            &AMP_CONTROL_MODEL_IDS,
            control.param_id(),
            &format!("Amp {}", control.label()),
        )
    }

    fn stomp_control_location(&self, control: StompControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Stomp {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(STOMP_SLOT, &[model_id], parameter_id, &label)
    }

    pub(super) fn stomp_whammy_knob_location(
        &self,
        control: StompWhammyKnob,
    ) -> Result<(usize, usize), ToneError> {
        let label = format!("Stomp Whammy {} knob position", control.label());
        if self
            .models
            .iter()
            .filter(|model| model.slot == STOMP_SLOT)
            .count()
            != 1
        {
            return Err(ToneError::format(format!(
                "{label} requires one unambiguous existing Stomp slot"
            )));
        }
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing Stomp slot")))?;
        if model_id != STOMP_WHAMMY_INTERVAL_MODEL_ID {
            return Err(ToneError::format(format!(
                "{label} applies only to Stomp model 0x{STOMP_WHAMMY_INTERVAL_MODEL_ID:08x}"
            )));
        }
        let expected_param = match control {
            StompWhammyKnob::Heel => STOMP_WHAMMY_HEEL_PARAM,
            StompWhammyKnob::Toe => STOMP_WHAMMY_TOE_PARAM,
        };
        debug_assert_eq!(control.param_id(), expected_param);
        self.control_location(
            STOMP_SLOT,
            &[STOMP_WHAMMY_INTERVAL_MODEL_ID],
            expected_param,
            &label,
        )
    }

    fn mod_control_location(&self, control: ModControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Mod {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == MOD_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(MOD_SLOT, &[model_id], parameter_id, &label)
    }

    fn reverb_control_location(&self, control: ReverbControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Reverb {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == REVERB_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(REVERB_SLOT, &[model_id], parameter_id, &label)
    }

    fn delay_control_location(&self, control: DelayControl) -> Result<(usize, usize), ToneError> {
        let label = format!("Delay {}", control.label());
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let parameter_id = control.param_id(model_id).ok_or_else(|| {
            ToneError::format(format!("{label} is not defined for model 0x{model_id:08x}"))
        })?;
        self.control_location(DELAY_SLOT, &[model_id], parameter_id, &label)
    }

    fn delay_association_location(
        &self,
        expected_model_id: u32,
        selector_id: u32,
        parameter_id: u32,
        label: &str,
    ) -> Result<(usize, usize), ToneError> {
        if self
            .models
            .iter()
            .filter(|model| model.slot == DELAY_SLOT)
            .count()
            != 1
        {
            return Err(ToneError::format(format!(
                "{label} requires one unambiguous existing model slot"
            )));
        }
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == DELAY_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        let expected_selector = match expected_model_id {
            0x0202_0005 => 0x0005_0004,
            0x0202_000d => 0x0005_0005,
            _ => 0,
        };
        let expected_parameter = match expected_model_id {
            0x0202_0005 => 0x0010_0002,
            0x0202_000d => 0x0010_0003,
            _ => 0,
        };
        let compatible_model = model_id == expected_model_id
            || (expected_model_id, model_id) == (0x0202_0005, 0x0202_000c);
        if !compatible_model
            || selector_id != expected_selector
            || parameter_id != expected_parameter
        {
            return Err(ToneError::format(format!(
                "{label} source mapping does not apply to model 0x{model_id:08x}"
            )));
        }
        self.control_location(DELAY_SLOT, &[model_id], parameter_id, label)
    }

    fn stomp_wave_location(&self) -> Result<(usize, usize), ToneError> {
        let slot_count = self
            .models
            .iter()
            .filter(|model| model.slot == STOMP_SLOT)
            .count();
        if slot_count != 1 {
            return Err(ToneError::format(
                "Stomp SYNWAVE requires one unambiguous existing model slot",
            ));
        }
        let model_id = self
            .models
            .iter()
            .find(|model| model.slot == STOMP_SLOT)
            .map(|model| model.model_id)
            .ok_or_else(|| ToneError::format("Stomp SYNWAVE requires an existing model slot"))?;
        if !STOMP_SYNTH_WAVE_MODEL_IDS.contains(&model_id) {
            return Err(ToneError::format(format!(
                "Stomp SYNWAVE source mapping does not apply to model 0x{model_id:08x}"
            )));
        }
        self.control_location(
            STOMP_SLOT,
            &[model_id],
            STOMP_SYNTH_WAVE_PARAM,
            "Stomp SYNWAVE",
        )
    }

    fn stomp_harmony_location(
        &self,
        control: StompHarmonyControl,
    ) -> Result<(usize, usize), ToneError> {
        let model_index = self
            .models
            .iter()
            .position(|model| model.slot == STOMP_SLOT)
            .ok_or_else(|| ToneError::format("Stomp harmony requires an existing model slot"))?;
        if self
            .models
            .iter()
            .filter(|model| model.slot == STOMP_SLOT)
            .count()
            != 1
            || self.models[model_index].model_id != 0x020a_0009
        {
            return Err(ToneError::format(
                "Stomp harmony requires one existing model 0x020a0009 slot",
            ));
        }
        let parameter_id = control.parameter_id();
        let target_count = self.models[model_index]
            .parameters
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter,
                    ModelParameter::Recorded { id, .. } if *id == parameter_id
                )
            })
            .count();
        if target_count != 1 {
            return Err(ToneError::format(format!(
                "Stomp harmony requires exactly one existing PARM 0x{parameter_id:08x}"
            )));
        }
        self.control_location(STOMP_SLOT, &[0x020a_0009], parameter_id, "Stomp harmony")
    }

    fn delay_association_bits_at(
        &self,
        model_index: usize,
        parameter_index: usize,
        label: &str,
    ) -> Result<u32, ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        Ok(*bits)
    }

    fn set_association_bits_at(
        &mut self,
        model_index: usize,
        parameter_index: usize,
        bits: u32,
        label: &str,
    ) -> Result<(), ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float {
                bits: existing_bits,
            },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        if !f32::from_bits(*existing_bits).is_finite() {
            return Err(ToneError::format(format!(
                "{label} cannot replace a non-finite stored association value"
            )));
        }
        let offset = self.parameter_value_offsets[model_index][parameter_index]
            .ok_or_else(|| ToneError::format(format!("{label} has no unique raw value word")))?;
        self.bytes[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
        if let ModelParameter::Recorded {
            value: ParameterValue::Float { bits: value_bits },
            ..
        } = &mut self.models[model_index].parameters[parameter_index]
        {
            *value_bits = bits;
        }
        Ok(())
    }

    fn control_location(
        &self,
        slot: u32,
        model_ids: &[u32],
        parameter_id: u32,
        label: &str,
    ) -> Result<(usize, usize), ToneError> {
        if self.device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::format(
                "control editing requires a PODxt Live PINF target",
            ));
        }
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "tone version is below the PODxt Live host profile minimum 3.00",
            ));
        }
        let model_index = self
            .models
            .iter()
            .position(|model| model.slot == slot)
            .ok_or_else(|| ToneError::format(format!("{label} requires an existing model slot")))?;
        if !model_ids.contains(&self.models[model_index].model_id) {
            return Err(ToneError::format(format!(
                "{label} host contract does not apply to this model"
            )));
        }
        if self.models[model_index]
            .parameters
            .iter()
            .any(|parameter| matches!(parameter, ModelParameter::Opaque { .. }))
        {
            return Err(ToneError::format(format!(
                "opaque PARM in {label} model makes control target ambiguous"
            )));
        }
        let matches: Vec<_> = self.models[model_index]
            .parameters
            .iter()
            .enumerate()
            .filter_map(|(index, parameter)| match parameter {
                ModelParameter::Recorded { id, .. } if *id == parameter_id => Some(index),
                _ => None,
            })
            .collect();
        let parameter_index = matches
            .last()
            .copied()
            .ok_or_else(|| ToneError::format(format!("{label} parameter is absent")))?;
        match &self.models[model_index].parameters[parameter_index] {
            ModelParameter::Recorded {
                prefix: 0x3f,
                value: ParameterValue::Float { .. },
                ..
            } => {}
            ModelParameter::Recorded { prefix, .. } if *prefix != 0x3f => {
                return Err(ToneError::format(format!(
                    "{label} raw prefix is not canonical 0x3f"
                )));
            }
            _ => {
                return Err(ToneError::format(format!(
                    "{label} target has an unsupported value type"
                )));
            }
        }
        if self.parameter_value_offsets[model_index][parameter_index].is_none() {
            return Err(ToneError::format(format!(
                "{label} target has no unique raw value word"
            )));
        }
        Ok((model_index, parameter_index))
    }

    fn validate_host_fraction(value: f32, label: &str) -> Result<(), ToneError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ToneError::format(format!(
                "{label} requires a finite host-scale value in 0..=1"
            )));
        }
        Ok(())
    }

    fn float_control_fraction_at(
        &self,
        model_index: usize,
        parameter_index: usize,
        label: &str,
    ) -> Result<f32, ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        let value = f32::from_bits(*bits);
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ToneError::format(format!(
                "{label} value is outside the finite host scale 0..=1"
            )));
        }
        Ok(value)
    }

    fn set_float_control_fraction_at(
        &mut self,
        model_index: usize,
        parameter_index: usize,
        value: f32,
        label: &str,
    ) -> Result<(), ToneError> {
        let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &self.models[model_index].parameters[parameter_index]
        else {
            return Err(ToneError::format(format!(
                "{label} target is not a float PARM"
            )));
        };
        let current = f32::from_bits(*bits);
        if !current.is_finite() || !(0.0..=1.0).contains(&current) {
            return Err(ToneError::format(format!(
                "existing {label} value is outside the finite host scale 0..=1"
            )));
        }
        let offset = self.parameter_value_offsets[model_index][parameter_index]
            .ok_or_else(|| ToneError::format(format!("{label} has no unique raw value word")))?;
        let new_bits = value.to_bits();
        self.bytes[offset..offset + 4].copy_from_slice(&new_bits.to_be_bytes());
        if let ModelParameter::Recorded {
            value: ParameterValue::Float { bits },
            ..
        } = &mut self.models[model_index].parameters[parameter_index]
        {
            *bits = new_bits;
        }
        Ok(())
    }

    /// Cab model id exposed by the existing L6AS API.
    pub fn cab_model_id(&self) -> Option<u32> {
        if !self.container.exposes_cab() {
            return None;
        }
        self.models
            .iter()
            .find(|model| model.slot == CAB_SLOT)
            .map(|model| model.model_id)
    }

    /// CAB model id for the narrowly admitted offline CAB/MIC editor.
    /// This adds the exact L6PA edit path without broadening `exposes_cab()`.
    pub fn cab_edit_model_id(&self) -> Option<u32> {
        matches!(self.container, ContainerKind::L6as | ContainerKind::L6pa)
            .then(|| {
                self.models
                    .iter()
                    .find(|model| model.slot == CAB_SLOT)
                    .map(|model| model.model_id)
            })
            .flatten()
    }

    /// CAB class for the offline CAB/MIC editor; unknown current IDs stay unknown.
    pub fn cab_edit_class(&self) -> Option<PodCabClass> {
        let id = self.cab_edit_model_id()?;
        if PODXT_BASS_CAB_MODEL_IDS.contains(&id) {
            Some(PodCabClass::Bass)
        } else if PODXT_CAB_MODEL_IDS.contains(&id) {
            Some(PodCabClass::Other)
        } else {
            None
        }
    }

    /// Whether the complete strict L6PA CAB/MIC context passes edit preflight.
    pub fn cab_mic_editable(&self) -> bool {
        if self.container == ContainerKind::L6pa {
            self.l6pa_mic_location().is_ok()
        } else {
            self.container == ContainerKind::L6as && self.cab_edit_location().is_ok()
        }
    }

    fn cab_edit_location(&self) -> Result<(usize, Range<usize>), ToneError> {
        if !matches!(self.container, ContainerKind::L6as | ContainerKind::L6pa) {
            return Err(ToneError::format(
                "CAB/MIC editing requires a FORM/L6AS or admitted FORM/L6PA tone",
            ));
        }
        if self.device_id != PODXT_LIVE_TONE_ID || self.device_version < PODXT_LIVE_HOST_MIN_VERSION
        {
            return Err(ToneError::format(
                "CAB/MIC editing requires the PODxt Live target and host profile",
            ));
        }
        if self.container == ContainerKind::L6pa && self.pinf_data_len != 76 {
            return Err(ToneError::format(
                "L6PA CAB/MIC editing requires one complete 76-byte PINF body",
            ));
        }
        let matches: Vec<_> = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, m)| m.slot == CAB_SLOT)
            .map(|(i, _)| i)
            .collect();
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(
                "CAB/MIC edit requires exactly one existing CAB slot",
            ));
        };
        let index = *index;
        let range = self
            .model_minfo_ranges
            .get(index)
            .cloned()
            .ok_or_else(|| ToneError::format("CAB MINF source range is unavailable"))?;
        if range.end > self.bytes.len()
            || (self.container == ContainerKind::L6pa && range.len() != 12)
            || range
                .start
                .checked_add(12)
                .is_none_or(|end| end > range.end)
        {
            return Err(ToneError::format(
                "CAB edit requires an existing MINF body of at least 12 bytes",
            ));
        }
        if self.container == ContainerKind::L6pa {
            let current = self.models[index].model_id;
            if !PODXT_CAB_MODEL_IDS.contains(&current)
                && !PODXT_BASS_CAB_MODEL_IDS.contains(&current)
            {
                return Err(ToneError::format(
                    "L6PA CAB/MIC editing requires a source-listed current CAB id",
                ));
            }
        }
        Ok((index, range))
    }

    fn l6pa_mic_location(&self) -> Result<(usize, usize), ToneError> {
        let (model_index, _) = self.cab_edit_location()?;
        if self.container != ContainerKind::L6pa {
            let mic_index = self.l6as_mic_index(model_index)?;
            return Ok((model_index, mic_index));
        }
        let hints = self
            .opaque_parameter_id_hints
            .get(model_index)
            .ok_or_else(|| ToneError::format("CAB parameter index is unavailable"))?;
        let parameters = self
            .models
            .get(model_index)
            .ok_or_else(|| ToneError::format("CAB model is unavailable"))?
            .parameters
            .as_slice();
        let mut matches = Vec::new();
        for (index, (hint, parameter)) in hints.iter().zip(parameters).enumerate() {
            match (hint, parameter) {
                (
                    Some(PODXT_MIC_PARAM),
                    ModelParameter::Recorded {
                        prefix: 0x3f,
                        value: ParameterValue::Int { bits },
                        ..
                    },
                ) if *bits <= 3 => matches.push(index),
                (Some(PODXT_MIC_PARAM), _) => {
                    return Err(ToneError::format(
                        "L6PA MIC requires one canonical 12-byte integer PARM id 0 with value 0..=3",
                    ));
                }
                (None, ModelParameter::Opaque { .. }) => {
                    return Err(ToneError::format(
                        "L6PA CAB contains an opaque PARM whose identity cannot be proven unique",
                    ));
                }
                _ => {}
            }
        }
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(
                "L6PA CAB/MIC editing requires exactly one canonical existing MIC PARM id 0",
            ));
        };
        Ok((model_index, *index))
    }

    fn l6as_mic_index(&self, model_index: usize) -> Result<usize, ToneError> {
        let matches: Vec<_> = self.opaque_parameter_id_hints[model_index]
            .iter()
            .enumerate()
            .filter_map(|(i, id)| (*id == Some(PODXT_MIC_PARAM)).then_some(i))
            .collect();
        let [index] = matches.as_slice() else {
            return Err(ToneError::format(
                "MIC edit requires exactly one recorded PARM id 0",
            ));
        };
        Ok(*index)
    }

    fn commit_l6pa_word(
        &mut self,
        offset: usize,
        value: u32,
        cab_word: bool,
    ) -> Result<(), ToneError> {
        let end = offset
            .checked_add(4)
            .ok_or_else(|| ToneError::format("edit word offset overflow"))?;
        if end > self.bytes.len() {
            return Err(ToneError::format("edit word exceeds tone bytes"));
        }
        let old_cab = self.cab_edit_model_id();
        let old_mic = self.mic_choice();
        let original = self.bytes.clone();
        let mut candidate = original.clone();
        candidate[offset..end].copy_from_slice(&value.to_be_bytes());
        let verified = PodTone::parse(candidate)?;
        if verified.bytes.len() != original.len()
            || verified
                .bytes
                .iter()
                .enumerate()
                .any(|(index, byte)| !(offset..end).contains(&index) && *byte != original[index])
        {
            return Err(ToneError::format(
                "L6PA edit changed bytes outside its selected word",
            ));
        }
        if cab_word {
            if verified.cab_edit_model_id() != Some(value) || verified.mic_choice() != old_mic {
                return Err(ToneError::format(
                    "L6PA CAB postcondition or raw MIC preservation failed",
                ));
            }
        } else if verified.cab_edit_model_id() != old_cab
            || verified.mic_choice().map(PodMicChoice::value) != Some(value)
        {
            return Err(ToneError::format("L6PA MIC postcondition failed"));
        }
        *self = verified;
        Ok(())
    }

    /// Apply one source-admitted unconditional CAB id to the existing L6AS slot.
    pub fn set_cab_model_choice(&mut self, choice: PodCabModelChoice) -> Result<(), ToneError> {
        let (index, range) = self.cab_edit_location()?;
        let id = choice.model_id();
        if self.container == ContainerKind::L6pa {
            let _ = self.l6pa_mic_location()?;
            return self.commit_l6pa_word(range.start, id, true);
        }
        if self.models[index].model_id != id {
            self.bytes[range.start..range.start + 4].copy_from_slice(&id.to_be_bytes());
            self.models[index].model_id = id;
        }
        Ok(())
    }

    /// Classify the stored CAB using the source's exact bass list.
    pub fn cab_class(&self) -> Option<PodCabClass> {
        let id = self.cab_model_id()?;
        (PODXT_CAB_MODEL_IDS.contains(&id) || PODXT_BASS_CAB_MODEL_IDS.contains(&id)).then_some(
            if PODXT_BASS_CAB_MODEL_IDS.contains(&id) {
                PodCabClass::Bass
            } else {
                PodCabClass::Other
            },
        )
    }

    /// Return a currently interpretable MIC integer; unknown raw integers remain preserved.
    pub fn mic_choice(&self) -> Option<PodMicChoice> {
        let (mi, _) = self.cab_edit_location().ok()?;
        if self.container == ContainerKind::L6pa {
            let (model_index, parameter_index) = self.l6pa_mic_location().ok()?;
            let ModelParameter::Recorded {
                value: ParameterValue::Int { bits },
                ..
            } = self.models[model_index].parameters.get(parameter_index)?
            else {
                return None;
            };
            return PodMicChoice::from_value(*bits);
        }
        let cab_id = self.models[mi].model_id;
        if !PODXT_CAB_MODEL_IDS.contains(&cab_id) && !PODXT_BASS_CAB_MODEL_IDS.contains(&cab_id) {
            return None;
        }
        let matches: Vec<_> = self.opaque_parameter_id_hints[mi]
            .iter()
            .enumerate()
            .filter_map(|(i, id)| (*id == Some(PODXT_MIC_PARAM)).then_some(i))
            .collect();
        let [pi] = matches.as_slice() else {
            return None;
        };
        match &self.models[mi].parameters[*pi] {
            ModelParameter::Recorded {
                prefix: 0x3f,
                value: ParameterValue::Int { bits },
                ..
            } => PodMicChoice::from_value(*bits),
            _ => None,
        }
    }

    /// Explicitly apply one MIC value. CAB selector changes never call this operation.
    pub fn set_mic_choice(&mut self, choice: PodMicChoice) -> Result<(), ToneError> {
        if self.container == ContainerKind::L6pa {
            let (model_index, parameter_index) = self.l6pa_mic_location()?;
            let offset = self.parameter_value_offsets[model_index][parameter_index]
                .ok_or_else(|| ToneError::format("L6PA MIC PARM has no unique raw value word"))?;
            return self.commit_l6pa_word(offset, choice.value(), false);
        }
        let (mi, _) = self.cab_edit_location()?;
        let id = self.models[mi].model_id;
        if !PODXT_CAB_MODEL_IDS.contains(&id) && !PODXT_BASS_CAB_MODEL_IDS.contains(&id) {
            return Err(ToneError::format(
                "MIC editing requires a source-listed current CAB id",
            ));
        }
        let matches: Vec<_> = self.opaque_parameter_id_hints[mi]
            .iter()
            .enumerate()
            .filter_map(|(i, id)| (*id == Some(PODXT_MIC_PARAM)).then_some(i))
            .collect();
        let [pi] = matches.as_slice() else {
            return Err(ToneError::format(
                "MIC edit requires exactly one recorded PARM id 0",
            ));
        };
        let pi = *pi;
        match &self.models[mi].parameters[pi] {
            ModelParameter::Recorded {
                prefix: 0x3f,
                value: ParameterValue::Int { .. },
                ..
            } => {}
            _ => {
                return Err(ToneError::format(
                    "MIC edit requires a canonical integer PARM",
                ));
            }
        }
        let offset = self.parameter_value_offsets[mi][pi]
            .ok_or_else(|| ToneError::format("MIC PARM has no unique value word"))?;
        let value = choice.value();
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        if let ModelParameter::Recorded {
            value: ParameterValue::Int { bits },
            ..
        } = &mut self.models[mi].parameters[pi]
        {
            *bits = value;
        }
        Ok(())
    }

    /// Replace the fixed 32 UTF-16BE PINF name units. No other byte changes.
    pub fn rename(&mut self, name: &str) -> Result<(), ToneError> {
        let units: Vec<u16> = name.encode_utf16().collect();
        if units.len() > 32 || units.contains(&0) {
            return Err(ToneError::format(
                "name must contain at most 32 UTF-16 code units and no NUL",
            ));
        }
        self.bytes[self.name_offset..self.name_offset + 64].fill(0);
        for (index, unit) in units.iter().enumerate() {
            let at = self.name_offset + 2 * index;
            self.bytes[at..at + 2].copy_from_slice(&unit.to_be_bytes());
        }
        self.name = name.to_owned();
        Ok(())
    }
}
