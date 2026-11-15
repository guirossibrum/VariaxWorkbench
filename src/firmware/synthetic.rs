//! Synthetic FORM/L6FF fixtures.
//!
//! These containers are generated in memory from small patterns. They are not
//! product firmware and this module does not write them to a device.

use super::container::inspect;
use super::{
    PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION,
    VARIAX_500_700_PRODUCT_ID,
};
use std::fmt;
use std::io::Write;

const PYTHON_REFERENCE: &[u8] = include_bytes!("../../tests/fixtures/synthetic-l6ff-python.bin");
const PYTHON_IMAGE_MD5: &str = "b2ea9f7fcea831a4a63b213f41a8855b";

#[derive(Clone, Copy)]
pub(crate) struct RegionSpec {
    pub type_raw: u32,
    pub version: u32,
    pub offset: u32,
    pub size: u32,
}

pub(crate) struct Spec {
    pub format_version: u32,
    pub version: u32,
    pub product: u32,
    pub compression: u32,
    pub image: Vec<u8>,
    pub regions: Option<Vec<RegionSpec>>,
    pub md5_override: Option<[u8; 16]>,
    pub declared_length: Option<u32>,
    pub duplicate_head: bool,
    pub duplicate_data: bool,
    pub unknown_chunk: bool,
    pub data_before_regions: bool,
    pub xor_compressed: Option<usize>,
    pub truncate_compressed_to: Option<usize>,
    pub append_compressed_byte: bool,
    pub omit_iff_pad: bool,
    pub reserved_tail: [u8; 16],
}

impl Spec {
    pub(crate) fn basic(product: u32, version: u32, image: &[u8]) -> Self {
        Self {
            format_version: super::FORMAT_VERSION,
            version,
            product,
            compression: 1,
            image: image.to_vec(),
            regions: None,
            md5_override: None,
            declared_length: None,
            duplicate_head: false,
            duplicate_data: false,
            unknown_chunk: false,
            data_before_regions: false,
            xor_compressed: None,
            truncate_compressed_to: None,
            append_compressed_byte: false,
            omit_iff_pad: false,
            reserved_tail: [0; 16],
        }
    }
}

pub(crate) fn build(spec: &Spec) -> Vec<u8> {
    let mut compressed = compress_zlib(&spec.image);
    if let Some(index) = spec.xor_compressed
        && !compressed.is_empty()
    {
        let index = index % compressed.len();
        compressed[index] ^= 0xff;
    }
    if let Some(limit) = spec.truncate_compressed_to {
        compressed.truncate(limit.min(compressed.len()));
    }
    if spec.append_compressed_byte || (spec.omit_iff_pad && compressed.len().is_multiple_of(2)) {
        compressed.push(0);
    }
    let declared = spec
        .declared_length
        .unwrap_or(u32::try_from(spec.image.len()).expect("synthetic image fits in u32"));
    let mut head = Vec::with_capacity(52);
    for word in [
        spec.format_version,
        spec.version,
        spec.product,
        spec.compression,
        declared,
    ] {
        head.extend_from_slice(&word.to_be_bytes());
    }
    let digest = spec.md5_override.unwrap_or_else(|| md5_bytes(&spec.image));
    head.extend_from_slice(&digest);
    head.extend_from_slice(&spec.reserved_tail);
    let regions = spec.regions.clone().unwrap_or_else(|| {
        vec![RegionSpec {
            type_raw: 0,
            version: spec.version,
            offset: 0,
            size: u32::try_from(spec.image.len()).expect("synthetic image fits in u32"),
        }]
    });
    let head_chunk = chunk(b"HEAD", &head, false);
    let mut region_chunks = Vec::new();
    for region in &regions {
        let mut payload = Vec::with_capacity(16);
        for word in [region.type_raw, region.version, region.offset, region.size] {
            payload.extend_from_slice(&word.to_be_bytes());
        }
        region_chunks.extend_from_slice(&chunk(b"dinf", &payload, false));
    }
    let data_chunk = chunk(b"data", &compressed, spec.omit_iff_pad);
    let mut body = b"L6FF".to_vec();
    body.extend_from_slice(&head_chunk);
    if spec.duplicate_head {
        body.extend_from_slice(&head_chunk);
    }
    if spec.data_before_regions {
        body.extend_from_slice(&data_chunk);
        body.extend_from_slice(&region_chunks);
    } else {
        body.extend_from_slice(&region_chunks);
        if spec.unknown_chunk {
            body.extend_from_slice(&chunk(b"ZZZZ", b"nope", false));
        }
        body.extend_from_slice(&data_chunk);
    }
    if spec.duplicate_data {
        body.extend_from_slice(&data_chunk);
    }
    wrap_form(&body)
}

/// Build one structurally valid synthetic container.
///
/// The result is test data. Nothing in this crate installs it.
pub fn synthetic_container(product_id: u32, version: u32, image: &[u8]) -> Vec<u8> {
    build(&Spec::basic(product_id, version, image))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RejectionCounts {
    pub truncated_prefixes: usize,
    pub damaged_structural_fields: usize,
    pub wrong_product: usize,
    pub wrong_version: usize,
    pub oversized: usize,
    pub overlapping: usize,
    pub duplicate_chunks: usize,
    pub corrupted_image: usize,
}

impl RejectionCounts {
    pub fn structural_total(self) -> usize {
        self.truncated_prefixes
            + self.damaged_structural_fields
            + self.oversized
            + self.overlapping
            + self.duplicate_chunks
            + self.corrupted_image
    }

    pub fn applicability_total(self) -> usize {
        self.wrong_product + self.wrong_version
    }
}

impl fmt::Display for RejectionCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "truncated_prefixes: {}", self.truncated_prefixes)?;
        writeln!(
            f,
            "damaged_structural_fields: {}",
            self.damaged_structural_fields
        )?;
        writeln!(f, "wrong_product: {}", self.wrong_product)?;
        writeln!(f, "wrong_version: {}", self.wrong_version)?;
        writeln!(f, "oversized: {}", self.oversized)?;
        writeln!(f, "overlapping: {}", self.overlapping)?;
        writeln!(f, "duplicate_chunks: {}", self.duplicate_chunks)?;
        writeln!(f, "corrupted_image: {}", self.corrupted_image)?;
        writeln!(f, "structural_rejections: {}", self.structural_total())?;
        writeln!(
            f,
            "applicability_rejections: {}",
            self.applicability_total()
        )?;
        writeln!(
            f,
            "integrity_is_not_authentication: true; no flashing API was exercised"
        )?;
        Ok(())
    }
}

/// Accept the Python reference container and reject the broader synthetic cases.
pub fn run_rejection_suite() -> Result<RejectionCounts, String> {
    accept_python_reference()?;
    Ok(RejectionCounts {
        truncated_prefixes: reject_prefixes()?,
        damaged_structural_fields: reject_damaged_fields()?,
        wrong_product: reject_wrong_products()?,
        wrong_version: reject_wrong_version()?,
        oversized: reject_oversized()?,
        overlapping: reject_overlapping()?,
        duplicate_chunks: reject_duplicate_chunks()?,
        corrupted_image: reject_corrupted_images()?,
    })
}

fn accept_python_reference() -> Result<(), String> {
    if PYTHON_REFERENCE.len() != 390 {
        return Err(format!(
            "python reference fixture is {} bytes",
            PYTHON_REFERENCE.len()
        ));
    }
    if &PYTHON_REFERENCE[96..100] != b"data" {
        return Err("python reference data chunk moved; damaged-field offsets are stale".into());
    }
    let inspection = inspect(PYTHON_REFERENCE).map_err(|error| error.to_string())?;
    let image: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
    if inspection.image() != image {
        return Err("python reference image did not round-trip".into());
    }
    if inspection.report.product_id != PODXT_LIVE_PRODUCT_ID
        || inspection.report.version != PODXT_LIVE_VERSION
    {
        return Err("python reference product or version changed".into());
    }
    if super::container::hex(&inspection.report.image_md5) != PYTHON_IMAGE_MD5 {
        return Err("python reference MD5 does not match the Rust digest".into());
    }
    if !inspection.report.header_md5_matches || !inspection.report.integrity_is_not_authentication {
        return Err("integrity flags were not reported".into());
    }
    if inspection.report.regions.len() != 1 {
        return Err("python reference region count changed".into());
    }
    inspection
        .require_target(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn reject_prefixes() -> Result<usize, String> {
    let mut rejected = 0;
    for length in 0..PYTHON_REFERENCE.len() {
        expect_structural_reject(&PYTHON_REFERENCE[..length], &format!("prefix {length}"))?;
        rejected += 1;
    }
    Ok(rejected)
}

fn reject_damaged_fields() -> Result<usize, String> {
    let mut rejected = 0;
    for offset in [4, 20, 32, 36, 40, 96] {
        let mut damaged = PYTHON_REFERENCE.to_vec();
        damaged[offset] ^= 0xff;
        expect_structural_reject(&damaged, &format!("damaged offset {offset}"))?;
        rejected += 1;
    }
    Ok(rejected)
}

fn reject_wrong_products() -> Result<usize, String> {
    let image = sample_image();
    let recognized = build(&Spec::basic(VARIAX_500_700_PRODUCT_ID, 0x0310_0000, &image));
    expect_target_reject(
        &recognized,
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
        "recognized 500/700 product",
    )?;
    let unknown = build(&Spec::basic(0x00ab_cdef, VARIAX_300_600_VERSION, &image));
    expect_target_reject(
        &unknown,
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
        "unknown product",
    )?;
    Ok(2)
}

fn reject_wrong_version() -> Result<usize, String> {
    let image = sample_image();
    // Synthetic version word. This is not a captured 3.07 firmware image.
    let bytes = build(&Spec::basic(VARIAX_300_600_PRODUCT_ID, 0x0307_0000, &image));
    expect_target_reject(
        &bytes,
        VARIAX_300_600_PRODUCT_ID,
        VARIAX_300_600_VERSION,
        "wrong version",
    )?;
    Ok(1)
}

fn reject_oversized() -> Result<usize, String> {
    let image = sample_image();
    let mut declared = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    declared.declared_length = Some(super::MAX_IMAGE_BYTES + 1);
    expect_structural_reject(&build(&declared), "declared length above 16 MiB")?;
    let mut short = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    short.declared_length = Some(8);
    expect_structural_reject(&build(&short), "decompressed image longer than declared")?;
    Ok(2)
}

fn reject_overlapping() -> Result<usize, String> {
    let image = sample_image();
    let mut overlap = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    overlap.regions = Some(vec![
        RegionSpec {
            type_raw: 1,
            version: PODXT_LIVE_VERSION,
            offset: 0,
            size: 24,
        },
        RegionSpec {
            type_raw: 2,
            version: PODXT_LIVE_VERSION,
            offset: 16,
            size: 16,
        },
    ]);
    expect_structural_reject(&build(&overlap), "overlapping regions")?;
    let mut duplicate = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    let size = u32::try_from(image.len()).expect("sample fits");
    duplicate.regions = Some(vec![
        RegionSpec {
            type_raw: 1,
            version: PODXT_LIVE_VERSION,
            offset: 0,
            size,
        },
        RegionSpec {
            type_raw: 1,
            version: PODXT_LIVE_VERSION,
            offset: 0,
            size,
        },
    ]);
    expect_structural_reject(&build(&duplicate), "duplicate region")?;
    Ok(2)
}

fn reject_duplicate_chunks() -> Result<usize, String> {
    let image = sample_image();
    let mut head = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    head.duplicate_head = true;
    expect_structural_reject(&build(&head), "duplicate HEAD")?;
    let mut data = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    data.duplicate_data = true;
    expect_structural_reject(&build(&data), "duplicate data")?;
    Ok(2)
}

fn reject_corrupted_images() -> Result<usize, String> {
    let image = sample_image();
    let mut digest = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    digest.md5_override = Some([0; 16]);
    expect_structural_reject(&build(&digest), "replaced MD5")?;
    let mut flipped = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    flipped.xor_compressed = Some(10);
    expect_structural_reject(&build(&flipped), "flipped compressed byte")?;
    let mut truncated = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    truncated.truncate_compressed_to = Some(8);
    expect_structural_reject(&build(&truncated), "truncated zlib stream")?;
    let mut extra = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image);
    extra.append_compressed_byte = true;
    expect_structural_reject(&build(&extra), "byte after zlib stream")?;
    Ok(4)
}

fn expect_structural_reject(bytes: &[u8], label: &str) -> Result<(), String> {
    match inspect(bytes) {
        Ok(_) => Err(format!("{label} was accepted")),
        Err(_) => Ok(()),
    }
}

fn expect_target_reject(
    bytes: &[u8],
    product: u32,
    version: u32,
    label: &str,
) -> Result<(), String> {
    let inspection = inspect(bytes).map_err(|error| format!("{label} should parse: {error}"))?;
    match inspection.require_target(product, version) {
        Ok(()) => Err(format!(
            "{label} was treated as applicable to {product:08x}/{version:08x}"
        )),
        Err(_) => Ok(()),
    }
}

fn sample_image() -> Vec<u8> {
    (0u8..32).collect()
}

fn compress_zlib(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(data)
        .expect("in-memory zlib encoder accepts the synthetic image");
    encoder.finish().expect("in-memory zlib encoder finishes")
}

fn md5_bytes(data: &[u8]) -> [u8; 16] {
    let digest = <md5::Md5 as md5::Digest>::digest(data);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest);
    out
}

fn chunk(tag: &[u8; 4], payload: &[u8], omit_pad: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 9);
    let size = u32::try_from(payload.len()).expect("chunk fits");
    out.extend_from_slice(tag);
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(payload);
    if !payload.len().is_multiple_of(2) && !omit_pad {
        out.push(0);
    }
    out
}

fn wrap_form(body: &[u8]) -> Vec<u8> {
    let size = u32::try_from(body.len()).expect("body fits");
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(body);
    out
}
