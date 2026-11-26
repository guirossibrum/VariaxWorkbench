//! PODxt backup v1: identity, active tones and raw dumps for stored tones,
//! amp setups and FX setups.

use super as podxt;
use crate::backup::{BackupFolder, read_optional, verify_manifest};
use std::path::Path;

mod snapshot;
pub use snapshot::{ValidatedSnapshot, validate_snapshot};

pub(crate) const STORED_FOLDERS: &[&str] = &["tones", "amp-setups", "fx-setups"];

pub const PODXT_RESTORE: &str =
    "PODxt Live library backup (read-only copy, line6-devices backup v1).

tones/NNN-LABEL.syx, amp-setups/NNN-N.syx and fx-setups/NNN-N.syx are the exact
168-byte SysEx dumps (F0 00 01 0C 03 74 .. F7) the PODxt sent when asked for each
stored location by address (Line 6 Edit 3.06 RequestPatch layout: name at bytes
7..22, controller n at byte 39+n). index.txt lists location, name and amp.
current-before.syx and current-after.syx are the active tone before and after.
Load with line6_devices::backup::load_podxt_library, or decode one dump with
podxt::EditBuffer::parse.

Check integrity:   sha256sum -c MANIFEST.sha256
Verified backup:   read the library again and compare the two folders
                   (line6_devices::backup::compare, `variax-probe compare`).

Restoring to the PODxt needs a write gate (G6/G7), the owner's direct
permission and an approved restore procedure.
";

pub fn podxt_entry_name(location: podxt::Location) -> String {
    format!(
        "{}/{:03}-{}.syx",
        location.kind().slug(),
        location.index() + 1,
        location.label().replace('#', "")
    )
}

pub fn podxt_index_line(stored: &podxt::StoredTone) -> String {
    format!(
        "{:<10} {:>4}: {:<16} amp {}",
        stored.location.kind().name(),
        stored.location.label(),
        stored.tone.name(),
        stored.tone.amp().name
    )
}

/// Writes a PODxt library read in the v1 layout, plus index and restore note.
pub fn save_podxt_library(
    folder: &mut BackupFolder,
    library: &podxt::Library,
) -> Result<(), String> {
    folder.file("identity.syx", &library.identity_reply)?;
    folder.file("current-before.syx", &library.before.raw)?;
    if let Ok(after) = &library.after {
        folder.file("current-after.syx", &after.raw)?;
    }
    let mut index = String::new();
    for stored in &library.entries {
        folder.file(&podxt_entry_name(stored.location), &stored.tone.raw)?;
        index.push_str(&podxt_index_line(stored));
        index.push('\n');
    }
    folder.file("index.txt", index.as_bytes())?;
    folder.set_restore_note(PODXT_RESTORE);
    Ok(())
}

pub struct LoadedPodxt {
    pub before: Option<podxt::EditBuffer>,
    pub after: Option<podxt::EditBuffer>,
    pub entries: Vec<podxt::StoredTone>,
}

/// Verifies the manifest, then decodes every stored entry present.
pub fn load_podxt_library(dir: &Path) -> Result<LoadedPodxt, String> {
    verify_manifest(dir)?;
    let dump = |name: &str| -> Result<Option<podxt::EditBuffer>, String> {
        read_optional(dir, name)?
            .map(|bytes| podxt::EditBuffer::parse(bytes).map_err(|e| format!("{name}: {e}")))
            .transpose()
    };
    let mut entries = Vec::new();
    for location in podxt::Location::all(&podxt::BankKind::ALL) {
        if let Some(tone) = dump(&podxt_entry_name(location))? {
            entries.push(podxt::StoredTone { location, tone });
        }
    }
    Ok(LoadedPodxt {
        before: dump("current-before.syx")?,
        after: dump("current-after.syx")?,
        entries,
    })
}
