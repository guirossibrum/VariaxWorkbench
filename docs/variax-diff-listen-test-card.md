# Variax diff-listen audition (#245)

**Not approved.** The live switch is closed. Difference sends have only been
checked offline. Use the offline check until the owner approves the reviewed run.

## Offline check

Supply the flash index, complete byte-level backup, fingerprinted Variax 3.10
archive, local Workbench model manifest and a log root. For Spank 1 use index 14.

```sh
cargo run --offline --locked --no-default-features --features owner-reso5-test \
  --bin variax-reso5-test -- --check diff-listen 14 FULL_BACKUP ARCHIVE.vxf MODELS.tsv LOG_ROOT
```

The output names the preset, shows its trim change and prints the SHA-256 of
the starting and expected working images. The tool changes only output trim
by −6 dB. It refuses if a word needed for the edit differs between the saved
program and the host's render before the edit.

## Owner procedure after approval

1. On a Variax 600 firmware 3.10 through a PODxt Live 3.01, close any other
   editor or Rig Check. Select the named preset by hand and leave its knobs
   steady. Remove `--check` from the command. **HEAR:** The selected sound
   stays loaded. **SEE:** Wait for both identities and the full starting
   program to be verified against the library. There is nothing to type.
2. **HEAR:** The preset should become quieter. Its body, pickups and tuning
   stay the same. **SEE:** Wait for the full read-back and **Play now**. The
   tool sends one `0x3B` and waits up to one second for `0x71`. It compares
   all 1,092 words, including the original checksum word that the difference
   leaves in place.
3. Play the strings only and leave the knobs alone. Press Enter when done.
   **HEAR:** The unedited preset returns. **SEE:** Wait for the return
   verification and summary. The guitar remains on the starting preset.

The tool sends neither a full upload nor a store. If the difference
transfer fails or its reply is ambiguous, stop and reselect the starting
preset by hand. If read-back fails after acknowledgement, the tool still
attempts to reload and verify the saved preset. Keep the log and report a
failed or unverified return.
