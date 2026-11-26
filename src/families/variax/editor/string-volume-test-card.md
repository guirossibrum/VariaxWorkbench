# String Volume Setup · #206 owner test card

**Draft. All three string-level approvals are closed.** Read and temporary
edit approval are required for the first test. Storage needs its own later
approval. Use the verified Variax 600 firmware 3.10 through the verified
PODxt Live firmware 3.01. The six levels are global and can affect every
model. A preset-library backup does not contain this calibration-like data.

## First test · read, preview and Cancel

1. Note the playing preset and its sound. Open **String Volume Setup…** on
   the verified guitar tab. The dialog offers six sliders in low E, A, D,
   G, B, high E order after the initial read and backup finish.
2. Keep the backup folder shown by the dialog. Verify its `MANIFEST.sha256`
   and the 18-byte `string-levels-ram.bin`. It holds the six original RAM
   words in wire order, high E to low E, three big-endian bytes per word.
   Record the folder and all six starting words before touching a slider.
3. Lower one string a small amount. Play that string and a neighboring
   string at comparable strength. Listen for the selected string's level
   change on the current modeled sound. Wait for verification to finish.
   The temporary set sends all six words with `0x54` and compares the
   `0x57` reply with the requested words in RAM.
4. Press **Cancel**. The six exact initial words are written back and
   verified against the backup with `0x57` if the levels changed. Check
   the six RAM words reported on success against the saved 18 bytes.
   Play the same strings and listen for
   the original balance. Keep the backup and any private capture. Stop on
   a failed set, a failed restoration, or a disconnect. Preset selection,
   preset files and stored library positions are outside this test.

## Later test · separately approved store and persistence

1. Start a fresh setup, preserve its initial RAM backup, change one
   level, and let the preview verify. **OK** opens a second confirmation.
   **Send store request** requires the store approval and G7.
2. The shared API reads `0x57` just before the store and compares all six
   verified RAM words. It sends one `0x56` request. Keep the original
   backup and its sibling `*.before-store-ram.bin`, which records the
   exact six words observed before the request. The editor reports
   **Store request sent** after transport delivery. A failed or uncertain
   send ends the session without an automatic resend or RAM restore after
   the attempted store. Preserve any evidence and stop.
3. Verify persistence only under a separately reviewed owner procedure
   that reloads the six words from flash and compares them with the
   pre-store file. The editor's `0x57` read-back is RAM evidence.
   The persistence and recovery procedure must be agreed before the
   first store test.

#181 traced `0x56` to erase and programming in the PE4-clear window at
numeric index 4, with no spare. The relationship to the bit-set preset
window remains unresolved. The dialog makes no flash-verification claim.
[Device API and trace links](../../../../docs/variax-string-levels.md).

## Separate dependency · piezo listening

Workbench 1.75's **Listen to piezo signal** calls `L6VariaxDevice.SetDSPEnabled`
to change DSP bypass, separate from the global string-level messages. It
needs a separately approved procedure that restores modeled output on
exit and handles an interrupted connection while bypassed.
