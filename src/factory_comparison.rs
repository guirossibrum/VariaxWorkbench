#![forbid(unsafe_code)]
//! Compare saved model/settings files with one fingerprinted firmware reference.
//! Equality here never establishes a full-bank/DSP/calibration/audio match,
//! shipping state, or the current state of a connected guitar.

use crate::{factory_reference::FactoryReference, preset::PresetFile};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsComparison {
    pub reference_slot: usize,
    pub reference_source_sha256: [u8; 32],
    pub subject_file_sha256: [u8; 32],
    pub name_matches: bool,
    pub models_match: bool,
    pub file_bytes_identical: bool,
    pub reference_model_count: usize,
    pub subject_model_count: usize,
    pub reference_parameter_count: usize,
    pub subject_parameter_count: usize,
    /// Model slot and ordinal; duplicate models/parameters are retained as multisets.
    pub changed_model_keys: Vec<(u32, u8)>,
}
impl SettingsComparison {
    pub fn model_settings_match(&self) -> bool {
        self.name_matches && self.models_match
    }
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "comparison_scope: saved-model-settings-only\nreference: archived firmware 3.10, slot {}\nreference_source_sha256: {}\nsubject_file_sha256: {}\nmodel_settings_match: {}\nname_matches: {}\nmodels_match: {}\nfile_bytes_identical: {}\nreference_models: {}\nsubject_models: {}\nreference_parameters: {}\nsubject_parameters: {}\n",
            self.reference_slot,
            hex(&self.reference_source_sha256),
            hex(&self.subject_file_sha256),
            self.model_settings_match(),
            self.name_matches,
            self.models_match,
            self.file_bytes_identical,
            self.reference_model_count,
            self.subject_model_count,
            self.reference_parameter_count,
            self.subject_parameter_count
        );
        for &(slot, ordinal) in &self.changed_model_keys {
            text.push_str(&format!("changed_model: {slot:08x}/{ordinal}\n"));
        }
        text.push_str("Limits: this compares supplied files, not a connected unit, full stored bank, DSP, calibration, audible tuning, shipping state or restoration safety.\n");
        text
    }
}

pub fn compare_slot(
    reference: &FactoryReference,
    slot: usize,
    subject: &PresetFile,
) -> Result<SettingsComparison, String> {
    let factory = reference
        .slots()
        .get(slot)
        .ok_or("factory slot must be in 0..60")?
        .settings_file()?;
    let core = compare_presets(&factory, subject)?;
    Ok(SettingsComparison {
        reference_slot: slot,
        reference_source_sha256: *reference.source_sha256(),
        subject_file_sha256: Sha256::digest(subject.as_bytes()).into(),
        name_matches: factory.name() == subject.name(),
        models_match: core.is_empty(),
        file_bytes_identical: factory.as_bytes() == subject.as_bytes(),
        reference_model_count: factory.models().len(),
        subject_model_count: subject.models().len(),
        reference_parameter_count: parameter_count(&factory),
        subject_parameter_count: parameter_count(subject),
        changed_model_keys: core,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ModelSnapshot {
    id: u32,
    enabled: bool,
    parameters: Vec<(u32, u32, u32)>,
}
fn snapshots(models: &[crate::preset::Model]) -> BTreeMap<(u32, u8), Vec<ModelSnapshot>> {
    let mut out: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for model in models {
        let mut parameters: Vec<_> = model
            .parameters
            .iter()
            .map(|p| (p.id, p.value_type, p.bits))
            .collect();
        parameters.sort();
        out.entry((model.slot, model.ordinal))
            .or_default()
            .push(ModelSnapshot {
                id: model.id,
                enabled: model.enabled,
                parameters,
            });
    }
    for models in out.values_mut() {
        models.sort();
    }
    out
}
fn compare_presets(reference: &PresetFile, subject: &PresetFile) -> Result<Vec<(u32, u8)>, String> {
    if reference.device_id() != subject.device_id() {
        return Err("different target model; no cross-family comparison or conversion".into());
    }
    if reference.device_version() != subject.device_version() {
        return Err("different firmware version; this reference cannot establish version-equivalent defaults".into());
    }
    let a = snapshots(reference.models());
    let b = snapshots(subject.models());
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).copied().collect();
    Ok(keys
        .into_iter()
        .filter(|key| a.get(key) != b.get(key))
        .collect())
}
fn parameter_count(preset: &PresetFile) -> usize {
    preset.models().iter().map(|m| m.parameters.len()).sum()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParameterValue, PresetMetadata};
    fn make(modify: impl FnOnce(&mut PresetMetadata)) -> PresetFile {
        let mut raw = [0; 192];
        raw[..18].fill(b' ');
        raw[..9].copy_from_slice(b"Synthetic");
        let mut metadata = PresetMetadata::from_bytes(raw).unwrap();
        modify(&mut metadata);
        PresetFile::from_metadata(&metadata, 0x70004, 0x03100000).unwrap()
    }
    #[test]
    fn raw_bits_and_tuning_changes_are_not_hidden_by_numeric_equality() {
        let reference = make(|_| {});
        let negative_zero = make(|m| {
            m.parameters
                .insert("pickup1_level", ParameterValue::Float(-0.0));
        });
        assert_eq!(
            compare_presets(&reference, &negative_zero).unwrap(),
            [(0x2010001, 0)]
        );
        let changed = make(|m| {
            m.parameters
                .insert("string1_tuning", ParameterValue::Integer(-2));
        });
        assert_eq!(
            compare_presets(&reference, &changed).unwrap(),
            [(0x2010005, 0)]
        );
        let changed = make(|m| {
            m.parameters
                .insert("alternate_tuning_enabled", ParameterValue::Boolean(true));
        });
        assert!(!compare_presets(&reference, &changed).unwrap().is_empty());
    }
    #[test]
    fn target_and_version_mismatches_are_explicit_rejections() {
        let a = make(|_| {});
        let meta = PresetMetadata::from_bytes([0; 192]).unwrap();
        for (id, version) in [(0x70000, 0x03100000), (0x70004, 0x03000000)] {
            let b = PresetFile::from_metadata(&meta, id, version).unwrap();
            assert!(compare_presets(&a, &b).is_err());
        }
        let mut renamed = a.clone();
        renamed.rename("Other name").unwrap();
        assert!(compare_presets(&a, &renamed).unwrap().is_empty());
        assert_ne!(a.name(), renamed.name());
    }
    #[test]
    fn snapshots_include_unknown_parameters_and_duplicate_multiplicity() {
        let preset = make(|_| {});
        // Model fields are lossless parser output. Exercise the snapshot logic
        // with an unknown parameter and a repeated model via the helper below.
        let mut models = preset.models().to_vec();
        let mut unknown = models[0].parameters[0].clone();
        unknown.id = 0xfefefe;
        unknown.bits = 0x7fc12345;
        models[0].parameters.push(unknown.clone());
        let base = snapshots(&models);
        models[0].parameters.push(unknown);
        assert_ne!(base, snapshots(&models));
        let base = snapshots(&models);
        models.push(models[0].clone());
        assert_ne!(base, snapshots(&models));
        models.reverse();
        let before = snapshots(&models);
        for model in &mut models {
            model.parameters.reverse();
        }
        assert_eq!(before, snapshots(&models));
    }
}
