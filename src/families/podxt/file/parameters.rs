use super::*;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

const TABLE: &str = include_str!("../../../../research/podxt-live-parameters.json");

#[derive(Deserialize)]
pub struct PodParameterTable {
    schema_version: u32,
    pub parameters: Vec<PodParameter>,
    pub catalog: Vec<PodModelChoice>,
    model_names: Vec<ModelName>,
    layouts: Vec<Layout>,
    definitions: BTreeMap<String, Definition>,
    formats: BTreeMap<String, DisplayFormat>,
    tweak_targets: TweakTargets,
    note_sync: NoteSync,
}

#[derive(Deserialize)]
pub struct PodParameter {
    pub key: String,
    pub block: String,
    pub selector: Option<u32>,
    availability: Option<String>,
    file_field: Option<FileField>,
    parm: Option<u32>,
    #[serde(default)]
    slots: Vec<u32>,
    definition: Option<String>,
    pub catalog: Option<String>,
    #[serde(default)]
    bass_cab_ids: Vec<u32>,
    bass_cab_definition: Option<String>,
    #[serde(default)]
    applicable_amp_ids: Vec<u32>,
    pub variax_family: Option<u32>,
    tone_encoding: Option<String>,
    #[serde(default)]
    pub tone_offsets: Vec<usize>,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
enum FileField {
    #[serde(rename = "MINF model ID")]
    Model,
    #[serde(rename = "MINF enabled")]
    Enable,
    #[serde(rename = "MINF slot ID")]
    Slot,
    #[serde(rename = "PARM")]
    Parameter,
}

#[derive(Deserialize)]
pub struct PodModelChoice {
    pub block: String,
    pub index: u8,
    pub id: u32,
    pub pack: u8,
}

#[derive(Deserialize)]
struct ModelName {
    id: u32,
    name: String,
}

#[derive(Deserialize)]
struct Layout {
    block: String,
    model_id: u32,
    knobs: BTreeMap<String, String>,
    editor_tweak_order: Vec<i32>,
}

#[derive(Deserialize)]
pub struct Definition {
    pub label: String,
    host_type: HostType,
    host_range: [f32; 2],
    format: String,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum HostType {
    Int,
    Float32,
}

#[derive(Deserialize)]
pub struct DisplayFormat {
    pub unit: Option<String>,
    pub decimals: Option<i32>,
    display_range: Option<[f32; 2]>,
    nine_points_hz: Option<[f64; 9]>,
    normalized_mapping_endpoints: Option<[f32; 2]>,
    alias: Option<String>,
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Choice {
    Integer((u32, String)),
    Association((u8, String, u32)),
}

#[derive(Deserialize)]
struct TweakTargets {
    ordinals: Vec<TweakTarget>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TweakTarget {
    Fixed([u32; 1]),
    Selector(u32),
    Layout { block: String, knob: usize },
}

#[derive(Deserialize)]
struct NoteSync {
    ordinal_1_to_13: [f32; 13],
}

pub fn pod_parameter_table() -> &'static PodParameterTable {
    static PARSED: OnceLock<PodParameterTable> = OnceLock::new();
    PARSED.get_or_init(|| {
        let parsed: PodParameterTable =
            serde_json::from_str(TABLE).expect("the committed PODxt table must be valid");
        assert_eq!(parsed.schema_version, 1);
        parsed
    })
}

impl PodParameterTable {
    pub fn parameter(&self, key: &str) -> Option<&PodParameter> {
        self.parameters.iter().find(|row| row.key == key)
    }

    pub fn model_name(&self, id: u32) -> Option<&str> {
        self.model_names
            .iter()
            .find(|model| model.id == id)
            .map(|model| model.name.as_str())
    }

    fn layout(&self, block: &str, id: u32) -> Option<&Layout> {
        self.layouts
            .iter()
            .find(|layout| layout.block == block && layout.model_id == id)
    }
}

pub struct PodControl<'a> {
    pub parameter: &'a PodParameter,
    pub definition: Option<&'a Definition>,
    pub format: Option<&'a DisplayFormat>,
    pub bits: Option<u32>,
    pub choices: Vec<(u32, &'a str)>,
}

impl PodControl<'_> {
    pub fn label(&self) -> &str {
        if self.parameter.catalog.is_some() {
            "Model"
        } else {
            self.definition.map_or(&self.parameter.key, |d| &d.label)
        }
    }

    pub fn displayed(&self, bits: u32) -> Option<f64> {
        let definition = self.definition?;
        let format = self.format?;
        if !definition.accepts(bits, &self.choices) {
            return None;
        }
        let [low, high] = definition.host_range;
        let raw = if definition.host_type == HostType::Float32 {
            f32::from_bits(bits)
        } else {
            bits as f32
        };
        if definition.format == "PERCENT" {
            let step = ((raw - low) / (high - low) * 127.0 + 0.5) as i32;
            return Some(f64::from(if step == 40 {
                32
            } else {
                (step as f32 * 100.0 / 127.0 + 0.5) as i32
            }));
        }
        if let Some(points) = format.nine_points_hz {
            let step = (f64::from(raw) * 127.0 + 0.5) as usize;
            if step >= 127 {
                return Some(points[8]);
            }
            let i = step >> 4;
            return Some(points[i] + (points[i + 1] - points[i]) * (step & 15) as f64 / 16.0);
        }
        if let Some([a, b]) = format.normalized_mapping_endpoints {
            let unit = (f64::from(raw - a) / f64::from(b - a)).clamp(0.0, 1.0);
            return Some(-24.0 + unit * 48.0);
        }
        match format.display_range {
            Some([min, max]) if [low, high] == [0.0, 1.0] => Some(
                (f64::from(min) + f64::from(raw) * f64::from(max - min))
                    .clamp(f64::from(min), f64::from(max)),
            ),
            _ => Some(f64::from(raw)),
        }
    }

    pub fn display_range(&self) -> Option<[f64; 2]> {
        let def = self.definition?;
        let fmt = self.format?;
        if def.format == "PERCENT" {
            return Some([0.0, 100.0]);
        }
        if let Some(points) = fmt.nine_points_hz {
            return Some([points[0], points[8]]);
        }
        Some(fmt.display_range.unwrap_or(def.host_range).map(f64::from))
    }

    pub fn bits_from_display(&self, shown: f64) -> Option<u32> {
        let def = self.definition?;
        let fmt = self.format?;
        let [min, max] = self.display_range()?;
        if !shown.is_finite() || !(min..=max).contains(&shown) {
            return None;
        }
        let unit = ((shown - min) / (max - min)) as f32;
        let result = if def.format == "PERCENT" {
            if def.host_type == HostType::Int {
                return Some((unit * def.host_range[1]).round() as u32);
            }
            unit
        } else if let Some(points) = fmt.nine_points_hz {
            if shown == points[8] {
                1.0
            } else {
                let i = points.windows(2).position(|range| shown <= range[1])?;
                ((i as f64 + (shown - points[i]) / (points[i + 1] - points[i])) / 8.0) as f32
            }
        } else if let Some([a, b]) = fmt.normalized_mapping_endpoints {
            let raw = a + unit * (b - a);
            (raw * 63.75).trunc() * f32::from_bits(0x3c80_8081)
        } else if let Some([a, b]) = fmt.display_range
            && def.host_range == [0.0, 1.0]
        {
            ((shown - f64::from(a)) / f64::from(b - a)) as f32
        } else {
            shown as f32
        };
        Some(result.to_bits())
    }

    pub fn neutral_bits(&self) -> Option<u32> {
        if let Some((bits, _)) = self.choices.first() {
            return Some(*bits);
        }
        let def = self.definition?;
        let [low, high] = self.display_range()?;
        let neutral = match def.format.as_str() {
            "TEMPO" => 120.0,
            "GATE_THRESH" => low,
            _ => 0.0_f64.clamp(low, high),
        };
        self.bits_from_display(neutral)
    }
}

impl Definition {
    fn accepts(&self, bits: u32, choices: &[(u32, &str)]) -> bool {
        if !choices.is_empty() {
            return choices.iter().any(|&(raw, _)| raw == bits);
        }
        let [min, max] = self.host_range;
        let value = if self.host_type == HostType::Float32 {
            f32::from_bits(bits)
        } else {
            bits as f32
        };
        (min..=max).contains(&value)
    }
}

/// Verified future providers can supply selected-model and companion host words.
pub trait PodModelDefaults {
    fn parameter_bits(&self, model_selector: u32, model_id: u32, parameter: u32) -> Option<u32>;
}

pub struct UnknownPodDefaults;

impl PodModelDefaults for UnknownPodDefaults {
    fn parameter_bits(&self, _: u32, _: u32, _: u32) -> Option<u32> {
        None
    }
}

#[derive(Default)]
pub struct PodModelChange {
    pub default_unknown: Vec<u32>,
    pub affected: Vec<u32>,
}

pub(super) fn iff_chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + data.len() + (data.len() & 1));
    bytes.extend_from_slice(tag);
    bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
    bytes.extend_from_slice(data);
    if data.len() & 1 != 0 {
        bytes.push(0);
    }
    bytes
}

pub(super) fn splice_iff(
    bytes: &mut Vec<u8>,
    range: Range<usize>,
    replacement: Vec<u8>,
    enclosing_sizes: &[usize],
) -> Result<(), ToneError> {
    let increase = replacement.len() as i64 - range.len() as i64;
    let length = bytes.len() as i64 + increase;
    if !(0..=TONE_BYTES_CEILING as i64).contains(&length) {
        return Err(ToneError::format(
            "tone would exceed the offline size limit",
        ));
    }
    for &offset in enclosing_sizes {
        let new_len = (i64::from(preset::word(bytes, offset)) + increase) as u32;
        bytes[offset..offset + 4].copy_from_slice(&new_len.to_be_bytes());
    }
    bytes.splice(range, replacement);
    Ok(())
}

impl PodTone {
    fn table_model(&self, slots: &[u32]) -> Result<Option<usize>, ToneError> {
        let mut found = self
            .models
            .iter()
            .enumerate()
            .filter(|(_, m)| slots.contains(&m.slot));
        let result = found.next().map(|(index, _)| index);
        if found.next().is_some() {
            return Err(ToneError::format("Both routing positions contain a model"));
        }
        Ok(result)
    }

    fn asset_for(&self, block: &str) -> Option<u32> {
        let table = pod_parameter_table();
        let row = table
            .parameters
            .iter()
            .find(|r| r.catalog.as_deref() == Some(block))?;
        let index = self.table_model(&row.slots).ok()??;
        Some(self.models[index].model_id)
    }

    pub fn table_controls(&self, packs: u8) -> Vec<PodControl<'static>> {
        pod_parameter_table()
            .parameters
            .iter()
            .filter_map(|row| self.table_control(row, packs))
            .collect()
    }

    fn table_control(&self, row: &'static PodParameter, packs: u8) -> Option<PodControl<'static>> {
        let table = pod_parameter_table();
        if row.file_field.is_none()
            || !matches!(
                row.availability.as_deref(),
                Some("Live" | "bass-model only")
            )
            || !row.applicable_amp_ids.is_empty()
                && !self
                    .asset_for("amp")
                    .is_some_and(|id| row.applicable_amp_ids.contains(&id))
        {
            return None;
        }
        let model = self
            .table_model(&row.slots)
            .ok()?
            .map(|i| self.models[i].model_id);
        let selected = match row.definition.as_deref()? {
            "asset selector" => None,
            "model-specific" => {
                let layout = table.layout(&row.block, model?)?;
                Some(layout.knobs.get(&row.key)?.as_str())
            }
            key if row.bass_cab_ids.contains(&model.unwrap_or(0)) => {
                row.bass_cab_definition.as_deref().or(Some(key))
            }
            key => Some(key),
        };
        let definition = selected.and_then(|key| table.definitions.get(key));
        let format = definition
            .and_then(|definition| table.formats.get(&definition.format))
            .map(|format| {
                format
                    .alias
                    .as_deref()
                    .map_or(format, |alias| &table.formats[alias])
            });
        let bits = self.read_table_bits(row).ok()?;
        let choices = if let Some(block) = &row.catalog {
            table
                .catalog
                .iter()
                .filter(|choice| {
                    choice.block == *block && (choice.pack == 0 || packs & choice.pack != 0)
                })
                .map(|choice| {
                    (
                        choice.id,
                        table.model_name(choice.id).expect("catalog label"),
                    )
                })
                .collect()
        } else if row.key == "TWEAK_PARAM" {
            self.tweak_choices()
        } else {
            format
                .into_iter()
                .flat_map(|format| &format.choices)
                .map(|choice| match choice {
                    Choice::Integer((raw, label)) | Choice::Association((_, label, raw)) => {
                        (*raw, label.as_str())
                    }
                })
                .collect()
        };
        Some(PodControl {
            parameter: row,
            definition,
            format,
            bits,
            choices,
        })
    }

    fn tweak_choices(&self) -> Vec<(u32, &'static str)> {
        let table = pod_parameter_table();
        table
            .tweak_targets
            .ordinals
            .iter()
            .enumerate()
            .filter_map(|(ordinal, target)| {
                let selector = match target {
                    TweakTarget::Fixed([selector]) | TweakTarget::Selector(selector) => *selector,
                    TweakTarget::Layout { block, knob } => {
                        let id = self.asset_for(block)?;
                        let layout = table.layout(block, id)?;
                        u32::try_from(*layout.editor_tweak_order.get(*knob)?).ok()?
                    }
                };
                let row = table
                    .parameters
                    .iter()
                    .find(|row| row.selector == Some(selector))?;
                Some((
                    ordinal as u32,
                    self.table_control(row, 0)?.definition?.label.as_str(),
                ))
            })
            .collect()
    }

    fn read_table_bits(&self, row: &PodParameter) -> Result<Option<u32>, ToneError> {
        let Some(index) = self.table_model(&row.slots)? else {
            return Ok(None);
        };
        let model = &self.models[index];
        Ok(match row.file_field {
            Some(FileField::Model) => Some(model.model_id),
            Some(FileField::Slot) => Some(u32::from(model.slot == row.slots[1])),
            Some(FileField::Enable) => {
                let range = &self.model_minfo_ranges[index];
                let flag = *self
                    .bytes
                    .get(range.start + 11)
                    .filter(|&&value| range.len() >= 12 && value <= 1)
                    .ok_or_else(|| ToneError::format("Model flag is missing or opaque"))?;
                Some(u32::from(flag))
            }
            Some(FileField::Parameter) => self
                .find_table_parm(index, row.parm.expect("PARM id"))?
                .map(|p| match model.parameters[p] {
                    ModelParameter::Recorded {
                        value: ParameterValue::Float { bits } | ParameterValue::Int { bits },
                        ..
                    } => bits,
                    _ => unreachable!("PARM type checked"),
                }),
            None => None,
        })
    }

    fn find_table_parm(&self, index: usize, id: u32) -> Result<Option<usize>, ToneError> {
        let mut found = None;
        for (position, (param, hint)) in self.models[index]
            .parameters
            .iter()
            .zip(&self.opaque_parameter_id_hints[index])
            .enumerate()
        {
            if hint.is_some_and(|candidate| candidate != id) {
                continue;
            }
            match param {
                ModelParameter::Recorded {
                    prefix: 0x3f,
                    value: ParameterValue::Int { .. } | ParameterValue::Float { .. },
                    ..
                } if found.is_none() => found = Some(position),
                _ => return Err(ToneError::format("Parameter record is ambiguous or opaque")),
            }
        }
        Ok(found)
    }

    fn editable_table_tone(&self) -> Result<(), ToneError> {
        if self.device_version < PODXT_LIVE_HOST_MIN_VERSION {
            return Err(ToneError::format(
                "This tone predates the Line 6 Edit 3.00 host profile",
            ));
        }
        Ok(())
    }

    fn patch_chunk(&self) -> Result<preset::Chunk, ToneError> {
        preset::chunks(&self.bytes, 12..self.bytes.len())
            .map_err(ToneError::format)?
            .into_iter()
            .find(|chunk| preset::container(&self.bytes, chunk, b"PATC"))
            .ok_or_else(|| ToneError::format("Missing PATC container"))
    }

    fn create_table_model(&mut self, row: &PodParameter) -> Result<usize, ToneError> {
        let slot = row.slots[0];
        let (id, enabled) = match slot {
            0x0001_0000 => (0x3e70_0000, true),  // GetSystemModel
            0x0002_0000 => (0x020b_0000, false), // GetGateModel
            0x0002_0001 => (0x0207_0000, true),  // GetVolPedalModel
            0x0002_0002 => (0x0206_0000, false), // GetWahPedalModel
            0x0003_0003 => (0x020b_0002, false), // GetAIRModel
            0x0003_0004 => (0x020c_0002, false), // GetEQModel
            0x0005_0000 => (0x020b_0001, false), // GetCompModel
            _ => {
                let catalog = row.catalog.as_deref().unwrap_or(&row.block);
                let Some(first) = pod_parameter_table()
                    .catalog
                    .iter()
                    .find(|m| m.block == catalog)
                else {
                    return Err(ToneError::format("No model is selected for this block"));
                };
                (first.id, matches!(catalog, "amp" | "cab"))
            }
        };
        let mut minf = Vec::from(id.to_be_bytes());
        minf.extend_from_slice(&slot.to_be_bytes());
        minf.extend_from_slice(&[0, 0, 0, u8::from(enabled)]);
        let mut data = b"MODL".to_vec();
        data.extend(iff_chunk(b"MINF", &minf));
        let patch = self.patch_chunk()?;
        let mut next = self.bytes.clone();
        splice_iff(
            &mut next,
            patch.data.end..patch.data.end,
            iff_chunk(b"LIST", &data),
            &[4, patch.data.start - 4],
        )?;
        *self = Self::parse(next)?;
        self.table_model(&row.slots)?
            .ok_or_else(|| ToneError::format("Model creation failed"))
    }

    fn write_table_bits(
        &mut self,
        row: &PodParameter,
        definition: Option<&Definition>,
        bits: u32,
    ) -> Result<(), ToneError> {
        let index = match self.table_model(&row.slots)? {
            Some(index) => index,
            None => self.create_table_model(row)?,
        };
        let minf = self.model_minfo_ranges[index].clone();
        let model_span = self.model_chunk_ranges[index].clone();
        let mut next = self.bytes.clone();
        match row.file_field {
            Some(FileField::Model) => {
                next[minf.start..minf.start + 4].copy_from_slice(&bits.to_be_bytes())
            }
            Some(FileField::Slot) => {
                next[minf.start + 4..minf.start + 8]
                    .copy_from_slice(&row.slots[bits as usize].to_be_bytes());
            }
            Some(FileField::Enable) if minf.len() >= 12 && next[minf.start + 11] <= 1 => {
                next[minf.start + 11] = bits as u8;
            }
            Some(FileField::Parameter) => {
                let id = row.parm.expect("PARM id");
                let is_float = definition.expect("PARM definition").host_type == HostType::Float32;
                if let Some(position) = self.find_table_parm(index, id)? {
                    let ModelParameter::Recorded { value, .. } =
                        self.models[index].parameters[position]
                    else {
                        unreachable!()
                    };
                    if is_float != matches!(value, ParameterValue::Float { .. }) {
                        return Err(ToneError::format(
                            "Parameter record has an unexpected value type",
                        ));
                    }
                    let offset =
                        self.parameter_value_offsets[index][position].expect("recorded PARM");
                    next[offset..offset + 4].copy_from_slice(&bits.to_be_bytes());
                } else {
                    let mut data = Vec::from((0x3f00_0000 | id).to_be_bytes());
                    data.extend_from_slice(&u32::from(is_float).to_be_bytes());
                    data.extend_from_slice(&bits.to_be_bytes());
                    let patch = self.patch_chunk()?;
                    let end =
                        model_span.start + 8 + preset::word(&next, model_span.start + 4) as usize;
                    splice_iff(
                        &mut next,
                        end..end,
                        iff_chunk(b"PARM", &data),
                        &[4, patch.data.start - 4, model_span.start + 4],
                    )?;
                }
            }
            _ => return Err(ToneError::format("Field is unavailable for editing")),
        }
        *self = Self::parse(next)?;
        Ok(())
    }

    /// Edit a single control and its NOTE/tempo companions as one transaction.
    pub fn set_table_parameter(&mut self, key: &str, bits: u32) -> Result<(), ToneError> {
        self.editable_table_tone()?;
        let row = pod_parameter_table()
            .parameter(key)
            .ok_or_else(|| ToneError::format("Unrecognized PODxt parameter"))?;
        let control = self
            .table_control(row, 0)
            .ok_or_else(|| ToneError::format("Parameter is unavailable in this model or file"))?;
        let definition = control
            .definition
            .ok_or_else(|| ToneError::format("Use model selection to edit an asset ID"))?;
        if !definition.accepts(bits, &control.choices)
            || key == "TWEAK_PARAM" && control.choices.is_empty()
        {
            return Err(ToneError::format("Value is outside the editor domain"));
        }
        let mut candidate = self.clone();
        candidate.write_table_bits(row, Some(definition), bits)?;
        match row.key.as_str() {
            "MOD_NOTE" | "DELAY_NOTE" | "TEMPO" => candidate.update_table_sync()?,
            "MOD_SPEED" | "DELAY_TIME" => {
                let note = if row.key == "MOD_SPEED" {
                    "MOD_NOTE"
                } else {
                    "DELAY_NOTE"
                };
                let note_row = pod_parameter_table().parameter(note).unwrap();
                if candidate
                    .read_table_bits(note_row)?
                    .is_some_and(|value| value > 0)
                {
                    let note_control = candidate
                        .table_control(note_row, 0)
                        .ok_or_else(|| ToneError::format("Cannot clear the stored NOTE"))?;
                    candidate.write_table_bits(note_row, note_control.definition, 0)?;
                }
            }
            _ => {}
        }
        *self = candidate;
        Ok(())
    }

    fn update_table_sync(&mut self) -> Result<(), ToneError> {
        let table = pod_parameter_table();
        let tempo = self
            .read_table_bits(table.parameter("TEMPO").unwrap())?
            .map_or(120.0, f32::from_bits);
        if !tempo.is_finite() || !(30.0..=240.0).contains(&tempo) {
            return Err(ToneError::format("Stored tempo is outside 30–240 BPM"));
        }
        for (note, scalar) in [("MOD_NOTE", "MOD_SPEED"), ("DELAY_NOTE", "DELAY_TIME")] {
            let raw = self
                .read_table_bits(table.parameter(note).unwrap())?
                .unwrap_or(0);
            if raw == 0 {
                continue;
            }
            let multiplier = table
                .note_sync
                .ordinal_1_to_13
                .get(raw as usize - 1)
                .ok_or_else(|| ToneError::format("Stored NOTE is outside the supported choices"))?;
            let q = multiplier * 4.0;
            let rate = if scalar == "MOD_SPEED" {
                (tempo / (q * 60.0)).clamp(0.1, 15.0)
            } else {
                (q / tempo * 1000.0 * 60.0).clamp(20.0, 2000.0)
            };
            let row = table.parameter(scalar).unwrap();
            let control = self
                .table_control(row, 0)
                .ok_or_else(|| ToneError::format("Synchronized rate is unavailable"))?;
            let bits = control
                .bits_from_display(f64::from(rate))
                .ok_or_else(|| ToneError::format("Synchronized rate is out of range"))?;
            self.write_table_bits(row, control.definition, bits)?;
        }
        Ok(())
    }

    /// Change a named model. Values shared by meaning follow the new layout,
    /// even when the old and new models place them at different knob positions.
    pub fn set_table_model(
        &mut self,
        key: &str,
        id: u32,
        packs: u8,
        amp_defaults: bool,
        source: &impl PodModelDefaults,
    ) -> Result<PodModelChange, ToneError> {
        self.editable_table_tone()?;
        let table = pod_parameter_table();
        let row = table
            .parameter(key)
            .filter(|r| r.catalog.is_some())
            .ok_or_else(|| ToneError::format("Unknown model selector"))?;
        let control = self
            .table_control(row, packs)
            .ok_or_else(|| ToneError::format("Model selector is unavailable"))?;
        if !control.choices.iter().any(|&(choice, _)| id == choice) {
            return Err(ToneError::format(
                "Model is outside the catalog or requires an installed pack",
            ));
        }
        let mut result = PodModelChange::default();
        if control.bits == Some(id) {
            return Ok(result);
        }
        let selector = row.selector.unwrap();
        let old_layout = control.bits.and_then(|old| table.layout(&row.block, old));
        let new_layout = table.layout(&row.block, id);
        let mut candidate = self.clone();
        candidate.write_table_bits(row, None, id)?;
        let use_defaults = key != "AMP_MODEL" || amp_defaults;
        if use_defaults {
            // Preflight fields through the original tone before changing offsets.
            let mut edits = Vec::new();
            for param in &table.parameters {
                let Some(parm_selector) = param.selector else {
                    continue;
                };
                if !matches!(
                    param.file_field,
                    Some(FileField::Parameter | FileField::Enable | FileField::Slot)
                ) {
                    continue;
                }
                let supplied = source.parameter_bits(selector, id, parm_selector);
                let new_knob = new_layout.and_then(|layout| layout.knobs.get(&param.key));
                let old_knob = old_layout.and_then(|layout| layout.knobs.get(&param.key));
                let same_slot = param.slots == row.slots;
                if supplied.is_none() && !same_slot {
                    continue;
                }
                if new_knob.is_none() && old_knob.is_some() {
                    // Known inactive knob positions can be cleared. Keep every
                    // opaque or unrecognized record elsewhere byte for byte.
                    let before = self.table_control(param, packs).ok_or_else(|| {
                        ToneError::format("Cannot reset the previous model layout")
                    })?;
                    if before.bits.is_some() {
                        edits.push((param, before.definition, before.neutral_bits().unwrap()));
                    }
                    result.affected.push(parm_selector);
                    continue;
                }
                let Some(after) = candidate.table_control(param, packs) else {
                    if supplied.is_some() || new_knob.is_some() {
                        return Err(ToneError::format("Target model parameter is unavailable"));
                    }
                    continue;
                };
                let definition = after.definition.expect("non-model control definition");
                if let Some(value) = supplied {
                    if !definition.accepts(value, &after.choices) {
                        return Err(ToneError::format(
                            "Default provider supplied a value outside the model domain",
                        ));
                    }
                    edits.push((param, Some(definition), value));
                    result.affected.push(parm_selector);
                    continue;
                }
                if new_knob.is_none() {
                    if param.file_field == Some(FileField::Parameter) && after.bits.is_none() {
                        edits.push((param, Some(definition), after.neutral_bits().unwrap()));
                        result.affected.push(parm_selector);
                        result.default_unknown.push(parm_selector);
                    }
                    continue;
                }
                let shared = old_layout.and_then(|layout| {
                    let mut entries = layout
                        .knobs
                        .iter()
                        .filter(|(_, def)| Some(*def) == new_knob);
                    let (key, _) = entries.next()?;
                    entries.next().is_none().then_some(key)
                });
                let old_control = shared
                    .and_then(|name| table.parameter(name))
                    .and_then(|other| self.table_control(other, packs));
                if shared.is_some() && old_control.is_none() {
                    return Err(ToneError::format("Shared parameter is opaque or ambiguous"));
                }
                result.affected.push(parm_selector);
                let value = old_control.and_then(|control| control.bits);
                if value.is_none() {
                    result.default_unknown.push(parm_selector);
                }
                edits.push((
                    param,
                    Some(definition),
                    value.unwrap_or_else(|| after.neutral_bits().unwrap()),
                ));
            }
            for (param, definition, value) in edits {
                candidate.write_table_bits(param, definition, value)?;
            }
        }
        *self = candidate;
        Ok(result)
    }

    /// Project one editor control onto its reference 160-byte tone fields.
    /// This describes encoding only and never opens a MIDI transport.
    pub fn table_tone_field(&self, key: &str) -> Option<Vec<(usize, u8)>> {
        let table = pod_parameter_table();
        let row = table.parameter(key)?;
        let bits = self.table_control(row, 15)?.bits?;
        let value = f32::from_bits(bits);
        let split = |raw: u16| vec![(raw >> 7) as u8, (raw & 127) as u8];
        let bytes = match row.tone_encoding.as_deref()? {
            "model-index" => vec![
                table
                    .catalog
                    .iter()
                    .find(|m| Some(m.block.as_str()) == row.catalog.as_deref() && m.id == bits)?
                    .index,
            ],
            "switch" => vec![if bits != 0 { 127 } else { 0 }],
            "inverted-switch" => vec![if bits != 0 { 0 } else { 127 }],
            "integer" => vec![bits.min(127) as u8],
            "float7-nearest" => vec![((value * 127.0 + 0.5) as i32).clamp(0, 127) as u8],
            "float14-truncate" => split(((value * 16383.0) as u16).min(16383)),
            "gate-db-ranged" => {
                vec![((((value + 96.0) / 96.0) * -96.0 + 96.0) as i32).clamp(0, 127) as u8]
            }
            "tempo-tenths" => split(((value * 10.0) as u16).min(16383)),
            "pedal-assignment" => vec![*[0u8, 64, 127].get(bits as usize)?],
            "constant-127" => vec![127],
            _ => return None,
        };
        Some(row.tone_offsets.iter().copied().zip(bytes).collect())
    }
}
