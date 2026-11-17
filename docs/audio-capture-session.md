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

The session is a deterministic software contract, not a real-time callback
implementation or an OS backend. Its mutable single-owner API does not promise
thread-safe multi-producer access. Tests must inject byte buffers directly and
must not enumerate or open audio devices. No resampling occurs and no
fractional source clock is rounded.

## Unsupported routes and remaining work

External identifiers always return `ExternalContractMissing`; a name, USB ID,
or registry accounting row cannot create capture support. The existing PODxt
`RecordRoute` remains signal-path metadata and is not reinterpreted as an
external-interface type. Registry-qualified direct PODxt and Variax 600 through
PODxt plans can be validated offline, but there is no native OS/backend adapter
and no native route is opened. Windows/macOS adapters, cross-platform compile
evidence, independently admitted external-interface records, negotiated OS
formats/rates, and physical external capture remain unavailable. The software
queue has no hardware timestamps, so it cannot measure or correct clock drift;
no resampling occurs, and source rational clocks are never rounded. There is no
global audio setting, privileged path, monitor/re-amp activation, or hardware
override.

Validation is limited to deterministic injected buffers, software file input,
the frozen static registry, and the configured exact-head command-v1 checks.
Tests must not enumerate or open audio devices. Test results apply only to the
exact head named in the README/project receipt; they do not establish platform
adapter behavior, physical capture, drift, or safety.
