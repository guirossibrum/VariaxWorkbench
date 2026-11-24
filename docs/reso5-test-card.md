# Reso-5 Variax 600 edit-buffer test card

Use a Variax 600 on firmware 3.10 through one PODxt Live on firmware 3.01.
The guitar's MIDI identity is shared with the Variax 300. Confirm the physical
guitar is a 600 before entering the stage confirmation. Keep the amp volume low
and keep the PODxt on the same channel throughout. A linked channel can recall a
different guitar preset.

## Prepare

1. Use a complete **Device check Variax backup** with all 60 presets.
   The backup can be from an earlier day. Use it for both stages. The tool
   requires the backup's Reso-5 settings to match the archive and the live
   preflight settings.
2. On the guitar, select bank **Reso**, position **5**. Leave the knob there
   while each command runs. Close other editors or device checks.

## Run each stage after its separate go

Each invocation takes the backup folder, exact archived Variax 3.10 firmware
file, local host model manifest and a capture directory. Build and run from
the issue worktree with host math:

```sh
cargo run --locked --no-default-features --features owner-reso5-test \
  --bin variax-reso5-test -- stage-1 BACKUP_DIR ARCHIVE.vxf MODELS.tsv LOG_ROOT
```

1. **Stage 1.** Enter the stage 1 confirmation only after the owner's go.
   Expect identical settings, no audible change and uninterrupted playing
   after the full program has applied. Select another model, then return to
   **Reso 5** to reload the stored preset.
2. **Stage 2.** After a separate go, run the same command with `stage-2`.
   Expect −6.0 dB on read-back and clearly quieter guitar output. Reselect a
   different model, then **Reso 5** again. The volume should return to normal.
3. Back up all 60 Variax presets again. Compare both backups with the existing
   file-only comparison. Expect 120 identical stored reply blocks and zero
   differences.

   ```sh
   cargo run --locked --manifest-path tools/variax-probe/Cargo.toml -- \
     compare BEFORE_BACKUP AFTER_BACKUP
   ```

Use `--card` to print this card. Add `--check` before the stage to verify
the files and render offline. Each invocation creates a fresh log folder.

## Stop rules

Stop and report a refusal, lack of sound, a sound change during stage 1, an
unexpected reply, a settings mismatch, loss of normal sound after reselecting,
any transport error, or any difference between the before and after backups.
Press Ctrl+C to request cancellation and wait for session and driver cleanup.
Keep the log folder and never repeat a send after an error before reviewing it.

The probe sends an edit-buffer program and demonstrates settings read-back.
Listening and the two library backups are separate owner checks. The probe
uses the two known metadata reads for words 851–914.


## Local program reference

Use the private model manifest described in [the host renderer](host-pipeline.md#local-use).
If you need to create one, the offline replay can write the manifest and special
pickup snapshots to a new ignored directory. Set `artifact_root` to the local
checkout with the archived firmware, extracted Workbench classes, model
resources, JDK and Unicorn environment.

```sh
CARGO_TARGET_DIR=target/reso5-std cargo build --offline --locked \
  --no-default-features --bin variax-workbench --bin variax-firmware-inspect
"$artifact_root/.tools/python/bin/python" scripts/verify_reso5_edit.py \
  --artifact-root "$artifact_root" \
  --archive "$artifact_root/vendor/installers/Variax300_3.10.vxf" \
  --extractor target/reso5-std/debug/variax-firmware-inspect \
  --host target/reso5-std/debug/variax-workbench \
  --models-dir target/reso5-models
```

The original Java overlay and emulated Mac coefficient code matched both
3,276-byte Rust programs. Stage 2's canonical SHA-256 is
`ad38b28c0390afced301367ea1cc9b2c0a5242cd1a4fb9a3250843c31f82175d`.
The 192 metadata bytes differ only in the trim field. Across all 45 tank and
allpass feedback words, each stage differs from firmware by at most one count
and the smallest coefficient margin below unity is 1,082,131 counts.
