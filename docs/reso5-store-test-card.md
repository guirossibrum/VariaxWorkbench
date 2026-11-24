# Reso-5 slot 50 store ladder test card

Owner run only. Each step needs its own owner approval after the reviewer
explains the procedure. Complete and review each step before asking for the next.

| Step | Mode | Program to send | Approval |
| --- | --- | --- | --- |
| 1a | `rename` | The captured Reso-5 program with name `Reso-5 T`, space padding and a recomputed checksum | **Approved 2026-11-24 for one owner run** |
| 1b | `restore` | The original captured Reso-5 program | **Not approved** |
| 2 | `factory` | The Variax 3.10 archive's slot 50 factory program | **Not approved** |

## Prepare each step

1. Use stable power, leave cables connected, and keep the amp volume low.
   The rig is Variax 300/600 firmware 3.10 through PODxt Live 3.01.
2. Take a complete **Device check Variax backup** on the same UTC day.
   The tool requires the backup to match the settings expected before this
   step. For restore, the backup must show `Reso-5 T`. Keep the original
   pre-rename backup as well. Close the app and any other editors.
3. Select **Reso, position 5** by hand. Keep the PODxt channel and the guitar
   controls steady. The tool sends no program change.
4. Supply the fingerprinted 3.10 archive and the original 35-file #234
   capture from the owner run. The capture is the same input for all three
   modes. Run the offline check with the mode from the table.

   ```sh
   cargo run --offline --locked --no-default-features --features owner-reso5-test \
     --bin variax-reso5-test -- --check store MODE BEFORE_BACKUP ARCHIVE.vxf CAPTURE_DIR LOG_ROOT
   ```

The offline check verifies the archive, same-day backup, capture and
checksums, then prints and logs the expected stored-image SHA-256. The
captured program is pinned to `3d8bae813991fa5996fb649ad80d116de8ef358c03724770183b89cdef5ad249`.
The firmware writes the four characters of `0310` to words 767–770 and puts
the low 24 bits of the sum of words 0–1,090 in word 1,091.

| Mode | Expected stored-image SHA-256 |
| --- | --- |
| Rename | `75bcaaf5f775bcd2f5ba22289aa4b8e9cac0c6115b4ac32b75144b4ccb5c716b` |
| Restore | `3d8bae813991fa5996fb649ad80d116de8ef358c03724770183b89cdef5ad249` |
| Factory | `90d79131f7909639aa2d0793cb3f617fbd95a95a1ada7a54f90ec84de4e28164` |

## Run after separate approval

1. Use the reviewed owner build. Remove `--check` from the command. The tool
   checks both identities and the 192 settings bytes, then reads the active
   buffer in 35 chunks. It requires an exact match to the original capture
   before rename or factory and to the renamed stored image before restore.
   It saves each completed reply in the new log folder.
2. Enter the displayed mode-specific phrase only when ready.

   - `STORE RESO-5 RENAME PROGRAM TO SLOT 50`
   - `STORE RESO-5 RESTORE PROGRAM TO SLOT 50`
   - `STORE RESO-5 FACTORY PROGRAM TO SLOT 50`

3. The tool uploads the program (`0x43`), waits 100 ms, sends `61 32` once and
   waits up to 15 seconds for `0x71`. Ctrl+C can cancel before `0x61`.
4. After the confirmed store and cleanup sends, follow the prompt. Select
   another preset and then return to **Reso, position 5** by hand. Enter
   `RESO 5 RESELECTED` after doing this. The tool checks all 1,092 reselected
   words against the expected stored image and checks all 192 settings bytes.
   The checks include words 871–872.
5. Confirm normal sound, take another Device check backup and compare it with
   the before backup. Reports should show the intended slot 50 settings for
   this mode and no changes to the other 59 presets. Keep both backups and
   the full log. Stop and report any other difference. Review the result
   before the next step.

## Stop rules

Stop and report any refusal, incomplete read, transport error, missing or
unexpected confirmation, full-image or settings mismatch, audible change,
or unexpected backup difference. Keep all completed chunk captures. After
`0x61`, wait for reply handling, verification and USB cleanup. The tool
never retries a store.

## If 0x71 does not arrive

**Keep the rig powered and untouched.** After a silent 15-second wait and
successful cleanup sends, the tool waits another **2 minutes** before releasing
USB. It sends no more MIDI during this phase.

Try selecting another preset. If the guitar responds, take a **Device check
Variax backup**, compare it with the before backup, and report the result.
If the guitar does not respond, report before powering off. Keep the store log.

**Do not store any slot in 40–59 until spare 62 is known blank.** The PODxt
Live route cannot read spare 62. The firmware flash polls have no timeout and
there is no spare-sector recovery at startup. Power loss between slot erase
and confirmed programming can leave slot 50 unusable until a later successful
store. [Flash trace](firmware-variax-dispatch.md#preset-store-0x61)
