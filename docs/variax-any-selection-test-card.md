# Variax store from another selection (#238)

Each rung needs separate owner approval after review.
Use a Variax 600 on firmware 3.10 and a PODxt Live on firmware 3.01.
Keep power stable and leave the preset selector and the PODxt channel alone
while the tool is running.

| Rung | Mode | Flash slot and MIDI program | Source | Approval |
| --- | --- | --- | --- | --- |
| 4a | `listen` | 14, Spank 1, MIDI 69 | Edited `Stratocaster.vwm` | **Approved 2026-11-25 for one owner run** |
| 4b | `store` | 14, Spank 1, MIDI 69 | The same `Stratocaster.vwm` | **Not approved** |
| 4c | `restore` | 14, Spank 1, MIDI 69 | #236 `14-Spank-1.bin` | **Not approved** |
| 3a | `3a` | 50, Reso 5, MIDI 105 | Archive Reso-5 renamed to `Reso-5 T` | **Not approved** |
| 3b | `3b` | 50, Reso 5, MIDI 105 | Archive Reso-5 | **Not approved** |

## Offline check

Use the complete #236 backup `variax-full-library-20261124T234218Z` and the
fingerprinted 3.10 archive. The check prints the upload hash, the expected
image hash, and the settings diff against the target's backup. Review each
changed parameter before confirming any future live run.

```sh
cargo run --offline --locked --no-default-features --features owner-reso5-test \
  --bin variax-reso5-test -- --check any-selection MODE FLASH_INDEX FULL_BACKUP ARCHIVE.vxf SOURCE MODELS.tsv LOG_ROOT
```

For 4a and 4b, use `Stratocaster.vwm` as SOURCE and provide the local model
manifest used by #176. For 4c, pass the same 4b source and manifest so
preflight can reconstruct the current slot image. The upload bytes come
from the target's file in the verified #236 backup. For 3a and 3b, pass
`archive` as SOURCE and `-` for the unused model manifest. In generic
listen and store modes, use `archive` for the target's factory program,
`archive:NN` for a different factory program, or a checksummed backup
`.bin` as SOURCE. Flash indices run from 0 through 59.

## Run after separate approval

1. Start on a different unedited preset for a store. The tool verifies both
   identities and matches a 35-chunk active-buffer read against all 60
   expected images. It stops if the match is missing or ambiguous. Slot 50
   uses the verified post-#177 factory image `90d79131…8164`. Rung 3b
   expects the renamed factory image from 3a. Rung 4c expects the stamped
   image from 4b in its target slot.
2. The tool switches to the target, compares its full program, then switches
   back and verifies the original snapshot byte for byte. Review the
   printed diff and type the displayed mode and slot confirmation.
3. For 4a, the tool uploads the working buffer with `0x43` and compares all
   1,092 words. Play after verification. Type `RETURN TO ORIGINAL` to reload
   and verify the original preset.
4. A store uploads with `0x43`, waits 100 ms, sends `0x61` once, and waits up
   to 15 seconds for `0x71`. Cancellation is honored before `0x61`. After
   confirmation, the tool reloads the target, compares the full stamped
   image and all 192 settings bytes, then reloads and checks the original.
   The restore must reproduce the backup file byte for byte. For Spank 1,
   the expected restore SHA-256 begins `4e6b812d`.

## Stop and report

Stop on any mismatch, transport error, refused confirmation, or changed
sound outside the audition. Keep the log and completed chunks. The tool
attempts to return to the identified snapshot after a failed preflight,
audition, or cancelled upload. It will not retry a store.

If `0x71` is silent, keep the rig powered and untouched for two more
minutes. The tool sends no further selection or read requests. Then check
whether the guitar responds, take a Device check backup if it responds,
and report with the log. If it does not respond, report before powering off.
The tool names the affected 20-slot group and its spare. For slot 14,
do not store in 0–19 until spare 60 is known blank. For slot 50, do not
store in 40–59 until spare 62 is known blank.
