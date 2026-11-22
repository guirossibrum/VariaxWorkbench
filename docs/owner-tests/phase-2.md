# Owner test card — Phase 2, Stage 1: USB recognition (G1)

> **DRAFT — do not use yet.** This card becomes usable only when (1) the owner
> has signed the [G1 authorization form](../hardware-gates/g1.md#5-g1-authorization-form)
> on [#125](https://github.com/guirossibrum/VariaxWorkbench/issues/125), and
> (2) the [#158](https://github.com/guirossibrum/VariaxWorkbench/issues/158)
> build that opens only the G1 lock has merged. Today's app runs the Device
> check in simulation only and never touches the PODxt Live.

## Where #158 stands

| Part | State |
| --- | --- |
| Real stage-1 reader (`LinuxSysfs`): reads only the paths in [g1.md section 1](../hardware-gates/g1.md#1-what-the-app-reads), through the G1 gate | written; tested on temporary fake trees only (4 tests) |
| GUI builds without the USB library (`usb` feature off by default) | done; G1 needs no USB library |
| Kernel behaviour at plug-in | read from the Linux v7.2.15 source ([g1.md section 3b](../hardware-gates/g1.md#3b-kernel-startup-traffic-not-caused-by-the-app)); never observed |
| G1 lock change, and wiring the real reader into the Device check | **not done**: waits for your signature |

## You need

- PODxt Live with the Variax 600 attached by its VDI cable, powered on.
- The USB cable, and the #158 build (the commit you wrote on the form).

## Steps

1. Leave the USB cable unplugged. Start the app:
   `cargo run --release -- --display x11` (or the launcher Claude prepares).
2. Click **Device check…**. Check that every step says
   *real device: locked* except step 1. If not, stop and report.
3. Close the app. Plug the PODxt Live into USB. Wait 10 seconds. Note anything
   the PODxt Live shows now: this is Linux's own start-up, before the app runs.
4. Start the app again and click **Device check…**.
5. Read step 1. Click **Yes, run this step** for step 1 only.
6. When step 2 shows *waiting for your answer*, click **No, stop**.
7. Open **Report**, select all of its text and copy it. Close the app.

## Expected

- Step 1: *passed*, naming `0e41:4650`, its USB bus path, interface 0 with
  driver `snd-usb-pod` and interface 1 with `snd-usb-variax`, and the sound
  card list. The list should include **PODxt Live** (card driver `Line6-POD`)
  if the POD answered Linux's start-up request. A separate Variax card is
  expected to be **missing**: the stock Variax driver does not accept this
  guitar's identity (source reading, not yet observed).
- Step 2: *stopped* — "You chose No. Nothing was run."
- Nothing changes on the PODxt Live display, lights or sound when you click Yes.

## Report on #125

Paste the Report text, the build commit, one line on what the PODxt Live
showed at plug-in (step 3), and one line saying whether anything changed when
you clicked Yes. If you stopped early, say at which step and what you saw.

## Stop at once if

The app offers to run steps 2–4, asks for a password or permissions, lists
more than one PODxt Live, shows an error, or the PODxt Live changes in any way
when you click Yes. Unplugging the USB cable is always allowed.
