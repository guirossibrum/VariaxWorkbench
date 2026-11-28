# Linux paired Follow input and handoff

**Pending Gatekeeper review and direct owner authorization.** Only the owner
runs the hardware portion. This is the isolated
[#275](https://github.com/guirossibrum/VariaxWorkbench/issues/275#issuecomment-7231520627)
probe for the two existing Linux input routes.

## Prepare

- Use the supported PODxt Live 3.01 and Variax 600 3.10. Confirm both identities
  with the existing approved Device Check first. Record the USB port path
  such as `8-2.1` and its current USB device address in decimal from this same
  connection. For the address, `lsusb -d 0e41:4650` lists the decimal
  `Device` number. Use the port path from the verified connection. Recheck
  both values if the rig re-enumerates.
- Keep USB and VDI connected throughout. Note the starting PODxt channel
  and guitar model/selector position. Close Workbench and other rig tools
  after Device Check. The probe's own lock-only thread supplies foreground
  contention.
- From the reviewed checkout, build the standalone binary. This step
  compiles it without running a device operation.

```sh
cargo build --offline --locked --manifest-path tools/variax-probe/Cargo.toml \
  --bin passive-ingress-probe --target-dir target/passive-ingress-probe
```

## Owner run after authorization

Use the observed USB port and decimal address in place of the examples.

```sh
target/passive-ingress-probe/debug/passive-ingress-probe observe 8-2.1 10
```

1. Wait for `OVERLAP` before touching either device. The probe verifies the
   read-only ALSA descriptor and Variax interface descriptors. It rechecks
   attachment identity after opening the USB handle, claims only interface 1,
   verifies the guitar identity, and uses the approved Follow
   session and relay operations. Interface 1 is temporarily detached and the
   PODxt EDIT display may briefly indicate computer control. No MIDI is
   sent on the PODxt ALSA port.
2. Within the 30-second window, physically select one different PODxt
   channel. After that settles, move the Variax selector to a known model.
   Wait at least one second, then move the guitar volume and tone knobs.
   Confirm a fresh `PODxt ALSA channel program` line and matching
   `Variax USB selector`, `Volume`, and `TonePosition` lines. Note which
   actions produced which observations. A linked model change when the
   PODxt channel changes does not replace the separate selector action.
3. Press **Ctrl+C once** while both inputs are open and before the window
   expires. Wait for cleanup. The output should show the lock-only waiter
   requesting the rig, successful session and relay off, verified USB
   interface release and driver return, successful checked ALSA close,
   then the waiter acquiring and the final `PASS` line.
4. Compare the four input observations to your physical actions and record
   their source and time, along with the ordered cleanup result. A timeout,
   unmatched or missing event, attachment change, busy refusal or cleanup
   failure leaves the run incomplete. After a cleanup failure, stop and
   report the message before reopening the rig. After a clean run, restore
   the original channel and guitar selection by hand if desired.

The probe sends the existing Variax identity and Follow on/off operations
only. It sends no preset selection, edit, store, output on ALSA or foreground
device request. The lock-only waiter checks this process's handoff. Share
the summarized events and cleanup result. Keep any full terminal log local.
