# Frozen inventory audio accounting

VariaxWorkbench issue #97 supplies the Rust registry/accounting foundation for
VariaxWorkbench issue #96. Neither inventory membership nor this registry is
working audio support. All stream opening, enumeration, capture/playback, driver
actions and live operations remain locked. There is no backend initializer or
new runtime dependency in this change.

## Source contract and identity

The operator-authorized frozen public names/roles snapshot in VariaxWorkbench
issue #96 supplies exactly 224 ordered rows: 186 aliases, 29 products, five guitars,
two bridges and two family labels. The reported “38 recovered” count includes
non-alias catalog roles; it is not 38 observed hardware/audio devices.
`research/audio-inventory-schema2.tsv` preserves spelling, order, roles and both
distinct rows named `TonePort`. Identity is the one-based source ordinal **within
this frozen projection**, never a normalized name or invented canonical device.
Aliases, family/model/revision ambiguities and duplicate catalog rows are retained.

Source schema 2 / reported original SHA-256:
`e808fe477b4d1c8a976bdaf11369e711e131317164f20d4140b816f69dd26c7b`.
The full original uncommitted schema file belongs to another worktree and was
not read/adopted. Its hash is a reported source pin, not a hash recomputed from
names. The independently pinned public projection SHA-256 is
`c3ed43ad76a0486ef327cb0cf1c42d389124c765418b0065b29f2d1db37ba790`.
`research/audio-inventory-source.json` records both evidence scopes.

VariaxWorkbench issue #93 owns the reviewed identity/probe contract. Its public
plan requires positive identity plus a separate capability record; names-only,
ambiguous aliases and bridge identities cannot identify an attached guitar.
This consumer follows that boundary and adds no numeric mapping or probe behavior.
The public frozen snapshot is sufficient for accounting, not adoption of the
other contributor's unreleased mappings. The coordination contract is
`Registry::from_projection(bytes, schema, original_sha256)`: a changed descriptor
or any projected byte/order/name/role/count/newline drift fails closed. An owner
delivery of revised IDs/aliases must be separately admitted and reviewed, with
an explicit ordinal migration; never edit another agent's inventory or silently
recompute pins. Original-file changes that are not delivered cannot be detected
by pretending to monitor that other worktree. Projection and full-source drift
are deliberately separate claims.

## Product API and machine-readable exports

The module `rig::audio` uses the existing independent `ProductProfile`, `RoleKind`
and initial physical-host metadata. It has no settings-codec dependency. Knowing
how to edit a preset never grants an audio route. The two offline CLI commands
write only deterministic registry/declaration data to stdout:

```sh
variax-workbench audio-capabilities
variax-workbench audio-route-declarations
```

The first export has a source-pin line, header and exactly 224 rows with ordinal,
original name/role, classification, planning bucket, admitted contract (or none)
and a row-specific gap. The second has one source-qualified static declaration.
Both can be consumed as TSV without a GUI feature or additional dependency.
They never discover hardware, save user files or accept a `--live` flag.

Accounting at this version:

| Classification | Rows | Meaning |
| --- | ---: | --- |
| Native USB declaration | 1 | Exact `PODxt Live` bridge row; existing Linux source declaration only |
| Maintenance-label denied | 3 | Catalyst Bootloader, Catalyst Freescale Bootloader, DL4 MK2 Bootloader: precautionary denial, normal-runtime association unresolved |
| Explicitly unresolved | 220 | Per-row identity/model/revision, endpoint or compatible external path and per-OS contract gaps; not inferred native USB |

The maintenance classification records why the named source state must not be
used for recording. It is not a verified physical identity or permission to enter
that state. Controller-like names remain unresolved aliases until reviewed rig
roles arrive; no controller is advertised as an audio source. All non-admitted
rows are rejected by the planner, including all 186 aliases. The latter includes
`Variax 600`: the existing independently typed initial-rig guitar role does not
magically establish a reviewed alias-to-source-row mapping.

## Exact declaration and separate instrument role

Only the independently existing PODxt Live source contract is populated. Evidence
is retained in [POD USB audio](pod-usb-audio.md): Linux `v7.2.15`
`sound/usb/line6/pod.c`, SHA-256
`7b07e2e44d983b03abbf78351b09f7c9cdb3a6968d455c8fb14feaf1b73fa7b6`,
and GearBox 2.0 Recording Setup Rev03, SHA-256
`0299bbb1d8c1c485adf33179deef2b2f424080ac8adcd83d687c1076efd029fa`.
This consumes accepted findings; it does not download or execute those sources.

| Field | Recorded scope |
| --- | --- |
| Actual capture-interface declaration | PODxt Live profile, USB VID/PID `0e41:4650`, POD interface 0, input endpoint `82`, output `01`; guitar control interface is separate |
| Native / specific bridge | Native interface may carry POD instrument input; initial rig may separately declare the existing Variax 600 logical guitar through that exact PODxt Live bridge. This is a typed offline rig, not a new source-alias mapping or physical discovery. |
| Directions / channels | Source declares two-channel capture and playback; no mono-stream assumption |
| Clock / format | Exact `78125/2`, signed packed three-byte little-endian 24-bit PCM; do not substitute 39062, 39063, 44100 or 48000 |
| Signal / monitoring | Guide modes 1/2: processed/clean Record Send 1–2; analog monitoring includes host playback; Aux In is not captured. No mode/monitor/reamp change or latency value is supplied. |
| OS / architecture / backend | Linux source declaration; no architecture-specific build/negotiation validated. Existing `snd-usb-pod` source is evidence, future ALSA adapter unimplemented here. No driver is installed/loaded. Windows/macOS rate/format/backend/driver contract remains unverified; requests rejected. |
| Revision / validation / gaps | Source contract `linux-v7.2.15-podxt-live-audio-v1`; installed firmware/USB revisions unknown. Static+documented+offline validation only. No permission/latency/resampling or physical support claim. |

A separate bounded trace of the already admitted macOS 7.6.8 driver identifies
the same exact PODxt Live product and a two-channel engine with four integer
rate declarations. Its shared input conversion uses an eight-byte stereo-frame
buffer stride, but the raw 24-byte format record is not mapped to the matching
Core Audio ABI. This source narrows the Mac source gap; it does not populate a
Mac registry record or prove runtime negotiation. See the qualified trace and
remaining mapping limits in [the common capture session](audio-capture-session.md).

`Registry::plan` accepts only that frozen capture row, correct profile, normal
runtime state, capture endpoint role, exact Linux source-contract version, no
claim of validated installed revision, exact channels/clock/format, and either
no attached instrument or the existing typed Variax 600 role. Wrong JTV/HD bridge
families, other profiles/rows/OSes, controller endpoints, maintenance states and
version/PCM drift fail closed. Returned `OfflineRoute` keeps capture profile,
source row and connected instrument separate, and exposes existing rig role
semantics. Its `Declaration` is informational; it contains no stream handle,
start/open method or route-changing command. A caller-supplied profile is a
planning input, not an authenticated connected device.

An analog/external-interface route is **not** fabricated for every guitar/amp/
wireless name. It needs exact documented output identity, level/connector and
compatible external-interface input plus clock/format/backend evidence. Those
records are currently absent and the gaps remain visible. VariaxWorkbench issue
#100 owns implementing those workflows; VariaxWorkbench issue #99 owns later
native family adapters and VariaxWorkbench issue #98 the common OS capture layer.

## Complete bounded family implementation plan

Every source ordinal has exactly one planning bucket exported by the Rust API.
Buckets use names as task-routing hints only; they neither merge rows nor prove
family identity. No hidden “other supported device” fallback exists. The final
effects/recorders bucket is checked against an explicit 13-name list in tests;
a new unmatched row changes the projection hash and needs review first.

| All-target planning bucket | Evidence/contract prerequisites | Later validation / adapter boundary |
| --- | --- | --- |
| PODxt including Bass variants | Exact variant/USB/runtime association and independently supported clock/format/channel contract; current Live declaration does not cover Pro/Bass | Native family adapter; per-variant OS/backend/fake-stream checks before separately approved hardware trials |
| POD HD / HD500X | Consume validated VariaxWorkbench issue #32 identity findings; exact interface and per-OS audio evidence | Independent HD adapter; never reuse PODxt rates or infer stream support from catalog IDs |
| Helix/HX | Model/revision-specific manuals/USB endpoint, monitoring and native-driver evidence | Bounded family subadapters; HX names do not imply all have audio interfaces |
| POD Go | Wireless/standard association and exact native audio contract | Own subadapter; no alias collapse |
| AMPLIFi / FX100 | Model-specific USB versus analog path, platform source/driver and signal-role evidence | Native or named external workflow only after source discrimination |
| Firehawk | Amp/FX variant, output/input and USB/backend distinctions | Per-variant family adapter or external route; no assumed parity |
| Catalyst | Normal runtime association and model-specific signal/USB contract | Native/external subadapter; both maintenance-labelled rows always denied |
| Spider | Per-generation/model outputs, native USB eligibility and source/clock/format | Separate generation contracts, not a single inferred Spider interface |
| Legacy amplifiers: AX2, Duoverb, DT, Flextone, HD147, Powercab, Vetta | Independently documented analog/digital outputs and compatible external interface; any native claim independently sourced | External/family adapters with gain/monitor-feedback safety contract; no generic USB route |
| Legacy POD / X3 / Floor / Pocket / Studio | Model-specific native versus analog path; Pocket POD no-PCM finding retained from existing source study | Distinct native/external routes; reference names and settings support cannot substitute evidence |
| Recording interfaces: GuitarPort, TonePort/POD Studio, KB/UX | Reviewed canonical variant/revision IDs and input/send/channel/driver/architecture evidence | Per-variant OS adapters; duplicate TonePort rows remain separate until reviewed mapping |
| Guitars/bridges: original Variax, JTV, Shuriken, Standard, family labels | Exact guitar identity versus USB bridge; evidence for actual audio host or analog external interface; consume VariaxWorkbench issue #61 boundaries | External/VDI rig workflows; guitar control endpoint never promoted to PCM, no JTV-specific assumption |
| Controllers: FBV variants, Helix Control | Reviewed control-only role and compatible rig association; no source endpoint | Coexistence-only adapter; explicit capture denial and no implicit MIDI/device opening |
| Wireless: Relay/XD-V transmitters/receivers | Exact transmitter/receiver pairing, audio-producing endpoint versus update-only USB, compatible output/interface | Named receiver→external-input workflow; no capture from a bodypack name or maintenance connection |
| Effects/recorders: BackTrack, BackTrack + Mic, DL4 spellings and maintenance label, Echo Pro, Filter Pro, JM4, M13, M5, M9, Mod Pro | Variant identity and native/file-export/analog path discrimination, connector/level and OS evidence | Bounded subadapters/external or file workflows; maintenance row denied, no unclassified remainder |

All buckets need deterministic fake/file lifecycle/cancellation/dropout/buffer
tests, rejected route/identity/version tests and sanitized reports. Native and
external recording claims require per-OS/architecture verification and later
separate explicit hardware authorization. The controller bucket needs no audio
backend but must prove its coexistence role without fabricating an audio source.

## Verification and remaining scope

Rust tests cover ordered 224-row accounting and duplicate preservation; descriptor/
projection drift; rejection of all 223 non-admitted source rows; independent
capture/guitar roles; wrong family/bridge/platform/revision/control/maintenance;
wrong exact clock/channel/format; deterministic machine exports; and the bounded
planning remainder. A CLI integration test exercises both real offline exports
and rejects `--live`. No test enumerates or opens a device or stream.

Local targeted Cargo testing initially stopped before compilation because this
task's offline Cargo index lacked `eframe`, even with no default features. That
setup failure is not a product pass or a worker failure. Configured focused checks
on the committed head must establish actual compilation/test results via the
coordinator; no raw full-suite substitute or dispatcher full CI is authorized.

This child can complete registry/accounting acceptance without completing parent
VariaxWorkbench issue #96. No all-device audio compatibility is claimed: 220 rows
are unresolved, three are denied maintenance labels and the one populated route
is a static declaration, not working live capture. Full schema-2 identity/alias
mapping, external route evidence, other OSes, real backends and observed hardware
validation remain independent prerequisites. Existing settings/codec support is
unchanged and does not override these gaps.

## Checkout-byte portability correction

The initial Windows run 44109577895 failed seven registry tests because Git
converted the byte-pinned TSV to CRLF. A direct `git -c core.autocrlf=true
cat-file --filters HEAD:research/audio-inventory-schema2.tsv` reproduced 225
CRLFs and SHA-256 `475882b66464eb690965c8d699af9a468fc9600a922f3a9428b207ac19b4b808`.
The file-specific `.gitattributes` rule now enforces LF checkout. The same Git
filter produces zero CRLFs and the unchanged pinned projection hash. A new
embedded-byte integration regression checks LF/count/admission on each platform.
The drift guard remains strict: no newline normalization or new accepted hash
is introduced. The failed run and initial focused pass remain historical evidence;
new-head focused and platform checks are required after this correction.
