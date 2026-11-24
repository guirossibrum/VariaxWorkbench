# Product families and application shell

**Step 1 approved.** Source inventory at `80934fd`, after #207, #177 and #181. Each
family owns its editor, files, validation and device behavior. The shell owns
navigation, the tab list, global commands and status display. The editor
layouts, file semantics and edit transactions carry forward. Artwork selection
for Line 6 and POD Farm belongs to the later layout phase.

The first three editor families are Variax 300/600, PODxt Live and POD HD500X.
The HD500X editor handles files and can show a USB presence match. Its tab says
“Live connection not supported yet.” Variax 300/600 is the file/editor family.
Automatic guitar identity is scoped to the approved Variax 600 through PODxt
Live. See the [product vision](product-vision.md), [initial rig model](unified-app.md)
and [hardware gates](hardware-gates.md).

## Target module tree

```text
app/
  main.rs                    GUI entry point, APP_VERSION, BUILD_NUMBER
  connect_try.rs             standalone experiment, outside the shell
src/
  lib.rs                     offline library entry point and compatibility exports
  main.rs                    existing CLI
  check_app.rs               existing manual-check CLI
  firmware_inspect.rs        existing offline firmware CLI
  app/
    shell/                   startup, top bar, tabs, status, help, close, screenshot
    registry.rs              composition root for built-in family registrations
    detection/               discovery service, sources, device keys, operation ownership
  families/
    variax/
      file/                  preset, bundle, document, edit domains
      editor/                Workbench panel, tab state, family artwork and help
      engine/                body, DSP, host, math, pickup and models
      live/                  identity adapter and existing editor read orchestration
      reference/             factory comparisons and bridge evidence
    podxt/
      file/                  tone and bank parsing, controls, edit transactions, library
      editor/                tone/bank panel, practice, file and tab lifecycle
      live/                  identity adapter and existing tone operation wrappers
      audio/                 existing PODxt routing policy and evidence
      reference/             model assets and USB firmware evidence
    hd500x/
      file/                  H5EP/H5ES/H5EB, controls, edits, library
      editor/                HD Edit panel and file and tab lifecycle
      discovery.rs           manual USB presence matcher
      reference/             HD driver, file, descriptor and audio evidence
  shared/
    dialogs.rs               dialog primitives and native/fake adapters
    artwork.rs               texture loading/cache, asset directory lookup
    preferences.rs           platform data paths and preference storage
    theme.rs                 existing theme application and refresh
    document.rs              atomic copy saves and family-neutral document support
    file.rs                  existing regular-file and bounded reader primitives
    transport/               shared MIDI framing and OS adapter boundaries
  tools/
    device_check/            Device check UI, reports and operation orchestration
    recording/               Test recording UI, recording core and adapters
  rig/                       existing offline capability/session compatibility API
  firmware/                  offline container inspector and applicability records
  simulation/                existing offline receiver emulation
  hardware_gates.rs          existing compile-time locks and enforcement
```

Core code remains reachable from `lib.rs` without GUI dependencies. GUI
modules remain feature-gated and can stay rooted in `app/main.rs` through path
modules during extraction. The GUI registry is independent of the static
offline rig API and of the device crate's wire-level family classifier.
Registration adds one entry at the composition root and code in the new
family. Family modules depend on shared contracts and their own device adapter.
Existing cross-family orchestration belongs at the tool or rig boundary.

### Application source inventory

The table gives ownership at the target boundary. Step 2 moves only the
listed panel and file areas. Other existing paths can remain behind adapters or
re-exports until their own scoped move. Public library paths and CLI entry
points remain available to current callers.

| Current path under `src/` unless shown otherwise | Target ownership |
| --- | --- |
| `gui.rs` | See the section map below. |
| `../app/main.rs`, `../app/connect_try.rs`, `main.rs`, `check_app.rs`, `firmware_inspect.rs` | Keep the entry points at their existing paths. The GUI calls the shell. The standalone connection experiment stays outside it. |
| `lib.rs` | Keep the library root and public exports. Its Variax wire helpers and metadata have Variax live ownership. Its PODxt routing constant belongs with PODxt live compatibility. |
| `preset.rs`, `bundle.rs`, `document.rs`, `edit.rs` | `families/variax/file/`. Variax owns its actual document, undo history, metadata and bundle transactions. `document::save_new` is already used across families and belongs in `shared/document.rs`, with the existing import path re-exported. Extract further undo primitives only when an existing caller can use them without changing its semantics. |
| `../app/guitar.rs`, `workbench_asset.rs` | `families/variax/editor/`. Generic asset location and texture cache serve the editor via `shared/artwork.rs`. Family catalog, rendering and original Workbench asset interpretation stay with Variax. |
| `body.rs` and `body/*`, `dsp.rs` and `dsp/*`, `host.rs` and `host/*`, `math.rs`, `models.rs`, `pickup.rs` and `pickup/*` | `families/variax/engine/`. Preserve coefficient profiles and offline fixture inputs. |
| `factory_comparison.rs`, `factory_reference.rs`, `variax_bridge_record.rs`, `linux_variax_prefix.rs` | `families/variax/reference/`, including the test-only prefix. |
| `pod_preset.rs` | PODxt and HD500X file trees, plus the existing compatibility façade. See the split below. |
| `pod_assets.rs`, `pod_usb_firmware.rs` | `families/podxt/reference/`. PODxt asset names can serve its file parser and editor. USB firmware data stays offline evidence. |
| `pod_farm_state.rs` | Keep the standalone POD Farm envelope inspector at its existing path. A future POD Farm family would own it. |
| `pod_audio.rs`, `rig/audio.rs`, `rig/record_transform.rs` | PODxt route data and source-specific transforms in `families/podxt/audio/`. Existing rig and recording interfaces can call the planner with the same types. |
| `podxt_live.rs`, `podxt_live_packs.rs` | `families/podxt/live/`, with Device check calling the wrappers through existing gates. |
| `pod_hd500x.rs`, `pod_hd_independent.rs`, `rig/hd500x_audio.rs`, `rig/windows_descriptor.rs` | `families/hd500x/reference/`. Its discovery registration reuses the static HD500X USB match. Existing cross-profile descriptor inspection retains its offline entry point. |
| `usb.rs`, `read_session.rs`, `identity_stage.rs` | Variax live ownership for the legacy USB/read adapters and step 2 exchange. Keep the three current paths through Step 2. Device check reports continue to consume identity exchange results. |
| `device_probe.rs` | `tools/device_check/` owns the Device check state machine, consent and rehearsal backend. `app/detection/` reuses its G1 metadata reader and synthetic source through `OsMetadataSource`. |
| `live_families.rs`, `library_backup.rs`, `manual_check.rs`, `probe_report.rs` | `tools/device_check/` owns combined read and backup orchestration, guided checks and reports. Family-specific read/backup procedures belong behind the existing family APIs. After #207, the Variax wrapper reports bank-name coverage and release failure and records partial sweeps. Keep those policies and reports at the current paths during Step 2. |
| `recording.rs`, `recording/{aiff,capture,clock}.rs`, `recording/adapters/*` | `tools/recording/` owns panel and recorder. Keep current offline API paths and feature/OS boundaries. Opening the panel does not start capture. |
| `midi.rs`, `linux_transport.rs` | `shared/transport/` owns legacy framing/identity primitives and the shared Linux-driver research record. |
| `firmware.rs` and `firmware/*`, `provenance.rs` | Existing offline firmware and provenance APIs. Shared container parsing and validation serve target-specific applicability records. |
| `rig.rs`, `rig/{backend,capability,model,registry,session,tests}.rs` | Existing offline `rig/` compatibility API. Family file calls delegate to family-owned implementations without changing the static capability matrix as part of this work. |
| `hardware_gates.rs`, `simulation/*` | Existing gate definitions and offline receiver simulator, including the test-only `simulation/write_transcript.rs` Workbench replay. The discovery simulator is a separate source for the shell. |

The `tests/*.rs` integration contracts and `tests/fixtures/**` stay at their
existing paths. Unit tests follow their owning code. Cross-panel regressions
stay with the shell integration tests. Standalone `tools/**`, build/packaging
files, documentation and local ignored research inputs retain their current
roles. The device-crate source map is in Step 4 below.

### `src/gui.rs` section map

Ranges are inclusive at the design baseline. For mixed ranges, the named
responsibility determines the destination.

| Lines | Target ownership |
| --- | --- |
| 1–74 | Imports and composition become module-local. Guitar theme constants, `ModelArt`, and Body/Pickups/Controls go to Variax editor. `Pending` splits between family open/read intent and shell close coordination. |
| 75–107 | PODxt membership and staged transactions to PODxt editor. |
| 108–298 | HD staged edit state and button labels to HD500X editor. |
| 299–415 | `FileDialogs`, native and fake adapters to shared dialogs. Family names, filters and suggested filenames are supplied by the callers once the shell lands. |
| 416–440 | PODxt filename suggestion and existing rejection presentation to PODxt editor. Preserve the HD use of the legacy rejection presenter during the mechanical move. |
| 441–459 | Recording route/clock button IDs to recording panel. |
| 460–673 | PODxt practice examples, pedalboard block enum and tone/bank fixtures to PODxt editor. |
| 674–748 | Device check backup/tone actions, availability, channel input and consent copy to Device check. |
| 749–1189 | `Workbench` state and constructor split by ownership. Dialogs, theme, help, screenshot, close and tab state belong to shell/shared services. Guitar document, catalog, drafts and read worker belong to Variax. POD/HD libraries, selections, stages and widget instrumentation belong to their panels. Recording and Device check own their state and workers. Split `pod_open` into shell navigation in Step 3. |
| 1190–1210 | Guitar load and dirty checks to Variax editor. |
| 1211–1243 | PODxt dirty and staged edit checks to PODxt editor. Shell aggregates HD dirty/staged state independently in Step 3. |
| 1244–1467 | Variax read/cancel/undo/redo, artwork, open and save commands to Variax editor and its live wrapper. Close continuations go to shell close coordination, with family callbacks. |
| 1468–2108 | Guitar edit controls, drawing and workspace to Variax editor. Generic texture cache to shared artwork. |
| 2109–2160 | Theme refresh to shared theme. App update to shell. |
| 2161–2245 | Pending dialogs and close dispatch to shell close coordination. Preserve the Variax prompt/operation handling inside the family. |
| 2246–2308 | Recording quit prompt to recording panel. |
| 2309–2716 | PODxt stage/bundle/tone prompts, save/discard and name refresh to PODxt editor. `save_next_dirty_pod` and `discard_next_dirty_pod` include HD fallbacks. Keep their invocation order in the Step 2 host and move the HD branches to HD500X editor. |
| 2717–2891 | Frame dispatch, navigation, shortcuts, footer and help window to shell. Move active read status/cancel and family Help content to Variax contributions. |
| 2892–3466 | Device check window, backend, reports, backup/tone workers and callbacks to Device check. |
| 3467–3619, 6393–6520 | Recording window, commands, inputs and file saves to recording panel. |
| 3620–6392 | PODxt panel, library, block controls, staged edits and saves to PODxt editor. The embedded HD block in `pod_window` (4971–5103), its button and scroll state, plus any HD references in POD status/revert/close, belong to HD500X or the temporary host. Keep HD embedded in Step 2, then render it as its own tab in Step 3. |
| 6521–8803 | HD quit flow, editor, slot/file selection, stage/apply, open and save to HD500X editor. Shared close coordination invokes it. |
| 8804–8812 | Cancel and join the read worker in Variax tab teardown, with shell close sequencing. |
| 8813–9053 | Startup options, display and screenshot to shell. Data directory and preference I/O to shared preferences. Family tab option, auto-select key and default artwork semantics belong to Variax. |
| 9054–13659 | HD GUI fixtures and regressions to HD500X editor tests. |
| 13660–14375 | Variax editor tests and shared app/prompt harness. |
| 14376–15460 | Recording and Device check fixtures/tests with each tool. Keep combined quit-continuation coverage in shell tests. |
| 15461–21244 | PODxt fixtures and GUI regressions with PODxt editor. Shared close simulation remains in the shell harness. |
| 21245–21534 | Variax rendering, artwork and bank tests with Variax. Startup/options, screenshot and cross-panel navigation tests with shell. |

### File split inside `pod_preset.rs`

- PODxt model IDs, typed controls, parameter domains and raw/display
  conversions go to `podxt/file/controls.rs`. Move IFF headers/chunks, PINF,
  MINF, PARM and tone accessors to `podxt/file/tone.rs`. Put validating and
  byte-patching `PodTone` methods in `podxt/file/edit.rs`. Keep tone library
  state and fixed-bank membership/child operations in `library.rs` and
  `bank.rs` respectively.
- HD framing constants, `OpaqueH5epPreset`, `H5esSetlist` and `H5ebBundle`
  go to `hd500x/file/` format modules. Model/slot association, defaults and
  typed control tables go to its `controls.rs`. Explicit edits and endian
  transforms live in the HD file implementation. `HdEditorLibrary` owns its
  dirty/baseline and extracted-child state.
- Keep `PodPresetFile`, its cross-format parser and `ToneError` compatibility
  at the existing `pod_preset` import boundary. The compatibility parser
  delegates to each codec. Both codecs can use a shared format-error primitive
  preserving existing variants and text at the façade. Route new shell opens
  directly through each family registration. The façade may know both
  families, and neither family uses the other's types or parser. Keep the
  existing regular-file admission and ceilings, sharing only the generic
  readers. Move fixtures and tests by the codec or transaction under test.

## Family and tab contract

The following is the GUI contract shape. Family IDs are stable names
registered at startup. A family registration is shared, and each tab has
its own document, selections, staging, history and runtime state.

```rust
trait ProductFamily {
    fn id(&self) -> FamilyId;
    fn title(&self) -> &'static str;
    fn file_types(&self) -> &[FileType];
    fn probe_file(&self, input: &FileCandidate) -> FileMatch;
    fn discovery(&self) -> &dyn FamilyDiscovery;
    fn open_tab(&self, input: TabInput, services: &Services)
        -> Result<Box<dyn FamilyTab>, OpenError>;
}

trait FamilyDiscovery {
    fn policy(&self) -> DetectionPolicy;
    fn candidates(&self, snapshot: &UsbSnapshot, known: &DetectedDevices)
        -> Vec<DeviceCandidate>;
    fn identify(&self, candidate: &DeviceCandidate, io: &mut DiscoveryIo)
        -> DetectionResult;
}

trait FamilyTab {
    fn show(&mut self, ui: &mut egui::Ui, services: &mut Services);
    fn connection_changed(&mut self, state: ConnectionState);
    fn status(&self) -> TabStatus;
    fn is_dirty(&self) -> bool;
    fn prepare_close(&mut self, reason: CloseReason) -> CloseState;
    fn resolve_close(&mut self, choice: CloseChoice, services: &mut Services)
        -> CloseState;
}
```

`TabInput` covers a new offline tab, a validated file and a connected
device. `DetectionPolicy` distinguishes automatic identity, manual USB
presence and offline-only families. Discovery receives only the operation
capabilities authorized for the selected plan. PODxt and Variax discovery
share USB topology through neutral candidate data. They neither import
each other's editors nor use each other's wire codecs.

The shell builds file filters and **New offline tab** from the registry.
Extensions are dialog hints. A bounded structural probe and the existing
family parser own admission. Variax claims the supported 300/600 model,
bank and collection formats by structure and target, PODxt claims its
verified tone and fixed-bank containers, and HD500X claims H5EP/H5ES/H5EB.
A shared FORM prefix is insufficient. The registry requires a unique
claim and reports ambiguous, rejected or unsupported files without
replacing any open work. Preserve family errors after admission and the
CLI path and Variax startup panel option. Each successful open creates its
own offline tab. New blank tabs reuse each family's existing example or
empty-library initialization. With no device or file at startup, show an empty shell with a start panel
for **Open file**, per-family **New offline tab**, and **Detect devices**.
**New offline tab → Variax (example banks)** opens the example banks.
The existing startup option continues to supply the example to current tests.

The top bar lists tabs and offers **Open file**, **New offline tab**,
**Detect devices**, **Device check**, **Test recording** and **Help**.
Existing editor commands and shortcuts delegate to the selected tab when
applicable. Use the tab ID as the egui ID scope so multiple editors of one
family keep their widget state independent. Device check and Test recording
are shared tools, each with its own state and existing consent or quit flow.
Help can include family contributions through the registry. The footer
shows selected-tab status alongside app version and build.

The shell sequences tab closes and application quit. A family reports its
own staged edits, applied dirty state and running operations, supplies its
existing save/discard/keep-editing prompts and resolves them. Cancel or a
failed save keeps the work and stops the close. Quit checks all tabs and
shared tools, including an unsaved recording. Copy saves use the existing
atomic non-overwriting writer.

## Detection and hot-plug

A `DetectedDevice` contains the registered family ID, a logical device
key, display model, optional validated firmware, attachment path, detection
evidence and connection capabilities. The key combines the current USB
attachment generation and logical role. A direct PODxt and its VDI guitar
share a physical USB path and have separate keys. USB addresses, an ALSA
card index and a port path alone cannot establish persistent device identity.
A late result carries the generation of its input snapshot and is ignored
after that attachment disappears or changes.

| Source | Job |
| --- | --- |
| OS USB presence | Use `device_probe::OsMetadataSource` and its G1 `LinuxSysfs` reader for cached Linux metadata. No app-originated traffic. The shell uses the USB list, and the identity adapter rechecks the existing Device check prerequisites. An unreadable snapshot leaves prior presence uncertain. A successful snapshot with a missing path is removal. Keep an adapter seam for other platforms. |
| Automatic identity | Run for PODxt Live and Variax 600 on startup or an eligible attachment event. Require a fresh G1 match for the supported PODxt Live rig, including the existing unique bridge and correct interface bindings for the Variax exchange. Cached USB presence can open the PODxt tab with identity pending or unavailable. A validated identity enables the corresponding live capabilities. Confirming the bridged guitar requires its own validated reply. Feature or OS refusal yields an unavailable status and offline work continues. |
| Manual detection | **Detect devices** opens a default-No confirmation showing “Tested on Linux: PODxt Live 3.01 and Variax 600 3.10.” It names the scan and asks before any requests can be sent. Confirming gives the other registered families a fresh OS snapshot. HD500X matches cached `0e41:4159` only. It receives zero traffic and its firmware stays unknown. Future family requests require their own approved procedure and gate as well as this consent. Cancel adds no devices and sends nothing. |
| Simulated sources | Inject a scripted USB snapshot stream, identity outcomes, and an operation owner into the same service. Headless GUI fixtures construct the shell with these sources. Existing G1 tests continue using fake metadata trees. |

The runtime may retain raw USB metadata for presence monitoring. Manual-only
families are classified and added only following explicit confirmation.
A subsequent cached snapshot can mark a previously admitted device removed.
A new attachment of a manual-only family requires a fresh confirmed scan.
For the example rig, startup brings up PODxt Live and Variax Workbench.
Confirming **Detect devices** adds the HD500X tab when its USB match is
present. Appending a device tab leaves the active editor and its edits in
place. A second snapshot for the same device updates its tab instead of
duplicating it.

### Step 3b presence behavior

The Linux shell reads cached USB metadata through the existing G1 adapter.
A supported PODxt Live USB match opens an identity-pending tab. Expected
interface bindings on that bridge also open a provisional Variax Workbench
tab with guitar identity pending. The binding describes the bridge. The
guitar's presence and firmware remain unverified until an identity reply
in Step 3c. Step 3b uses the existing offline editor constructors for these
tabs.

A confirmed manual scan admits an HD500X USB attachment for its lifetime.
The scan sends zero device requests. A subsequent attachment needs fresh
confirmation. Injected sources provide USB snapshots and logical link
observations keyed to their parent attachment. Successful absence or a
changed USB product advances the attachment generation. An unplug and
replug of the same USB product between two successful snapshots can escape
observation using the existing G1 fields.

### Bounded identity and interface ownership

Step 3c replaces the provisional Variax device tab. The PODxt tab shows
**Guitar: checking…** while identity is pending and **Guitar: no guitar
answered** after a silent exchange. Only a valid Variax 600 identity with
successful interface cleanup admits a connected Variax tab. The
[first-run owner test card](automatic-identity-owner-test-card.md) tracks
physical verification for #213 and #201.

- PODxt Live uses an identity-only session on its own ALSA raw MIDI port,
  sharing the family's typed identity request and session transport. The
  session requires PODxt Live 3.01 and runs under the existing policy mapping
  (`DeviceCheckCurrentSettingsRead`) plus G1. Automatic detection sends
  only the identity query. It does not call the combined read, tuner,
  channel, edit, pack, backup or store commands.
- Variax 600 reuses Device check step 2's G1/G2 preflight and the existing
  interface-1 open, detach, claim, alternate setting and cleanup sequence.
  The deployed Device check step 2 wrapper at this baseline performs one
  request. Reuse the device family's tested lost-first-packet behavior for
  the automatic variant. Evidence of an orphaned reply tail permits one
  resend on the same connection. Silence, mismatched identity, competing
  replies and cleanup failure stop that attempt. The current Device check
  button retains its existing consent and request semantics. Validate a
  unique supported guitar reply, release after successful claim and
  reattach after successful detach on every exit. Refusal to open stops
  before detach or send. Interface 0 is left alone by the guitar exchange.
  The automatic variant uses G1 and G2 through `run_if_unlocked`. No
  current settings or preset data are fetched as part of detection.
- Acquire one nonblocking operation guard for the physical rig before
  opening either endpoint or detaching a driver. Both logical tabs and
  Device check use it, including backup, tone and read workers. A holder
  keeps it through the complete transaction, including any undo, release,
  reattach and worker shutdown. In particular, the raw MIDI identity
  adapter must close its reader as well as its writer before it returns.
  The raw MIDI transport uses one nonblocking descriptor for bounded
  reads and writes. That descriptor closes before the identity worker
  returns. Quitting waits for the identity worker or an in-flight Device
  check backup or tone operation to finish.
  Detection defers while busy, rechecks the generation and preflight when
  ownership is available, and neither queues repeated identities per
  frame nor tries to preempt the holder.
- On Linux the app and rebuilt `tools/reso5-test` and `tools/variax-probe`
  share the advisory lock `variax-workbench-podxt-live.lock` in the
  session's `XDG_RUNTIME_DIR`, with `/tmp` as the Unix fallback. Use the
  same runtime directory for the app and both probes. The linked-recall launcher passes the locked descriptor to its subprocesses
  and holds it across pauses and restore. Older binaries and unrelated
  programs need to be closed before using the same rig. A busy or uncertain
  endpoint defers entry. A cleanup failure suspends further identity on
  that attachment.

### Disconnect and reconnect

Confirmed USB removal takes the two logical roles of a removed bridge offline.
Step 3b keeps disconnected tabs until the user closes them. This retains
uncommitted editor input during a disconnect. Dirty tabs use their normal
save-copy and quit dialogs. Keep a detached tab available for
offline work and never replace its contents with another attachment's state.
An admitted device reattaches the most recently disconnected tab of its
family. The old tab stays offline while identity is pending. If no
disconnected family tab exists, detection opens a new one. Reattachment
keeps the editor, undo history and unsaved file changes. Other disconnected
tabs remain offline and editable. Reattachment sends no editor data.
Explicitly closed device tabs stay dismissed for the current attachment
generation. Offline tabs opened by the user are independent of presence
events.

Variax VDI insertion/removal with USB still connected is a separate case.
The USB descriptor and interface-1 driver binding describe the bridge.
Run one automatic identity exchange per eligible bridge appearance, plus
an explicit **Refresh guitar** action on the bridge tab and the guitar
tab when present. Both use the same preflight, guard and bounded exchange.
The app does no periodic identity polling. A completed refresh with no reply
makes the guitar unavailable. A busy endpoint or unreadable OS snapshot
leaves the last observation uncertain. On Linux, the app listens for the
code-derived `03 11` and `7F 03` notices on the verified PODxt Live's raw
MIDI input while the rig is idle. A changed notice queues the same bounded
identity check. An identity result still determines guitar availability.
The passive listener holds the rig lock only while it has the input open
and yields to active work. Real delivery of these notices still needs an
owner run. Identity from Variax 300/600 firmware below 3.07 produces an
informational warning on the PODxt tab and stays outside live admission.

## Delivery and verification

1. **Design.** The owner approved the design on 2026-11-24. Before the
   mechanical moves, rebase onto `80934fd` and record the clarified
   startup, guitar refresh and shared-operation ownership decisions here.
   The rebased baseline is 736 passing app tests and 82 passing
   device-crate tests.
2. **Mechanical moves after approval.** Extract one area per commit in this
   order: PODxt panel ([#141]), HD500X panel ([#142]), recording panel
   ([#143]), and the PODxt/HD file split ([#144]). Move the Variax panel and
   file modules ([#140]) last, after reconciling Craft's `app/guitar.rs` and
   `gui.rs` changes. The owner-directed order supersedes the old panel
   dependency order in the issue descriptions. Keep the old host as the
   caller so layout, widget IDs, messages, save and quit behavior and
   tests stay stable. Move tests and their imports with the code. Keep
   `src/library_backup.rs`, `src/usb.rs`, `src/read_session.rs`, `crates/**`
   and `tools/**` at their current paths and untouched in this step.
3. **App shell and detection.** The owner reviews each part before the next
   begins.
   - **3a, offline shell.** The registry and tab manager open a separate
     editor instance for each file or new tab. HD500X uses its own
     `HdEditFile` type and tab. The shell coordinates dirty tabs and the
     Test recording take on quit. The start panel exposes the offline
     entry points and the disabled Detect devices button. The original
     `--tab` startup option still opens the requested Variax view.
   - **3b, detection with simulated sources.** Add device keys, the
     `OsMetadataSource` adapter, confirmation for Detect devices, and
     hot-plug and dirty-disconnect handling. Headless tests cover zero
     devices, PODxt only, PODxt plus Variax, and PODxt plus manually
     detected HD500X plus Variax. Test duplicate and stale events,
     cancelled detection, and offline tabs alongside connection events.
     Use injected USB snapshots and device events.
   - **3c, identity and operation ownership.** Add the two automatic
     identity procedures, Refresh guitar, and the cross-process lock
     across the app and both owner probes. Tool changes add the lock
     around the approved procedures and leave their messages, stages and
     confirmations intact. Test gate and busy refusal, the single allowed
     resend, and cleanup with fake transports. All test constructors use
     injected sources.
4. **Device crate tidy ([#214]).** The Variax and PODxt modules each own
   `backup.rs` for their v1 format, validation, and the directories included
   in comparison. The shared backup module keeps atomic I/O, manifests and
   the comparison algorithm. It re-exports the public backup API used by
   the app and owner probes. Existing files and restore text keep the v1
   format. HD500X remains file-only.

   Each family owns its endpoints under `family/<name>/transport.rs`.
   The shared transports select and open those endpoints. Requests carry
   a `session::Path` identifier, which the session compares with the link's
   identifier. A new link can define its own unique path in its family.
   A family may also supply an error when cancelling a chunked transfer.
   The session still handles chunking, the partial-send check and EOX recovery.

   `gate::Operation` stays the single operation registry. To add a family,
   define its requests and transport descriptors in the family module.
   Add the operations to `Operation` and have each request name its operation.
   Update each caller's `GatePolicy` to deny the new operation or explicitly
   map it to an authorized compile-time gate. Test wrong-path and closed-gate
   refusal with a fake link. Add identity classification in `family/mod.rs`
   and register any stored-file directories with the shared backup comparer.

[#214]: https://github.com/guirossibrum/VariaxWorkbench/issues/214

Run `cargo fmt --all -- --check`,
`cargo clippy --offline --all-features --all-targets -- -D warnings`,
`cargo test --offline --all-features`, and the same offline all-feature
test command in `crates/line6-devices` before every commit. Track both suite
counts and keep test counts unchanged for move-only steps. Shell tests add
coverage to the existing suites. The branch keeps the build number and
hardware gate constants. Record completed scope and verified counts in
README as implementation lands. The repository tracking parent is [#121].

[#121]: https://github.com/guirossibrum/VariaxWorkbench/issues/121
[#140]: https://github.com/guirossibrum/VariaxWorkbench/issues/140
[#141]: https://github.com/guirossibrum/VariaxWorkbench/issues/141
[#142]: https://github.com/guirossibrum/VariaxWorkbench/issues/142
[#143]: https://github.com/guirossibrum/VariaxWorkbench/issues/143
[#144]: https://github.com/guirossibrum/VariaxWorkbench/issues/144
[#177]: https://github.com/guirossibrum/VariaxWorkbench/issues/177
[#207]: https://github.com/guirossibrum/VariaxWorkbench/issues/207
