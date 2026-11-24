use super::*;
use crate::shared::file::read_bounded_file;

/// Static Edit 2.27 file framing. The tag is an HD500x editor-group
/// observation; it does not prove a physical target or profile compatibility.
pub const H5EP_HEADER_BYTES: usize = 0x28;
pub const H5EP_BODY_BYTES: usize = 0x1000;
pub const H5EP_MIN_BYTES: usize = H5EP_HEADER_BYTES + H5EP_BODY_BYTES;
pub const H5ES_HEADER_BYTES: usize = 0x38;
pub const H5ES_PRESET_COUNT: usize = 64;
pub const H5ES_MIN_BYTES: usize = H5ES_HEADER_BYTES + H5ES_PRESET_COUNT * H5EP_MIN_BYTES;
pub const H5EB_HEADER_BYTES: usize = 0x28;
pub const H5EB_SETLIST_COUNT: usize = 8;
pub const H5EB_MIN_BYTES: usize = H5EB_HEADER_BYTES + H5EB_SETLIST_COUNT * H5ES_MIN_BYTES;
const HD_PRESET_BYTES_CEILING: usize = 1_048_576;
const HD_SETLIST_BYTES_CEILING: usize = 1_048_576;
pub const HD_BUNDLE_BYTES_CEILING: usize = 3 * 1024 * 1024;
pub(super) const HD_EFFECT_BLOCK_OFFSET: usize = 0x428;
pub(super) const HD_EFFECT_BLOCK_STRIDE: usize = 0x100;
pub(super) const HD_EFFECT_COUNT: usize = 8;
pub(super) const HD_EFFECT_ROWS_OFFSET: usize = 0x10;
pub(super) const HD_PARAMETER_ROW_STRIDE: usize = 0x14;
pub(super) const HD_PARAMETER_ROW_FLAG_OFFSET: usize = 0x10;
/// A statically framed H5EP record. Header words other than magic/version and
/// all body data remain raw unless a narrowly evidenced control is edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueH5epPreset {
    pub(super) bytes: Vec<u8>,
}

/// A complete editor setlist with exactly 64 fixed-size H5EP children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct H5esSetlist {
    bytes: Vec<u8>,
}

/// A complete editor bundle with exactly eight fixed-size H5ES children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct H5ebBundle {
    bytes: Vec<u8>,
}

impl OpaqueH5epPreset {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_PRESET_BYTES_CEILING {
            return Err(ToneError::format(
                "H5EP exceeds the offline file-size ceiling",
            ));
        }
        validate_h5ep_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn header_bytes(&self) -> &[u8] {
        &self.bytes[..H5EP_HEADER_BYTES]
    }

    /// Raw H5EP header words at offsets 0x10 and 0x14. The Mac Editor 2.27
    /// writer sources them from `Memory + 0x18/+0x1c`, but a file alone does
    /// not establish which editor wrote them or whether they identify a
    /// physical target.
    pub fn header_words_10_14(&self) -> (u32, u32) {
        (
            u32::from_be_bytes(self.bytes[0x10..0x14].try_into().unwrap()),
            u32::from_be_bytes(self.bytes[0x14..0x18].try_into().unwrap()),
        )
    }

    pub fn body_bytes(&self) -> &[u8] {
        &self.bytes[H5EP_HEADER_BYTES..H5EP_MIN_BYTES]
    }

    /// Returns the editor's in-memory byte order using the statically traced
    /// selective word transform. Unmapped bytes are copied verbatim.
    pub fn body_in_editor_order(&self) -> Vec<u8> {
        let mut body = self.body_bytes().to_vec();
        transform_hd_body_words(&mut body);
        body
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5EP_MIN_BYTES..]
    }

    /// The Edit 2.27 static source maps this control to a normalized float in
    /// the first matching effect row. The result is editor data, not a claim
    /// of physical-device availability.
    pub fn screamer_drive_percent(&self, effect_slot: usize) -> Option<u8> {
        self.screamer_control_percent(effect_slot, ScreamerControl::Drive)
    }

    /// Reads a statically joined Screamer type-1 control from its first keyed
    /// row when the row assignment state is admitted.
    pub fn screamer_control_percent(
        &self,
        effect_slot: usize,
        control: ScreamerControl,
    ) -> Option<u8> {
        self.hd_type1_control_percent(effect_slot, HdType1Control::Screamer(control))
    }

    pub fn hd_type1_control_percent(
        &self,
        effect_slot: usize,
        control: HdType1Control,
    ) -> Option<u8> {
        let value_at = self.hd_type1_control_value_offset(effect_slot, control)?;
        let value = f32::from_bits(read_be_u32(self.body_bytes(), value_at)?);
        exact_drive_percent(value)
    }

    /// Reads a selected source-joined delay-family type-1 control.
    pub fn hd_type1_mapped_control_percent(
        &self,
        effect_slot: usize,
        control: HdType1MappedControl,
    ) -> Option<u8> {
        let value_at = self.hd_type1_mapped_control_value_offset(effect_slot, control)?;
        let value = f32::from_bits(read_be_u32(self.body_bytes(), value_at)?);
        exact_drive_percent(value)
    }

    /// Returns a selected non-type-1 control's canonical editor integer value.
    /// Values outside the exact normalized integer grid remain opaque.
    pub fn hd_numeric_control_value(
        &self,
        effect_slot: usize,
        control: HdNumericControl,
    ) -> Option<i32> {
        let value_at = self.hd_numeric_control_value_offset(effect_slot, control)?;
        let value = f32::from_bits(read_be_u32(self.body_bytes(), value_at)?);
        exact_numeric_value(value, control.min, control.max)
    }

    /// Reads a statically joined type-9 choice control. Unknown finite
    /// and non-finite words remain observable without being normalized.
    pub fn hd_type9_choice_value(&self, effect_slot: HdFxSlot) -> HdType9ChoiceValue {
        let Ok(value_at) = self.hd_type9_choice_value_offset(effect_slot) else {
            return HdType9ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType9ChoiceValue::Unavailable;
        };
        match bits {
            0x0000_0000 => HdType9ChoiceValue::Choice(HdType9Choice::Choice0),
            0x3f00_0000 => HdType9ChoiceValue::Choice(HdType9Choice::Choice1),
            0x3f80_0000 => HdType9ChoiceValue::Choice(HdType9Choice::Choice2),
            _ if f32::from_bits(bits).is_finite() => HdType9ChoiceValue::UnknownFinite(bits),
            _ => HdType9ChoiceValue::NonFinite(bits),
        }
    }

    /// Writes one explicit type-9 choice into exactly one existing H5EP row.
    /// All admission guards run before the byte-identical no-op check.
    pub fn set_hd_type9_choice(
        &mut self,
        effect_slot: HdFxSlot,
        choice: HdType9Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type9_choice_value_offset(effect_slot)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-9 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-9 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }

        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    /// Writes one explicit type-8 choice into exactly one existing H5EP row.
    /// The two-choice API is distinct from type 9 and cannot accept Choice2.
    /// All admission guards run before the byte-identical no-op check.
    pub fn set_hd_type8_choice(
        &mut self,
        effect_slot: HdFxSlot,
        choice: HdType8Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type8_choice_value_offset(effect_slot)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-8 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-8 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }

        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    /// Reads the exact type-8 row without normalizing unknown or non-finite
    /// imported words.
    pub fn hd_type8_choice_value(&self, effect_slot: HdFxSlot) -> HdType8ChoiceValue {
        let Ok(value_at) = self.hd_type8_choice_value_offset(effect_slot) else {
            return HdType8ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType8ChoiceValue::Unavailable;
        };
        match bits {
            0x0000_0000 => HdType8ChoiceValue::Choice(HdType8Choice::Choice0),
            0x3f80_0000 => HdType8ChoiceValue::Choice(HdType8Choice::Choice1),
            _ if f32::from_bits(bits).is_finite() => HdType8ChoiceValue::UnknownFinite(bits),
            _ => HdType8ChoiceValue::NonFinite(bits),
        }
    }

    pub fn hd_type10_choice_value(&self, effect_slot: HdFxSlot) -> HdType10ChoiceValue {
        let Ok(value_at) = self.hd_type10_choice_value_offset(effect_slot) else {
            return HdType10ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType10ChoiceValue::Unavailable;
        };
        match bits {
            0 => HdType10ChoiceValue::Choice(HdType10Choice::Choice0),
            0x3f80_0000 => HdType10ChoiceValue::Choice(HdType10Choice::Choice1),
            _ if f32::from_bits(bits).is_finite() => HdType10ChoiceValue::UnknownFinite(bits),
            _ => HdType10ChoiceValue::NonFinite(bits),
        }
    }

    pub fn set_hd_type10_choice(
        &mut self,
        effect_slot: HdFxSlot,
        choice: HdType10Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type10_choice_value_offset(effect_slot)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-10 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-10 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    fn hd_type10_choice_value_offset(&self, effect_slot: HdFxSlot) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        let Some(key) = hd_type10_choice_key(model_id) else {
            return Err(ToneError::format(
                "HD type-10 choice requires an admitted model in the selected FX slot",
            ));
        };
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-10 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-10 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-10 choice requires an effect row count in 1..=12",
            ));
        }
        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(key) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-10 choice row is missing"
            } else {
                "HD type-10 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-10 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-10 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    pub fn hd_type11_choice_value(
        &self,
        effect_slot: HdFxSlot,
        control: HdType11Control,
    ) -> HdType11ChoiceValue {
        let Ok(value_at) = self.hd_type11_choice_value_offset(effect_slot, control) else {
            return HdType11ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType11ChoiceValue::Unavailable;
        };
        if let Some(choice) = HdType11Choice::ALL
            .into_iter()
            .find(|choice| choice.bits() == bits)
        {
            HdType11ChoiceValue::Choice(choice)
        } else if f32::from_bits(bits).is_finite() {
            HdType11ChoiceValue::UnknownFinite(bits)
        } else {
            HdType11ChoiceValue::NonFinite(bits)
        }
    }

    pub fn set_hd_type11_choice(
        &mut self,
        effect_slot: HdFxSlot,
        control: HdType11Control,
        choice: HdType11Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type11_choice_value_offset(effect_slot, control)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-11 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-11 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    fn hd_type11_choice_value_offset(
        &self,
        effect_slot: HdFxSlot,
        control: HdType11Control,
    ) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        if model_id != control.model_id()
            || !HD_TYPE11_CHOICE_MODEL_KEYS.contains(&(control.model_id(), control.key()))
        {
            return Err(ToneError::format(
                "HD type-11 choice requires the exact admitted model and control key in the selected FX slot",
            ));
        }
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-11 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-11 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-11 choice requires an effect row count in 1..=12",
            ));
        }
        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(control.key()) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-11 choice row is missing"
            } else {
                "HD type-11 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-11 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-11 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    /// Reads the exact admitted type-12 row losslessly; unknown finite words
    /// are preserved and non-finite words are exposed without normalization.
    pub fn hd_type12_choice_value(
        &self,
        effect_slot: HdFxSlot,
        control: HdType12Control,
    ) -> HdType12ChoiceValue {
        let Ok(value_at) = self.hd_type12_choice_value_offset(effect_slot, control) else {
            return HdType12ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType12ChoiceValue::Unavailable;
        };
        if let Some(choice) = HdType12Choice::ALL
            .into_iter()
            .find(|choice| choice.bits() == bits)
        {
            HdType12ChoiceValue::Choice(choice)
        } else if f32::from_bits(bits).is_finite() {
            HdType12ChoiceValue::UnknownFinite(bits)
        } else {
            HdType12ChoiceValue::NonFinite(bits)
        }
    }

    /// Patches only the admitted type-12 row's value word after all structure,
    /// target and finiteness guards pass and the complete H5EP reparses.
    pub fn set_hd_type12_choice(
        &mut self,
        effect_slot: HdFxSlot,
        control: HdType12Control,
        choice: HdType12Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type12_choice_value_offset(effect_slot, control)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-12 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-12 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        let reparsed = Self::parse(candidate)?;
        self.bytes = reparsed.bytes;
        Ok(())
    }

    fn hd_type12_choice_value_offset(
        &self,
        effect_slot: HdFxSlot,
        control: HdType12Control,
    ) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        if model_id != control.model_id()
            || control != HdType12Control::Model020a001eKey3f100004
            || control.key() != HD_TYPE12_CHOICE_KEY
        {
            return Err(ToneError::format(
                "HD type-12 choice requires model 0x020a001e and key 0x3f100004 in the selected FX slot",
            ));
        }
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-12 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-12 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-12 choice requires an effect row count in 1..=12",
            ));
        }
        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(control.key()) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-12 choice row is missing"
            } else {
                "HD type-12 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-12 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-12 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    pub fn hd_type13_choice_value(
        &self,
        effect_slot: HdFxSlot,
        control: HdType13Control,
    ) -> HdType13ChoiceValue {
        let Ok(value_at) = self.hd_type13_choice_value_offset(effect_slot, control) else {
            return HdType13ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType13ChoiceValue::Unavailable;
        };
        if let Some(choice) = HdType13Choice::ALL
            .into_iter()
            .find(|choice| choice.bits() == bits)
        {
            HdType13ChoiceValue::Choice(choice)
        } else if f32::from_bits(bits).is_finite() {
            HdType13ChoiceValue::UnknownFinite(bits)
        } else {
            HdType13ChoiceValue::NonFinite(bits)
        }
    }

    pub fn set_hd_type13_choice(
        &mut self,
        effect_slot: HdFxSlot,
        control: HdType13Control,
        choice: HdType13Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type13_choice_value_offset(effect_slot, control)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD type-13 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD type-13 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        self.bytes = Self::parse(candidate)?.bytes;
        Ok(())
    }

    fn hd_type13_choice_value_offset(
        &self,
        effect_slot: HdFxSlot,
        control: HdType13Control,
    ) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        if model_id != HD_TYPE13_CHOICE_MODEL_ID
            || control != HdType13Control::Model020a001dKey3f100003
            || control.key() != HD_TYPE13_CHOICE_KEY
        {
            return Err(ToneError::format(
                "HD type-13 choice requires model 0x020a001d and key 0x3f100003 in the selected FX slot",
            ));
        }
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-13 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-13 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-13 choice requires an effect row count in 1..=12",
            ));
        }
        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(control.key()) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-13 choice row is missing"
            } else {
                "HD type-13 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-13 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-13 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    pub fn hd_type19_choice_value(
        &self,
        effect_slot: HdFxSlot,
        control: HdType19Control,
    ) -> HdType19ChoiceValue {
        let Ok(value_at) = self.hd_type19_choice_value_offset(effect_slot, control) else {
            return HdType19ChoiceValue::Unavailable;
        };
        let Some(bits) = read_be_u32(self.body_bytes(), value_at) else {
            return HdType19ChoiceValue::Unavailable;
        };
        if let Some(choice) = HdType19Choice::ALL
            .into_iter()
            .find(|choice| choice.bits() == bits)
        {
            HdType19ChoiceValue::Choice(choice)
        } else if f32::from_bits(bits).is_finite() {
            HdType19ChoiceValue::UnknownFinite(bits)
        } else {
            HdType19ChoiceValue::NonFinite(bits)
        }
    }

    pub fn set_hd_type19_choice(
        &mut self,
        effect_slot: HdFxSlot,
        control: HdType19Control,
        choice: HdType19Choice,
    ) -> Result<(), ToneError> {
        let value_at = self.hd_type19_choice_value_offset(effect_slot, control)?;
        let current = read_be_u32(self.body_bytes(), value_at)
            .ok_or_else(|| ToneError::format("HD descriptor type-0x13 value word is truncated"))?;
        if !f32::from_bits(current).is_finite() {
            return Err(ToneError::format(
                "existing HD descriptor type-0x13 value is non-finite and cannot be edited",
            ));
        }
        if current == choice.bits() {
            return Ok(());
        }
        let mut candidate = self.bytes.clone();
        let file_at = H5EP_HEADER_BYTES + value_at;
        candidate[file_at..file_at + 4].copy_from_slice(&choice.bits().to_be_bytes());
        self.bytes = Self::parse(candidate)?.bytes;
        Ok(())
    }

    fn hd_type19_choice_value_offset(
        &self,
        effect_slot: HdFxSlot,
        control: HdType19Control,
    ) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        if model_id != HD_TYPE19_CHOICE_MODEL_ID
            || control != HdType19Control::Model020a0011Key3f100003
            || control.key() != HD_TYPE19_CHOICE_KEY
        {
            return Err(ToneError::format(
                "HD descriptor type-0x13 choice requires model 0x020a0011 and key 0x3f100003 in the selected FX slot",
            ));
        }
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD descriptor type-0x13 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD descriptor type-0x13 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD descriptor type-0x13 choice requires an effect row count in 1..=12",
            ));
        }
        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(control.key()) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD descriptor type-0x13 choice row is missing"
            } else {
                "HD descriptor type-0x13 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching key counted");
        if row >= count {
            return Err(ToneError::format(
                "HD descriptor type-0x13 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD descriptor type-0x13 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    fn hd_type8_choice_value_offset(&self, effect_slot: HdFxSlot) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        let Some(key) = hd_type8_choice_key(model_id) else {
            return Err(ToneError::format(
                "HD type-8 choice requires model 0x020a0013 in the selected FX slot",
            ));
        };
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-8 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-8 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-8 choice requires an effect row count in 1..=12",
            ));
        }

        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(key) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-8 choice row is missing"
            } else {
                "HD type-8 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-8 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-8 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    fn hd_type9_choice_value_offset(&self, effect_slot: HdFxSlot) -> Result<usize, ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block).unwrap_or_default();
        let Some(key) = hd_type9_choice_key(model_id) else {
            return Err(ToneError::format(
                "HD type-9 choice requires an admitted model in the selected FX slot",
            ));
        };
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD type-9 choice requires a valid, non-empty effect slot",
            ));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(ToneError::format(
                "HD type-9 choice requires a canonical enable byte",
            ));
        }
        let count = body[block + 0x0f] as usize;
        if !(1..=HD_PARAMETER_ROW_COUNT).contains(&count) {
            return Err(ToneError::format(
                "HD type-9 choice requires an effect row count in 1..=12",
            ));
        }

        let mut found = None;
        let mut matches = 0;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at) == Some(key) {
                found = Some((row, row_at));
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(ToneError::format(if matches == 0 {
                "HD type-9 choice row is missing"
            } else {
                "HD type-9 choice row key is ambiguous"
            }));
        }
        let (row, row_at) = found.expect("one matching row was counted");
        if row >= count {
            return Err(ToneError::format(
                "HD type-9 choice row is outside the declared effect row count",
            ));
        }
        if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
            return Err(ToneError::format(
                "HD type-9 choice row must have an unassigned marker",
            ));
        }
        Ok(row_at + 4)
    }

    pub fn effect_model_id(&self, effect_slot: usize) -> Option<u32> {
        if effect_slot >= HD_EFFECT_COUNT {
            return None;
        }
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        read_be_u32(self.body_bytes(), block)
    }

    /// Reads the source-traced enable flag for an admitted existing HD effect.
    /// The on-disk byte is interpreted as a boolean; this does not claim that
    /// the file is applicable to a connected product.
    pub fn effect_enabled(&self, effect_slot: HdFxSlot) -> Option<bool> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block)?;
        (body[block + 0x0b] == 1 && is_admitted_hd_enable_model(model_id))
            .then_some(body[block + 8] != 0)
    }

    /// Changes only the existing effect's source-traced enable byte. Nonzero
    /// values read as enabled, but writes require a canonical stored byte so
    /// malformed values are never silently normalized.
    pub fn set_effect_enabled(
        &mut self,
        effect_slot: HdFxSlot,
        enabled: bool,
    ) -> Result<(), ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let model_id = read_be_u32(body, block)
            .ok_or_else(|| ToneError::format("truncated HD effect model id"))?;
        if model_id == HD_EMPTY_EFFECT_MODEL_ID {
            return Err(ToneError::format(
                "cannot enable or disable an empty HD effect slot",
            ));
        }
        if !is_admitted_hd_enable_model(model_id) {
            return Err(ToneError::format(format!(
                "HD effect model {model_id:08x} has no admitted enable contract"
            )));
        }
        if body[block + 0x0b] != 1 {
            return Err(ToneError::format(
                "HD effect enable requires a valid, non-empty effect slot",
            ));
        }
        if body[block + 8] > 1 {
            return Err(ToneError::format(
                "HD effect enable requires a canonical 0 or 1 byte",
            ));
        }
        if (body[block + 8] != 0) == enabled {
            return Ok(());
        }

        // Stage the single-byte write and validate the complete record before
        // publishing it, so a failure cannot leave a partial mutation.
        let mut candidate = self.bytes.clone();
        candidate[H5EP_HEADER_BYTES + block + 8] = u8::from(enabled);
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    /// Replaces one existing Octave Fuzz with the source-derived Vintage Pre
    /// default prefix. This pure file operation has no vendor runtime context
    /// and makes no claim about physical target applicability.
    pub fn replace_octave_fuzz_with_vintage_pre_defaults(
        &mut self,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            HD_VINTAGE_PRE_MODEL_ID,
            &HD_VINTAGE_PRE_DEFAULT_KEYS,
            "replacement",
        )
    }

    /// Replaces one existing Octave Fuzz with the source-derived Screamer
    /// default prefix. This remains a typed, standalone file operation.
    pub fn replace_octave_fuzz_with_screamer_defaults(
        &mut self,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            HD_SCREAMER_MODEL_ID,
            &HD_SCREAMER_DEFAULT_KEYS,
            "Screamer",
        )
    }

    /// Replaces one existing Octave Fuzz with the source-derived Sub Octave
    /// Fuzz default prefix. This remains a typed, standalone file operation.
    pub fn replace_octave_fuzz_with_sub_octave_fuzz_defaults(
        &mut self,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            HD_SUB_OCTAVE_FUZZ_MODEL_ID,
            &HD_SUB_OCTAVE_FUZZ_DEFAULT_KEYS,
            "Sub Octave Fuzz",
        )
    }

    /// Replaces one existing Octave Fuzz with the two source-derived Noise
    /// Gate default rows. This remains a typed standalone file operation.
    pub fn replace_octave_fuzz_with_noise_gate_defaults(
        &mut self,
        effect_slot: HdFxSlot,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            HD_NOISE_GATE_MODEL_ID,
            &HD_NOISE_GATE_DEFAULT_KEYS,
            "Noise Gate",
        )
    }

    /// Applies one member of the exact finite distortion/fuzz default set
    /// admitted for this explicit standalone replacement.
    pub fn replace_octave_fuzz_with_admitted_defaults(
        &mut self,
        effect_slot: HdFxSlot,
        target: HdDistortionDefault,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            target.model_id(),
            &HD_SCREAMER_DEFAULT_KEYS,
            target.label(),
        )
    }

    /// Applies one member of the exact finite dynamics/FX default set admitted
    /// for this explicit standalone replacement.
    pub fn replace_octave_fuzz_with_admitted_dynamics_defaults(
        &mut self,
        effect_slot: HdFxSlot,
        target: HdDynamicsDefault,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            target.model_id(),
            target.default_keys(),
            target.label(),
        )
    }

    /// Applies one member of the exact finite reverb default set admitted
    /// for this explicit standalone replacement.
    pub fn replace_octave_fuzz_with_admitted_reverb_defaults(
        &mut self,
        effect_slot: HdFxSlot,
        target: HdReverbDefault,
    ) -> Result<(), ToneError> {
        self.replace_octave_fuzz_with_default_prefix(
            effect_slot,
            target.model_id(),
            target.default_keys(),
            target.label(),
        )
    }

    fn replace_octave_fuzz_with_default_prefix(
        &mut self,
        effect_slot: HdFxSlot,
        target_model_id: u32,
        default_keys: &[u32],
        target_label: &str,
    ) -> Result<(), ToneError> {
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot.index() * HD_EFFECT_BLOCK_STRIDE;
        let prefix_error = format!(
            "{target_label} replacement requires each default key exactly once in the first five rows"
        );
        let format_error = |message: String| ToneError::format(message);

        if read_be_u32(body, block) != Some(HD_OCTAVE_FUZZ_MODEL_ID) {
            return Err(format_error(format!(
                "{target_label} replacement requires an existing Octave Fuzz model"
            )));
        }
        if body[block + 0x0b] != 1 {
            return Err(format_error(format!(
                "{target_label} replacement requires a valid, non-empty effect slot"
            )));
        }
        if !matches!(body[block + 8], 0 | 1) {
            return Err(format_error(format!(
                "{target_label} replacement requires a canonical effect enable byte"
            )));
        }
        if !(5..=HD_PARAMETER_ROW_COUNT as u8).contains(&body[block + 0x0f]) {
            return Err(format_error(format!(
                "{target_label} replacement requires an effect row count in 5..=12"
            )));
        }

        // The source Octave Fuzz must have these five keys exactly once, in any
        // order. Target labels may differ; this operation applies defaults.
        let mut seen = [false; 5];
        for row in 0..5 {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            let key = read_be_u32(body, row_at)
                .ok_or_else(|| ToneError::format("truncated HD effect parameter prefix"))?;
            let Some(key_index) = HD_SCREAMER_DEFAULT_KEYS
                .iter()
                .position(|expected| *expected == key)
            else {
                return Err(format_error(prefix_error.clone()));
            };
            if std::mem::replace(&mut seen[key_index], true) {
                return Err(format_error(prefix_error.clone()));
            }
        }
        if seen.iter().any(|present| !present) {
            return Err(format_error(prefix_error));
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let marker_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10;
            if !matches!(body[marker_at], 0..=2) {
                return Err(ToneError::format(format!(
                    "{target_label} replacement requires canonical effect assignment marker bytes"
                )));
            }
        }

        // Stage every write in a candidate and reparse the complete record
        // before changing the original object.
        let mut candidate = self.bytes.clone();
        let file_block = H5EP_HEADER_BYTES + block;
        candidate[file_block..file_block + 4].copy_from_slice(&target_model_id.to_be_bytes());
        for (row, key) in default_keys.iter().copied().enumerate() {
            let row_at = file_block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            candidate[row_at..row_at + 4].copy_from_slice(&key.to_be_bytes());
            candidate[row_at + 4..row_at + 8].copy_from_slice(&0u32.to_be_bytes());
        }
        candidate[file_block + 0x0f] = default_keys.len() as u8;
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let marker_at =
                file_block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE + 0x10;
            candidate[marker_at] = 0;
        }
        let staged = Self::parse(candidate)?;
        self.bytes = staged.bytes;
        Ok(())
    }

    pub fn tube_drive_control_percent(
        &self,
        effect_slot: usize,
        control: TubeDriveControl,
    ) -> Option<u8> {
        self.hd_type1_control_percent(effect_slot, HdType1Control::TubeDrive(control))
    }

    pub fn octave_fuzz_control_percent(
        &self,
        effect_slot: usize,
        control: OctaveFuzzControl,
    ) -> Option<u8> {
        self.hd_type1_control_percent(effect_slot, HdType1Control::OctaveFuzz(control))
    }

    /// Edits only the first statically identified Screamer DRIVE value word.
    /// Unknown rows, duplicates and every other source byte remain unchanged.
    pub fn set_screamer_drive_percent(
        &mut self,
        effect_slot: usize,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_screamer_control_percent(effect_slot, ScreamerControl::Drive, percent)
    }

    /// Edits only the first matching keyed value for an admitted Screamer
    /// control. All unrelated rows and bytes are preserved.
    pub fn set_screamer_control_percent(
        &mut self,
        effect_slot: usize,
        control: ScreamerControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_hd_type1_control_percent(effect_slot, HdType1Control::Screamer(control), percent)
    }

    pub fn set_tube_drive_control_percent(
        &mut self,
        effect_slot: usize,
        control: TubeDriveControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_hd_type1_control_percent(effect_slot, HdType1Control::TubeDrive(control), percent)
    }

    pub fn set_octave_fuzz_control_percent(
        &mut self,
        effect_slot: usize,
        control: OctaveFuzzControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_hd_type1_control_percent(effect_slot, HdType1Control::OctaveFuzz(control), percent)
    }

    pub fn set_hd_type1_control_percent(
        &mut self,
        effect_slot: usize,
        control: HdType1Control,
        percent: u8,
    ) -> Result<(), ToneError> {
        if percent > 100 {
            return Err(ToneError::format(format!(
                "{} {} must be in 0..=100",
                control.model_label(),
                control.label()
            )));
        }
        let value_at = self
            .hd_type1_control_value_offset(effect_slot, control)
            .ok_or_else(|| {
                ToneError::format(
                    "HD type-1 control slot, key, model, or assignment state is not admitted for editing",
                )
            })?;
        let current = f32::from_bits(
            read_be_u32(self.body_bytes(), value_at)
                .ok_or_else(|| ToneError::format("Screamer DRIVE row is truncated"))?,
        );
        exact_drive_percent(current).ok_or_else(|| {
            ToneError::format(
                "existing HD control value is outside the exact recovered editor encoding",
            )
        })?;
        let file_at = H5EP_HEADER_BYTES + value_at;
        self.bytes[file_at..file_at + 4]
            .copy_from_slice(&(percent as f32 / 100.0).to_bits().to_be_bytes());
        Ok(())
    }

    /// Edits only the first matching delay-family key's normalized value.
    pub fn set_hd_type1_mapped_control_percent(
        &mut self,
        effect_slot: usize,
        control: HdType1MappedControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        if percent > 100 {
            return Err(ToneError::format(format!(
                "{} {} must be in 0..=100",
                control.model_label, control.label
            )));
        }
        let value_at = self
            .hd_type1_mapped_control_value_offset(effect_slot, control)
            .ok_or_else(|| {
                ToneError::format(
                    "HD mapped type-1 control slot, key, model, or assignment state is not admitted for editing",
                )
            })?;
        let current = f32::from_bits(
            read_be_u32(self.body_bytes(), value_at)
                .ok_or_else(|| ToneError::format("HD mapped type-1 row is truncated"))?,
        );
        exact_drive_percent(current).ok_or_else(|| {
            ToneError::format("existing HD mapped type-1 value is outside the editor percent grid")
        })?;
        let file_at = H5EP_HEADER_BYTES + value_at;
        self.bytes[file_at..file_at + 4]
            .copy_from_slice(&(percent as f32 / 100.0).to_bits().to_be_bytes());
        Ok(())
    }

    /// Edits one selected non-type-1 numeric widget value. The stored float
    /// must already be on the exact source-configured integer grid; only its
    /// keyed value word changes.
    pub fn set_hd_numeric_control_value(
        &mut self,
        effect_slot: usize,
        control: HdNumericControl,
        value: i32,
    ) -> Result<(), ToneError> {
        if !(control.min..=control.max).contains(&value) {
            return Err(ToneError::format(format!(
                "{} {} must be in {}..={}",
                control.model_label, control.label, control.min, control.max
            )));
        }
        let value_at = self
            .hd_numeric_control_value_offset(effect_slot, control)
            .ok_or_else(|| {
                ToneError::format(
                    "HD numeric control slot, key, model, or assignment state is not admitted for editing",
                )
            })?;
        let current = f32::from_bits(
            read_be_u32(self.body_bytes(), value_at)
                .ok_or_else(|| ToneError::format("HD numeric row is truncated"))?,
        );
        exact_numeric_value(current, control.min, control.max).ok_or_else(|| {
            ToneError::format(
                "existing HD numeric value is outside the exact recovered editor grid",
            )
        })?;
        let normalized = normalized_numeric_value(value, control.min, control.max);
        let file_at = H5EP_HEADER_BYTES + value_at;
        self.bytes[file_at..file_at + 4].copy_from_slice(&normalized.to_bits().to_be_bytes());
        Ok(())
    }

    /// Exact no-op serialization. The envelope's unknown fields remain raw.
    pub fn serialize_noop(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }

    fn hd_type1_control_value_offset(
        &self,
        effect_slot: usize,
        control: HdType1Control,
    ) -> Option<usize> {
        self.hd_type1_value_offset(effect_slot, control.model_id(), control.key())
    }

    fn hd_type1_mapped_control_value_offset(
        &self,
        effect_slot: usize,
        control: HdType1MappedControl,
    ) -> Option<usize> {
        self.hd_type1_value_offset(effect_slot, control.model_id, control.key)
    }

    fn hd_type1_value_offset(&self, effect_slot: usize, model_id: u32, key: u32) -> Option<usize> {
        if effect_slot >= HD_EFFECT_COUNT {
            return None;
        }
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        if read_be_u32(body, block)? != model_id {
            return None;
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at)? == key {
                // The source sends the row flag into widget assignment state,
                // while the callback suppresses writes for expression controls.
                // Only the zero-flag state is admitted; auxiliary flag bytes
                // remain opaque and are never normalized.
                if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
                    return None;
                }
                return Some(row_at + 4);
            }
        }
        None
    }

    fn hd_numeric_control_value_offset(
        &self,
        effect_slot: usize,
        control: HdNumericControl,
    ) -> Option<usize> {
        if effect_slot >= HD_EFFECT_COUNT {
            return None;
        }
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        if read_be_u32(body, block)? != control.model_id {
            return None;
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at)? == control.key() {
                if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
                    return None;
                }
                return Some(row_at + 4);
            }
        }
        None
    }
}

impl H5esSetlist {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_SETLIST_BYTES_CEILING {
            return Err(ToneError::format(
                "H5ES exceeds the offline file-size ceiling",
            ));
        }
        validate_h5es_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5ES_MIN_BYTES..]
    }

    pub fn preset_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= H5ES_PRESET_COUNT {
            return None;
        }
        let start = H5ES_HEADER_BYTES + index * H5EP_MIN_BYTES;
        self.bytes.get(start..start + H5EP_MIN_BYTES)
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

impl H5ebBundle {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "H5EB exceeds the offline file-size ceiling",
            ));
        }
        validate_h5eb_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5EB_MIN_BYTES..]
    }

    pub fn setlist_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= H5EB_SETLIST_COUNT {
            return None;
        }
        let start = H5EB_HEADER_BYTES + index * H5ES_MIN_BYTES;
        self.bytes.get(start..start + H5ES_MIN_BYTES)
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

fn validate_h5ep_record(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5EP_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5EP record: expected at least {H5EP_MIN_BYTES} bytes"
        )));
    }
    if &bytes[..4] != b"H5EP" {
        return Err(ToneError::format("expected H5EP preset magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5EP version"));
    }
    validate_hd_body(&bytes[H5EP_HEADER_BYTES..H5EP_MIN_BYTES])
}

fn validate_h5es_header(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5ES_HEADER_BYTES {
        return Err(ToneError::format("truncated H5ES header"));
    }
    if &bytes[..4] != b"H5ES" {
        return Err(ToneError::format("expected H5ES setlist magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5ES version"));
    }
    Ok(())
}

fn validate_h5es_record(bytes: &[u8]) -> Result<(), ToneError> {
    validate_h5es_header(bytes)?;
    if bytes.len() < H5ES_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5ES children: expected at least {H5ES_MIN_BYTES} bytes"
        )));
    }
    for index in 0..H5ES_PRESET_COUNT {
        let start = H5ES_HEADER_BYTES + index * H5EP_MIN_BYTES;
        validate_h5ep_record(&bytes[start..start + H5EP_MIN_BYTES])?;
    }
    Ok(())
}

fn validate_h5eb_record(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5EB_HEADER_BYTES {
        return Err(ToneError::format("truncated H5EB header"));
    }
    if &bytes[..4] != b"H5EB" {
        return Err(ToneError::format("expected H5EB bundle magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5EB version"));
    }
    if bytes.len() < H5EB_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5EB children: expected at least {H5EB_MIN_BYTES} bytes"
        )));
    }
    for index in 0..H5EB_SETLIST_COUNT {
        let start = H5EB_HEADER_BYTES + index * H5ES_MIN_BYTES;
        validate_h5es_record(&bytes[start..start + H5ES_MIN_BYTES])?;
    }
    Ok(())
}

fn validate_hd_body(body: &[u8]) -> Result<(), ToneError> {
    if body.len() != H5EP_BODY_BYTES {
        return Err(ToneError::format("invalid H5EP body length"));
    }
    for index in 0..2 {
        if body[0x028 + index * HD_EFFECT_BLOCK_STRIDE] != 0 {
            return Err(ToneError::format("invalid H5EP amp model class"));
        }
        if body[0x228 + index * HD_EFFECT_BLOCK_STRIDE] != 1 {
            return Err(ToneError::format("invalid H5EP cab model class"));
        }
    }
    let mut loop_count = 0;
    for index in 0..HD_EFFECT_COUNT {
        let offset = HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE;
        if body[offset] != 2 {
            return Err(ToneError::format("invalid H5EP effect model class"));
        }
        if read_be_u32(body, offset) == Some(0x0208_0000) {
            loop_count += 1;
        }
    }
    if loop_count >= 2 {
        return Err(ToneError::format("H5EP contains two FX Loop models"));
    }
    for (offset, maximum, field) in [
        (0xff3, 1, "TrailsEnabled"),
        (0xff6, 1, "AmpControl"),
        (0xff8, 7, "Mic[0]"),
        (0xff9, 7, "Mic[1]"),
        (0xffa, 1, "MainMute[0]"),
        (0xffb, 1, "MainMute[1]"),
        (0xffe, 10, "Input1Source"),
        (0xfff, 10, "Input2Source"),
    ] {
        if body[offset] > maximum {
            return Err(ToneError::format(format!("invalid H5EP {field} value")));
        }
    }
    Ok(())
}

pub(super) fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// Enable editing is bounded to model ids already present in a source-joined
/// typed control table. This is intentionally not the editor's complete model
/// menu and cannot assign a model or infer slot applicability.
pub(super) fn transform_hd_body_words(body: &mut [u8]) {
    const FIXED_WORDS: [usize; 9] = [0x20, 0x24, 0xd48, 0xd4c, 0xd50, 0xd54, 0xd58, 0xd7c, 0xd80];
    for offset in FIXED_WORDS {
        body[offset..offset + 4].reverse();
    }
    const BLOCKS: [usize; 4] = [0x28, 0x128, 0x228, 0x328];
    for offset in BLOCKS.into_iter().chain(
        (0..HD_EFFECT_COUNT).map(|index| HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE),
    ) {
        for word in 0..2 {
            let at = offset + word * 4;
            body[at..at + 4].reverse();
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = offset + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            for word in 0..4 {
                let at = row_at + word * 4;
                body[at..at + 4].reverse();
            }
        }
    }
}

pub const HD_EDITOR_OFFLINE_NOTICE: &str = "HD Edit files are handled offline. Internal target and physical compatibility remain unverified.";

/// Reads a regular offline `.5xe`/`.5xs`/`.5xb` candidate under the largest
/// bounded format size. Device nodes, pipes, and oversized files are refused
/// before the reader closure is called.
pub fn read_regular_hd_editor_file<R: Read>(
    facts: std::io::Result<PathFacts>,
    read: impl FnOnce() -> std::io::Result<R>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "HD editor input must be a regular file; non-file paths are not opened",
        ));
    }
    if facts.len > HD_BUNDLE_BYTES_CEILING as u64 {
        return Err(ToneError::format(
            "HD editor file exceeds the offline file-size ceiling",
        ));
    }
    read_bounded_file(
        read().map_err(|error| ToneError::format(error.to_string()))?,
        HD_BUNDLE_BYTES_CEILING as u64,
        "HD editor file exceeds the offline file-size ceiling",
    )
}
