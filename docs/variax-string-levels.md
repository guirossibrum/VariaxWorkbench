# Variax global string levels

[#206](https://github.com/guirossibrum/VariaxWorkbench/issues/206) device API
for the Variax 300/600 on firmware 3.10 through a PODxt Live. The levels
affect every preset. This implementation has been tested with fake links.

| Command | Wire shape | Evidence |
| --- | --- | --- |
| `0x57` | 8-byte request. The 44-byte reply contains six low-nibble-first 24-bit words. | The reply shows the six words in RAM at the time of the read. |
| `0x54` | 44-byte set with all six words. | #181 traces a RAM write. The API reads the six words back with `0x57` before accepting the edit or restoration. |
| `0x56` | 8-byte store request. | #181 traces erase and programming from RAM through the PE4-clear window at numeric index 4, with no spare. The API reports transport delivery only. |

See [#181's String levels trace](firmware-variax-dispatch.md#string-levels-0x54-0x56-and-0x57)
and [Workbench's slider representation](factory-extraction.md#known-global-string-level-structure-unit-calibration-remains-separate).
These are static findings. The relationship between the PE4-clear window and
the bit-set preset window remains unresolved. A preset-library backup does not
include the global levels.

## API contract for lane 3

Use `line6_devices::family::variax::string_levels` on a worker that holds
the rig lock and owns the open Variax session through cleanup. The worker
can wait for dialog commands inside the edit callback. Keep the session
exclusive, and queue or coalesce slider changes. The API verifies the guitar
identity, enables the PODxt relay and Variax session, and attempts to release
both on return. The worker owns USB release and driver reattachment.

| API | Caller contract |
| --- | --- |
| `read_ram(&mut session)` | Returns `Levels` for a read-only preview. Requires `VariaxIdentity`, `VariaxRead` and `APPROVED_STRING_LEVEL_READ`. |
| `with_temporary_edit(&mut session, backup_root, backup_name, callback)` | Reads a fresh snapshot and completes a new backup before calling the dialog worker. Pass a private backup root and a fresh single-component folder name. Requires the read permissions, `VariaxEditBufferWrite` and `APPROVED_STRING_LEVEL_EDIT`. The app should require its backup and G6 gates for this path. |
| `edit.original()`, `edit.backup_folder()` | Display the verified starting RAM levels and the backup location. The manifest covers `string-levels-ram.bin`, which holds 18 bytes in wire order, most significant byte first within each word. |
| `edit.set_ram(levels)` | Pass six levels at once. A returned value is the verified RAM read-back. A failed set leaves restoration as the remaining action. |
| `edit.restore_ram()` | Cancel can call this explicitly, or return from the callback and let it restore. It writes the original raw words and reads them back. A failed restore is reported without retry, with the backup still available. |
| `edit.request_store()` | Call only on separately confirmed OK. Requires `VariaxStoreRestore` (G7) and `APPROVED_STRING_LEVEL_STORE`. The method checks that a fresh `0x57` still matches the verified RAM state, sends `0x56` once, and returns `StoreRequestSent { ram_before_request }`. Report “Store request sent” and retain the RAM evidence. |

`Levels::workbench_levels()` returns fractions in low-to-high E A D G B E
order. Use `edit.original().with_workbench_levels(values)` for slider
changes. It applies Workbench's alternating signs and its inversion
flag inferred from the original read. Raw backups preserve every bit.
Use `Levels::from_backup_bytes` for those exact words, even if the same
slider positions would quantize differently.

An ordinary callback return attempts restoration whenever RAM may have
changed and no store request was attempted. After attempting `0x56`, the
worker closes the dialog's edit session. It must never retry the request or
automatically restore RAM in that session. The crate attempts session and
relay cleanup after a transport error. Report any error and keep the backup.

All three approval switches are closed. The app's per-path gate and approval
must also be in place before it exposes device actions. Piezo listening
belongs to a separate procedure.

## Owner verification draft

**Not approved.**

1. Read the six RAM words and save and verify their backup. Confirm the file
   exists before any temporary edit.
2. With separate read and edit approval, change one string level and check
   its RAM read-back. Cancel and verify all six original words. A failed
   set triggers an attempt to restore. Stop further edits and keep the
   backup and transcript if either verification fails.
3. The store path needs separate approval. A matched `0x57` read-back is
   RAM evidence. Establish flash persistence in a separately reviewed owner
   verification that reloads the levels from flash and compares all six
   words. Stop after an uncertain send and keep the evidence.
