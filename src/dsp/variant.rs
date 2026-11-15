//! All-block assembly from a typed body variant, in original native call order.
use super::{NativePatch, PickupResponseControls};
use crate::{
    body::{BodyVariant, PickupResponse},
    pickup::{CalculatedResponse, GeometryInput, GeometryProjection},
};

pub struct RenderOptions<'a> {
    pub response: Option<&'a CalculatedResponse>,
    pub response_controls: Option<PickupResponseControls>,
    pub geometry: Option<&'a GeometryProjection>,
    pub volume_trim_db: Option<f32>,
    pub mic_scale_length: f32,
}
impl Default for RenderOptions<'_> {
    fn default() -> Self {
        Self {
            response: None,
            response_controls: None,
            geometry: None,
            volume_trim_db: None,
            mic_scale_length: 25.0,
        }
    }
}
fn from_body_response(body: &PickupResponse, rate: f32) -> Result<CalculatedResponse, String> {
    if !rate.is_finite()
        || !(1.0..=384000.0).contains(&rate)
        || body
            .lowpass_frequency
            .iter()
            .chain(body.tone_off_frequency.iter())
            .chain([body.tone2_frequency].iter())
            .any(|v| !v.is_finite())
    {
        return Err("unsupported body pickup-response frequencies".into());
    }
    let coefficient =
        |freq: f32| (1.0 / f64::from((freq * std::f32::consts::PI) / rate).tan()) as f32;
    Ok(CalculatedResponse {
        lowpass_frequency: body.lowpass_frequency,
        secondary_gain: body.tone2_gain,
        secondary_q: body.tone2_q,
        secondary_frequency: body.tone2_frequency,
        tone_gain: body.tone1_gain,
        primary_gain: body.tone_off_gain,
        primary_q: body.tone_off_q,
        primary_frequency: body.tone_off_frequency,
        lowpass_coefficient: body
            .lowpass_frequency
            .map(|f| ((f * -std::f32::consts::TAU) / rate).exp()),
        secondary_frequency_coefficient: coefficient(body.tone2_frequency),
        primary_frequency_coefficient: body.tone_off_frequency.map(coefficient),
    })
}
impl NativePatch {
    /// Render numeric blocks only. Model selection, Java conversions, names,
    /// final host masks and device-specific transport are not performed here.
    pub fn from_body_variant(
        body: &BodyVariant,
        rate: f32,
        options: RenderOptions<'_>,
    ) -> Result<Self, String> {
        Self::render_body_variant(body, rate, options, false)
    }
    pub(crate) fn from_interpolated_body_variant(
        body: &BodyVariant,
        rate: f32,
        options: RenderOptions<'_>,
    ) -> Result<Self, String> {
        Self::render_body_variant(body, rate, options, true)
    }
    fn render_body_variant(
        body: &BodyVariant,
        rate: f32,
        options: RenderOptions<'_>,
        derived: bool,
    ) -> Result<Self, String> {
        let mut patch = Self::default();
        patch.write_body_resonance(&body.body_resonance, rate)?;
        patch.write_low_cut(&body.low_cut, rate)?;
        patch.write_string_eq(&body.string_eq, rate)?;
        patch.write_string_tone(&body.string_tone, rate)?;
        patch.write_global_decay(&body.global_decay, rate)?;
        patch.write_sum_to_mono(false, body.sum_to_mono_levels)?;
        patch.write_reverb(&body.reverb, rate)?;
        patch.write_volume(&body.volume, options.volume_trim_db)?;
        patch.write_twelve_string_eq(&body.twelve_string_eq, rate)?;
        if derived {
            patch.write_interpolated_sitar(&body.sitar_buzz, rate)?;
        } else {
            patch.write_sitar(&body.sitar_buzz, rate)?;
        }
        patch.write_pitch_shift(&body.pitch_shift, rate)?;
        patch.write_drone(&body.drone, rate)?;
        let calculated;
        let response = if let Some(response) = options.response {
            response
        } else {
            calculated = from_body_response(&body.pickup_response, rate)?;
            &calculated
        };
        let controls = options.response_controls.unwrap_or(PickupResponseControls {
            enabled: body.pickup_response.enabled,
            tone_position: 0.,
            save_tone: false,
            tone_function: 1,
            primary_gain_scalar: body.pickup_response.tone_off_gain_scalar,
        });
        patch.write_pickup_response(response, controls)?;
        let projected;
        let geometry = if let Some(geometry) = options.geometry {
            geometry
        } else {
            let input = &body.pickup_position;
            projected = GeometryInput {
                enabled: input.enabled,
                scale_length: input.scale_length,
                num_coils: input.num_coils,
                magnetic_aperture: input.magnetic_aperture,
                coil_gain_db: [0.; 4],
                positions: std::array::from_fn(|i| input.coils[i].pole_positions),
                phases: std::array::from_fn(|i| input.coils[i].reverse_phase),
            }
            .project()?;
            &projected
        };
        patch.write_pickup_geometry(geometry)?;
        // These optional final blocks intentionally overlay existing ranges.
        if body.pick_filter.enabled {
            patch.write_pick_filter(&body.pick_filter, rate)?;
        }
        if body.compressor.enabled {
            patch.write_compressor(&body.compressor, rate)?;
        }
        if body.mic_position.enabled {
            patch.write_mic_position_scaled(&body.mic_position, options.mic_scale_length)?;
        }
        Ok(patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{BODY_DEFINITION_BYTES, BodyDefinition};
    use crate::dsp::{DATA_WORDS, LEGACY_SAMPLE_RATE};

    #[test]
    fn all_block_assembly_matches_native_for_48_synthetic_variants() {
        let inputs = include_bytes!("../../tests/fixtures/native-dsp-body-inputs.bin");
        let outputs = include_bytes!("../../tests/fixtures/native-dsp-body-outputs.bin");
        assert_eq!(inputs.len(), 8 * BODY_DEFINITION_BYTES);
        assert_eq!(outputs.len(), 48 * DATA_WORDS * 4);
        for (body_index, bytes) in inputs
            .as_chunks::<BODY_DEFINITION_BYTES>()
            .0
            .iter()
            .enumerate()
        {
            let body = BodyDefinition::parse(bytes).unwrap();
            for (variant_index, variant) in body.variants.iter().enumerate() {
                let patch = NativePatch::from_body_variant(
                    variant,
                    LEGACY_SAMPLE_RATE,
                    RenderOptions::default(),
                )
                .unwrap();
                let offset = (body_index * 6 + variant_index) * DATA_WORDS * 4;
                for (word_index, actual) in patch.words.iter().enumerate() {
                    let start = offset + word_index * 4;
                    let expected =
                        u32::from_le_bytes(outputs[start..start + 4].try_into().unwrap());
                    assert_eq!(
                        *actual, expected,
                        "body {body_index}, variant {variant_index}, word {word_index}"
                    );
                }
            }
        }
    }

    #[test]
    fn all_block_assembly_rejects_invalid_rate_and_control_overrides() {
        let inputs = include_bytes!("../../tests/fixtures/native-dsp-body-inputs.bin");
        let body = BodyDefinition::parse(&inputs[..BODY_DEFINITION_BYTES]).unwrap();
        for rate in [f32::NAN, f32::INFINITY, 0., -1., 384001.] {
            assert!(
                NativePatch::from_body_variant(&body.variants[0], rate, RenderOptions::default())
                    .is_err()
            );
        }
        assert!(
            NativePatch::from_body_variant(
                &body.variants[0],
                LEGACY_SAMPLE_RATE,
                RenderOptions {
                    volume_trim_db: Some(f32::NAN),
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
