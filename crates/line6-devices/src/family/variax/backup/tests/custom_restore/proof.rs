//! Independent byte oracle for the synthetic unity-gain Custom 1 fixture.
//! Captured programs need separately established trim and DSP provenance.

use super::{Image, METADATA, PROGRAM_BYTES, Program, procedures};
use std::ops::Range;

const GAIN: Range<usize> = 555 * 3..556 * 3;
const TRIM: Range<usize> = METADATA.start + 65..METADATA.start + 68;
const VERSION: Range<usize> = 767 * 3..771 * 3;
const CHECKSUM: Range<usize> = PROGRAM_BYTES - 3..PROGRAM_BYTES;
const VERSION_310: [u8; 12] = [0, 0, b'0', 0, 0, b'3', 0, 0, b'1', 0, 0, b'0'];
const ZERO_DB: [u8; 3] = [0; 3];
const MINUS_SIX_DB: [u8; 3] = [0xc0, 0xc0, 0x00];
const UNITY_GAIN: [u8; 3] = [0xff; 3];
// The trim flag (bit 23) and round(0.5011872336 × 8,388,607).
const ATTENUATED_GAIN: [u8; 3] = [0xc0, 0x26, 0xe7];

fn sum(image: &Image, end_word: usize) -> [u8; 3] {
    let sum: u64 = image.as_chunks::<3>().0[..end_word]
        .iter()
        .map(|&[high, mid, low]| (u64::from(high) << 16) | (u64::from(mid) << 8) | u64::from(low))
        .sum();
    let [.., high, mid, low] = sum.to_be_bytes();
    [high, mid, low]
}

fn stamp(image: &Image) -> Image {
    let mut stored = *image;
    stored[VERSION].copy_from_slice(&VERSION_310);
    let checksum = sum(&stored, 1091);
    stored[CHECKSUM].copy_from_slice(&checksum);
    stored
}

pub(super) fn verify(recovery: &Program<'_>, upload: &Program<'_>) -> Result<Image, String> {
    let original = recovery.bytes();
    let edit = upload.bytes();
    for image in [original, edit] {
        if image[CHECKSUM] != sum(image, 1089) || image[CHECKSUM] != sum(image, 1091) {
            return Err("Independent upload checksum disagrees with the validated program".into());
        }
    }
    if stamp(original) != *original || procedures::expected_stored_image(original) != *original {
        return Err("Restore image changes under firmware stamping".into());
    }
    if original[TRIM] != ZERO_DB || original[GAIN] != UNITY_GAIN {
        return Err("Synthetic baseline does not have 0 dB trim and unity DSP gain".into());
    }
    if edit[VERSION] != VERSION_310 && edit[VERSION] != [0; 12] {
        return Err("Synthetic edit has unrelated changes in the upload version words".into());
    }

    let mut expected = *original;
    expected[TRIM].copy_from_slice(&MINUS_SIX_DB);
    expected[GAIN].copy_from_slice(&ATTENUATED_GAIN);
    let expected = stamp(&expected);
    let stored = stamp(edit);
    if let Some(offset) = stored
        .iter()
        .zip(expected)
        .position(|(&actual, want)| actual != want)
    {
        return Err(format!(
            "Synthetic edit has unrelated changes or wrong trim bytes at byte {offset}"
        ));
    }
    if procedures::expected_stored_image(edit) != expected {
        return Err(
            "Shared firmware stamping disagrees with the independent edit prediction".into(),
        );
    }
    Ok(expected)
}

#[cfg(test)]
mod tests {
    use super::super::{TARGET, fixture};
    use super::*;

    fn check(original: &Image, edit: &Image) -> Result<Image, String> {
        let name = "Preset 4";
        verify(&Program::new(name, original)?, &Program::new(name, edit)?)
    }

    #[test]
    fn custom_restore_proof_accepts_exact_edit_with_either_upload_stamp() {
        let original = fixture::fixture_image(TARGET);
        let edit = fixture::fixture_edit(&original);
        let mut unstamped = edit;
        unstamped[VERSION].fill(0);
        let checksum = sum(&unstamped, 1089);
        unstamped[CHECKSUM].copy_from_slice(&checksum);
        assert_eq!(check(&original, &edit).unwrap(), edit);
        assert_ne!(unstamped[CHECKSUM], edit[CHECKSUM]);
        assert_eq!(check(&original, &unstamped).unwrap(), edit);

        let offsets: Vec<_> = original
            .iter()
            .zip(edit)
            .enumerate()
            .filter_map(|(offset, (&old, new))| (old != new).then_some(offset))
            .collect();
        assert_eq!(offsets, [1665, 1666, 1667, 2618, 2619, 3273, 3274, 3275]);
        println!(
            "Synthetic edit changes {} bytes at offsets {offsets:?}; all {} recovery bytes survive stamping.",
            offsets.len(),
            original.len()
        );
    }

    #[test]
    fn custom_restore_proof_refuses_every_single_byte_corruption() {
        let original = fixture::fixture_image(TARGET);
        let edit = fixture::fixture_edit(&original);
        for offset in 0..PROGRAM_BYTES {
            let mut changed = edit;
            changed[offset] ^= 1;
            if !CHECKSUM.contains(&offset) {
                // Keep the firmware sum valid so it cannot hide a comparison
                // failure. The name and the other upload sum still have to pass.
                let checksum = sum(&changed, 1091);
                changed[CHECKSUM].copy_from_slice(&checksum);
            }
            assert!(
                check(&original, &changed).is_err(),
                "Accepted byte {offset}"
            );
        }
        assert!(
            check(&original, &original)
                .unwrap_err()
                .contains("wrong trim")
        );
        println!("Rejected {PROGRAM_BYTES} single-byte corruptions and the unchanged image.");
    }

    #[test]
    fn custom_restore_proof_refuses_changes_that_cancel_in_the_checksum() {
        let original = fixture::fixture_image(TARGET);
        let mut edit = fixture::fixture_edit(&original);
        // Move one count between two words so both upload sums stay valid.
        edit[2] += 1;
        edit[555 * 3 + 2] -= 1;
        let upload = Program::new("Preset 4", &edit).unwrap();
        let recovery = Program::new("Preset 4", &original).unwrap();
        assert!(verify(&recovery, &upload).unwrap_err().contains("byte 2"));
    }

    #[test]
    fn custom_restore_proof_refuses_valid_upload_as_recovery_if_stamping_changes_it() {
        let original = fixture::fixture_image(TARGET);
        let mut unstamped = original;
        unstamped[VERSION].fill(0);
        let checksum = sum(&unstamped, 1089);
        unstamped[CHECKSUM].copy_from_slice(&checksum);
        let edit = fixture::fixture_edit(&original);
        assert_eq!(stamp(&unstamped), original);
        assert_eq!(procedures::expected_stored_image(&unstamped), original);
        assert_eq!(
            check(&unstamped, &edit).unwrap_err(),
            "Restore image changes under firmware stamping"
        );
    }
}
