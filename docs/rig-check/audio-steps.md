# Rig Check audio and DAW steps 8.2–8.5 (design)

[#265](https://github.com/guirossibrum/VariaxWorkbench/issues/265) ·
**Not available in this version.** Opening an audio stream needs the owner's
separate approval (AGENTS.md, "Product scope"). Until then the catalogue
marks 8.2–8.5 "not available in this version", the runner never offers them,
`APPROVED_STEP_8_x` and `AUDIO_STREAMS_APPROVED` are `false`, and the code
path (`rig_check::audio::record_or_play`) returns before any stream is opened.
Step 8.1 (device and formats, no stream) is implemented (#264).

## Common rules

- Each step shows its card and waits for Run, like every other step.
- Streams use the existing recording core (`src/recording`, `CaptureSession`
  and `Take`, #98) through the PODxt Live's own audio device. No resampling;
  the PODxt clock is 78125/2 Hz (S24_3LE, 2 channels, `pod_audio`).
- Level warning first: "Turn your amplifier and headphones down" with a
  default of No.
- Clips go to the evidence folder only, never into the report. The report
  records numbers (levels, rates, xruns, latency) and file hashes.

## 8.2 Play a test tone

- **Does:** plays a 1 kHz sine at -24 dBFS for 2 s through the PODxt Live
  outputs, after 0.5 s of silence and a 50 ms fade in and out.
- **Asks:** "Did you hear a short beep?" Yes / No / Not sure.
- **Records:** device, format, actual rate reported by the stream, xruns,
  the answer.
- **Stops on:** stream open failure, any xrun count above zero is reported,
  not retried.

## 8.3 Record 10 seconds while you strum

- **Does:** records 10 s from the PODxt Live with a live level meter.
- **Records:** peak and RMS per channel (dBFS), samples at full scale
  (clipping), dropouts/xruns with their time, the frame count against the
  expected 10 s at 78125/2 Hz (actual rate), and the clip's SHA-256.
- **Hints:** no signal (check the PODxt input and the guitar volume), clipping
  (lower the PODxt channel volume).

## 8.4 Measure latency

- **Preferred:** loop the 8.2 tone back through the PODxt's monitoring path
  and find its onset in the recording (cross-correlation), reporting the
  round trip in ms and in frames.
- **Fallback:** "Tap along" — the person taps a key with a metronome click;
  reported as a rough estimate only, labelled as such.

## 8.5 Record each test model

- **Needs:** 4.3 (test models) and 8.3.
- **Does:** for each 4.3 test model: audition it in the working buffer (the
  4.3 procedure), ask the person to strum the open strings, record 5 s, then
  return to the starting preset (verified).
- **Records:** one clip per model in the evidence folder with its hash and
  levels. The comparison across operating systems is #266
  (`rig_check::compare`), which runs offline on these clips.

## Before the first run

1. Owner approval for audio streams, recorded in AGENTS.md.
2. `AUDIO_STREAMS_APPROVED` and each `APPROVED_STEP_8_x` set to `true` by the
   owner's decision.
3. An owner-run card like the other test cards, with the level warning.
