//! Preservation-only XFP6 envelope inspection. No plugin application/conversion.
//!
//! The strict admission policy is intentionally narrower than native readers.
//! Unknown prefix/name bytes, section ordering, gaps, nested bytes and tails stay
//! byte-identical. Host automation, model/pack semantics and target compatibility
//! remain unverified; this module never dispatches to an engine or transport.
#![forbid(unsafe_code)]

use std::{ops::Range, path::Path};

const PREFIX: usize = 244;
/// Owned policy cap, not a recovered vendor or host limit.
pub const MAX_STATE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    Size,
    Signature,
    HeaderExtension,
    TotalLength,
    IncompleteSpan,
    SpanBounds,
    OverlappingSections,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    ToneA,
    ToneB,
    Mapping,
    Settings,
}

impl Section {
    fn index(self) -> usize {
        match self {
            Self::ToneA => 0,
            Self::ToneB => 1,
            Self::Mapping => 2,
            Self::Settings => 3,
        }
    }
}

/// Explicit inspection limits; none is a claim about native rejection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnverifiedMeaning {
    NestedToneAndPackCompatibility,
    AutomationBindings,
    PositionalSettingsApplication,
    HostProductAndVersion,
}

pub const UNVERIFIED: [UnverifiedMeaning; 4] = [
    UnverifiedMeaning::NestedToneAndPackCompatibility,
    UnverifiedMeaning::AutomationBindings,
    UnverifiedMeaning::PositionalSettingsApplication,
    UnverifiedMeaning::HostProductAndVersion,
];

#[derive(Clone, Debug)]
pub struct PreservedState {
    bytes: Vec<u8>,
    spans: [Option<Range<usize>>; 4],
}

impl PreservedState {
    /// Admit only fixed 244-byte envelopes with exact total and disjoint spans.
    /// Nested data is opaque: success means safe bounded inspection/copy only.
    pub fn inspect(bytes: &[u8]) -> Result<Self, StateError> {
        if bytes.len() < PREFIX || bytes.len() > MAX_STATE_BYTES {
            return Err(StateError::Size);
        }
        let little = match &bytes[..4] {
            b"XFP6" => true,
            b"6PFX" => false,
            _ => return Err(StateError::Signature),
        };
        let word = |offset: usize| {
            let a = bytes[offset..offset + 4].try_into().expect("fixed prefix");
            if little {
                u32::from_le_bytes(a)
            } else {
                u32::from_be_bytes(a)
            }
        };
        if word(4) != PREFIX as u32 {
            return Err(StateError::HeaderExtension);
        }
        if u64::from(word(8)) != bytes.len() as u64 {
            return Err(StateError::TotalLength);
        }
        let mut spans: [Option<Range<usize>>; 4] = std::array::from_fn(|_| None);
        for (i, slot) in spans.iter_mut().enumerate() {
            let start = u64::from(word(0x18 + i * 8));
            let len = u64::from(word(0x1c + i * 8));
            if start == 0 && len == 0 {
                continue;
            }
            if start == 0 || len == 0 {
                return Err(StateError::IncompleteSpan);
            }
            let end = start + len; // u32 + u32 cannot overflow u64.
            if start < PREFIX as u64 || end > bytes.len() as u64 {
                return Err(StateError::SpanBounds);
            }
            *slot = Some(start as usize..end as usize);
        }
        for (i, a) in spans.iter().enumerate() {
            for b in spans.iter().skip(i + 1) {
                if let (Some(a), Some(b)) = (a, b)
                    && a.start < b.end
                    && b.start < a.end
                {
                    return Err(StateError::OverlappingSections);
                }
            }
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            spans,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn section(&self, section: Section) -> Option<&[u8]> {
        self.spans[section.index()]
            .as_ref()
            .map(|r| &self.bytes[r.clone()])
    }

    /// Atomic non-overwriting copy; no canonicalization or nested serialization.
    pub fn save_copy(&self, path: &Path) -> Result<(), String> {
        crate::document::save_new(path, &self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(b: &mut [u8], at: usize, value: u32, little: bool) {
        b[at..at + 4].copy_from_slice(&if little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        });
    }
    fn fixture(little: bool) -> Vec<u8> {
        // Owned synthetic sections deliberately include repeated unknown records,
        // a non-text name, gaps, and a tail; section order differs from header order.
        let mut b = vec![0xa5; PREFIX + 40];
        b[..4].copy_from_slice(if little { b"XFP6" } else { b"6PFX" });
        word(&mut b, 4, PREFIX as u32, little);
        word(&mut b, 8, (PREFIX + 40) as u32, little);
        for (i, (offset, len)) in [(PREFIX + 16, 8), (0, 0), (PREFIX, 8), (PREFIX + 28, 4)]
            .into_iter()
            .enumerate()
        {
            word(&mut b, 0x18 + i * 8, offset as u32, little);
            word(&mut b, 0x1c + i * 8, len, little);
        }
        b[PREFIX..PREFIX + 8].copy_from_slice(b"????????");
        b
    }

    #[test]
    fn opaque_roundtrip_preserves_both_endians_unknowns_order_and_duplicates() {
        for little in [true, false] {
            let b = fixture(little);
            let state = PreservedState::inspect(&b).unwrap();
            assert_eq!(state.as_bytes(), b);
            assert_eq!(
                state.section(Section::Mapping),
                Some(b"????????".as_slice())
            );
            assert!(state.section(Section::ToneB).is_none());
            assert_eq!(
                state.section(Section::ToneA).unwrap(),
                &b[PREFIX + 16..PREFIX + 24]
            );
            assert_eq!(UNVERIFIED.len(), 4);
            assert_eq!(
                PreservedState::inspect(state.as_bytes())
                    .unwrap()
                    .as_bytes(),
                b
            );
        }
    }

    #[test]
    fn every_truncation_and_appended_data_disagreement_is_rejected() {
        let b = fixture(true);
        for n in 0..b.len() {
            assert!(PreservedState::inspect(&b[..n]).is_err(), "{n}");
        }
        let mut extra = b;
        extra.push(0);
        assert_eq!(
            PreservedState::inspect(&extra).unwrap_err(),
            StateError::TotalLength
        );
    }

    #[test]
    fn extensions_aliases_and_dangling_spans_are_explicitly_rejected() {
        for little in [true, false] {
            let b = fixture(little);
            for value in [0, 243, 245, u32::MAX] {
                let mut x = b.clone();
                word(&mut x, 4, value, little);
                assert_eq!(
                    PreservedState::inspect(&x).unwrap_err(),
                    StateError::HeaderExtension
                );
            }
            for (offset, len, error) in [
                (0, 1, StateError::IncompleteSpan),
                (244, 0, StateError::IncompleteSpan),
                (243, 1, StateError::SpanBounds),
                (u32::MAX, u32::MAX, StateError::SpanBounds),
                (244, 8, StateError::OverlappingSections),
            ] {
                let mut x = b.clone();
                word(&mut x, 0x18, offset, little);
                word(&mut x, 0x1c, len, little);
                assert_eq!(PreservedState::inspect(&x).unwrap_err(), error);
            }
        }
    }

    #[test]
    fn signatures_are_not_product_or_semantic_admission() {
        for magic in [b"FORM", b"L6P2", b"PINF", b"XFP7"] {
            let mut b = fixture(true);
            b[..4].copy_from_slice(magic);
            assert_eq!(
                PreservedState::inspect(&b).unwrap_err(),
                StateError::Signature
            );
        }
        let mut b = fixture(true);
        // Arbitrary claimed host product words and nested bytes remain opaque.
        word(&mut b, 0x10, 0x3000a, true);
        assert!(PreservedState::inspect(&b).is_ok());
        assert!(UNVERIFIED.contains(&UnverifiedMeaning::HostProductAndVersion));
    }

    #[test]
    fn policy_cap_and_empty_section_envelope() {
        let mut b = fixture(true);
        for i in 0..8 {
            word(&mut b, 0x18 + i * 4, 0, true);
        }
        assert!(PreservedState::inspect(&b).is_ok());
        assert_eq!(
            PreservedState::inspect(&vec![0; MAX_STATE_BYTES + 1]).unwrap_err(),
            StateError::Size
        );
    }

    #[test]
    fn save_copy_is_atomic_non_overwriting_and_preserves_source_and_state() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("original");
        let copy = dir.path().join("copy");
        let b = fixture(true);
        std::fs::write(&original, &b).unwrap();
        let state = PreservedState::inspect(&std::fs::read(&original).unwrap()).unwrap();
        state.save_copy(&copy).unwrap();
        assert_eq!(std::fs::read(&copy).unwrap(), b);
        assert!(state.save_copy(&copy).is_err());
        assert!(state.save_copy(&dir.path().join("missing/child")).is_err());
        assert_eq!(std::fs::read(&original).unwrap(), b);
        assert_eq!(state.as_bytes(), b);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
