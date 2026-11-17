//! Bounded, offline structural inspection of Workbench HD `FORM/WB2A` assets.
//!
//! This parser reports stored metadata and source byte ranges only. It does not
//! decrypt, decompress, CRC-check, validate model applicability, or expose an
//! editor. The WB2A reader uses unpadded IFF chunks; that is distinct from the
//! padded L6PA preset parser in [`crate::preset`].

use std::{collections::BTreeMap, fmt, ops::Range};

pub const MAX_ASSET_FILE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 8192;
pub const MAX_NESTING_DEPTH: usize = 16;
pub const MAX_MODEL_DEFINITIONS: usize = 1024;

const MODEL_KEY_OFFSET: usize = 0x10b4;
const MODEL_KEY_END: usize = MODEL_KEY_OFFSET + 4;
const MDAT_HEADER_BYTES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InspectError {
    FileTooLarge { limit: usize },
    ExpectedFormWb2a,
    FormLengthMismatch { declared: usize, actual: usize },
    TruncatedChunkHeader { offset: usize },
    ChunkExceedsParent { offset: usize },
    MissingContainerType { offset: usize },
    NestingLimit { offset: usize, limit: usize },
    ChunkLimit { limit: usize },
    ModelLimit { limit: usize },
    MissingAinf,
    InvalidAinf,
    MissingMinfo { offset: usize },
    InvalidMinfo { offset: usize },
    ShortMdatHeader { offset: usize },
    AllocationLimit,
}

impl fmt::Display for InspectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileTooLarge { limit } => write!(f, "asset exceeds the {limit}-byte limit"),
            Self::ExpectedFormWb2a => f.write_str("expected FORM/WB2A"),
            Self::FormLengthMismatch { declared, actual } => write!(
                f,
                "FORM declares {declared} bytes after its length word, file contains {actual}"
            ),
            Self::TruncatedChunkHeader { offset } => {
                write!(f, "truncated chunk header at byte {offset}")
            }
            Self::ChunkExceedsParent { offset } => {
                write!(f, "chunk at byte {offset} exceeds its parent")
            }
            Self::MissingContainerType { offset } => {
                write!(f, "container at byte {offset} lacks its four-byte type")
            }
            Self::NestingLimit { offset, limit } => {
                write!(
                    f,
                    "container at byte {offset} exceeds nesting limit {limit}"
                )
            }
            Self::ChunkLimit { limit } => write!(f, "asset exceeds {limit} chunks"),
            Self::ModelLimit { limit } => {
                write!(f, "asset exceeds {limit} model definitions")
            }
            Self::MissingAinf => f.write_str("FORM/WB2A must begin with AINF"),
            Self::InvalidAinf => f.write_str("AINF metadata is short or lacks a nul terminator"),
            Self::MissingMinfo { offset } => {
                write!(f, "LIST/MDEF at byte {offset} must begin with MINFO")
            }
            Self::InvalidMinfo { offset } => write!(
                f,
                "MINF (MINFO) at byte {offset} is short or lacks a nul terminator"
            ),
            Self::ShortMdatHeader { offset } => {
                write!(
                    f,
                    "MDAT at byte {offset} is shorter than its 16-byte metadata header"
                )
            }
            Self::AllocationLimit => f.write_str("could not allocate bounded parser metadata"),
        }
    }
}

impl std::error::Error for InspectError {}

/// A source span for one IFF chunk. Container payload ranges include their
/// four-byte type code; child spans point into the original caller-owned bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkInfo {
    pub tag: [u8; 4],
    pub type_code: Option<[u8; 4]>,
    pub header_offset: usize,
    pub payload: Range<usize>,
    pub children: Vec<ChunkInfo>,
}

impl ChunkInfo {
    pub fn is_container(&self, tag: &[u8; 4], type_code: &[u8; 4]) -> bool {
        &self.tag == tag && self.type_code.as_ref() == Some(type_code)
    }
}

/// Raw AINF fields consumed by the native loader. `unknown_word` is retained
/// without assigning meaning because the observed loader does not use it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredAssetInfo {
    pub unknown_word: u32,
    pub asset_id: u32,
    pub asset_pbn: u32,
    pub asset_version: u32,
    pub name_bytes: Range<usize>,
    pub trailing_bytes: Range<usize>,
}

/// The MINFO identifier and raw nul-terminated name span stored in one MDEF.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredModelInfo {
    pub model_id: AssetModelId,
    pub name_bytes: Range<usize>,
    pub trailing_bytes: Range<usize>,
}

/// MDAT header fields and an opaque source range for its stored data.
/// Presence here means only that the structural header was readable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredModelDataInfo {
    pub decoded_size: u32,
    pub stored_size: u32,
    pub encryption_flag: u8,
    pub compression_flag: u8,
    pub unknown_header_bytes: [u8; 2],
    pub declared_crc: u32,
    pub payload_bytes: Range<usize>,
    pub stored_size_matches_payload: bool,
}

/// A structurally parsed MDEF record. Other child chunks remain available as
/// ranges in [`WorkbenchAssetInspection::root`] and are not interpreted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelDefinition {
    pub mdef_offset: usize,
    pub info: StoredModelInfo,
    pub data: Option<StoredModelDataInfo>,
    pub opaque_children: Vec<OpaqueChunkSpan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueChunkSpan {
    pub tag: [u8; 4],
    pub type_code: Option<[u8; 4]>,
    pub header_offset: usize,
    pub payload: Range<usize>,
}

/// Successful parsing establishes only that checked WB2A chunk structure and
/// the observed AINF/MINFO/MDAT metadata shapes are present. It is not a decoded
/// or validated model and says nothing about physical device applicability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkbenchAssetInspection {
    pub source_len: usize,
    pub root: ChunkInfo,
    pub asset_info: StoredAssetInfo,
    pub models: Vec<ModelDefinition>,
}

impl WorkbenchAssetInspection {
    pub fn parse(bytes: &[u8]) -> Result<Self, InspectError> {
        if bytes.len() > MAX_ASSET_FILE_BYTES {
            return Err(InspectError::FileTooLarge {
                limit: MAX_ASSET_FILE_BYTES,
            });
        }
        if bytes.len() < 12 || bytes.get(0..4) != Some(b"FORM") || bytes.get(8..12) != Some(b"WB2A")
        {
            return Err(InspectError::ExpectedFormWb2a);
        }
        let declared = read_be_u32(bytes, 4).ok_or(InspectError::ExpectedFormWb2a)? as usize;
        let actual = bytes.len() - 8;
        if declared != actual {
            return Err(InspectError::FormLengthMismatch { declared, actual });
        }

        let mut parsed_chunks = 0;
        let children = parse_children(bytes, 12..bytes.len(), 1, &mut parsed_chunks)?;
        let root = ChunkInfo {
            tag: *b"FORM",
            type_code: Some(*b"WB2A"),
            header_offset: 0,
            payload: 8..bytes.len(),
            children,
        };

        let ainf = root
            .children
            .first()
            .filter(|chunk| chunk.tag == *b"AINF")
            .ok_or(InspectError::MissingAinf)?;
        let asset_info = parse_asset_info(bytes, ainf)?;

        let mut models = Vec::new();
        for chunk in root
            .children
            .iter()
            .filter(|chunk| chunk.is_container(b"LIST", b"MDEF"))
        {
            if models.len() == MAX_MODEL_DEFINITIONS {
                return Err(InspectError::ModelLimit {
                    limit: MAX_MODEL_DEFINITIONS,
                });
            }
            models
                .try_reserve(1)
                .map_err(|_| InspectError::AllocationLimit)?;
            models.push(parse_model_definition(bytes, chunk)?);
        }

        Ok(Self {
            source_len: bytes.len(),
            root,
            asset_info,
            models,
        })
    }

    /// Concise structural summary. Stored MDAT fields are explicitly not
    /// described as decoded, integrity-checked, or device-applicable.
    pub fn summary(&self) -> String {
        let mut out = format!(
            "container: FORM/WB2A\nsource_bytes: {}\nasset_id: {:08x}\nasset_pbn: {:08x}\nasset_version: {:08x}\nmodel_definitions: {}\n",
            self.source_len,
            self.asset_info.asset_id,
            self.asset_info.asset_pbn,
            self.asset_info.asset_version,
            self.models.len()
        );
        for (index, model) in self.models.iter().enumerate() {
            out.push_str(&format!(
                "model[{index}]: id={:08x} mdef_offset={} name_bytes={} ",
                model.info.model_id.0,
                model.mdef_offset,
                model.info.name_bytes.len()
            ));
            if let Some(data) = &model.data {
                out.push_str(&format!(
                    "mdat=stored(decoded_size={},stored_size={},payload_bytes={},encryption_flag={},compression_flag={},crc={:08x})\n",
                    data.decoded_size,
                    data.stored_size,
                    data.payload_bytes.len(),
                    data.encryption_flag,
                    data.compression_flag,
                    data.declared_crc
                ));
            } else {
                out.push_str("mdat=absent\n");
            }
        }
        out.push_str("interpretation: structural metadata only; payloads are opaque and physical applicability is unknown\n");
        out
    }
}

fn parse_asset_info(bytes: &[u8], chunk: &ChunkInfo) -> Result<StoredAssetInfo, InspectError> {
    let payload = &bytes[chunk.payload.clone()];
    if payload.len() < 17 {
        return Err(InspectError::InvalidAinf);
    }
    let name_start = chunk.payload.start + 16;
    let name_end = bytes[name_start..chunk.payload.end]
        .iter()
        .position(|&byte| byte == 0)
        .map(|relative| name_start + relative)
        .ok_or(InspectError::InvalidAinf)?;
    Ok(StoredAssetInfo {
        unknown_word: read_be_u32(bytes, chunk.payload.start).ok_or(InspectError::InvalidAinf)?,
        asset_id: read_be_u32(bytes, chunk.payload.start + 4).ok_or(InspectError::InvalidAinf)?,
        asset_pbn: read_be_u32(bytes, chunk.payload.start + 8).ok_or(InspectError::InvalidAinf)?,
        asset_version: read_be_u32(bytes, chunk.payload.start + 12)
            .ok_or(InspectError::InvalidAinf)?,
        name_bytes: name_start..name_end,
        trailing_bytes: name_end + 1..chunk.payload.end,
    })
}

fn parse_model_definition(bytes: &[u8], mdef: &ChunkInfo) -> Result<ModelDefinition, InspectError> {
    let info_chunk = mdef
        .children
        .first()
        .filter(|chunk| chunk.tag == *b"MINF")
        .ok_or(InspectError::MissingMinfo {
            offset: mdef.header_offset,
        })?;
    let info_payload = &bytes[info_chunk.payload.clone()];
    if info_payload.len() < 5 {
        return Err(InspectError::InvalidMinfo {
            offset: info_chunk.header_offset,
        });
    }
    let name_start = info_chunk.payload.start + 4;
    let name_end = bytes[name_start..info_chunk.payload.end]
        .iter()
        .position(|&byte| byte == 0)
        .map(|relative| name_start + relative)
        .ok_or(InspectError::InvalidMinfo {
            offset: info_chunk.header_offset,
        })?;
    let info = StoredModelInfo {
        model_id: AssetModelId(read_be_u32(bytes, info_chunk.payload.start).ok_or(
            InspectError::InvalidMinfo {
                offset: info_chunk.header_offset,
            },
        )?),
        name_bytes: name_start..name_end,
        trailing_bytes: name_end + 1..info_chunk.payload.end,
    };

    let data_chunk_index = 1;
    let data = mdef
        .children
        .get(data_chunk_index)
        .filter(|chunk| chunk.tag == *b"MDAT")
        .map(|chunk| parse_model_data(bytes, chunk))
        .transpose()?;
    let mut opaque_children = Vec::new();
    for (index, chunk) in mdef.children.iter().enumerate() {
        if index == 0 || (index == data_chunk_index && data.is_some()) {
            continue;
        }
        opaque_children
            .try_reserve(1)
            .map_err(|_| InspectError::AllocationLimit)?;
        opaque_children.push(OpaqueChunkSpan {
            tag: chunk.tag,
            type_code: chunk.type_code,
            header_offset: chunk.header_offset,
            payload: chunk.payload.clone(),
        });
    }

    Ok(ModelDefinition {
        mdef_offset: mdef.header_offset,
        info,
        data,
        opaque_children,
    })
}

fn parse_model_data(bytes: &[u8], chunk: &ChunkInfo) -> Result<StoredModelDataInfo, InspectError> {
    if chunk.payload.len() < MDAT_HEADER_BYTES {
        return Err(InspectError::ShortMdatHeader {
            offset: chunk.header_offset,
        });
    }
    let start = chunk.payload.start;
    let payload_bytes = start + MDAT_HEADER_BYTES..chunk.payload.end;
    Ok(StoredModelDataInfo {
        decoded_size: read_be_u32(bytes, start).ok_or(InspectError::ShortMdatHeader {
            offset: chunk.header_offset,
        })?,
        stored_size: read_be_u32(bytes, start + 4).ok_or(InspectError::ShortMdatHeader {
            offset: chunk.header_offset,
        })?,
        encryption_flag: bytes[start + 8],
        compression_flag: bytes[start + 9],
        unknown_header_bytes: [bytes[start + 10], bytes[start + 11]],
        declared_crc: read_be_u32(bytes, start + 12).ok_or(InspectError::ShortMdatHeader {
            offset: chunk.header_offset,
        })?,
        stored_size_matches_payload: read_be_u32(bytes, start + 4)
            .is_some_and(|size| size as usize == payload_bytes.len()),
        payload_bytes,
    })
}

fn parse_children(
    bytes: &[u8],
    region: Range<usize>,
    depth: usize,
    count: &mut usize,
) -> Result<Vec<ChunkInfo>, InspectError> {
    let mut cursor = region.start;
    let mut children = Vec::new();
    while cursor < region.end {
        if region.end - cursor < 8 {
            return Err(InspectError::TruncatedChunkHeader { offset: cursor });
        }
        *count += 1;
        if *count > MAX_CHUNKS {
            return Err(InspectError::ChunkLimit { limit: MAX_CHUNKS });
        }
        let tag: [u8; 4] = bytes[cursor..cursor + 4]
            .try_into()
            .expect("checked header");
        let length = read_be_u32(bytes, cursor + 4).expect("checked header") as usize;
        let payload_start = cursor
            .checked_add(8)
            .ok_or(InspectError::ChunkExceedsParent { offset: cursor })?;
        let payload_end = payload_start
            .checked_add(length)
            .filter(|&end| end <= region.end)
            .ok_or(InspectError::ChunkExceedsParent { offset: cursor })?;

        let is_container = tag == *b"FORM" || tag == *b"LIST";
        let (type_code, nested) = if is_container {
            if length < 4 {
                return Err(InspectError::MissingContainerType { offset: cursor });
            }
            if depth >= MAX_NESTING_DEPTH {
                return Err(InspectError::NestingLimit {
                    offset: cursor,
                    limit: MAX_NESTING_DEPTH,
                });
            }
            let kind: [u8; 4] = bytes[payload_start..payload_start + 4]
                .try_into()
                .expect("container type length checked");
            let nested = parse_children(bytes, payload_start + 4..payload_end, depth + 1, count)?;
            (Some(kind), nested)
        } else {
            (None, Vec::new())
        };
        children
            .try_reserve(1)
            .map_err(|_| InspectError::AllocationLimit)?;
        children.push(ChunkInfo {
            tag,
            type_code,
            header_offset: cursor,
            payload: payload_start..payload_end,
            children: nested,
        });
        // The static WB2A reader advances by header + declared chunk length;
        // unlike FORM/L6PA this format has no word-alignment padding.
        cursor = payload_end;
    }
    Ok(children)
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_be_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

/// Asset model identifiers from MINFO, distinct from catalog VariaxID keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetModelId(pub u32);

/// Logical model-data key used by the Workbench body/pickup catalog. This is
/// not the physical identity or product ID of a connected instrument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogVariaxId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelCategory {
    Body,
    Pickup,
}

impl AssetModelId {
    pub fn category(self) -> Option<ModelCategory> {
        match self.0 & 0x00ff_0000 {
            0x0010_0000 => Some(ModelCategory::Body),
            0x0011_0000 => Some(ModelCategory::Pickup),
            _ => None,
        }
    }
}

/// Bytes that a caller claims are already decoded model data. This module
/// does not verify decryption, decompression, CRC, or provenance.
#[derive(Clone, Copy, Debug)]
pub struct CallerSuppliedDecodedModel<'a> {
    pub model_id: AssetModelId,
    pub bytes: &'a [u8],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    ModelDataTooShort {
        model_id: AssetModelId,
        actual: usize,
        minimum: usize,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelDataTooShort {
                model_id,
                actual,
                minimum,
            } => write!(
                f,
                "caller-supplied model {:08x} data is {actual} bytes; catalog key requires {minimum}",
                model_id.0
            ),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Logical asset lookup tables built only from caller-supplied decoded bytes.
/// Construction does not establish that those bytes came from a vendor asset,
/// passed integrity checks, or apply to any physical guitar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelCatalog {
    bodies: BTreeMap<CatalogVariaxId, AssetModelId>,
    pickups: BTreeMap<CatalogVariaxId, AssetModelId>,
}

impl ModelCatalog {
    /// Reconstruct the observed native map behavior from already-decoded
    /// synthetic/caller data. Duplicate MINFO IDs retain the first input.
    /// Remaining model IDs are visited in ascending order, matching the native
    /// asset tree traversal; duplicate logical keys therefore retain the last
    /// (highest-ID) model in that order.
    pub fn from_caller_supplied_decoded<'a>(
        records: impl IntoIterator<Item = CallerSuppliedDecodedModel<'a>>,
    ) -> Result<Self, CatalogError> {
        let mut first_by_model_id = BTreeMap::new();
        for record in records {
            first_by_model_id
                .entry(record.model_id)
                .or_insert(record.bytes);
        }

        let mut catalog = Self::default();
        for (model_id, bytes) in first_by_model_id {
            let Some(category) = model_id.category() else {
                continue;
            };
            if bytes.len() < MODEL_KEY_END {
                return Err(CatalogError::ModelDataTooShort {
                    model_id,
                    actual: bytes.len(),
                    minimum: MODEL_KEY_END,
                });
            }
            let key = CatalogVariaxId(u32::from_le_bytes(
                bytes[MODEL_KEY_OFFSET..MODEL_KEY_END]
                    .try_into()
                    .expect("length checked"),
            ));
            let map = match category {
                ModelCategory::Body => &mut catalog.bodies,
                ModelCategory::Pickup => &mut catalog.pickups,
            };
            map.insert(key, model_id);
        }
        Ok(catalog)
    }

    pub fn body_asset(&self, key: CatalogVariaxId) -> Option<AssetModelId> {
        self.bodies.get(&key).copied()
    }

    pub fn pickup_asset(&self, key: CatalogVariaxId) -> Option<AssetModelId> {
        self.pickups.get(&key).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + payload.len());
        out.extend_from_slice(tag);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn container(tag: &[u8; 4], kind: &[u8; 4], children: &[u8]) -> Vec<u8> {
        let mut payload = kind.to_vec();
        payload.extend_from_slice(children);
        chunk(tag, &payload)
    }

    fn mdat(data: &[u8], decoded_size: u32) -> Vec<u8> {
        let mut payload = Vec::with_capacity(MDAT_HEADER_BYTES + data.len());
        payload.extend_from_slice(&decoded_size.to_be_bytes());
        payload.extend_from_slice(&(data.len() as u32).to_be_bytes());
        payload.extend_from_slice(&[0, 0, 0, 0]);
        payload.extend_from_slice(&0x1234_5678u32.to_be_bytes());
        payload.extend_from_slice(data);
        chunk(b"MDAT", &payload)
    }

    fn mdef(model_id: u32, name: &[u8], extra: &[u8]) -> Vec<u8> {
        let mut minfo = model_id.to_be_bytes().to_vec();
        minfo.extend_from_slice(name);
        minfo.push(0);
        minfo.extend_from_slice(b"tail");
        let mut children = chunk(b"MINF", &minfo);
        children.extend_from_slice(&mdat(b"opaque model bytes", 200));
        children.extend_from_slice(&chunk(b"XTRA", extra));
        container(b"LIST", b"MDEF", &children)
    }

    fn asset(children: &[u8]) -> Vec<u8> {
        let mut ainf = Vec::new();
        ainf.extend_from_slice(&0x0102_0304u32.to_be_bytes());
        ainf.extend_from_slice(&0x1112_1314u32.to_be_bytes());
        ainf.extend_from_slice(&0x2122_2324u32.to_be_bytes());
        ainf.extend_from_slice(&0x3132_3334u32.to_be_bytes());
        ainf.extend_from_slice(b"synthetic\0");
        let mut body = chunk(b"AINF", &ainf);
        body.extend_from_slice(children);
        container(b"FORM", b"WB2A", &body)
    }

    fn data_with_key(key: u32) -> Vec<u8> {
        let mut data = vec![0; MODEL_KEY_END];
        data[MODEL_KEY_OFFSET..MODEL_KEY_END].copy_from_slice(&key.to_le_bytes());
        data
    }

    #[test]
    fn parses_source_shaped_synthetic_body_pickup_and_opaque_chunks() {
        let body = mdef(0x0310_0102, b"synthetic body", b"odd");
        let pickup = mdef(0x0311_0103, b"synthetic pickup", b"x");
        let mut children = body;
        children.extend_from_slice(&pickup);
        let bytes = asset(&children);
        let inspected = WorkbenchAssetInspection::parse(&bytes).expect("valid synthetic WB2A");

        assert_eq!(inspected.asset_info.unknown_word, 0x0102_0304);
        assert_eq!(inspected.asset_info.asset_id, 0x1112_1314);
        assert_eq!(inspected.asset_info.asset_pbn, 0x2122_2324);
        assert_eq!(inspected.asset_info.asset_version, 0x3132_3334);
        assert_eq!(
            &bytes[inspected.asset_info.name_bytes.clone()],
            b"synthetic"
        );
        assert_eq!(inspected.models.len(), 2);
        assert_eq!(inspected.models[0].info.model_id, AssetModelId(0x0310_0102));
        assert_eq!(
            &bytes[inspected.models[0].info.name_bytes.clone()],
            b"synthetic body"
        );
        let stored = inspected.models[0].data.as_ref().expect("MDAT metadata");
        assert_eq!(stored.decoded_size, 200);
        assert_eq!(stored.stored_size, b"opaque model bytes".len() as u32);
        assert_eq!(stored.encryption_flag, 0);
        assert_eq!(stored.compression_flag, 0);
        assert_eq!(stored.declared_crc, 0x1234_5678);
        assert!(stored.stored_size_matches_payload);
        assert_ne!(stored.decoded_size, stored.stored_size);
        assert_eq!(&bytes[stored.payload_bytes.clone()], b"opaque model bytes");
        assert_eq!(inspected.models[0].opaque_children[0].tag, *b"XTRA");
        assert_eq!(
            &bytes[inspected.models[0].opaque_children[0].payload.clone()],
            b"odd"
        );
        assert!(inspected.summary().contains("payloads are opaque"));
    }

    #[test]
    fn catalog_keeps_namespaces_separate_and_returns_absent_for_missing_keys() {
        let body_id = AssetModelId(0x0310_0101);
        let pickup_id = AssetModelId(0x0311_0101);
        let key = CatalogVariaxId(0x1234_5678);
        let body_data = data_with_key(key.0);
        let pickup_data = data_with_key(key.0);
        let catalog = ModelCatalog::from_caller_supplied_decoded([
            CallerSuppliedDecodedModel {
                model_id: body_id,
                bytes: &body_data,
            },
            CallerSuppliedDecodedModel {
                model_id: pickup_id,
                bytes: &pickup_data,
            },
        ])
        .expect("synthetic decoded data");
        assert_eq!(catalog.body_asset(key), Some(body_id));
        assert_eq!(catalog.pickup_asset(key), Some(pickup_id));
        assert_eq!(catalog.body_asset(CatalogVariaxId(9)), None);
        assert_eq!(catalog.pickup_asset(CatalogVariaxId(9)), None);
        assert_eq!(body_id.category(), Some(ModelCategory::Body));
        assert_eq!(pickup_id.category(), Some(ModelCategory::Pickup));
        assert_ne!(body_id, pickup_id);
    }

    #[test]
    fn model_id_duplicates_keep_first_and_logical_key_duplicates_keep_last_sorted_id() {
        let key = 44;
        let lower = AssetModelId(0x0310_0101);
        let duplicate_lower = data_with_key(99);
        let lower_data = data_with_key(key);
        let higher = AssetModelId(0x0310_0102);
        let higher_data = data_with_key(key);
        let catalog = ModelCatalog::from_caller_supplied_decoded([
            CallerSuppliedDecodedModel {
                model_id: higher,
                bytes: &higher_data,
            },
            CallerSuppliedDecodedModel {
                model_id: lower,
                bytes: &lower_data,
            },
            CallerSuppliedDecodedModel {
                model_id: lower,
                bytes: &duplicate_lower,
            },
        ])
        .expect("synthetic decoded data");
        assert_eq!(catalog.body_asset(CatalogVariaxId(key)), Some(higher));
        assert_eq!(catalog.body_asset(CatalogVariaxId(99)), None);
    }

    #[test]
    fn rejects_short_caller_supplied_catalog_data_safely() {
        let error = ModelCatalog::from_caller_supplied_decoded([CallerSuppliedDecodedModel {
            model_id: AssetModelId(0x0310_0100),
            bytes: &[0; MODEL_KEY_END - 1],
        }])
        .expect_err("key word is truncated");
        assert!(matches!(error, CatalogError::ModelDataTooShort { .. }));
    }

    #[test]
    fn rejects_form_lengths_chunk_overruns_and_truncated_headers() {
        let valid = asset(&mdef(0x0310_0100, b"body", b"x"));
        for prefix in 0..12 {
            assert!(WorkbenchAssetInspection::parse(&valid[..prefix]).is_err());
        }
        let mut wrong_form_len = valid.clone();
        wrong_form_len[7] ^= 1;
        assert!(matches!(
            WorkbenchAssetInspection::parse(&wrong_form_len),
            Err(InspectError::FormLengthMismatch { .. })
        ));
        let mut bad_child = b"AINF\xff\xff\xff\xff".to_vec();
        bad_child.extend_from_slice(b"12345678");
        let invalid = container(b"FORM", b"WB2A", &bad_child);
        assert!(matches!(
            WorkbenchAssetInspection::parse(&invalid),
            Err(InspectError::ChunkExceedsParent { .. })
        ));
        let mut trailing_header = asset(&[]);
        trailing_header.extend_from_slice(b"short");
        let root_size = (trailing_header.len() - 8) as u32;
        trailing_header[4..8].copy_from_slice(&root_size.to_be_bytes());
        assert!(matches!(
            WorkbenchAssetInspection::parse(&trailing_header),
            Err(InspectError::TruncatedChunkHeader { .. })
        ));
    }

    #[test]
    fn rejects_short_ainf_minfo_and_mdat_prefixes() {
        let short_ainf = container(b"FORM", b"WB2A", &chunk(b"AINF", &[0; 16]));
        assert!(matches!(
            WorkbenchAssetInspection::parse(&short_ainf),
            Err(InspectError::InvalidAinf)
        ));

        let short_minfo = container(b"FORM", b"WB2A", &{
            let mut root = chunk(b"AINF", &[0; 17]);
            root.extend_from_slice(&container(b"LIST", b"MDEF", &chunk(b"MINF", &[0, 0, 0, 1])));
            root
        });
        assert!(matches!(
            WorkbenchAssetInspection::parse(&short_minfo),
            Err(InspectError::InvalidMinfo { .. })
        ));

        let mut info = 0x0310_0100u32.to_be_bytes().to_vec();
        info.extend_from_slice(b"body\0");
        let mut model_children = chunk(b"MINF", &info);
        model_children.extend_from_slice(&chunk(b"MDAT", &[0; MDAT_HEADER_BYTES - 1]));
        let mut root = chunk(b"AINF", &[0; 17]);
        root.extend_from_slice(&container(b"LIST", b"MDEF", &model_children));
        let short_mdat = container(b"FORM", b"WB2A", &root);
        assert!(matches!(
            WorkbenchAssetInspection::parse(&short_mdat),
            Err(InspectError::ShortMdatHeader { .. })
        ));
    }

    #[test]
    fn nesting_and_chunk_counts_are_bounded() {
        let mut nested = chunk(b"LEAF", b"");
        for _ in 0..=MAX_NESTING_DEPTH {
            nested = container(b"LIST", b"NEST", &nested);
        }
        let mut root_children = chunk(b"AINF", &[0; 17]);
        root_children.extend_from_slice(&nested);
        let too_deep = container(b"FORM", b"WB2A", &root_children);
        assert!(matches!(
            WorkbenchAssetInspection::parse(&too_deep),
            Err(InspectError::NestingLimit { .. })
        ));

        let mut many = chunk(b"AINF", &[0; 17]);
        for _ in 0..MAX_CHUNKS {
            many.extend_from_slice(&chunk(b"UNKN", b""));
        }
        let too_many = container(b"FORM", b"WB2A", &many);
        assert!(matches!(
            WorkbenchAssetInspection::parse(&too_many),
            Err(InspectError::ChunkLimit { .. })
        ));
    }

    #[test]
    fn odd_unknown_chunk_is_unpadded_as_in_workbench_hd_reader() {
        let bytes = asset(&{
            let mut chunks = mdef(0x0310_0100, b"body", b"odd");
            chunks.extend_from_slice(&chunk(b"ODD!", b"z"));
            chunks
        });
        let inspected = WorkbenchAssetInspection::parse(&bytes).expect("no-pad WB2A");
        assert_eq!(inspected.root.children.len(), 3);
        assert_eq!(inspected.root.children[2].tag, *b"ODD!");
    }
}
