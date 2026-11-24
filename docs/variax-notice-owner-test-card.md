# PODxt Live VDI notice capture

**Not approved.** The reviewer explains this card to the owner before any
run. Only the owner runs it on hardware.

[#201](https://github.com/guirossibrum/VariaxWorkbench/issues/201) asks
whether a PODxt Live 3.01 forwards connect and disconnect notices from a
Variax 600 3.10 while Variax messages stay enabled. Workbench 1.75 opens the
bridge with `03 23 01` and `2A 01` and keeps it enabled until close. Its
bridge-mode poll always succeeds, and its event handler recognizes 11-byte
`03 11` and 12-byte `7F 03` frames. Delivery on this rig is unverified.
The probe uses the requested 1,000 ms settle between those enable
commands. The one-second delay is documented in Monkey's firmware writer.
[Workbench and Monkey evidence](podxt-variax-control-check.md)

## Procedure for the owner after review

1. Close Workbench and other rig tools. Start with the PODxt Live connected
   to Linux over USB and the Variax connected over VDI.
2. From the reviewed checkout, run the command below. The probe holds the
   rig lock through capture and cleanup. It checks the PODxt Live 3.01 and
   Variax 300/600 3.10 identities, with one guitar resend only after a
   lost first packet. If the guitar is already unplugged, silence is
   logged and the capture continues.
3. When **Listening for 60 s** appears, start a stopwatch. Unplug the
   guitar's VDI cable around 15 s, then plug it back in around 35 s. Note
   both actual times. Leave PODxt USB connected for the whole run.
4. Let the probe finish. Ctrl+C ends listening and attempts the same
   cleanup as normal completion. Once enable has been attempted, the
   probe sends `2A 00` then `03 23 00` and returns USB interface 1 to
   the kernel. Record any cleanup error. The enabled bridge can
   temporarily change the last PODxt EDIT page to computer control.

```sh
cargo run --offline --manifest-path tools/variax-probe/Cargo.toml -- \
  podxt variax-notices --out analysis/captures
```

Supply a duration after `variax-notices` to choose 1–120 seconds. The
capture logs both input sources as timestamped hex, including MIDI
realtime bytes. The elapsed time records when a chunk was read. Match the
owner's stopwatch times to the timestamp on the **Listening** line. The
probe saves a new capture directory under `--out` and labels interrupted
or failed runs `-INCOMPLETE`.

## Record for review

- The two cable-change times and whether the full window completed.
- The timestamps, input source, and decoded state of every marked
  `03 11` and `7F 03` frame. For `03 11`, the log shows the raw state
  at zero-based byte 9, with zero decoded as disconnected.
- Any `28`, `4D`, or `74` Variax notices and their input source and times.
- Whether interface cleanup succeeded. Keep the raw log local.

A completed window with no matching frames is a negative observation for
this rig and this sequence. Leave the question open if the run stopped
early or the cable-change times could not be matched to the log. The
probe only sends identities and the two on/off pairs. It sends no preset
reads, edits, program changes, or stores.
