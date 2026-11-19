//! Lossless reader/editor for individual Workbench FORM/L6PA/L6FS/L6AS preset files.
//! Unknown chunks, ordering, padding and parameter bit patterns remain untouched.
//! File parameters are model-local IDs, not the MIDI metadata parameter IDs.

use std::{fmt::Write, ops::Range};

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

#[derive(Clone)]
pub(crate) struct Chunk {
    pub(crate) id: [u8; 4],
    pub(crate) data: Range<usize>,
}

pub(crate) fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().expect("validated range"))
}

/// Parse only the children of a validated range, with word-aligned leaf chunks.
pub(crate) fn chunks(bytes: &[u8], region: Range<usize>) -> Result<Vec<Chunk>, String> {
    let mut cursor = region.start;
    let mut result = Vec::new();
    while cursor < region.end {
        if region.end - cursor < 8 {
            return Err(format!("truncated chunk header at {cursor}"));
        }
        let length = word(bytes, cursor + 4) as usize;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|&n| n <= region.end)
            .ok_or_else(|| format!("chunk at {cursor} exceeds its container"))?;
        let id = bytes[cursor..cursor + 4].try_into().unwrap();
        if (id == *b"LIST" || id == *b"FORM") && length < 4 {
            return Err("container lacks its type code".into());
        }
        result.push(Chunk {
            id,
            data: start..end,
        });
        cursor = end
            .checked_add(length & 1)
            .filter(|&n| n <= region.end)
            .ok_or("missing odd-length chunk padding")?;
    }
    Ok(result)
}

pub(crate) fn unique<'a>(items: &'a [Chunk], id: &[u8; 4]) -> Result<&'a Chunk, String> {
    let mut found = items.iter().filter(|c| &c.id == id);
    let result = found
        .next()
        .ok_or_else(|| format!("missing {} chunk", String::from_utf8_lossy(id)))?;
    if found.next().is_some() {
        return Err(format!(
            "ambiguous duplicate {} chunks",
            String::from_utf8_lossy(id)
        ));
    }
    Ok(result)
}

pub(crate) fn container(bytes: &[u8], chunk: &Chunk, kind: &[u8; 4]) -> bool {
    (chunk.id == *b"FORM" || chunk.id == *b"LIST")
        && &bytes[chunk.data.start..chunk.data.start + 4] == kind
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

    pub fn set_enabled(&mut self, slot: u32, enabled: bool) -> Result<(), String> {
        self.require_legacy_electric()?;
        if !matches!(
            slot,
            crate::edit::PICKUP1 | crate::edit::PICKUP2 | crate::edit::TUNING
        ) {
            return Err("only pickup and alternate-tuning enables are editable".into());
        }
        self.edit_availability(slot, None)?;
        let model = self
            .models
            .iter_mut()
            .find(|m| m.slot == slot)
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

    #[test]
    fn unicode_names_are_lossless_and_overlong_names_are_rejected() {
        let mut patch = PresetFile::parse(example()).unwrap();
        patch.rename("Guitare 🎸").unwrap();
        assert_eq!(
            PresetFile::parse(patch.as_bytes().to_vec()).unwrap().name,
            "Guitare 🎸"
        );
        assert!(patch.rename(&"🎸".repeat(17)).is_err());
        assert!(patch.rename("bad\0name").is_err());
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
