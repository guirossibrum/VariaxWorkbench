# First Write to device and Sync (#205)

Run each procedure after separate owner approval, in a build that opens
only its own store switch. Use the Variax 600 on 3.10 and PODxt Live on
3.01. Leave Follow and Live editing off. Import a complete, verified
byte-level library backup and prepare the candidate Workbench reference
images using the [reference recipe](live-edit-test-card.md). The source
backup stays intact. Test deliberately audible variants and keep the exact
source `.vwm` files. Agree on the restore commands for each target before
starting. For Spank 1 and Spank 2 these are flash positions 14 and 13 in
the [approved restore procedure](../../../../docs/variax-any-selection-test-card.md).

## Write to device

1. Select **Spank 1** on the guitar and confirm the normal sound. In the
   editor, prepare the tuning-enabled variant on Spank 1. Right-click **Copy**
   on Spank 1 and **Paste** on **Spank 2**. Both positions should be marked
   modified. Right-click Spank 1 and **Read unmodified from device**.
   Confirm that the original Spank 1 plays and that its marker clears.
2. Right-click **Spank 2** and choose **Write to device**. Check that the
   confirmation names Spank 2 and the intended model, then select
   **Overwrite**. The original Spank 2 will sound during the preflight.
   The edit may sound during upload. Wait for the verified result, then
   play the strings and check that the tuning-enabled Spank 2 plays.
3. The editor compares the selected slot’s old image with its backup.
   It uploads once with `0x43`, waits 100 ms, sends one `0x61` for flash
   position 13, and waits up to 15 seconds for `0x71`. It reloads Spank 2
   and checks all 3,276 bytes against the firmware-stamped program,
   including its 192 settings bytes and checksum. Check that only the
   verified Spank 2 marker clears. Restore and verify the original Spank 2
   via the approved procedure before moving on to Sync.

## Sync

1. With the original Spank 1 and Spank 2 restored and backed up, prepare
   one distinct approved edit in each position. Use a clear tuning change
   in one and a clearly different pickup or body in the other. Start on
   **Spank 1** and confirm that the guitar still plays its unmodified sound.
2. Choose **Sync**. Confirm the dialog reports two modified Models. Choose
   **Save modified Models to Variax**, then review and confirm **Overwrite**
   for Spank 1 and Spank 2 only. The guitar plays each original during its
   preflight and reloads each edited sound for verification. When the
   process finishes it plays the last verified position, Spank 2. Play
   both slots after completion and compare their sounds with the planned
   edits. Each slot gets the same byte checks and single store as above.
3. Check that **Sync** now shows “There are no modified tones!” Restore
   both originals with the approved procedure. A separate Sync test may
   choose **Revert to unmodified Models** and confirm it replaces the
   editor copies with stored models without issuing a store.

Leave the knobs and selector alone during reads and stores. On silence,
keep the rig powered and untouched during the full two-minute wait. Stop
on silence, uncertain status, or any mismatch. Report which slots were
verified. Follow the result’s spare-group instructions before attempting
another write or restore.

## Offline close and quit checkpoint

The headless checks use a simulated verified attachment and a synthetic
library. They exercise both closing the Variax tab and quitting the app.

- With pending positions in a library downloaded on the current
  attachment, the warning counts those positions. The count includes a
  pasted position whose model bytes match its previous contents.
  **Keep editing** cancels and preserves the document, pending markers
  and attachment. **Save a copy…** saves the library bytes to a new file
  and keeps the warning open. **Discard changes** allows closing with
  positions still pending.
- An edited historical library uses the file prompt. **Keep editing**
  preserves its edits. A clean library or standalone preset closes directly.
- Close and quit never start a guitar store. **Write to device** and
  **Sync** have separate confirmations in the procedures above.
