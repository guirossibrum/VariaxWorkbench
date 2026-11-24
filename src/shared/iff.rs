//! IFF read primitives shared by Workbench and PODxt file codecs.
use std::ops::Range;

#[derive(Clone)]
pub(crate) struct Chunk {
    pub(crate) id: [u8; 4],
    pub(crate) data: Range<usize>,
}

pub(crate) fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().expect("validated range"))
}

/// Parse only the children of a validated range, with word-aligned leaf chunks.
pub(crate) fn chunks(bytes: &[u8], region: Range<usize>) -> Result<Vec<Chunk>, String> {
    let mut cursor = region.start;
    let mut result = Vec::new();
    while cursor < region.end {
        if region.end - cursor < 8 {
            return Err(format!("truncated chunk header at {cursor}"));
        }
        let length = word(bytes, cursor + 4) as usize;
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|&n| n <= region.end)
            .ok_or_else(|| format!("chunk at {cursor} exceeds its container"))?;
        let id = bytes[cursor..cursor + 4].try_into().unwrap();
        if (id == *b"LIST" || id == *b"FORM") && length < 4 {
            return Err("container lacks its type code".into());
        }
        result.push(Chunk {
            id,
            data: start..end,
        });
        cursor = end
            .checked_add(length & 1)
            .filter(|&n| n <= region.end)
            .ok_or("missing odd-length chunk padding")?;
    }
    Ok(result)
}

pub(crate) fn unique<'a>(items: &'a [Chunk], id: &[u8; 4]) -> Result<&'a Chunk, String> {
    let mut found = items.iter().filter(|c| &c.id == id);
    let result = found
        .next()
        .ok_or_else(|| format!("missing {} chunk", String::from_utf8_lossy(id)))?;
    if found.next().is_some() {
        return Err(format!(
            "ambiguous duplicate {} chunks",
            String::from_utf8_lossy(id)
        ));
    }
    Ok(result)
}

pub(crate) fn container(bytes: &[u8], chunk: &Chunk, kind: &[u8; 4]) -> bool {
    (chunk.id == *b"FORM" || chunk.id == *b"LIST")
        && &bytes[chunk.data.start..chunk.data.start + 4] == kind
}
