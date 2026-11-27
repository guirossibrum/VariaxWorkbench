# Rig Check source sharing on Linux and macOS

Issue [#308][308]. The comparison pins
`99a62f37e9990fbcdf773162e0f9a6af17f9e4fc` (Workbench and Rig Check 0.9.0,
catalogue 1.6). Linux and macOS use the same catalogue, runner, gated family
procedures and portable coefficient math. The major transport difference is
PODxt interface 0. Linux uses the kernel driver's ALSA raw MIDI node, while
macOS claims the USB interface directly. Both use interface 1 for Variax with
the same application-level protocol and a different ownership and cleanup
policy. The passing Mac report covers a continuation. The matching Linux
package has build evidence, and the Linux runtime report available for this
audit covers the earlier read and backup path.

## Build and entry points

| | Linux owner package | macOS owner package |
| --- | --- | --- |
| Executable entry | ELF `rig-check`, declared as `src/rig_check/bin.rs` in Cargo | `Rig Check.app/Contents/MacOS/rig-check`, from the same Cargo bin and Rust entry |
| Target and compiler | `x86_64-unknown-linux-gnu`, Rust 1.99.1 (`58cf8f9c3`) | `aarch64-apple-darwin` (11.0 minimum) and `x86_64-apple-darwin` (10.15 minimum), same Rust release and commit, SDK 27.2 in the verification record |
| Profile | Locked offline Cargo `release`, defaults off, `gui,portable-math,usb`; package label `owner-usb` | Same profile and root features for both slices; `build.sh --owner-usb`; `lipo` then ad-hoc signing |
| USB linkage | The delivered ELF has `DT_NEEDED libusb-1.0.so.0`; the device library's `rusb` uses the same checked-in Rust wrapper as on Mac | Both slices have the verified static libusb symbols and use Apple system frameworks, including IOKit, CoreFoundation and Security. There is no Line 6 driver dependency |
| Other launch path | The same `main` dispatches to the wizard or to the terminal with `--text` and explicit `--real`/`--practice`. The older Linux standalone kit's shell wrapper passes `--out` beside the executable | Same wizard and terminal dispatch. Finder launches the bundle; Mac-specific argument filtering removes `-psn_` and the native options explicitly select Glow. Linux uses the default renderer of this Glow-enabled build |
| Companion binary | `variax-workbench-gui`, rooted at `app/main.rs` and `src/gui.rs` | Same companion binary sources, packaged separately as `Variax Workbench.app` |

The Linux and Mac verification files, release build logs, Mac Cargo artifact
records and the paired-package checksum manifest establish the build scope.
The package label is distinct from Cargo's profile name. The [pair
recipe][pair] was integrated after the pinned source revision and checked out
that exact tree for both builds. The [pinned macOS recipe][mac-build] also
has a different, public offline profile (`gui,portable-math`, no `usb`).
The [pinned Linux standalone kit][linux-kit] uses Cargo defaults
(`gui,usb`) and therefore host math unless built with a different invocation.
These two older/default recipes alone do not establish matching math or USB
features. Build evidence is from the owner pair and the earlier private Mac
owner delivery from the same commit. The earlier DMG and paired DMG have
different file hashes. The runtime report identifies the source commit,
target and profile; byte identity of its running Rig Check executable with
the later paired DMG is unverified. [Cargo manifest][manifest] ·
[Rig Check entry][entry]

Both bin roots link the app library. Workbench has its own editor shell and
Device Check workflows. For example, the Mac owner observed a passing Rig
Check Variax continuation and an identity timeout in Workbench from the same
revision. Rig Check 3.1 retries silence through the family procedure, whereas
Workbench automatic detection and Device Check G2 have different request
bounds. The owner observation contains no message trace for the Workbench
failure. This is a separate consumer of the USB adapter and is tracked under
[#201][201]. [Rig Check identity][steps] · [Workbench Mac discovery][mac-app] ·
[Device Check USB entry][app-usb]

## Steps and platform behavior

The 41 entries run in `catalogue::RUN_ORDER` through `steps::execute`,
`Runner` and `RigPort`. Both use the same approvals, firmware checks,
reporting and start/save confirmations. For this rig, 2.7 and 3.7 are
skipped on verified firmware. 2.4 exists in the historical Linux report but
is absent from catalogue 1.6 on **both** targets. Step 7.5 runs before 7.3
and 7.4, and 6.2 is the final guard. [Catalogue][catalogue] · [Dispatcher of
step procedures][steps] · [Runner][runner] · [Approvals][approvals]

| Step | Shared procedure and user-visible result | Effective platform difference |
| --- | --- | --- |
| 0.1 / 0.2 | `steps::computer_system` and `computer_usb` populate one report schema. OS inventory is passive and does not identify device firmware | Linux `computer::linux::{system,usb}` collects distro/kernel/CPU, USB topology and speed, interfaces/drivers, device-node permissions, ALSA cards/MIDI nodes and relevant udev rules. Mac `other::{mac_system,mac_host_devices,mac_usb_view}` uses `sw_vers`, `sysctl`, loaded/listed extension queries and the IOKit registry snapshot to report PODxt location, route, address and published MIDI interface services. Host controller, per-interface binding, device-node permissions, USB speed and CoreMIDI ports are unavailable on the Mac path. CoreMIDI server discovery is deliberately absent. Presence of a published interface service does not establish claimability. Mac distinguishes absent, unavailable and partial inventory. [#251][251] |
| 0.3 | `engine_check::builtin` and the bundled model audition exercise `HostEngine` with the same fixtures, model data and `software-wide-v1/libm-0.2.16`. Optional local reference-folder comparison is shared | Host metadata and CPU target differ. The historical Linux report used `host-std-v1`; its 0.3 pass is evidence for that build only. This check compares words and hashes offline |
| 1.1 / 1.2 | `connection::see_podxt` requires exactly one supported PODxt Live from 0.2. `open_and_release` opens and gives back the guitar interface without sending a family message | Linux reports before/after `snd-usb-variax` and requires the expected binding on return. Mac checks ownership before claim, refuses an active driver on either interface, and reports interface release without a kernel rebind requirement |
| 2.1–2.3 / 2.5–2.7 | `steps::{podxt_identity,podxt_current_tone,podxt_library,podxt_version_comparison}`, `podxt_reads::one_channel` and `live::podxt_editor_get` use the same identity parser, 168-byte current/stored tone, addressed library sweep (299 tones and setups), packs/globals reads and tested-firmware decision. 2.5 compares the playing tone after reading storage | Linux uses `RawMidiLink` throughout. Mac uses interface 0 whenever the PODxt link is requested. Adapter receive boundaries and timeout/ownership errors can differ even with identical family requests. [#262][262] |
| 3.1–3.5 / 3.7 | `steps::{variax_identity,variax_current_program,variax_all_presets}`, `variax_reads` and `variax_evidence` share identity (300/600 cannot be distinguished by this reply), current settings, the 35-chunk 1,092-word read, the 60-preset/12-bank-name sweep and return, and the one-preset read/compare. Backup may select working presets, then attempts return. Firmware outside the tested scope has dedicated read-only evidence paths | Both use PODxt interface 1. Linux detaches/rebinds and Mac releases/claims on family handoff. A pass on 3.1 or 3.3 says nothing about whether a separate editor consumer retries the same way |
| 6.1 / 7.1–7.5 | `recover::vdi_replug` checks unplug/replug. `guided::{selector,knobs,footswitch,pedal,podxt_knobs}` share ready/action windows, listen and retry prompts. 7.1 recognizes target/return, 7.2 logs guitar volume/tone, 7.3 checks the requested and return channel against backed-up tones and can restore unsaved edits, 7.4 listens for the pedal, and 7.5 passively checks all six tone knobs (near-ends ≤20 and ≥107, exact 0/127 reported separately) | Linux listens via the driver for PODxt and USB for Variax. Mac switches the single claim to the family required. Identical acceptance ranges are source evidence. Native event delivery and timing are separate evidence. [#298][298] |
| 4.1–4.8 | Shared reversible checks cover PODxt control/undo, Variax own bytes, Variax audition/return, trim and tuning/return, PODxt own-tone upload, channel recall/return and tuner on/read/off. The recorded current or verified saved baseline drives the comparisons | Same application and family code. A Mac interface switch can fail before the opposite-family procedure, with a latched release error when the prior claim could not be closed |
| 5.1–5.4 / 6.2 | Shared backup/slot checks, typed confirmation, single-attempt stores, bounded receipt waits and read-back. 5.1 saves a Variax slot's own bytes, 5.2 does likewise for a PODxt channel, 5.3 verifies temporary Variax trim and restores the backup, 5.4 verifies two own-byte stores and returns. 6.2 compares current state with the run's baseline, may offer recovery and releases the connection | Mac 5.1 has a direct-USB-only pre-upload recovery/retry path. Final Mac proof includes release of the active USB interface. Linux proves guitar driver return when it held the guitar claim; the ALSA descriptor closes with the port. [#258][258] |
| 8.1–8.5 | 8.1 is inventory only. 8.2–8.5 are unavailable with audio streams disabled in both builds | `audio::detect_linux` reads published ALSA PCM metadata and PipeWire/Pulse socket presence. `detect_macos` parses `system_profiler SPAudioDataType`. A passing 8.1 can report no PODxt audio device, as in the inherited Mac result. Neither report establishes recording, playback or latency |

Source for the table: [Linux inventory][linux-inv] · [Mac inventory][mac-inv] ·
[Connection][connection] · [Engine][engine] · [PODxt reads][pod-reads] ·
[Variax reads][guitar-reads] · [Guided checks][guided] · [Live PODxt][live] ·
[Live Variax][guitar-live] · [Recovery][recover] · [Audio inventory][audio]

### Selection, transfer, retry and release

- **Linux real port.** `RealPort::podxt` resolves `PODxtLive` via
  `RawMidiLink::open`, with the ALSA card list selecting a nonblocking
  `/dev/snd/midiC<n>D0` node. `RealPort::variax` acquires the rig lock
  and uses `UsbInterfaceLink::open` to select a single matching USB
  VID/PID. Those opens do not carry the exact attachment from 0.2 through
  both links. A replacement between inventory and open is therefore not
  excluded by the inventory selection itself. Family identities are checked
  separately. Interface 1 detaches only its active driver if needed.
  `UsbInterfaceLink::finish` releases the interface and verifies the
  expected driver on the observed route/address with a bounded rebind
  attempt. Rig Check can hold the raw MIDI node while it has the guitar
  claim. The separately compiled Linux direct-interface-0 branch is closed
  by `PODXT_DIRECT_USB_APPROVED` and is outside Rig Check's Linux PODxt
  path. [Real port][port] · [ALSA link][raw] · [USB link][usb] ·
  [PODxt USB profile][pod-usb] · [Linux driver check][guitar-driver]
- **Mac real port.** `open_direct_usb` locks the rig, requires one IOKit
  candidate with MIDI services and compares the saved registry generation
  from inventory, if present. It matches route/address against libusb and
  verifies the locked registry ID and attachment again at each open.
  `DirectUsb::link` releases the prior family before acquiring the next.
  `UsbInterfaceLink::prepare` checks both interfaces for drivers and
  refuses a conflict without detaching. The two specs use raw interrupt
  transfers, alternate setting 1 and up to 16-byte writes. Interface 0
  uses endpoints `03/84`; interface 1 uses `05/86` and a 5 ms gap on
  both systems. A release failure is latched and prevents reopening.
  [Real port][port] · [Mac USB claims][mac-usb] · [Registry match][mac-match] ·
  [PODxt USB profile][pod-usb] · [Variax USB profile][guitar-usb]
- **Shared protocol and stop rules.** `with_podxt_progress` and
  `with_variax_progress` wrap the selected link in the same gated, logged
  `Session`. Current-tone reads recheck identity, use shared bounded
  retries, reject a mid-read channel change and quarantine unresolved
  outstanding replies. 3.1 retries silent identity; chunked Variax reads
  handle damaged replies in the family layer. Guided retries repeat a
  listening window. Stop is passed to procedures that accept a stop
  callback, with bounded I/O and cleanup at the relevant boundary. 6.2
  is normally offered after Stop. Unknown store outcome, checkpoint failure,
  uncertain direct USB cleanup or an unverified relay's evidence-only run
  takes the release-only path. Active Mac USB is explicitly released if
  the final guard is declined. Linux retains separate file-handle drop
  and guitar binding checks. [Sessions][session] · [Session wrappers][procedures] ·
  [Current tone][pod-reads] · [Family chunk reads][span] · [Runner][runner]
- **Store policy and the 5.1 exception.** `Runner::precheck` requires
  earlier applicable steps to pass before a store. An earlier failure
  blocks later live/store cards. The final guard has separate recovery rules. 5.1 checks identity and the slot against
  3.4, uses one `0x43` upload, a 100 ms gap, one `0x61` store, a single
  `0x71` wait of at most 15 s, and full read-back. Silent store waits
  two minutes with no more device messages and no resend. 5.2 compares
  with 2.5, waits up to seven seconds for the PODxt receipt and reads back the addressed slot.
  Mac's `check_save_identity` may release and reopen once for the known
  silence/orphan-tail sequence, and offers manual **Try again** after
  eligible pre-upload failure with verified release (also for complete
  silence). The attempt remains in the report. Retrying redoes preflight
  and confirmation. The shared 5.1 checkpoint advances to `send_possible`
  before upload, which rules out that retry. On Linux the direct-USB
  identity recovery branch is off. [Variax save][guitar-save] ·
  [PODxt save][pod-save] · [Save identity][save-id] · [Runner][runner] ·
  [Step record][record] · [#258][258]

### Reports, current state and Resume

Both builds create a unique `rig-check-<UTC>.txt` with human-readable
results and format-1 JSON, a sibling `.state` checkpoint, and an
`-evidence` directory for real runs. `ReportFile::update` atomically
replaces that run's working report, and `save_copy` creates a new file
without overwriting. Audit scope is the pinned redactor and all raw owner
material remains private. On Linux `report_locations` defaults to
`rig-check-output` beside the executable (relative fallback if the
executable path is unavailable). That directory needs to be writable.
Mac defaults to `~/Documents/Rig Check` with temporary-directory fallback
if home is unknown. `--out` overrides the write location on both. Mac
also discovers legacy `~/Library/Application Support/Rig Check/reports`
and can browse it separately. Linux's `last_report` picks the newest
report with a checkpoint in the output directory. Mac's
`last_mac_report` searches both roots, accepts a collected `latest.txt`
with companion state, and prefers the newest compatible candidate for the
requested mode. Linux uses `rfd` for choose/save dialogs. Mac uses
Standard Additions via `osascript`, with an Open-in-directory fallback.
The terminal path starts a run and writes reports but has no wizard
Resume selector. [Entry][entry] · [Report writer][report] ·
[Wizard][wizard] · [Native dialogs][dialog] · [Resume][resume]

The shared `resume::open`, `Plan::revalidate` and `continue_on` validate
the report/checkpoint and evidence hashes, keep prior attempts, and create
a separate successor report. Ordinary Resume rechecks identities and
refreshes PODxt channel/tone and the Variax program from current physical
state. Earlier passes and valid backup evidence carry over. A changed
working buffer can become the new baseline without automatic restoration
of the old working buffer. An unmatched Variax program can still be used
for working-buffer checks, but a store needs a verified eligible slot.
An unresolved store or unverified cleanup requires manual verification.
A blocked read-only recheck names the selected step and failing prerequisite,
and **Start fresh** after a blocked recheck depends on verified release.
There is also a constrained migration for historical reports stopped at
the old guard. It revalidates the saved baseline and libraries and admits
guided checks and a fresh guard with stores and live cards unavailable.
Mac has two roots to check for unresolved prior runs, Linux has one. On
either platform, an unrelated valid unfinished report with proof of no
pending store does not prompt during selected Resume. [#297][297] ·
[#298][298] · [Resume][resume] · [Evidence checks][resume-evidence] ·
[Recovery][recover] · [Wizard][wizard]

## Runtime evidence and offline checks

| Evidence | Observed scope | Remaining uncertainty |
| --- | --- | --- |
| Mac owner, 99a62f37, ARM64 Apple M1, macOS 27.2, firmware PODxt 3.01 and Variax 3.10 | Completed continuation targeting 5.1. The final catalogue has **35 passed, zero failed, two tested-firmware skips and four audio steps unavailable**. Thirty earlier passes came from the `175144ca` run. At 99a62f37 the read-only 2.1, 2.2, 3.1 and 3.3 rechecks passed. New steps 5.1–5.4 and 6.2 passed. 5.1 recorded one store, its receipt and matching read-back. 5.2 recorded its receipt and byte-for-byte channel read-back. 5.3 verified the temporary change and restoration, 5.4 verified both slots and return, and 6.2 reported the original 168-byte tone and 1,092-word program plus interface release | A clean start through the whole 99a62f37 catalogue and Intel Mac runtime are unverified. The inherited 0.3 had 48/48 synthetic cases match, a full 1,092-word synthetic preset match and the expected bundled audition hash. The inherited 7.5 observed 0–127 on each of the six physical knobs. Those rows are owner evidence at the predecessor revision |
| Mac predecessors, same portable math | `175144ca` supplied the successful 0.1–4.8 sequence in run order, including 6.1, 7.x and 8.1. Its attempt history includes a 4.2 orphan-byte failure followed by a pass and a 5.1 pre-upload identity failure with no store. Another attempt at that revision was blocked in 2.2 revalidation. An earlier `dac5321` report failed 3.3. The current-settings continuation at the pinned revision succeeded | Cause of the orphan-tail replies remains open. A successful Resume from saved preflight evidence does not exercise the wizard's **Try again** button during a physical run. Legacy directory migration and error paths have narrower evidence |
| Linux owner, `0e78a06c914b`, x86-64, catalogue 1.4, `host-std-v1`, firmware 3.01/3.10 | **15 passed, zero failed.** Physical rig portion: **0.2, 1.1, 1.2, 2.1–2.6 and 3.1–3.4**. 0.1 measured the real host and 0.3 passed the older offline check. The report observes `snd_usb_pod` and `snd_usb_variax` and verifies 1.2 return. 3.7 skipped on verified firmware. 3.5, 6.1, all 7.x, 8.1 and all 4.x/5.x were not reached. The operator stopped before the 6.2 guard | Historical owner runtime supports the Linux read/backup route, including the since-removed 2.4. It leaves the pinned portable-math run, six-knob sweep, current-state Resume, save, final guard and cancellation paths without same-package physical evidence |
| Linux owner pair, 99a62f37, `gui,portable-math,usb` | Package checksums, ELF dependencies, build verification, and matching commit stamp. A second local Linux build at the same commit states the same root features and has a separate checksum manifest | No real-device Rig Check report from either exact Linux package was found in the available owner evidence. Build checks cannot establish setup, driver return, USB timing or recovery on the rig |
| Earlier Linux procedures outside Rig Check | Device probe ledger documents addressed PODxt reads, CC edit/undo, recall/return and tuner, as well as the Variax full read (#234), edit-buffer audition (#176) and edit/store/restore ladder (#238) | These are physical observations of those procedures. They do not supply a Linux Rig Check result for 3.5, 4.x, 5.x or 6.2, or a PODxt Rig Check store result. [Owner procedure ledger][status] |

The inspection parsed real-mode report headers and per-step JSON, including
`reused`, `prior_attempts`, `save_phase` and attempt timestamps. For reproducible
private lookup, the report SHA-256 values are
`059ee29d6e696c99c121f4632fe0b0f55dba4477767bc6f2a2a1517e4f65b3ba` for
the Mac 99a62f37 pass,
`3b7a47149a87f5c178082ba871f165fa3daa33966553af39933bb7a15dc976e2` for
its resumed source, and
`d391717bc518ba57de9a8f4bf7fa68db697bb5a1f75556dc79d011f6018a3f30` for
the Linux 0e78a06 report. Reports, state files, backups, screenshots and
raw captures stay in private local storage.

Existing offline verification at the pinned integration head records
**1,280 app tests passed (5 ignored)** and **315 device tests passed
(2 ignored)**, with zero failures and formatting/strict Clippy recorded.
The focused 5.1 fake-link run passed nine cases including silence/orphan
tail, release failure, stop, repeat, checkpoint failure and the
post-upload boundary. Headless UI verified visibility of **Try again**
for pre-upload failures only. Private offline previews picked 5.1 for
the `175144ca` report (30 passes eligible for reuse) and 3.3 for the
`dac5321` report. The checked-in test cases also show fake interface
handoff/cleanup and failed-release recovery, report discovery with two
roots, current-state revalidation, uncertain-store refusal, synthetic
inventory parsers and knob/footswitch events. Practice injects a Linux-shaped
computer and simulated links on both platforms. This audit inspected the
existing logs and source and ran static evidence checks. It did not
execute the binaries, build Cargo targets or access devices.
[Fake direct USB][fake-usb] · [5.1 fake preflight][fake-save] ·
[Headless retry][gui-tests] · [Resume tests][resume-tests] ·
[Guided synthetic cases][guided] · [Inventory fixtures][mac-inv]

## Ranked gaps and source consolidation

| Priority | Confirmed gap or difference and consequence | Maintenance cost and scope of possible consolidation |
| --- | --- | --- |
| 1 | The delivered Linux pair has no matching physical report, and the passing Mac report is ARM64 and reuses earlier passes. Same catalogue and hashes cannot settle six-knob event delivery, release/cancel or full save/Resume behavior on Linux. Intel Mac has native build and linkage evidence only. [#263][263] · [#298][298] | No protocol consolidation needed. Comparable native acceptance evidence at the pinned build would close the largest gap. Carry reused versus fresh rows in any acceptance record |
| 2 | Linux real Rig Check selects PODxt by ALSA card ID and guitar by VID/PID, while Mac binds route/address and generation under the lock. Inventory and open can describe different Linux attachments after a change. The two OS close paths also offer different evidence to the runner. [#262][262] · [#304][304] | Highest implementation cost. The existing separate capture adapters already converge on `capture_fresh`. The Linux attachment-aware API is available elsewhere in the crate. Any shared Rig Check attachment/finalization policy would need to preserve the different descriptor/USB lifetimes and Mac's one-claim rule. Keep native adapters |
| 3 | Mac false starts in identity or revalidation are observable, and its safe pre-upload retry differs from Linux. Retry admission after clean release and refusal after possible upload are separate from transport recovery. The passing continuation proves the saved pre-send path on one rig; physical timeout, cancel, reattach and failed-release branches lack comparable evidence. [#258][258] · [#297][297] | Medium cost. The common runner already owns retry history, checkpoints and refusal rules. Reuse them if another adapter can prove a safe pre-send failure and release. Preserve per-link recovery and existing store boundaries |
| 4 | Mac 0.2 reports less driver/device detail, and 8.1 can pass with no PODxt audio device reported. Linux users get driver/node advice and ALSA PCM metadata. Screen and report checks that assume Linux fields cannot infer missing Mac details. [#251][251] · [#262][262] | Low to medium cost for shared inventory presentation and status vocabulary. OS data collection and missing-field reasons must remain distinct. No stream tests follow from 8.1 |
| 5 | Report selection differs by OS. Linux may pick an incompatible newest checkpoint when an older usable one exists. Mac prefers a compatible report across its roots. Users also encounter different default paths and native dialogs. [#297][297] · [#298][298] | Lowest code cost for reusing compatibility-first selection with one or more roots. Retain the actual roots, native dialogs and Mac migration behavior |

The pinned tree supplies one application implementation for most steps.
The table below identifies the places where effective implementations
diverge. Among the distinct production Rust files enumerated there, the
Git blobs contain no complete file duplicates. Exact file comparison does
not exclude repeated small statements. The relevant overlapping blocks
and their semantic differences are:

- `computer/linux.rs` and `computer/other.rs` both produce host and
  device report fields. The collectors read different OS sources and
  already converge on `FileSource` or `CommandRunner` inputs and a
  `UsbView`/line result. This is OS adaptation. The common scheduling and
  rendering live in `steps.rs` and `report.rs`.
- `transport/rawmidi.rs` and `transport/usb.rs` both implement `Link`
  (`path`, time, read and write). There are small identical trait-method
  bodies for timekeeping. ALSA uses file I/O against the driver, while
  USB chunks interrupt transfers and explicitly manages the claim. The
  parser, timeout policy at the family level and messages are shared at
  `Session` and the family procedures. Consolidating the adapter I/O
  would obscure the ownership difference.
- `snapshot/linux_raw_midi.rs` and `snapshot/macos_usb.rs` serve the
  same separate fresh-capture purpose. Both pass a checked open and finish
  to `snapshot/fresh.rs::capture_fresh`. Linux supplies a cancelled
  callback and attachment-bound ALSA descriptor, while Mac verifies a
  registry generation and uses a fresh borrowed USB claim. These are
  parallel adapters with different callbacks, not separate implementations
  of the 299-entry protocol. Rig Check 2.5 uses `podxt::read_library`
  through `RealPort`, without these fresh-capture wrappers. [Linux
  capture][linux-capture] · [Mac capture][mac-capture] · [2.5][steps]
- `procedures::{with_podxt_progress,with_variax_progress}` repeat the
  literal step-policy construction, progress/stop closures and log
  absorption in one shared file. The PODxt wrapper also handles reply
  quarantine, and the Variax wrapper enforces expected SysEx. This is
  shared application boilerplate across families. Factoring it offers
  little platform benefit and would need to preserve both receive policies.
- `bin.rs` and `gui/mod.rs` hold the real differences in roots, report
  selection and dialogs. `macos_dialog.rs` and the Linux `rfd` call
  serve the same UI purpose through different OS facilities. The report
  writer, parser and recovery admission are already common. The policy
  difference in which candidate is chosen is the part amenable to
  consolidation.

The original Workbench Java and native code is a historical reference.
[#262][262] cites its derived Workbench 1.75 Java device layer comparison
and Line 6 private port/relay inference. The original application's
private driver port and 256-byte host chunks describe the historical
route. The current Mac build uses a driverless direct claim and its own
16-byte USB transfer profile. [Derived transport trace][transport-research]

## Source and package inputs

The tables cover production Rig Check files, the relevant main-crate and
device-library dependency closure, the USB wrapper/native selectors, and
the companion binary roots used to distinguish the two executables. They
describe module selection from manifests and source, with artifact and
linkage records for the builds. They are not linker maps of retained
functions or a registry dependency SBOM. In particular the Linux-shaped
inventory files are compiled in the app library for both targets so
Practice can use them. Standalone test files, unrelated editor modules and
other probes are outside the Rig Check source table.

Reproduce a row with the pinned commit and its repository-relative path.
The third field of `git ls-tree` is the blob ID. Use the full source tree
(`ec193459b7c2132118c94c1f75e49a7d85cc2e41`) and search for module
declarations, target predicates and `include_bytes!` to check selection.
The one later packaging row names its own revision.

```sh
rev=99a62f37e9990fbcdf773162e0f9a6af17f9e4fc
git ls-tree "$rev" -- src/rig_check/port.rs
git rev-parse "$rev:src/rig_check/port.rs"
git grep -nE 'cfg|include(_bytes|_str)?!' "$rev" -- src/rig_check crates/line6-devices/src
git show "$rev:Cargo.toml"
git show "$rev:crates/line6-devices/Cargo.toml"
```

In the tables, **Both** means the same source path and production branch
for the compared profiles, except where the selection column qualifies
it. **Both, branched** indicates a material OS or feature selection within
that file. **RC bin** and **WB bin** identify exclusive binary roots
(Linux and Mac), even though both binaries can link the shared library.
Practice and off-path library APIs compiled under the same target are
labelled when they matter.

### Rig Check, engine and binary roots

| Repository path | Git blob ID | Effective selection |
| --- | --- | --- |
| `src/rig_check/approvals.rs` | `88cb51a5599a8d5e56c8e0767ae3e0bf46e97b37` | Both |
| `src/rig_check/audio.rs` | `49b1ffcd9c39b749d70ab8acd9dabfc1514c2871` | Both, branched · OS-specific passive inventory |
| `src/rig_check/bin.rs` | `c53cca195250c1c737c077cd9dca96c76e42c363` | RC bin, branched · GUI/text, Mac launch, output roots |
| `src/rig_check/catalogue.rs` | `d23d51c9ede67a71a51174b4fb8796ba016a3acb` | Both |
| `src/rig_check/cli.rs` | `4b57581a550a13449ac3ff98fd2761f227f0703b` | Both |
| `src/rig_check/compare.rs` | `548d642b03e33e09be4e8a32bbceff3d87f845b9` | Both |
| `src/rig_check/computer/linux.rs` | `0cdce5d696b5cadfd87eb001ba436e0f78e324c4` | Both · native Linux and synthetic Practice on both |
| `src/rig_check/computer/mod.rs` | `60e17a2c44389f4dc4a0da37d110a7243f560b94` | Both, branched · architecture flags and `usb` runtime version |
| `src/rig_check/computer/other.rs` | `ef4e06d21ff2e0cba29a104244c774a4e44da4ed` | Both, branched · Mac/Windows command admission and Mac USB view |
| `src/rig_check/connection.rs` | `a87f9f393fe96c49cc058ca448fb3d3ea3a972e3` | Both, branched · Linux/Practice driver checks |
| `src/rig_check/differences.rs` | `974f27e95a17063d64b1d2ef9f728e5fea4052a3` | Both |
| `src/rig_check/engine_check.rs` | `f4fafa92fb8880d864fb169a5d099c36ec0893fb` | Both |
| `src/rig_check/evidence.rs` | `951aab2a73a2a5cd6e6277104bffb0f7d60df467` | Both |
| `src/rig_check/gui/mod.rs` | `c490a0d557c88e6d4c66155c08ace53dbadb101a` | Both, branched · native dialog and availability branches |
| `src/rig_check/gui/raster.rs` | `572a3f3458df714c04eddfc387319a4114643a3b` | Both · `gui` module, off the normal wizard render path |
| `src/rig_check/guided.rs` | `4fab4ff8a7c53552818b9f82fc6427f39b2b27a4` | Both |
| `src/rig_check/hints.rs` | `7b0947d9be0f2481b02c7911f096a5126dca2c88` | Both |
| `src/rig_check/history.rs` | `380b312c27dbba1f89084a80675ab4d84902097a` | Both |
| `src/rig_check/live.rs` | `1dd328391dffa64bd4aec6f55c37f7fb0d0db186` | Both |
| `src/rig_check/mod.rs` | `89d85be223356fd7ce17c99c4e47d4a0ae2ef86e` | Both, branched · `gui` selects wizard |
| `src/rig_check/podxt_reads.rs` | `44c5afac506636220a2c7d1af141c4fc2c7b47d8` | Both |
| `src/rig_check/podxt_save.rs` | `54c7f3ce6f015b4287a9584bded2faf1cb9f23e7` | Both |
| `src/rig_check/port.rs` | `c77b0f5b978228655df6c9e2c8ef2ff21e7e57b3` | Both, branched · Linux ALSA/guitar handle vs Mac single direct USB claim |
| `src/rig_check/practice.rs` | `c7d3c66ceef2c85eaafb6013887edcbe30b6528d` | Both |
| `src/rig_check/procedures.rs` | `feee0872d4ff4ec554ce6edaeb5d53a61b6f0b75` | Both |
| `src/rig_check/record.rs` | `239df33f090a19e13a5289e188ba42e0efc88964` | Both |
| `src/rig_check/recover.rs` | `8f458e3dbb0926dcdecaac7ad25871a92ca44c5f` | Both |
| `src/rig_check/recover/vdi.rs` | `3d2406eb06ba5d8af361a582492958b5cbe3b9bb` | Both |
| `src/rig_check/redact.rs` | `3bb4aa451c22a91390636d567002ee075a41ba90` | Both |
| `src/rig_check/report.rs` | `cbf70a7f944d13b20f270fcbf67ff9c4eb5c94db` | Both |
| `src/rig_check/resume.rs` | `b9698ba742dbfd80699c3853c37839f386523adb` | Both, branched · `gui` report-discovery helper; direct-USB policy via port |
| `src/rig_check/resume/evidence.rs` | `7a13265d333bc1ae6b0aff259aa1ef94f7414297` | Both |
| `src/rig_check/runner.rs` | `300ea2f3717c41ecfbfec3dbede7fadf05e14795` | Both, branched · build availability notice and runtime direct-USB cleanup |
| `src/rig_check/steps.rs` | `4f18f27987ed37810a597754677404b451be37e5` | Both, branched · native inventory dispatch; common step table |
| `src/rig_check/variax_evidence.rs` | `cbad22fd2676ae0182977a1e80fc9911c1d609a2` | Both |
| `src/rig_check/variax_live.rs` | `d6b4cc62d9b8cdafabb9ca1d829b5548c71ed981` | Both |
| `src/rig_check/variax_reads.rs` | `c36c2d9e79b68dfe7e74b322d147f3e7ce3e4683` | Both |
| `src/rig_check/variax_save.rs` | `eff0d3600522d0b89569c2609bc5b8da658ac63f` | Both · direct-USB-only preflight recovery selected at runtime |
| `src/lib.rs` | `58aa321c0962edd6d7da3680dd0ddc65c201c46f` | Both, branched · `gui`, `usb` and Mac consumer selection |
| `src/hardware_gates.rs` | `9e52ad5ebb668039b9b5a9ab9eda6495568dc0eb` | Both |
| `src/math.rs` | `a74e624cb3a0b98830eb10713d0928894be07199` | Both, branched · `portable-math` selected on both |
| `src/usb.rs` | `faea7369a419771f25b5c73cb4f871e15508a853` | Both, branched · companion USB entry points under `usb` |
| `src/macos_dialog.rs` | `868663f594f6cb075fbd444b3b581c20222da838` | Mac only · `gui`, Standard Additions adapter |
| `app/main.rs` | `9762708a76d551125f503ec948726ba17523f172` | WB bin · entry root |
| `app/BUILD_NUMBER` | `aabe6ec3909c9d4871f8f89ef88ca3b6795ebd29` | WB bin · compile-time included integer |
| `src/device_probe.rs` | `1765e6fe1363c8937f21de78da64f9ac01f173bb` | Both, branched · Device Check G2 branch outside Rig Check |
| `src/gui.rs` | `f72d5357cc22e0ccc087c6becd4c747a561b1f06` | WB bin · editor shell root with platform branches |
| `src/dsp.rs` | `17d632fe3310e87db8077bcca631ed519edf071e` | Both |
| `src/dsp/coefficients.rs` | `ddf240c6402eb929f17bd3bb55a2692cafdfd25a` | Both |
| `src/dsp/equalizer.rs` | `bebb5857ab483d975371ed19918161a4a52c271c` | Both |
| `src/dsp/filters.rs` | `65c04fce45e7e1cbf931495797874944f71036f8` | Both |
| `src/dsp/patch.rs` | `196f7875320a9524c2cf4181e262084636ad46ac` | Both |
| `src/dsp/pickup.rs` | `1cc0a8d0b45c72f7bd4e9b1549aa34702d96cd5e` | Both |
| `src/dsp/resonance.rs` | `d2d14539824e3ece7e664873e7d297a852076a06` | Both |
| `src/dsp/simple.rs` | `f14b936ee275a3b8aae07ba1a799329c1befd461` | Both |
| `src/dsp/tables.rs` | `5fcb7e87a260c43ceb318d207bff7a53d3e6d401` | Both |
| `src/dsp/variant.rs` | `e445210df65002992df80a4c16a746434ede1522` | Both |
| `src/host.rs` | `ec26248df707203a129e586a23667b70f1cf37d7` | Both |
| `src/host/embedded_models.rs` | `5b9db86b285180c3c136efdd2b8577c4ad15d47c` | Both |
| `src/host/engine.rs` | `1863feb29944b1643af042fa57f41e26f86f43a9` | Both |

### Device-library and USB wrapper sources

| Repository path | Git blob ID | Effective selection |
| --- | --- | --- |
| `crates/line6-devices/src/backup.rs` | `dd29e5192f147dfd7d1f72526e564cb22a7f34c7` | Both |
| `crates/line6-devices/src/device_lock.rs` | `6ccc0264804e59c142e94021f57fd670c5ce3929` | Both |
| `crates/line6-devices/src/family/mod.rs` | `65b6069477287fa6323da42fbe96f74cdfe754b6` | Both |
| `crates/line6-devices/src/family/podxt/backup.rs` | `b2643d4f7a86bfcec2ee9bb637dd5177d19b492e` | Both, branched · re-exports platform snapshot adapters |
| `crates/line6-devices/src/family/podxt/backup/snapshot.rs` | `0e19550068713a35f1b0458c377bd7f274698aa4` | Both, branched · selects snapshot adapters |
| `crates/line6-devices/src/family/podxt/backup/snapshot/fresh.rs` | `76f10e3b26b0135706bca01dc7c5ad146b67f573` | Both · fresh capture, outside Rig Check 2.5 |
| `crates/line6-devices/src/family/podxt/backup/snapshot/linux_raw_midi.rs` | `0c6a77a9c1858e37e92c9857a3d6957afb7ef966` | Linux only · fresh capture, outside 2.5 |
| `crates/line6-devices/src/family/podxt/backup/snapshot/macos_usb.rs` | `e2ba896feeed0d8b383f4a89ede5fd75e09e701e` | Mac only · fresh capture, outside 2.5 |
| `crates/line6-devices/src/family/podxt/backup/snapshot/provenance.rs` | `c89bfc7c1fbf605867c373bb14bb5f5cb5d69685` | Both |
| `crates/line6-devices/src/family/podxt/controls.rs` | `c2287005233c5dd0f95ab73290ebb8b7e32812ff` | Both |
| `crates/line6-devices/src/family/podxt/current_read.rs` | `20ae3d717f99e6bea1a50a7a70d8ed0412c52a9b` | Both |
| `crates/line6-devices/src/family/podxt/firmware_read.rs` | `da46527efa8dbb2296f4bfeaaea5fa845f2f71a6` | Both |
| `crates/line6-devices/src/family/podxt/live_edit.rs` | `8931a93ae291772d897d61b1377846e7b8f3b004` | Both |
| `crates/line6-devices/src/family/podxt/live_read.rs` | `2a8175a63df859249d4215f1ec298ca17b0a380d` | Both |
| `crates/line6-devices/src/family/podxt/live_settings.rs` | `7bb73bd40824d777124023796a39f7c754208122` | Both |
| `crates/line6-devices/src/family/podxt/mod.rs` | `144d45de3ca151b90d35b9194755a705ddb82a9e` | Both |
| `crates/line6-devices/src/family/podxt/models.rs` | `2fedb35fd1dd6131325f98981649f52b8a7bd990` | Both |
| `crates/line6-devices/src/family/podxt/procedures.rs` | `1dc5d607cf01b03403c6d2d52a6f97d69e7cd521` | Both |
| `crates/line6-devices/src/family/podxt/store.rs` | `0d52850d685845a587c993d858e8a170ff777a22` | Both |
| `crates/line6-devices/src/family/podxt/store_backup.rs` | `aa7e98dbcf16d651a47b46d76ff7899f9b7c1d04` | Both |
| `crates/line6-devices/src/family/podxt/transfer.rs` | `1e0c614293c27599239de6b3908348964aec353a` | Both |
| `crates/line6-devices/src/family/podxt/transport.rs` | `b789e0356d8df383d0a038c22cc8f81f35afcb9c` | Both |
| `crates/line6-devices/src/family/variax.rs` | `1eb7d5008dfb0a4171d6be748933c6975c346880` | Both |
| `crates/line6-devices/src/family/variax/backup.rs` | `45aaeb1887df303985a68b21759dba5a866092da` | Both |
| `crates/line6-devices/src/family/variax/backup/provenance.rs` | `d7744ec1356fa78ccb386958b60f3fb39366551c` | Both |
| `crates/line6-devices/src/family/variax/backup/snapshot.rs` | `78b0c375d33fc51f5adcd1153e2cacce668d9457` | Both |
| `crates/line6-devices/src/family/variax/driver.rs` | `9dacc1f815f5f470b2f15cd1181b98438b49fc63` | Both, branched · binding predicates and Linux-only verification |
| `crates/line6-devices/src/family/variax/firmware_read.rs` | `56f0a46443e1933306fe85cd7290df7f179583f5` | Both |
| `crates/line6-devices/src/family/variax/firmware_read/replay.rs` | `4cc00a153339b8859ff14c3fe7a0cd9bdaa66651` | Both |
| `crates/line6-devices/src/family/variax/follow.rs` | `7f9be9e87fc709c1e9ff0489c338b9bb86952c9b` | Both |
| `crates/line6-devices/src/family/variax/procedures.rs` | `98a2bd23c7b06099dc3d95d80a051872d29b01a5` | Both |
| `crates/line6-devices/src/family/variax/procedures/save_identity.rs` | `8e3a7f962379018e7535cc04d8fb08e1481600a1` | Both |
| `crates/line6-devices/src/family/variax/slot_read.rs` | `b45a7163228b48252d8bea987032a0feb966e6cb` | Both |
| `crates/line6-devices/src/family/variax/span_read.rs` | `4db3dc0a1735df4bc0a796e6e534ebc2c065761b` | Both |
| `crates/line6-devices/src/family/variax/string_levels.rs` | `c348af11d9bc4165c87aaf027eab1f0bc472c780` | Both |
| `crates/line6-devices/src/family/variax/transport.rs` | `891bc000fa039702253efdf2128f931de672133c` | Both |
| `crates/line6-devices/src/family/variax/write.rs` | `e7ab452f52cf77304827dd15573ff6fcdb1bf795` | Both |
| `crates/line6-devices/src/gate.rs` | `8e93134d39a3bfdc60696d361065bbc52f209dd6` | Both |
| `crates/line6-devices/src/identity.rs` | `082a0248ef9eef42890b53891e2da5e180cbbbe1` | Both |
| `crates/line6-devices/src/lib.rs` | `93118a7afadb3f6caefaa47e3df2090e13173f1e` | Both |
| `crates/line6-devices/src/midi.rs` | `6dc483966355c6fd1e46cd32aacb51456c6d3ada` | Both |
| `crates/line6-devices/src/session.rs` | `0d2848a080fa3855b59c96c6b088b2aa5f386002` | Both |
| `crates/line6-devices/src/transport/mod.rs` | `c4854e6ac80f94888aefc49052b1dc35e7c4859e` | Both, branched · Linux raw MIDI, shared USB selected by feature |
| `crates/line6-devices/src/transport/rawmidi.rs` | `3314520db081e4eda2f098346246fe663be2805d` | Linux only · active PODxt Rig Check link |
| `crates/line6-devices/src/transport/rawmidi/attachment.rs` | `54a17e5012f404310f59bc4114077f126589ce28` | Linux only · attachment-bound raw MIDI API, outside Rig Check |
| `crates/line6-devices/src/transport/rawmidi/attachment/inventory.rs` | `6b1dfe2622aa047193146140cd1296ce74eccccf` | Linux only · attachment-bound raw MIDI API, outside Rig Check |
| `crates/line6-devices/src/transport/rawmidi/attachment/native.rs` | `72a77b710efd7f65c053b13a33775615001d1f93` | Linux only · attachment-bound raw MIDI API, outside Rig Check |
| `crates/line6-devices/src/transport/usb.rs` | `f6c29b609f1e1121e28bd0414b5a8f0e2e59ca37` | Both, branched · USB transport with Mac ownership and Linux return policy |
| `crates/line6-devices/src/transport/usb/macos.rs` | `ec3d3f8101f40e4891c6e65b25a7a9a1c41f0103` | Mac only · native claim policy in production |
| `crates/line6-devices/src/transport/usb/macos_inventory.rs` | `3ce589372632b2653d50e10da7dc99685d23add4` | Mac only · registry selection in production |
| `crates/line6-devices/src/transport/usb/podxt.rs` | `dedca5ebc8881e7342b4256596369e22c00114ea` | Both, branched · Mac interface 0 active; separate Linux direct path closed |
| `crates/macos-usb-inventory/src/lib.rs` | `44707a8ea33e1513bc25c9ad300e75f9548984e8` | Mac only · passive IOKit and CoreFoundation FFI |
| `third_party/usb/libusb1-sys-0.7.0/src/constants.rs` | `a1d95518e1b7d11f1b888e85dd2fa2ddba74b38e` | Both |
| `third_party/usb/libusb1-sys-0.7.0/src/lib.rs` | `b0ac87db2c61253bac6577cd974d675ca6360d4b` | Both |
| `third_party/usb/rusb-0.9.4/src/config_descriptor.rs` | `cf322783a1e6b2e45d370a1a5711491915cac11b` | Both |
| `third_party/usb/rusb-0.9.4/src/context.rs` | `2add6b34fab6742cd6137ccf0b1871b74396e7e7` | Both |
| `third_party/usb/rusb-0.9.4/src/device.rs` | `af0042d82fb9b5d0c9bcd49d192bab24237a4c41` | Both |
| `third_party/usb/rusb-0.9.4/src/device_descriptor.rs` | `70c4bedf50eab59d7b76e8cb9da5a28c7d7188a8` | Both |
| `third_party/usb/rusb-0.9.4/src/device_handle.rs` | `185dfa9d3c1ef1cf13fe080a989657476ce76b71` | Both |
| `third_party/usb/rusb-0.9.4/src/device_list.rs` | `3873c9476fcfdd5237ed068edd8a08662990c14e` | Both |
| `third_party/usb/rusb-0.9.4/src/endpoint_descriptor.rs` | `945655af398295ab3f57e55e12765a39ae629c56` | Both |
| `third_party/usb/rusb-0.9.4/src/error.rs` | `199b48332e2d06c508e129e676420eb079e578d9` | Both |
| `third_party/usb/rusb-0.9.4/src/fields.rs` | `2ead8912a918e27f07ab25aa6c966e28fe5fbb91` | Both |
| `third_party/usb/rusb-0.9.4/src/hotplug.rs` | `e067ffe100abab7db646e1663e565f2337d1d99b` | Both |
| `third_party/usb/rusb-0.9.4/src/interface_descriptor.rs` | `3519599b625fbfd396e5e896abcc22a5d3513420` | Both |
| `third_party/usb/rusb-0.9.4/src/language.rs` | `4fec0d1640949dbb5d9f990744407704aa4c2bb9` | Both |
| `third_party/usb/rusb-0.9.4/src/lib.rs` | `4c1627702a984212bb578c30c95561da78a641ba` | Both |
| `third_party/usb/rusb-0.9.4/src/options.rs` | `c527bd823f2628c94460cbeb71703b29b7b717b1` | Both |
| `third_party/usb/rusb-0.9.4/src/version.rs` | `fb624e6c9dbec5ecb72181749e4059c621d9f1d8` | Both |

### Build selection and native USB inputs

| Repository path | Git blob ID | Effective selection |
| --- | --- | --- |
| `Cargo.toml` | `78b48fea0b0f760efd6177eae536721074f3db58` | Both · bin, features and target dependencies |
| `Cargo.lock` | `debf034119d16bb58bdc944aee3ce259372a0535` | Both · resolved Rust dependency versions |
| `.cargo/config.toml` | `aa8202b4893bb783e46d70fcea1eaa9fc58b0683` | Both · development/debugging profiles; release selected by recipe |
| `crates/line6-devices/Cargo.toml` | `b2c7a850877a210dbef6d78dd99f537804df750b` | Both · `usb`, Unix libc, Mac-specific USB crate |
| `crates/macos-usb-inventory/Cargo.toml` | `fa0cf7aab2950cf9c8d6629d50b51ed3e1647777` | Mac only · optional via device crate `usb` |
| `third_party/usb/rusb-0.9.4/Cargo.toml` | `57c7033e73ddab933925c8821c6d82dc02e0a551` | Both · Mac activates `vendored` |
| `third_party/usb/rusb-0.9.4/build.rs` | `9d65603a7951adff62d1ec75c6442ff85d335912` | Both · Rust USB wrapper build script |
| `third_party/usb/libusb1-sys-0.7.0/Cargo.toml` | `67ecedb7d614e9a9e50cfa4df8166efd99f85a89` | Both · FFI and host build dependencies |
| `third_party/usb/libusb1-sys-0.7.0/build.rs` | `bd1f43ba82e61db535f468e430c75e1f8f00f1e1` | Both, branched · pkg-config system vs `vendored` archive |
| `packaging/macos/build.sh` | `e27fea00eea8d380c51fe2f1715d3cc14ffa3fe9` | Mac recipe · native slices, bundle, signing and DMG |
| `packaging/macos/package.py` | `b33ef3ba333b2af5f34111cbe4f060f81caace88` | Mac recipe · bundle metadata, assets and linkage checks |
| `packaging/rig-check/build-linux.sh` | `ae8e6d34d152fe7654bcc7de6a841a1f98997c61` | Historical Linux kit recipe · defaults differ from pair |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/core.c` | `ffe33b7758c4717397ed56d23b2ae68391e044a7` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/descriptor.c` | `4623ad1e35636c1abe84ba92c7af171445eb82ff` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/hotplug.c` | `3c64f69197b347244a0bedaa978dbe41cd0f41ca` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/io.c` | `ab84ba620f99f04bec674fb205849bd9aef0386b` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/strerror.c` | `eb355034b198fa04178456f9146999b34b70419c` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/sync.c` | `146cce23c285858333ddd2806c4c6311e7a23637` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/libusb.h` | `f4e9203c641a123f39219ad30964f8c4e6e0c71b` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c` | `5d604e39918cc814f62477d2f56b02d76f60e76a` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.h` | `86646bfee1a5b00313fc9061ca8fa3bc521f78cc` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/events_posix.c` | `4056dae2e2a4c9e0fbbe6ce0b01ee8643244798e` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/threads_posix.c` | `0079fd598a516ae284e717c86939d06457018d0e` | Mac package static archive input · Linux package uses system library |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/linux_netlink.c` | `899084f222711db823cc0362a137360a42377354` | Linux vendor fallback only · absent from delivered ELF source inputs |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/linux_usbfs.c` | `ed8597ba910ce646ceaf92d3998b9348569f8958` | Linux vendor fallback only · absent from delivered ELF source inputs |
| `third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/linux_udev.c` | `9ec9eb17d1bc577bc42ec6d11cb1b28f7db74511` | Optional native fallback if pkg-config finds libudev · no evidence it was selected for either package |
| `packaging/build-owner-pair.sh` | `c50b03b746ea80647408d4f02ed616edd73eabae` | Both · later delivery recipe at `2d25c4c` |

### Embedded data and packaging inputs

| Repository path | Git blob ID | Effective selection |
| --- | --- | --- |
| `assets/variax-models/legacy.vxm.zlib` | `34f1a9f2c7921e5e6887a1f2762062d4e48fc014` | Both · embedded model data via `host/embedded_models.rs` |
| `tests/fixtures/native-dsp-body-inputs.bin` | `5834d09c2d0c8d32a15fdf03c4c86f5311c5cb7c` | Both · embedded step 0.3 body fixtures |
| `tests/fixtures/native-host-pickups.bin` | `808d1ddf02cfbad66a23d72931c4b49c011e5639` | Both · embedded step 0.3 pickup fixtures |
| `tests/fixtures/native-host-inputs.bin` | `393fe7435e76f1efb81d7c746720f0c64abfb760` | Both · embedded step 0.3 inputs |
| `tests/fixtures/native-host-outputs.bin` | `b0ccfdbd246e3e3c8a9183c78209e3868ed927b4` | Both · embedded step 0.3 expected outputs |
| `tests/fixtures/synthetic-host-preset.vwm` | `d538c816fcd37bd14e53c457f75ae9a735572f3c` | Both · embedded step 0.3 preset |
| `tests/fixtures/synthetic-host-preset.packed` | `0a8c1da333017182fd8cc8ba825795ae8ff0995b` | Both · embedded step 0.3 expected preset program |
| `packaging/macos/background.png` | `00610be3e99a5f71d6c9fbb835c2c6456e41fed1` | Mac DMG background input |
| `packaging/macos/rig-check.png` | `d468c2700219a019750f6eea2ccff95feca13603` | Mac Rig Check icon source |
| `packaging/macos/workbench.png` | `923c9313f11fc1c92a5979770ed3231882e84fd9` | Mac Workbench icon source |
| `packaging/macos/How to open.txt` | `aa49c4a9f1cdbcdd047c4a9e678303d345bd14d5` | Mac DMG instructions |
| `docs/rig-check/catalogue.md` | `f886e51f12d0c9fd93804f599397bdbd924d8318` | Linux paired loose file; catalogue consistency checked in tests |
| `docs/rig-check/coverage.md` | `b8acb241395aabd0ba653f2ffbe1c1184caa0b03` | Linux paired loose file |
| `docs/rig-check/release-notes.md` | `713a90ac1da0ca8248ceb9130155f968953ef577` | Linux paired loose file |

`gui` selects `eframe` 0.33.3 (Glow) and `winit` 0.30.13, with
platform-specific windowing underneath. On Linux the enabled Wayland/X11
backends and `rfd` provide the windowing and file-picker paths. On Mac the
native windowing dependencies back AppKit, and the manifest omits `rfd`.
`usb` selects both the app's `rusb` and `line6-devices/usb`. Both use the
repository's `rusb` 0.9.4 and `libusb1-sys` 0.7.0 Rust sources. Mac
enables `vendored` and the optional `macos-usb-inventory` crate. Linux
uses the system libusb in this package and has the device crate's Unix
`libc` for ALSA file I/O and locks. Unix lock code also compiles on Mac.
`portable-math` enables `libm` 0.2.16 with `force-soft-floats` on both,
even though the Linux ELF also lists the system math DSO. That linkage does
not select the coefficient profile. The Mac Cargo artifact logs record both
bin targets with `gui,podxt-table,portable-math,usb` and release optimization
level 3. Neither owner feature set enables `linux-alsa-native` or
`owner-reso5-test`. [App manifest][manifest] · [Device manifest][device-manifest] ·
[USB build selector][usb-build] · [Math profile][math]

The paired Linux directory contains two ELF binaries, separate catalogue,
coverage and release-note files, and verification metadata. It has no Mac
bundle resources or historical shell launcher. The Mac DMG contains two
universal apps, opening instructions and DMG presentation assets. The recipe
generates icons and plists, verifies ad-hoc signatures, and stages the ten
tracked UI PNGs with their manifest under **Workbench.app** Resources.
Rig Check carries its own app icon and its embedded engine inputs. The
compressed model bundle and step 0.3 reference data are compile-time data
in the relevant binary, with no requirement for separately installed vendor
files. Raw proprietary sources and owner materials are absent from the
source and package inputs used here. [Mac recipe][mac-build] ·
[Mac package verifier][mac-package] · [UI asset provenance][art] ·
[Model provenance][models]

[pair]: https://github.com/guirossibrum/VariaxWorkbench/blob/2d25c4c2dc72e145d9dd5bb502f275c58ca9193d/packaging/build-owner-pair.sh#L99
[mac-build]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/packaging/macos/build.sh#L7
[mac-package]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/packaging/macos/package.py#L15
[linux-kit]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/packaging/rig-check/build-linux.sh#L33
[manifest]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/Cargo.toml#L43
[device-manifest]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/Cargo.toml#L8
[usb-build]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/third_party/usb/libusb1-sys-0.7.0/build.rs#L204
[math]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/math.rs#L6
[engine]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/engine_check.rs#L273
[entry]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/bin.rs#L86
[mac-app]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/app/detection/identity.rs#L251
[app-usb]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/device_probe.rs#L1187
[catalogue]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/catalogue.rs#L355
[steps]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/steps.rs#L157
[runner]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/runner.rs#L323
[approvals]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/approvals.rs#L101
[linux-inv]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/computer/linux.rs#L43
[mac-inv]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/computer/other.rs#L345
[connection]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/connection.rs#L42
[pod-reads]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/podxt_reads.rs#L28
[guitar-reads]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/variax_reads.rs#L45
[guided]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/guided.rs
[live]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/live.rs
[guitar-live]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/variax_live.rs
[recover]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/recover.rs#L292
[audio]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/audio.rs#L108
[port]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/port.rs#L304
[raw]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/transport/rawmidi.rs#L29
[usb]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/transport/usb.rs#L103
[pod-usb]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/transport/usb/podxt.rs#L9
[guitar-driver]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/family/variax/driver.rs#L10
[mac-usb]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/transport/usb/macos.rs#L59
[mac-match]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/transport/usb/macos_inventory.rs#L121
[guitar-usb]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/family/variax/transport.rs#L13
[session]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/session.rs#L36
[procedures]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/procedures.rs#L133
[span]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/family/variax/span_read.rs
[guitar-save]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/variax_save.rs#L70
[pod-save]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/podxt_save.rs#L29
[save-id]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/family/variax/procedures/save_identity.rs
[record]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/record.rs#L275
[report]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/report.rs#L367
[wizard]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/gui/mod.rs#L1486
[dialog]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/macos_dialog.rs#L6
[resume]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/resume.rs
[resume-evidence]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/resume/evidence.rs#L75
[status]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/docs/status.md
[fake-usb]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/testing/direct_usb.rs#L429
[fake-save]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/testing/direct_usb/save_preflight.rs
[gui-tests]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/gui/tests.rs#L668
[resume-tests]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/src/rig_check/resume/tests.rs
[linux-capture]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/family/podxt/backup/snapshot/linux_raw_midi.rs
[mac-capture]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/crates/line6-devices/src/family/podxt/backup/snapshot/macos_usb.rs
[transport-research]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/docs/rig-check/transport-research.md
[art]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/assets/ui/SOURCES.md
[models]: https://github.com/guirossibrum/VariaxWorkbench/blob/99a62f37e9990fbcdf773162e0f9a6af17f9e4fc/assets/variax-models/SOURCES.md
[201]: https://github.com/guirossibrum/VariaxWorkbench/issues/201
[251]: https://github.com/guirossibrum/VariaxWorkbench/issues/251
[258]: https://github.com/guirossibrum/VariaxWorkbench/issues/258
[262]: https://github.com/guirossibrum/VariaxWorkbench/issues/262
[263]: https://github.com/guirossibrum/VariaxWorkbench/issues/263
[297]: https://github.com/guirossibrum/VariaxWorkbench/issues/297
[298]: https://github.com/guirossibrum/VariaxWorkbench/issues/298
[304]: https://github.com/guirossibrum/VariaxWorkbench/issues/304
[308]: https://github.com/guirossibrum/VariaxWorkbench/issues/308
