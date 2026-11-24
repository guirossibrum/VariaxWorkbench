# PODxt Live Tone Locker and Sync owner test card

**Not approved.** Sync, setup stores and channel switching each await a
separate owner decision. The #222 whole-tone and store test stages remain
unapproved. Approving the #222 tone store alone leaves the separate setup
and Sync locks closed. This card is a proposed procedure for a later owner run.

## Offline rehearsal

1. In the PODxt tab, use **New tone**, **New bundle**, and **New collection**.
   Save each as a new file. The new tone uses the supported PODxt Live fields
   of Line 6 Edit's starter tone, with Variax linking explicitly off.
2. Open a copied PODxt library, export its HTML tone list, and check escaping
   with a tone name containing an ampersand. Keep all original files.
3. A backup bundle uses the Line 6 Edit `L6BA` structure with `HEAD`,
   fixed `BANK` lists and 128 `L6PA`, 107 `L6AS`, and 64 `L6FS`
   entries in device order. The original Java file reader loaded all 299
   generated entries. Workbench generates its own optional metadata and
   does not reproduce Java's generated timestamps. Use the verified raw
   Device Check backup for store authorization and byte comparisons.

## First owner Sync run after approval

1. Close other software using the rig. Wait for automatic PODxt Live 3.01
   identity. Keep the Variax link off for the store trials. Make two
   complete Device Check backups on the same UTC day, verify both
   manifests and compare all 299 stored entries.
2. In **Tone Locker**, choose **Get bundle**. Check 128 channels, 107 amp
   setups and 64 FX setups. Load one setup of each kind into the file
   editor and save copies. Compare the model names and settings with the
   source. Choose **Save backup bundle** and open the resulting file in
   Line 6 Edit offline.
3. Copy a device tone into the file browser, drag a compatible file to a
   device slot, and use **Stage rename**. Confirm that staging leaves the
   device unchanged. Try **Revert selected**. Request to close the tab
   and choose **Keep working** at the staged-changes warning.
4. For the first setup store, use **Store original to slot** with the chosen
   setup bank and its original slot. Choose the same-day backup and type
   the exact prompt, for example **STORE PODXT AMP 1** or **STORE PODXT FX
   1**. Confirm the acknowledged readback matches all bytes and compare a
   new complete Device Check backup with the baseline.
5. On a separately approved Sync trial, stage a small rename on a chosen
   slot. Choose **Write selected slot**, select the verified backup, then
   **Prepare next slot**. Verify the old and new names and type the slot
   confirmation. After the write, require a verified readback and compare
   all 299 slots to see that only the chosen slot changed. **Sync modified
   slots** queues items for the same per-slot confirmation. Each next
   slot must be prepared and confirmed separately.
6. A negative acknowledgement, transport failure after dispatch, silence
   or failed readback leaves the slot state unknown. Stop the trial,
   retain the staged edits and record the error. The app makes no
   automatic retry. Any restore or new attempt needs its own decision.

The channel-change review shows the linked model in the destination and
warns about a different recall. Owner evidence covers standalone recall
with USB disconnected. The Tone Locker's channel control stays at review
only until an editor channel-switch gate is available and approved.

## Compatibility check

From this worktree, using the local ignored Line 6 Edit extraction and JDK:

```sh
artifact_root=../..
jdk="$artifact_root/.tools/jdk/bin"
classes="$artifact_root/analysis/extracted/rea-full/line6-edit-3.06.0-win/embedded-zip"
output="$PWD/analysis/issue-223/oracle"
mkdir -p "$output"
"$jdk/javac" -cp "$classes" -d "$output" scripts/VerifyPodxtLocker.java
"$jdk/java" -cp "$output:$classes" VerifyPodxtLocker "$output/java.l6b"
PODXT_LOCKER_ORACLE_INPUT="$output/java.l6b" PODXT_LOCKER_ORACLE_OUTPUT="$output/rust.l6b" \
  cargo test --locked --offline --all-features --lib parameters::wire
"$jdk/java" -cp "$output:$classes" VerifyPodxtLocker check "$output/rust.l6b"
```

Use a new output path for repeated exports. Both the Rust harness and the
Java check operate on synthetic tones, with device constructors bypassed.
Read and write protocol limits remain as described in the
[#222 test card](podxt-store-test-card.md).
