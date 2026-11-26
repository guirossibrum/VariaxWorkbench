//! Step 0.3, the engine check (#252): build reference programs on this
//! computer and compare all 1,092 words with the expected result.
//!
//! Built-in references use only the repository's synthetic fixtures (see
//! `tests/fixtures/README.md`): eight synthetic body resources, four synthetic
//! pickup resources, 48 synthetic native inputs with their original native
//! outputs, and one synthetic preset with its original Java/native result. No
//! vendor model data, factory preset or owner file is built in.
//!
//! A person may add a local, private folder of reference models: a
//! `models.tsv` model manifest (their own copy of the model data, as used by
//! `variax-workbench render-preset`) and `NAME.vwm` files, each with either
//! `NAME.packed` (the expected 3,276-byte program) or `NAME.sha256` (only its
//! hash). Nothing from that folder is copied into the report except names,
//! hashes and word differences.

use crate::body::BodyDefinition;
use crate::host::{HostEngine, HostOptions, HostParameters, ModelLibrary, WorkbenchSettings};
use crate::models::PickupDefinition;
use crate::preset::PresetFile;
use std::path::Path;
use std::sync::OnceLock;

pub const WORDS: usize = 1092;
pub const PACKED: usize = WORDS * 3;

const BODY_IDS: [u32; 8] = [
    0x3100003, 0x310000d, 0x3100012, 0x3100013, 0x3100015, 0x3100018, 0x310001b, 0x3100011,
];
const PICKUP_IDS: [u32; 4] = [0x3110001, 0x3110000, (-2i32) as u32, (-3i32) as u32];
const BODIES: &[u8] = include_bytes!("../../tests/fixtures/native-dsp-body-inputs.bin");
const PICKUPS: &[u8] = include_bytes!("../../tests/fixtures/native-host-pickups.bin");
const INPUTS: &[u8] = include_bytes!("../../tests/fixtures/native-host-inputs.bin");
const OUTPUTS: &[u8] = include_bytes!("../../tests/fixtures/native-host-outputs.bin");
const PRESET: &[u8] = include_bytes!("../../tests/fixtures/synthetic-host-preset.vwm");
const PRESET_PACKED: &[u8] = include_bytes!("../../tests/fixtures/synthetic-host-preset.packed");

/// The sound part a program word belongs to, from the observed native word
/// map (`docs/dsp-word-map.md`) and the Workbench program layout.
pub fn sound_part(word: usize) -> &'static str {
    match word {
        0..=1 => "block switches",
        2..=155 => "pickups",
        156..=203 => "body",
        204..=347 => "body (string tone and decay)",
        348..=545 => "EQ",
        546..=554 => "volume and tone controls",
        555..=611 => "sitar and drone",
        612..=679 => "reverb and other",
        680..=691 => "pitch/tuning",
        692..=763 => "12-string EQ",
        764..=850 => "pickup coil gain and unknown",
        851..=914 => "settings",
        915..=1090 => "unknown (not written by the host)",
        1091 => "checksum",
        _ => "outside the program",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WordDiff {
    pub word: usize,
    pub expected: u32,
    pub actual: u32,
    pub xor: u32,
    /// `actual - expected`, both read as signed 24-bit numbers.
    pub difference: i64,
    pub part: &'static str,
}

fn signed24(word: u32) -> i64 {
    let word = i64::from(word & 0xff_ffff);
    if word & 0x80_0000 != 0 {
        word - 0x100_0000
    } else {
        word
    }
}

fn word(bytes: &[u8], index: usize) -> u32 {
    u32::from_be_bytes([
        0,
        bytes[index * 3],
        bytes[index * 3 + 1],
        bytes[index * 3 + 2],
    ])
}

/// Every differing word between two 3,276-byte programs.
pub fn compare(expected: &[u8], actual: &[u8]) -> Vec<WordDiff> {
    (0..WORDS)
        .filter_map(|i| {
            let (e, a) = (word(expected, i), word(actual, i));
            (e != a).then(|| WordDiff {
                word: i,
                expected: e,
                actual: a,
                xor: e ^ a,
                difference: signed24(a) - signed24(e),
                part: sound_part(i),
            })
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ModelResult {
    pub name: String,
    /// The program built here, for local-folder models only (test models
    /// for step 4.3 must come from a model that matched its reference).
    pub program: Option<Vec<u8>>,
    /// `NAME.listen.txt`: the A/B question for this model in step 4.3.
    pub question: Option<String>,
    pub diffs: Vec<WordDiff>,
    pub actual_sha256: Option<String>,
    /// Only the expected hash was available (no word detail).
    pub hash_only: Option<bool>,
    pub error: Option<String>,
}

impl ModelResult {
    pub fn identical(&self) -> bool {
        self.error.is_none() && self.diffs.is_empty() && self.hash_only != Some(false)
    }

    pub fn summary(&self) -> String {
        if let Some(error) = &self.error {
            return format!("{}: not built: {error}", self.name);
        }
        match self.hash_only {
            Some(true) => return format!("{}: matches the expected SHA-256", self.name),
            Some(false) => {
                return format!(
                    "{}: DIFFERS from the expected SHA-256 (no word detail without the expected program)",
                    self.name
                );
            }
            None => {}
        }
        if self.diffs.is_empty() {
            return format!("{}: all 1,092 words identical", self.name);
        }
        let mut parts: Vec<(&str, usize)> = Vec::new();
        for diff in &self.diffs {
            match parts.iter_mut().find(|(p, _)| *p == diff.part) {
                Some((_, n)) => *n += 1,
                None => parts.push((diff.part, 1)),
            }
        }
        let parts: Vec<String> = parts.iter().map(|(p, n)| format!("{p} {n}")).collect();
        format!(
            "{}: {} of 1,092 words differ ({})",
            self.name,
            self.diffs.len(),
            parts.join(", ")
        )
    }

    pub fn details(&self) -> Vec<String> {
        self.diffs
            .iter()
            .map(|d| {
                format!(
                    "  word {:4} {:<34} expected 0x{:06X} actual 0x{:06X} xor 0x{:06X} ({} bit{}) difference {:+}",
                    d.word,
                    d.part,
                    d.expected,
                    d.actual,
                    d.xor,
                    d.xor.count_ones(),
                    if d.xor.count_ones() == 1 { "" } else { "s" },
                    d.difference
                )
            })
            .collect()
    }
}

fn synthetic_library() -> Result<ModelLibrary, String> {
    let mut library = ModelLibrary::default();
    for (id, bytes) in BODY_IDS.iter().zip(BODIES.chunks(17483)) {
        library.bodies.insert(*id, BodyDefinition::parse(bytes)?);
    }
    for (id, bytes) in PICKUP_IDS.iter().zip(PICKUPS.chunks(85)) {
        library.pickups.insert(*id, PickupDefinition::parse(bytes)?);
    }
    Ok(library)
}

/// Render a preset file through the complete host pipeline.
pub fn render_preset(library: &ModelLibrary, preset: &PresetFile) -> Result<Vec<u8>, String> {
    let parameters = HostParameters::from_preset(preset)?;
    let settings = parameters.map(HostOptions::default())?;
    let generated = HostEngine::new(library).generate(&settings)?;
    Ok(parameters
        .finish(preset.name(), &settings, generated.packed())
        .to_vec())
}

fn result(name: String, expected: &[u8], actual: Result<Vec<u8>, String>) -> ModelResult {
    match actual {
        Ok(actual) if actual.len() == PACKED && expected.len() == PACKED => ModelResult {
            diffs: compare(expected, &actual),
            actual_sha256: Some(super::record::sha256_hex(&actual)),
            name,
            program: None,
            question: None,
            hash_only: None,
            error: None,
        },
        Ok(_) => ModelResult {
            name,
            program: None,
            question: None,
            diffs: Vec::new(),
            actual_sha256: None,
            hash_only: None,
            error: Some("unexpected program length".into()),
        },
        Err(error) => ModelResult {
            name,
            program: None,
            question: None,
            diffs: Vec::new(),
            actual_sha256: None,
            hash_only: None,
            error: Some(error),
        },
    }
}

/// The built-in synthetic references, rendered on this computer. Cached:
/// the result for one process does not change.
pub fn builtin() -> &'static [ModelResult] {
    static RESULTS: OnceLock<Vec<ModelResult>> = OnceLock::new();
    RESULTS.get_or_init(|| {
        let library = match synthetic_library() {
            Ok(library) => library,
            Err(error) => {
                return vec![ModelResult {
                    name: "Synthetic references".into(),
                    program: None,
                    question: None,
                    diffs: Vec::new(),
                    actual_sha256: None,
                    hash_only: None,
                    error: Some(error),
                }];
            }
        };
        let mut results = Vec::new();
        let preset = PresetFile::parse(PRESET.to_vec());
        results.push(result(
            "Synthetic preset (full pipeline)".into(),
            PRESET_PACKED,
            preset.and_then(|p| render_preset(&library, &p)),
        ));
        // The 48 native cases run in order: the engine keeps state between them.
        let mut engine = HostEngine::new(&library);
        for (i, (input, expected)) in INPUTS.chunks(192).zip(OUTPUTS.chunks(PACKED)).enumerate() {
            let settings = WorkbenchSettings {
                native: input.try_into().expect("192-byte records"),
                block_mask: u32::MAX,
            };
            let actual = engine
                .generate(&settings)
                .map(|patch| patch.packed().to_vec());
            results.push(result(
                format!("Synthetic engine case {:02}", i + 1),
                expected,
                actual,
            ));
        }
        results
    })
}

/// References from a private local folder (see the module documentation).
pub fn local_folder(dir: &Path) -> Result<Vec<ModelResult>, String> {
    let library = ModelLibrary::load_manifest(&dir.join("models.tsv"))?;
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".vwm").map(str::to_owned)
        })
        .collect();
    names.sort();
    let mut results = Vec::new();
    for name in names {
        let actual = std::fs::read(dir.join(format!("{name}.vwm")))
            .map_err(|e| e.to_string())
            .and_then(PresetFile::parse)
            .and_then(|preset| render_preset(&library, &preset));
        let question = std::fs::read_to_string(dir.join(format!("{name}.listen.txt")))
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty());
        let built = actual.as_ref().ok().cloned();
        let before = results.len();
        if let Ok(expected) = std::fs::read(dir.join(format!("{name}.packed"))) {
            results.push(result(name, &expected, actual));
        } else if let Ok(hash) = std::fs::read_to_string(dir.join(format!("{name}.sha256"))) {
            let expected = hash
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            results.push(match actual {
                Ok(actual) => {
                    let sha = super::record::sha256_hex(&actual);
                    ModelResult {
                        name,
                        program: None,
                        question: None,
                        diffs: Vec::new(),
                        hash_only: Some(sha == expected),
                        actual_sha256: Some(sha),
                        error: None,
                    }
                }
                Err(error) => ModelResult {
                    name,
                    program: None,
                    question: None,
                    diffs: Vec::new(),
                    actual_sha256: None,
                    hash_only: None,
                    error: Some(error),
                },
            });
        } else {
            results.push(ModelResult {
                name,
                program: None,
                question: None,
                diffs: Vec::new(),
                actual_sha256: None,
                hash_only: None,
                error: Some("no NAME.packed or NAME.sha256 next to the model".into()),
            });
        }
        if let Some(last) = results.get_mut(before) {
            last.program = built;
            last.question = question;
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_references_match_on_this_computer() {
        let results = builtin();
        assert_eq!(results.len(), 49);
        for result in results {
            assert!(result.identical(), "{}", result.summary());
        }
        assert_eq!(
            results[0].summary(),
            "Synthetic preset (full pipeline): all 1,092 words identical"
        );
    }

    #[test]
    fn a_one_bit_difference_names_its_sound_part() {
        let expected = PRESET_PACKED.to_vec();
        for (word, part) in [
            (40, "pickups"),
            (170, "body"),
            (400, "EQ"),
            (682, "pitch/tuning"),
            (860, "settings"),
            (1091, "checksum"),
        ] {
            let mut actual = expected.clone();
            actual[word * 3 + 2] ^= 0x01;
            let result = result("Injected".into(), &expected, Ok(actual));
            assert_eq!(result.diffs.len(), 1);
            let diff = &result.diffs[0];
            assert_eq!((diff.word, diff.part, diff.xor), (word, part, 1));
            assert_eq!(diff.difference.abs(), 1);
            assert!(
                result.summary().contains(&format!("({part} 1)")),
                "{}",
                result.summary()
            );
            assert!(result.details()[0].contains(&format!("word {word:4}")));
            assert!(result.details()[0].contains("(1 bit)"));
        }
    }

    #[test]
    fn signed_differences_and_high_bits() {
        let mut expected = vec![0u8; PACKED];
        let mut actual = expected.clone();
        expected[0..3].copy_from_slice(&[0x7f, 0xff, 0xff]);
        actual[0..3].copy_from_slice(&[0x80, 0x00, 0x00]);
        let diff = &compare(&expected, &actual)[0];
        assert_eq!(diff.xor, 0xff_ffff);
        assert_eq!(diff.difference, -0x80_0000 - 0x7f_ffff);
    }

    #[test]
    fn a_reference_difference_is_reported_and_does_not_block_a_save() {
        use crate::rig_check::{
            catalogue::StepId, practice::Faults, run_practice, runner::NoReport, steps::RunData,
            testing::Script,
        };
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = String::new();
        for (i, (id, bytes)) in BODY_IDS.iter().zip(BODIES.chunks(17483)).enumerate() {
            std::fs::write(dir.path().join(format!("b{i}.bin")), bytes).unwrap();
            manifest.push_str(&format!("body\t{id:08x}\tb{i}.bin\n"));
        }
        for (i, (id, bytes)) in PICKUP_IDS.iter().zip(PICKUPS.chunks(85)).enumerate() {
            std::fs::write(dir.path().join(format!("p{i}.bin")), bytes).unwrap();
            manifest.push_str(&format!("pickup\t{id:08x}\tp{i}.bin\n"));
        }
        std::fs::write(dir.path().join("models.tsv"), manifest).unwrap();
        std::fs::write(dir.path().join("A.vwm"), PRESET).unwrap();
        let mut reference = PRESET_PACKED.to_vec();
        reference[121] ^= 1;
        std::fs::write(dir.path().join("A.packed"), reference).unwrap();
        let (run, _, _) = run_practice(
            Faults::default(),
            &mut Script::run_all(),
            &mut NoReport,
            RunData {
                engine_folder: Some(dir.path().into()),
                ..RunData::default()
            },
        );
        assert!(run.step(StepId(0, 3)).outcome.passed());
        assert!(
            run.report_differences()
                .any(|line| line.contains("A: 1 of 1,092"))
        );
        assert!(run.step(StepId(5, 1)).outcome.passed());
    }

    #[test]
    fn a_local_folder_reports_hashes_and_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = String::new();
        for (i, (id, bytes)) in BODY_IDS.iter().zip(BODIES.chunks(17483)).enumerate() {
            std::fs::write(dir.path().join(format!("b{i}.bin")), bytes).unwrap();
            manifest.push_str(&format!("body\t{id:08x}\tb{i}.bin\n"));
        }
        for (i, (id, bytes)) in PICKUP_IDS.iter().zip(PICKUPS.chunks(85)).enumerate() {
            std::fs::write(dir.path().join(format!("p{i}.bin")), bytes).unwrap();
            manifest.push_str(&format!("pickup\t{id:08x}\tp{i}.bin\n"));
        }
        std::fs::write(dir.path().join("models.tsv"), manifest).unwrap();
        for name in ["A", "B", "C"] {
            std::fs::write(dir.path().join(format!("{name}.vwm")), PRESET).unwrap();
        }
        std::fs::write(dir.path().join("A.packed"), PRESET_PACKED).unwrap();
        std::fs::write(
            dir.path().join("B.sha256"),
            format!(
                "{}  B.packed\n",
                super::super::record::sha256_hex(PRESET_PACKED)
            ),
        )
        .unwrap();
        let results = local_folder(dir.path()).unwrap();
        assert!(results[0].identical(), "{}", results[0].summary());
        assert_eq!(results[1].summary(), "B: matches the expected SHA-256");
        assert!(results[2].error.is_some());
    }
}
