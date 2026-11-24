# Automatic identity first-run card

Owner-run check for Step 3c ([#213] and [#201]). The offline suite checks the
control flow with fakes. This card records what happens on the physical rig.

## Setup

- Use Linux, one PODxt Live with firmware 3.01, and the Variax 600 with firmware
  3.10. Start with the guitar disconnected from the PODxt.
- From the review worktree, record `git rev-parse HEAD` and build the app with
  `cargo build --offline --locked --bin variax-workbench-gui`. Close other
  editors, old app builds and older copies of the owner tools before starting.
- This run covers identity only. Leave Device check live actions, Test
  recording, and the store and edit procedures out of this run.

## Check the tabs and driver return

1. With the PODxt disconnected, launch `target/debug/variax-workbench-gui`.
   The start panel should offer **Open file**, **New offline tab**, and
   **Detect devices**. There should be no connected tabs.
2. Connect the PODxt to USB and select the **PODxt Live** tab. Expect
   **Guitar: checking…** followed by **Guitar: no guitar answered** if the
   interface is available and the guitar is silent. If Linux lacks the
   expected interface binding, expect the corresponding status. A Variax
   tab should never appear in this step. Confirm that the PODxt reports
   firmware 3.01 when its MIDI identity succeeds.
3. Connect the Variax to the PODxt using the usual connection procedure.
   Leave the USB connection in place. Watch the PODxt tab for an automatic
   **Guitar: checking…** transition. Expect a **Variax Workbench** device
   tab with firmware 3.10 only after identity succeeds. If no automatic
   transition occurs, record that result and use **Refresh guitar**.
   Check that the expected interface-1 driver is bound again using Device
   check step 1 or the operating system's USB status.
4. Disconnect the guitar with the USB link still in place. Watch for the
   automatic recheck. A silent identity check should take the Variax tab
   offline and leave its work available. The PODxt should show the last
   verified guitar separately from the current result. If no automatic
   transition occurs, record that result and use **Refresh guitar**.
5. Reconnect the guitar, then unplug and reconnect the PODxt USB link.
   Expect one automatic identity attempt for the new USB attachment. As
   each identity succeeds, its existing PODxt Live or Variax tab should
   reconnect in place, keeping its document and undo history. If the tab
   has unsaved edits, expect **Reconnected. Your unsaved edits are kept;
   nothing was sent.** The reattachment sends no editor data.
6. In a separate run, quit while **Guitar: checking…** is visible. Confirm
   the app waits for cleanup if needed, then exits. Check that the expected
   interface-1 driver is bound again.

If the PODxt identity succeeds but the guitar interface is unavailable,
record the status and the bound drivers. If cleanup fails, record the
message and stop the run there.

## Record passive notice and firmware evidence

- Workbench 1.75 code checks an 11-byte `03 11` frame with zero-based
  byte 9 as the state (zero means disconnected). It treats a 12-byte
  `7F 03` frame as disconnected. These frames are code-derived and have
  yet to be observed on the PODxt Live. The app listens on the PODxt Live
  MIDI input while the rig is idle and releases it for active operations.
  A notice can be missed while the listener is paused, or if the device
  never forwards it. If you already have a capture, record which frames
  were seen and which were followed by the check. Unknown or malformed
  notices appear in the terminal log. The app does not enable forwarding
  or send the editor model when it sees a notice.
- The offline fixture verifies that a Variax 300/600 identity below 3.07
  shows **Update Firmware** on the PODxt tab and stays out of the live
  Variax tab. With the approved 3.10 rig, expect the ordinary verified
  status. Leave installed firmware as it is for this check.

## Check lock contention

With the app stopped and no device operation in progress, hold the same
advisory lock for 15 seconds from a terminal:

```sh
flock -n "${XDG_RUNTIME_DIR:-/tmp}/variax-workbench-podxt-live.lock" sleep 15
```

While that command holds the lock, launch the app with the rig connected.
Expect **Guitar: checking… (waiting for the rig)** when the PODxt is visible,
with no Variax device tab. When the command exits, the queued check should
run once and show the guitar only after verification. This uses an idle lock
holder. The owner probes can be exercised separately under their own
approved procedures and confirmations using binaries rebuilt at the review
commit.

Record the commit, device versions, visible status transitions, driver return,
and any unexpected behavior. If an owner capture is available, check for one
PODxt identity request and one guitar identity request per applicable action.
Each changed notice can trigger one check, with duplicate notices coalesced.
A lone closing byte from a lost guitar packet can trigger one guitar resend
on the same connection. The operating system's driver may send its own
identity requests after reattachment.

[#213]: https://github.com/guirossibrum/VariaxWorkbench/issues/213
[#201]: https://github.com/guirossibrum/VariaxWorkbench/issues/201
