# Shared Variax procedures (#245)

Use `line6_devices::family::variax::procedures` from the editor (#204/#205)
and Rig Check. The owner tool uses the same exchanges. The procedures
support a Variax 300/600 firmware 3.10 reached through the PODxt Live USB
guitar interface. Their tests use simulated links. Difference messages
still await the [first owner run](variax-diff-listen-test-card.md).

## Open the session

Keep the rig lock from the verified attachment and identity through the
last read or return. Stop any Follow reader sharing the transport. Open
the correct bridge path and release the USB interface even when the
procedure returns early. Keep PODxt Live 3.01 identity verification
and the path-specific compile-time approval switches in the caller. Use the caller's
`run_if_unlocked` gates and a narrow `GatePolicy`; each request still
checks its operation. Before writes, establish the required verified backup
and separately approved restore procedure.

| Procedure | Operation and result |
| --- | --- |
| `identify` | `VariaxIdentity`. Verifies firmware 3.10. Resends once immediately for an orphan tail or after two seconds for total silence. |
| `read_settings` | Identity plus `VariaxRead`. Performs the checked two-block metadata read with the same identity retry. |
| `read_buffer` | `VariaxRead`. Reads 34 spans of 32 words and one of 4. Malformed replies can be retried twice with 100 ms between attempts. The observer can save each complete raw reply. |
| `read_unmodified` | `VariaxPresetSwitch` plus `VariaxRead`. Reloads the requested saved preset by program change and reads its complete buffer. The selection remains on that preset. |
| `return_to_preset` | Same gates as `read_unmodified`. Reloads the starting preset and compares all 1,092 words with its saved snapshot, even after cancellation of an earlier operation. |

The caller tracks the selected preset and owns policy for unsaved edits.
Take a full starting snapshot and establish that it matches the saved
program before offering verified return. `pack` converts a full read into
the 3,276-byte Workbench image. `compare_image` reports the first differing
word and names the settings field where possible.

## Edit the working buffer (#204)

Render and validate each full candidate with `write::Program::new`.
For an unknown edit-buffer baseline use `BufferEdit::Full`. For a known
baseline and an ordinary parameter change use `BufferEdit::Difference`.
It sends contiguous runs in `0x3B`, waits for the reply and omits the old
checksum word from the payload. A successful full comparison checks every
word, including that unchanged checksum. Carry forward the exact returned
buffer image. For the Workbench alternate-tuning transition use
`BufferEdit::AlternateTuning` with the prior image, the transition rendered with
`HostOptions { muted: true, ..Default::default() }` and the final program
rendered with mute off. It sends the difference first
and the final `0x43` after the difference acknowledgement. See the
[wire contract](variax-write-transactions.md).

Call `send_buffer_verified` for the upload and full read-back. Its
after-send callback supplies the observer for raw replies. Keep
`VariaxEditBufferWrite` and `VariaxRead` available. If the editor defers
read-back during a drag, `send_buffer` supplies a predicted image and the
reply window still finishes before another send. Compare a fresh full read
at the end of the gesture before marking it verified. Treat an uncertain
difference as a lost baseline and finish cleanup before reopening the link.
There is no difference or full-write retry.

## Store and Sync (#205)

Compile a checksum-valid full `Program`. The original selection and the
target are separate. Before a store preflight, confirm the target and
requested write or restore in the caller. `expected_stored_image` provides
the offline expected image, with words 767–770 set to `0310` and the
firmware checksum recomputed over words 0–1,090.

Use `store_and_reload` with `VariaxStoreRestore`, `VariaxPresetSwitch`
and `VariaxRead`. It sends `0x43`, waits 100 ms, attempts `0x61` once,
and finishes one 15-second confirmation wait. After `0x71`, it reloads the
target and compares the full image. For manual reselection, the lower-level
`store_verified` accepts a read-back callback which runs only after
acknowledgement. Use `return_to_preset` after safe outcomes if the starting
preset was elsewhere and its saved program still matches the snapshot.
A successful write to the starting slot replaces that snapshot.

| Result | Caller action |
| --- | --- |
| `Ok(())` | Report the verified target. Return and verify a different starting preset when needed. |
| `BeforeCommit` | The slot request was never attempted. A verified reload can discard a partial working-buffer upload. |
| `Verification` | Acknowledged with an unverified image. Stop the batch and report the verification error. |
| `SilentTimeout` | The procedure already notified the caller and paused for two minutes after normal session cleanup. Stop the batch and send no further commands. Ask the owner to check the guitar, take a Rig Check backup if it responds, and report. |
| `Unknown` | A slot request may have reached the guitar. Stop the batch and send no further commands. Report the unknown outcome. |

Before the slot request, cancellation is honored. After it, finish the wait
and verification regardless of cancellation. A missing or ambiguous reply
never triggers a resend or program change. For unknown state, include
`spare_warning(target)` in the result. For example, do not store to slots
40–59 until spare 62 is known blank.

## Compatibility read (#271)

`line6_devices::family::variax::firmware_read::read_current_program` reads
a Variax 300/600 identity, current settings and the complete current buffer.
It returns the reported identity and a `FirmwareStatus` of `Verified310`
or `Unverified`. This status describes the firmware identity. Data from
another version is diagnostic and does not establish compatibility.

The caller needs the rig lock and the identity and read gates. The separate
`APPROVED_FIRMWARE_READ` switch is closed. The procedure keeps the current
preset selected. Rig Check uses a separate approval for its evidence step
and practice replays in-memory replies through the same decoder. The
firmware 3.10 checks still apply to sends and stores.
