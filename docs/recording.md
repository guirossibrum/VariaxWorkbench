# Offline test recording

Issue #28 adds a recording workflow inside the same Rust app. It records
synthetic audio or a WAV file the caller already has. It does not record from
PODxt Live, the Variax, or the host sound server.

Opening the test-recording window does not start a take, select a route on a
device, or change monitoring or re-amping. Start, Feed, Stop, Cancel, and Save
are separate controls. Save runs only after Stop. Cancel discards the
in-memory take and writes nothing new. Copies saved earlier remain.

Device capture and playback stay locked. `StartAudioCapture` stays a physical
operation. `LIVE_BACKEND_INITIALIZER` stays `None`. No ALSA, PipeWire,
PulseAudio, USB, or MIDI port is opened, and no driver, reset, or update path
runs. The window says this is an offline test recording, distinct from a
recording made from a connected device.

## Input and route

The user picks both pieces before Start:

| Choice | What it does |
| --- | --- |
| Record route | One of the four GearBox Record Send 1-2 labels from `src/pod_audio.rs`. The label is stored with the take. It does not send `PODXT_ROUTING_ENABLE` and it does not change a live monitor or re-amp mix. |
| Synthetic generator | A deterministic 16-bit stereo pattern at an integer clock. |
| WAV file | A regular canonical 16-bit stereo PCM file whose header rate matches the selected integer clock. A pipe, socket, or device node is refused before it is opened, including when its reported length is zero. |
| Native source clock | Linux `pod_ratden` `78125/2`, packed as declared S24_3LE frames in memory. |

Guitar `SelectRecordRoute` stays unsupported. Workbench HD, JTV-69, and POD
HD500X stay unverified. The session gate checks the POD role, proves the
capture closure is not called, and keeps `stream_opened` false.

Processed, clean, and re-amp are the documented meanings of the selected
route. They are not a measurement of a signal that passed through a POD.

## Clocks and files

Integer clocks that may appear in a WAV header are 22050, 44100, 48000, 88200,
and 96000 Hz. 44100 and 48000 are the rates named in the GearBox digital-clock
section. The others are ordinary file clocks for offline tests. The header
stores that exact integer. A canonical file is PCM format 1, stereo, 16-bit,
little-endian, with a 16-byte `fmt` chunk.

The PODxt source clock remains the rational `78125/2`. `rate_min` 39062 and
`rate_max` 39063 stay the integer fields beside that knot. They are not open
parameters and they are not written as a WAV sample rate. 78125 is the
numerator, not a sample rate. Exporting `78125/2` as PCM WAV is rejected.
There is no resampler and no other container that has been verified to store
that rational exactly.

Software that reads canonical 16-bit stereo WAV can import an integer-rate
file this recorder writes. This repository has not opened a DAW. Nothing here
claims that a particular product accepts the file, and latency was not
measured. The GearBox guide says the recording program adds latency and gives
no number. Host monitoring and re-amping were not exercised.

Each save uses the existing non-overwriting file creation. It writes a new
`.wav` and a sibling `.wav.recording.json` sidecar. If either path already
exists, nothing is replaced. The sidecar records the route, the integer rate,
dropout gaps, integer peaks, the PCM and WAV SHA-256 values, and the explicit
statement that this is not a device recording. Native-clock takes are not
saved.

## Bounds

These limits stay separate:

| Limit | Value | Meaning |
| --- | --- | --- |
| Streaming chunk | 8192 bytes | Offline ceiling taken from the declared period-byte maximum. A chunk must be a whole number of frames. It is not a measurement that 44100 audio uses the device buffer. |
| Native chunk window | `plan_buffer(chunk, 1)` | Declared S24_3LE chunks also have to pass the Linux period window: a multiple of 6, at least 64, at most 8192. This does not loosen `plan_buffer`. |
| Take cap | caller value, at most 32 MiB | The whole take. The declared 60000-byte device period product is not this cap. A saved integer-rate take may be larger than 60000 bytes. |
| One dropout | 2048 frames | An explicit test gap. The recorder inserts that many zero frames and records the gap. It is not a measured USB dropout. A gap that would pass the take cap is rejected and inserts nothing. |

The panel's own take cap is 1 MiB. Feed copies 256 frames at a time, and only
when Feed is clicked.

## Tests

`src/recording.rs` has thirteen tests: truthful WAV save and refusal to overwrite,
rejection of `78125/2` without rounding, rejection of 39062, 39063, and 78125
as WAV rates, file round-trip, dropout zeros and sidecar integrity, cancel
before any save, cancel after a save keeping that copy, separate chunk and
take bounds, integer peaks including `i16::MIN`, malformed WAV rejection, an
idle panel until Start, synthetic renders one frame past the chunk ceiling and
an overflowing frame index, and a zero-length non-regular path whose reader is
not called.

`offline_recording_gate_does_not_call_capture_or_open_a_stream` checks the
session gate. Two headless GUI tests cover the window. One checks the offline
wording and runs Start, Feed, Stop, and Save without starting a worker. The
other asks to load a directory path and checks that the message refuses it
before any device open.

Passing these tests does not authorize a later connection.

## Limits

- No USB, MIDI, or audio device was enumerated, opened, or streamed.
- No driver was installed, loaded, bound, or probed.
- No original vendor audio or capture is in the tree.
- Latency, host routing, and actual DAW import were not tested.
- The four route names are GearBox guide text.
- The native frame and period window are the Linux `v7.2.15` declarations
  already recorded in [POD USB audio](pod-usb-audio.md).


## Unsaved takes and quitting

Starting a test take marks that take unsaved, including an armed take with no
frames yet. A complete successful WAV and sidecar save marks that take saved.
A cancelled picker or failed save retains its audio and unsaved status; a refused
additional copy does not invalidate an earlier successful save of unchanged data.
Starting a later take resets the status. Saved files survive in-memory discard.

Quitting now checks guitar edits, applied POD tone edits and the current unsaved
test take. Guitar/POD save or discard continuations cannot skip the recording
prompt. The recording prompt offers Stop and save copy, Keep editing, or Discard
this take. Stop occurs only after that explicit save action; cancelling the file
picker leaves a stopped, unsaved take. Background recording/POD editors are
inactive while an unsaved-changes prompt is pending. Closing the recorder window
still retains its take; quitting the application is guarded separately.

The native 78125/2 source-clock take cannot currently be exported as WAV. Its
quit prompt explains the missing exact-clock export and disables Save; it requires
Keep editing or an explicit Discard. There is no rounding, resampling, automatic
save, stream, device operation or implicit monitoring change. Exact-clock export
remains #69. These protections cover normal application close paths, including
screenshot completion; a process crash or forced termination can still lose
in-memory data. Save a file copy to persist it.
