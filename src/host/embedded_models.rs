//! Versioned, little-endian model records for the legacy host engine.
use super::ModelLibrary;
use crate::{
    body::{BODY_DEFINITION_BYTES, BodyDefinition},
    models::PickupDefinition,
    pickup::CalculatedResponse,
};
use flate2::{Compression, Decompress, FlushDecompress, Status, write::ZlibEncoder};
use std::{io::Write, sync::OnceLock};

const BUNDLE: &[u8] = include_bytes!("../../assets/variax-models/legacy.vxm.zlib");
const MAGIC: &[u8; 8] = b"VXMODEL1";
const BODY_IDS: std::ops::RangeInclusive<u32> = 0x0310_0000..=0x0310_001b;
const BODY_COUNT: usize = 28;
const PICKUP_COUNT: usize = 43;
const SPECIAL_IDS: [i32; 2] = [-7, -3];
const PICKUP_BYTES: usize = 85;
const RESPONSE_BYTES: usize = 83 * 4;
const DECODED_BYTES: usize = MAGIC.len()
    + BODY_COUNT * (4 + BODY_DEFINITION_BYTES)
    + PICKUP_COUNT * (4 + PICKUP_BYTES)
    + SPECIAL_IDS.len() * (4 + RESPONSE_BYTES);

impl ModelLibrary {
    /// The bundled Workbench 1.75 definitions. Fully offline and cached.
    pub fn built_in() -> Result<&'static Self, String> {
        static MODELS: OnceLock<Result<ModelLibrary, String>> = OnceLock::new();
        MODELS
            .get_or_init(|| decode(BUNDLE))
            .as_ref()
            .map_err(Clone::clone)
    }

    pub fn built_in_asset() -> &'static [u8] {
        BUNDLE
    }

    /// Convert a complete local Workbench manifest into the app-owned format.
    pub fn encode_model_bundle(&self) -> Result<Vec<u8>, String> {
        self.check_model_set()?;
        let mut data = Vec::with_capacity(DECODED_BYTES);
        data.extend_from_slice(MAGIC);
        for (&id, model) in &self.bodies {
            data.extend(id.to_le_bytes());
            data.extend(model.model_record());
        }
        for (&id, model) in &self.pickups {
            data.extend(id.to_le_bytes());
            data.push(u8::from(model.is_humbucker));
            for value in [model.volume_offset]
                .into_iter()
                .chain(model.pole_offsets)
                .chain(model.coil_spacing)
            {
                data.extend(value.to_le_bytes());
            }
            for value in [
                model.dc_resistance,
                model.magnetic_resistance,
                model.inductance,
                model.capacitance,
            ] {
                data.extend(value.to_le_bytes());
            }
        }
        for (&id, model) in &self.special_responses {
            data.extend(id.to_le_bytes());
            for (_, bits) in model.fields() {
                data.extend(bits.to_le_bytes());
            }
        }
        let mut compressed = ZlibEncoder::new(Vec::new(), Compression::best());
        compressed
            .write_all(&data)
            .map_err(|error| error.to_string())?;
        compressed.finish().map_err(|error| error.to_string())
    }

    fn check_model_set(&self) -> Result<(), String> {
        if !self.bodies.keys().copied().eq(BODY_IDS)
            || !self.pickups.keys().copied().eq((0x0311_0000..=0x0311_0015)
                .chain(0x0311_8000..=0x0311_8009)
                .chain(-12i32..=-2)
                .map(|id| id as u32))
            || !self.special_responses.keys().copied().eq(SPECIAL_IDS)
        {
            return Err("The model bundle requires 28 legacy bodies, 43 pickups and special responses -7 and -3.".into());
        }
        Ok(())
    }
}

fn decode(compressed: &[u8]) -> Result<ModelLibrary, String> {
    let mut data = vec![0; DECODED_BYTES + 1];
    let mut decoder = Decompress::new(true);
    let status = decoder
        .decompress(compressed, &mut data, FlushDecompress::Finish)
        .map_err(|error| format!("Could not inflate the Variax model bundle: {error}"))?;
    if status != Status::StreamEnd
        || decoder.total_out() != DECODED_BYTES as u64
        || !data.starts_with(MAGIC)
        || decoder.total_in() != compressed.len() as u64
    {
        return Err("Invalid Variax model bundle version or length.".into());
    }

    let mut models = ModelLibrary::default();
    let mut records = &data[MAGIC.len()..DECODED_BYTES];
    for _ in 0..BODY_COUNT {
        let id = u32::from_le_bytes(take(&mut records));
        let model =
            BodyDefinition::parse_model_record(&take::<BODY_DEFINITION_BYTES>(&mut records))?;
        models.bodies.insert(id, model);
    }
    for _ in 0..PICKUP_COUNT {
        let id = u32::from_le_bytes(take(&mut records));
        let [is_humbucker] = take(&mut records);
        let volume_offset = read_float(&mut records);
        let pole_offsets = std::array::from_fn(|_| read_float(&mut records));
        let coil_spacing = std::array::from_fn(|_| read_float(&mut records));
        let model = PickupDefinition {
            dc_resistance: f64::from_le_bytes(take(&mut records)),
            magnetic_resistance: f64::from_le_bytes(take(&mut records)),
            inductance: f64::from_le_bytes(take(&mut records)),
            capacitance: f64::from_le_bytes(take(&mut records)),
            is_humbucker: is_humbucker != 0,
            volume_offset,
            pole_offsets,
            coil_spacing,
        };
        models.pickups.insert(id, model);
    }
    for _ in SPECIAL_IDS {
        let id = i32::from_le_bytes(take(&mut records));
        let response = CalculatedResponse {
            lowpass_frequency: read_curve(&mut records),
            secondary_gain: read_curve(&mut records),
            secondary_q: read_curve(&mut records),
            secondary_frequency: read_float(&mut records),
            tone_gain: read_curve(&mut records),
            primary_gain: read_curve(&mut records),
            primary_q: read_curve(&mut records),
            primary_frequency: read_curve(&mut records),
            lowpass_coefficient: read_curve(&mut records),
            secondary_frequency_coefficient: read_float(&mut records),
            primary_frequency_coefficient: read_curve(&mut records),
        };
        models.special_responses.insert(id, response);
    }
    models.check_model_set()?;
    debug_assert!(records.is_empty());
    Ok(models)
}

// The version and exact decompressed length have been checked before parsing.
fn take<const N: usize>(bytes: &mut &[u8]) -> [u8; N] {
    let (value, rest) = bytes.split_first_chunk::<N>().expect("model bundle layout");
    *bytes = rest;
    *value
}

fn read_float(bytes: &mut &[u8]) -> f32 {
    f32::from_le_bytes(take(bytes))
}

fn read_curve(bytes: &mut &[u8]) -> [f32; 9] {
    std::array::from_fn(|_| read_float(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_models_round_trip_into_the_converters_canonical_format() {
        let bundle = ModelLibrary::built_in().unwrap();
        assert_eq!(bundle.encode_model_bundle().unwrap(), BUNDLE);
        assert!(
            bundle
                .bodies
                .values()
                .all(|body| body.original_bytes().is_empty())
        );
    }

    #[test]
    fn invalid_streams_do_not_load() {
        assert!(decode(&BUNDLE[..BUNDLE.len() - 3]).is_err());
        assert!(decode(&[BUNDLE, b"extra"].concat()).is_err());
        assert!(ModelLibrary::default().encode_model_bundle().is_err());
    }
}
