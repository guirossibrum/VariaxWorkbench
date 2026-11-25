# Variax store from another selection (#238)

Each rung needs separate owner approval after review.
Use a Variax 600 on firmware 3.10 and a PODxt Live on firmware 3.01.
Keep power stable and leave the preset selector and the PODxt channel alone
while the tool is running.

| Rung | Mode | Flash slot and MIDI program | Source | Approval |
| --- | --- | --- | --- | --- |
| 4a | `listen` | 14, Spank 1, MIDI 69 | Edited `Stratocaster.vwm` | **Completed. Any repeat needs separate approval.** |
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

Start on a different unedited preset for a store. The following example
uses **Reso 4** as the starting preset and **Spank 1** as the target.
The tool prints your actual preset names. A clean full read takes about
seven seconds. Retries can lengthen any wait.

| Step | HEAR before the step | SEE before the step | Result after the step |
| --- | --- | --- | --- |
| Check the PODxt Live | Your current preset stays selected while the tool checks the rig. | Wait for the PODxt Live check. Nothing to type. | PODxt Live verified. |
| Identify | Your current sound stays selected during the Variax identity and full-buffer checks. | Wait for the starting preset's name. Nothing to type. | Starting preset is Reso 4, verified. |
| Check target | Spank 1 replaces Reso 4 while the tool reads it, then Reso 4 returns. | Wait for the Spank 1 result and the return to Reso 4. Nothing to type. | Spank 1 matches the expected current program. |
| Return before upload | Reso 4 returns and stays selected. | Wait for verification. Nothing to type. | Back on Reso 4, verified. |
| 4a confirm and verify | Reso 4 stays selected until you confirm. The edit then plays while the tool checks it and while you audition it. | Type `LISTEN SLOT 14 Spank 1` to load the edit temporarily. Wait for **Play now**. | The working buffer matches the edited program. |
| 4a play and return | The prompt compares the edit's body, pickups and tuning with Reso 4. The edit stays loaded until you finish. | Play strings only. Leave the knobs and selector alone. Type `RETURN TO ORIGINAL` when finished to reload and verify Reso 4. | Back on Reso 4, verified. The summary says the edit was sent and nothing was saved. |
| Store confirm | Reso 4 stays selected until you confirm. The sound may change during upload and the save wait. The tool then checks the target and returns to your starting preset. | For 4b, type `STORE SLOT 14 Spank 1`. For another rung, use the displayed mode and target. Wait up to 15 seconds for the save reply, then wait for verification. Leave the guitar controls alone. | The guitar confirmed the save to Spank 1. |
| Verify saved target | The tool reloads Spank 1 for a full read and settings check, followed by Reso 4. | Wait for the saved-image and settings checks. Nothing to type. | Spank 1 matches the expected saved sound and settings. |
| Return after store | Reso 4 returns and stays selected. | Wait for verification and the final summary. Nothing to type. | Back on Reso 4, verified. The summary names what was sent, saved and selected. |

The tool refuses a missing or ambiguous starting-image match. Slot 50
uses the verified post-#177 factory image `90d79131…8164`. Rung 3b
expects the renamed factory image from 3a. Rung 4c expects the stamped
image from 4b and must reproduce the backup file byte for byte
(`4e6b812d…` for Spank 1).

The 4a source used in the owner run had alternate tuning off. Its six
semitone offsets were saved in the settings, with pitch shifting disabled
in the compiled sound. Reloading a 4b store of that file should give the
edited tone without the six programmed pitch offsets.

The 4a audition checks all 1,092 words after the temporary `0x43` upload.
A store uploads with `0x43`, waits 100 ms, sends `0x61` once, and waits up
to 15 seconds for `0x71`. Cancellation is honored before `0x61`. After
confirmation, the verification checks the full stamped image and all 192
settings bytes, followed by a byte-for-byte check of the starting preset.

## Stop and report

Stop on any mismatch, transport error, refused confirmation, or changed
sound outside the audition. Keep the log and completed chunks. The tool
attempts to return to the identified snapshot after a failed preflight,
audition, or cancelled upload. It will not retry a store.

If `0x71` is silent, **HEAR:** the edit may remain loaded. Keep the rig
powered and untouched for two more minutes. **SEE:** wait for the stop
message, then follow the guitar check and backup instructions. The tool sends no further selection or read requests. Then check
whether the guitar responds, take a Device check backup if it responds,
and report with the log. If it does not respond, report before powering off.
The tool names the affected 20-slot group and its spare. For slot 14,
do not store in 0–19 until spare 60 is known blank. For slot 50, do not
store in 40–59 until spare 62 is known blank.
