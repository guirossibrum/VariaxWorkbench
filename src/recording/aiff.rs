//! Exact-clock, uncompressed AIFF profile for synthetic POD S24_3LE takes.
//! File codec only. No resampling, playback, device access or OS audio dependency.
#![forbid(unsafe_code)]

use super::{TAKE_BYTES_CEILING, TakeError};

pub const HEADER_BYTES: usize = 54;
/// IEEE extended: exponent 16383+15, explicit significand 78125 << 47.
/// Represents 78125/2 exactly, using integer construction on every platform.
pub const SOURCE_RATE_EXTENDED: [u8; 10] = [0x40, 0x0e, 0x98, 0x96, 0x80, 0, 0, 0, 0, 0];

/// Encode interleaved stereo signed 24-bit little-endian source samples.
/// Only sample byte order changes; original PCM is never modified.
pub fn encode(source_pcm: &[u8]) -> Result<Vec<u8>, TakeError> {
    if source_pcm.len() as u64 > TAKE_BYTES_CEILING {
        return Err(TakeError::CapExceeded);
    }
    if !source_pcm.len().is_multiple_of(6) {
        return Err(TakeError::Aiff(
            "source must contain complete stereo 24-bit frames",
        ));
    }
    let size = u32::try_from(source_pcm.len()).map_err(|_| TakeError::CapExceeded)?;
    let mut file = Vec::with_capacity(HEADER_BYTES + source_pcm.len());
    file.extend_from_slice(b"FORM");
    file.extend_from_slice(&(size + 46).to_be_bytes());
    file.extend_from_slice(b"AIFFCOMM");
    file.extend_from_slice(&18u32.to_be_bytes());
    file.extend_from_slice(&2u16.to_be_bytes());
    file.extend_from_slice(&(size / 6).to_be_bytes());
    file.extend_from_slice(&24u16.to_be_bytes());
    file.extend_from_slice(&SOURCE_RATE_EXTENDED);
    file.extend_from_slice(b"SSND");
    file.extend_from_slice(&(size + 8).to_be_bytes());
    file.extend_from_slice(&[0; 8]); // offset=0, blockSize=0
    for sample in source_pcm.as_chunks::<3>().0 {
        file.extend([sample[2], sample[1], sample[0]]);
    }
    Ok(file)
}

/// Validate/decode this export profile only, returning original S24_3LE PCM.
/// General AIFF, other rates/chunks/compression/layouts are unsupported rather
/// than silently normalized. This parser alone is not independent validation.
pub fn decode(file: &[u8]) -> Result<Vec<u8>, TakeError> {
    if file.len() as u64 > TAKE_BYTES_CEILING + HEADER_BYTES as u64 {
        return Err(TakeError::CapExceeded);
    }
    if file.len() < HEADER_BYTES {
        return Err(TakeError::Aiff("truncated export header"));
    }
    let word = |at| u32::from_be_bytes(file[at..at + 4].try_into().unwrap());
    if &file[..4] != b"FORM"
        || word(4) as usize != file.len() - 8
        || &file[8..16] != b"AIFFCOMM"
        || word(16) != 18
        || file[20..22] != [0, 2]
        || file[26..28] != [0, 24]
        || file[28..38] != SOURCE_RATE_EXTENDED
        || &file[38..42] != b"SSND"
        || word(42) as usize != file.len() - 46
        || file[46..54] != [0; 8]
    {
        return Err(TakeError::Aiff(
            "unsupported or inconsistent exact-clock export profile",
        ));
    }
    let frames = usize::try_from(word(22)).map_err(|_| TakeError::CapExceeded)?;
    if frames.checked_mul(6) != Some(file.len() - HEADER_BYTES) {
        return Err(TakeError::Aiff("frame count does not match sample data"));
    }
    let mut source = Vec::with_capacity(file.len() - HEADER_BYTES);
    for sample in file[HEADER_BYTES..].as_chunks::<3>().0 {
        source.extend([sample[2], sample[1], sample[0]]);
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_extended_clock_and_signed_boundaries_preserve_source_bytes() {
        let values = [0i32, 1, -1, 8_388_607, -8_388_608, -2000];
        let source: Vec<_> = values
            .iter()
            .flat_map(|v| v.to_le_bytes()[..3].to_vec())
            .collect();
        let original = source.clone();
        let file = encode(&source).unwrap();
        assert_eq!(source, original);
        assert_eq!(
            &file[28..38],
            &[0x40, 0x0e, 0x98, 0x96, 0x80, 0, 0, 0, 0, 0]
        );
        // Decode the clock independently as an integer significand and exponent.
        let exponent = u16::from_be_bytes(file[28..30].try_into().unwrap()) as u32;
        let significand = u64::from_be_bytes(file[30..38].try_into().unwrap());
        assert_eq!(exponent, 16_383 + 15);
        assert_eq!(significand, 78_125u64 << 47);
        assert_eq!(
            &file[HEADER_BYTES..],
            &[
                0, 0, 0, 0, 0, 1, 255, 255, 255, 127, 255, 255, 128, 0, 0, 255, 248, 48
            ]
        );
        assert_eq!(decode(&file).unwrap(), original);
        assert_eq!(decode(&encode(&[]).unwrap()).unwrap(), Vec::<u8>::new());
        // Derived fingerprints were independently inspected/decoded with SoX_ng.
        // No decoder is required by the application or default tests.
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../research/aiff-export-reference.json"))
                .unwrap();
        assert_eq!(reference["schema"], 1);
        for case in reference["cases"].as_array().unwrap() {
            let frames: usize = case["frames"].as_str().unwrap().parse().unwrap();
            let mut source = Vec::new();
            for sample in 0..frames * 2 {
                let value = match sample % 8 {
                    0 => 0,
                    1 => 1,
                    2 => 0xffffff,
                    3 => 0x7fffff,
                    4 => 0x800000,
                    _ => {
                        (sample as u32)
                            .wrapping_mul(1_103_515_245)
                            .wrapping_add(12_345)
                            & 0xffffff
                    }
                };
                source.extend_from_slice(&value.to_le_bytes()[..3]);
            }
            assert_eq!(super::super::sha256_hex(&source), case["source_pcm_sha256"]);
            assert_eq!(
                super::super::sha256_hex(&encode(&source).unwrap()),
                case["aiff_sha256"]
            );
            assert_eq!(case["rate"], "39062.5");
        }
    }

    #[test]
    fn malformed_and_truncated_profiles_are_rejected_without_normalization() {
        let file = encode(&[0x23; 12]).unwrap();
        for end in 0..file.len() {
            assert!(decode(&file[..end]).is_err(), "prefix {end}");
        }
        for at in [
            0, 4, 8, 12, 16, 20, 21, 22, 26, 27, 28, 29, 30, 37, 38, 42, 46, 50,
        ] {
            let mut damaged = file.clone();
            damaged[at] ^= 1;
            assert!(decode(&damaged).is_err(), "header byte {at}");
        }
        assert!(encode(&[0; 5]).is_err());
        assert!(matches!(
            encode(&vec![0; TAKE_BYTES_CEILING as usize + 1]),
            Err(TakeError::CapExceeded)
        ));
        assert!(matches!(
            decode(&vec![0; TAKE_BYTES_CEILING as usize + HEADER_BYTES + 1]),
            Err(TakeError::CapExceeded)
        ));
    }
}
