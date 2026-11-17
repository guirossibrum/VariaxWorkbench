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
the recorder's supported integer WAV rates. The callback transport accepts
interleaved S16 or the statically declared S24 byte shape, applies the selected
left/right channel map, and queues whole stereo frames. One owner thread drains
the bounded 64 KiB queue into `Take`; queue overflow rejects the whole chunk
and records an explicit missing-frame count. Generation numbers reject late
callbacks. Stop drains then closes the take; cancel clears queued and recorded
memory while preserving prior saved files; errors remain inspectable.

The session is a deterministic software contract, not a real-time callback
implementation or an OS backend. Its mutable single-owner API does not promise
thread-safe multi-producer access. Tests must inject byte buffers directly and
must not enumerate or open audio devices. No resampling occurs and no
fractional source clock is rounded.

## Unsupported routes and remaining work

External identifiers always return `ExternalContractMissing`; a name, USB ID,
or registry accounting row cannot create capture support. The existing PODxt
`RecordRoute` remains signal-path metadata and is not reinterpreted as an
external-interface type. Windows/macOS adapters, platform compile evidence,
actual UI selection of the common session, external interface records, and
physical external capture remain incomplete. There is no global audio setting,
privileged path, monitor/re-amp activation, or hardware override.

Validation scope for this implementation is the focused `recording::capture`
unit tests plus the repository's declared cargo checks when dependencies are
available. The local offline Cargo index currently lacks `eframe`, so crate
tests cannot resolve offline in this environment; this is a dependency
availability failure, not a passing test result.
