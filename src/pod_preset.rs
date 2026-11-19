#![forbid(unsafe_code)]
//! Offline PODxt Live tone files.
//!
//! The container is the shared Workbench IFF patch, not a Variax model program
//! and not a firmware resource-directory or region-17 record. `L6ToneFileIFFMap`
//! sends FORM/L6PA, FORM/L6FS, and FORM/L6AS to `L6Patch`. `L6ProductConstants.PODXT_LIVE`
//! is 196618 (`0x0003000A`). `L6Patch.UpdateMetaAmpName` reads model slot 196608
//! (`0x00030000`) for a Sharc tone. `UpdateMetaNotesForSetup` reads slot 196609
//! (`0x00030001`) only when the patch type is 2 (L6AS). Asset names are the
//! PODxt Live `GetAssetName` results in `pod_assets`. A 12-byte MINF carries
//! the signed ordinal, two raw reserved bytes, and `m_enabled`. `L6Buffer.Read8`
//! returns a Java signed byte on both mac and Windows. `L6Model` stores that
//! byte in `int m_ordinal` and sets enabled from `Read8() > 0`, so bytes
//! `0x80` through `0xFF` are negative and disabled. The file bytes stay as
//! stored. A 12-byte PARM carries an id, a value type, and raw bits. No
//! parameter range was recovered, and this module does not edit those values.
//! Other bytes are preserved.
//!
//! The MIDI hardware id `L6HW_ID_PODXTLIVE` (18000) is a different namespace.
//! The archived firmware version `0x03010000` is not a requirement of a tone file.

use crate::document;
use crate::pod_assets::{self, SlotRole};
use crate::preset;
use std::io::{self, Read};
use std::path::Path;

/// PINF target for PODxt Live. Equal to `L6ProductConstants.PODXT_LIVE`.
pub const PODXT_LIVE_TONE_ID: u32 = 0x0003_000a;
/// Amp model slot from `L6Patch.UpdateMetaAmpName` and `kGPXTSlot_Amp`.
pub const AMP_SLOT: u32 = 0x0003_0000;
/// Cab model slot from `L6Patch.UpdateMetaNotesForSetup` and `kGPXTSlot_Cab`.
/// Only an L6AS tone exposes it as the cab model.
pub const CAB_SLOT: u32 = 0x0003_0001;
/// Application bound for the offline librarian. Not a recovered Line 6 limit.
pub const TONE_BYTES_CEILING: u64 = 1_048_576;

/// Static Edit 2.27 file framing. The tag is an HD500X editor-group
/// observation; it does not prove a physical target or profile compatibility.
pub const H5EP_HEADER_BYTES: usize = 0x28;
pub const H5EP_BODY_BYTES: usize = 0x1000;
pub const H5EP_MIN_BYTES: usize = H5EP_HEADER_BYTES + H5EP_BODY_BYTES;
pub const H5ES_HEADER_BYTES: usize = 0x38;
pub const H5ES_PRESET_COUNT: usize = 64;
pub const H5ES_MIN_BYTES: usize = H5ES_HEADER_BYTES + H5ES_PRESET_COUNT * H5EP_MIN_BYTES;
pub const H5EB_HEADER_BYTES: usize = 0x28;
pub const H5EB_SETLIST_COUNT: usize = 8;
pub const H5EB_MIN_BYTES: usize = H5EB_HEADER_BYTES + H5EB_SETLIST_COUNT * H5ES_MIN_BYTES;
const HD_PRESET_BYTES_CEILING: usize = 1_048_576;
const HD_SETLIST_BYTES_CEILING: usize = 1_048_576;
pub const HD_BUNDLE_BYTES_CEILING: usize = 3 * 1024 * 1024;
const HD_EFFECT_BLOCK_OFFSET: usize = 0x428;
const HD_EFFECT_BLOCK_STRIDE: usize = 0x100;
const HD_EFFECT_COUNT: usize = 8;
const HD_EFFECT_ROWS_OFFSET: usize = 0x10;
const HD_PARAMETER_ROW_STRIDE: usize = 0x14;
const HD_PARAMETER_ROW_FLAG_OFFSET: usize = 0x10;
const HD_PARAMETER_ROW_COUNT: usize = 12;
const HD_SCREAMER_MODEL_ID: u32 = 0x0205_000d;
const HD_SCREAMER_DRIVE_KEY: u32 = 0x3f10_0001;
const HD_SCREAMER_BASS_KEY: u32 = 0x3f10_0002;
const HD_SCREAMER_MID_KEY: u32 = 0x3f10_0003;
const HD_SCREAMER_TREBLE_KEY: u32 = 0x3f10_0004;
const HD_SCREAMER_OUTPUT_KEY: u32 = 0x3f10_0005;

pub const OFFLINE_BANNER: &str =
    "Offline PODxt Live tone file. This does not edit a connected POD.";
pub const HD_EDITOR_OFFLINE_NOTICE: &str = "HD Edit files are handled offline. Internal target and physical compatibility remain unverified.";
pub const NOT_GUITAR: &str =
    "This is not the Variax guitar editor. Guitar model slots are not applied.";
pub const NOT_SENT: &str = "Nothing is sent to a device. No preset is switched.";
pub const PAYLOADS_NOT_DECODED: &str = "Known asset names and 12-byte parameter records are shown. Parameter ranges are not decoded. Unknown ids stay raw.";

const VARIAX_FAMILY_MASK: u32 = 0xffff_0000;
const VARIAX_FAMILY_HIGH: u32 = 0x0007_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    /// `L6Patch` type 0.
    L6pa,
    /// `L6Patch` type 1.
    L6fs,
    /// `L6Patch` type 2. The only container that exposes the cab slot.
    L6as,
}

impl ContainerKind {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::L6pa => "L6PA",
            Self::L6fs => "L6FS",
            Self::L6as => "L6AS",
        }
    }

    pub const fn exposes_cab(self) -> bool {
        matches!(self, Self::L6as)
    }

    fn from_bytes(tag: &[u8]) -> Option<Self> {
        match tag {
            b"L6PA" => Some(Self::L6pa),
            b"L6FS" => Some(Self::L6fs),
            b"L6AS" => Some(Self::L6as),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToneError {
    Format { detail: String },
    WrongTarget { device_id: u32 },
}

/// A POD-family file recognized by its byte structure, without implying that
/// its format is supported by a particular product profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PodPresetFile {
    PodxtLive(PodTone),
    H5ep(OpaqueH5epPreset),
    H5es(H5esSetlist),
    H5eb(H5ebBundle),
}

/// A statically framed H5EP record. Header words other than magic/version and
/// all body data remain raw unless a narrowly evidenced control is edited.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueH5epPreset {
    bytes: Vec<u8>,
}

/// A complete editor setlist with exactly 64 fixed-size H5EP children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct H5esSetlist {
    bytes: Vec<u8>,
}

/// A complete editor bundle with exactly eight fixed-size H5ES children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct H5ebBundle {
    bytes: Vec<u8>,
}

/// Screamer controls whose Editor 2.27 descriptor, numeric widget, and keyed
/// float write path are statically joined. This describes editor behavior,
/// not availability on a physical device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreamerControl {
    Drive,
    Bass,
    Mid,
    Treble,
    Output,
}

impl ScreamerControl {
    pub const ALL: [Self; 5] = [
        Self::Drive,
        Self::Bass,
        Self::Mid,
        Self::Treble,
        Self::Output,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Drive => "DRIVE",
            Self::Bass => "BASS",
            Self::Mid => "MID",
            Self::Treble => "TREBLE",
            Self::Output => "OUTPUT",
        }
    }

    const fn key(self) -> u32 {
        match self {
            Self::Drive => HD_SCREAMER_DRIVE_KEY,
            Self::Bass => HD_SCREAMER_BASS_KEY,
            Self::Mid => HD_SCREAMER_MID_KEY,
            Self::Treble => HD_SCREAMER_TREBLE_KEY,
            Self::Output => HD_SCREAMER_OUTPUT_KEY,
        }
    }
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

impl OpaqueH5epPreset {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_PRESET_BYTES_CEILING {
            return Err(ToneError::format(
                "H5EP exceeds the offline file-size ceiling",
            ));
        }
        validate_h5ep_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn header_bytes(&self) -> &[u8] {
        &self.bytes[..H5EP_HEADER_BYTES]
    }

    pub fn body_bytes(&self) -> &[u8] {
        &self.bytes[H5EP_HEADER_BYTES..H5EP_MIN_BYTES]
    }

    /// Returns the editor's in-memory byte order using the statically traced
    /// selective word transform. Unmapped bytes are copied verbatim.
    pub fn body_in_editor_order(&self) -> Vec<u8> {
        let mut body = self.body_bytes().to_vec();
        transform_hd_body_words(&mut body);
        body
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5EP_MIN_BYTES..]
    }

    /// The Edit 2.27 static source maps this control to a normalized float in
    /// the first matching effect row. The result is editor data, not a claim
    /// of physical-device availability.
    pub fn screamer_drive_percent(&self, effect_slot: usize) -> Option<u8> {
        self.screamer_control_percent(effect_slot, ScreamerControl::Drive)
    }

    /// Reads a statically joined Screamer type-1 control from its first keyed
    /// row when the row assignment state is admitted.
    pub fn screamer_control_percent(
        &self,
        effect_slot: usize,
        control: ScreamerControl,
    ) -> Option<u8> {
        let value_at = self.screamer_control_value_offset(effect_slot, control)?;
        let value = f32::from_bits(read_be_u32(self.body_bytes(), value_at)?);
        exact_drive_percent(value)
    }

    /// Edits only the first statically identified Screamer DRIVE value word.
    /// Unknown rows, duplicates and every other source byte remain unchanged.
    pub fn set_screamer_drive_percent(
        &mut self,
        effect_slot: usize,
        percent: u8,
    ) -> Result<(), ToneError> {
        self.set_screamer_control_percent(effect_slot, ScreamerControl::Drive, percent)
    }

    /// Edits only the first matching keyed value for an admitted Screamer
    /// control. All unrelated rows and bytes are preserved.
    pub fn set_screamer_control_percent(
        &mut self,
        effect_slot: usize,
        control: ScreamerControl,
        percent: u8,
    ) -> Result<(), ToneError> {
        if percent > 100 {
            return Err(ToneError::format(format!(
                "Screamer {} must be in 0..=100",
                control.label()
            )));
        }
        let value_at = self
            .screamer_control_value_offset(effect_slot, control)
            .ok_or_else(|| {
                ToneError::format(
                    "Screamer control slot, key, or assignment state is not admitted for editing",
                )
            })?;
        let current = f32::from_bits(
            read_be_u32(self.body_bytes(), value_at)
                .ok_or_else(|| ToneError::format("Screamer DRIVE row is truncated"))?,
        );
        exact_drive_percent(current).ok_or_else(|| {
            ToneError::format(
                "existing Screamer control value is outside the exact recovered editor encoding",
            )
        })?;
        let file_at = H5EP_HEADER_BYTES + value_at;
        self.bytes[file_at..file_at + 4]
            .copy_from_slice(&(percent as f32 / 100.0).to_bits().to_be_bytes());
        Ok(())
    }

    /// Exact no-op serialization. The envelope's unknown fields remain raw.
    pub fn serialize_noop(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }

    fn screamer_control_value_offset(
        &self,
        effect_slot: usize,
        control: ScreamerControl,
    ) -> Option<usize> {
        if effect_slot >= HD_EFFECT_COUNT {
            return None;
        }
        let body = self.body_bytes();
        let block = HD_EFFECT_BLOCK_OFFSET + effect_slot * HD_EFFECT_BLOCK_STRIDE;
        if read_be_u32(body, block)? != HD_SCREAMER_MODEL_ID {
            return None;
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = block + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            if read_be_u32(body, row_at)? == control.key() {
                // The source sends the row flag into widget assignment state,
                // while the callback suppresses writes for expression controls.
                // Only the zero-flag state is admitted; auxiliary flag bytes
                // remain opaque and are never normalized.
                if body[row_at + HD_PARAMETER_ROW_FLAG_OFFSET] != 0 {
                    return None;
                }
                return Some(row_at + 4);
            }
        }
        None
    }
}

impl H5esSetlist {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_SETLIST_BYTES_CEILING {
            return Err(ToneError::format(
                "H5ES exceeds the offline file-size ceiling",
            ));
        }
        validate_h5es_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5ES_MIN_BYTES..]
    }

    pub fn preset_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= H5ES_PRESET_COUNT {
            return None;
        }
        let start = H5ES_HEADER_BYTES + index * H5EP_MIN_BYTES;
        self.bytes.get(start..start + H5EP_MIN_BYTES)
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

impl H5ebBundle {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() > HD_BUNDLE_BYTES_CEILING {
            return Err(ToneError::format(
                "H5EB exceeds the offline file-size ceiling",
            ));
        }
        validate_h5eb_record(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.bytes[H5EB_MIN_BYTES..]
    }

    pub fn setlist_bytes(&self, index: usize) -> Option<&[u8]> {
        if index >= H5EB_SETLIST_COUNT {
            return None;
        }
        let start = H5EB_HEADER_BYTES + index * H5ES_MIN_BYTES;
        self.bytes.get(start..start + H5ES_MIN_BYTES)
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredHdFile {
    file: PodPresetFile,
    saved: Vec<u8>,
}

/// Shared in-memory library for statically framed HD Edit files. Imported
/// containers and their source bytes are retained independently; extracting a
/// child creates a separate preset entry and never rewrites its parent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HdEditorLibrary {
    files: Vec<StoredHdFile>,
}

impl HdEditorLibrary {
    pub fn import_bytes(&mut self, bytes: Vec<u8>) -> Result<usize, ToneError> {
        let file = PodPresetFile::parse(bytes)?;
        if file.hd_container_kind().is_none() {
            return Err(ToneError::format(
                "HD editor library accepts H5EP, H5ES, and H5EB files only",
            ));
        }
        let saved = file.as_bytes().to_vec();
        self.files.push(StoredHdFile { file, saved });
        Ok(self.files.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn file(&self, index: usize) -> Option<&PodPresetFile> {
        self.files.get(index).map(|entry| &entry.file)
    }

    pub fn file_mut(&mut self, index: usize) -> Option<&mut PodPresetFile> {
        self.files.get_mut(index).map(|entry| &mut entry.file)
    }

    pub fn extract_preset(
        &mut self,
        index: usize,
        setlist: usize,
        preset: usize,
    ) -> Result<usize, ToneError> {
        let child = match self.file(index) {
            Some(PodPresetFile::H5ep(_)) if setlist == 0 && preset == 0 => {
                return Ok(index);
            }
            Some(PodPresetFile::H5es(file)) if setlist == 0 => file.preset_bytes(preset),
            Some(PodPresetFile::H5eb(file)) => file.setlist_bytes(setlist).and_then(|bytes| {
                let start = H5ES_HEADER_BYTES + preset.checked_mul(H5EP_MIN_BYTES)?;
                bytes.get(start..start.checked_add(H5EP_MIN_BYTES)?)
            }),
            _ => None,
        }
        .ok_or_else(|| ToneError::format("setlist or preset index is outside the file"))?
        .to_vec();
        let file = PodPresetFile::H5ep(OpaqueH5epPreset::parse(child)?);
        let saved = file.as_bytes().to_vec();
        self.files.push(StoredHdFile { file, saved });
        Ok(self.files.len() - 1)
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.files
            .get(index)
            .is_some_and(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn any_dirty(&self) -> bool {
        self.files
            .iter()
            .any(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn dirty_count(&self) -> usize {
        self.files
            .iter()
            .filter(|entry| entry.file.as_bytes() != entry.saved)
            .count()
    }

    pub fn first_dirty(&self) -> Option<usize> {
        self.files
            .iter()
            .position(|entry| entry.file.as_bytes() != entry.saved)
    }

    pub fn note_saved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        entry.saved = entry.file.as_bytes().to_vec();
        Ok(())
    }

    pub fn revert_unsaved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .files
            .get_mut(index)
            .ok_or_else(|| ToneError::format("file index is outside the HD editor library"))?;
        if entry.file.as_bytes() != entry.saved {
            entry.file = PodPresetFile::parse(entry.saved.clone())?;
        }
        Ok(())
    }
}

fn validate_h5ep_record(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5EP_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5EP record: expected at least {H5EP_MIN_BYTES} bytes"
        )));
    }
    if &bytes[..4] != b"H5EP" {
        return Err(ToneError::format("expected H5EP preset magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5EP version"));
    }
    validate_hd_body(&bytes[H5EP_HEADER_BYTES..H5EP_MIN_BYTES])
}

fn validate_h5es_header(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5ES_HEADER_BYTES {
        return Err(ToneError::format("truncated H5ES header"));
    }
    if &bytes[..4] != b"H5ES" {
        return Err(ToneError::format("expected H5ES setlist magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5ES version"));
    }
    Ok(())
}

fn validate_h5es_record(bytes: &[u8]) -> Result<(), ToneError> {
    validate_h5es_header(bytes)?;
    if bytes.len() < H5ES_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5ES children: expected at least {H5ES_MIN_BYTES} bytes"
        )));
    }
    for index in 0..H5ES_PRESET_COUNT {
        let start = H5ES_HEADER_BYTES + index * H5EP_MIN_BYTES;
        validate_h5ep_record(&bytes[start..start + H5EP_MIN_BYTES])?;
    }
    Ok(())
}

fn validate_h5eb_record(bytes: &[u8]) -> Result<(), ToneError> {
    if bytes.len() < H5EB_HEADER_BYTES {
        return Err(ToneError::format("truncated H5EB header"));
    }
    if &bytes[..4] != b"H5EB" {
        return Err(ToneError::format("expected H5EB bundle magic"));
    }
    if bytes[4..8] != 1u32.to_be_bytes() {
        return Err(ToneError::format("unsupported H5EB version"));
    }
    if bytes.len() < H5EB_MIN_BYTES {
        return Err(ToneError::format(format!(
            "truncated H5EB children: expected at least {H5EB_MIN_BYTES} bytes"
        )));
    }
    for index in 0..H5EB_SETLIST_COUNT {
        let start = H5EB_HEADER_BYTES + index * H5ES_MIN_BYTES;
        validate_h5es_record(&bytes[start..start + H5ES_MIN_BYTES])?;
    }
    Ok(())
}

fn validate_hd_body(body: &[u8]) -> Result<(), ToneError> {
    if body.len() != H5EP_BODY_BYTES {
        return Err(ToneError::format("invalid H5EP body length"));
    }
    for index in 0..2 {
        if body[0x028 + index * HD_EFFECT_BLOCK_STRIDE] != 0 {
            return Err(ToneError::format("invalid H5EP amp model class"));
        }
        if body[0x228 + index * HD_EFFECT_BLOCK_STRIDE] != 1 {
            return Err(ToneError::format("invalid H5EP cab model class"));
        }
    }
    let mut loop_count = 0;
    for index in 0..HD_EFFECT_COUNT {
        let offset = HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE;
        if body[offset] != 2 {
            return Err(ToneError::format("invalid H5EP effect model class"));
        }
        if read_be_u32(body, offset) == Some(0x0208_0000) {
            loop_count += 1;
        }
    }
    if loop_count >= 2 {
        return Err(ToneError::format("H5EP contains two FX Loop models"));
    }
    for (offset, maximum, field) in [
        (0xff3, 1, "TrailsEnabled"),
        (0xff6, 1, "AmpControl"),
        (0xff8, 7, "Mic[0]"),
        (0xff9, 7, "Mic[1]"),
        (0xffa, 1, "MainMute[0]"),
        (0xffb, 1, "MainMute[1]"),
        (0xffe, 10, "Input1Source"),
        (0xfff, 10, "Input2Source"),
    ] {
        if body[offset] > maximum {
            return Err(ToneError::format(format!("invalid H5EP {field} value")));
        }
    }
    Ok(())
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn exact_drive_percent(value: f32) -> Option<u8> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    let percent = (value * 100.0).round() as u8;
    ((percent as f32 / 100.0).to_bits() == value.to_bits()).then_some(percent)
}

fn transform_hd_body_words(body: &mut [u8]) {
    const FIXED_WORDS: [usize; 9] = [0x20, 0x24, 0xd48, 0xd4c, 0xd50, 0xd54, 0xd58, 0xd7c, 0xd80];
    for offset in FIXED_WORDS {
        body[offset..offset + 4].reverse();
    }
    const BLOCKS: [usize; 4] = [0x28, 0x128, 0x228, 0x328];
    for offset in BLOCKS.into_iter().chain(
        (0..HD_EFFECT_COUNT).map(|index| HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE),
    ) {
        for word in 0..2 {
            let at = offset + word * 4;
            body[at..at + 4].reverse();
        }
        for row in 0..HD_PARAMETER_ROW_COUNT {
            let row_at = offset + HD_EFFECT_ROWS_OFFSET + row * HD_PARAMETER_ROW_STRIDE;
            for word in 0..4 {
                let at = row_at + word * 4;
                body[at..at + 4].reverse();
            }
        }
    }
}

impl ToneError {
    fn format(detail: impl Into<String>) -> Self {
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

/// Value stored in a 12-byte PARM record. The bits are not a knob position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterValue {
    /// `L6ModelParam` value type 0. The bits are the Java `int`.
    Int { bits: u32 },
    /// `L6ModelParam` value type 1. The bits are an IEEE-754 `float`.
    Float { bits: u32 },
    /// Any other type word. Both words stay raw.
    Other { type_word: u32, bits: u32 },
}

/// One PARM chunk. Only the 12-byte `L6ModelParam` record is structured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelParameter {
    Recorded {
        /// High byte of the stored first word. A write uses `0x3F`. A read keeps any byte.
        prefix: u8,
        /// Low 24 bits of the first word.
        id: u32,
        value: ParameterValue,
    },
    /// A PARM chunk whose length is not 12. Its bytes stay in the tone file.
    Opaque { len: usize },
}

/// One model slot copied from MINF. Bytes outside the decoded fields stay in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelSlot {
    pub slot: u32,
    pub model_id: u32,
    /// MINF byte 8 as the Java signed byte stored in `int m_ordinal`.
    /// `0x80` is -128 and `0xFF` is -1. The file byte is not rewritten.
    pub ordinal: Option<i8>,
    /// MINF bytes 9 and 10 when the chunk is at least 12 bytes. Not rewritten.
    pub reserved: Option<[u8; 2]>,
    /// MINF byte 11 as `Read8() > 0`. A byte from `0x80` through `0xFF` is not
    /// enabled. This is `m_enabled`, not a bypass flag, and it is not rewritten.
    pub enabled: Option<bool>,
    pub parameters: Vec<ModelParameter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodTone {
    bytes: Vec<u8>,
    name_offset: usize,
    name: String,
    device_id: u32,
    device_version: u32,
    container: ContainerKind,
    models: Vec<ModelSlot>,
}

impl ParameterValue {
    fn label(self) -> String {
        match self {
            Self::Int { bits } => format!("int {bits:08x}"),
            Self::Float { bits } => format!("float {bits:08x}"),
            Self::Other { type_word, bits } => format!("type {type_word:08x} bits {bits:08x}"),
        }
    }
}

fn read_parameters(bytes: &[u8], fields: &[preset::Chunk]) -> Vec<ModelParameter> {
    fields
        .iter()
        .filter(|chunk| chunk.id == *b"PARM")
        .map(|chunk| {
            if chunk.data.len() != 12 {
                return ModelParameter::Opaque {
                    len: chunk.data.len(),
                };
            }
            let word0 = preset::word(bytes, chunk.data.start);
            let type_word = preset::word(bytes, chunk.data.start + 4);
            let bits = preset::word(bytes, chunk.data.start + 8);
            let value = match type_word {
                0 => ParameterValue::Int { bits },
                1 => ParameterValue::Float { bits },
                other => ParameterValue::Other {
                    type_word: other,
                    bits,
                },
            };
            ModelParameter::Recorded {
                prefix: (word0 >> 24) as u8,
                id: word0 & 0x00ff_ffff,
                value,
            }
        })
        .collect()
}

impl PodTone {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ToneError> {
        if bytes.len() as u64 > TONE_BYTES_CEILING {
            return Err(ToneError::format(
                "tone file exceeds the 1 MiB offline librarian ceiling",
            ));
        }
        if bytes.len() < 12 || &bytes[..4] != b"FORM" {
            return Err(ToneError::format(
                "expected an individual FORM tone; banks and collections are not POD presets",
            ));
        }
        let Some(container) = ContainerKind::from_bytes(&bytes[8..12]) else {
            return Err(ToneError::format(
                "expected FORM/L6PA, FORM/L6FS, or FORM/L6AS",
            ));
        };
        if preset::word(&bytes, 4) as usize != bytes.len() - 8 {
            return Err(ToneError::format("FORM length does not match the file"));
        }
        let children = preset::chunks(&bytes, 12..bytes.len()).map_err(ToneError::format)?;
        let info = preset::unique(&children, b"PINF").map_err(ToneError::format)?;
        if info.data.len() < 76 {
            return Err(ToneError::format("truncated PINF chunk"));
        }
        let start = info.data.start;
        if preset::word(&bytes, start) != 1 {
            return Err(ToneError::format(
                "unsupported PINF version; the shared layout uses version word 1",
            ));
        }
        let device_id = preset::word(&bytes, start + 4);
        if device_id != PODXT_LIVE_TONE_ID {
            return Err(ToneError::WrongTarget { device_id });
        }
        let name_offset = start + 8;
        let name = utf16_name(&bytes[name_offset..name_offset + 64])?;
        let device_version = preset::word(&bytes, start + 72);
        let patch_containers: Vec<_> = children
            .iter()
            .filter(|chunk| preset::container(&bytes, chunk, b"PATC"))
            .collect();
        if patch_containers.len() != 1 {
            return Err(ToneError::format("expected exactly one PATC container"));
        }
        let patch = patch_containers[0];
        let model_chunks = preset::chunks(&bytes, patch.data.start + 4..patch.data.end)
            .map_err(ToneError::format)?;
        let mut models = Vec::new();
        for entry in model_chunks
            .iter()
            .filter(|chunk| preset::container(&bytes, chunk, b"MODL"))
        {
            let fields = preset::chunks(&bytes, entry.data.start + 4..entry.data.end)
                .map_err(ToneError::format)?;
            let minf = preset::unique(&fields, b"MINF").map_err(ToneError::format)?;
            if minf.data.len() < 8 {
                return Err(ToneError::format("truncated MINF chunk"));
            }
            let slot = preset::word(&bytes, minf.data.start + 4);
            if models.iter().any(|model: &ModelSlot| model.slot == slot) {
                return Err(ToneError::format(format!(
                    "ambiguous duplicate model slot {slot:08x}"
                )));
            }
            let (ordinal, reserved, enabled) = if minf.data.len() >= 12 {
                let ordinal_byte = bytes[minf.data.start + 8];
                let enabled_byte = bytes[minf.data.start + 11];
                (
                    Some(ordinal_byte as i8),
                    Some([bytes[minf.data.start + 9], bytes[minf.data.start + 10]]),
                    Some((enabled_byte as i8) > 0),
                )
            } else {
                (None, None, None)
            };
            models.push(ModelSlot {
                slot,
                model_id: preset::word(&bytes, minf.data.start),
                ordinal,
                reserved,
                enabled,
                parameters: read_parameters(&bytes, &fields),
            });
        }
        Ok(Self {
            bytes,
            name_offset,
            name,
            device_id,
            device_version,
            container,
            models,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    pub fn device_version(&self) -> u32 {
        self.device_version
    }

    pub fn container(&self) -> ContainerKind {
        self.container
    }

    pub fn models(&self) -> &[ModelSlot] {
        &self.models
    }

    /// Amp model id when slot `0x00030000` is present.
    pub fn amp_model_id(&self) -> Option<u32> {
        self.models
            .iter()
            .find(|model| model.slot == AMP_SLOT)
            .map(|model| model.model_id)
    }

    /// Asset name for the amp model id, when the PODxt Live table has one.
    pub fn amp_asset_name(&self) -> Option<&'static str> {
        self.amp_model_id().and_then(pod_assets::asset_name)
    }

    /// Cab model id. Only L6AS exposes slot `0x00030001` as a cab.
    pub fn cab_model_id(&self) -> Option<u32> {
        if !self.container.exposes_cab() {
            return None;
        }
        self.models
            .iter()
            .find(|model| model.slot == CAB_SLOT)
            .map(|model| model.model_id)
    }

    /// Asset name for the exposed cab model id.
    pub fn cab_asset_name(&self) -> Option<&'static str> {
        self.cab_model_id().and_then(pod_assets::asset_name)
    }

    /// Slots whose id is not in `kGPXTSlot_*`.
    pub fn undecoded_slots(&self) -> usize {
        self.models
            .iter()
            .filter(|model| SlotRole::from_id(model.slot).is_none())
            .count()
    }

    /// PARM chunks whose length is the 12-byte record. Their ranges are still unknown.
    pub fn recorded_parameter_chunks(&self) -> usize {
        self.models
            .iter()
            .flat_map(|model| &model.parameters)
            .filter(|parameter| matches!(parameter, ModelParameter::Recorded { .. }))
            .count()
    }

    /// PARM chunks that are not the 12-byte record.
    pub fn undecoded_parameter_chunks(&self) -> usize {
        self.models
            .iter()
            .flat_map(|model| &model.parameters)
            .filter(|parameter| matches!(parameter, ModelParameter::Opaque { .. }))
            .count()
    }

    /// One line per model, then one line per parameter. This does not edit the file.
    pub fn control_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for model in &self.models {
            lines.push(pod_assets::describe_model(
                model.slot,
                model.model_id,
                model.ordinal,
                model.reserved,
                model.enabled,
            ));
            for parameter in &model.parameters {
                lines.push(match parameter {
                    ModelParameter::Opaque { len } => format!("opaque parameter {len} bytes"),
                    ModelParameter::Recorded { prefix, id, value } => {
                        pod_assets::describe_parameter(*prefix, *id, &value.label())
                    }
                });
            }
        }
        lines
    }

    /// Hex id, with the asset name in front when the table has one.
    pub fn model_label(id: Option<u32>) -> String {
        match id {
            Some(id) => match pod_assets::asset_name(id) {
                Some(name) => format!("{name} {id:08x}"),
                None => format!("{id:08x}"),
            },
            None => "absent".into(),
        }
    }

    /// Replace the fixed 32 UTF-16BE PINF name units. No other byte changes.
    pub fn rename(&mut self, name: &str) -> Result<(), ToneError> {
        let units: Vec<u16> = name.encode_utf16().collect();
        if units.len() > 32 || units.contains(&0) {
            return Err(ToneError::format(
                "name must contain at most 32 UTF-16 code units and no NUL",
            ));
        }
        self.bytes[self.name_offset..self.name_offset + 64].fill(0);
        for (index, unit) in units.iter().enumerate() {
            let at = self.name_offset + 2 * index;
            self.bytes[at..at + 2].copy_from_slice(&unit.to_be_bytes());
        }
        self.name = name.to_owned();
        Ok(())
    }

    pub fn save_copy(&self, path: &Path) -> Result<(), ToneError> {
        document::save_new(path, &self.bytes).map_err(ToneError::format)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredTone {
    tone: PodTone,
    /// Bytes last imported or successfully saved. Compared with the applied tone.
    saved: Vec<u8>,
}

/// In-memory list of individual tone files. This is not a bank format.
///
/// Each tone keeps the bytes last imported or successfully saved. An applied
/// edit that differs from that baseline is unsaved. Saving or discarding one
/// tone does not change another tone's baseline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Library {
    tones: Vec<StoredTone>,
}

impl Library {
    pub fn import_bytes(&mut self, bytes: Vec<u8>) -> Result<usize, ToneError> {
        let tone = PodTone::parse(bytes)?;
        let saved = tone.as_bytes().to_vec();
        self.tones.push(StoredTone { tone, saved });
        Ok(self.tones.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.tones.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tones.is_empty()
    }

    pub fn tone(&self, index: usize) -> Option<&PodTone> {
        self.tones.get(index).map(|entry| &entry.tone)
    }

    pub fn replace(&mut self, index: usize, bytes: Vec<u8>) -> Result<(), ToneError> {
        let tone = PodTone::parse(bytes)?;
        let slot = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        slot.tone = tone;
        Ok(())
    }

    pub fn is_dirty(&self, index: usize) -> bool {
        self.tones
            .get(index)
            .is_some_and(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    pub fn any_dirty(&self) -> bool {
        self.tones
            .iter()
            .any(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    pub fn dirty_count(&self) -> usize {
        self.tones
            .iter()
            .filter(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
            .count()
    }

    pub fn first_dirty(&self) -> Option<usize> {
        self.tones
            .iter()
            .position(|entry| entry.tone.as_bytes() != entry.saved.as_slice())
    }

    /// Record a successful save of this tone only.
    pub fn note_saved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        entry.saved = entry.tone.as_bytes().to_vec();
        Ok(())
    }

    /// Restore one tone to its imported or last-saved bytes.
    pub fn revert_unsaved(&mut self, index: usize) -> Result<(), ToneError> {
        let entry = self
            .tones
            .get_mut(index)
            .ok_or_else(|| ToneError::format("tone index is outside the library"))?;
        if entry.tone.as_bytes() != entry.saved.as_slice() {
            entry.tone = PodTone::parse(entry.saved.clone())?;
        }
        Ok(())
    }
}

fn utf16_name(bytes: &[u8]) -> Result<String, ToneError> {
    if bytes.len() != 64 {
        return Err(ToneError::format("PINF name field is not 64 bytes"));
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .take_while(|&unit| unit != 0)
        .collect();
    String::from_utf16(&units).map_err(|_| ToneError::format("invalid UTF-16 tone name"))
}

/// Facts from [`std::fs::metadata`]. That call does not open the path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathFacts {
    pub regular_file: bool,
    pub len: u64,
}

pub fn path_facts(path: &Path) -> std::io::Result<PathFacts> {
    let meta = std::fs::metadata(path)?;
    Ok(PathFacts {
        regular_file: meta.is_file(),
        len: meta.len(),
    })
}

/// Opens an input only when the opened handle itself names a regular file.
/// Callers also check the path facts first so non-file paths are not opened.
pub fn open_regular_file(path: &Path) -> std::io::Result<std::fs::File> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input must be a regular file",
        ));
    }
    Ok(file)
}

/// Reads only after `facts` names a regular file at or under the ceiling.
///
/// A pipe, socket, or device node is refused first, including when its
/// reported length is zero. `read` is not called in that case.
pub fn read_regular_tone<R: Read>(
    facts: std::io::Result<PathFacts>,
    read: impl FnOnce() -> std::io::Result<R>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "tone input must be a regular file. A pipe, socket, or device node is not opened.",
        ));
    }
    if facts.len > TONE_BYTES_CEILING {
        return Err(ToneError::format(
            "tone file exceeds the 1 MiB offline librarian ceiling",
        ));
    }
    read_bounded_file(
        read().map_err(|error| ToneError::format(error.to_string()))?,
        TONE_BYTES_CEILING,
        "tone file exceeds the 1 MiB offline librarian ceiling",
    )
}

/// Reads a regular offline `.5xe`/`.5xs`/`.5xb` candidate under the largest
/// bounded format size. Device nodes, pipes, and oversized files are refused
/// before the reader closure is called.
pub fn read_regular_hd_editor_file<R: Read>(
    facts: std::io::Result<PathFacts>,
    read: impl FnOnce() -> std::io::Result<R>,
) -> Result<Vec<u8>, ToneError> {
    let facts = facts.map_err(|error| ToneError::format(error.to_string()))?;
    if !facts.regular_file {
        return Err(ToneError::format(
            "HD editor input must be a regular file; non-file paths are not opened",
        ));
    }
    if facts.len > HD_BUNDLE_BYTES_CEILING as u64 {
        return Err(ToneError::format(
            "HD editor file exceeds the offline file-size ceiling",
        ));
    }
    read_bounded_file(
        read().map_err(|error| ToneError::format(error.to_string()))?,
        HD_BUNDLE_BYTES_CEILING as u64,
        "HD editor file exceeds the offline file-size ceiling",
    )
}

fn read_bounded_file<R: Read>(
    mut reader: R,
    ceiling: u64,
    too_large: &str,
) -> Result<Vec<u8>, ToneError> {
    let capacity = usize::try_from(
        ceiling
            .checked_add(1)
            .ok_or_else(|| ToneError::format("offline file-size ceiling overflow"))?,
    )
    .map_err(|_| ToneError::format("offline file-size ceiling is not addressable"))?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut buffer = [0u8; 8192];
    loop {
        let remaining = capacity - bytes.len();
        if remaining == 0 {
            break;
        }
        let read_len = buffer.len().min(remaining);
        let count = reader
            .read(&mut buffer[..read_len])
            .map_err(|error| ToneError::format(error.to_string()))?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if bytes.len() as u64 > ceiling {
        return Err(ToneError::format(too_large));
    }
    Ok(bytes)
}

#[cfg(test)]
pub(crate) fn fixture(
    kind: ContainerKind,
    device_id: u32,
    device_version: u32,
    slots: &[(u32, u32)],
) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    let mut info = vec![0; 80];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&device_id.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
    info[72..76].copy_from_slice(&device_version.to_be_bytes());
    info[76..80].copy_from_slice(&[0x5a, 0x5a, 0x5a, 0x5a]);
    let mut patch = b"PATC".to_vec();
    for &(slot, model_id) in slots {
        let mut minf = Vec::new();
        minf.extend_from_slice(&model_id.to_be_bytes());
        minf.extend_from_slice(&slot.to_be_bytes());
        minf.extend_from_slice(&[0x10, 0x20, 0x30, 0x40]);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        model.extend(chunk(b"PARM", &[0x11, 0x22, 0x33, 0x44]));
        model.extend(chunk(
            b"PARM",
            &[
                0xab, 0xcd, 0xef, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
            ],
        ));
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = kind.tag().as_bytes().to_vec();
    data.extend(chunk(b"JUNK", &[9, 8, 7]));
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

#[cfg(test)]
struct BuiltModel<'a> {
    slot: u32,
    model_id: u32,
    tail: &'a [u8],
    parameters: &'a [&'a [u8]],
}

#[cfg(test)]
fn custom_tone(kind: ContainerKind, models: &[BuiltModel<'_>]) -> Vec<u8> {
    fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = id.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(data);
        if !data.len().is_multiple_of(2) {
            output.push(0xa5);
        }
        output
    }
    let mut info = vec![0; 76];
    info[..4].copy_from_slice(&1u32.to_be_bytes());
    info[4..8].copy_from_slice(&PODXT_LIVE_TONE_ID.to_be_bytes());
    info[8..10].copy_from_slice(&u16::from(b'A').to_be_bytes());
    let mut patch = b"PATC".to_vec();
    for entry in models {
        let mut minf = Vec::new();
        minf.extend_from_slice(&entry.model_id.to_be_bytes());
        minf.extend_from_slice(&entry.slot.to_be_bytes());
        minf.extend_from_slice(entry.tail);
        let mut model = b"MODL".to_vec();
        model.extend(chunk(b"MINF", &minf));
        for parameter in entry.parameters {
            model.extend(chunk(b"PARM", parameter));
        }
        patch.extend(chunk(b"LIST", &model));
    }
    let mut data = kind.tag().as_bytes().to_vec();
    data.extend(chunk(b"PINF", &info));
    data.extend(chunk(b"LIST", &patch));
    chunk(b"FORM", &data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Cursor;
    use std::rc::Rc;

    struct ContinuousReader(Rc<Cell<usize>>);

    impl Read for ContinuousReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            buffer.fill(0x5a);
            self.0.set(self.0.get() + buffer.len());
            Ok(buffer.len())
        }
    }

    fn synthetic_h5ep(extra: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0; H5EP_MIN_BYTES];
        bytes[..4].copy_from_slice(b"H5EP");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        let body = &mut bytes[H5EP_HEADER_BYTES..];
        for offset in [0x228, 0x328] {
            body[offset..offset + 4].copy_from_slice(&0x0100_0000u32.to_be_bytes());
        }
        for index in 0..HD_EFFECT_COUNT {
            let offset = HD_EFFECT_BLOCK_OFFSET + index * HD_EFFECT_BLOCK_STRIDE;
            body[offset..offset + 4].copy_from_slice(&0x0200_0000u32.to_be_bytes());
        }
        bytes.extend_from_slice(extra);
        bytes
    }

    #[test]
    fn opaque_h5ep_dispatch_and_noop_round_trip_preserve_every_byte() {
        let mut original = synthetic_h5ep(b"uninterpreted trailing bytes");
        // Exercise noncanonical/unknown header and body values, including bytes
        // that resemble duplicated fields. No field is normalized or decoded.
        original[8..H5EP_HEADER_BYTES].fill(0xa5);
        original[H5EP_HEADER_BYTES + 0xd90..H5EP_HEADER_BYTES + 0xda0].fill(0x5a);
        original[0x0b] = 0x2e;
        original[0x13..0x16].copy_from_slice(&[0x06, 0x02, 0x62]);
        let file = PodPresetFile::parse(original.clone()).unwrap();
        let PodPresetFile::H5ep(envelope) = file else {
            panic!("H5EP must dispatch to the structured editor record");
        };
        assert_eq!(envelope.header_bytes().len(), H5EP_HEADER_BYTES);
        assert_eq!(envelope.body_bytes().len(), H5EP_BODY_BYTES);
        assert_eq!(envelope.trailing_bytes(), b"uninterpreted trailing bytes");
        assert_eq!(envelope.serialize_noop(), original);
    }

    #[test]
    fn opaque_h5ep_rejects_bad_magic_version_truncation_and_oversize() {
        let valid = synthetic_h5ep(&[]);
        for len in [0, 3, 4, H5EP_HEADER_BYTES - 1, H5EP_MIN_BYTES - 1] {
            assert!(
                OpaqueH5epPreset::parse(valid[..len].to_vec()).is_err(),
                "len {len}"
            );
        }
        let mut bad_magic = valid.clone();
        bad_magic[..4].copy_from_slice(b"H5ES");
        assert!(OpaqueH5epPreset::parse(bad_magic).is_err());
        let mut bad_version = valid;
        bad_version[7] = 2;
        assert!(OpaqueH5epPreset::parse(bad_version).is_err());
        assert!(OpaqueH5epPreset::parse(vec![0; TONE_BYTES_CEILING as usize + 1]).is_err());
    }

    #[test]
    fn ceiling_is_checked_after_valid_magic_and_version_by_parser_and_dispatcher() {
        let mut at_ceiling = synthetic_h5ep(&[]);
        at_ceiling.resize(TONE_BYTES_CEILING as usize, 0x6d);
        let parsed = OpaqueH5epPreset::parse(at_ceiling.clone()).unwrap();
        assert_eq!(parsed.serialize_noop(), at_ceiling);
        let dispatched = PodPresetFile::parse(at_ceiling.clone()).unwrap();
        assert_eq!(dispatched.serialize_noop(), at_ceiling);

        let mut over_ceiling = at_ceiling.clone();
        over_ceiling.push(0x6d);
        for result in [
            OpaqueH5epPreset::parse(over_ceiling.clone()),
            match PodPresetFile::parse(over_ceiling.clone()) {
                Err(error) => Err(error),
                Ok(_) => panic!("oversized H5EP must fail dispatch"),
            },
        ] {
            assert!(
                matches!(result, Err(ToneError::Format { detail }) if detail.contains("ceiling"))
            );
        }
    }

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

    fn synthetic_h5es(extra: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xa7; H5ES_HEADER_BYTES];
        bytes[..4].copy_from_slice(b"H5ES");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        for _ in 0..H5ES_PRESET_COUNT {
            bytes.extend_from_slice(&synthetic_h5ep(&[]));
        }
        bytes.extend_from_slice(extra);
        bytes
    }

    fn synthetic_h5eb(extra: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0x39; H5EB_HEADER_BYTES];
        bytes[..4].copy_from_slice(b"H5EB");
        bytes[4..8].copy_from_slice(&1u32.to_be_bytes());
        for _ in 0..H5EB_SETLIST_COUNT {
            bytes.extend_from_slice(&synthetic_h5es(&[]));
        }
        bytes.extend_from_slice(extra);
        bytes
    }

    #[test]
    fn h5es_and_h5eb_parse_fixed_nested_records_and_preserve_tails() {
        let setlist_bytes = synthetic_h5es(b"setlist tail");
        assert_eq!(setlist_bytes.len(), H5ES_MIN_BYTES + 12);
        let dispatched = PodPresetFile::parse(setlist_bytes.clone()).unwrap();
        let PodPresetFile::H5es(setlist) = dispatched else {
            panic!("H5ES should dispatch to a setlist");
        };
        assert_eq!(setlist.preset_bytes(0).unwrap().len(), H5EP_MIN_BYTES);
        assert_eq!(setlist.preset_bytes(63).unwrap().len(), H5EP_MIN_BYTES);
        assert!(setlist.preset_bytes(64).is_none());
        assert_eq!(setlist.trailing_bytes(), b"setlist tail");
        assert_eq!(setlist.as_bytes(), setlist_bytes);

        let bundle_bytes = synthetic_h5eb(b"bundle tail");
        assert_eq!(bundle_bytes.len(), H5EB_MIN_BYTES + 11);
        let dispatched = PodPresetFile::parse(bundle_bytes.clone()).unwrap();
        let PodPresetFile::H5eb(bundle) = dispatched else {
            panic!("H5EB should dispatch to a bundle");
        };
        assert_eq!(bundle.setlist_bytes(0).unwrap().len(), H5ES_MIN_BYTES);
        assert_eq!(bundle.setlist_bytes(7).unwrap().len(), H5ES_MIN_BYTES);
        assert!(bundle.setlist_bytes(8).is_none());
        assert_eq!(bundle.trailing_bytes(), b"bundle tail");
        assert_eq!(bundle.as_bytes(), bundle_bytes);
    }

    #[test]
    fn h5es_and_h5eb_reject_bad_nested_tags_versions_truncation_and_caps() {
        let setlist = synthetic_h5es(&[]);
        assert!(H5esSetlist::parse(setlist[..H5ES_MIN_BYTES - 1].to_vec()).is_err());
        let mut bad_child = setlist.clone();
        bad_child[H5ES_HEADER_BYTES..H5ES_HEADER_BYTES + 4].copy_from_slice(b"H5EB");
        assert!(H5esSetlist::parse(bad_child).is_err());
        let mut bad_version = setlist;
        bad_version[7] = 2;
        assert!(H5esSetlist::parse(bad_version).is_err());

        let bundle = synthetic_h5eb(&[]);
        assert!(H5ebBundle::parse(bundle[..H5EB_MIN_BYTES - 1].to_vec()).is_err());
        let mut bad_nested_setlist = bundle.clone();
        bad_nested_setlist[H5EB_HEADER_BYTES..H5EB_HEADER_BYTES + 4].copy_from_slice(b"H5EP");
        assert!(H5ebBundle::parse(bad_nested_setlist).is_err());
        let mut over_cap = bundle;
        over_cap.resize(HD_BUNDLE_BYTES_CEILING + 1, 0);
        assert!(
            matches!(H5ebBundle::parse(over_cap), Err(ToneError::Format { detail }) if detail.contains("ceiling"))
        );
    }

    #[test]
    fn hd_editor_library_tracks_edits_and_extracts_children_without_changing_parents() {
        let mut bytes = synthetic_h5ep(&[]);
        let body = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        bytes[body + block..body + block + 4].copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        let row = body + block + HD_EFFECT_ROWS_OFFSET;
        bytes[row..row + 4].copy_from_slice(&HD_SCREAMER_DRIVE_KEY.to_be_bytes());
        bytes[row + 4..row + 8].copy_from_slice(&0.25f32.to_bits().to_be_bytes());

        let mut library = HdEditorLibrary::default();
        let preset = library.import_bytes(bytes.clone()).unwrap();
        assert_eq!(
            library.file(preset).unwrap().hd_container_kind(),
            Some("H5EP preset")
        );
        assert!(!library.any_dirty());
        let Some(PodPresetFile::H5ep(preset_file)) = library.file_mut(preset) else {
            panic!("expected H5EP");
        };
        assert_eq!(preset_file.screamer_drive_percent(0), Some(25));
        preset_file.set_screamer_drive_percent(0, 40).unwrap();
        assert!(library.is_dirty(preset));
        library.note_saved(preset).unwrap();
        assert!(!library.is_dirty(preset));

        let parent_bytes = synthetic_h5es(b"tail");
        let parent = library.import_bytes(parent_bytes.clone()).unwrap();
        let child = library.extract_preset(parent, 0, 17).unwrap();
        assert_ne!(parent, child);
        assert_eq!(library.file(parent).unwrap().as_bytes(), parent_bytes);
        let start = H5ES_HEADER_BYTES + 17 * H5EP_MIN_BYTES;
        assert_eq!(
            library.file(child).unwrap().as_bytes(),
            &parent_bytes[start..start + H5EP_MIN_BYTES]
        );
        assert!(!library.any_dirty());
    }

    #[test]
    fn hd_editor_input_reader_checks_regular_file_and_growth_bounds() {
        let called = std::cell::Cell::new(false);
        assert!(
            read_regular_hd_editor_file(
                Ok(PathFacts {
                    regular_file: false,
                    len: 0,
                }),
                || {
                    called.set(true);
                    Ok(Cursor::new(vec![]))
                }
            )
            .is_err()
        );
        assert!(!called.get());
        assert!(
            read_regular_hd_editor_file(
                Ok(PathFacts {
                    regular_file: true,
                    len: (HD_BUNDLE_BYTES_CEILING + 1) as u64,
                }),
                || {
                    called.set(true);
                    Ok(Cursor::new(vec![]))
                }
            )
            .is_err()
        );
        assert!(!called.get());
        assert_eq!(
            read_regular_hd_editor_file(
                Ok(PathFacts {
                    regular_file: true,
                    len: 3,
                }),
                || Ok(Cursor::new(vec![1, 2, 3])),
            )
            .unwrap(),
            [1, 2, 3]
        );

        let consumed = Rc::new(Cell::new(0));
        let error = read_regular_hd_editor_file(
            Ok(PathFacts {
                regular_file: true,
                len: 1,
            }),
            || Ok(ContinuousReader(Rc::clone(&consumed))),
        )
        .unwrap_err();
        assert!(error.detail().contains("ceiling"));
        assert_eq!(consumed.get(), HD_BUNDLE_BYTES_CEILING + 1);
    }

    #[test]
    fn h5ep_body_conversion_is_involutive_and_known_drive_edit_is_narrow() {
        let mut original = synthetic_h5ep(b"opaque tail");
        let body_start = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        original[body_start + block..body_start + block + 4]
            .copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        let row0 = body_start + block + HD_EFFECT_ROWS_OFFSET;
        original[row0..row0 + 4].copy_from_slice(&HD_SCREAMER_DRIVE_KEY.to_be_bytes());
        original[row0 + 4..row0 + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        original[row0 + 0x10..row0 + 0x14].copy_from_slice(&[0x00, 0xde, 0xad, 0xbe]);
        let row1 = row0 + HD_PARAMETER_ROW_STRIDE;
        original[row1..row1 + 4].copy_from_slice(&HD_SCREAMER_DRIVE_KEY.to_be_bytes());
        original[row1 + 4..row1 + 8].copy_from_slice(&0.75f32.to_bits().to_be_bytes());
        let file = OpaqueH5epPreset::parse(original.clone()).unwrap();
        assert_eq!(file.screamer_drive_percent(0), Some(50));

        let disk_body = file.body_bytes().to_vec();
        let mut transformed = disk_body.clone();
        transform_hd_body_words(&mut transformed);
        transform_hd_body_words(&mut transformed);
        assert_eq!(transformed, disk_body);
        let editor_body = file.body_in_editor_order();
        let mut round_trip_body = editor_body;
        transform_hd_body_words(&mut round_trip_body);
        assert_eq!(round_trip_body, disk_body);
        assert_eq!(file.serialize_noop(), original);

        let mut edited = file;
        for percent in [0, 50, 100] {
            edited.set_screamer_drive_percent(0, percent).unwrap();
            assert_eq!(edited.screamer_drive_percent(0), Some(percent));
        }
        let mut expected = original.clone();
        expected[row0 + 4..row0 + 8].copy_from_slice(&1.0f32.to_bits().to_be_bytes());
        assert_eq!(edited.as_bytes(), expected);
        assert_eq!(
            &edited.as_bytes()[row0 + 0x10..row0 + 0x14],
            &[0, 0xde, 0xad, 0xbe]
        );
        let before = edited.as_bytes().to_vec();
        assert!(edited.set_screamer_drive_percent(0, 101).is_err());
        assert!(edited.set_screamer_drive_percent(8, 50).is_err());
        assert_eq!(edited.as_bytes(), before);

        let mut non_grid = original.clone();
        non_grid[row0 + 4..row0 + 8].copy_from_slice(&0.123_456_7f32.to_bits().to_be_bytes());
        let mut non_grid = OpaqueH5epPreset::parse(non_grid).unwrap();
        assert_eq!(non_grid.screamer_drive_percent(0), None);
        let non_grid_before = non_grid.as_bytes().to_vec();
        assert!(non_grid.set_screamer_drive_percent(0, 12).is_err());
        assert_eq!(non_grid.as_bytes(), non_grid_before);
    }

    #[test]
    fn screamer_type1_controls_read_and_edit_only_their_first_keyed_value() {
        let mut original = synthetic_h5ep(&[]);
        let body_start = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        original[body_start + block..body_start + block + 4]
            .copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        let first_row = body_start + block + HD_EFFECT_ROWS_OFFSET;
        for (index, control) in ScreamerControl::ALL.into_iter().enumerate() {
            let row = first_row + index * HD_PARAMETER_ROW_STRIDE;
            original[row..row + 4].copy_from_slice(&control.key().to_be_bytes());
            original[row + 4..row + 8]
                .copy_from_slice(&((index as u32 + 10) as f32 / 100.0).to_bits().to_be_bytes());
            original[row + HD_PARAMETER_ROW_FLAG_OFFSET..row + HD_PARAMETER_ROW_STRIDE]
                .copy_from_slice(&[
                    0,
                    0x41 + index as u8,
                    0x51 + index as u8,
                    0x61 + index as u8,
                ]);
        }
        let mut preset = OpaqueH5epPreset::parse(original).unwrap();

        for (index, control) in ScreamerControl::ALL.into_iter().enumerate() {
            assert_eq!(
                preset.screamer_control_percent(0, control),
                Some(10 + index as u8)
            );
            let before = preset.as_bytes().to_vec();
            preset.set_screamer_control_percent(0, control, 73).unwrap();
            assert_eq!(preset.screamer_control_percent(0, control), Some(73));
            let value_at = first_row + index * HD_PARAMETER_ROW_STRIDE + 4;
            let mut expected = before;
            expected[value_at..value_at + 4].copy_from_slice(&0.73f32.to_bits().to_be_bytes());
            assert_eq!(preset.as_bytes(), expected);
        }
    }

    #[test]
    fn screamer_drive_admission_rejects_nonzero_row_flags_and_bad_values_atomically() {
        let mut eligible = synthetic_h5ep(&[]);
        let body_start = H5EP_HEADER_BYTES;
        let block = HD_EFFECT_BLOCK_OFFSET;
        eligible[body_start + block..body_start + block + 4]
            .copy_from_slice(&HD_SCREAMER_MODEL_ID.to_be_bytes());
        let row = body_start + block + HD_EFFECT_ROWS_OFFSET;
        eligible[row..row + 4].copy_from_slice(&HD_SCREAMER_DRIVE_KEY.to_be_bytes());
        eligible[row + 4..row + 8].copy_from_slice(&0.5f32.to_bits().to_be_bytes());
        eligible[row + HD_PARAMETER_ROW_FLAG_OFFSET..row + HD_PARAMETER_ROW_STRIDE]
            .copy_from_slice(&[0, 0x11, 0x22, 0x33]);

        // Zero is the only admitted row flag; every nonzero value is treated
        // as unresolved assignment state, without changing any row bytes.
        for flag in 1..=u8::MAX {
            let mut bytes = eligible.clone();
            bytes[row + HD_PARAMETER_ROW_FLAG_OFFSET] = flag;
            let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
            assert_eq!(preset.screamer_drive_percent(0), None, "flag {flag:#04x}");
            let before = preset.as_bytes().to_vec();
            assert!(preset.set_screamer_drive_percent(0, 25).is_err());
            assert_eq!(preset.as_bytes(), before, "flag {flag:#04x}");
        }

        let mut missing_key = eligible.clone();
        missing_key[row..row + 4].copy_from_slice(&0x3f10_0002u32.to_be_bytes());
        let mut missing_key = OpaqueH5epPreset::parse(missing_key).unwrap();
        assert_eq!(missing_key.screamer_drive_percent(0), None);
        let before = missing_key.as_bytes().to_vec();
        assert!(missing_key.set_screamer_drive_percent(0, 25).is_err());
        assert_eq!(missing_key.as_bytes(), before);

        for value in [f32::NAN, f32::INFINITY, -0.01, 1.01, 0.123_456_7] {
            let mut bytes = eligible.clone();
            bytes[row + 4..row + 8].copy_from_slice(&value.to_bits().to_be_bytes());
            let mut preset = OpaqueH5epPreset::parse(bytes).unwrap();
            assert_eq!(preset.screamer_drive_percent(0), None);
            let before = preset.as_bytes().to_vec();
            assert!(preset.set_screamer_drive_percent(0, 25).is_err());
            assert_eq!(preset.as_bytes(), before);
        }
    }

    #[test]
    fn header_context_byte_is_preserved_but_not_admitted_as_product_identity() {
        for context_byte in [0x2d, 0x2e] {
            let mut bytes = synthetic_h5ep(&[]);
            bytes[0x0b] = context_byte;
            let parsed = PodPresetFile::parse(bytes.clone()).unwrap();
            assert!(matches!(parsed, PodPresetFile::H5ep(_)));
            assert_eq!(parsed.serialize_noop(), bytes);
        }
    }

    #[test]
    fn opaque_h5ep_save_copy_is_atomic_and_does_not_overwrite() {
        let original = synthetic_h5ep(b"tail");
        let file = PodPresetFile::parse(original.clone()).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("copy.5xe");
        file.save_copy(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(file.save_copy(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);

        let blocked_dir = directory.path().join("existing-directory");
        std::fs::create_dir(&blocked_dir).unwrap();
        let sentinel = blocked_dir.join("preserve.bin");
        std::fs::write(&sentinel, b"sentinel").unwrap();
        assert!(file.save_copy(&blocked_dir).is_err());
        assert_eq!(file.as_bytes(), original);
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");

        let invalid_destination = directory.path().join("missing-parent").join("copy.5xe");
        assert!(file.save_copy(&invalid_destination).is_err());
        assert!(!invalid_destination.exists());
        assert_eq!(file.as_bytes(), original);
        assert_eq!(file.serialize_noop(), original);
    }
    use crate::pod_assets::{self, SlotRole};

    fn live_tone() -> Vec<u8> {
        fixture(
            ContainerKind::L6as,
            PODXT_LIVE_TONE_ID,
            0x0011_2233,
            &[
                (AMP_SLOT, 0x1111_0002),
                (CAB_SLOT, 0x2222_0003),
                (0x0003_00aa, 0x3333_0004),
            ],
        )
    }

    #[test]
    fn tone_id_matches_the_archived_product_id_and_not_the_midi_hardware_id() {
        assert_eq!(PODXT_LIVE_TONE_ID, crate::firmware::PODXT_LIVE_PRODUCT_ID);
        assert_eq!(PODXT_LIVE_TONE_ID, 196_618);
        assert_ne!(PODXT_LIVE_TONE_ID, 18_000);
        const {
            assert!(TONE_BYTES_CEILING == 1_048_576);
        }
    }

    #[test]
    fn l6as_exposes_raw_amp_and_cab_and_preserves_unknown_bytes() {
        let original = live_tone();
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(tone.as_bytes(), original);
        assert_eq!(tone.container(), ContainerKind::L6as);
        assert_eq!(tone.device_version(), 0x0011_2233);
        assert_ne!(tone.device_version(), crate::firmware::PODXT_LIVE_VERSION);
        assert_eq!(tone.amp_model_id(), Some(0x1111_0002));
        assert_eq!(tone.cab_model_id(), Some(0x2222_0003));
        assert_eq!(tone.amp_asset_name(), None);
        assert_eq!(tone.cab_asset_name(), None);
        assert_eq!(tone.undecoded_slots(), 1);
        assert_eq!(tone.recorded_parameter_chunks(), 3);
        assert_eq!(tone.undecoded_parameter_chunks(), 3);
        assert_eq!(tone.models()[0].ordinal, Some(0x10));
        assert_eq!(tone.models()[0].reserved, Some([0x20, 0x30]));
        assert_eq!(tone.models()[0].enabled, Some(true));
        assert!(matches!(
            tone.models()[0].parameters[0],
            ModelParameter::Opaque { len: 4 }
        ));
        assert!(matches!(
            tone.models()[0].parameters[1],
            ModelParameter::Recorded {
                prefix: 0xab,
                id: 0x00cd_ef01,
                value: ParameterValue::Other {
                    type_word: 0x0203_0405,
                    bits: 0x0607_0809,
                },
            }
        ));
        assert_eq!(tone.name(), "A");
        tone.rename("Stage").unwrap();
        let reread = PodTone::parse(tone.as_bytes().to_vec()).unwrap();
        assert_eq!(reread.name(), "Stage");
        assert_eq!(reread.amp_model_id(), Some(0x1111_0002));
        assert_eq!(reread.cab_model_id(), Some(0x2222_0003));
        let name_at = tone_name_offset(&original);
        for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
            if !(name_at..name_at + 64).contains(&index) {
                assert_eq!(before, after, "byte {index} changed");
            }
        }
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0x11, 0x22, 0x33, 0x44])
        );
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0xab, 0xcd, 0xef, 0x01])
        );
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0x5a, 0x5a, 0x5a, 0x5a])
        );
        assert!(tone.as_bytes().windows(3).any(|bytes| bytes == [9, 8, 7]));
    }

    #[test]
    fn cab_slot_on_l6pa_and_l6fs_is_not_the_cab_model() {
        for kind in [ContainerKind::L6pa, ContainerKind::L6fs] {
            let tone = PodTone::parse(fixture(
                kind,
                PODXT_LIVE_TONE_ID,
                0,
                &[(AMP_SLOT, 7), (CAB_SLOT, 8)],
            ))
            .unwrap();
            assert_eq!(tone.container(), kind);
            assert_eq!(tone.amp_model_id(), Some(7));
            assert_eq!(tone.cab_model_id(), None);
            assert_eq!(tone.cab_asset_name(), None);
            assert_eq!(tone.undecoded_slots(), 0);
            assert_eq!(
                SlotRole::from_id(tone.models()[1].slot),
                Some(SlotRole::Cab)
            );
            assert_eq!(tone.models()[1].slot, CAB_SLOT);
            assert_eq!(tone.models()[1].model_id, 8);
        }
    }

    #[test]
    fn missing_amp_is_absent_and_other_products_are_rejected() {
        let empty =
            PodTone::parse(fixture(ContainerKind::L6pa, PODXT_LIVE_TONE_ID, 0, &[])).unwrap();
        assert_eq!(empty.amp_model_id(), None);
        assert_eq!(empty.cab_model_id(), None);
        assert_eq!(empty.undecoded_slots(), 0);
        for device_id in [
            0x0007_0004u32,
            0x0007_0000,
            0x0003_000b,
            0x0003_0002,
            196_608,
        ] {
            let error =
                PodTone::parse(fixture(ContainerKind::L6pa, device_id, 0, &[(AMP_SLOT, 1)]))
                    .unwrap_err();
            assert_eq!(error, ToneError::WrongTarget { device_id });
            assert!(!error.detail().contains("decoded"));
        }
    }

    #[test]
    fn rejects_banks_bad_versions_duplicates_and_truncation() {
        let source = live_tone();
        let mut bank = source.clone();
        bank[8..12].copy_from_slice(b"L6BA");
        assert!(PodTone::parse(bank).is_err());
        let mut version = source.clone();
        let info = find_chunk_data(&version, b"PINF");
        version[info..info + 4].copy_from_slice(&2u32.to_be_bytes());
        assert!(PodTone::parse(version).is_err());
        let mut duplicated = fixture(
            ContainerKind::L6as,
            PODXT_LIVE_TONE_ID,
            1,
            &[(AMP_SLOT, 1), (AMP_SLOT, 2)],
        );
        assert!(
            PodTone::parse(duplicated.clone())
                .unwrap_err()
                .detail()
                .contains("duplicate")
        );
        for end in 0..12 {
            assert!(PodTone::parse(source[..end].to_vec()).is_err());
        }
        duplicated.truncate(20);
        duplicated[4..8].copy_from_slice(&12u32.to_be_bytes());
        assert!(PodTone::parse(duplicated).is_err());
        assert!(PodTone::parse(vec![0; TONE_BYTES_CEILING as usize + 1]).is_err());
    }

    #[test]
    fn recovered_controls_keep_unknown_bytes_and_do_not_edit_parameters() {
        let original = custom_tone(
            ContainerKind::L6as,
            &[
                BuiltModel {
                    slot: AMP_SLOT,
                    model_id: 131_073,
                    tail: &[0x04, 0x00, 0x00, 0x01, 0xaa, 0xbb, 0xcc, 0xdd],
                    parameters: &[
                        &[
                            0x3f, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x01, 0x3f, 0x80, 0x00, 0x00,
                        ],
                        &[0x3f, 0x00, 0x00, 0x01],
                        &[
                            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                            0xde, 0xef, 0x01, 0x02,
                        ],
                    ],
                },
                BuiltModel {
                    slot: CAB_SLOT,
                    model_id: 0x0100_000f,
                    tail: &[0x02, 0x7f, 0x7e, 0x00],
                    parameters: &[],
                },
                BuiltModel {
                    slot: 131_074,
                    model_id: 0x0205_0000,
                    tail: &[0x01, 0x10, 0x20, 0x03],
                    parameters: &[],
                },
                BuiltModel {
                    slot: 0x0003_00aa,
                    model_id: 0x3333_0004,
                    tail: &[],
                    parameters: &[],
                },
            ],
        );
        let mut tone = PodTone::parse(original.clone()).unwrap();
        assert_eq!(tone.amp_model_id(), Some(131_073));
        assert_eq!(tone.amp_asset_name(), Some("Line 6 Clean"));
        assert_eq!(tone.cab_model_id(), Some(0x0100_000f));
        assert_eq!(tone.cab_asset_name(), Some("No Cabinet"));
        assert_eq!(tone.undecoded_slots(), 1);
        assert_eq!(tone.recorded_parameter_chunks(), 1);
        assert_eq!(tone.undecoded_parameter_chunks(), 2);
        let amp = &tone.models()[0];
        assert_eq!(amp.ordinal, Some(0x04));
        assert_eq!(amp.reserved, Some([0x00, 0x00]));
        assert_eq!(amp.enabled, Some(true));
        assert_eq!(
            amp.parameters[0],
            ModelParameter::Recorded {
                prefix: 0x3f,
                id: 7,
                value: ParameterValue::Float { bits: 0x3f80_0000 },
            }
        );
        assert_eq!(amp.parameters[1], ModelParameter::Opaque { len: 4 });
        assert_eq!(amp.parameters[2], ModelParameter::Opaque { len: 16 });
        let cab = &tone.models()[1];
        assert_eq!(cab.enabled, Some(false));
        assert_eq!(cab.reserved, Some([0x7f, 0x7e]));
        let wah = &tone.models()[2];
        assert_eq!(SlotRole::from_id(wah.slot), Some(SlotRole::Wah));
        assert_eq!(pod_assets::asset_name(wah.model_id), Some("Facial Fuzz"));
        assert_eq!(wah.ordinal, Some(1));
        assert_eq!(wah.enabled, Some(true));
        let unknown = &tone.models()[3];
        assert_eq!(unknown.ordinal, None);
        assert_eq!(unknown.reserved, None);
        assert_eq!(unknown.enabled, None);
        assert_eq!(SlotRole::from_id(unknown.slot), None);
        let lines = tone.control_lines();
        assert!(lines[0].contains("Amp slot 00030000 model Line 6 Clean 00020001"));
        assert!(lines[0].contains(" enabled"));
        assert!(!lines[0].contains("disabled"));
        assert!(lines[0].contains("ordinal 4"));
        assert!(lines[0].contains("reserved 00 00"));
        assert!(lines[1].contains("parameter 000007 prefix 3f float 3f800000 no range"));
        assert!(lines[2].contains("opaque parameter 4 bytes"));
        assert!(lines[3].contains("opaque parameter 16 bytes"));
        assert!(lines[4].contains("Cab slot 00030001 model No Cabinet 0100000f"));
        assert!(lines[4].contains("disabled"));
        assert!(lines[5].contains("Wah slot 00020002 model Facial Fuzz 02050000"));
        assert!(lines[6].contains("no GPXT role slot 000300aa model 33330004"));
        assert!(!lines.iter().any(|line| line.contains("bypass")));
        tone.rename("Named").unwrap();
        assert_eq!(tone.amp_asset_name(), Some("Line 6 Clean"));
        assert_eq!(
            tone.models()[0].parameters,
            amp_parameters_after_rename(&original)
        );
        for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
            let name_at = tone_name_offset(&original);
            if !(name_at..name_at + 64).contains(&index) {
                assert_eq!(before, after, "byte {index} changed");
            }
        }
        assert!(
            tone.as_bytes()
                .windows(4)
                .any(|bytes| bytes == [0xaa, 0xbb, 0xcc, 0xdd])
        );
        let closed = PodTone::parse(custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: CAB_SLOT,
                model_id: 0x0100_000f,
                tail: &[0, 0, 0, 1],
                parameters: &[],
            }],
        ))
        .unwrap();
        assert_eq!(closed.cab_model_id(), None);
        assert_eq!(
            pod_assets::asset_name(closed.models()[0].model_id),
            Some("No Cabinet")
        );
        let partial = PodTone::parse(custom_tone(
            ContainerKind::L6pa,
            &[BuiltModel {
                slot: AMP_SLOT,
                model_id: 1,
                tail: &[0x11, 0x22, 0x33],
                parameters: &[],
            }],
        ))
        .unwrap();
        assert_eq!(partial.models()[0].ordinal, None);
        assert_eq!(partial.models()[0].reserved, None);
        assert_eq!(partial.models()[0].enabled, None);
        assert!(
            partial
                .as_bytes()
                .windows(3)
                .any(|bytes| bytes == [0x11, 0x22, 0x33])
        );
    }

    #[test]
    fn signed_minf_fields_follow_java_read8_and_rename_keeps_the_bytes() {
        // Java `byte` values of the file bytes 0x00, 0x01, 0x7F, 0x80, and 0xFF.
        // Both the ordinal and the enabled byte use each value.
        const TAILS: [[u8; 4]; 5] = [
            [0x00, 0x5a, 0xa5, 0x00],
            [0x01, 0x5a, 0xa5, 0x01],
            [0x7f, 0x5a, 0xa5, 0x7f],
            [0x80, 0x5a, 0xa5, 0x80],
            [0xff, 0x5a, 0xa5, 0xff],
        ];
        let expected: [(u32, i8, bool); 5] = [
            (65_536, 0, false),
            (131_072, 1, true),
            (131_073, 127, true),
            (131_074, -128, false),
            (AMP_SLOT, -1, false),
        ];
        let models: Vec<BuiltModel<'_>> = expected
            .iter()
            .enumerate()
            .map(|(index, &(slot, _, _))| BuiltModel {
                slot,
                model_id: 0x1111_0002,
                tail: &TAILS[index],
                parameters: &[],
            })
            .collect();
        let original = custom_tone(ContainerKind::L6as, &models);
        let mut tone = PodTone::parse(original.clone()).unwrap();
        let lines = tone.control_lines();
        assert_eq!(lines.len(), expected.len());
        for (index, &(slot, ordinal, enabled)) in expected.iter().enumerate() {
            let model = &tone.models()[index];
            assert_eq!(model.slot, slot);
            assert_eq!(model.ordinal, Some(ordinal));
            assert_eq!(model.reserved, Some([0x5a, 0xa5]));
            assert_eq!(model.enabled, Some(enabled));
            assert!(lines[index].ends_with(&format!("ordinal {ordinal} reserved 5a a5")));
            if enabled {
                assert!(lines[index].contains(" enabled"));
                assert!(!lines[index].contains("disabled"));
            } else {
                assert!(lines[index].contains(" disabled"));
            }
        }
        tone.rename("Signed").unwrap();
        assert_eq!(tone.name(), "Signed");
        let name_at = tone_name_offset(&original);
        for (index, (&before, &after)) in original.iter().zip(tone.as_bytes()).enumerate() {
            if !(name_at..name_at + 64).contains(&index) {
                assert_eq!(before, after, "byte {index} changed");
            }
        }
        for tail in TAILS {
            assert!(
                tone.as_bytes().windows(4).any(|bytes| bytes == tail),
                "MINF tail {tail:02x?} was not preserved"
            );
        }
    }

    fn amp_parameters_after_rename(original: &[u8]) -> Vec<ModelParameter> {
        PodTone::parse(original.to_vec()).unwrap().models()[0]
            .parameters
            .clone()
    }

    #[test]
    fn library_rename_and_save_do_not_overwrite_or_cross_tones() {
        let mut library = Library::default();
        let first = library.import_bytes(live_tone()).unwrap();
        let second = library
            .import_bytes(fixture(
                ContainerKind::L6pa,
                PODXT_LIVE_TONE_ID,
                4,
                &[(AMP_SLOT, 9)],
            ))
            .unwrap();
        let mut edited = library.tone(first).unwrap().clone();
        edited.rename("Guitare 🎸").unwrap();
        library.replace(first, edited.into_bytes()).unwrap();
        assert!(library.is_dirty(first));
        assert!(!library.is_dirty(second));
        assert_eq!(library.tone(first).unwrap().name(), "Guitare 🎸");
        assert_eq!(library.tone(second).unwrap().amp_model_id(), Some(9));
        assert_eq!(library.tone(second).unwrap().name(), "A");
        assert!(edited_name_too_long(library.tone(first).unwrap()));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("copy.tone");
        library.tone(first).unwrap().save_copy(&path).unwrap();
        assert!(library.is_dirty(first));
        library.note_saved(first).unwrap();
        assert!(!library.is_dirty(first));
        assert!(!library.is_dirty(second));
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(saved, library.tone(first).unwrap().as_bytes());
        assert!(library.tone(first).unwrap().save_copy(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        let mut other = library.tone(second).unwrap().clone();
        other.rename("Other").unwrap();
        library.replace(second, other.into_bytes()).unwrap();
        assert!(!library.is_dirty(first));
        assert!(library.is_dirty(second));
        library.revert_unsaved(second).unwrap();
        assert_eq!(library.tone(second).unwrap().name(), "A");
        assert!(!library.is_dirty(second));
        assert_eq!(library.tone(first).unwrap().name(), "Guitare 🎸");
        assert!(library.replace(7, live_tone()).is_err());
        assert!(library.note_saved(7).is_err());
        assert!(library.revert_unsaved(7).is_err());
    }

    #[test]
    fn zero_length_nonregular_facts_are_not_read() {
        let mut called = false;
        let error = read_regular_tone(
            Ok(PathFacts {
                regular_file: false,
                len: 0,
            }),
            || {
                called = true;
                Ok(Cursor::new(vec![1]))
            },
        )
        .unwrap_err();
        assert!(!called);
        assert!(error.detail().contains("regular file"));
        assert!(error.detail().contains("not opened"));
        let mut called = false;
        let error = read_regular_tone(
            Ok(PathFacts {
                regular_file: true,
                len: TONE_BYTES_CEILING + 1,
            }),
            || {
                called = true;
                Ok(Cursor::new(vec![1]))
            },
        )
        .unwrap_err();
        assert!(!called);
        assert!(error.detail().contains("ceiling"));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-a-tone");
        std::fs::create_dir(&path).unwrap();
        assert!(open_regular_file(&path).is_err());
        let error = read_regular_tone(path_facts(&path), || -> io::Result<Cursor<Vec<u8>>> {
            panic!("directory must not be read")
        })
        .unwrap_err();
        assert!(error.detail().contains("regular file"));
    }

    fn edited_name_too_long(tone: &PodTone) -> bool {
        let mut tone = tone.clone();
        tone.rename(&"🎸".repeat(17)).is_err() && tone.rename("bad\0name").is_err()
    }

    fn tone_name_offset(bytes: &[u8]) -> usize {
        find_chunk_data(bytes, b"PINF") + 8
    }

    fn find_chunk_data(bytes: &[u8], id: &[u8; 4]) -> usize {
        let children = preset::chunks(bytes, 12..bytes.len()).unwrap();
        preset::unique(&children, id).unwrap().data.start
    }
}
