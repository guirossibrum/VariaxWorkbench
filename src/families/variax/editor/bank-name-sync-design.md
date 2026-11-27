# Variax bank names in Sync (#205)

Offline design checkpoint for the Variax 600 on firmware 3.10 through PODxt
Live 3.01. The proposed editor state and UI can be reviewed independently of
the device transaction. The [preset owner card](device-sync-test-card.md)
continues to cover the two existing preset store paths.

## Reference and current editor

- **Workbench 1.75 build 121, Windows, static.** `ValidateRename` marks the
  device bundle’s bank attributes unsynchronized when the name changes.
  `GetNumUnsyncedItems` counts each modified bank once, alongside modified
  models. `SyncDialog` can show a separate `Bank Name` row, while the librarian
  marks the bank independently of its five models. `Synchronize` handles bank
  attributes before that bank’s model loop, with separate selection flags.
  On revert it requests the guitar’s bank name. On write it marks the bank
  synchronized when the send returns, without a persistence check. The
  flagged and unflagged branches were checked against class bytecode because
  the decompiled expression makes the selection assignment ambiguous.
- **Rust at `5eddd17`.** Rename changes the selected bundle’s title in one
  undo step and updates file dirtiness. `EditorDocument` already holds a
  working bundle, a file-save snapshot, and a separate library baseline.
  `modified_guitar_positions` compares the 60 model positions only, including
  the explicit pending mark for an equal-byte paste. Sync and the close/quit
  warning consume that list. `LibraryOrigin::AttachedDownload` scopes that
  comparison to the matching live attachment. Reopened downloads,
  imports, cached libraries and factory bundles lack that live provenance.
  [Document state](../file/document.rs) · [Sync](device_sync.rs) · [Library origin](startup_library.rs)
- **Approved preset workflow.** To move an edit from Spank 1 to Spank 2,
  Copy at Spank 1, Paste at Spank 2, Read unmodified at Spank 1 and Write to
  device at Spank 2. Sync targets each modified model’s existing position.
  The worker checks each target against its manifested byte-level restore
  image, uses #245 `store_and_reload` and clears only verified positions.
  Keep model confirmation, position mapping, equal-byte Paste behavior,
  read-back and failure handling for those actions. [Preset worker](procedure_device.rs) · [Merged exit fix](https://github.com/guirossibrum/VariaxWorkbench/commit/9b0bebf) · [Owner’s workflow](https://github.com/guirossibrum/VariaxWorkbench/issues/205#issuecomment-7153792364)

## Proposed editor checkpoint

1. Derive pending names by comparing the working bundle with the captured
   names in its library baseline. Identify each entry by the fixed zero-based
   bank index on the current verified attachment. Only a complete attached
   download with name replies associated with the addressed banks by the
   read sweep supplies that baseline. Keep the existing limit of 18
   printable ASCII characters. For pending comparisons, use
   the readable device name, trimming trailing spaces as the `0x6D` decoder
   does after removing wire padding. Preserve the edited file text and raw
   capture bytes. Undo, redo and a rename back to the baseline recompute the
   difference. Saving a file copy updates only the file baseline.
2. In Sync show the existing modified-model count and a separate pending
   bank-name count. For each pending name show the fixed bank number and
   captured name → edited name so duplicate or empty labels remain
   identifiable. A rename creates one row and zero modified models. If only
   names are pending, show them and explain that storing bank names is
   unavailable. With both kinds pending, keep the existing model actions and
   confirmation scoped to their model positions. A model confirmation must
   never silently include a bank name. The empty-state message requires both
   lists to be empty.
3. Offer an explicitly local **Discard bank-name edits** action for the
   pending names, with confirmation. It restores the listed titles from the
   captured baseline in one undo step and preserves any model edits. This
   differs from Workbench’s fresh device read. A later **Read unmodified**
   action for bank names would need a separately reviewed G5 read path that
   verifies the addressed bank and handles the selection change. A model
   read, model store, live audition or file save cannot verify a bank name.
4. Extend the same-attachment close/quit warning to count pending bank
   names alongside pending positions, including the name-only case after
   **Save a copy…**. Keep editing preserves the edits and attachment.
   Discard permits close. All three choices send zero device commands for
   bank names. A stale attachment invalidates an open Sync confirmation.
   The document retains its working names and can be saved as a historical
   file. Require a fresh download when the attachment identity changes.

## Device acceptance boundary

The preset path sends `0x43`, waits 100 ms, attempts `0x61` once, waits for
one `0x71`, then reloads and compares the complete firmware-stamped 3,276-byte
program. Its silence and spare-group rules apply to that slot store.
Bank-name storage needs its own result and recovery contract. The family
helper `rename_bank` selects slot 0 of the bank, waits 200 ms, sends `0x6C`,
waits 700 ms and compares a decoded `0x6D` reply. There is no bank-name
acknowledgement. `0x6D` reads the selected name from RAM and carries no bank
identifier. `rename_bank` leaves the changed selection in place. The current
editor worker accepts only preset targets and its library policy omits
`VariaxBankNameRead`. [#245 preset contract](../../../../docs/variax-shared-procedures.md#store-and-sync-205) · [Family helper](../../../../crates/line6-devices/src/family/variax/write.rs) · [Wire evidence](../../../../docs/variax-write-transactions.md)

| Prerequisite | Evidence required before bank-name storage can be offered |
| --- | --- |
| G5 backup and fresh preflight | On the same verified rig, preserve a complete, manifest-verified capture with all 12 addressed raw name replies, all 60 settings pairs and all 60 checksum-verified full programs, plus starting selection, verified return and cleanup evidence. Check the current name table and relevant saved state against that backup before any approved write. Historical settings-only backups lack the exact programs and may lack name replies. Metadata snapshots with all 12 names still lack the programs. The #236 full-program format can preserve the readable names and programs, but it cannot capture the physical name sector, its marker or all 132 words through `0x6D`. No accepted bank-name restore source and procedure is recorded. [Backup layout](../../../../docs/device-check.md#variax-library-backup) · [Snapshot checks](../../../../docs/first-connection-cache.md) |
| #181 flash and addressing proof | The firmware trace shows that `0x6C` erases and rewrites the whole name table before checking the trailer, with PE4 clear and no spare. A truncated transfer can commit once all 18 data bytes arrive. The bank-to-RAM offset remains unmeasured. Numeric erase index 8 with PE4 clear and preset index 8 (program 63) with PE4 set do not establish physical aliasing or isolation. An existing owner backup reads a valid program 63, which does not measure the effect of a rename. Require evidence for correct bank selection and table placement, and demonstrate that other names and saved presets survive the operation, including program 63. Require name and program comparison after an approved restart to establish persistence. A matching immediate `0x6D` or the static boot copy path alone establishes neither. [#181 bank-name trace](../../../../docs/firmware-variax-dispatch.md#bank-names-0x6c-and-0x6d) |
| Recovery and unknown outcomes | Agree on a recovery source whose coverage is established. If recovery uses the 12 name replies, prove their mapping and round-trip restore coverage, including padding and any table data the replies omit. If required state cannot be reconstructed, identify an approved way to preserve and restore it before proceeding. Define separately approved recovery for both a completed rename and an uncertain interruption. Verify restored names, all saved programs, starting selection and cleanup against the backup. Stop the batch on silence, bad or mismatched read-back, a potentially partial send, an unverified selection, or loss of cleanup. Keep uncertain entries pending and prevent further writes until owner-directed verification resolves the state. No automatic resend, rollback, or continuation after an uncertain `0x6C`. The #177/#178 preset restore and #261 Rig Check recovery do not establish recovery for the name window. [Custom restore scope](../../../../docs/variax-custom-restore-test-card.md) · [Rig Check coverage](../../../../docs/rig-check/coverage.md) |
| Gate and owner decision | A future proposal must route the complete operation through `run_if_unlocked` with a narrowly scoped device policy, verified attachment and exclusive rig lock. It needs the existing identity/selection/read prerequisites, G5 `VariaxBankNameRead` and G7 `VariaxStoreRestore`, verified backup, the applicable #6/#22 decision and a separately approved restore procedure. Review how to reject or preserve a live edit before selecting a bank and verify safe return and cleanup. Obtain direct owner approval for a bounded first owner run and separate compile-time path approval before enabling bank-name storage for ordinary use. Open G5/G7 and either of the preset switches confer no bank-name approval. [Project hardware rules](../../../../AGENTS.md) · [Gate definitions](../../../../docs/hardware-gates.md) |

## Acceptance cases

These cases define future tests. This checkpoint changes documentation only.

| Offline case | Expected result |
| --- | --- |
| Clean, name-only, model-only and mixed on a verified attachment | Both counts and the empty state are accurate. Renaming one bank never adds a preset target. Renaming two banks to the same title yields two distinct indexed rows. Bank-name storage is unavailable and the model overwrite lists only existing pending positions. |
| Undo, redo, rename back, invalid input, and trailing-space-only edit | The pending marker reflects the captured readable name. File dirtiness follows document bytes. An invalid rename leaves both states intact. |
| Local discard, then Undo | Discard restores the listed pending names from the captured baseline in one undo step and preserves model edits. Undo makes the names pending again. No device worker starts. |
| Save a copy, close tab, quit, disconnect or reconnect | Save a copy leaves the current attachment’s pending warning open for Keep editing or Discard changes. A stale confirmation cannot apply to a replaced library or attachment. Historical files still use the file prompt and never arm device Sync. |
| Existing preset regressions | Equal-byte Paste marks the destination; Read unmodified and Write selected retain one-to-one scope. A verified model result leaves every bank-name marker intact. Preserve preset batch stop and the zero-store exit regression. |
| Later separate device proposal, fake transport | Wrong identity or selection, missing or stale backup, closed path, malformed response, mismatch, silence, cancellation and possible partial send must exercise the admission and stop rules. Update a bank baseline only for a verified result bound to the originating attachment and document. An uncertain result never clears the marker or starts another write. Owner-approved physical persistence and recovery evidence belong to that later checkpoint. |

## Reference pins

- Workbench Windows 1.75 build 121, 2012-07-09. The local installer
  `Variax Workbench v1.75 Installer.exe` has SHA-256
  `a6870ad2792fb0ee70683be0402cbc828ac316dcd1aa66abb2813cdc1df40f99`.
  The Windows extraction paths used were `line6editbase/app/L6EditorApp`,
  `line6editbase/librarian/L6Librarian`, `cmn/l6java/device/L6Device`,
  `cmn/l6java/patch/L6PatchManager`, and
  `modules/variax/device/L6VariaxPatchManager`. The `L6PatchManager.class`
  inspected with `javap` has SHA-256
  `f34e32d8e3517f16131d05ccc3b0f5196f6b94fbd7a2f7d7ea81fb2574a70717`.
  The 2026-11-23 local comparison in #205 is background for the preset flow.
- Firmware 3.10 static findings use the archive SHA-256
  `e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a` from
  the pinned [#181 trace record](../../../../research/variax-310-flash-handlers.json)
  at `5eddd17`. The [#175 transaction report](../../../../docs/variax-write-transactions.md)
  supplies previous host transcript and fake-transport evidence. No vendor
  application or firmware was executed for this checkpoint.
