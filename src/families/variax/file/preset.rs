//! Lossless reader/editor for individual Workbench FORM/L6PA/L6FS/L6AS preset files.
//! Unknown chunks, ordering, padding and parameter bit patterns remain untouched.
//! File parameters are model-local IDs, not the MIDI metadata parameter IDs.

pub(crate) use crate::shared::iff::{Chunk, chunks, container, unique, word};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileParameter {
    pub id: u32,
    pub value_type: u32,
    pub bits: u32,
    value_offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    pub id: u32,
    pub slot: u32,
    pub ordinal: u8,
    pub enabled: bool,
    pub parameters: Vec<FileParameter>,
    info_offset: usize,
}

#[derive(Debug, Clone)]
pub struct PresetFile {
    bytes: Vec<u8>,
    name_offset: usize,
    name: String,
    device_id: u32,
    device_version: u32,
    models: Vec<Model>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PresetText {
    Author,
    Notes,
}

impl PresetText {
    fn tag(self) -> [u8; 4] {
        match self {
            Self::Author => *b"IAUT",
            Self::Notes => *b"ICMT",
        }
    }
}

/// Why a stored model name or author cannot be sent to a legacy Variax.
/// A new edit uses [`reject_new_legacy_text`] instead of keeping this text.
pub fn legacy_text_issue(value: &str, already_stored: bool) -> Option<&'static str> {
    if value.len() <= 18 && value.bytes().all(|byte| matches!(byte, 0x20..=0x7e)) {
        return None;
    }
    Some(if already_stored {
        "Kept as saved. The guitar only stores 18 letters, numbers, spaces, and keyboard symbols."
    } else {
        "Not saved. Use at most 18 letters, numbers, spaces, and keyboard symbols."
    })
}

fn reject_new_legacy_text(value: &str) -> Result<(), String> {
    if legacy_text_issue(value, false).is_some() {
        Err("Use at most 18 letters, numbers, spaces, and keyboard symbols.".into())
    } else {
        Ok(())
    }
}

impl PresetFile {
    /// Construct a portable model/settings file from a current-preset metadata
    /// read. This does not contain the guitar's full DSP or firmware memory.
    pub fn from_metadata(
        metadata: &crate::PresetMetadata,
        device_id: u32,
        version: u32,
    ) -> Result<Self, String> {
        use crate::ParameterValue;
        if !matches!(device_id, 0x70000 | 0x70004) {
            return Err("unsupported Variax model".into());
        }
        let value = |name: &str| {
            metadata
                .parameters
                .get(name)
                .ok_or_else(|| format!("missing metadata field {name}"))
        };
        let integer = |name: &str| match value(name)? {
            ParameterValue::Integer(v) => Ok(*v as u32),
            _ => Err(format!("expected integer {name}")),
        };
        let boolean = |name: &str| match value(name)? {
            ParameterValue::Boolean(v) => Ok(*v),
            _ => Err(format!("expected boolean {name}")),
        };
        let make_chunk = |tag: &[u8; 4], payload: &[u8]| {
            let mut bytes = tag.to_vec();
            bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            bytes.extend_from_slice(payload);
            if !payload.len().is_multiple_of(2) {
                bytes.push(0);
            }
            bytes
        };
        let mut models = b"PATC".to_vec();
        let mut model =
            |slot: u32, id: u32, enabled: bool, fields: &[(u32, &str)]| -> Result<(), String> {
                let mut info = id.to_be_bytes().to_vec();
                info.extend_from_slice(&slot.to_be_bytes());
                info.extend_from_slice(&[0, 0, 0, u8::from(enabled)]);
                let mut bytes = b"MODL".to_vec();
                bytes.extend(make_chunk(b"MINF", &info));
                for &(parameter, key) in fields {
                    let (kind, bits) = match value(key)? {
                        ParameterValue::Integer(v) => (0u32, *v as u32),
                        ParameterValue::Boolean(v) => (0, u32::from(*v)),
                        ParameterValue::Float(v) => (1, v.to_bits()),
                    };
                    let mut payload = (0x3f000000 | parameter).to_be_bytes().to_vec();
                    payload.extend_from_slice(&kind.to_be_bytes());
                    payload.extend_from_slice(&bits.to_be_bytes());
                    bytes.extend(make_chunk(b"PARM", &payload));
                }
                models.extend(make_chunk(b"LIST", &bytes));
                Ok(())
            };
        model(0x02010000, 0, true, &[(0x200000, "trim")])?;
        model(
            0x02010001,
            integer("pickup1_model")?,
            boolean("pickup1_enabled")?,
            &[
                (0x200000, "pickup1_position"),
                (0x200001, "pickup1_angle"),
                (0x200003, "pickup1_level"),
            ],
        )?;
        model(
            0x02010002,
            integer("pickup2_model")?,
            boolean("pickup2_enabled")?,
            &[
                (0x200000, "pickup2_position"),
                (0x200001, "pickup2_angle"),
                (0x200002, "pickup2_phase_reverse"),
                (0x200003, "pickup2_level"),
                (0x200004, "pickups_in_series"),
            ],
        )?;
        model(0x02010003, integer("body_model")?, true, &[])?;
        model(
            0x02010004,
            0,
            true,
            &[
                (0x200000, "volume_resistance"),
                (0x200001, "volume_taper"),
                (0x200002, "tone_resistance"),
                (0x200003, "tone_capacitance"),
                (0x200004, "tone_position"),
                (0x200005, "save_tone_position"),
            ],
        )?;
        model(
            0x02010005,
            0,
            boolean("alternate_tuning_enabled")?,
            &[
                (0x200000, "string1_tuning"),
                (0x200001, "string2_tuning"),
                (0x200002, "string3_tuning"),
                (0x200003, "string4_tuning"),
                (0x200004, "string5_tuning"),
                (0x200005, "string6_tuning"),
                (0x200006, "string1_detune"),
                (0x200007, "string2_detune"),
                (0x200008, "string3_detune"),
                (0x200009, "string4_detune"),
                (0x20000a, "string5_detune"),
                (0x20000b, "string6_detune"),
                (0x20000c, "string1_mix"),
                (0x20000d, "string2_mix"),
                (0x20000e, "string3_mix"),
                (0x20000f, "string4_mix"),
                (0x200010, "string5_mix"),
                (0x200011, "string6_mix"),
                (0x200012, "string_spread"),
            ],
        )?;
        let mut info = 1u32.to_be_bytes().to_vec();
        info.extend_from_slice(&device_id.to_be_bytes());
        let mut name = [0u8; 64];
        let units: Vec<_> = metadata.name.encode_utf16().collect();
        if units.len() > 32 {
            return Err("preset name exceeds file capacity".into());
        }
        for (i, unit) in units.iter().enumerate() {
            name[i * 2..i * 2 + 2].copy_from_slice(&unit.to_be_bytes());
        }
        info.extend_from_slice(&name);
        info.extend_from_slice(&version.to_be_bytes());
        let mut payload = b"L6PA".to_vec();
        payload.extend(make_chunk(b"PINF", &info));
        payload.extend(make_chunk(b"LIST", &models));
        Self::parse(make_chunk(b"FORM", &payload))
    }

    pub fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 12
            || &bytes[..4] != b"FORM"
            || !matches!(&bytes[8..12], b"L6PA" | b"L6FS" | b"L6AS")
        {
            return Err("expected an individual FORM/L6PA, FORM/L6FS, or FORM/L6AS preset".into());
        }
        if word(&bytes, 4) as usize != bytes.len() - 8 {
            return Err("FORM length does not match the file".into());
        }
        let children = chunks(&bytes, 12..bytes.len())?;
        let info = unique(&children, b"PINF")?;
        if info.data.len() < 76 || word(&bytes, info.data.start) != 1 {
            return Err("unsupported or truncated PINF version".into());
        }
        let start = info.data.start;
        let name_offset = start + 8;
        let units: Vec<u16> = bytes[name_offset..name_offset + 64]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_be_bytes(*b))
            .take_while(|&c| c != 0)
            .collect();
        let name = String::from_utf16(&units).map_err(|_| "invalid UTF-16 preset name")?;
        let device_id = word(&bytes, start + 4);
        let device_version = word(&bytes, start + 72);
        let patch_containers: Vec<_> = children
            .iter()
            .filter(|c| container(&bytes, c, b"PATC"))
            .collect();
        if patch_containers.len() != 1 {
            return Err("expected exactly one PATC container".into());
        }
        let patch = patch_containers[0];
        let model_chunks = chunks(&bytes, patch.data.start + 4..patch.data.end)?;
        let mut models = Vec::new();
        for entry in model_chunks
            .iter()
            .filter(|c| container(&bytes, c, b"MODL"))
        {
            let children = chunks(&bytes, entry.data.start + 4..entry.data.end)?;
            let info = unique(&children, b"MINF")?;
            if info.data.len() < 12 {
                return Err("truncated MINF chunk".into());
            }
            let start = info.data.start;
            let mut parameters = Vec::new();
            for parameter in children.iter().filter(|c| c.id == *b"PARM") {
                if parameter.data.len() < 12 {
                    return Err("truncated PARM chunk".into());
                }
                let start = parameter.data.start;
                let id = word(&bytes, start) & 0xffffff;
                if parameters.iter().any(|p: &FileParameter| p.id == id) {
                    return Err(format!("ambiguous duplicate parameter {id:06x}"));
                }
                parameters.push(FileParameter {
                    id,
                    value_type: word(&bytes, start + 4),
                    bits: word(&bytes, start + 8),
                    value_offset: start + 8,
                });
            }
            let slot = word(&bytes, start + 4);
            if models.iter().any(|m: &Model| m.slot == slot) {
                return Err(format!("ambiguous duplicate model slot {slot:08x}"));
            }
            models.push(Model {
                id: word(&bytes, start),
                slot,
                ordinal: bytes[start + 8],
                enabled: bytes[start + 11] != 0,
                parameters,
                info_offset: start,
            });
        }
        Ok(Self {
            bytes,
            name_offset,
            name,
            device_id,
            device_version,
            models,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Variax 300/500/600/700 store a model or author as 18 printable ASCII
    /// bytes. Other devices keep the older 32 UTF-16 edit rule.
    pub fn limits_legacy_name(&self) -> bool {
        matches!(self.device_id, 0x70000 | 0x70001 | 0x70004)
    }

    fn metadata_chunks(&self, field: PresetText) -> Result<Option<(Chunk, Option<Chunk>)>, String> {
        let root = chunks(&self.bytes, 12..self.bytes.len())?;
        let mut lists = root.into_iter().filter(|chunk| {
            container(&self.bytes, chunk, b"UNFO") || container(&self.bytes, chunk, b"INFO")
        });
        let Some(list) = lists.next() else {
            return Ok(None);
        };
        if lists.next().is_some() {
            return Err("ambiguous preset metadata containers".into());
        }
        let fields = chunks(&self.bytes, list.data.start + 4..list.data.end)?;
        let mut entries = fields.into_iter().filter(|chunk| chunk.id == field.tag());
        let entry = entries.next();
        if entries.next().is_some() {
            return Err("ambiguous duplicate preset text fields".into());
        }
        Ok(Some((list, entry)))
    }

    pub fn metadata(&self, field: PresetText) -> Result<String, String> {
        let Some((list, Some(entry))) = self.metadata_chunks(field)? else {
            return Ok(String::new());
        };
        let bytes = &self.bytes[entry.data];
        if container(&self.bytes, &list, b"UNFO") {
            let (units, remainder) = bytes.as_chunks::<2>();
            if !remainder.is_empty() {
                return Err("truncated UTF-16 preset text".into());
            }
            let units: Vec<_> = units.iter().copied().map(u16::from_be_bytes).collect();
            String::from_utf16(&units).map_err(|_| "invalid UTF-16 preset text".into())
        } else {
            String::from_utf8(bytes.to_vec()).map_err(|_| "invalid UTF-8 preset text".into())
        }
    }

    pub fn set_metadata(&mut self, field: PresetText, value: &str) -> Result<(), String> {
        self.require_legacy_electric()?;
        if value.contains('\0') {
            return Err("preset text cannot contain NUL".into());
        }
        if self.metadata(field)? == value {
            return Ok(());
        }
        if field == PresetText::Author && self.limits_legacy_name() {
            reject_new_legacy_text(value)?;
        }
        let location = self.metadata_chunks(field)?;
        let encode_chunk = |tag: [u8; 4], payload: &[u8]| -> Result<Vec<u8>, String> {
            let length = u32::try_from(payload.len()).map_err(|_| "preset text is too large")?;
            let mut bytes = tag.to_vec();
            bytes.extend(length.to_be_bytes());
            bytes.extend(payload);
            if !length.is_multiple_of(2) {
                bytes.push(0);
            }
            Ok(bytes)
        };
        let payload = if location
            .as_ref()
            .is_some_and(|(list, _)| container(&self.bytes, list, b"INFO"))
        {
            value.as_bytes().to_vec()
        } else {
            value.encode_utf16().flat_map(u16::to_be_bytes).collect()
        };
        let replacement = encode_chunk(field.tag(), &payload)?;
        let mut bytes = self.bytes.clone();
        if let Some((list, entry)) = location {
            let range = entry.map_or(list.data.end..list.data.end, |entry| {
                entry.data.start - 8..entry.data.end + (entry.data.len() & 1)
            });
            let length = list.data.len() - range.len() + replacement.len();
            let length = u32::try_from(length).map_err(|_| "preset metadata is too large")?;
            bytes.splice(range, replacement);
            bytes[list.data.start - 4..list.data.start].copy_from_slice(&length.to_be_bytes());
        } else {
            let mut list = b"UNFO".to_vec();
            list.extend(replacement);
            bytes.extend(encode_chunk(*b"LIST", &list)?);
        }
        let length = u32::try_from(bytes.len() - 8).map_err(|_| "preset file is too large")?;
        bytes[4..8].copy_from_slice(&length.to_be_bytes());
        *self = Self::parse(bytes)?;
        Ok(())
    }

    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    /// PODxt Live tones share the FORM/L6PA container and are not guitar presets.
    pub fn is_podxt_live_target(&self) -> bool {
        self.device_id == 0x0003_000a
    }
    pub fn device_version(&self) -> u32 {
        self.device_version
    }
    pub fn models(&self) -> &[Model] {
        &self.models
    }

    pub fn model(&self, slot: u32) -> Option<&Model> {
        self.models.iter().find(|m| m.slot == slot)
    }

    pub fn parameter(&self, slot: u32, id: u32) -> Option<&FileParameter> {
        self.model(slot)?.parameters.iter().find(|p| p.id == id)
    }

    pub fn select_model(&mut self, slot: u32, id: u32) -> Result<(), String> {
        self.require_legacy_electric()?;
        self.edit_availability(slot, None)?;
        if !crate::edit::model_allowed(slot, id) {
            return Err("model ID is not an original UI choice for this slot".into());
        }
        let model = self
            .models
            .iter_mut()
            .find(|m| m.slot == slot)
            .ok_or("model slot does not exist")?;
        self.bytes[model.info_offset..model.info_offset + 4].copy_from_slice(&id.to_be_bytes());
        model.id = id;
        Ok(())
    }

    /// Original `L6VariaxEditor.UpdateControlValuesFromPickupModels`.
    /// Either humbucker pickup selects 500 kΩ, 500 kΩ and 0.047 µF.
    /// Otherwise the values are 250 kΩ, 250 kΩ and 0.022 µF.
    /// Volume taper and the saved tone-knob position stay as they are.
    pub fn apply_pickup_control_values(&mut self) -> Result<(), String> {
        let humbucker = [crate::edit::PICKUP1, crate::edit::PICKUP2]
            .into_iter()
            .any(|slot| {
                self.model(slot)
                    .is_some_and(|model| crate::edit::is_humbucker_model(model.id))
            });
        let (volume, tone, capacitance): (f32, f32, f32) = if humbucker {
            (500.0, 500.0, 0.047)
        } else {
            (250.0, 250.0, 0.022)
        };
        self.set_parameter(crate::edit::WIRING, 0x200000, 1, volume.to_bits())?;
        self.set_parameter(crate::edit::WIRING, 0x200002, 1, tone.to_bits())?;
        self.set_parameter(crate::edit::WIRING, 0x200003, 1, capacitance.to_bits())?;
        Ok(())
    }

    /// Turn a pickup or the alternate tuning on or off.
    ///
    /// The original `OnParamChange` for pickup enables `0x2020001` and
    /// `0x2030001` never leaves both pickups off: turning one off while the
    /// other is already off turns the other on in the same edit. A file that
    /// already has both off is left alone until an enable is edited. Alternate
    /// tuning has no interlock.
    pub fn set_enabled(&mut self, slot: u32, enabled: bool) -> Result<(), String> {
        self.require_legacy_electric()?;
        if !matches!(
            slot,
            crate::edit::PICKUP1 | crate::edit::PICKUP2 | crate::edit::TUNING
        ) {
            return Err("only pickup and alternate-tuning enables are editable".into());
        }
        self.edit_availability(slot, None)?;
        if self.model(slot).is_none() {
            return Err("model slot does not exist".into());
        }
        let other = match slot {
            crate::edit::PICKUP1 => Some(crate::edit::PICKUP2),
            crate::edit::PICKUP2 => Some(crate::edit::PICKUP1),
            _ => None,
        };
        let force_other = if enabled {
            None
        } else {
            other.filter(|other| self.model(*other).is_some_and(|model| !model.enabled))
        };
        self.write_enabled(slot, enabled)?;
        if let Some(other) = force_other {
            self.write_enabled(other, true)?;
        }
        Ok(())
    }

    fn write_enabled(&mut self, slot: u32, enabled: bool) -> Result<(), String> {
        let model = self
            .models
            .iter_mut()
            .find(|model| model.slot == slot)
            .ok_or("model slot does not exist")?;
        self.bytes[model.info_offset + 11] = u8::from(enabled);
        model.enabled = enabled;
        Ok(())
    }

    fn require_legacy_electric(&self) -> Result<(), String> {
        if !matches!(self.device_id, 0x70000 | 0x70004) {
            return Err("parameter editing is limited to legacy electric Variax presets".into());
        }
        Ok(())
    }

    pub fn edit_availability(&self, slot: u32, id: Option<u32>) -> Result<(), String> {
        crate::edit::availability(self.model(crate::edit::BODY).map(|m| m.id), slot, id)?;
        if slot == crate::edit::WIRING
            && id == Some(0x200004)
            && !self
                .parameter(slot, 0x200005)
                .is_some_and(|p| p.value_type == 0 && p.bits == 1)
        {
            return Err("enable saved tone-knob position before editing it".into());
        }
        Ok(())
    }

    pub fn rename(&mut self, name: &str) -> Result<(), String> {
        if self.is_podxt_live_target() {
            return Err(
                "the guitar editor does not rename a PODxt Live tone; use the offline POD librarian"
                    .into(),
            );
        }
        if name == self.name {
            return Ok(());
        }
        if self.limits_legacy_name() {
            reject_new_legacy_text(name)?;
        } else {
            let units: Vec<u16> = name.encode_utf16().collect();
            if units.len() > 32 || units.contains(&0) {
                return Err("name must contain at most 32 UTF-16 code units and no NUL".into());
            }
        }
        self.set_stored_name(name)
    }

    /// Write a name the way a file can already contain it, including text the
    /// guitar would refuse. A person editing the name uses [`rename`].
    pub fn set_stored_name(&mut self, name: &str) -> Result<(), String> {
        let units: Vec<u16> = name.encode_utf16().collect();
        if units.len() > 32 || units.contains(&0) {
            return Err("name must contain at most 32 UTF-16 code units and no NUL".into());
        }
        self.bytes[self.name_offset..self.name_offset + 64].fill(0);
        for (i, unit) in units.iter().enumerate() {
            self.bytes[self.name_offset + 2 * i..self.name_offset + 2 * i + 2]
                .copy_from_slice(&unit.to_be_bytes());
        }
        self.name = name.to_owned();
        Ok(())
    }

    /// Update one existing parameter, retaining its declared value type.
    /// Validate only the explicit edit. Existing unknown/out-of-domain fields
    /// remain lossless on import, rename and copy; failures do not mutate bytes.
    pub fn set_parameter(
        &mut self,
        slot: u32,
        id: u32,
        value_type: u32,
        bits: u32,
    ) -> Result<(), String> {
        self.require_legacy_electric()?;
        self.edit_availability(slot, Some(id))?;
        crate::edit::domain(slot, id)
            .ok_or("parameter has no recovered editable domain")?
            .validate(value_type, bits)?;
        let parameter = self
            .models
            .iter_mut()
            .find(|m| m.slot == slot)
            .and_then(|m| m.parameters.iter_mut().find(|p| p.id == id))
            .ok_or("model slot or parameter does not exist")?;
        if !matches!(value_type, 0 | 1) || parameter.value_type != value_type {
            return Err("value type does not match the stored parameter".into());
        }
        self.bytes[parameter.value_offset..parameter.value_offset + 4]
            .copy_from_slice(&bits.to_be_bytes());
        parameter.bits = bits;
        Ok(())
    }

    /// Stable summary shared with the original-Java verification harness.
    pub fn summary(&self) -> String {
        let mut out = format!(
            "name\t{}\ndevice\t{:08x}\nversion\t{:08x}\n",
            self.name, self.device_id, self.device_version
        );
        let mut models: Vec<_> = self.models.iter().collect();
        models.sort_by_key(|m| m.slot);
        for model in models {
            writeln!(
                out,
                "model\t{:08x}\t{:08x}\t{}\t{}",
                model.slot, model.id, model.ordinal, model.enabled
            )
            .unwrap();
            let mut parameters: Vec<_> = model.parameters.iter().collect();
            parameters.sort_by_key(|p| p.id);
            for parameter in parameters {
                writeln!(
                    out,
                    "param\t{:08x}\t{:06x}\t{:08x}\t{:08x}",
                    model.slot, parameter.id, parameter.value_type, parameter.bits
                )
                .unwrap();
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }

    fn example() -> Vec<u8> {
        let mut info = vec![0; 76];
        info[..4].copy_from_slice(&1u32.to_be_bytes());
        info[4..8].copy_from_slice(&0x70004u32.to_be_bytes());
        info[8..10].copy_from_slice(&('A' as u16).to_be_bytes());
        let mut minfo = vec![];
        minfo.extend_from_slice(&0x03110004u32.to_be_bytes());
        minfo.extend_from_slice(&0x02010001u32.to_be_bytes());
        minfo.extend_from_slice(&[0, 0, 0, 1]);
        let mut param = vec![];
        for value in [0x3f200000u32, 1, 2.5f32.to_bits()] {
            param.extend_from_slice(&value.to_be_bytes());
        }
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minfo));
        model.extend(chunk(b"PARM", &param));
        let mut patch = b"PATC".to_vec();
        patch.extend(chunk(b"LIST", &model));
        let mut data = b"L6PA".to_vec();
        data.extend(chunk(b"PINF", &info));
        data.extend(chunk(b"JUNK", &[9, 8, 7]));
        data.extend(chunk(b"LIST", &patch));
        chunk(b"FORM", &data)
    }

    #[test]
    fn preset_text_preserves_other_chunks_and_rejects_ambiguous_fields() {
        let make = |list: Vec<u8>| {
            let mut bytes = example();
            bytes.extend(chunk(b"LIST", &list));
            let len = (bytes.len() - 8) as u32;
            bytes[4..8].copy_from_slice(&len.to_be_bytes());
            PresetFile::parse(bytes).unwrap()
        };
        let unknown = chunk(b"JUNK", &[8, 7, 6]);
        let mut list = b"UNFO".to_vec();
        list.extend_from_slice(&unknown);
        let mut preset = make(list.clone());
        preset.set_metadata(PresetText::Author, "Ana").unwrap();
        preset
            .set_metadata(PresetText::Notes, "First\nSecond 🎸")
            .unwrap();
        let reopened = PresetFile::parse(preset.as_bytes().to_vec()).unwrap();
        assert_eq!(reopened.metadata(PresetText::Author).unwrap(), "Ana");
        assert_eq!(
            reopened.metadata(PresetText::Notes).unwrap(),
            "First\nSecond 🎸"
        );
        assert!(
            reopened
                .as_bytes()
                .windows(unknown.len())
                .any(|bytes| bytes == unknown)
        );
        assert_eq!(
            reopened.models(),
            PresetFile::parse(example()).unwrap().models()
        );

        let field = chunk(b"IAUT", &[0, b'A']);
        list.extend_from_slice(&field);
        list.extend_from_slice(&field);
        let mut ambiguous = make(list);
        let original = ambiguous.as_bytes().to_vec();
        assert!(
            ambiguous
                .set_metadata(PresetText::Author, "Someone")
                .is_err()
        );
        assert_eq!(ambiguous.as_bytes(), original);

        let mut info = b"INFO".to_vec();
        info.extend(chunk(b"IAUT", b"A"));
        info.extend_from_slice(&unknown);
        let mut legacy = make(info);
        assert_eq!(legacy.metadata(PresetText::Author).unwrap(), "A");
        legacy.set_metadata(PresetText::Author, "New").unwrap();
        assert_eq!(legacy.metadata(PresetText::Author).unwrap(), "New");
        assert!(legacy.as_bytes().ends_with(&unknown));
    }

    #[test]
    fn preserves_unknown_chunks_and_edits_only_selected_bytes() {
        let original = example();
        let mut patch = PresetFile::parse(original.clone()).unwrap();
        assert_eq!(patch.as_bytes(), original);
        let before = patch.models[0].parameters[0].clone();
        patch
            .set_parameter(0x02010001, 0x200000, 1, 3.25f32.to_bits())
            .unwrap();
        for (i, (&a, &b)) in original.iter().zip(patch.as_bytes()).enumerate() {
            if !(before.value_offset..before.value_offset + 4).contains(&i) {
                assert_eq!(a, b);
            }
        }
        let reread = PresetFile::parse(patch.as_bytes().to_vec()).unwrap();
        assert_eq!(reread.models[0].parameters[0].bits, 3.25f32.to_bits());
        assert!(patch.set_parameter(0x02010001, 0x200000, 0, 3).is_err());
        assert!(patch.set_parameter(0x02010001, 0xffffff, 1, 0).is_err());
    }

    fn stored_name(bytes: Vec<u8>, name: &str) -> PresetFile {
        let preset = PresetFile::parse(bytes).unwrap();
        let mut bytes = preset.as_bytes().to_vec();
        let encoded: Vec<u8> = name.encode_utf16().flat_map(u16::to_be_bytes).collect();
        assert!(encoded.len() <= 64);
        bytes[preset.name_offset..preset.name_offset + 64].fill(0);
        bytes[preset.name_offset..preset.name_offset + encoded.len()].copy_from_slice(&encoded);
        PresetFile::parse(bytes).unwrap()
    }

    #[test]
    fn legacy_names_and_authors_stop_at_18_printable_ascii_and_keep_older_text() {
        let mut patch = PresetFile::parse(example()).unwrap();
        assert!(patch.limits_legacy_name());
        let before = patch.as_bytes().to_vec();
        patch.rename("123456789012345678").unwrap();
        assert_eq!(patch.name(), "123456789012345678");
        let accepted = patch.as_bytes().to_vec();
        assert!(patch.rename("1234567890123456789").is_err());
        assert!(patch.rename("cafe\u{e9}").is_err());
        assert!(patch.rename("bad\u{7f}").is_err());
        assert!(patch.rename("has\nline").is_err());
        assert_eq!(patch.as_bytes(), accepted);
        patch.rename("space and ~ ok").unwrap();
        assert_eq!(patch.name(), "space and ~ ok");
        let mut document = crate::document::EditorDocument::from_preset(
            PresetFile::parse(example()).unwrap(),
            std::path::PathBuf::from("name.vwm"),
        );
        document
            .apply(|preset| preset.rename("123456789012345678"))
            .unwrap();
        assert!(
            document
                .apply(|preset| preset.rename("1234567890123456789"))
                .is_err()
        );
        assert_eq!(document.preset().name(), "123456789012345678");
        document.undo();
        assert_eq!(document.preset().name(), "A");
        assert!(!document.can_undo());

        let kept = stored_name(before.clone(), "Guitare \u{1f3b8}");
        assert_eq!(kept.name(), "Guitare \u{1f3b8}");
        assert!(
            legacy_text_issue(kept.name(), true)
                .unwrap()
                .contains("Kept as saved")
        );
        let kept_bytes = kept.as_bytes().to_vec();
        let mut same = kept.clone();
        let same_name = same.name().to_owned();
        same.rename(&same_name).unwrap();
        assert_eq!(same.as_bytes(), kept_bytes);
        let mut other = kept.clone();
        other
            .set_parameter(0x02010001, 0x200000, 1, 1.5f32.to_bits())
            .unwrap();
        assert_eq!(other.name(), "Guitare \u{1f3b8}");
        other.rename("Short").unwrap();
        assert_eq!(other.name(), "Short");

        let mut author = PresetFile::parse(example()).unwrap();
        let author_before = author.as_bytes().to_vec();
        assert!(
            author
                .set_metadata(PresetText::Author, "1234567890123456789")
                .is_err()
        );
        assert!(
            author
                .set_metadata(PresetText::Author, "Ana \u{1f3b8}")
                .is_err()
        );
        assert_eq!(author.as_bytes(), author_before);
        author
            .set_metadata(PresetText::Author, "123456789012345678")
            .unwrap();
        assert_eq!(
            author.metadata(PresetText::Author).unwrap(),
            "123456789012345678"
        );

        let mut list = b"UNFO".to_vec();
        let emoji = "Ana \u{1f3b8}"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>();
        list.extend(chunk(b"IAUT", &emoji));
        let mut wrapped = example();
        wrapped.extend(chunk(b"LIST", &list));
        let len = (wrapped.len() - 8) as u32;
        wrapped[4..8].copy_from_slice(&len.to_be_bytes());
        let opened = PresetFile::parse(wrapped.clone()).unwrap();
        assert_eq!(
            opened.metadata(PresetText::Author).unwrap(),
            "Ana \u{1f3b8}"
        );
        assert_eq!(opened.as_bytes(), wrapped);
        assert!(legacy_text_issue(&opened.metadata(PresetText::Author).unwrap(), true).is_some());
        let mut notes = opened.clone();
        notes
            .set_metadata(PresetText::Notes, "Still free \u{1f3b8}")
            .unwrap();
        assert_eq!(notes.metadata(PresetText::Author).unwrap(), "Ana \u{1f3b8}");

        let mut other_device = PresetFile::parse(example()).unwrap();
        let at = other_device.name_offset - 4;
        other_device.bytes[at..at + 4].copy_from_slice(&0x0001_0001u32.to_be_bytes());
        let mut other_device = PresetFile::parse(other_device.bytes).unwrap();
        assert!(!other_device.limits_legacy_name());
        other_device.rename("Guitare \u{1f3b8}").unwrap();
        assert_eq!(other_device.name(), "Guitare \u{1f3b8}");
    }

    #[test]
    fn imported_unknown_and_out_of_domain_fields_are_preserved_without_becoming_editable() {
        let mut patch = PresetFile::parse(example()).unwrap();
        let offset = patch.models[0].parameters[0].value_offset;
        patch.bytes[offset..offset + 4].copy_from_slice(&f32::NAN.to_bits().to_be_bytes());
        let mut imported = PresetFile::parse(patch.bytes).unwrap();
        let before = imported.as_bytes().to_vec();
        assert!(
            imported
                .set_parameter(0x02010001, 0x200000, 1, f32::INFINITY.to_bits())
                .is_err()
        );
        assert_eq!(imported.as_bytes(), before);
        imported.rename("Preserved").unwrap();
        assert!(f32::from_bits(imported.parameter(0x02010001, 0x200000).unwrap().bits).is_nan());
        imported.bytes[offset - 8..offset - 4].copy_from_slice(&0x3fffffffu32.to_be_bytes());
        imported.bytes[offset - 4..offset].copy_from_slice(&77u32.to_be_bytes());
        let mut unknown = PresetFile::parse(imported.bytes).unwrap();
        let before = unknown.as_bytes().to_vec();
        assert!(unknown.set_parameter(0x02010001, 0xffffff, 77, 0).is_err());
        assert_eq!(unknown.as_bytes(), before);
        unknown.rename("Unknown retained").unwrap();
        assert_eq!(
            &unknown.as_bytes()[offset - 8..offset + 4],
            &before[offset - 8..offset + 4]
        );
    }

    fn enable_offset(bytes: &[u8], slot: u32) -> usize {
        let pattern = slot.to_be_bytes();
        bytes
            .windows(4)
            .enumerate()
            .find_map(|(index, window)| {
                (window == pattern && index >= 4 && bytes[index - 4] == 0x03).then_some(index + 7)
            })
            .unwrap()
    }

    fn host_preset() -> PresetFile {
        PresetFile::parse(
            include_bytes!("../../../../tests/fixtures/synthetic-host-preset.vwm").to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn turning_the_last_pickup_off_turns_the_other_on_in_the_same_edit() {
        use crate::edit::{BODY, PICKUP1, PICKUP2, TUNING};

        let mut preset = host_preset();
        assert!(preset.model(PICKUP1).unwrap().enabled);
        assert!(preset.model(PICKUP2).unwrap().enabled);
        let before = preset.as_bytes().to_vec();
        preset.set_enabled(PICKUP2, false).unwrap();
        assert!(!preset.model(PICKUP2).unwrap().enabled);
        assert!(preset.model(PICKUP1).unwrap().enabled);
        let pickup2 = enable_offset(&before, PICKUP2);
        assert_eq!(preset.as_bytes()[pickup2], 0);
        assert!(
            before
                .iter()
                .zip(preset.as_bytes())
                .enumerate()
                .all(|(index, (left, right))| index == pickup2 || left == right)
        );

        let mut document = crate::document::EditorDocument::from_preset(
            preset,
            std::path::PathBuf::from("memory.vwm"),
        );
        document
            .apply(|preset| preset.set_enabled(PICKUP1, false))
            .unwrap();
        assert!(!document.preset().model(PICKUP1).unwrap().enabled);
        assert!(document.preset().model(PICKUP2).unwrap().enabled);
        assert!(document.can_undo());
        assert!(!document.can_redo());
        document.undo();
        assert!(document.preset().model(PICKUP1).unwrap().enabled);
        assert!(!document.preset().model(PICKUP2).unwrap().enabled);
        assert!(!document.can_undo());
        document.redo();
        assert!(!document.preset().model(PICKUP1).unwrap().enabled);
        assert!(document.preset().model(PICKUP2).unwrap().enabled);

        let mut tuning = host_preset();
        let tuning_was = tuning.model(TUNING).unwrap().enabled;
        let tuning_before = tuning.as_bytes().to_vec();
        tuning.set_enabled(TUNING, !tuning_was).unwrap();
        assert_eq!(tuning.model(TUNING).unwrap().enabled, !tuning_was);
        assert!(tuning.model(PICKUP1).unwrap().enabled);
        assert!(tuning.model(PICKUP2).unwrap().enabled);
        assert_eq!(
            tuning_before
                .iter()
                .zip(tuning.as_bytes())
                .filter(|(left, right)| left != right)
                .count(),
            1
        );

        let mut acoustic = host_preset();
        acoustic.select_model(BODY, 0x03100012).unwrap();
        let acoustic_before = acoustic.as_bytes().to_vec();
        assert!(acoustic.set_enabled(PICKUP1, false).is_err());
        assert_eq!(acoustic.as_bytes(), acoustic_before);

        let mut only_one = PresetFile::parse(example()).unwrap();
        let single_before = only_one.as_bytes().to_vec();
        only_one.set_enabled(PICKUP1, false).unwrap();
        assert!(!only_one.model(PICKUP1).unwrap().enabled);
        assert_eq!(
            only_one.as_bytes()[enable_offset(only_one.as_bytes(), PICKUP1)],
            0
        );
        assert_eq!(
            single_before
                .iter()
                .zip(only_one.as_bytes())
                .filter(|(left, right)| left != right)
                .count(),
            1
        );
    }

    #[test]
    fn a_file_with_both_pickups_off_stays_off_until_an_enable_is_edited() {
        use crate::edit::{PICKUP1, PICKUP2};

        let mut bytes =
            include_bytes!("../../../../tests/fixtures/synthetic-host-preset.vwm").to_vec();
        for slot in [PICKUP1, PICKUP2] {
            let at = enable_offset(&bytes, slot);
            bytes[at] = 0;
        }
        let opened = PresetFile::parse(bytes.clone()).unwrap();
        assert!(!opened.model(PICKUP1).unwrap().enabled);
        assert!(!opened.model(PICKUP2).unwrap().enabled);
        assert_eq!(opened.as_bytes(), bytes);

        let mut turned_on = opened.clone();
        turned_on.set_enabled(PICKUP1, true).unwrap();
        assert!(turned_on.model(PICKUP1).unwrap().enabled);
        assert!(!turned_on.model(PICKUP2).unwrap().enabled);

        let mut forced = PresetFile::parse(bytes).unwrap();
        forced.set_enabled(PICKUP2, false).unwrap();
        assert!(forced.model(PICKUP1).unwrap().enabled);
        assert!(!forced.model(PICKUP2).unwrap().enabled);
    }

    #[test]
    fn pickup_models_select_humbucker_or_single_coil_control_values() {
        use crate::edit::{PICKUP1, PICKUP2, WIRING, is_humbucker_model};

        assert!(is_humbucker_model(0x03118000));
        assert!(is_humbucker_model(0x03118009));
        assert!(is_humbucker_model(0x00118000));
        assert!(!is_humbucker_model(0x03110000));
        assert!(!is_humbucker_model(0x03110015));
        assert!(!is_humbucker_model(0x03117000));

        let mut preset = host_preset();
        preset.select_model(PICKUP1, 0x03110004).unwrap();
        preset.select_model(PICKUP2, 0x03110003).unwrap();
        preset
            .set_parameter(WIRING, 0x200000, 1, 500.0_f32.to_bits())
            .unwrap();
        preset
            .set_parameter(WIRING, 0x200002, 1, 470.0_f32.to_bits())
            .unwrap();
        preset
            .set_parameter(WIRING, 0x200003, 1, 0.011_f32.to_bits())
            .unwrap();
        let taper = preset.parameter(WIRING, 0x200001).unwrap().bits;
        let position = preset.parameter(WIRING, 0x200004).unwrap().bits;
        preset.apply_pickup_control_values().unwrap();
        assert_eq!(
            preset.parameter(WIRING, 0x200000).unwrap().bits,
            250.0_f32.to_bits()
        );
        assert_eq!(
            preset.parameter(WIRING, 0x200002).unwrap().bits,
            250.0_f32.to_bits()
        );
        assert_eq!(
            preset.parameter(WIRING, 0x200003).unwrap().bits,
            0.022_f32.to_bits()
        );
        assert_eq!(preset.parameter(WIRING, 0x200001).unwrap().bits, taper);
        assert_eq!(preset.parameter(WIRING, 0x200004).unwrap().bits, position);

        preset.select_model(PICKUP1, 0x03118000).unwrap();
        preset.apply_pickup_control_values().unwrap();
        assert_eq!(
            preset.parameter(WIRING, 0x200000).unwrap().bits,
            500.0_f32.to_bits()
        );
        assert_eq!(
            preset.parameter(WIRING, 0x200003).unwrap().bits,
            0.047_f32.to_bits()
        );

        preset.select_model(PICKUP1, 0x03110015).unwrap();
        preset.select_model(PICKUP2, 0x03118009).unwrap();
        preset.apply_pickup_control_values().unwrap();
        assert_eq!(
            preset.parameter(WIRING, 0x200000).unwrap().bits,
            500.0_f32.to_bits()
        );
        assert_eq!(
            preset.parameter(WIRING, 0x200002).unwrap().bits,
            500.0_f32.to_bits()
        );

        let before = preset.as_bytes().to_vec();
        let mut document = crate::document::EditorDocument::from_preset(
            preset,
            std::path::PathBuf::from("controls.vwm"),
        );
        document
            .apply(|preset| {
                preset.select_model(PICKUP2, 0x03110003)?;
                preset.apply_pickup_control_values()
            })
            .unwrap();
        assert_eq!(document.preset().model(PICKUP2).unwrap().id, 0x03110003);
        assert_eq!(
            document.preset().parameter(WIRING, 0x200000).unwrap().bits,
            250.0_f32.to_bits()
        );
        assert!(document.can_undo());
        document.undo();
        assert_eq!(document.as_bytes(), before);
        assert!(!document.can_undo());
    }

    #[test]
    fn guitar_editor_does_not_rename_a_podxt_live_tone() {
        let mut bytes = example();
        bytes[24..28].copy_from_slice(&0x0003_000au32.to_be_bytes());
        let mut preset = PresetFile::parse(bytes).unwrap();
        assert!(preset.is_podxt_live_target());
        let before = preset.as_bytes().to_vec();
        let error = preset.rename("Nope").unwrap_err();
        assert!(error.contains("PODxt Live"));
        assert_eq!(preset.as_bytes(), before);
        assert!(preset.set_parameter(0x02010001, 0x200000, 1, 0).is_err());
        assert_eq!(preset.as_bytes(), before);
    }

    #[test]
    fn rejects_truncated_and_out_of_bounds_chunks_without_panicking() {
        let source = example();
        for end in 0..source.len() {
            assert!(PresetFile::parse(source[..end].to_vec()).is_err());
            if end >= 12 {
                // A valid outer length must not hide truncated inner chunks.
                let mut truncated = source[..end].to_vec();
                truncated[4..8].copy_from_slice(&((end - 8) as u32).to_be_bytes());
                assert!(PresetFile::parse(truncated).is_err());
            }
        }
        let mut broken = source;
        broken[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(PresetFile::parse(broken).is_err());
    }
}
