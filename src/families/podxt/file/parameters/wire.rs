use super::*;

pub const PODXT_PAYLOAD_BYTES: usize = 160;
pub const PODXT_BANKS: [(&str, ContainerKind, usize); 3] = [
    ("Tones", ContainerKind::L6pa, 128),
    ("Amp Setups", ContainerKind::L6as, 107),
    ("FX Setups", ContainerKind::L6fs, 64),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodVariaxLink {
    pub family: u8,
    pub model: u8,
    pub label: String,
}

impl PodVariaxLink {
    pub fn recalls_model(&self) -> bool {
        self.family != 127 && self.model != 0
    }
}

pub fn pod_variax_link(payload: &[u8; PODXT_PAYLOAD_BYTES]) -> PodVariaxLink {
    let table = pod_parameter_table();
    let family = payload[32 + 101];
    let offset = [120, 96, 122, 126, 99].get(usize::from(family));
    let model = offset.map_or(0, |&cc| payload[32 + cc]);
    let label = if family == 127 {
        "No Variax".to_owned()
    } else if offset.is_none() {
        format!("Unknown Variax family {family}")
    } else if model == 0 {
        "Keep current Variax model".to_owned()
    } else {
        let key = format!("VARIAX_FAMILY_{family}_MODEL");
        table
            .formats
            .get(&key)
            .and_then(|format| {
                format.choices.iter().find_map(|choice| match choice {
                    Choice::Integer((value, label)) if *value == u32::from(model) => {
                        Some(label.clone())
                    }
                    _ => None,
                })
            })
            .unwrap_or_else(|| format!("Variax family {family}, model {model}"))
    };
    PodVariaxLink {
        family,
        model,
        label,
    }
}

fn rows() -> impl Iterator<Item = &'static PodParameter> {
    pod_parameter_table().parameters.iter().filter(|row| {
        row.availability.as_deref() == Some("Live")
            && !row.tone_offsets.is_empty()
            && row.file_field.is_some()
    })
}

fn encode_name(target: &mut [u8], name: &str) {
    target.fill(b' ');
    for (byte, character) in target.iter_mut().zip(name.encode_utf16()) {
        *byte = match character as u8 {
            0 => b' ',
            b' '..=b'z' if character as u8 != b'`' => character as u8,
            _ => b'_',
        };
    }
}

impl PodTone {
    /// Encode the file's PODxt controls using Line 6 Edit's whole-tone fields.
    /// Metadata outside PINF and parameters outside the PODxt profile stay in the file.
    pub fn to_podxt_payload(&self) -> Result<[u8; PODXT_PAYLOAD_BYTES], ToneError> {
        self.editable_table_tone()?;
        let mut payload = [0; PODXT_PAYLOAD_BYTES];
        encode_name(&mut payload[..32], self.name());
        // EncodePatch derives amp enable from the model flag and sets pedal level to full.
        payload[32 + 111] = 127;
        payload[32 + 7] = 127;
        for row in rows() {
            if row.tone_encoding.as_deref() == Some("constant-127") {
                continue;
            }
            let Some(index) = self.table_model(&row.slots)? else {
                continue;
            };
            let model = &self.models[index];
            let definition = definition(row, Some(model.model_id));
            if row.file_field == Some(FileField::Parameter) && definition.is_none() {
                continue;
            }
            let Some(_) = self.read_table_bits(row, definition)? else {
                continue;
            };
            let Some(fields) = self.table_tone_field(&row.key) else {
                // The original editor does not send model-specific controls outside their layout.
                if !row.applicable_amp_ids.is_empty() {
                    continue;
                }
                return Err(ToneError::format(format!(
                    "Cannot encode PODxt field {}.",
                    row.key
                )));
            };
            for (offset, value) in fields {
                payload[offset] = value;
            }
        }
        Ok(payload)
    }

    pub fn from_podxt_payload(
        payload: &[u8; PODXT_PAYLOAD_BYTES],
        kind: ContainerKind,
    ) -> Result<Self, ToneError> {
        if payload.iter().any(|&value| value > 127) {
            return Err(ToneError::format(
                "Expected a seven-bit PODxt Live tone payload.",
            ));
        }
        let name = String::from_utf8_lossy(&payload[..16])
            .trim_end_matches([' ', '\0'])
            .to_owned();
        let mut slots = BTreeMap::<u32, DecodedModel>::new();
        let routing: Vec<_> = rows()
            .filter(|row| row.file_field == Some(FileField::Slot))
            .map(|row| {
                (
                    row.slots.as_slice(),
                    usize::from(payload[row.tone_offsets[0]] > 63),
                )
            })
            .collect();
        let slot_for = |row: &PodParameter| {
            routing
                .iter()
                .find(|(pair, _)| pair.contains(&row.slots[0]))
                .map_or(row.slots[0], |(pair, choice)| pair[*choice])
        };
        let table = pod_parameter_table();
        for row in rows().filter(|row| row.file_field == Some(FileField::Model)) {
            let catalog = row.catalog.as_deref();
            let index = payload[row.tone_offsets[0]];
            let model = table
                .catalog
                .iter()
                .find(|model| Some(model.block.as_str()) == catalog && model.index == index)
                .or_else(|| {
                    table
                        .catalog
                        .iter()
                        .find(|model| Some(model.block.as_str()) == catalog && model.index == 0)
                })
                .ok_or_else(|| ToneError::format("Missing PODxt model catalog."))?;
            slots.insert(slot_for(row), DecodedModel::new(model.id, true));
        }
        for row in rows().filter(|row| {
            matches!(
                row.file_field,
                Some(FileField::Enable | FileField::Parameter)
            )
        }) {
            let slot = slot_for(row);
            let model = slots.entry(slot).or_insert_with(|| {
                let id = fixed_model_id(slot).unwrap_or_else(|| match slot {
                    0x0001_0000 => 0x3e70_0000,
                    0x0002_0001 | 0x0005_0002 => 0x0207_0000,
                    _ => unreachable!("all table model slots have an explicit model"),
                });
                DecodedModel::new(id, true)
            });
            let bytes = &row.tone_offsets;
            let high = payload[bytes[0]];
            if row.file_field == Some(FileField::Enable) {
                model.enabled = if row.tone_encoding.as_deref() == Some("inverted-switch") {
                    high < 64
                } else {
                    high > 63
                };
                continue;
            }
            let Some(definition) = definition(row, Some(model.id)) else {
                continue;
            };
            let raw = |scale: f32| {
                let combined = (u16::from(high) << 7) | u16::from(payload[bytes[1]]);
                (f32::from(combined) / scale).to_bits()
            };
            let bits = match row.tone_encoding.as_deref() {
                Some("integer") => u32::from(high),
                Some("switch") => u32::from(high > 63),
                Some("float7-nearest" | "constant-127") => (f32::from(high) / 127.0).to_bits(),
                Some("float14-truncate") => raw(16383.0),
                Some("tempo-tenths") => raw(10.0),
                Some("gate-db-ranged") => (-f32::from(high)).to_bits(),
                Some("pedal-assignment") => u32::from(if high < 32 {
                    0u8
                } else if high < 96 {
                    1
                } else {
                    2
                }),
                _ => continue,
            };
            model
                .parameters
                .insert(row.parm.expect("PARM id"), (definition.host_type, bits));
        }
        tone_file(kind, &name, slots)
    }

    /// Local initial settings. Firmware model defaults still use the editor's
    /// existing default-unknown path.
    pub fn new_podxt_tone(name: &str) -> Result<Self, ToneError> {
        let mut payload = [0; PODXT_PAYLOAD_BYTES];
        // Line 6 Edit 3.06's Default Tone projected through EncodePatch.
        // The starter explicitly has no Variax link.
        const DEFAULT_CONTROLLERS: &[(usize, u8)] = &[
            (4, 63),
            (7, 127),
            (9, 127),
            (13, 62),
            (14, 62),
            (15, 63),
            (16, 63),
            (17, 61),
            (18, 25),
            (20, 10),
            (21, 64),
            (29, 4),
            (30, 31),
            (31, 6),
            (33, 64),
            (34, 63),
            (35, 64),
            (36, 127),
            (39, 64),
            (40, 64),
            (41, 127),
            (42, 28),
            (47, 127),
            (52, 63),
            (53, 63),
            (54, 63),
            (56, 63),
            (57, 127),
            (60, 36),
            (61, 37),
            (62, 3),
            (77, 65),
            (79, 127),
            (80, 64),
            (81, 64),
            (85, 64),
            (87, 127),
            (89, 9),
            (90, 48),
            (101, 127),
            (105, 99),
            (111, 127),
            (114, 64),
            (116, 64),
            (117, 64),
            (119, 64),
        ];
        for &(cc, value) in DEFAULT_CONTROLLERS {
            payload[32 + cc] = value;
        }
        let mut tone = Self::from_podxt_payload(&payload, ContainerKind::L6pa)?;
        tone.rename(name)?;
        Ok(tone)
    }
}

fn definition(row: &PodParameter, model: Option<u32>) -> Option<&'static Definition> {
    let table = pod_parameter_table();
    let key = match row.definition.as_deref()? {
        "model-specific" => table
            .layout(&row.block, model?)?
            .knobs
            .get(&row.key)?
            .as_str(),
        _ if row.bass_cab_ids.contains(&model.unwrap_or(0)) => {
            row.bass_cab_definition.as_deref()?
        }
        other => other,
    };
    table.definitions.get(key)
}

struct DecodedModel {
    id: u32,
    enabled: bool,
    parameters: BTreeMap<u32, (HostType, u32)>,
}

impl DecodedModel {
    fn new(id: u32, enabled: bool) -> Self {
        Self {
            id,
            enabled,
            parameters: BTreeMap::new(),
        }
    }
}

fn unicode_name(data: &mut Vec<u8>, name: &str) {
    for unit in name.encode_utf16().chain(std::iter::repeat(0)).take(32) {
        data.extend(unit.to_be_bytes());
    }
}

fn tone_file(
    kind: ContainerKind,
    name: &str,
    slots: BTreeMap<u32, DecodedModel>,
) -> Result<PodTone, ToneError> {
    let mut info = Vec::from(1u32.to_be_bytes());
    info.extend(PODXT_LIVE_TONE_ID.to_be_bytes());
    unicode_name(&mut info, name);
    info.extend(0x0301_0000u32.to_be_bytes());

    let mut patch = b"PATC".to_vec();
    for (slot, model) in slots {
        let mut minf = Vec::from(model.id.to_be_bytes());
        minf.extend(slot.to_be_bytes());
        minf.extend([0, 0, 0, u8::from(model.enabled)]);
        let mut data = b"MODL".to_vec();
        data.extend(iff_chunk(b"MINF", &minf));
        for (parm, (kind, bits)) in model.parameters {
            let mut value = Vec::from((0x3f00_0000 | parm).to_be_bytes());
            value.extend(u32::from(kind == HostType::Float32).to_be_bytes());
            value.extend(bits.to_be_bytes());
            data.extend(iff_chunk(b"PARM", &value));
        }
        patch.extend(iff_chunk(b"LIST", &data));
    }
    let mut form = kind.tag().as_bytes().to_vec();
    form.extend(iff_chunk(b"PINF", &info));
    form.extend(iff_chunk(b"LIST", &patch));
    form.extend(iff_chunk(b"LIST", b"UNFO"));
    PodTone::parse(iff_chunk(b"FORM", &form))
}

/// Create either a complete Line 6 Edit L6BA backup or an L6CO collection.
pub fn pod_bundle_from_tones(
    fixed: bool,
    banks: &[(&str, Vec<PodTone>)],
) -> Result<PodBundleLibrary, ToneError> {
    if fixed
        && (banks.len() != PODXT_BANKS.len()
            || banks
                .iter()
                .zip(PODXT_BANKS)
                .any(|((name, entries), (expected, kind, count))| {
                    *name != expected
                        || entries.len() != count
                        || entries.iter().any(|tone| tone.container() != kind)
                }))
    {
        return Err(ToneError::format(
            "A PODxt Live bundle needs 128 tones, 107 amp setups and 64 FX setups in device order.",
        ));
    }
    let mut form = if fixed {
        b"L6BA".to_vec()
    } else {
        b"L6CO".to_vec()
    };
    form.extend(iff_chunk(b"HEAD", &1u32.to_be_bytes()));
    for &(name, ref entries) in banks {
        let mut bank = if fixed {
            b"BANK".to_vec()
        } else {
            b"FLDR".to_vec()
        };
        let mut info = Vec::from(0u32.to_be_bytes());
        unicode_name(&mut info, name);
        bank.extend(iff_chunk(b"BINF", &info));
        for tone in entries {
            bank.extend_from_slice(tone.as_bytes());
            if tone.as_bytes().len() % 2 != 0 {
                bank.push(0);
            }
        }
        form.extend(iff_chunk(b"LIST", &bank));
    }
    PodBundleLibrary::parse(iff_chunk(b"FORM", &form))
}

pub fn new_podxt_bundle(fixed: bool) -> Result<PodBundleLibrary, ToneError> {
    let tone = PodTone::new_podxt_tone("New Tone")?;
    if !fixed {
        return pod_bundle_from_tones(false, &[("Tones", vec![tone])]);
    }
    let mut banks = Vec::new();
    let mut payload = tone.to_podxt_payload()?;
    for (name, kind, count) in PODXT_BANKS {
        let entries = (0..count)
            .map(|index| {
                payload[32 + 12] = if kind == ContainerKind::L6as {
                    index as u8
                } else {
                    0
                };
                let mut tone = PodTone::from_podxt_payload(&payload, kind)?;
                let name = match kind {
                    ContainerKind::L6pa => format!("New Tone {}", index + 1),
                    ContainerKind::L6as => pod_parameter_table()
                        .catalog
                        .iter()
                        .find(|model| model.block == "amp" && usize::from(model.index) == index)
                        .and_then(|model| pod_parameter_table().model_name(model.id))
                        .unwrap_or("Amp Setup")
                        .to_owned(),
                    ContainerKind::L6fs => "Default FX Setup".to_owned(),
                };
                tone.rename(&name)?;
                Ok(tone)
            })
            .collect::<Result<Vec<_>, ToneError>>()?;
        banks.push((name, entries));
    }
    pod_bundle_from_tones(true, &banks)
}

pub fn save_podxt_html<'a>(
    path: &Path,
    entries: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<(), ToneError> {
    use std::fmt::Write;
    fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#39;")
    }
    let mut html = String::from(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>PODxt Live tones</title>\n<h1>PODxt Live tones</h1>\n<table><thead><tr><th>Location</th><th>Tone</th></tr></thead><tbody>\n",
    );
    for (location, tone) in entries {
        writeln!(
            &mut html,
            "<tr><td>{}</td><td>{}</td></tr>",
            escape(location),
            escape(tone)
        )
        .expect("writing to String");
    }
    html.push_str("</tbody></table>\n");
    document::save_new(path, html.as_bytes()).map_err(ToneError::format)
}

#[cfg(test)]
mod tests;
