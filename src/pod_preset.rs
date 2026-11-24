#![forbid(unsafe_code)]
//! Compatibility entry point for PODxt and HD500X offline files.

use crate::document;
use std::path::Path;

pub use crate::families::hd500x::file::*;
pub use crate::families::podxt::file::*;
pub use crate::shared::file::{PathFacts, ToneError, open_regular_file, path_facts};

const VARIAX_FAMILY_MASK: u32 = 0xffff_0000;
const VARIAX_FAMILY_HIGH: u32 = 0x0007_0000;

/// A POD-family file recognized by its byte structure, without implying that
/// its format is supported by a particular product profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PodPresetFile {
    PodxtLive(PodTone),
    H5ep(OpaqueH5epPreset),
    H5es(H5esSetlist),
    H5eb(H5ebBundle),
}

impl PodPresetFile {
    /// Dispatches only on the file's structural magic. Suffixes are not used.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.starts_with(b"H5EP") {
            OpaqueH5epPreset::parse(bytes).map(Self::H5ep)
        } else if bytes.starts_with(b"H5ES") {
            H5esSetlist::parse(bytes).map(Self::H5es)
        } else if bytes.starts_with(b"H5EB") {
            H5ebBundle::parse(bytes).map(Self::H5eb)
        } else if bytes.starts_with(b"FORM") {
            PodTone::parse(bytes).map(Self::PodxtLive)
        } else {
            Err(ToneError::format(
                "unrecognized individual POD file structure",
            ))
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::PodxtLive(tone) => tone.as_bytes(),
            Self::H5ep(preset) => preset.as_bytes(),
            Self::H5es(setlist) => setlist.as_bytes(),
            Self::H5eb(bundle) => bundle.as_bytes(),
        }
    }

    /// Exact no-op serialization. This never edits or normalizes stored bytes.
    pub fn serialize_noop(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, self.as_bytes()).map_err(ToneError::format)
    }

    pub fn hd_container_kind(&self) -> Option<&'static str> {
        match self {
            Self::H5ep(_) => Some("H5EP preset"),
            Self::H5es(_) => Some("H5ES setlist"),
            Self::H5eb(_) => Some("H5EB bundle"),
            Self::PodxtLive(_) => None,
        }
    }
}

impl ToneError {
    pub fn format(detail: impl Into<String>) -> Self {
        Self::Format {
            detail: detail.into(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            Self::Format { detail } => detail.clone(),
            Self::WrongTarget { device_id } => {
                format!("PINF target {device_id:08x} is not PODxt Live {PODXT_LIVE_TONE_ID:08x}")
            }
        }
    }

    pub(crate) fn is_variax_family(device_id: u32) -> bool {
        device_id & VARIAX_FAMILY_MASK == VARIAX_FAMILY_HIGH
    }
}

#[cfg(test)]
pub(crate) use crate::families::podxt::file::tests::fixture;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) use crate::families::podxt::file::tests::delay_note_fixture;
    use crate::families::podxt::file::tests::live_tone;

    #[test]
    fn pod_dispatch_preserves_valid_tones_and_rejects_wrong_pinf_and_other_tags() {
        let valid = live_tone();
        let dispatched = PodPresetFile::parse(valid.clone()).unwrap();
        assert!(matches!(&dispatched, PodPresetFile::PodxtLive(_)));
        assert_eq!(dispatched.serialize_noop(), valid);

        let wrong_target = fixture(ContainerKind::L6pa, 0x0003_000b, 1, &[(AMP_SLOT, 7)]);
        assert!(matches!(
            PodPresetFile::parse(wrong_target),
            Err(ToneError::WrongTarget {
                device_id: 0x0003_000b
            })
        ));
        for tag in [b"H5ES".as_slice(), b"H5EB".as_slice()] {
            let mut unsupported = vec![0u8; H5EP_MIN_BYTES];
            unsupported[..4].copy_from_slice(tag);
            unsupported[4..8].copy_from_slice(&1u32.to_be_bytes());
            assert!(matches!(
                PodPresetFile::parse(unsupported),
                Err(ToneError::Format { .. })
            ));
        }
    }
}
