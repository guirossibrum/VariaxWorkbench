# Common offline capture session

`recording::capture` adds a shared, bounded software session around the
existing `recording::Take` storage, meter, dropout, cancel/stop and
non-overwriting WAV/AIFF save code. Capture-interface identity and provenance
are carried separately from the optional connected instrument and from the
POD signal-path selection used by existing take metadata.

## Admission and transport

`ValidatedPlan::native_podxt` reruns `Registry::plan` and then checks the
complete Linux PODxt Live contract: exact registry row/profile, normal capture
use, contract version, source declaration, two channels, packed S24_3LE and
the unreduced `78125/2` clock. An installed firmware version is not accepted
as proof. The resulting plan is still static source admission; session
initialization only creates the existing in-memory `Take`. No device or OS
audio API is opened.

`ValidatedPlan::software_file` admits an explicit software source at one of
the recorder's supported integer WAV rates. The callback transport applies the
selected left/right channel map and queues whole stereo frames. The 64 KiB
payload queue stores audio and dropout events in one chronological sequence;
repeated overflow gaps coalesce only when adjacent. A bounded event-count cap
also bounds metadata overhead. One owner thread drains events into `Take`.
Generation numbers reject late callbacks. Terminal storage/drain errors freeze
input; stop still closes the take and keeps the failure cause. If accepted audio
cannot be committed to `Take`, it remains available as ordered pending audio or
dropout events for explicit recovery after stop. Cancel discards only in-memory
work and leaves prior saved files intact.

The recording `Panel` now routes its selected WAV-file input through this same
`CaptureSession` and existing `Take` save path, while keeping the existing
unsaved-take and non-overwriting-save checks. The synthetic-input path retains
its existing direct `Take` workflow. This is offline in-app integration only;
it does not create or open an operating-system capture stream.

The session is a deterministic software contract, not a live real-time callback
or stream initializer. The inactive Linux ALSA adapter translates the admitted
PODxt negotiation tuple and already-obtained interleaved S24_3LE periods into
this session. It has no FFI, endpoint discovery, PCM handle, or production
initialization route. Its constants match the installed ALSA 1.2.16.1
`/usr/include/alsa/pcm.h` declaration: `SND_PCM_ACCESS_RW_INTERLEAVED` is 3,
`SND_PCM_FORMAT_S24_3LE` is 32, and `snd_pcm_hw_params_get_rate_numden` exposes
the rational clock pair. The inspected header SHA-256 is
`7b7bee209aca7a45b16c475bd950920b70b55734a6d80efedac1c98ed4a7e25e`. The
adapter rejects other access modes, formats, channel counts, and clocks.
Construction and tests consume supplied values; they do not query ALSA or a
device.

`SoftwareClockTracker` accepts explicit monotonic software timestamps and
cumulative frame positions. It computes expected elapsed nanoseconds using the
exact rational source rate, signed drift and parts-per-million, and cumulative
reported dropout frames with checked arithmetic. This measures only the
software observations supplied by an adapter. It does not measure device clock
drift, hardware timestamps, stream latency, or synchronization. Tests inject
observations and periods directly; they must not enumerate or open audio
devices. No resampling occurs and no fractional source clock is rounded.

## Unsupported routes and remaining work

External identifiers always return `ExternalContractMissing`; a name, USB ID,
or registry accounting row cannot create capture support. The existing PODxt
`RecordRoute` remains signal-path metadata and is not reinterpreted as an
external-interface type. Registry-qualified direct PODxt and Variax 600 through
PODxt plans can be validated offline; the Linux ALSA adapter consumes that
static declaration but does not open a native route. It does not establish a
physical capture backend or actual negotiation results.

Platform blockers are tied to the inspected repository evidence:

| Platform | Available source paths and facts | Exact missing contract and effect |
| --- | --- | --- |
| Windows | `src/linux_transport.rs` records a PODxt Live hardware ID without an interface/endpoint audio contract. `docs/pod-usb-audio.md` identifies Windows packet text as a different-source clue, not a PODxt Live negotiated format or callback contract. `src/pod_hd500x.rs` records HD500X INF defaults only. | No admitted PODxt Live WASAPI endpoint/format/rational clock/channel/callback ownership/lifetime contract. `audioclient.h`, `mmdeviceapi.h`, and the matching Windows SDK were not found in this environment. Without those inputs an inactive WASAPI adapter cannot be source-checked or cross-compiled; the available ID/default fields cannot safely construct its callback contract. |
| macOS | `src/linux_transport.rs` and `docs/pod-usb-audio.md` record a static macOS audio personality identity only; no audio endpoint mapping or stream contract is included. | No admitted PODxt Live `AudioStreamBasicDescription`, buffer-list, callback ownership, or lifetime contract. Core Audio headers (`AudioHardware.h`, `AudioUnit.h`, and AudioQueue headers) and the matching macOS SDK were not found in this environment. The identity-only personality cannot safely construct an inactive adapter or its compile checks. |

The task forbids acquiring new SDKs or source files, so those contracts cannot
be filled by guessing API structures or by using another product's defaults.
Independent external-interface records are also absent: `research/audio-inventory-schema2.tsv`, `src/rig/audio.rs`, and `docs/audio-capability-registry.md` contain no record binding an exact interface identity/revision to input channels and mapping, supported sample format, rational source clock, backend, and provenance. The PODxt declaration is a Linux static driver capability, not an external interface or a runtime-negotiated result. No physical capture or hardware clock timing is inferred. There is no global audio setting, privileged path, monitor/re-amp activation, or hardware override.

Validation is limited to deterministic injected buffers, software file input,
the frozen static registry, and the configured exact-head command-v1 checks.
Tests must not enumerate or open audio devices. Test results apply only to the
exact head named in the README/project receipt; they do not establish platform
adapter behavior, physical capture, drift, or safety.
