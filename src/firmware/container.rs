//! Bounded FORM/L6FF parser.
//!
//! Chunk rules follow `scripts/inspect_firmware.py`: big-endian sizes, a pad
//! byte after an odd payload, one 52-byte `HEAD`, one zlib `data` chunk, and
//! `dinf` regions that begin at offset zero and cover the image without gaps
//! or overlap. This parser also rejects empty regions and containers above
//! [`MAX_CONTAINER_BYTES`].

use super::applicability::{Applicability, ApplicabilityError, classify};
use std::fmt;

pub const FORMAT_VERSION: u32 = 0x0001_0000;
pub const MAX_IMAGE_BYTES: u32 = 16 * 1024 * 1024;
pub const MAX_CONTAINER_BYTES: usize = MAX_IMAGE_BYTES as usize + 64 * 1024;
const COMPRESSION_ZLIB: u32 = 1;
const HEAD_LEN: usize = 52;

pub const INTEGRITY_STATEMENT: &str = "Embedded MD5 agreement checks the uncompressed image against the digest stored in HEAD. It is not a signature and does not authenticate the publisher, the product, or the file's origin.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub type_raw: u32,
    pub version: u32,
    pub offset: u32,
    pub len: u32,
    pub sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub source_len: usize,
    pub source_sha256: [u8; 32],
    pub format_version: u32,
    pub product_id: u32,
    pub version: u32,
    pub image_len: usize,
    pub image_md5: [u8; 16],
    pub image_sha256: [u8; 32],
    /// True only on a successful inspection. A mismatch is [`InspectError::Md5Mismatch`].
    pub header_md5_matches: bool,
    /// Sixteen bytes after the HEAD digest. Archived images store zeros here.
    /// The meaning is unknown, so non-zero values are preserved and accepted.
    pub reserved_tail: [u8; 16],
    pub regions: Vec<Region>,
    pub applicability: Applicability,
    pub integrity_is_not_authentication: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Inspection {
    pub report: Report,
    image: Vec<u8>,
}

impl fmt::Debug for Inspection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inspection")
            .field("report", &self.report)
            .field("image_len", &self.image.len())
            .finish()
    }
}

impl Inspection {
    pub fn image(&self) -> &[u8] {
        &self.image
    }

    /// Require the declared product id and version word.
    ///
    /// Success means those two header fields match the caller's target. It does
    /// not compare the image with an archived file and it does not authenticate
    /// the container.
    pub fn require_target(&self, product_id: u32, version: u32) -> Result<(), ApplicabilityError> {
        if self.report.product_id != product_id {
            return Err(ApplicabilityError::WrongProduct {
                declared: self.report.product_id,
                expected: product_id,
            });
        }
        if self.report.version != version {
            return Err(ApplicabilityError::WrongVersion {
                declared: self.report.version,
                expected: version,
            });
        }
        Ok(())
    }

    pub fn to_text(&self) -> String {
        let report = &self.report;
        let label = super::applicability::observed_version_label(report.version)
            .unwrap_or_else(|| "unavailable".to_owned());
        let mut out = String::new();
        out.push_str("container: FORM/L6FF\n");
        out.push_str(&format!("source_bytes: {}\n", report.source_len));
        out.push_str(&format!("source_sha256: {}\n", hex(&report.source_sha256)));
        out.push_str(&format!("product_id: {:08x}\n", report.product_id));
        out.push_str(&format!("version: {:08x}\n", report.version));
        out.push_str(&format!(
            "version_label: {label} (top two bytes as hex when the low 16 bits are clear; observed for archived 3.01 and 3.10)\n"
        ));
        out.push_str(&format!("applicability: {}\n", report.applicability));
        out.push_str("compression: zlib\n");
        out.push_str(&format!("image_bytes: {}\n", report.image_len));
        out.push_str(&format!("image_md5: {}\n", hex(&report.image_md5)));
        out.push_str(&format!("image_sha256: {}\n", hex(&report.image_sha256)));
        out.push_str(&format!(
            "header_md5_matches: {}\n",
            report.header_md5_matches
        ));
        out.push_str(&format!(
            "integrity_is_not_authentication: {}\n",
            report.integrity_is_not_authentication
        ));
        out.push_str(&format!("{INTEGRITY_STATEMENT}\n"));
        out.push_str(&format!("reserved_tail: {}\n", hex(&report.reserved_tail)));
        out.push_str(&format!("regions: {}\n", report.regions.len()));
        for (index, region) in report.regions.iter().enumerate() {
            out.push_str(&format!(
                "region\t{index}\ttype_raw={}\tversion={:08x}\toffset={}\tbytes={}\tsha256={}\n",
                region.type_raw,
                region.version,
                region.offset,
                region.len,
                hex(&region.sha256)
            ));
        }
        out
    }

    pub fn to_json(&self) -> String {
        let report = &self.report;
        let label = super::applicability::observed_version_label(report.version)
            .map(|value| json_string(&value))
            .unwrap_or_else(|| "null".to_owned());
        let catalog_name = report
            .applicability
            .catalog_name()
            .map(json_string)
            .unwrap_or_else(|| "null".to_owned());
        let recorded = report
            .applicability
            .recorded_version()
            .map(|value| format!("\"{value:08x}\""))
            .unwrap_or_else(|| "null".to_owned());
        let mut regions = String::from("[\n");
        for (index, region) in report.regions.iter().enumerate() {
            if index > 0 {
                regions.push_str(",\n");
            }
            regions.push_str(&format!(
                "    {{\"type_raw\": {}, \"version\": \"{:08x}\", \"offset\": {}, \"bytes\": {}, \"sha256\": \"{}\"}}",
                region.type_raw,
                region.version,
                region.offset,
                region.len,
                hex(&region.sha256)
            ));
        }
        regions.push_str("\n  ]");
        format!(
            "{{\n  \"container\": \"FORM/L6FF\",\n  \"source_bytes\": {},\n  \"source_sha256\": \"{}\",\n  \"product_id\": \"{:08x}\",\n  \"version\": \"{:08x}\",\n  \"version_label\": {label},\n  \"applicability\": \"{}\",\n  \"applicability_detail\": {},\n  \"catalog_name\": {catalog_name},\n  \"recorded_version\": {recorded},\n  \"compression\": \"zlib\",\n  \"image_bytes\": {},\n  \"image_md5\": \"{}\",\n  \"image_sha256\": \"{}\",\n  \"header_md5_matches\": {},\n  \"integrity_is_not_authentication\": {},\n  \"integrity_statement\": {},\n  \"reserved_tail\": \"{}\",\n  \"regions\": {regions}\n}}\n",
            report.source_len,
            hex(&report.source_sha256),
            report.product_id,
            report.version,
            report.applicability.tag(),
            json_string(&report.applicability.to_string()),
            report.image_len,
            hex(&report.image_md5),
            hex(&report.image_sha256),
            report.header_md5_matches,
            report.integrity_is_not_authentication,
            json_string(INTEGRITY_STATEMENT),
            hex(&report.reserved_tail),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectError {
    ExpectedFormL6ff,
    FormLengthMismatch {
        declared: usize,
        actual: usize,
    },
    ContainerTooLarge {
        len: usize,
        limit: usize,
    },
    TruncatedChunkHeader {
        offset: usize,
    },
    ChunkExceedsFile {
        tag: [u8; 4],
        offset: usize,
    },
    UnsupportedChunk {
        tag: [u8; 4],
    },
    HeadCount(usize),
    HeadLength(usize),
    DataCount(usize),
    UnsupportedContainer {
        format_version: u32,
        compression: u32,
    },
    ImageLength(u32),
    Decompress(String),
    StreamLength {
        produced: usize,
        declared: usize,
    },
    StreamNotTerminated,
    UnusedCompressedTail {
        unused: usize,
    },
    Md5Mismatch {
        declared: [u8; 16],
        actual: [u8; 16],
    },
    RegionPayloadLength {
        index: usize,
        len: usize,
    },
    EmptyRegion {
        index: usize,
    },
    OverlappingRegion {
        index: usize,
        offset: u32,
        expected: u32,
    },
    RegionGap {
        index: usize,
        offset: u32,
        expected: u32,
    },
    RegionOutOfRange {
        index: usize,
        offset: u32,
        size: u32,
        image_len: usize,
    },
    RegionExtentOverflow {
        index: usize,
    },
    IncompleteCoverage {
        covered: u32,
        image_len: usize,
    },
    NoRegions,
}

impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpectedFormL6ff => write!(f, "expected FORM/L6FF"),
            Self::FormLengthMismatch { declared, actual } => write!(
                f,
                "FORM length mismatch: declared {declared}, file payload {actual}"
            ),
            Self::ContainerTooLarge { len, limit } => {
                write!(f, "container is {len} bytes and the limit is {limit}")
            }
            Self::TruncatedChunkHeader { offset } => {
                write!(f, "truncated chunk header at offset {offset}")
            }
            Self::ChunkExceedsFile { tag, offset } => write!(
                f,
                "chunk {} at offset {offset} exceeds the file",
                tag_label(*tag)
            ),
            Self::UnsupportedChunk { tag } => {
                write!(f, "unsupported chunk {}", tag_label(*tag))
            }
            Self::HeadCount(count) => write!(f, "expected one HEAD chunk, found {count}"),
            Self::HeadLength(len) => write!(f, "expected a 52-byte HEAD, found {len} bytes"),
            Self::DataCount(count) => write!(f, "expected one data chunk, found {count}"),
            Self::UnsupportedContainer {
                format_version,
                compression,
            } => write!(
                f,
                "unsupported format {format_version:08x} or compression {compression}"
            ),
            Self::ImageLength(len) => {
                write!(f, "image length {len} is outside 1..= {MAX_IMAGE_BYTES}")
            }
            Self::Decompress(message) => write!(f, "zlib decompress failed: {message}"),
            Self::StreamLength { produced, declared } => write!(
                f,
                "decompressed {produced} bytes and HEAD declared {declared}"
            ),
            Self::StreamNotTerminated => write!(
                f,
                "zlib stream did not end inside the declared image length"
            ),
            Self::UnusedCompressedTail { unused } => {
                write!(
                    f,
                    "zlib stream left {unused} unused bytes in the data chunk"
                )
            }
            Self::Md5Mismatch { declared, actual } => write!(
                f,
                "uncompressed image MD5 mismatch: HEAD {} actual {}",
                hex(declared),
                hex(actual)
            ),
            Self::RegionPayloadLength { index, len } => {
                write!(f, "region {index} information is {len} bytes; expected 16")
            }
            Self::EmptyRegion { index } => write!(f, "region {index} has zero length"),
            Self::OverlappingRegion {
                index,
                offset,
                expected,
            } => write!(
                f,
                "region {index} overlaps at offset {offset}; expected {expected}"
            ),
            Self::RegionGap {
                index,
                offset,
                expected,
            } => write!(
                f,
                "region {index} leaves a gap at offset {offset}; expected {expected}"
            ),
            Self::RegionOutOfRange {
                index,
                offset,
                size,
                image_len,
            } => write!(
                f,
                "region {index} offset {offset} size {size} exceeds the {image_len}-byte image"
            ),
            Self::RegionExtentOverflow { index } => {
                write!(f, "region {index} offset and size overflow")
            }
            Self::IncompleteCoverage { covered, image_len } => write!(
                f,
                "regions cover {covered} bytes of the {image_len}-byte image"
            ),
            Self::NoRegions => write!(f, "container has no region information"),
        }
    }
}

impl std::error::Error for InspectError {}

struct RawChunk<'a> {
    tag: [u8; 4],
    payload: &'a [u8],
}

/// Inspect one FORM/L6FF container.
///
/// The returned image is the decompressed payload for offline inspection. This
/// function does not write it anywhere.
pub fn inspect(data: &[u8]) -> Result<Inspection, InspectError> {
    if data.len() > MAX_CONTAINER_BYTES {
        return Err(InspectError::ContainerTooLarge {
            len: data.len(),
            limit: MAX_CONTAINER_BYTES,
        });
    }
    if data.len() < 12 || &data[..4] != b"FORM" || &data[8..12] != b"L6FF" {
        return Err(InspectError::ExpectedFormL6ff);
    }
    let declared = u32::from_be_bytes(data[4..8].try_into().expect("FORM length")) as usize;
    let actual = data.len() - 8;
    if declared != actual {
        return Err(InspectError::FormLengthMismatch { declared, actual });
    }
    let chunks = parse_chunks(data)?;
    let heads: Vec<_> = chunks
        .iter()
        .filter(|chunk| &chunk.tag == b"HEAD")
        .collect();
    let compressed: Vec<_> = chunks
        .iter()
        .filter(|chunk| &chunk.tag == b"data")
        .collect();
    if heads.len() != 1 {
        return Err(InspectError::HeadCount(heads.len()));
    }
    if heads[0].payload.len() != HEAD_LEN {
        return Err(InspectError::HeadLength(heads[0].payload.len()));
    }
    if compressed.len() != 1 {
        return Err(InspectError::DataCount(compressed.len()));
    }
    let header = heads[0].payload;
    let format_version = be_u32(header, 0);
    let version = be_u32(header, 4);
    let product_id = be_u32(header, 8);
    let compression = be_u32(header, 12);
    let image_len = be_u32(header, 16);
    if format_version != FORMAT_VERSION || compression != COMPRESSION_ZLIB {
        return Err(InspectError::UnsupportedContainer {
            format_version,
            compression,
        });
    }
    if image_len == 0 || image_len > MAX_IMAGE_BYTES {
        return Err(InspectError::ImageLength(image_len));
    }
    let mut header_md5 = [0u8; 16];
    header_md5.copy_from_slice(&header[20..36]);
    let mut reserved_tail = [0u8; 16];
    reserved_tail.copy_from_slice(&header[36..52]);
    let image = decompress_exact(compressed[0].payload, image_len as usize)?;
    let actual_md5 = md5_bytes(&image);
    if actual_md5 != header_md5 {
        return Err(InspectError::Md5Mismatch {
            declared: header_md5,
            actual: actual_md5,
        });
    }
    let regions = regions(&chunks, &image)?;
    Ok(Inspection {
        report: Report {
            source_len: data.len(),
            source_sha256: sha256_bytes(data),
            format_version,
            product_id,
            version,
            image_len: image.len(),
            image_md5: actual_md5,
            image_sha256: sha256_bytes(&image),
            header_md5_matches: true,
            reserved_tail,
            regions,
            applicability: classify(product_id, version),
            integrity_is_not_authentication: true,
        },
        image,
    })
}

fn parse_chunks(data: &[u8]) -> Result<Vec<RawChunk<'_>>, InspectError> {
    let mut cursor = 12;
    let mut chunks = Vec::new();
    while cursor < data.len() {
        if data.len() - cursor < 8 {
            return Err(InspectError::TruncatedChunkHeader { offset: cursor });
        }
        let tag: [u8; 4] = data[cursor..cursor + 4].try_into().expect("tag");
        let size = be_u32(data, cursor + 4) as usize;
        let Some(payload_end) = cursor
            .checked_add(8)
            .and_then(|start| start.checked_add(size))
        else {
            return Err(InspectError::ChunkExceedsFile {
                tag,
                offset: cursor,
            });
        };
        let Some(next) = payload_end.checked_add(size % 2) else {
            return Err(InspectError::ChunkExceedsFile {
                tag,
                offset: cursor,
            });
        };
        if next > data.len() {
            return Err(InspectError::ChunkExceedsFile {
                tag,
                offset: cursor,
            });
        }
        if !matches!(&tag, b"HEAD" | b"dinf" | b"data") {
            return Err(InspectError::UnsupportedChunk { tag });
        }
        chunks.push(RawChunk {
            tag,
            payload: &data[cursor + 8..payload_end],
        });
        cursor = next;
    }
    Ok(chunks)
}

/// Decompress at most `length + 1` bytes so an overlong stream cannot expand
/// without bound. Success requires a finished zlib member, no unused input, and
/// an output length equal to the HEAD declaration.
fn decompress_exact(compressed: &[u8], length: usize) -> Result<Vec<u8>, InspectError> {
    use flate2::{Decompress, FlushDecompress, Status};
    let mut decoder = Decompress::new(true);
    let mut output = vec![0u8; length + 1];
    let mut input_pos = 0usize;
    let mut output_pos = 0usize;
    let spin_limit = compressed
        .len()
        .saturating_add(output.len())
        .saturating_add(2);
    for _ in 0..spin_limit {
        let before_in = decoder.total_in();
        let before_out = decoder.total_out();
        let status = decoder
            .decompress(
                &compressed[input_pos..],
                &mut output[output_pos..],
                FlushDecompress::Finish,
            )
            .map_err(|error| InspectError::Decompress(error.to_string()))?;
        let consumed = usize::try_from(decoder.total_in() - before_in)
            .map_err(|_| InspectError::StreamNotTerminated)?;
        let produced = usize::try_from(decoder.total_out() - before_out)
            .map_err(|_| InspectError::StreamNotTerminated)?;
        input_pos += consumed;
        output_pos += produced;
        if output_pos > length {
            return Err(InspectError::StreamLength {
                produced: output_pos,
                declared: length,
            });
        }
        match status {
            Status::StreamEnd => {
                if output_pos != length {
                    return Err(InspectError::StreamLength {
                        produced: output_pos,
                        declared: length,
                    });
                }
                if input_pos != compressed.len() {
                    return Err(InspectError::UnusedCompressedTail {
                        unused: compressed.len() - input_pos,
                    });
                }
                output.truncate(length);
                return Ok(output);
            }
            Status::Ok | Status::BufError => {
                if consumed == 0 && produced == 0 {
                    return Err(InspectError::StreamNotTerminated);
                }
            }
        }
    }
    Err(InspectError::StreamNotTerminated)
}

fn regions(chunks: &[RawChunk<'_>], image: &[u8]) -> Result<Vec<Region>, InspectError> {
    let mut found = Vec::new();
    let mut expected = 0u32;
    for chunk in chunks {
        if &chunk.tag != b"dinf" {
            continue;
        }
        let index = found.len();
        if chunk.payload.len() != 16 {
            return Err(InspectError::RegionPayloadLength {
                index,
                len: chunk.payload.len(),
            });
        }
        let type_raw = be_u32(chunk.payload, 0);
        let version = be_u32(chunk.payload, 4);
        let offset = be_u32(chunk.payload, 8);
        let size = be_u32(chunk.payload, 12);
        if size == 0 {
            return Err(InspectError::EmptyRegion { index });
        }
        if offset < expected {
            return Err(InspectError::OverlappingRegion {
                index,
                offset,
                expected,
            });
        }
        if offset > expected {
            return Err(InspectError::RegionGap {
                index,
                offset,
                expected,
            });
        }
        let Some(end) = offset.checked_add(size) else {
            return Err(InspectError::RegionExtentOverflow { index });
        };
        if end as usize > image.len() {
            return Err(InspectError::RegionOutOfRange {
                index,
                offset,
                size,
                image_len: image.len(),
            });
        }
        let raw = &image[offset as usize..end as usize];
        found.push(Region {
            type_raw,
            version,
            offset,
            len: size,
            sha256: sha256_bytes(raw),
        });
        expected = end;
    }
    if found.is_empty() {
        return Err(InspectError::NoRegions);
    }
    if expected as usize != image.len() {
        return Err(InspectError::IncompleteCoverage {
            covered: expected,
            image_len: image.len(),
        });
    }
    Ok(found)
}

fn be_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(data[offset..offset + 4].try_into().expect("four bytes"))
}

fn md5_bytes(data: &[u8]) -> [u8; 16] {
    let digest = <md5::Md5 as md5::Digest>::digest(data);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let digest = <sha2::Sha256 as sha2::Digest>::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    out
}

fn tag_label(tag: [u8; 4]) -> String {
    let text: String = tag
        .iter()
        .map(|byte| {
            if byte.is_ascii_graphic() {
                *byte as char
            } else {
                '.'
            }
        })
        .collect();
    format!("{text} {}", hex(&tag))
}

fn json_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped.push('"');
    escaped
}
