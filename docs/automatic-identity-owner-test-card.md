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
   Leave the USB connection in place. Confirm that the previous guitar
   status stays in place until you select **Refresh guitar**. A valid
   reply should open one **Variax Workbench** device tab and show firmware
   3.10. Check that the expected interface-1 driver is bound again using
   Device check step 1 or the operating system's USB status.
4. Disconnect the guitar using the usual procedure and choose **Refresh
   guitar** from the PODxt tab. The Variax tab should go offline and stay
   available for editing. The PODxt should show the last verified guitar
   separately from the current result.
5. Reconnect the guitar, then unplug and reconnect the PODxt USB link.
   Expect one automatic identity attempt for the new USB attachment and
   one fresh Variax device tab after a valid reply. Previously disconnected
   tabs should keep their own work.
6. In a separate run, quit while **Guitar: checking…** is visible. Confirm
   the app waits for cleanup if needed, then exits. Check that the expected
   interface-1 driver is bound again.

If the PODxt identity succeeds but the guitar interface is unavailable,
record the status and the bound drivers. If cleanup fails, record the
message and stop the run there.

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
A lone closing byte from a lost guitar packet can trigger one guitar resend
on the same connection. The operating system's driver may send its own
identity requests after reattachment.

[#213]: https://github.com/guirossibrum/VariaxWorkbench/issues/213
[#201]: https://github.com/guirossibrum/VariaxWorkbench/issues/201
