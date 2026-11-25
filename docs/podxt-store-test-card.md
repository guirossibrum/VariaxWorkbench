# PODxt Live whole-tone and store owner test card

**Not approved.** Both #222 hardware stages await separate owner decisions.
The offline build keeps their hardware entry points closed. The Line 6 Edit
transcript establishes host behaviour. The PODxt Live 3.01 host-facing
controller and its flash routing remain unlocated in #211.

## A. Resend the playing tone

After a separate go for this stage, use the verified PODxt Live 3.01 with the
Variax link off and other tools closed. Keep the pedals, knobs and footswitches
still. Note the playing channel and tone name. Use a complete Device check
PODxt Live library backup with a verified manifest covering all 299 stored
entries. In the verified PODxt tab, choose **Get edit buffer**, then
**Verify backup for whole-tone resend…** and select the backup. Choose
**Resend whole current tone** once. Before sending, the app reads the saved
slot for the playing channel and requires all 160 tone bytes to match that
slot's backup entry. If the read fails or any byte differs, stop and record
the refusal. Verify the edit-buffer readback and check that the playing
channel, settings and audible tone are unchanged. Record any difference.
Stop here for the separate store decision.

## B. Write one slot's own bytes back to that slot

After a separate go for this stage:

1. Use a complete Device check PODxt Live library backup from the same rig.
   Verify the manifest covering all 299 stored entries. Record the folder
   and the chosen saved tone slot. Both stages accept a backup regardless of
   its date when the addressed live slot matches it byte for byte.
2. In the verified PODxt tab, select the destination slot and choose
   **Verify library backup**. Check the displayed slot and tone name. The
   first-run store takes the candidate from that slot's own backup file.
3. Type the displayed confirmation exactly, for example **STORE PODXT 17A**.
   Choose **Store original to slot** once. The app reads the slot and
   requires all 160 tone bytes to match its backup entry before sending a
   store message. If the read fails or any byte differs, stop and record the
   refusal. Cancellation ends when the first store message starts. Leave the
   rig connected while the transaction waits.
4. Record the acknowledgement and slot readback. A negative or missing
   acknowledgement, a transport error after dispatch, or a different
   readback leaves the slot state unknown. Stop the procedure and report it.
5. After an acknowledged and verified readback, run Device check's full
   PODxt library backup again and compare all 299 entries with the original
   backup. Report the firmware, chosen slot, exact slot readback, comparison
   counts, and any audible or front-panel change.

## Host messages and the #211 trap list

All frames here are host-side MIDI. The [#211 trap list](firmware-podxt-dispatch.md#trap-list)
describes internal SHARC indices. Host-to-SHARC translation remains unknown.

| Host traffic | Evidence and boundary |
| --- | --- |
| Universal identity `F0 7E 7F 06 01 F7` | Existing verified identity request. The trap list records `0x7E` leaving the loaded program if delivered directly to the SHARC. The host port must identify as PODxt Live 3.01 before the transfer. |
| Global channel `03 57`, pack query `03 0E` | Existing #220/#221 reads used for whole-tone send preflight. The command table and trap list describe why the host bytes do not identify SHARC handlers. Only the existing typed requests are used. |
| Edit-buffer read `03 75` and reply `03 74` | Existing host read used before and after send. The #211 command table records the candidate table words and unknown translation. |
| Whole-tone send `03 74 0A` plus 160 bytes | Original `SendDeviceEditBuffer` and `EncodePatch`. The trap list records an internal handler that reads 160 words without testing EOX. The write accepts only a complete seven-bit PODxt Live payload. |
| Stored-slot read `03 73` | Existing address-based read before either write and after a store OK. Tone slots resolve through the bounded `Location` type and the two count bytes are zero. The trap list records the internal 409 sentinel and out-of-range indices. |
| Store `03 71 0A <address>` plus 160 bytes | Original `SendDevicePatch` and `EncodePatch`. The trap list's internal `0x71` path programs words without a `0x50` reply. Host-to-SHARC translation and flash safety remain unverified. |
| Block end `03 72` | Original `EndSendPatchBlock`. Internal `0x72` is listed with half-copy and erase code. Its relation to the host request is unknown. The host command is sent once, after the complete addressed frame. |
| OK `03 50` and failure `03 51` | Reply bytes consumed by the original `EndSendPatchBlock`. This editor only receives them. The editor waits at most seven seconds, even if unrelated messages arrive. |

## Reproduce the offline transcript

From the issue worktree, point these paths at the ignored local extraction
and JDK 17. The harness bypasses device constructors. The original patch
manager and MIDI manager reach only a fake native sender and scripted
replies. The resulting fixture contains a synthetic tone.

```sh
artifact_root=../..
jdk="$artifact_root/.tools/jdk/bin"
classes="$artifact_root/analysis/extracted/rea-full/line6-edit-3.06.0-win/embedded-zip"
output=analysis/issue-222/oracle
mkdir -p "$output"
"$jdk/javac" -cp "$classes" -d "$output" scripts/VerifyPodxtTransfer.java
"$jdk/java" -cp "$output:$classes" VerifyPodxtTransfer > "$output/transcript.txt"
cmp "$output/transcript.txt" crates/line6-devices/src/family/podxt/fixtures/line6-edit-3.06-transfer.txt
```

Line 6 Edit starts another 7,000 ms wait after dispatching an unrelated
packet. The Rust version keeps one deadline. A successful host
acknowledgement is followed by an addressed readback. There is no automatic
retry, fallback write, program change, or firmware operation.
