//! Private synthetic sender descriptions. No generated buffer is exposed through
//! the public model API and there is no transport to forward it to hardware.

use super::{ModelError, Stage};

fn low_nibbles(value: u32) -> [u8; 6] {
    std::array::from_fn(|index| ((value >> (index * 4)) & 15) as u8)
}

pub(super) fn variax(stage: Stage, payload: &[u8]) -> Result<Vec<Vec<u8>>, ModelError> {
    let mut header = crate::HEADER.to_vec();
    header.push(stage.command());
    let mut output = Vec::new();
    match stage {
        Stage::MainCode => {
            if payload.len() > super::receiver::MAX_MAIN_WORDS * 3 {
                return Err(ModelError::Limit);
            }
            let words: Vec<_> = payload
                .as_chunks::<3>()
                .0
                .iter()
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], 0]))
                .collect();
            header.extend(low_nibbles(words.len() as u32 + 1));
            output.push(header);
            for chunk in words.chunks(32) {
                output.push(chunk.iter().flat_map(|&word| low_nibbles(word)).collect());
            }
            let sum = words.iter().fold(0u32, |sum, &word| sum.wrapping_add(word)) & 0xff_ffff;
            output.push(low_nibbles(sum).to_vec());
        }
        Stage::BootRegion => {
            if payload.len() > 16_384 {
                return Err(ModelError::Limit);
            }
            header.push(2);
            output.push(header);
            for chunk in payload.chunks(96) {
                output.push(
                    chunk
                        .iter()
                        .flat_map(|&byte| [byte >> 4, byte & 15])
                        .collect(),
                );
            }
            let sum = payload
                .iter()
                .fold(0u8, |sum, &byte| sum.wrapping_add(byte));
            output.push(vec![sum >> 4, sum & 15]);
        }
        Stage::FactoryTemplates => {
            if payload.len() != 50 * 2619 {
                return Err(ModelError::Limit);
            }
            output.push(header);
            for template in payload.as_chunks::<2619>().0 {
                let encoded: Vec<_> = template.iter().flat_map(|&b| [b & 15, b >> 4]).collect();
                output.extend(encoded.chunks(192).map(<[u8]>::to_vec));
            }
        }
        Stage::UserPatches => {
            if payload.len() != 60 * 4096 {
                return Err(ModelError::Limit);
            }
            output.push(header);
            for (group, slots) in payload.as_chunks::<{ 20 * 4096 }>().0.iter().enumerate() {
                for slot in slots.as_chunks::<4096>().0 {
                    let encoded: Vec<_> = slot[..3276]
                        .iter()
                        .flat_map(|&b| [b & 15, b >> 4])
                        .collect();
                    output.extend(encoded.chunks(192).map(<[u8]>::to_vec));
                }
                let mut trailer = [255u8; 16];
                trailer[13] = 0x45;
                trailer[15] = group as u8;
                output.push(trailer.iter().flat_map(|&b| [b & 15, b >> 4]).collect());
            }
        }
        Stage::EraseUser | Stage::EraseFactory => {
            if !payload.is_empty() {
                return Err(ModelError::ExtraData);
            }
            output.push(header);
        }
    }
    output.push(vec![0xf7]);
    Ok(output)
}

pub(super) fn pod(payload: &[u8]) -> Result<Vec<Vec<u8>>, ModelError> {
    if payload.is_empty() || payload.len() > 65_536 || !payload.len().is_multiple_of(2) {
        return Err(ModelError::Limit);
    }
    if payload.iter().all(|&byte| byte == 255) {
        return Ok(Vec::new());
    }
    let mut frames = Vec::new();
    for chunk in payload.chunks(128) {
        let mut frame = vec![0xf0, 0, 1, 12, 3, 0x6d, (chunk.len() / 2) as u8];
        for word in chunk.as_chunks::<2>().0 {
            let value = u16::from_be_bytes([word[0], word[1]]);
            frame.extend([
                (value >> 10) as u8,
                ((value >> 4) & 63) as u8,
                (value & 15) as u8,
            ]);
        }
        frame.push(0xf7);
        frames.push(frame);
    }
    Ok(frames)
}
