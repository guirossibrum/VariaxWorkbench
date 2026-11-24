# Reso-5 factory store test card

Owner run only. Review the flash-interruption findings from #181 item 1 and
wait for the separate go for #177 before running the live command. The target is Reso, lever position 5, flash slot 50 on a Variax
300/600 with firmware 3.10 through one PODxt Live with firmware 3.01. The two
guitar models share a MIDI identity. The tool reads the active name and settings
and the owner verifies the physical selector position.

## Prepare and run

1. In Device check, choose **Back up the Variax**. Keep the completed folder
   from the same UTC day and close the app and any other editor.
2. Use stable power for the PODxt, which powers the guitar. Keep volume low.
   Leave cables connected and keep the PODxt on the same channel throughout.
3. Select **Reso, position 5** on the guitar.
4. In the issue worktree, run the offline check with the private firmware archive
   and the completed backup.

   ```sh
   cargo run --offline --locked --no-default-features --features owner-reso5-test \
     --bin variax-reso5-test -- --check store BEFORE_BACKUP ARCHIVE.vxf LOG_ROOT
   ```

5. After the owner's go, run the same command with `store` in place of
   `--check store`. The tool checks both identities and reads the guitar's
   settings before asking for `STORE RESO-5 FACTORY PROGRAM TO SLOT 50`.
   Enter that exact phrase only when ready. Each invocation creates its own
   log folder. To print this card separately, pass `--store-card`.
6. Expect the eight-byte `0x71` confirmation within seconds, all 192 settings
   bytes equal on read-back, and no audible change. Select another preset
   and return to **Reso 5**. Expect normal sound.
7. Run **Back up the Variax** again in Device check. Run `--check store` with
   the new folder too. Compare the backups with the file-only command below.
   Expect 120 identical stored reply files and zero differences.

   ```sh
   cargo run --offline --locked --manifest-path tools/variax-probe/Cargo.toml -- \
     compare BEFORE_BACKUP AFTER_BACKUP
   ```

The 3,276-byte payload is the archived firmware's own slot 50 program. The
store assumes the guitar's original DSP words are still the factory words.
The checks compare metadata and the observed sound. Full stored DSP read-back
is still open under [#119](https://github.com/guirossibrum/VariaxWorkbench/issues/119).

## Stop rules

Stop and report any refusal, transport error, absent confirmation, unexpected
reply, differing settings on read-back, audible change, abnormal sound after
reselecting, or difference between the backups. The store wait logs and skips
only the three known Variax notices for clip, DSP overload and MIDI queue overflow.
Ctrl+C can cancel up to the store command. Once it is sent, wait for reply
handling, verification on success and USB cleanup. Keep the log folder and
never repeat the store before the log has been reviewed.

## If 0x71 does not arrive

**Do not power off immediately.** Flash operations may still be running.
Wait at least **2 minutes** without touching anything and note what the guitar
and PODxt do. Then try selecting another preset on the guitar. If the guitar
responds, take a backup and compare it with the first one. If it does not
respond, report before powering off so the next step can be decided with
[#181](https://github.com/guirossibrum/VariaxWorkbench/issues/181)'s findings.
Keep the log folder for review.

**Pending #181 item 1.** The effects of power loss during each phase of the
store and any startup recovery will be filled in from the reviewed flash
analysis.
