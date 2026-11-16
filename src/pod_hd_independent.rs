#![forbid(unsafe_code)]
//! Bounds measured from independently published POD HD files.
//!
//! The measurements are not an original POD HD500X Edit result, not a product
//! id, and not a copy of the GPL converter. Nothing here rewrites a file,
//! opens a device, or registers a model.

/// Author page that links the source repository. Fetched 2026-11-16.
pub const AUTHOR_PAGE: &str = "https://www.jzab.de/content/pod-hd-preset-tool/";
/// Commit checked out nowhere. The local clone is a no-checkout git directory.
pub const SOURCE_REPOSITORY: &str = "https://github.com/johanneszab/podhd";
pub const SOURCE_COMMIT: &str = "58e95d5b514b44ae667dd5463ec63c8bafa6735a";
/// `LICENSE` at `SOURCE_COMMIT` is the GNU GPL version 2 text.
pub const SOURCE_LICENSE: &str = "GNU GPL version 2";
/// SHA-256 of `src/mainwindow.cpp` at `SOURCE_COMMIT`.
pub const SOURCE_FILE_SHA256: &str =
    "6656f6a6cfa4304b6a85164f9a3437c6bcf00b631809c82b1b582b77f0dfc597";

pub const PRESET_MAGIC: &[u8; 4] = b"H5EP";
pub const SETLIST_MAGIC: &[u8; 4] = b"H5ES";
pub const OBSERVED_PRESET_LEN: usize = 4136;
pub const OBSERVED_SETLIST_LEN: usize = 264_782;
pub const SETLIST_FIRST_RECORD: usize = 56;
pub const SETLIST_RECORD_COUNT: usize = 64;
pub const SETLIST_STRIDE: usize = 4136;
pub const SETLIST_TAIL_LEN: usize = 22;

/// Offsets named by the source when it assigns the six-byte sequences below.
pub const TUPLE_OFFSETS: [usize; 6] = [0x0b, 0x0c, 0x0d, 0x13, 0x14, 0x15];

/// Six bytes shared by the three directory files. Not a product id.
pub const DIRECTORY_OBSERVED_TUPLE: [u8; 6] = [0x2e, 0x02, 0x26, 0x06, 0x02, 0x62];
/// Six bytes shared by the 64 embedded records. Not a product id.
pub const EMBEDDED_OBSERVED_TUPLE: [u8; 6] = [0x2d, 0x02, 0x26, 0x05, 0x00, 0x00];
/// Values the source writes in the branch whose text is `ProX`.
pub const SOURCE_PROX_WRITE: [u8; 6] = [0x2e, 0x02, 0x26, 0x06, 0x02, 0x62];
/// Values the source writes in the branch whose text is `500X`.
pub const SOURCE_500X_WRITE: [u8; 6] = [0x2d, 0x02, 0x26, 0x05, 0x02, 0x62];

/// A label printed by the source comment at offset 0x0b. `registered` stays false.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommentLabel {
    pub value: u8,
    pub author_text: &'static str,
    pub registered: bool,
}

pub const COMMENT_LABELS: &[CommentLabel] = &[
    CommentLabel {
        value: 0x27,
        author_text: "HD 500",
        registered: false,
    },
    CommentLabel {
        value: 0x2d,
        author_text: "HD 500X",
        registered: false,
    },
    CommentLabel {
        value: 0x28,
        author_text: "HD Desktop",
        registered: false,
    },
    CommentLabel {
        value: 0x29,
        author_text: "HD Pro",
        registered: false,
    },
    CommentLabel {
        value: 0x2e,
        author_text: "HD ProX",
        registered: false,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishedFile {
    pub name: &'static str,
    pub suffix: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
    pub fetched: &'static str,
    pub proves_hd500x: bool,
}

/// Files fetched from the author's HD500X directory. The directory name is not a product id.
pub const PUBLISHED_FILES: &[PublishedFile] = &[
    PublishedFile {
        name: "7S_Das_Pod_L.5xe",
        suffix: ".5xe",
        bytes: 4136,
        sha256: "4e651fe1830952993ff803720a566e76587a367159a4d774ebfd8843edeee797",
        fetched: "Mon, 16 Nov 2026 04:59:41 GMT",
        proves_hd500x: false,
    },
    PublishedFile {
        name: "BS_Clean_L.5xe",
        suffix: ".5xe",
        bytes: 4136,
        sha256: "d13d20d1fca31bbc699fde60be6c7af3ae6e5ed2a9aa25242c480469adb50714",
        fetched: "Mon, 16 Nov 2026 04:59:42 GMT",
        proves_hd500x: false,
    },
    PublishedFile {
        name: "CL_ChmpCln_N.5xe",
        suffix: ".5xe",
        bytes: 4136,
        sha256: "ca1e4f5dc81c2c78ffbe1fcee1878f102f2cf004877d0733f9748f0043691176",
        fetched: "Mon, 16 Nov 2026 04:59:42 GMT",
        proves_hd500x: false,
    },
    PublishedFile {
        name: "MAB_Final.5xs",
        suffix: ".5xs",
        bytes: 264_782,
        sha256: "bcf81c9a495e959129e967c0840fc57ad1714903b20073eb9844b72997e1f400",
        fetched: "Mon, 16 Nov 2026 04:59:42 GMT",
        proves_hd500x: false,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetlistMeasure {
    pub record_count: usize,
    pub stride: usize,
    pub first_record: usize,
    pub tail_len: usize,
    pub header_byte_0x17: u8,
    pub uniform_record_byte_0x0b: Option<u8>,
}

/// Reports the observed setlist span. It does not name a product.
pub fn measure_observed_setlist(bytes: &[u8]) -> Result<SetlistMeasure, &'static str> {
    if bytes.len() != OBSERVED_SETLIST_LEN {
        return Err("length");
    }
    if bytes.get(..4) != Some(SETLIST_MAGIC.as_slice()) {
        return Err("magic");
    }
    let mut count = 0usize;
    let mut first = None;
    let mut previous = None;
    let mut uniform = None;
    let mut index = 0usize;
    while index + PRESET_MAGIC.len() <= bytes.len() {
        if &bytes[index..index + PRESET_MAGIC.len()] == PRESET_MAGIC {
            if let Some(prev) = previous {
                if index - prev != SETLIST_STRIDE {
                    return Err("stride");
                }
            } else {
                first = Some(index);
            }
            let value = *bytes.get(index + 0x0b).ok_or("record byte")?;
            uniform = match uniform {
                None => Some(value),
                Some(seen) if seen == value => Some(seen),
                Some(_) => return Err("mixed record byte"),
            };
            count += 1;
            previous = Some(index);
        }
        index += 1;
    }
    if count != SETLIST_RECORD_COUNT || first != Some(SETLIST_FIRST_RECORD) {
        return Err("record placement");
    }
    let occupied = SETLIST_FIRST_RECORD + SETLIST_RECORD_COUNT * SETLIST_STRIDE;
    let tail_len = bytes.len().checked_sub(occupied).ok_or("tail")?;
    if tail_len != SETLIST_TAIL_LEN {
        return Err("tail");
    }
    Ok(SetlistMeasure {
        record_count: count,
        stride: SETLIST_STRIDE,
        first_record: SETLIST_FIRST_RECORD,
        tail_len,
        header_byte_0x17: bytes[0x17],
        uniform_record_byte_0x0b: uniform,
    })
}

/// Byte 0x0b of one observed-length H5EP buffer. The value is not a product id.
pub fn observed_preset_byte_0x0b(bytes: &[u8]) -> Result<u8, &'static str> {
    if bytes.len() != OBSERVED_PRESET_LEN {
        return Err("length");
    }
    if bytes.get(..4) != Some(PRESET_MAGIC.as_slice()) {
        return Err("magic");
    }
    Ok(bytes[0x0b])
}

#[cfg(test)]
mod tests {
    use super::{
        COMMENT_LABELS, DIRECTORY_OBSERVED_TUPLE, EMBEDDED_OBSERVED_TUPLE, OBSERVED_PRESET_LEN,
        OBSERVED_SETLIST_LEN, PRESET_MAGIC, PUBLISHED_FILES, SETLIST_FIRST_RECORD, SETLIST_MAGIC,
        SETLIST_RECORD_COUNT, SETLIST_STRIDE, SETLIST_TAIL_LEN, SOURCE_500X_WRITE, SOURCE_COMMIT,
        SOURCE_FILE_SHA256, SOURCE_PROX_WRITE, measure_observed_setlist, observed_preset_byte_0x0b,
    };
    use crate::pod_hd500x;
    use crate::rig::{Operation, ProductProfile, Rejection, RoleKind, Session};

    #[test]
    fn independent_bounds_do_not_register_a_product() {
        assert_eq!(
            SETLIST_FIRST_RECORD + SETLIST_RECORD_COUNT * SETLIST_STRIDE + SETLIST_TAIL_LEN,
            OBSERVED_SETLIST_LEN
        );
        assert_eq!(DIRECTORY_OBSERVED_TUPLE, SOURCE_PROX_WRITE);
        assert_ne!(EMBEDDED_OBSERVED_TUPLE, SOURCE_500X_WRITE);
        assert_eq!(EMBEDDED_OBSERVED_TUPLE[4..], [0x00, 0x00]);
        assert!(COMMENT_LABELS.iter().all(|label| !label.registered));
        assert!(PUBLISHED_FILES.iter().all(|file| !file.proves_hd500x));
        let record = pod_hd500x::record();
        assert!(!record.preset_container_recovered);
        assert!(record.file_product_id.is_none());
        assert!(!record.model_ids_recovered);
        assert!(!record.effect_ids_recovered);

        let mut preset = vec![0u8; OBSERVED_PRESET_LEN];
        preset[..4].copy_from_slice(PRESET_MAGIC);
        preset[0x0b] = 0x2e;
        assert_eq!(observed_preset_byte_0x0b(&preset), Ok(0x2e));

        let mut setlist = vec![0u8; OBSERVED_SETLIST_LEN];
        setlist[..4].copy_from_slice(SETLIST_MAGIC);
        setlist[0x17] = 0x2d;
        for index in 0..SETLIST_RECORD_COUNT {
            let at = SETLIST_FIRST_RECORD + index * SETLIST_STRIDE;
            setlist[at..at + 4].copy_from_slice(PRESET_MAGIC);
            setlist[at + 0x0b] = 0x2d;
        }
        let measured = measure_observed_setlist(&setlist).expect("synthetic span");
        assert_eq!(measured.record_count, 64);
        assert_eq!(measured.stride, 4136);
        assert_eq!(measured.first_record, 56);
        assert_eq!(measured.tail_len, 22);
        assert_eq!(measured.header_byte_0x17, 0x2d);
        assert_eq!(measured.uniform_record_byte_0x0b, Some(0x2d));
        assert_ne!(measured.header_byte_0x17, setlist[0x0b]);
        setlist.pop();
        assert!(measure_observed_setlist(&setlist).is_err());

        let session = Session::extension(ProductProfile::PodHd500x).expect("extension");
        assert_eq!(
            session.open_pod_preset(RoleKind::Pod, &preset),
            Err(Rejection::Unverified {
                profile: ProductProfile::PodHd500x,
                operation: Operation::OpenPodPreset,
            })
        );
        let evidence = include_str!("../docs/pod-hd500x.md");
        assert!(evidence.contains(SOURCE_COMMIT));
        assert!(evidence.contains(SOURCE_FILE_SHA256));
        for file in PUBLISHED_FILES {
            assert!(evidence.contains(file.sha256));
        }
        assert!(evidence.contains("The three directory files have byte 0x0b equal to 0x2e."));
        assert!(evidence.contains("Neither label is registered."));
        assert!(
            evidence.contains(
                "The directory name and the .5xe suffix do not prove a POD HD500X product."
            )
        );
    }
}
