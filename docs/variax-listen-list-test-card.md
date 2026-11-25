# Variax listen-list audition (#238 follow-up)

**Approved 2026-11-25 for one owner run.** This run uses the four test
models in order A–D. The list uses temporary working-buffer uploads and
returns the guitar to its verified starting preset.

## Offline check

Supply the complete #236 backup, the fingerprinted Variax 3.10 archive,
the corrected local #176 model manifest, a log root, and one or more
editor `.vwm` paths in the desired listening order.

```sh
cargo run --offline --locked --no-default-features --features owner-reso5-test \
  --bin variax-reso5-test -- --check listen-list FULL_BACKUP ARCHIVE.vxf MODELS.tsv LOG_ROOT A.vwm B.vwm
```

The check renders every input before an audition can start. It prints the
file order and the SHA-256 of each upload and expected working image.
Review the oracle comparison for the files you intend to audition.

## Owner procedure after separate approval

1. Use the Variax 600 firmware 3.10 and PODxt Live firmware 3.01 rig. Start
   on **Spank 1**, unedited, with its restored #236 program. The full
   program must match exactly one library entry.
   **HEAR:** The starting sound stays selected. **SEE:** Wait for the tool
   to identify both devices and name and verify the starting preset.
2. For each file, **HEAR:** the tool loads the next sound and describes its
   body, pickups, and tuning against the starting preset. **SEE:** wait
   for the full working-buffer verification and **Play now**. Play the
   strings only and leave the knobs and selector alone. Press Enter when
   finished with that sound. The tool waits for a fresh Enter after each
   file, including the last.
3. After the last Enter, **HEAR:** the starting sound returns. **SEE:** wait
   for the starting preset to be verified and the summary. The tool sends
   one full `0x43` upload per file and saves nothing to the guitar.

Stop and keep the log on a mismatch, a refusal or any unexpected sound.
Once an upload has been attempted, a failed read-back, cancellation or
input error stops the list and triggers a verified return attempt. The
tool reports if it could not verify the starting preset.
