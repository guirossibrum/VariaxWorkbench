# Reso-5 factory store test card

Owner run only. Review the #181 flash-interruption findings and wait for the
separate go for #177. The target is Reso, lever position 5, flash slot 50 on a
Variax 300/600 with firmware 3.10 through a PODxt Live with firmware 3.01.
The two guitar models share a MIDI identity. The tool checks the active name
and settings, and the owner verifies the physical selector position.

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

The 3,276-byte payload is the firmware archive's slot 50 factory program. The
owner's #225 run on 2026-11-24 at 15:08 UTC received no `0x44` reply through the
PODxt Live, and the guitar worked normally afterward. The store procedure uses
the established identity and 192-byte settings reads. The original slot's DSP
words and the spare sector remain unverified on this route.

For future direct-interface readback only, the expected 3,276-byte stored
image has SHA-256
`90d79131f7909639aa2d0793cb3f617fbd95a95a1ada7a54f90ec84de4e28164`.
The firmware writes `0310` to words 767–770, then sets word 1,091 to the low 24
bits of the sum of words 0–1,090 (`A3219C`). The offline check verifies this
reference against the private archive.

## Stop rules

Stop and report any refusal, transport error, absent confirmation, unexpected
reply, differing settings on read-back, audible change, abnormal sound after
reselecting, or difference between the backups. The store wait logs and skips
only the three known Variax notices for clip, DSP overload and MIDI queue overflow.
Ctrl+C can cancel up to the store command. Once it is sent, wait for reply
handling, verification on success and USB cleanup. Keep the log folder and
never repeat the store before the log has been reviewed.

## If 0x71 does not arrive

**Keep the rig powered and untouched.** After a silent 15-second confirmation
wait and successful cleanup sends, the tool announces and waits another
**2 minutes**, then releases the interface and tells you to check the guitar.
It sends no more MIDI messages during this phase.

Try selecting another preset. If the guitar responds, take a **Device check
Variax backup**, compare it with the first one, and report the result. If the
guitar does not respond, report before powering off. Keep the store log.

**Do not store any slot in 40–59 until spare 62 reads blank.** The current
PODxt Live route cannot verify that condition. The [#181 static trace](firmware-variax-dispatch.md#preset-store-0x61)
shows that the firmware has no timeout on the flash polls or spare-sector
recovery at startup. A loss of power between slot erase and confirmed
programming can leave slot 50 unusable until a later successful store.
