//! Offline decoding of the original Workbench body-definition resources.
//!
//! Verified against the original i386 reader; see docs/body-definitions.md.
//! Decoded values are a view of the input. The original bytes remain unchanged.

mod interpolation;

pub const BODY_DEFINITION_BYTES: usize = 17_483;
pub const BODY_VARIANT_BYTES: usize = 2_897;
pub const BODY_VARIANT_NAMES: [&str; 6] = ["bb", "mb", "nb", "mm", "nm", "nn"];

#[derive(Debug, Clone, PartialEq)]
pub struct PickupDefaults {
    pub position: f32,
    pub angle: f32,
    pub pole_offsets: [f32; 6],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BodyResonance {
    pub enabled: bool,
    pub resonator_enabled: [bool; 12],
    pub frequencies: [f32; 12],
    pub q: [f32; 12],
    pub gain: [f32; 12],
    pub treble_frequency: f32,
    pub rolloff_frequency: f32,
    pub master: f32,
    pub body_mix: f32,
    pub mid_mix: f32,
    pub treble_mix: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LowCut {
    pub enabled: bool,
    pub frequency: f32,
    pub fourth_order: bool,
}

// Wire declarations follow the original serializer's field order. Integer
// discriminants remain raw until their DSP semantics are independently verified.
trait Wire: Sized {
    fn decode(input: &mut Reader<'_>) -> Self;
    fn encode(&self, output: &mut Vec<u8>);
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let value = self.bytes[self.offset..self.offset + N].try_into().unwrap();
        self.offset += N;
        value
    }
}
impl Wire for bool {
    fn decode(input: &mut Reader<'_>) -> Self {
        input.take::<1>()[0] != 0
    }
    fn encode(&self, output: &mut Vec<u8>) {
        output.push(u8::from(*self));
    }
}
impl Wire for u32 {
    fn decode(input: &mut Reader<'_>) -> Self {
        Self::from_be_bytes(input.take())
    }
    fn encode(&self, output: &mut Vec<u8>) {
        output.extend(self.to_be_bytes());
    }
}
impl Wire for f32 {
    fn decode(input: &mut Reader<'_>) -> Self {
        Self::from_be_bytes(input.take())
    }
    fn encode(&self, output: &mut Vec<u8>) {
        output.extend(self.to_be_bytes());
    }
}
impl<T: Wire, const N: usize> Wire for [T; N] {
    fn decode(input: &mut Reader<'_>) -> Self {
        std::array::from_fn(|_| T::decode(input))
    }
    fn encode(&self, output: &mut Vec<u8>) {
        for value in self {
            value.encode(output);
        }
    }
}
macro_rules! wire_struct {
    ($name:ident $(<$n:ident>)? { $($field:ident: $kind:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name $(<const $n: usize>)? { $(pub $field: $kind),* }
        impl $(<const $n: usize>)? Wire for $name $(<$n>)? {
            fn decode(input: &mut Reader<'_>) -> Self {
                Self { $($field: <$kind>::decode(input)),* }
            }
            fn encode(&self, output: &mut Vec<u8>) { $(self.$field.encode(output);)* }
        }
    };
}
// These two public structs retain their original API.
impl Wire for BodyResonance {
    fn decode(input: &mut Reader<'_>) -> Self {
        Self {
            enabled: bool::decode(input),
            resonator_enabled: <[bool; 12]>::decode(input),
            frequencies: <[f32; 12]>::decode(input),
            q: <[f32; 12]>::decode(input),
            gain: <[f32; 12]>::decode(input),
            treble_frequency: f32::decode(input),
            rolloff_frequency: f32::decode(input),
            master: f32::decode(input),
            body_mix: f32::decode(input),
            mid_mix: f32::decode(input),
            treble_mix: f32::decode(input),
        }
    }
    fn encode(&self, out: &mut Vec<u8>) {
        self.enabled.encode(out);
        self.resonator_enabled.encode(out);
        self.frequencies.encode(out);
        self.q.encode(out);
        self.gain.encode(out);
        self.treble_frequency.encode(out);
        self.rolloff_frequency.encode(out);
        self.master.encode(out);
        self.body_mix.encode(out);
        self.mid_mix.encode(out);
        self.treble_mix.encode(out);
    }
}
impl Wire for LowCut {
    fn decode(input: &mut Reader<'_>) -> Self {
        Self {
            enabled: bool::decode(input),
            frequency: f32::decode(input),
            fourth_order: bool::decode(input),
        }
    }
    fn encode(&self, out: &mut Vec<u8>) {
        self.enabled.encode(out);
        self.frequency.encode(out);
        self.fourth_order.encode(out);
    }
}
wire_struct!(EqBands<N> {
    filter_types: [u32; N], frequencies: [f32; N], q: [f32; N], gain: [f32; N], mix: [f32; N],
});
wire_struct!(StringEqString { level: f32, bands: EqBands<6> });
wire_struct!(StringEq {
    enabled: bool,
    strings: [StringEqString; 6]
});
wire_struct!(TwelveStringEq { enabled: bool, strings: [EqBands<2>; 6] });
wire_struct!(StringToneString {
    master: f32,
    hold_off: f32,
    decay: f32,
    resonator_enabled: [bool; 6],
    frequencies: [f32; 6],
    q: [f32; 6],
    gain: [f32; 6],
});
wire_struct!(StringTone {
    enabled: bool,
    strings: [StringToneString; 6]
});
wire_struct!(GlobalDecay {
    enabled: bool,
    hold_off: [f32; 6],
    decay: [f32; 6],
    minimum: [f32; 6]
});
wire_struct!(Volume {
    enabled: bool,
    curve: [f32; 9],
    analog_gain: u32,
    continuous_gain: f32
});
wire_struct!(Reverb {
    enabled: bool,
    allpass_delays: [f32; 4],
    allpass_density: [f32; 9],
    tank_decay: [f32; 9],
    pre_delay: f32,
    allpass_density_control_source: u32,
    tank_feedback_control_source: u32,
});
wire_struct!(Drone {
    enabled: bool,
    num_strings: u32,
    level: f32,
    frequencies: [f32; 12],
    feedback: [f32; 12]
});
wire_struct!(SitarBuzz {
    enabled: bool,
    decay: [f32; 6],
    start_frequency: [f32; 6],
    stop_frequency: [f32; 6],
    q: [f32; 6],
    mix: [f32; 6],
});
wire_struct!(PitchShift {
    enabled: bool,
    tune: [f32; 6],
    delay: [f32; 6],
    mix: [f32; 6],
    level: [f32; 6]
});
wire_struct!(PickupResponse {
    enabled: bool,
    lowpass_frequency: [f32; 9],
    tone2_gain: [f32; 9],
    tone2_q: [f32; 9],
    tone1_gain: [f32; 9],
    tone_off_frequency: [f32; 9],
    tone_off_q: [f32; 9],
    tone_off_gain: [f32; 9],
    tone_off_gain_scalar: f32,
    tone2_frequency: f32,
});
wire_struct!(PickupCoil {
    pole_positions: [f32; 6],
    reverse_phase: [bool; 6]
});
wire_struct!(PickupPosition {
    enabled: bool,
    scale_length: f32,
    num_coils: u32,
    magnetic_aperture: [u32; 6],
    coils: [PickupCoil; 4]
});
wire_struct!(PickFilter {
    enabled: bool,
    frequency: f32,
    q: f32,
    decay: f32,
    gain: [f32; 9]
});
wire_struct!(Compressor {
    enabled: bool,
    attack_time: f32,
    release_time: f32,
    post_gain: [f32; 9]
});
wire_struct!(MicPosition {
    enabled: bool,
    notch_depth: [f32; 9],
    position: [f32; 9],
    random_level: [f32; 9]
});
wire_struct!(BodyVariant {
    body_resonance: BodyResonance,
    low_cut: LowCut,
    string_eq: StringEq,
    twelve_string_eq: TwelveStringEq,
    string_tone: StringTone,
    global_decay: GlobalDecay,
    sum_to_mono_levels: [f32; 6],
    volume: Volume,
    reverb: Reverb,
    drone: Drone,
    sitar_buzz: SitarBuzz,
    pitch_shift: PitchShift,
    pickup_response: PickupResponse,
    pickup_position: PickupPosition,
    pick_filter: PickFilter,
    compressor: Compressor,
    mic_position: MicPosition,
});

#[derive(Debug, Clone, PartialEq)]
pub struct BodyDefinition {
    original: Vec<u8>,
    pub has_middle: bool,
    /// Bridge, middle, then neck, including defaults for absent pickups.
    pub pickup_defaults: [PickupDefaults; 3],
    /// Native interpolation mode 0..4; see docs/body-definitions.md.
    pub interpolation_mode: u32,
    /// Original order: bb, mb, nb, mm, nm, nn.
    pub variants: [BodyVariant; 6],
}

fn float(bytes: &[u8], offset: usize) -> f32 {
    f32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn floats<const N: usize>(bytes: &[u8], offset: usize) -> [f32; N] {
    std::array::from_fn(|i| float(bytes, offset + i * 4))
}

/// The exact nested interpolation operations selected by the native routing
/// helpers. Field evaluation is implemented separately by evaluate().
#[derive(Debug, Clone, PartialEq)]
pub enum BodyBlendPlan {
    Variant(usize),
    Blend {
        left: Box<Self>,
        right: Box<Self>,
        weight: f32,
    },
}
impl BodyBlendPlan {
    fn blend(left: Self, right: Self, weight: f32) -> Self {
        Self::Blend {
            left: Box::new(left),
            right: Box::new(right),
            weight,
        }
    }
    fn triangle(x: f32, y: f32, a: usize, b: usize, c: usize) -> Self {
        let x = x.clamp(0.0, 1.0);
        let y = y.clamp(0.0, x);
        let weight = if x == 0.0 { 0.0 } else { y / x };
        Self::blend(
            Self::blend(Self::Variant(a), Self::Variant(b), x),
            Self::blend(Self::Variant(a), Self::Variant(c), x),
            weight,
        )
    }
    fn bilinear(x: f32, y: f32, a: usize, b: usize, c: usize, d: usize) -> Self {
        let x = x.clamp(0.0, 1.0);
        let y = y.clamp(0.0, 1.0);
        Self::blend(
            Self::blend(Self::Variant(a), Self::Variant(b), x),
            Self::blend(Self::Variant(c), Self::Variant(d), x),
            y,
        )
    }
    pub fn from_normalized(mode: u32, x: f32, y: f32) -> Result<Self, String> {
        if !x.is_finite()
            || !y.is_finite()
            || !(0.0..=1.0).contains(&x)
            || !(0.0..=1.0).contains(&y)
        {
            return Err("normalized pickup positions must be finite and in 0..=1".into());
        }
        let (high, low) = if x < y { (y, x) } else { (x, y) };
        Ok(match mode {
            1 => Self::blend(Self::Variant(0), Self::Variant(5), (x + y) * 0.5),
            2 => {
                let sum = x + y;
                if sum >= 1.0 {
                    Self::blend(Self::Variant(3), Self::Variant(5), sum - 1.0)
                } else {
                    Self::blend(Self::Variant(0), Self::Variant(3), sum)
                }
            }
            3 => Self::triangle(high, low, 0, 2, 5),
            4 => {
                let high = high + high;
                let low = low + low;
                if high < 1.0 {
                    Self::triangle(high, low, 0, 1, 3)
                } else if low < 1.0 {
                    Self::bilinear(high - 1.0, low, 1, 2, 3, 4)
                } else {
                    Self::triangle(high - 1.0, low - 1.0, 3, 4, 5)
                }
            }
            _ => Self::Variant(0),
        })
    }
}
impl std::fmt::Display for BodyBlendPlan {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Variant(index) => write!(out, "V{index}"),
            Self::Blend {
                left,
                right,
                weight,
            } => write!(out, "B({:08x},{left},{right})", weight.to_bits()),
        }
    }
}

impl BodyDefinition {
    /// Decode an exact legacy resource. Booleans follow the native != 0 rule;
    /// floating-point bit patterns are retained, including nonfinite values.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != BODY_DEFINITION_BYTES {
            return Err(format!(
                "body definition must contain {BODY_DEFINITION_BYTES} bytes, got {}",
                bytes.len()
            ));
        }
        let pickup_defaults = std::array::from_fn(|i| {
            let start = 1 + i * 32;
            PickupDefaults {
                position: float(bytes, start),
                angle: float(bytes, start + 4),
                pole_offsets: floats(bytes, start + 8),
            }
        });
        let variants = std::array::from_fn(|i| {
            let b = &bytes[101 + i * BODY_VARIANT_BYTES..101 + (i + 1) * BODY_VARIANT_BYTES];
            let mut input = Reader {
                bytes: b,
                offset: 0,
            };
            let result = BodyVariant::decode(&mut input);
            assert_eq!(input.offset, BODY_VARIANT_BYTES);
            result
        });
        Ok(Self {
            original: bytes.to_vec(),
            has_middle: bytes[0] != 0,
            pickup_defaults,
            interpolation_mode: u32::from_be_bytes(bytes[97..101].try_into().unwrap()),
            variants,
        })
    }

    /// Serialize the typed view for offline verification. True booleans become
    /// byte 1; use original_bytes() to preserve noncanonical boolean bytes.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(BODY_DEFINITION_BYTES);
        self.has_middle.encode(&mut bytes);
        for defaults in &self.pickup_defaults {
            defaults.position.encode(&mut bytes);
            defaults.angle.encode(&mut bytes);
            defaults.pole_offsets.encode(&mut bytes);
        }
        self.interpolation_mode.encode(&mut bytes);
        self.variants.encode(&mut bytes);
        bytes
    }

    /// Native piecewise mapping from stored pickup-position units to 0..=1.
    /// Invalid anchors/nonfinite input are rejected before any calculation.
    pub fn normalized_pickup_position(&self, position: f32) -> Result<f32, String> {
        let [bridge, middle, neck] = self.pickup_defaults.each_ref().map(|p| p.position);
        if !position.is_finite()
            || !bridge.is_finite()
            || !neck.is_finite()
            || bridge >= neck
            || !(neck - bridge).is_finite()
            || (self.has_middle && (!middle.is_finite() || middle <= bridge || middle >= neck))
        {
            return Err(
                "pickup position and active anchors must be finite, strictly ordered and non-overflowing".into(),
            );
        }
        Ok(if position <= bridge {
            0.0
        } else if position >= neck {
            1.0
        } else if !self.has_middle {
            (position - bridge) / (neck - bridge)
        } else if position >= middle {
            ((position - middle) / (neck - middle)) * 0.5 + 0.5
        } else {
            ((position - bridge) / (middle - bridge)) * 0.5
        })
    }

    /// Plan using physical positions in the resource's stored units. Blending
    /// the selected fields is performed separately by BodyBlendPlan::evaluate.
    pub fn interpolation_plan(&self, first: f32, second: f32) -> Result<BodyBlendPlan, String> {
        BodyBlendPlan::from_normalized(
            self.interpolation_mode,
            self.normalized_pickup_position(first)?,
            self.normalized_pickup_position(second)?,
        )
    }

    /// Exact input, including every variant field. Editing the
    /// decoded public values does not modify these bytes or create a device patch.
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }

    pub fn variant_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= self.variants.len() {
            return None;
        }
        let start = 101 + index * BODY_VARIANT_BYTES;
        Some(&self.original[start..start + BODY_VARIANT_BYTES])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/native-body-definition.bin");

    #[test]
    fn native_verified_synthetic_header_and_all_variants() {
        let body = BodyDefinition::parse(FIXTURE).unwrap();
        assert!(body.has_middle);
        assert_eq!(body.interpolation_mode, 4);
        for (i, p) in body.pickup_defaults.iter().enumerate() {
            assert_eq!(p.position, (2 + i * 3) as f32 * 0.125);
            assert_eq!(p.angle, (3 + i * 3) as f32 * 0.125);
            for j in 0..6 {
                assert_eq!(
                    p.pole_offsets[j],
                    (4 + i * 3) as f32 * 0.125 + j as f32 * 0.03125
                );
            }
        }
        for (i, variant) in body.variants.iter().enumerate() {
            let base = i * 235;
            let r = &variant.body_resonance;
            assert_eq!(r.enabled, (11 + base) % 2 != 0);
            for j in 0..12 {
                assert_eq!(r.resonator_enabled[j], (12 + base + j) % 2 != 0);
                assert_eq!(
                    r.frequencies[j],
                    (14 + base) as f32 * 0.125 + j as f32 * 0.03125
                );
                assert_eq!(r.q[j], (15 + base) as f32 * 0.125 + j as f32 * 0.03125);
                assert_eq!(r.gain[j], (16 + base) as f32 * 0.125 + j as f32 * 0.03125);
            }
            assert_eq!(
                [
                    r.treble_frequency,
                    r.rolloff_frequency,
                    r.master,
                    r.body_mix,
                    r.mid_mix,
                    r.treble_mix
                ],
                std::array::from_fn::<_, 6, _>(|j| (17 + base + j) as f32 * 0.125)
            );
            assert_eq!(variant.low_cut.enabled, (22 + base) % 2 != 0);
            assert_eq!(variant.low_cut.frequency, (24 + base) as f32 * 0.125);
            assert_eq!(variant.low_cut.fourth_order, (24 + base) % 2 != 0);
            assert_eq!(body.variant_bytes(i).unwrap().len(), BODY_VARIANT_BYTES);
        }
        assert_eq!(body.original_bytes(), FIXTURE);
        assert!(body.variant_bytes(6).is_none());
        assert!(body.variant_bytes(usize::MAX).is_none());
    }

    #[test]
    fn rejects_every_truncated_input_and_trailing_data() {
        for len in 0..FIXTURE.len() {
            assert!(BodyDefinition::parse(&FIXTURE[..len]).is_err());
        }
        let mut extra = FIXTURE.to_vec();
        extra.push(0);
        assert!(BodyDefinition::parse(&extra).is_err());
    }

    #[test]
    fn native_boolean_semantics_and_float_bits_are_preserved() {
        let mut bytes = FIXTURE.to_vec();
        bytes[0] = 255;
        bytes[1..5].copy_from_slice(&0x7fc01234u32.to_be_bytes());
        let body = BodyDefinition::parse(&bytes).unwrap();
        assert!(body.has_middle);
        assert_eq!(body.pickup_defaults[0].position.to_bits(), 0x7fc01234);
        assert_eq!(body.original_bytes(), bytes);
    }
    #[test]
    fn every_typed_field_reserializes_and_named_tail_fields_match_the_native_schema() {
        let mut body = BodyDefinition::parse(FIXTURE).unwrap();
        assert_eq!(body.canonical_bytes(), FIXTURE);
        let variant = &body.variants[0];
        assert_eq!(
            variant.string_eq.strings[2].bands.filter_types[4],
            u32::from_be_bytes(FIXTURE[557..561].try_into().unwrap())
        );
        assert_eq!(
            variant.twelve_string_eq.strings[5].mix[1].to_bits(),
            float(FIXTURE, 1270).to_bits()
        );
        assert_eq!(
            variant.pickup_position.coils[3].pole_positions[5].to_bits(),
            float(FIXTURE, 2785).to_bits()
        );
        assert_eq!(
            variant.mic_position.random_level[8].to_bits(),
            float(FIXTURE, 2994).to_bits()
        );
        body.variants[0].mic_position.random_level[8] = f32::from_bits(0x7fc01234);
        let canonical = body.canonical_bytes();
        assert_eq!(&canonical[2994..2998], &0x7fc01234u32.to_be_bytes());
        assert_eq!(body.original_bytes(), FIXTURE);
        assert_eq!(&canonical[..2994], &FIXTURE[..2994]);
        assert_eq!(&canonical[2998..], &FIXTURE[2998..]);
    }

    #[test]
    fn canonical_boolean_encoding_does_not_change_original_bytes() {
        let mut bytes = FIXTURE.to_vec();
        bytes[0] = 255;
        let body = BodyDefinition::parse(&bytes).unwrap();
        assert_eq!(body.canonical_bytes()[0], 1);
        assert_eq!(body.original_bytes(), bytes);
    }

    #[test]
    fn normalized_positions_match_original_machine_code_on_synthetic_anchors() {
        let body = BodyDefinition::parse(FIXTURE).unwrap();
        for line in include_str!("../tests/fixtures/native-body-normalization.txt").lines() {
            let columns: Vec<_> = line.split_whitespace().collect();
            let input = f32::from_bits(u32::from_str_radix(columns[0], 16).unwrap());
            let expected = u32::from_str_radix(columns[1], 16).unwrap();
            assert_eq!(
                body.normalized_pickup_position(input).unwrap().to_bits(),
                expected
            );
        }
    }

    #[test]
    fn normalization_rejects_invalid_active_anchors_and_nonfinite_inputs() {
        let mut body = BodyDefinition::parse(FIXTURE).unwrap();
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(body.normalized_pickup_position(invalid).is_err());
        }
        body.pickup_defaults[1].position = f32::NAN;
        assert!(body.normalized_pickup_position(0.5).is_err());
        body.has_middle = false;
        assert_eq!(body.normalized_pickup_position(0.625).unwrap(), 0.5);
        body.pickup_defaults[2].position = body.pickup_defaults[0].position;
        assert!(body.normalized_pickup_position(0.5).is_err());
        body.pickup_defaults[0].position = -f32::MAX;
        body.pickup_defaults[2].position = f32::MAX;
        assert!(body.normalized_pickup_position(0.0).is_err());
    }

    #[test]
    fn plans_match_original_mode_selection_and_nested_weights() {
        for line in include_str!("../tests/fixtures/native-body-interpolation-plans.txt").lines() {
            let columns: Vec<_> = line.split_whitespace().collect();
            let mode = columns[0].parse().unwrap();
            let x = f32::from_bits(u32::from_str_radix(columns[1], 16).unwrap());
            let y = f32::from_bits(u32::from_str_radix(columns[2], 16).unwrap());
            assert_eq!(
                BodyBlendPlan::from_normalized(mode, x, y)
                    .unwrap()
                    .to_string(),
                columns[3]
            );
        }
    }

    #[test]
    fn plans_reject_nonfinite_or_unnormalized_positions() {
        for bad in [f32::NAN, f32::INFINITY, -0.001, 1.001] {
            assert!(BodyBlendPlan::from_normalized(4, bad, 0.5).is_err());
            assert!(BodyBlendPlan::from_normalized(4, 0.5, bad).is_err());
        }
    }
    #[test]
    fn full_interpolation_matches_original_machine_code_synthetic_results() {
        let body = BodyDefinition::parse(include_bytes!(
            "../tests/fixtures/native-body-interpolation-input.bin"
        ))
        .unwrap();
        let bytes = include_bytes!("../tests/fixtures/native-body-interpolated-output.bin");
        let cases: Vec<_> = include_str!("../tests/fixtures/native-body-interpolated-cases.txt")
            .lines()
            .collect();
        assert_eq!(bytes.len(), cases.len() * BODY_VARIANT_BYTES);
        for (line, expected) in cases.iter().zip(bytes.as_chunks::<BODY_VARIANT_BYTES>().0) {
            let c: Vec<_> = line.split_whitespace().collect();
            let parameter = |s: &str| f32::from_bits(u32::from_str_radix(s, 16).unwrap());
            let result = if c[0] == "blend" {
                body.variants[c[1].parse::<usize>().unwrap()]
                    .interpolate_with(
                        &body.variants[c[2].parse::<usize>().unwrap()],
                        parameter(c[3]),
                    )
                    .unwrap()
            } else {
                BodyBlendPlan::from_normalized(
                    c[1].parse().unwrap(),
                    parameter(c[2]),
                    parameter(c[3]),
                )
                .unwrap()
                .evaluate(&body.variants)
                .unwrap()
            };
            assert_eq!(
                result.canonical_bytes(),
                expected,
                "native mismatch for {line}"
            );
        }
    }

    #[test]
    fn interpolation_rejects_invalid_weights_indices_and_coil_counts() {
        let body = BodyDefinition::parse(include_bytes!(
            "../tests/fixtures/native-body-interpolation-input.bin"
        ))
        .unwrap();
        for invalid in [f32::NAN, f32::INFINITY, -0.001, 1.001] {
            assert!(
                body.variants[0]
                    .interpolate_with(&body.variants[1], invalid)
                    .is_err()
            );
        }
        assert!(BodyBlendPlan::Variant(6).evaluate(&body.variants).is_err());
        let mut invalid = body.variants[0].clone();
        invalid.pickup_position.num_coils = 5;
        assert!(invalid.interpolate_with(&body.variants[1], 0.5).is_err());
        assert!(body.variants[1].interpolate_with(&invalid, 0.5).is_err());
    }
}
