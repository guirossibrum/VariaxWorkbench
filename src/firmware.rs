#![forbid(unsafe_code)]
//! Offline FORM/L6FF firmware-container inspection.
//!
//! The inspector reads bytes the caller already has. It does not open USB, MIDI,
//! or audio devices, and it exposes no firmware install, flash, reset, or device
//! write operation. Embedded MD5 agreement proves only that the uncompressed
//! image matches the digest stored in the container. It is not authentication.
//!
//! A structurally valid container can still be the wrong product or version.
//! [`Inspection::require_target`] is the applicability gate.

mod applicability;
mod container;
pub mod synthetic;

pub use applicability::{
    Applicability, ApplicabilityError, CatalogEntry, PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION,
    PRODUCT_CATALOG, VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION, VARIAX_500_700_PRODUCT_ID,
    observed_version_label,
};
pub use container::{
    FORMAT_VERSION, INTEGRITY_STATEMENT, InspectError, Inspection, MAX_CONTAINER_BYTES,
    MAX_IMAGE_BYTES, Region, Report, inspect,
};

#[cfg(test)]
mod tests {
    use super::container::InspectError;
    use super::synthetic::{RegionSpec, Spec, build, run_rejection_suite, synthetic_container};
    use super::{
        Applicability, PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, PRODUCT_CATALOG,
        VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION, VARIAX_500_700_PRODUCT_ID, inspect,
        observed_version_label,
    };

    fn image(len: usize) -> Vec<u8> {
        (0..len).map(|index| index as u8).collect()
    }

    #[test]
    fn rejection_suite_covers_required_classes() {
        let counts = run_rejection_suite().expect("synthetic rejection suite");
        assert_eq!(counts.truncated_prefixes, 390);
        assert_eq!(counts.damaged_structural_fields, 6);
        assert_eq!(counts.wrong_product, 2);
        assert_eq!(counts.wrong_version, 1);
        assert_eq!(counts.oversized, 2);
        assert_eq!(counts.overlapping, 2);
        assert_eq!(counts.duplicate_chunks, 2);
        assert_eq!(counts.corrupted_image, 4);
        assert_eq!(counts.structural_total(), 406);
        assert_eq!(counts.applicability_total(), 3);
    }

    #[test]
    fn multi_region_container_round_trips_and_reports_checksums() {
        let bytes_image = image(30);
        let mut spec = Spec::basic(
            VARIAX_300_600_PRODUCT_ID,
            VARIAX_300_600_VERSION,
            &bytes_image,
        );
        spec.reserved_tail = [0x11; 16];
        spec.regions = Some(vec![
            RegionSpec {
                type_raw: 5,
                version: VARIAX_300_600_VERSION,
                offset: 0,
                size: 10,
            },
            RegionSpec {
                type_raw: 0,
                version: VARIAX_300_600_VERSION,
                offset: 10,
                size: 20,
            },
        ]);
        let inspection = inspect(&build(&spec)).expect("two contiguous regions");
        assert_eq!(inspection.image(), bytes_image.as_slice());
        assert!(inspection.report.header_md5_matches);
        assert!(inspection.report.integrity_is_not_authentication);
        assert_eq!(inspection.report.reserved_tail, [0x11; 16]);
        assert_eq!(inspection.report.regions.len(), 2);
        assert_eq!(inspection.report.regions[0].type_raw, 5);
        assert_eq!(inspection.report.regions[1].offset, 10);
        let region_sha = <sha2::Sha256 as sha2::Digest>::digest(&bytes_image[..10]);
        assert_eq!(
            inspection.report.regions[0].sha256.as_slice(),
            region_sha.as_slice()
        );
        inspection
            .require_target(VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION)
            .expect("declared target");
        let json = inspection.to_json();
        assert!(json.contains("\"integrity_is_not_authentication\": true"));
        assert!(json.contains("not a signature"));
        assert!(!json.contains("signature_valid"));
        assert!(
            inspection
                .to_text()
                .contains("integrity_is_not_authentication: true")
        );
    }

    #[test]
    fn data_chunk_may_precede_region_table() {
        let bytes_image = image(16);
        let mut spec = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        spec.data_before_regions = true;
        let inspection = inspect(&build(&spec)).expect("data before dinf");
        assert_eq!(inspection.report.regions.len(), 1);
    }

    #[test]
    fn recorded_declaration_is_not_the_archived_file() {
        let bytes = synthetic_container(
            VARIAX_300_600_PRODUCT_ID,
            VARIAX_300_600_VERSION,
            b"synthetic-image-not-the-archived-firmware",
        );
        let inspection = inspect(&bytes).expect("synthetic declaration parses");
        assert!(matches!(
            inspection.report.applicability,
            Applicability::RecordedDeclaration {
                name: "Variax 300/600"
            }
        ));
        let fingerprint = inspection.to_text();
        assert!(
            !fingerprint
                .contains("e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a")
        );
        assert!(fingerprint.contains("not authenticity"));
    }

    #[test]
    fn applicability_distinguishes_known_wrong_and_unknown_products() {
        let bytes_image = image(12);
        let other = inspect(&synthetic_container(
            PODXT_LIVE_PRODUCT_ID,
            PODXT_LIVE_VERSION,
            &bytes_image,
        ))
        .expect("pod declaration");
        assert!(matches!(
            other.require_target(VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION),
            Err(super::ApplicabilityError::WrongProduct { .. })
        ));
        let wrong_version = inspect(&synthetic_container(
            VARIAX_300_600_PRODUCT_ID,
            0x0307_0000,
            &bytes_image,
        ))
        .expect("synthetic wrong version parses");
        assert!(matches!(
            wrong_version.report.applicability,
            Applicability::WrongVersion {
                name: "Variax 300/600",
                recorded_version: VARIAX_300_600_VERSION
            }
        ));
        assert!(matches!(
            wrong_version.require_target(VARIAX_300_600_PRODUCT_ID, VARIAX_300_600_VERSION),
            Err(super::ApplicabilityError::WrongVersion { .. })
        ));
        let recognized = inspect(&synthetic_container(
            VARIAX_500_700_PRODUCT_ID,
            0x0310_0000,
            &bytes_image,
        ))
        .expect("500/700 id parses");
        assert!(matches!(
            recognized.report.applicability,
            Applicability::RecognizedWithoutArchivedImage {
                name: "Variax 500/700"
            }
        ));
        let unknown =
            inspect(&synthetic_container(0x00ab_cdef, 1, &bytes_image)).expect("unknown id parses");
        assert!(matches!(
            unknown.report.applicability,
            Applicability::UnknownProduct
        ));
    }

    #[test]
    fn malformed_regions_and_chunks_are_rejected() {
        let bytes_image = image(30);
        let mut gap = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        gap.regions = Some(vec![
            RegionSpec {
                type_raw: 0,
                version: PODXT_LIVE_VERSION,
                offset: 0,
                size: 10,
            },
            RegionSpec {
                type_raw: 0,
                version: PODXT_LIVE_VERSION,
                offset: 12,
                size: 18,
            },
        ]);
        assert!(matches!(
            inspect(&build(&gap)),
            Err(InspectError::RegionGap { .. })
        ));

        let mut empty = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        empty.regions = Some(vec![RegionSpec {
            type_raw: 0,
            version: 0,
            offset: 0,
            size: 0,
        }]);
        assert!(matches!(
            inspect(&build(&empty)),
            Err(InspectError::EmptyRegion { index: 0 })
        ));

        let mut none = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        none.regions = Some(Vec::new());
        assert!(matches!(
            inspect(&build(&none)),
            Err(InspectError::NoRegions)
        ));

        let mut short = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        short.regions = Some(vec![RegionSpec {
            type_raw: 0,
            version: 0,
            offset: 0,
            size: 10,
        }]);
        assert!(matches!(
            inspect(&build(&short)),
            Err(InspectError::IncompleteCoverage { .. })
        ));

        let mut wide = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        wide.regions = Some(vec![RegionSpec {
            type_raw: 0,
            version: 0,
            offset: 0,
            size: 40,
        }]);
        assert!(matches!(
            inspect(&build(&wide)),
            Err(InspectError::RegionOutOfRange { .. })
        ));

        let mut unknown = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        unknown.unknown_chunk = true;
        assert!(matches!(
            inspect(&build(&unknown)),
            Err(InspectError::UnsupportedChunk { .. })
        ));

        let mut format = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        format.format_version = 2;
        assert!(matches!(
            inspect(&build(&format)),
            Err(InspectError::UnsupportedContainer { .. })
        ));

        let mut compression = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        compression.compression = 0;
        assert!(matches!(
            inspect(&build(&compression)),
            Err(InspectError::UnsupportedContainer { .. })
        ));

        let mut zero = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        zero.declared_length = Some(0);
        assert!(matches!(
            inspect(&build(&zero)),
            Err(InspectError::ImageLength(0))
        ));

        let mut length =
            synthetic_container(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &bytes_image);
        length[4] ^= 0xff;
        assert!(matches!(
            inspect(&length),
            Err(InspectError::FormLengthMismatch { .. })
        ));
    }

    #[test]
    fn odd_chunk_without_pad_is_rejected_and_pad_is_not_payload_length() {
        let mut spec = Spec::basic(PODXT_LIVE_PRODUCT_ID, PODXT_LIVE_VERSION, &image(20));
        spec.omit_iff_pad = true;
        let bytes = build(&spec);
        assert!(matches!(
            inspect(&bytes),
            Err(InspectError::ChunkExceedsFile { .. })
        ));
        let mut padded = bytes;
        let form_len = u32::from_be_bytes(padded[4..8].try_into().unwrap()) + 1;
        padded[4..8].copy_from_slice(&form_len.to_be_bytes());
        padded.push(0);
        let error = inspect(&padded).expect_err("extra compressed byte still fails");
        assert!(!matches!(
            error,
            InspectError::ChunkExceedsFile { .. } | InspectError::FormLengthMismatch { .. }
        ));
    }

    #[test]
    fn oversized_container_is_rejected_before_decompression() {
        let bytes = vec![0u8; super::MAX_CONTAINER_BYTES + 1];
        assert!(matches!(
            inspect(&bytes),
            Err(InspectError::ContainerTooLarge { .. })
        ));
    }

    #[test]
    fn version_label_matches_the_two_archived_revisions_only_as_a_display() {
        assert_eq!(
            observed_version_label(PODXT_LIVE_VERSION).as_deref(),
            Some("3.01")
        );
        assert_eq!(
            observed_version_label(VARIAX_300_600_VERSION).as_deref(),
            Some("3.10")
        );
        assert_eq!(observed_version_label(0x0301_0001), None);
    }

    #[test]
    fn catalog_product_ids_are_unique() {
        let mut ids: Vec<u32> = PRODUCT_CATALOG
            .iter()
            .map(|entry| entry.product_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PRODUCT_CATALOG.len());
        assert!(
            PRODUCT_CATALOG
                .iter()
                .any(|entry| entry.archived_version.is_none())
        );
    }
}
