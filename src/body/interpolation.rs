//! Compatibility implementation of the original patch-field interpolator.
//! Discrete fields begin as a copy of the left input, matching native memcpy.
use super::*;

fn linear(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn logarithmic(a: f32, b: f32, t: f32) -> f32 {
    // Disassembly shows double log and double products; only the sum is
    // rounded to float before expf. The complement itself is float32.
    let left = (a as f64).ln();
    let right = (b as f64).ln();
    ((left * (1.0 - t) as f64 + right * t as f64) as f32).exp()
}
fn array<const N: usize>(a: &[f32; N], b: &[f32; N], t: f32) -> [f32; N] {
    std::array::from_fn(|i| linear(a[i], b[i], t))
}
fn log_array<const N: usize>(a: &[f32; N], b: &[f32; N], t: f32) -> [f32; N] {
    std::array::from_fn(|i| logarithmic(a[i], b[i], t))
}
fn integer(a: u32, b: u32, t: f32) -> u32 {
    // Native cvtsi2ss is signed, including the wrapping integer subtraction.
    let value = a as i32 as f32 + (b as i32).wrapping_sub(a as i32) as f32 * t;
    value as u32
}
fn bands<const N: usize>(a: &EqBands<N>, b: &EqBands<N>, t: f32) -> EqBands<N> {
    let mut out = a.clone();
    for i in 0..N {
        let left_type = f32::from_bits(a.filter_types[i]);
        let right_type = f32::from_bits(b.filter_types[i]);
        if left_type == 0.0 && right_type != 0.0 {
            out.filter_types[i] = b.filter_types[i];
            out.frequencies[i] = b.frequencies[i];
            out.q[i] = b.q[i];
            out.gain[i] = b.gain[i];
            out.mix[i] = t * b.mix[i];
        } else if left_type != 0.0 && right_type == 0.0 {
            out.mix[i] = (1.0 - t) * a.mix[i];
        } else if left_type == right_type {
            out.frequencies[i] = logarithmic(a.frequencies[i], b.frequencies[i], t);
            out.q[i] = linear(a.q[i], b.q[i], t);
            out.gain[i] = linear(a.gain[i], b.gain[i], t);
            out.mix[i] = linear(a.mix[i], b.mix[i], t);
        }
        // Native prints a mismatch diagnostic and retains the left band when
        // both nonzero filter types differ. Do not invent a filter conversion.
    }
    out
}
fn resonator(
    left: (bool, f32, f32, f32),
    right: (bool, f32, f32, f32),
    t: f32,
) -> (bool, f32, f32, f32) {
    let (ae, af, aq, ag) = left;
    let (be, bf, bq, bg) = right;
    match (ae, be) {
        (false, true) => (true, bf, bq, t * bg + (1.0 - t) * -100.0),
        (true, false) => (true, af, aq, (1.0 - t) * ag + t * -100.0),
        _ => (
            ae,
            logarithmic(af, bf, t),
            linear(aq, bq, t),
            linear(ag, bg, t),
        ),
    }
}

impl BodyVariant {
    /// Canonical big-endian field encoding, including normalized boolean bytes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(BODY_VARIANT_BYTES);
        self.encode(&mut bytes);
        bytes
    }

    /// Original host-library field blending, not DSP execution or device output.
    /// Zero-frequency logarithms may produce NaNs at endpoints, as in native code.
    pub fn interpolate_with(&self, other: &Self, t: f32) -> Result<Self, String> {
        if !t.is_finite() || !(0.0..=1.0).contains(&t) {
            return Err("interpolation weight must be finite and in 0..=1".into());
        }
        if self.pickup_position.num_coils > 4 || other.pickup_position.num_coils > 4 {
            return Err("body interpolation supports at most four pickup coils".into());
        }
        let a = self;
        let b = other;
        let mut out = a.clone();
        let ar = &a.body_resonance;
        let br = &b.body_resonance;
        let r = &mut out.body_resonance;
        if ar.enabled == br.enabled {
            for i in 0..12 {
                (r.resonator_enabled[i], r.frequencies[i], r.q[i], r.gain[i]) = resonator(
                    (
                        ar.resonator_enabled[i],
                        ar.frequencies[i],
                        ar.q[i],
                        ar.gain[i],
                    ),
                    (
                        br.resonator_enabled[i],
                        br.frequencies[i],
                        br.q[i],
                        br.gain[i],
                    ),
                    t,
                );
            }
            r.treble_frequency = logarithmic(ar.treble_frequency, br.treble_frequency, t);
            r.rolloff_frequency = logarithmic(ar.rolloff_frequency, br.rolloff_frequency, t);
            r.master = linear(ar.master, br.master, t);
            r.body_mix = linear(ar.body_mix, br.body_mix, t);
            r.mid_mix = linear(ar.mid_mix, br.mid_mix, t);
            r.treble_mix = linear(ar.treble_mix, br.treble_mix, t);
        } else {
            let active = if ar.enabled { ar } else { br };
            r.resonator_enabled = active.resonator_enabled;
            r.frequencies = active.frequencies;
            r.q = active.q;
            r.gain = active.gain;
            r.treble_frequency = active.treble_frequency;
            r.rolloff_frequency = active.rolloff_frequency;
            r.master = active.master;
            if ar.enabled {
                r.body_mix = linear(ar.body_mix, 0.0, t);
                r.mid_mix = linear(ar.mid_mix, 1.0, t);
                r.treble_mix = linear(ar.treble_mix, 0.0, t);
            } else {
                r.body_mix = t * br.body_mix + 0.0;
                r.mid_mix = (br.mid_mix - 1.0) * t + 1.0;
                r.treble_mix = t * br.treble_mix + 0.0;
            }
        }
        if a.low_cut.enabled == b.low_cut.enabled {
            out.low_cut.frequency = logarithmic(a.low_cut.frequency, b.low_cut.frequency, t);
        } else {
            out.low_cut.enabled = true;
            out.low_cut.fourth_order = if a.low_cut.enabled {
                a.low_cut.fourth_order
            } else {
                b.low_cut.fourth_order
            };
            out.low_cut.frequency = if a.low_cut.enabled {
                logarithmic(a.low_cut.frequency, 20.0, t)
            } else {
                logarithmic(20.0, b.low_cut.frequency, t)
            };
        }
        for i in 0..6 {
            out.string_eq.strings[i].level = linear(
                a.string_eq.strings[i].level,
                b.string_eq.strings[i].level,
                t,
            );
            out.string_eq.strings[i].bands = bands(
                &a.string_eq.strings[i].bands,
                &b.string_eq.strings[i].bands,
                t,
            );
            out.twelve_string_eq.strings[i] = bands(
                &a.twelve_string_eq.strings[i],
                &b.twelve_string_eq.strings[i],
                t,
            );
            let sa = &a.string_tone.strings[i];
            let sb = &b.string_tone.strings[i];
            let so = &mut out.string_tone.strings[i];
            so.master = linear(sa.master, sb.master, t);
            so.hold_off = linear(sa.hold_off, sb.hold_off, t);
            so.decay = linear(sa.decay, sb.decay, t);
            for j in 0..6 {
                (
                    so.resonator_enabled[j],
                    so.frequencies[j],
                    so.q[j],
                    so.gain[j],
                ) = resonator(
                    (
                        sa.resonator_enabled[j],
                        sa.frequencies[j],
                        sa.q[j],
                        sa.gain[j],
                    ),
                    (
                        sb.resonator_enabled[j],
                        sb.frequencies[j],
                        sb.q[j],
                        sb.gain[j],
                    ),
                    t,
                );
            }
        }
        out.global_decay.hold_off = array(&a.global_decay.hold_off, &b.global_decay.hold_off, t);
        out.global_decay.decay = array(&a.global_decay.decay, &b.global_decay.decay, t);
        out.global_decay.minimum = array(&a.global_decay.minimum, &b.global_decay.minimum, t);
        out.sum_to_mono_levels = array(&a.sum_to_mono_levels, &b.sum_to_mono_levels, t);
        out.volume.curve = array(&a.volume.curve, &b.volume.curve, t);
        out.volume.analog_gain = integer(a.volume.analog_gain, b.volume.analog_gain, t);
        out.volume.continuous_gain = linear(a.volume.continuous_gain, b.volume.continuous_gain, t);
        out.reverb.allpass_delays = array(&a.reverb.allpass_delays, &b.reverb.allpass_delays, t);
        out.reverb.allpass_density = array(&a.reverb.allpass_density, &b.reverb.allpass_density, t);
        out.reverb.tank_decay = array(&a.reverb.tank_decay, &b.reverb.tank_decay, t);
        out.reverb.pre_delay = linear(a.reverb.pre_delay, b.reverb.pre_delay, t);
        out.drone.level = linear(a.drone.level, b.drone.level, t);
        out.drone.frequencies = log_array(&a.drone.frequencies, &b.drone.frequencies, t);
        out.drone.feedback = array(&a.drone.feedback, &b.drone.feedback, t);
        out.sitar_buzz.decay = array(&a.sitar_buzz.decay, &b.sitar_buzz.decay, t);
        out.sitar_buzz.start_frequency = log_array(
            &a.sitar_buzz.start_frequency,
            &b.sitar_buzz.start_frequency,
            t,
        );
        out.sitar_buzz.stop_frequency = log_array(
            &a.sitar_buzz.stop_frequency,
            &b.sitar_buzz.stop_frequency,
            t,
        );
        out.sitar_buzz.q = array(&a.sitar_buzz.q, &b.sitar_buzz.q, t);
        out.sitar_buzz.mix = array(&a.sitar_buzz.mix, &b.sitar_buzz.mix, t);
        out.pitch_shift.tune = array(&a.pitch_shift.tune, &b.pitch_shift.tune, t);
        out.pitch_shift.delay = array(&a.pitch_shift.delay, &b.pitch_shift.delay, t);
        out.pitch_shift.mix = array(&a.pitch_shift.mix, &b.pitch_shift.mix, t);
        out.pitch_shift.level = array(&a.pitch_shift.level, &b.pitch_shift.level, t);
        out.pickup_response.lowpass_frequency = log_array(
            &a.pickup_response.lowpass_frequency,
            &b.pickup_response.lowpass_frequency,
            t,
        );
        out.pickup_response.tone2_gain = array(
            &a.pickup_response.tone2_gain,
            &b.pickup_response.tone2_gain,
            t,
        );
        out.pickup_response.tone2_q =
            array(&a.pickup_response.tone2_q, &b.pickup_response.tone2_q, t);
        out.pickup_response.tone1_gain = array(
            &a.pickup_response.tone1_gain,
            &b.pickup_response.tone1_gain,
            t,
        );
        out.pickup_response.tone_off_frequency = log_array(
            &a.pickup_response.tone_off_frequency,
            &b.pickup_response.tone_off_frequency,
            t,
        );
        out.pickup_response.tone_off_q = array(
            &a.pickup_response.tone_off_q,
            &b.pickup_response.tone_off_q,
            t,
        );
        out.pickup_response.tone_off_gain = array(
            &a.pickup_response.tone_off_gain,
            &b.pickup_response.tone_off_gain,
            t,
        );
        out.pickup_response.tone_off_gain_scalar = linear(
            a.pickup_response.tone_off_gain_scalar,
            b.pickup_response.tone_off_gain_scalar,
            t,
        );
        out.pickup_response.tone2_frequency = logarithmic(
            a.pickup_response.tone2_frequency,
            b.pickup_response.tone2_frequency,
            t,
        );
        out.pickup_position.scale_length = linear(
            a.pickup_position.scale_length,
            b.pickup_position.scale_length,
            t,
        );
        out.pickup_position.num_coils =
            a.pickup_position.num_coils.max(b.pickup_position.num_coils);
        out.pickup_position.magnetic_aperture = std::array::from_fn(|i| {
            integer(
                a.pickup_position.magnetic_aperture[i],
                b.pickup_position.magnetic_aperture[i],
                t,
            )
        });
        for coil in 0..4 {
            let ai = if a.pickup_position.num_coils != 0
                && coil >= a.pickup_position.num_coils as usize
            {
                coil - a.pickup_position.num_coils as usize
            } else {
                coil
            };
            let bi = if b.pickup_position.num_coils != 0
                && coil >= b.pickup_position.num_coils as usize
            {
                coil - b.pickup_position.num_coils as usize
            } else {
                coil
            };
            out.pickup_position.coils[coil].pole_positions = array(
                &a.pickup_position.coils[ai].pole_positions,
                &b.pickup_position.coils[bi].pole_positions,
                t,
            );
        }
        out.pick_filter.frequency =
            logarithmic(a.pick_filter.frequency, b.pick_filter.frequency, t);
        out.pick_filter.q = linear(a.pick_filter.q, b.pick_filter.q, t);
        out.pick_filter.decay = linear(a.pick_filter.decay, b.pick_filter.decay, t);
        out.pick_filter.gain = array(&a.pick_filter.gain, &b.pick_filter.gain, t);
        out.compressor.attack_time = linear(a.compressor.attack_time, b.compressor.attack_time, t);
        out.compressor.release_time =
            linear(a.compressor.release_time, b.compressor.release_time, t);
        out.compressor.post_gain = array(&a.compressor.post_gain, &b.compressor.post_gain, t);
        out.mic_position.notch_depth =
            array(&a.mic_position.notch_depth, &b.mic_position.notch_depth, t);
        out.mic_position.position = array(&a.mic_position.position, &b.mic_position.position, t);
        out.mic_position.random_level = array(
            &a.mic_position.random_level,
            &b.mic_position.random_level,
            t,
        );
        Ok(out)
    }
}

impl BodyBlendPlan {
    pub fn evaluate(&self, variants: &[BodyVariant; 6]) -> Result<BodyVariant, String> {
        match self {
            Self::Variant(i) => variants
                .get(*i)
                .cloned()
                .ok_or_else(|| "body variant index out of range".into()),
            Self::Blend {
                left,
                right,
                weight,
            } => left
                .evaluate(variants)?
                .interpolate_with(&right.evaluate(variants)?, *weight),
        }
    }
}
impl BodyDefinition {
    pub fn interpolate_normalized(&self, x: f32, y: f32) -> Result<BodyVariant, String> {
        BodyBlendPlan::from_normalized(self.interpolation_mode, x, y)?.evaluate(&self.variants)
    }
}
