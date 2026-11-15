//! Native host orchestration for the legacy (non-0x70005) device path only.
use super::WorkbenchSettings;
use crate::{
    body::BodyDefinition,
    dsp::{LEGACY_SAMPLE_RATE, NativePatch, PickupResponseControls, RenderOptions},
    models::{PickupCircuit, PickupDefinition, PickupWiring},
    pickup::{
        BalancedLevels, CalculatedResponse, GeometryInput, PickupLevel, TableSettings, VolumeTaper,
        WiringProperties,
    },
};
use std::collections::BTreeMap;

/// Definitions must be supplied from the user's local files; none are bundled.
#[derive(Default, Debug)]
pub struct ModelLibrary {
    pub bodies: BTreeMap<u32, BodyDefinition>,
    pub pickups: BTreeMap<u32, PickupDefinition>,
    /// Original special response tables for alternate IDs -3 and -7, supplied
    /// separately when the original volume/tone/capacitance predicates match.
    pub special_responses: BTreeMap<i32, CalculatedResponse>,
}
pub struct HostEngine<'a> {
    library: &'a ModelLibrary,
    coil_gain_db: [f32; 4],
}
impl<'a> HostEngine<'a> {
    pub fn new(library: &'a ModelLibrary) -> Self {
        Self {
            library,
            coil_gain_db: [0.; 4],
        }
    }
    pub fn generate(&mut self, settings: &WorkbenchSettings) -> Result<NativePatch, String> {
        let s = settings;
        if [0, 1, 8, 9, 48, 49, 50, 76, 156, 164]
            .into_iter()
            .any(|i| s.native[i] > 1)
            || (24..48)
                .step_by(4)
                .chain((56..76).step_by(4))
                .chain((80..156).step_by(4))
                .chain((168..192).step_by(4))
                .any(|i| !s.float(i).is_finite())
        {
            return Err("noncanonical flag or nonfinite host input".into());
        }
        let library = self.library;
        let body = library
            .bodies
            .get(&s.word(4))
            .or_else(|| library.bodies.get(&0x3100003))
            .ok_or("requested body and native fallback body are unavailable")?;
        if body.variants.iter().any(|v| {
            v.sitar_buzz
                .start_frequency
                .iter()
                .chain(v.sitar_buzz.stop_frequency.iter())
                .any(|x| !x.is_finite())
        }) {
            return Err(
                "nonfinite source sitar frequency; only internally derived NaNs are supported"
                    .into(),
            );
        }
        let acoustic = s.native[0] != 0;
        let selected: [Option<&PickupDefinition>; 2] = std::array::from_fn(|i| {
            if acoustic || s.native[8 + i] == 0 || s.word(12 + i * 4) == u32::MAX {
                None
            } else {
                library.pickups.get(&s.word(12 + i * 4))
            }
        });
        let alternate = if acoustic {
            None
        } else {
            library.pickups.get(&s.word(20))
        };
        let normalized = [
            body.normalized_pickup_position(s.float(24))?,
            body.normalized_pickup_position(s.float(28))?,
        ];
        let x = if selected[0].is_some() {
            normalized[0]
        } else {
            normalized[1]
        };
        let y = if selected[1].is_some() {
            normalized[1]
        } else {
            normalized[0]
        };
        let mut patch = body.interpolate_normalized(x, y)?;
        let calculated = if selected.iter().any(Option::is_some) {
            let empty = PickupCircuit {
                dc_resistance: 0.,
                magnetic_resistance: 0.,
                inductance: 0.,
                capacitance: 0.,
            };
            let mut circuits = selected.map(|p| {
                p.map_or(empty, |p| {
                    let mut c = PickupCircuit::from(p);
                    c.dc_resistance = c.dc_resistance.clamp(1., 1e6);
                    c.magnetic_resistance = c.magnetic_resistance.clamp(1., 1e6);
                    c.inductance = c.inductance.clamp(0., 100.);
                    c.capacitance = c.capacitance.clamp(1e-14, 0.005);
                    c
                })
            });
            if let Some(p) = alternate {
                circuits = [PickupCircuit::from(p), empty];
            }
            let cap = s.float(64) as f64;
            let wiring = WiringProperties {
                load_resistance: s.float(68) as f64,
                load_capacitance: s.float(72) as f64,
                volume_resistance: (s.float(56) as f64).clamp(100., 1e7),
                tone_resistance: (s.float(60) as f64).clamp(100., 1e7),
                tone_capacitance: cap.abs().clamp(1e-15, 0.1).copysign(cap),
            };
            circuits[0].calculated_response(
                circuits[1],
                wiring,
                TableSettings {
                    sample_rate: LEGACY_SAMPLE_RATE,
                    wiring: if s.native[50] != 0 {
                        PickupWiring::Series
                    } else {
                        PickupWiring::Parallel
                    },
                    taper: if s.word(52) != 0 {
                        VolumeTaper::Logarithmic
                    } else {
                        VolumeTaper::Linear
                    },
                },
            )?
        } else {
            None
        };
        let special = s.word(20) as i32;
        let special_active = match special {
            -3 => {
                (s.float(56) - 1e6).abs() < 100.
                    && (s.float(60) - 1e6).abs() < 100.
                    && (s.float(64) - 2.2e-8).abs() < 1e-9
            }
            -7 => {
                (s.float(56) - 500000.).abs() < 100.
                    && (s.float(60) - 500000.).abs() < 100.
                    && (s.float(64) - 4.7e-8).abs() < 1e-9
            }
            _ => false,
        };
        patch.volume.curve = if s.word(52) != 0 {
            [1.; 9]
        } else {
            std::array::from_fn(|i| i as f32 * 0.125)
        };
        let mut geometry = GeometryInput {
            enabled: patch.pickup_position.enabled,
            scale_length: patch.pickup_position.scale_length,
            num_coils: patch.pickup_position.num_coils,
            magnetic_aperture: patch.pickup_position.magnetic_aperture,
            coil_gain_db: self.coil_gain_db,
            positions: std::array::from_fn(|i| patch.pickup_position.coils[i].pole_positions),
            phases: std::array::from_fn(|i| patch.pickup_position.coils[i].reverse_phase),
        };
        let trim;
        if !acoustic {
            let levels = BalancedLevels::electric(
                PickupLevel {
                    model_offset_db: selected[0].map(|p| p.volume_offset),
                    level_db: s.float(40),
                },
                PickupLevel {
                    model_offset_db: selected[1].map(|p| p.volume_offset),
                    level_db: s.float(44),
                },
                alternate.map(|p| p.volume_offset),
                patch.volume.continuous_gain,
                s.float(188),
                normalized[0] < normalized[1],
            )?;
            trim = levels.trim_db;
            let single_coils = (s.native[76] != 0 || s.native[164] != 0 || s.native[1] != 0)
                && selected.iter().all(Option::is_some);
            let mut count = 0;
            for (i, model) in selected.into_iter().enumerate() {
                let Some(model) = model else {
                    continue;
                };
                let coils = model.place_coils(
                    body.pole_offsets_at(normalized[i])?,
                    s.float(24 + i * 4),
                    s.float(32 + i * 4),
                    s.native[48 + i] != 0,
                    single_coils,
                )?;
                for positions in coils.positions {
                    geometry.positions[count] = positions;
                    geometry.phases[count] = [coils.reverse_phase; 6];
                    geometry.coil_gain_db[count] = levels.pickup_db[i];
                    count += 1;
                }
            }
            geometry.num_coils = count as u32;
        } else {
            let analog = match patch.volume.analog_gain {
                1 => -6.2,
                2 => -10.5,
                3 => -15.5,
                _ => 0.,
            };
            trim = (patch.volume.continuous_gain + (analog + s.float(188))).min(0.);
        }
        patch.volume.enabled = true;
        // Native local_d8c counts newly placed electric coils, not the body's
        // stored geometry count used by acoustic definitions.
        let host_coil_count = if acoustic { 0 } else { geometry.num_coils };
        let tuning = s.native[76] != 0 || s.native[1] != 0;
        if tuning {
            let was_pitch = patch.pitch_shift.enabled;
            patch.pitch_shift.enabled = true;
            patch.twelve_string_eq.enabled = true;
            if !was_pitch || host_coil_count > 1 {
                patch.string_eq.enabled = false;
                patch.string_tone.enabled = false;
                patch.drone.enabled = false;
                if patch.pickup_response.enabled {
                    patch.reverb.enabled = false;
                }
                if host_coil_count > 1 {
                    patch.low_cut.enabled = false;
                    patch.global_decay.enabled = false;
                }
            }
            if s.native[1] == 0 {
                for i in 0..6 {
                    patch.pitch_shift.tune[5 - i] = s.float(80 + i * 4);
                    patch.pitch_shift.mix[5 - i] = s.float(104 + i * 4);
                    if s.float(128) >= 0. {
                        patch.pitch_shift.delay[5 - i] = s.float(128 + i * 4);
                    }
                }
            }
        } else {
            patch.pitch_shift.enabled = false;
            patch.twelve_string_eq.enabled = false;
        }
        if s.native[164] != 0 {
            patch.sitar_buzz.enabled = true;
            patch.drone.enabled = true;
            patch.pitch_shift.enabled = false;
            patch.twelve_string_eq.enabled = false;
            patch.drone.num_strings = 5;
            patch.drone.level = 0.08;
            for (i, feedback) in [0.997, 0.99, 0.99, 0.996, 0.995].into_iter().enumerate() {
                patch.drone.frequencies[i] = s.float(168 + i * 4);
                patch.drone.feedback[i] = feedback;
            }
        }
        let projected = geometry.project()?;
        // Special response tables replace the calculated tables, as in native.
        let response = if special_active {
            Some(
                library
                    .special_responses
                    .get(&special)
                    .ok_or("required native special-combination response tables are unavailable")?,
            )
        } else {
            calculated.as_ref()
        };
        let mut output = NativePatch::from_interpolated_body_variant(
            &patch,
            LEGACY_SAMPLE_RATE,
            RenderOptions {
                response,
                response_controls: Some(PickupResponseControls {
                    enabled: patch.pickup_response.enabled,
                    tone_position: s.float(152),
                    save_tone: s.native[156] != 0,
                    tone_function: u16::try_from(s.word(160))
                        .map_err(|_| "unsupported tone-function selector")?,
                    primary_gain_scalar: patch.pickup_response.tone_off_gain_scalar,
                }),
                geometry: Some(&projected),
                volume_trim_db: Some(trim),
                ..Default::default()
            },
        )?;
        if tuning || s.native[164] != 0 {
            output.set_block_enabled(0x8000, true);
        }
        output.set_block_enabled(0x10000, !tuning);
        output.set_block_status(output.block_status() & s.block_mask);
        self.coil_gain_db = geometry.coil_gain_db;
        Ok(output)
    }
}

impl ModelLibrary {
    /// A local, tab-separated kind/hex-ID/path manifest. Paths may be absolute
    /// or relative to the manifest. Original resources stay outside the repo.
    pub fn load_manifest(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        if text.len() > 1024 * 1024 {
            return Err("model manifest is oversized".into());
        }
        let mut out = Self::default();
        let mut count = 0;
        for line in text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            count += 1;
            if count > 256 {
                return Err("model manifest exceeds 256 definitions".into());
            }
            let f: Vec<_> = line.splitn(3, '\t').collect();
            if f.len() != 3 {
                return Err("manifest requires kind, hexadecimal ID and path".into());
            }
            let id = u32::from_str_radix(f[1], 16).map_err(|e| e.to_string())?;
            let resource = path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join(f[2]);
            let size = match f[0] {
                "body" => crate::body::BODY_DEFINITION_BYTES,
                "pickup" => 85,
                "special" if matches!(id as i32, -3 | -7) => 360,
                _ => return Err("unsupported manifest definition kind".into()),
            };
            let meta = std::fs::metadata(&resource).map_err(|e| e.to_string())?;
            if !meta.is_file() || meta.len() != size as u64 {
                return Err("model resource is not a regular file of the required size".into());
            }
            let bytes = std::fs::read(resource).map_err(|e| e.to_string())?;
            let duplicate = if f[0] == "body" {
                out.bodies
                    .insert(id, BodyDefinition::parse(&bytes)?)
                    .is_some()
            } else if f[0] == "pickup" {
                out.pickups
                    .insert(id, PickupDefinition::parse(&bytes)?)
                    .is_some()
            } else {
                if bytes[8..12] != LEGACY_SAMPLE_RATE.to_le_bytes() {
                    return Err("special response snapshot has the wrong internal rate".into());
                }
                let (response, _) = crate::dsp::response_from_analysis(&bytes)?;
                out.special_responses.insert(id as i32, response).is_some()
            };
            if duplicate {
                return Err("duplicate model definition ID".into());
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const BODY_IDS: [u32; 8] = [
        0x3100003, 0x310000d, 0x3100012, 0x3100013, 0x3100015, 0x3100018, 0x310001b, 0x3100011,
    ];
    fn synthetic_library() -> ModelLibrary {
        let mut out = ModelLibrary::default();
        for (i, body) in include_bytes!("../../tests/fixtures/native-dsp-body-inputs.bin")
            .as_chunks::<17483>()
            .0
            .iter()
            .enumerate()
        {
            out.bodies
                .insert(BODY_IDS[i], BodyDefinition::parse(body).unwrap());
        }
        for (id, bytes) in [0x3110001, 0x3110000, (-2i32) as u32, (-3i32) as u32]
            .into_iter()
            .zip(
                include_bytes!("../../tests/fixtures/native-host-pickups.bin")
                    .as_chunks::<85>()
                    .0,
            )
        {
            out.pickups
                .insert(id, PickupDefinition::parse(bytes).unwrap());
        }
        out
    }
    fn settings(index: usize) -> WorkbenchSettings {
        let bytes = include_bytes!("../../tests/fixtures/native-host-inputs.bin");
        WorkbenchSettings {
            native: bytes[index * 192..(index + 1) * 192].try_into().unwrap(),
            block_mask: u32::MAX,
        }
    }
    #[test]
    fn complete_stateful_host_engine_matches_48_original_synthetic_outputs() {
        let library = synthetic_library();
        let mut engine = HostEngine::new(&library);
        let expected = include_bytes!("../../tests/fixtures/native-host-outputs.bin");
        assert_eq!(expected.len(), 48 * 3276);
        for (i, packed) in expected.as_chunks::<3276>().0.iter().enumerate() {
            assert_eq!(
                &engine.generate(&settings(i)).unwrap().packed(),
                packed,
                "synthetic preset {i}"
            );
        }
    }
    #[test]
    fn failed_input_and_missing_special_tables_leave_engine_state_unchanged() {
        let library = synthetic_library();
        let mut engine = HostEngine::new(&library);
        let mut control = HostEngine::new(&library);
        for i in 0..2 {
            assert_eq!(
                engine.generate(&settings(i)).unwrap(),
                control.generate(&settings(i)).unwrap()
            );
        }
        let mut invalid = settings(2);
        invalid.native[8] = 2;
        assert!(engine.generate(&invalid).is_err());
        invalid = settings(2);
        invalid.native[24..28].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(engine.generate(&invalid).is_err());
        invalid = settings(1);
        invalid.native[20..24].copy_from_slice(&(-3i32).to_le_bytes());
        for (offset, value) in [(56, 1e6_f32), (60, 1e6), (64, 2.2e-8)] {
            invalid.native[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        assert!(
            engine
                .generate(&invalid)
                .unwrap_err()
                .contains("special-combination")
        );
        assert_eq!(
            engine.generate(&settings(2)).unwrap(),
            control.generate(&settings(2)).unwrap()
        );
    }
    #[test]
    fn host_block_mask_preserves_the_native_always_enabled_bits() {
        let library = synthetic_library();
        let mut all = HostEngine::new(&library);
        let mut masked = HostEngine::new(&library);
        let mut input = settings(1);
        let mut expected = all.generate(&input).unwrap();
        input.block_mask = 0;
        let actual = masked.generate(&input).unwrap();
        assert_eq!(actual.block_status(), 0x883);
        expected.words[1] = 0x883;
        assert_eq!(actual, expected);
    }
    #[test]
    fn manifest_rejects_ambiguous_ids_bad_resource_sizes_and_wrong_special_rate() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = dir.path().join("models.tsv");
        let model = dir.path().join("pickup.bin");
        std::fs::write(
            &model,
            &include_bytes!("../../tests/fixtures/native-host-pickups.bin")[..85],
        )
        .unwrap();
        std::fs::write(
            &manifest,
            "pickup\t03110001\tpickup.bin\npickup\t03110001\tpickup.bin\n",
        )
        .unwrap();
        assert!(
            ModelLibrary::load_manifest(&manifest)
                .unwrap_err()
                .contains("duplicate")
        );
        std::fs::write(&model, [0; 84]).unwrap();
        std::fs::write(&manifest, "pickup\t03110001\tpickup.bin\n").unwrap();
        assert!(
            ModelLibrary::load_manifest(&manifest)
                .unwrap_err()
                .contains("required size")
        );
        std::fs::write(&model, [0; 360]).unwrap();
        std::fs::write(&manifest, "special\tfffffffd\tpickup.bin\n").unwrap();
        assert!(
            ModelLibrary::load_manifest(&manifest)
                .unwrap_err()
                .contains("wrong internal rate")
        );
    }
}
