# Owner test card — Phase 2, Stage 1: USB recognition (G1)

Issue [#158](https://github.com/guirossibrum/VariaxWorkbench/issues/158); results
go to [#125](https://github.com/guirossibrum/VariaxWorkbench/issues/125).

> **Use this card only with the build from `master` after #158 has merged.**
> Earlier builds keep G1 locked and offer Simulation only.

G1 is open in that build by your direct permission of 2026-11-22
([G1 ledger, section 5](../hardware-gates/g1.md#5-g1-authorization-record)).
Only step 1 can run on real hardware. It reads what Linux already knows about
connected USB devices and sound cards and sends nothing to the PODxt Live or
the guitar. Steps 2–4 (gates G2–G3) stay locked.

## You need

- PODxt Live with the Variax 600 attached by its VDI cable, powered on.
- The USB cable.
- This checkout at the `master` commit that contains #158.

## Steps

1. Leave the USB cable unplugged. In the project folder, update to `master`
   and start the app:

   ```sh
   cargo run --release -- --display x11
   ```

   (or the launcher Claude prepares for you).
2. Click **Device check…**. Check:
   - the orange banner says *Only gate G1 (USB recognition) is open in this
     build*;
   - **Mode** shows **Simulation** selected;
   - step 1 says *real device: open* and steps 2–4 say *real device: locked*.

   If any of these differ, stop and report.
3. Close the app. Plug the PODxt Live into USB. Wait 10 seconds. Note anything
   the PODxt Live shows now: this is Linux's own start-up, before the app runs
   ([g1.md section 3b](../hardware-gates/g1.md#3b-kernel-startup-traffic-not-caused-by-the-app)).
4. Start the app again and click **Device check…**.
5. Under **Mode**, choose **This computer (G1)**. Steps 2–4 must now say
   *locked in this build* and step 1 *waiting for your answer*. Nothing has
   been read yet.
6. Read step 1's *What it does* and *Risk*. Click
   **Yes, read this computer's USB list**. (Pressing Enter instead answers No.)
7. Step 1 finishes. The window says *Step 1 finished. Steps 2–4 stay locked in
   this build*, and no Yes/No buttons are shown any more.
8. Click **Copy report** (or open **Report** and copy its text). Close the app.

## Expected display

- Step 1 *passed*, with a line like
  `PODxt Live 0e41:4650 found at USB 1-2; interface 0 driver snd-usb-pod,
  interface 1 driver snd-usb-variax; sound cards: 0 "PODxt Live"`.
  The bus path (`1-2`) and card numbers depend on your computer.
- The sound-card list should include **PODxt Live** if the POD answered
  Linux's start-up request. A separate Variax card is expected to be
  **missing**: the stock Variax driver does not accept this guitar's identity
  (a source reading, not yet observed).
- If interface 1 shows `driver none` or the PODxt Live is not found, that is a
  result to report, not a failure of the test.
- Nothing changes on the PODxt Live display, lights or sound when you click Yes.

## Paste into #125

1. The Report text. It starts with
   `device-check schema=1 mode=this-computer gates-open=G1 writes=none` and
   lists, for step 1: gate G1, your Yes/No, passed/stopped, the USB
   vendor/product, the bus path, interface numbers and driver names, and the
   sound-card list. It contains no serial number, file path or preset name.
2. The `master` commit you ran (`git rev-parse --short HEAD`).
3. One line on what the PODxt Live showed at plug-in (step 3).
4. One line saying whether anything changed when you clicked Yes.

If you stopped early, say at which step and what you saw.

## Stop at once if

- the app offers to run step 2, 3 or 4 in **This computer (G1)** mode;
- it asks for a password or permissions;
- it lists more than one PODxt Live;
- it shows an error;
- the PODxt Live changes in any way when you click Yes.

Unplugging the USB cable is always allowed.
