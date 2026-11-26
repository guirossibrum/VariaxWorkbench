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

The recording `Panel` routes selected WAV-file and synthetic input through this
same `CaptureSession` and existing `Take` save path, while keeping the existing
unsaved-take and non-overwriting-save checks. Integer-clock synthetic input is
admitted as an explicitly generated S16 software source. The selected exact
PODxt rational clock has a separate software-synthetic S24 plan with no device
identity, declaration, or capture row; it does not initialize ALSA or claim
that the bytes came from a connected PODxt. Separately, the Panel can prepare
and revalidate
the source-supported Linux PODxt Live declaration from the selected Record Send
route, displaying its exact `78125/2` clock, S24_3LE format, channel map,
provenance contract, and take cap. This only inspects a static plan: it creates
no take and cannot start a stream. The external-interface check reports the
missing independently admitted identity/revision, route/map, format, rational
clock, OS backend, and provenance without fabricating an interface record.
Synthetic-input recording remains identified as synthetic and does not pretend
to be externally captured audio. This is offline in-app integration only; it
does not create or open an operating-system capture stream.

The session is a deterministic software contract, not a live real-time callback
or stream initializer. The inactive Linux ALSA translator now requires a
`ValidatedPlan` token created from the frozen registry; it revalidates source
identity, route kind, declaration, provenance, format, channel map, and exact
rational clock together with the supplied PCM negotiation tuple. A same-shaped
tuple cannot promote a software or unknown source to PODxt. This remains a
static source admission, not a report of physical runtime negotiation.

The adapter's pure `translate_read_result` boundary models the installed ALSA
`snd_pcm_readi` contract: a signed frame count, requested frame capacity, and a
borrowed interleaved buffer. It checks the buffer's byte extent from the admitted
two-channel three-byte sample format and only copies the returned frame prefix
into the bounded common queue. A zero-frame result remains a no-op only after
the common session validates owner, generation, and lifecycle; it adds no PCM
and changes no meter, clock, or queue state. The borrow ends before the callback
returns. A short read from a valid period can commit whole frames below the
minimum configured period size. The translator records `-EPIPE` as a terminal
discontinuity, `-ESTRPIPE` as a terminal suspension, and other negative results
as terminal adapter failures. ALSA read results give no exact lost-frame count.
The translator performs no FFI and retains no native buffer or PCM handle.

Native numeric values match the installed ALSA 1.2.16.1
`/usr/include/alsa/pcm.h`: `SND_PCM_ACCESS_RW_INTERLEAVED` is 3,
`SND_PCM_FORMAT_S24_3LE` is 32, `snd_pcm_sframes_t` is `long`,
`snd_pcm_readi` returns that signed frame type, and
`snd_pcm_hw_params_get_rate_numden` exposes the rational clock pair. The
`-EPIPE` and `-ESTRPIPE` results are translated using the local Linux errno
headers (`/usr/include/asm-generic/errno-base.h` defines `EPIPE=32`;
`/usr/include/asm-generic/errno.h` defines `ESTRPIPE=86`). The inspected ALSA
header SHA-256 is
`7b7bee209aca7a45b16c475bd950920b70b55734a6d80efedac1c98ed4a7e25e`. The
adapter rejects other access modes, formats, channel counts, clocks, impossible
frame counts, and buffers outside the bounded period size. Construction and
tests consume supplied values; they do not query ALSA or a device.

`SoftwareClockTracker` accepts explicit monotonic software timestamps and
cumulative frame positions. It computes expected elapsed nanoseconds using the
exact rational source rate, signed drift and parts-per-million, and cumulative
reported dropout frames with checked arithmetic. This measures only the
software observations supplied by an adapter. It does not measure device clock
drift, hardware timestamps, stream latency, or synchronization. Tests inject
observations and periods directly; they must not enumerate or open audio
devices. No resampling occurs and no fractional source clock is rounded.

## Inactive Linux ALSA stream lifecycle (#146)

The initial source is Variax 600 → PODxt Live Record Send 1–2 → stock
`snd-usb-pod`. The stream adapter takes an injected, configured but unstarted
PCM. The platform must supply independent evidence for that same attachment
and raw hardware PCM, the guitar and selected send, and channel order. A
display name, static plan, or matching sample tuple cannot establish the
route. The adapter admits the already-selected processed or clean instrument
send and uses the handle for capture.

On that same PCM the platform must report the current negotiated
RW_INTERLEAVED, packed S24_3LE, two-channel format, and exact `78125/2`
rate from the rational getter. The adapter also requires whole-frame period
and buffer geometry within the existing source-declared window, with the
buffer composed of the reported number of equal periods. These bounds come
from source. Runtime negotiation remains unverified. The adapter starts the
PCM only after all checks pass. The same owner
thread reads a bounded period and feeds the existing translator and common
session. Stop, cancel, disconnect and terminal read or storage failure
release the PCM once. The common take preserves accepted audio and any
pending recovery events. An overrun or suspend ends capture without
recovery, retries or a guessed lost-frame count. Cleanup errors are
retained separately from the reason capture stopped.

The optional Linux adapter connects this lifecycle to libasound 1.2.16.1's
PCM ABI. It takes ownership of a prepared PCM, checks the independently
supplied evidence against the same handle's raw capture address and state,
and queries its current hardware parameters. It reads at most one period per
call in nonblocking mode. Native calls are covered by injected fakes. The
public handoff requires a capture authorization with no issuer in the current
build. The application initializer stays disconnected.

The [#145 source contract](hardware-gates/g4-contract.md) records the
installed module checks and the open, configure, prepare, start and cleanup
effects. The exact ALSA request and independent physical route observer
remain open. The owner procedure and approval for in-app live acceptance
stay in [#128](https://github.com/guirossibrum/VariaxWorkbench/issues/128).

### DAW route and in-app recording

A DAW can select an operating system capture input independently of
Workbench. The DAW path requires the OS or audio server to expose the
verified PODxt capture source. A DAW or server fixed to an integer project
rate such as 44.1 or 48 kHz cannot use the declared `78125/2` clock
unchanged. A route with conversion requires the server or DAW to open the
hardware successfully and convert from the source clock to its graph or
project clock. PipeWire documents resampling between device and graph
rates. Support for opening this PCM and preserving its channel order on
the installed ALSA node/profile remains unverified. The chosen server/DAW,
actual exposed source, and source and graph rates belong in a separate
owner-run DAW procedure.
[PipeWire format and resampler properties](https://docs.pipewire.org/page_man_pipewire-props_7.html)

The planned Workbench capture workflow uses a direct hardware PCM and
requires exact native parameters throughout. Its existing file path rejects
integer WAV for the rational clock and supports an exact-clock AIFF with
sidecar for offline takes. The WAV wording in #128 needs reconciliation
with that requirement before any live acceptance procedure.

## Unsupported routes and remaining work

External identifiers always return `ExternalContractMissing`; a name, USB ID,
or registry accounting row cannot create capture support. The existing PODxt
`RecordRoute` remains signal-path metadata and is not reinterpreted as an
external-interface type. Registry-qualified direct PODxt and Variax 600 through
PODxt plans can be validated offline; the Linux ALSA adapter consumes that
static declaration but does not open a native route. It does not establish a
physical capture backend or actual negotiation results.

Platform blockers are tied to concrete source and bounded local environment
checks. The local search covered `/usr/include`, `/usr/local/include`, `/opt`,
and the existing Cargo registry source cache; it found no Windows SDK
`audioclient.h`/`mmdeviceapi.h` or macOS Core Audio `AudioHardware.h`,
`AudioUnit.h`, or AudioQueue headers. The Windows implementation path is ASIO,
so the Windows SDK header result does not resolve its missing versioned callback
declarations. An earlier contributor-shell search also
found no `cargo`, `rustc`, or `rustup` on its default `PATH`; that is historical
environment evidence. The currently provisioned shared pinned toolchain is
Cargo 1.99.1, rustc 1.99.1, and rustfmt 1.10.0-stable. This toolchain does not
provide Windows or macOS SDK headers in this Linux worktree. The configured
`.github/workflows/offline-core.yml` matrix runs portable-core/file checks on
Ubuntu 24.04, Windows 2025, and macOS 15, with no native audio-adapter target
or SDK contract step. Those jobs are not native-adapter compile proof.

Platform blockers:

| Platform | Available source paths and facts | Exact missing contract and effect |
| --- | --- | --- |
| Windows | `src/linux_transport.rs` records a PODxt Live hardware ID without an interface/endpoint audio contract. `docs/pod-usb-audio.md` identifies Windows packet text as a different-source clue, not a PODxt Live negotiated format. A version-pinned static trace of the supplied PODxt Live x64 ASIO DLL and driver establishes a Windows ASIO host-buffer/control path and bounded byte-level callback ownership. An independently implemented offline model in `recording::adapters::offline_asio` covers owned two-half sample buffers, simulated notifications, and the observed selector-response-equals-one branch. `src/pod_hd500x.rs` records HD500X INF defaults only. | The offline module is a software contract test, not an ASIO ABI declaration, device emulator, or Windows backend. The version-identified native callback/time declarations, source-qualified physical input channel/rate contract, and independently admitted interface record remain unavailable. No generic ABI or modern header substitutes for the missing versioned declaration. The portable-core Windows CI job does not compile a native adapter or provide the missing API contract. |
| macOS | The admitted 7.6.8 driver source statically maps the exact PODxt Live personality to interface 0 and an `isoenginext` engine with two-channel input and output streams. It advertises one raw linear signed PCM format record and integer rates 44,100, 48,000, 88,200, and 96,000 Hz. The source's input conversion copies frame slices at an eight-byte stereo-frame buffer stride. These are driver declarations and callback-buffer dataflow, not OS-negotiated or physical stream results. | The matching Core Audio `AudioStreamBasicDescription`/`AudioBufferList` mapping, format-tail semantics, host callback ownership/lifetime, and an app adapter are absent. The 24-byte driver format record's trailing fields and the meaning of the eight-byte callback stride relative to its 24-bit fields remain unresolved; do not label this packed S24 or S24-in-32. The master audio clock remains unknown. The portable-core macOS CI job does not compile an adapter or supply the missing SDK/API contract. |

### Bounded macOS input-buffer trace

The already admitted Mac driver binary provides a narrow additional fact. Its
PODxt Live engine selects one two-channel input stream and exposes a source
format record carrying the `lpcm`/`sint` identifiers and raw 24/24 width bytes.
Its engine setup accepts the 24-width selector. The shared input conversion
routine copies the requested frame range from `source + first_frame * 8` for
`frame_count * 8` bytes. This proves an eight-byte frame stride at that internal
callback boundary. It does not identify channel byte order, valid-bit
alignment, the USB packet sample representation, or how the raw format fields
map to the matching Apple framework ABI. The driver format's trailing words
remain opaque. This narrows the static source gap but does not admit a macOS
capture plan or adapter. No physical capture or clock behavior is inferred.

The task forbids acquiring new SDKs or source files, so the OS contracts cannot
be filled by guessing API structures or by using another product's defaults.
The independently admitted external-interface record is missing entirely. In
`research/audio-inventory-schema2.tsv` the inventory schema has only `name` and
`role`; `src/rig/audio.rs` exports one static PODxt Live Linux declaration;
`docs/audio-capability-registry.md` leaves other rows unresolved or denied. No
admitted row binds a canonical interface identity and revision to the input
channel map, sample format, rational source clock, OS backend, and evidence
provenance. Those are the missing fields and source relation needed before an
external plan can pass admission. The PODxt declaration is not an external
interface or a runtime-negotiated result. No physical capture or hardware
clock timing is inferred. There is no global audio setting, privileged path,
monitor/re-amp activation, or hardware override.

### Acceptance-criterion status matrix

This finite matrix ties the six audio-integration criteria to the current
implementation and exact remaining dependencies. A documented platform blocker
counts for criterion 2; it does not imply an adapter or live capability.

| Criterion | Current implementation or evidence | Status and remaining dependency |
| --- | --- | --- |
| 1. Integrate recorder behavior with the common capture API | `ValidatedPlan` and `CaptureSession` own software-file and synthetic Panel sessions; `Take` remains the shared storage/save implementation. The Panel's simulated-callback choice writes generated interleaved S16 into Rust-owned fake halves, submits explicit software notifications to the same `CaptureSession`, and drains into the same `Take`; there is no second queue or Take. An atomic backpressure rejection retains the Panel cursor and permits retrying the same frames; tests verify accepted data and saved WAV chronology. Workflow tests cover both integer clocks, cancellation/restart, exact PCM/WAV payloads, save collision/failure/success, and re-open parsing. | Partial. This adds a portable software workflow only; complete platform/source-planning operations remain outstanding. |
| 2. Establish each platform adapter contract or record a bounded blocker | Linux has an inactive prepared-handle libasound adapter and fake native-boundary tests. Windows has the version-pinned ASIO static trace and a Rust-owned offline double-buffer/notification model; the latter tests selector response `1` selection without defining native ABI. macOS has the bounded driver declaration/buffer trace; Core Audio mapping remains unresolved. | Partial. Linux still needs a reviewed acquisition procedure, exact negotiation and physical route evidence. Windows still lacks the versioned native ABI, physical source channel/rate contract, and interface admission; macOS requires the matching Core Audio mapping and callback ownership contract. No acquisition or live probe is authorized. |
| 3. Admit source records and negotiated format/clock provenance | The frozen registry admits one static Linux PODxt Live declaration. External records remain absent; Windows and macOS facts do not provide an admitted plan or runtime negotiation. | Partial. Requires independently sourced identity/revision, channel map, format, rational clock, backend, and provenance for each additional source. |
| 4. Preserve bounded queue, chronology, error, recovery, and lifecycle behavior | `CaptureSession` uses bounded chronological audio/dropout events, owner-thread drain, generation checks, terminal errors, recovery events, and stop/cancel ownership. The offline ASIO-style model interleaves borrowed source-order channel buffers, applies the channel map once in `CaptureSession`, and atomically stages only the bounded clock tracker and current chunk. Adapter lifecycle/delivery operations are bound to the exact external session identity and original owner; matching-plan replacement sessions are rejected before state changes. Tests cover bounded staging independent of Take length, malformed foreign-thread notifications, same-thread alternate sessions, fresh foreign-thread sessions, repeated lifecycle, cancel/disconnect/dispose, fresh generations, and late callbacks. | Partial. The offline software tests do not verify native callback ownership, physical stream recovery, or device timing. |
| 5. Use common controls for admitted native/external plans | Panel offers route, clock, input, and shared start/feed/stop/cancel/save controls; its Linux PODxt test option revalidates the static declaration and routes generated S24 PCM through the ALSA read-result translator and common session. The source-neutral simulated-callback choice is available for 44.1/48 kHz software clocks and uses the common Panel session, Take, and save path. External-plan checks report missing admission fields. | Partial. Both generated-data paths verify offline planning/adapter translation and common Panel lifecycle, but neither negotiates or captures a native stream. An external plan still requires an independently admitted interface record; it cannot be inferred from PODxt, WAV, or synthetic metadata. |
| 6. Validate portability and dependency provenance | Offline portable-core/file workflow covers Linux, Windows, and macOS; it does not compile native audio adapters or supply platform SDK contracts. ALSA declarations are tied to the installed header version and digest. | Partial. Requires platform-specific adapter builds/contracts and exact-head checks when code changes; current configured workflow is not adapter validation. |

Validation is limited to deterministic injected buffers, software file input,
the frozen static registry, and the configured exact-head command-v1 checks.
Tests must not enumerate or open audio devices. Test results apply only to the
exact head named in the README/project receipt; they do not establish platform
adapter behavior, physical capture, drift, or safety.
