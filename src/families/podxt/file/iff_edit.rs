use super::*;

pub(super) fn iff_chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + data.len() + (data.len() & 1));
    bytes.extend_from_slice(tag);
    bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
    bytes.extend_from_slice(data);
    if data.len() & 1 != 0 {
        bytes.push(0);
    }
    bytes
}

pub(super) fn splice_iff(
    bytes: &mut Vec<u8>,
    range: Range<usize>,
    replacement: Vec<u8>,
    enclosing_sizes: &[usize],
) -> Result<(), ToneError> {
    let increase = replacement.len() as i64 - range.len() as i64;
    let length = bytes.len() as i64 + increase;
    if !(0..=TONE_BYTES_CEILING as i64).contains(&length) {
        return Err(ToneError::format(
            "tone would exceed the offline size limit",
        ));
    }
    for &offset in enclosing_sizes {
        let new_len = (i64::from(preset::word(bytes, offset)) + increase) as u32;
        bytes[offset..offset + 4].copy_from_slice(&new_len.to_be_bytes());
    }
    bytes.splice(range, replacement);
    Ok(())
}
