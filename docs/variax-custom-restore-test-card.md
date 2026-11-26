# Variax Custom 1 position 1 edit and restore (#178)

**Offline preparation approved. Live run not approved.** Owner Inbox
`variax-custom-restore-178` revision 1 selected captured-DSP recovery for
offline preparation. The proposed target is **Custom 1, selector position 1
(bridge end)**, `Preset::new(0, 0)`, MIDI program **4**, flash index **4**.
The proposed edit is output trim from **0.00 to −6.00 dB**. Spare **60**
is shared by flash **0–19**. Custom 2 is outside this card.

## Historical file evidence and open prerequisites

- The historical #236 backup validated as 60 complete programs, 60
  settings pairs and 12 bank names. For flash 4, the 3,276-byte captured
  image matched its separate metadata pair, passed `write::Program::new`,
  and already carried the `0310` stamp. An independent calculation of both
  upload checksums and the stored image agreed with the shared Rust
  procedures. All 3,276 bytes survived stamping unchanged. A synthetic
  image with a valid upload checksum and zero version words changed on
  stamping, so a valid upload alone does not establish exact recovery.
- The historical trim candidate is **ineligible**. The stamped unedited
  render differs from the capture in **56 of 1,092 words**. Changing
  output trim to −6.00 dB changes only the trim bytes in the settings
  slice, and words 555, 872, 873 and 1,091 between the two stamped renders.
  Compared directly with the capture, the proposed edited render still
  changes **55 words outside that render pair's delta**. This pair does
  not establish that its changed words can be transplanted into the
  captured program. No patched candidate was produced. The #278 oracle
  sweep validates the renderer for its test inputs; the fresh target
  still needs its own exact edit proof.
- #238 verified completed stores in this group, including the restore of
  Spank 1 in flash 14. #181's static trace shows the final spare erase
  completes before `0x71`. #225 received no `0x44` reply through the PODxt
  Live, so the current contents of spare 60 cannot be measured on this rig.
  An unknown or interrupted store in flash 0–19 after the last confirmed
  group store blocks this card until the spare is known blank. If relying
  on a clean incident history and the last confirmed store, state that
  inference and have the owner review it explicitly.

## Recovery and comparison data

1. Use the verified Variax 600 (300/600 identity, firmware **3.10**) and the
   PODxt Live **3.01** on one matched attachment. Start on a known, unedited
   preset other than the target. Keep the PODxt channel and guitar knob
   positions fixed. A bank label or patch name alone does not identify the
   slot. Require a unique full-image match for the starting selection.
2. Make a new, complete **full-program** G5 backup on the same rig before
   approval. Require a validated manifest and identity, 60 complete
   checksum-verified **3,276-byte** canonical big-endian images, all 60
   metadata pairs and all 12 bank-name replies. Preserve the starting image
   and selection, the sweep's verified return and session/link cleanup
   evidence. Stop on uncertain return, incomplete capture, checksum failure
   or ambiguous identity. Keep the original tree immutable and retain a
   separate verified non-overwriting copy. An ordinary settings backup,
   a handbook assignment, and a firmware archive cannot provide the
   owner's exact restore image. The historical #236 capture is file evidence
   only. For example, its slot 50 predates the final #177 store.
3. Pin the target recovery file `programs/04-Custom_1-1.bin` to the fresh
   manifest. Its packed metadata (words 851–914) must agree with both
   captured metadata replies. Record the decoded `save_tone_position`
   flag and the represented tone position separately. Preserve both and
   compare all 192 settings bytes at each checkpoint. Metadata bytes 61–63
   can reflect knob position; byte 64 carries the save flag. Do not mask
   words 871–872 or the checksum for this card. If stable exact reads
   cannot be reproduced with controls held fixed, stop.
4. Validate name padding and both upload sums with
   `write::Program::new`: the low 24 bits of the big-endian word sums over
   0–1,088 (host) and 0–1,090 (firmware) must each equal word 1,091.
   `procedures::expected_stored_image` writes ASCII `0310` into words
   767–770 and replaces word 1,091 with the firmware sum. Apply this
   prediction to the planned edit and restore uploads before a run.
   **The predicted restore must equal the recovery file byte for byte.**
   Record the private hashes for the original, both upload images and
   both predicted stored images, and review the full edit word diff.
   Refuse disagreement even if both checksums are independently valid.
5. Verify the proposed trim edit against the fresh, reloaded capture. Stop
   if its starting trim differs from 0.00 dB or the slot differs from the
   reviewed target. Record where the unedited model came from and pin the renderer, math
   profile and model data. For a paired-render proof, require the stamped
   no-edit render to equal the capture across all 3,276 bytes. Change
   only the trim field in that same model and record the exact expected
   changed byte offsets and old/new values, including any trim-dependent
   DSP bytes and checksum. Require a direct comparison of the stamped edit
   with the capture to match that expectation. Confirm every byte outside
   the expected changes is equal and all raw settings bytes outside trim
   are equal, including the tone mode and position. Treat the historical
   word indices as evidence about those renders. A derived candidate with
   no independently verified baseline and byte-level change provenance
   is a blocker.

## Proposed owner sequence after separate approval

Obtain separate owner approval for the edit and the exact-image restore
before starting any write. Each store also has its own typed confirmation.
The displayed phrase, mode, target, settings diff and expected hashes must
agree with the reviewed plan. Each
store uses the shared routine: one `0x43`, 100 ms, one `61 04`, and one
15-second wait for `0x71`. Never retry the upload or store automatically.
Cancellation is honored up to `0x61`. A successful read-back requires a deliberate preset reload and all 35 `0x3C` chunks.
These are recalled program images through the relay. There is no direct
sector or spare read in this procedure.

| Checkpoint | Hear and see | Required evidence |
| --- | --- | --- |
| Baseline | During the sweep the guitar changes presets and then goes back. Play Custom 1 position 1 when asked to establish the starting sound. | Reloaded target and 192 settings bytes match the fresh baseline. Original selection and image are known. |
| Edit | Type `STORE EDIT CUSTOM 1 POSITION 1 FLASH 4` only when ready. Wait for the reload result before playing. With controls fixed, the same phrase should sound quieter. | First `0x71`; full target read and settings equal the pinned edited image. Make a complete intermediate backup. Exactly the target differs among all 60 programs and all 59 other metadata pairs are unchanged. All 12 bank names are unchanged. |
| Restore | Type `RESTORE CUSTOM 1 POSITION 1 FLASH 4` only after the edited target is reread and still matches the pinned edited image. Wait for the reload result, then play the same phrase. | Second `0x71`; full target read and settings equal the original, and the original sound returns by ear. A complete final backup matches all **60 programs**, **60 metadata pairs**, and **12 bank names** by slot or bank against the baseline. Verify the starting selection and its original full image, then verify cleanup. |

Retain the full baseline, intermediate and final backups, counts, indexed
differences, reply times, return/cleanup evidence and listening notes locally.
A generic backup-folder comparison covers stored `.syx` entries and does not
prove full-program or bank-name equality. Compare each validated program and
bank-name reply explicitly. Original/current metadata outside the slot pairs
is provenance, not a stored entry. Record only reviewable summaries in #22
and the status ledger after an owner run.

## Refuse, stop and report

- Before any store, refuse on the wrong attachment, identity, firmware,
  target, active selection, wrong confirmation, unstable power or controls,
  any incomplete evidence, an unexplained word change, or a mismatch in
  preflight, recovery bytes, stamp or checksum. Treat an unmatched or
  duplicate starting buffer as ambiguous. Do not choose another slot.
- After `0x61`, an unexpected reply, transport/cleanup error, failed
  reload, any comparison failure, or any unexplained audible result stops
  the ladder. A failed edit verification does not authorize the second
  write. A failed restore blocks all further writes. Keep the evidence and
  ask for a separate recovery decision.
- If `0x71` is silent, keep the rig powered and untouched for two more
  minutes after the 15-second wait. End the workflow after the shared
  transaction cleanup, with no further preset changes, reads or writes
  from the store path. Never resend. Then the owner checks whether the
  guitar responds, takes a read-only Device check backup if it does, and
  reports. A responsive guitar or a readable target does not establish
  that the spare is clear. **Do not store any slot in flash 0–19 until
  spare 60 is known blank.** There is no automatic spare-sector recovery.
- No bank-name write, string-volume operation, alternate-tuning edit,
  multi-slot Sync, direct flash read or erase, or firmware operation.

## What is already available

- #177 verified version stamping and reselected full-image comparison on
  Reso 5. #238 verified capture-based exact restoration on Spank 1, outside
  the Custom banks. Neither is a Custom-slot result.
- [The backup API](../crates/line6-devices/src/family/variax/backup.rs)
  provides the full sweep, per-program checksums and partial-capture
  reporting. Its strict `validate_snapshot` validates the saved files;
  verified return and cleanup also require evidence from the capture.
- [The shared procedures](../crates/line6-devices/src/family/variax/procedures.rs)
  can reload and compare the full image and stop on an ambiguous store.
  [The editor](../src/families/variax/editor/procedure_device.rs) checks the
  target's current bytes against the verified backup before storing a
  rendered model. A second editor Write using the original backup would
  refuse the intentionally edited target. Rig Check 5.3 uses a rendered
  trim change and then the captured image; that general catalogue step is
  outside this one-slot approval. [The #238 owner route](variax-any-selection-test-card.md)
  models the two expected target states and exact restore rule. Its
  historical input assumptions and closed approval switches remain.
  There is no approved end-to-end Custom runner in this checkpoint.

## Focused synthetic rehearsal

The [test-only rehearsal](../crates/line6-devices/src/family/variax/backup/tests/custom_restore.rs)
runs the shared store, full backup and reselect procedures through a
scripted fake link. It uses distinct synthetic bytes for each preset and
validates the written snapshot files before comparing by preset or bank
key. The success case confirms the edit and restore separately, consumes
two `0x71` replies for exactly two slot-4 stores, and checks **60 programs,
60 settings pairs and 12 bank names** at each of the three checkpoints.
The preflight checks both uploads with `write::Program::new` and uses an
independent checksum and stamp calculation as an oracle for the shared
firmware prediction. The synthetic baseline assumes unity DSP gain.
The allowed edit changes the 24-bit gain at word 555 from `ffffff` to
`c026e7` (trim flag set), and the three-byte trim at settings bytes
65–67 from `000000` to `c0c000` (0 to −6 dB). Every other byte must
match, apart from the verified checksum and the canonical upload stamp
that becomes `0310` in the predicted stored image. In the stamped pair
the exact changed byte offsets are 1665–1667, 2618–2619 and 3273–3275.
The oracle rejects one-bit corruption at each of the 3,276 byte offsets
and compensating changes that preserve both upload checksums. The
original recovery image is a fixed point of both stamp calculations
across all 3,276 bytes. A checksum-valid unstamped recovery image is
refused.

An offline test in the existing Reso-5 tool also renders the legacy
Default Patch with the built-in model library, treating the stamped
baseline as a synthetic capture. Changing only trim lowers the DSP
gain and changes only words 555, 872, 873 and 1,091. It compares all 192
settings bytes against the exact trim substitution and verifies that
restamping preserves the baseline. Neither synthetic test supplies the
provenance or gain coefficient for the owner's current slot.

| Phase | Required result |
| --- | --- |
| Baseline | Exact fresh capture of all 60 programs, 60 settings pairs and 12 names. A reloaded slot 4 and its settings match that capture. A verified starting selection returns byte for byte. Neither `0x43` nor `0x61` is sent before both input images and the recovery stamp are verified. |
| Edit | Wrong or absent Edit confirmation sends no store. Success sends one `61 04` after the pinned edit upload, receives `0x71`, reloads and reads all 35 chunks, then checks the intermediate backup. The target equals the predicted edit. The other 59 programs and settings pairs and all 12 bank-name replies equal baseline. |
| Restore | Require the successful intermediate comparison and a fresh exact target recheck before the Restore confirmation. Success sends one more `61 04` using the preserved bytes and receives its own `0x71`. The reloaded target and all final backup programs, metadata pairs and bank-name replies equal baseline. Verify return and cleanup. |
| Refusals and faults | Use a missing or duplicate slot, corrupted manifest or checksum, invalid starting match, a valid upload that changes under stamping, and an unrelated edit byte. Each must cause zero stores at preflight. Inject missing or unexpected `0x71`, incomplete reload, and cleanup failure. Verify no retry after `0x61`, the additional two-minute wait on silence, and no second store after failed edit verification. Change an untouched program, metadata pair or bank-name reply independently and require the intermediate comparison to reject each. Inject wrong Restore confirmation and restore read-back mismatch and verify no further store. |

A rehearsal verifies the workflow on fake links. A fresh full-program
backup, verified one-setting edit for that capture, resolved spare 60
state, Gatekeeper review, and separate owner approvals and confirmations
remain pending for the real test.
