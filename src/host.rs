//! File-only legacy Java host mapping and final metadata overlay.
//! This module never opens a device or builds a transport session.
use crate::{dsp::PACKED_BYTES, preset::PresetFile};
use std::collections::BTreeMap;
mod engine;
pub use engine::{HostEngine, ModelLibrary};

/// Original logical parameter IDs and raw scalar bits, before Java conversions.
#[derive(Clone, Debug, Default)]
pub struct HostParameters(pub BTreeMap<u32, u32>);
impl HostParameters {
    pub fn bits(&self, id: u32) -> u32 {
        self.0.get(&id).copied().unwrap_or(0)
    }
    pub fn float(&self, id: u32) -> f32 {
        f32::from_bits(self.bits(id))
    }
    /// Require a complete legacy document rather than invent missing models or
    /// mutate the editor as the original GetModelForGroup(create=true) can do.
    pub fn from_preset(preset: &PresetFile) -> Result<Self, String> {
        if !matches!(preset.device_id(), 0x70000 | 0x70004) {
            return Err("unsupported host-pipeline device family".into());
        }
        let mut out = Self::default();
        for (slot, model_id, enabled_id, fields) in [
            (0x2010000, None, None, vec![(0x200000, 0xff0000)]),
            (
                0x2010001,
                Some(0x1020000),
                Some(0x2020001),
                vec![(0x200000, 131074), (0x200001, 131075), (0x200003, 131076)],
            ),
            (
                0x2010002,
                Some(0x1030000),
                Some(0x2030001),
                vec![
                    (0x200000, 196610),
                    (0x200001, 196611),
                    (0x200003, 196612),
                    (0x200002, 196613),
                    (0x200004, 196614),
                ],
            ),
            (0x2010003, Some(0x1010000), None, vec![]),
            (
                0x2010004,
                None,
                None,
                (0..6).map(|i| (0x200000 + i, 262144 + i)).collect(),
            ),
            (
                0x2010005,
                None,
                Some(0x2050000),
                (0..19).map(|i| (0x200000 + i, 327681 + i)).collect(),
            ),
        ] {
            let model = preset
                .model(slot)
                .ok_or_else(|| format!("missing host model slot {slot:08x}"))?;
            if let Some(id) = model_id {
                out.0.insert(id, model.id);
            }
            if let Some(id) = enabled_id {
                out.0.insert(id, u32::from(model.enabled));
            }
            for (local, logical) in fields {
                let field = model
                    .parameters
                    .iter()
                    .find(|p| p.id == local)
                    .ok_or_else(|| format!("missing host parameter {slot:08x}/{local:06x}"))?;
                out.0.insert(logical, field.bits);
            }
        }
        Ok(out)
    }
    pub fn map(&self, options: HostOptions) -> Result<WorkbenchSettings, String> {
        let mut out = WorkbenchSettings {
            native: [0; 192],
            block_mask: options.block_mask,
        };
        let body = self.bits(0x1010000);
        let acoustic = matches!(body, 0x3100012..=0x3100017 | 0x310001a | 0x310001b);
        let twelve = matches!(body, 0x310000d | 0x3100013 | 0x3100015);
        let sitar = body == 0x3100018;
        out.native[0] = u8::from(acoustic);
        out.put(4, body);
        for (i, start) in [131074, 196610].into_iter().enumerate() {
            out.native[8 + i] = u8::from(self.bits([0x2020001, 0x2030001][i]) != 0);
            out.put(12 + i * 4, self.bits([0x1020000, 0x1030000][i]));
            out.put_float(24 + i * 4, self.float(start))?;
            out.put_float(32 + i * 4, self.float(start + 1) / 45.0 * 1.0)?;
            out.put_float(40 + i * 4, self.float(start + 2))?;
        }
        out.put(20, u32::MAX);
        out.native[49] = u8::from(self.bits(196613) != 0);
        out.native[50] = u8::from(self.bits(196614) != 0);
        out.put(52, self.bits(262145));
        for (offset, id, scale) in [(56, 262144, 1000.), (60, 262146, 1000.), (64, 262147, 1e-6)] {
            out.put_float(offset, self.float(id) * scale)?;
        }
        out.put_float(68, 1e6)?;
        out.put_float(72, 3.5e-10)?;
        let tuning = self.bits(0x2050000) != 0 && !sitar;
        out.native[76] = u8::from(tuning);
        for i in 0..6 {
            let mut tune = self.bits(327681 + i as u32) as i32 as f32;
            let mix;
            if !twelve && !options.enable_all_twelve_string {
                mix = 1.;
            } else if tuning || !twelve {
                mix = self.float(327693 + i as u32);
                tune += self.float(327687 + i as u32) / 100.;
            } else {
                out.native[1] = 1;
                mix = 0.;
            }
            out.put_float(80 + i * 4, tune)?;
            out.put_float(104 + i * 4, mix)?;
            out.put_float(128 + i * 4, if twelve { -1. } else { 0. })?;
        }
        out.put_float(152, self.float(262148))?;
        out.native[156] = u8::from(self.bits(262149) != 0);
        out.put(
            160,
            if acoustic {
                3
            } else if sitar {
                2
            } else {
                1
            },
        );
        out.native[164] = u8::from(sitar);
        for (i, value) in [493.8, 261.6, 293.7, 329.6, 392.08].into_iter().enumerate() {
            out.put_float(168 + i * 4, value)?;
        }
        out.put_float(
            188,
            if options.muted {
                -1000.
            } else {
                self.float(0xff0000)
            },
        )?;
        out.apply_model_adjustments();
        Ok(out)
    }
    /// Overlay original name/metadata fields, then recompute the Java checksum.
    /// The original covers 1089 words (not the native packer's 1091 words).
    pub fn finish(
        &self,
        name: &str,
        settings: &WorkbenchSettings,
        mut packed: [u8; PACKED_BYTES],
    ) -> [u8; PACKED_BYTES] {
        let ascii: Vec<_> = name
            .chars()
            .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
            .collect();
        for (i, byte) in packed[2553..2571].iter_mut().enumerate() {
            *byte = ascii.get(i).copied().unwrap_or(b' ');
        }
        let mut meta = Vec::new();
        let f24 = |meta: &mut Vec<u8>, bits: u32| meta.extend_from_slice(&bits.to_be_bytes()[..3]);
        meta.extend_from_slice(&self.bits(0x1010000).to_be_bytes());
        for (enable, model, start) in [
            (0x2020001, 0x1020000, 131074),
            (0x2030001, 0x1030000, 196610),
        ] {
            meta.push(u8::from(self.bits(enable) != 0));
            meta.extend_from_slice(&self.bits(model).to_be_bytes());
            for id in start..start + 3 {
                f24(&mut meta, self.bits(id));
            }
        }
        meta.push(u8::from(self.bits(196613) != 0));
        for id in [262147, 262146, 262144] {
            f24(&mut meta, self.bits(id));
        }
        meta.push(self.bits(262145) as u8);
        f24(&mut meta, self.bits(262148));
        meta.push(u8::from(self.bits(262149) != 0));
        f24(&mut meta, settings.word(188));
        meta.push(u8::from(self.bits(0x2050000) != 0));
        for id in 327681..=327686 {
            meta.push(self.bits(id) as u8);
        }
        for id in 327687..=327699 {
            f24(&mut meta, self.bits(id));
        }
        meta.push(u8::from(self.bits(196614) != 0));
        packed[2571..2571 + meta.len()].copy_from_slice(&meta);
        let checksum = packed[..1089 * 3]
            .as_chunks::<3>()
            .0
            .iter()
            .fold(0_u32, |sum, w| {
                sum.wrapping_add(u32::from_be_bytes([0, w[0], w[1], w[2]]))
            })
            & 0xffffff;
        packed[3273..].copy_from_slice(&checksum.to_be_bytes()[1..]);
        packed
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HostOptions {
    pub muted: bool,
    pub enable_all_twelve_string: bool,
    pub block_mask: u32,
}
impl Default for HostOptions {
    fn default() -> Self {
        Self {
            muted: false,
            enable_all_twelve_string: false,
            block_mask: u32::MAX,
        }
    }
}
#[derive(Clone, Debug)]
pub struct WorkbenchSettings {
    pub native: [u8; 192],
    pub block_mask: u32,
}
impl WorkbenchSettings {
    pub fn word(&self, offset: usize) -> u32 {
        u32::from_le_bytes(self.native[offset..offset + 4].try_into().unwrap())
    }
    pub fn float(&self, offset: usize) -> f32 {
        f32::from_bits(self.word(offset))
    }
    fn put(&mut self, offset: usize, value: u32) {
        self.native[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn put_float(&mut self, offset: usize, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err("nonfinite host setting or converted value".into());
        }
        self.put(offset, value.to_bits());
        Ok(())
    }
    fn apply_model_adjustments(&mut self) {
        const COMBINATIONS: [(u32, u32, bool, i32); 11] = [
            (0x3110001, 0x3110000, false, -2),
            (0x3110001, 0x3110000, true, -3),
            (0x3110003, 51445762, false, -4),
            (51445764, 0x3110003, false, -5),
            (51478537, 51478536, false, -6),
            (51478529, 51478528, false, -7),
            (51445766, 51445765, false, -8),
            (51445771, 51445770, false, -9),
            (51445775, 51445774, false, -10),
            (51478531, 51478530, false, -11),
            (51478533, 51478532, false, -12),
        ];
        let ids = [self.word(12), self.word(16)];
        if self.native[8] != 0 && self.native[9] != 0 {
            for (a, b, series, combined) in COMBINATIONS {
                if (ids == [a, b] || ids == [b, a]) && series == (self.native[50] != 0) {
                    self.put(20, combined as u32);
                }
            }
        }
        for (i, id) in ids.into_iter().enumerate() {
            if self.native[8 + i] == 0 {
                continue;
            }
            let cap = match id {
                51445778 | 0x3110013 => 8.75e-10_f32,
                51445770 | 51445771 | 51445774 | 51445775 | 51478532 | 51478533 => 5.25e-10,
                _ => continue,
            };
            if cap > self.float(72) {
                self.put(72, cap.to_bits());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }
    #[test]
    fn original_java_mapping_hacks_metadata_and_checksum_match_240_cases() {
        let mut count = 0;
        for line in include_str!("../tests/fixtures/original-java-host-pipeline.txt").lines() {
            let (input, expected) = line.split_once(" => ").unwrap();
            let f: Vec<_> = input.split_whitespace().collect();
            let name = String::from_utf8(hex(f[0])).unwrap();
            let flags = f[1].parse::<u32>().unwrap();
            let mask = u32::from_str_radix(f[2], 16).unwrap();
            let mut seed = u32::from_str_radix(f[3], 16).unwrap();
            let mut parameters = HostParameters::default();
            for item in &f[4..] {
                let (id, bits) = item.split_once(':').unwrap();
                parameters.0.insert(
                    u32::from_str_radix(id, 16).unwrap(),
                    u32::from_str_radix(bits, 16).unwrap(),
                );
            }
            let settings = parameters
                .map(HostOptions {
                    muted: flags & 2 != 0,
                    enable_all_twelve_string: flags & 1 != 0,
                    block_mask: mask,
                })
                .unwrap();
            let e: Vec<_> = expected.split_whitespace().collect();
            assert_eq!(
                settings.native.as_slice(),
                hex(e[0]),
                "native mapping case {count}"
            );
            assert_eq!(settings.block_mask, u32::from_str_radix(e[1], 16).unwrap());
            let mut packed = [0; PACKED_BYTES];
            for byte in &mut packed {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                *byte = (seed >> 24) as u8;
            }
            assert_eq!(
                parameters.finish(&name, &settings, packed).as_slice(),
                hex(e[2]),
                "final bytes case {count}"
            );
            count += 1;
        }
        assert_eq!(count, 240);
    }
    #[test]
    fn file_mapping_uses_model_local_ids_without_mutating_the_document() {
        let mut raw = [0; 192];
        raw[..18].fill(b' ');
        let metadata = crate::PresetMetadata::from_bytes(raw).unwrap();
        let preset = PresetFile::from_metadata(&metadata, 0x70004, 0x310).unwrap();
        let before = preset.as_bytes().to_vec();
        let mapped = HostParameters::from_preset(&preset).unwrap();
        assert_eq!(mapped.0.len(), 40);
        assert_eq!(mapped.bits(0x1010000), 0);
        assert_eq!(mapped.bits(0x2050000), 0);
        assert_eq!(preset.as_bytes(), before);
        let incomplete =
            PresetFile::parse(include_bytes!("../tests/fixtures/editor-preset.vwm").to_vec())
                .unwrap();
        assert!(HostParameters::from_preset(&incomplete).is_err());
    }
    #[test]
    fn conversion_overflows_and_nonfinite_used_fields_are_rejected() {
        for value in [f32::NAN, f32::INFINITY, f32::MAX] {
            let mut p = HostParameters::default();
            p.0.insert(262144, value.to_bits());
            assert!(p.map(HostOptions::default()).is_err());
            assert_eq!(p.float(262144).to_bits(), value.to_bits());
        }
    }
}
