# Variax Workbench for Linux

A Rust replacement under development for the original Line 6 Variax 600,
connected by VDI through a PODxt Live. The current milestone is an **offline
preset editor and reverse-engineering toolkit**, with the original guitar and
pickup artwork recovered from Workbench 1.75.

## Final product target

One Rust application for **guitar + POD + USB recording + device maintenance**.
The first complete rig is Variax 600 through PODxt Live on Linux. Future backends
will bring Workbench HD/JTV-69, POD HD500X and other models into the same app,
with independently verified capabilities and device-specific implementations.
These are roadmap targets, not current support claims. See the
[product vision](docs/product-vision.md).

PODxt Live historically supports USB guitar recording, including processed/clean
routing, as documented in [Line 6's recording guide](https://line6.com/media/support/toneporthelp/GearBox2_OnlineSupport_Recording_Rev03.pdf).
Linux recording, playback and routing remain unimplemented/unverified here.

## Experimental software — use entirely at your own risk

This is an independent, reverse-engineered project developed with AI assistance.
It is not an official Line 6 product and is not affiliated with, endorsed by, or
supported by Line 6, Yamaha, or any equipment manufacturer. No technical support is provided, and no maintenance, recovery service, or
continued development is promised.

The software is provided **“as is”, without warranty of any kind**. Its output,
sound, timing, controls and other behavior may differ from the original software.
Errors, incomplete reverse engineering, AI-generated mistakes or incompatibilities
may cause lost presets or calibration data, unexpected or damaging audio output,
unusable devices, or **permanent damage to or complete destruction of connected
hardware**. Tests, backups and the current hardware locks do not guarantee safety
or recoverability. Anyone who downloads, modifies, builds or uses this project
does so entirely at their own risk and assumes responsibility for that decision.

**Contributors and testers are subject to the same risks during development.**
Use offline tests, saved data and simulations with the equipment disconnected
from the computer; keep the USB cable unplugged. Do not connect hardware merely
to try a change or because tests pass. Connecting a POD, Variax or other device,
including for identification or supposedly read-only testing, is entirely at
your own risk. A contribution, test result or project issue does not authorize
live device access or establish that a connection is safe.

**To the maximum extent permitted by applicable law, the project owner, authors
and contributors disclaim all warranties and accept no responsibility or
liability for any claim, loss, damage, injury, expense or other consequence arising
from this project or its use**, whether direct, indirect, incidental or
consequential, including equipment damage, loss of data or loss of use, even if
they have been advised that such consequences are possible.

**Hardware access is locked before device enumeration or opening.** The GUI,
Rust USB backend and Python probe have no runtime unlock. No preset uploads,
flash writes or firmware updates have been performed. Earlier identity/metadata
probes produced one current-preset settings capture, not a full guitar backup.

Read [the latest generator/firmware results](docs/offline-generator-results.md),
[the full report](docs/report.md) for changes, validation, risks and remaining
work. Use [GitHub issues](https://github.com/guirossibrum/VariaxWorkbench/issues)
and the [project](https://github.com/users/guirossibrum/projects/17) for work
tracking; [AGENTS.md](AGENTS.md) records the hardware boundary.

## Implementation and validation checklist

Keep this checklist current as issues progress. Checked items passed the stated
offline checks; they do not certify physical safety or hardware/audio equivalence.
The guitar uses piezo sensing; pickup controls are virtual models. [Variable coverage](docs/pickup-coverage.md) distinguishes tested samples from pending ranges and audio behavior.
Integration results and remaining risks: [offline runtime and HD research](docs/runtime-hd-integration.md).

Detailed evidence and limitations: [body interpolation](docs/body-definitions.md),
[pickup response](docs/pickup-response.md), [generator results](docs/offline-generator-results.md), and [backlog](docs/github-backlog.md).

### Verified and passing

- [x] Rust CLI and offline GUI build, including optimized release builds.
- [x] Short guided-check **offline rehearsal** app: five prompts, cancellation/skip handling and protected report saving. Run `cargo run --bin variax-check -- plan`; see [usage and limits](docs/manual-check.md).
- [x] **218 Rust tests per math profile** (host configuration and experimental software configuration), Clippy with warnings denied, and formatting checks on Linux.
- [x] **23 headless GUI tests** cover actual dirty-prompt/bank-row clicks, save cancellation/failure/retry, grouped slider drags, operation cancellation/error recovery, artwork recovery, document preservation and offline recording controls/file rejection and POD tone quit protection. Native OS dialogs/window behavior still need manual platform checks; see [GUI scope](docs/gui-validation.md).
- [x] Shared CLI/GUI edit validation for **34 parameters**, 28 body and 32 pickup choices; **84 original-Java endpoint/choice cases** and **1,792 body/pickup/slot selections** pass. Imported unknown/out-of-range values survive rendering/copying; see [domains and limits](docs/parameter-domains.md).
- [x] Individual preset parsing/editing and protected copy saves: **6 original-Java compatibility checks**.
- [x] L6BA/L6CO browsing, editing, whole-bank undo/redo and protected save-copy in the GUI; **three banks/five presets per format compared with original Java**, including resized replacement. [Bank scope and limits](docs/bank-editor.md).
- [x] Offline GUI with recovered artwork, preset controls, undo/redo and guarded document operations; selected headless tests and rendering smoke checks pass.
- [x] All **43 pickup definitions** and **28 body definitions** verified against original native serialization.
- [x] Complete typed Rust body decoding and **748 bit-exact pickup-normalization comparisons**.
- [x] Rust body interpolation routing: **600 native routing-plan comparisons**.
- [x] Actual Rust body-field interpolation: **252 vendor-body outputs + 84 synthetic outputs** match every serialized field in debug and release under the documented host-libm oracle.
- [x] Initialized original-library reference harness: **26 startup routines, 52 complex constants, 110 generator cases and four JNI mapping comparisons** pass with disclosed host-libm adapters. This is reference execution, not full Rust or audio parity; [scope and evidence](docs/native-runtime.md).
- [x] Offline math sensitivity audit: **206,228 distinct math inputs** agree between MPFR-256/512 after target-width rounding; glibc differs by up to one ULP on some inputs. MPFR substitution changes four coefficient words in one of 110 packed cases. A separately recompiled Apple source candidate is not an authenticated Mac runtime. See [math runtime evidence and limits](docs/math-runtime-audit.md).
- [x] Historical Apple 88.3.9 math candidate: 206,228 observed tuples and 110 whole-generator cases complete offline. Both guest and current-CPU x87 candidates match 109/110 recorded glibc outputs and 110/110 existing MPFR candidates. Fresh Linux Rust debug/release portable-profile binaries match all 110 saved Apple candidates; the host profile retains its 110 glibc references. Emulator x87 precision and complete historical-runtime parity remain limited; [source, reproduction and boundaries](docs/apple-tiger-math.md). Default math is unchanged.
- [x] Original embedded Windows math instructions: **206,228 inputs in each of three explicit CPU/precision modes** complete offline. Initialized SSE math substitution into the Mac generator matches all 110 MPFR-256 packed candidates. This isolates math effects; it is not full Windows generator/JVM parity.
- [x] Experimental `portable-math` build: pinned Rust algorithms, **44 centralized math call sites**, **110 complete numeric patches**, **48 synthetic-resource cases**, **828 raw component cases**, and **144 complete Java/native preset-file cases** match corresponding original-instruction software-math references in debug/release. Software CLI binaries import none of the tested host math functions. The default profile remains host math; [profile contract, dependency and limits](docs/math-profiles.md).
- [x] Fresh offline research setup: digest-checked archive extraction, isolated hash-locked Python environment, 28-body/43-pickup catalog, eight native slices, derived procedure metadata and an ordered 39-stage reference recipe. Nine synthetic setup guards pass on Linux/Windows/macOS. A fresh uninterrupted replay passed all 39 stages without fixture changes. Replay evidence and retained bounded-run failures are [documented](docs/reproducing-references.md); the Linux research environment is separate from application dependencies.
- [x] Linux offline package builder, explicit digest manifest, desktop/MIME integration and protected install/removal. Both math-profile packages passed staged checks, all four tools on synthetic inputs and native X11 welcome/preset captures; eight package guards pass. Vendor resources are excluded; [build/install and limits](docs/linux-package.md) — [#11](https://github.com/guirossibrum/VariaxWorkbench/issues/11).
- [x] Software-profile core/file CI: **194 tests in debug and release per target** on Linux x86-64, Windows x86-64 and macOS ARM, with Clippy clean. Cross-target checks found and fixed Windows CLI stack exhaustion and ARM NaN-sign differences; GUI/USB integration is outside this CI scope.
- [x] Raw Rust pickup circuit combination: **3,826 bit-exact comparisons** with the native routine.
- [x] Pickup electronics: **2,322 raw responses + 387 primary fits + 282 secondary fits**, **900 biquad checks**, **280 complete tables** and **252 smoothed-response comparisons**. Synthetic fixtures pass debug/release; host integration is covered below.
- [x] Rust DSP packing/checksum, selected resonator conversions, **1,012 VX encodings and 315 scalar-setter cases** match original-machine-code fixtures. **2,145 coefficient-writer cases**, **1,320 complete pickup block cases** and **1,008 simple-block cases** pass. A further **600 biquad checks and 864 filter-block cases** pass. All **17 DSP block paths** now have parameter-to-word implementations: a further **2,304 EQ coefficient cases**, **432 EQ block cases**, **288 sitar cases**, **432 body/string-tone cases**, and **216 complete raw-body assemblies** match the original instructions under the documented host-libm adapters. Scope is [documented](docs/dsp-implementation.md); the [observed word map](docs/dsp-word-map.md) distinguishes 748 written words from 343 still-unknown words.
- [x] Pickup geometry and electric level primitives: **261 pole-offset outputs, 120 projection traces, 129 bundled-model placements, 72 synthetic host cases and 216 level/trim cases** match the documented native paths — [#2](https://github.com/guirossibrum/VariaxWorkbench/issues/2).
- [x] Offline Rust preset renderer: **240 Java mapping/overlay cases**, **110 complete stateful native cases**, **144 complete Java/native final-byte comparisons** (including 24 special-combination cases), and **48 synthetic full-engine outputs** pass. Extra regressions cover **300 fractional EQ mixes**, **48 derived-frequency sitar cases**, state retention, atomic errors and protected file output. External model resources remain private. Historical libm/audio parity remains pending; see [host pipeline scope](docs/host-pipeline.md) — [#4](https://github.com/guirossibrum/VariaxWorkbench/issues/4).
- [x] Firmware inspection: both supplied images unpack with matching embedded MD5 and complete region coverage; **396 corrupt/truncated synthetic inputs rejected**.
- [x] Offline Rust FORM/L6FF inspector (`variax-firmware-inspect`): bounded decompression, chunk and stream termination checks, region coverage, MD5 integrity and SHA-256 reporting. Synthetic rejection covers the same **390 prefixes and 6 damaged fields**, plus wrong-product, wrong-version, oversized, overlapping, duplicate, and corrupted-image fixtures. MD5 agreement is not authenticity. No flashing or device API. Details: [firmware containers](docs/firmware-container.md) — [#17](https://github.com/guirossibrum/VariaxWorkbench/issues/17).
- [x] Offline artifact provenance: product, version, size, MD5, SHA-256, and a source URL only where one was captured. Variax 300/600 3.10 size and MD5 agree with Central release 1062. That agreement is not a signature. Details: [artifact provenance](docs/artifact-provenance.md) — [#12](https://github.com/guirossibrum/VariaxWorkbench/issues/12).
- [x] Linux Variax matching/routing investigation: interface ownership, the model-4 identity mismatch, an uninstalled patch, independently rebuilt module text and bounded Rust interpretation of compiled identity/startup/reset paths. Sends, timers and ALSA stay stubbed; [scope and review](docs/linux-transport.md) — [#21](https://github.com/guirossibrum/VariaxWorkbench/issues/21).
- [x] Simulated read-session fragmentation, cancellation, timeout, disconnect and cleanup failures; physical-access locks remain tested and enabled.

### Pending implementation or validation

- [ ] Historical macOS math parity and full Rust/native validation remain pending. The old uninitialized whole-generator/JNI baseline stays invalidated; use only the versioned candidates described in [native runtime evidence](docs/native-runtime.md).

- [ ] Historical math-runtime parity, cross-target/default-rollout review of the experimental portable profile, and broader complete Rust/native validation — [#5](https://github.com/guirossibrum/VariaxWorkbench/issues/5), [#6](https://github.com/guirossibrum/VariaxWorkbench/issues/6). The synthetic-only core now passes Windows/macOS/Linux CI; other targets and full application integration remain pending.
- [x] Static firmware baseline map: Variax DSP56300-compatible load records, two boot-region streams and template/slot relationships; POD SHARC-compatible blocks/vectors, additive checksums and 486 named resource records. Exact ICs, opaque payloads, calibration and update/recovery behavior remain unverified. Ten synthetic layout guards; no firmware execution. Details: [firmware map](docs/firmware-map.md) — [#13](https://github.com/guirossibrum/VariaxWorkbench/issues/13).
- [x] Exact archived Variax 3.10 reference extraction: 50 compact templates, 60 full raw slots and 60 settings files; every program checksum validates and all 60 settings agree with the original Java decoder. Outputs remain private, save only to a new directory and are not calibration/current-unit backups. Details: [factory extraction](docs/factory-extraction.md).
- [x] Saved Reso-5 metadata matches the firmware 3.10 default exactly: 6 models, 34 parameter type/value pairs, and identical settings-file bytes. Repeatable file-only comparison rejects other models/versions and preserves raw-bit distinctions. Earlier shipping state and the other stored presets remain unverified. Details: [factory comparison](docs/factory-comparison.md) — [#15](https://github.com/guirossibrum/VariaxWorkbench/issues/15).
- [ ] Additional calibration completeness/preservation, full-bank backup and physical restoration safety — [#16](https://github.com/guirossibrum/VariaxWorkbench/issues/16), [#23](https://github.com/guirossibrum/VariaxWorkbench/issues/23).
- [x] Bounded updater research oracle: 46 synthetic cases exercise six original host methods, two applicability helpers and three callbacks in private emulated memory. Code/preset/boot/POD encodings, checksums and selected timeout/error paths are corroborated with fake transports. No native DLL load, DLL entry point, device firmware execution or hardware access. This does not prove preservation or recovery safety. Details: [update research](docs/update-recovery.md).
- [x] Bounded static update/recovery map for the initial rig: product dispatch, transfer stages, integrity checks, native keep-preset flag path, documented POD safe-mode entry and explicit ROM/flash/calibration unknowns. This is research, not an updater or proven recovery path — [#16](https://github.com/guirossibrum/VariaxWorkbench/issues/16).
- [x] Bounded in-memory Variax receiver stages and virtual POD/guitar metadata sessions, independent updater host-reference hashes, corruption/interruption tests and explicit unsupported recovery are implemented. See [simulation scope](docs/receiver-simulation.md) — [#18](https://github.com/guirossibrum/VariaxWorkbench/issues/18); physical recovery and backup readiness remain unverified under [#22](https://github.com/guirossibrum/VariaxWorkbench/issues/22)–[#25](https://github.com/guirossibrum/VariaxWorkbench/issues/25). Rust container inspection is [#17](https://github.com/guirossibrum/VariaxWorkbench/issues/17).
- [ ] PODxt Live USB-component artifact and installed version remain unmet. A macOS Monkey userspace builder shape was recorded and was not sent. USB PID `0x4650` constructs an `L6XTAudioControlMac` and, when control slot `+0x18` succeeds, virtually calls description slot `+0x0c` with that control. No symbol stub binds the setter. A later `AcquireService(0)` miss stores an `L6KernelBrandiServiceMac`, and the base acquire call returns that cache slot. Kernel `GetVersion` calls `IOConnectCallScalarMethod` selector 6. Sharedxt selector 6 builds the same five fields in a device request and forwards that pointer. That handler is not `GetIOPVersion` and does not identify a firmware image. The forwarded callee at slot `+0x978` is not in the local archive, so the three returned bytes stay uninterpreted. That is a static branch. The five fields stay unapproved. The partial record is [PODxt Live USB firmware](docs/pod-usb-firmware.md). The installed USB version is unknown — [#20](https://github.com/guirossibrum/VariaxWorkbench/issues/20).
- [x] Offline firmware-improvement feasibility assessment: specific host/firmware candidates, architecture and integrity constraints, calibration/preservation and recovery gaps. No deployable patch was generated; device changes are not ready. See [assessment](docs/firmware-feasibility.md) — [#19](https://github.com/guirossibrum/VariaxWorkbench/issues/19).
- [ ] Late-stage synthetic test expansion to **at least 5x distinct samples**, including body-dependent sustain/note decay, continuous string bends/vibrato, bends during decay and alternate/custom tuning in combination — [#33](https://github.com/guirossibrum/VariaxWorkbench/issues/33). Audio behavior stays unverified without a suitable independent DSP/audio reference.
- [ ] Readiness review, comprehensive backups and any future authorized hardware integration/write design — [#22](https://github.com/guirossibrum/VariaxWorkbench/issues/22)–[#25](https://github.com/guirossibrum/VariaxWorkbench/issues/25).

- [x] Shared session, static registry, and capability matrix for one PODxt Live USB host with Variax 600 and PODxt Live roles. Guitar file open/save and per-family FORM/L6FF inspection are offline. PODxt Live tone open/rename is the offline librarian in #29. Physical enumeration, audio, MIDI, flash, reset, and driver actions stay locked. USB-component, installed-unit, and HD/JTV-69/POD HD500X versions stay unknown. Details: [unified app](docs/unified-app.md) — [#26](https://github.com/guirossibrum/VariaxWorkbench/issues/26).
- [x] #61 static package evidence, checked scopes v1–v5: **5 supplied originals, 10/10 inventoried copies matched** the supplied sizes and SHA-256 references; **30 paired Windows/macOS Central JSON resources** parsed to equal objects. Workbench HD 2.15's paired `FactoryPresets.bun` copies match by SHA-256; an offline bounded parser counted 12 banks × 5 presets, 12-byte `PINF` records, and eight typed parameter groups in each of 60 presets. Static macOS code confirms the `WB2P`/`PINF`/12-byte `PARM` contract and parameter index/type checks. A bounded Windows/macOS static comparison confirms the standalone `LIST/TUNE` serializer's six 8-byte `STRG` records: slots 0–5 map to `ALTT` indices 1–6 and float-typed values. Duplicate slots are last-wins; unknown children and malformed sizes fail; the macOS reader skips out-of-range slots while Windows fails. Six reproducible synthetic CPU-reference checks passed. Static UI wiring labels parameter indices 1–6 as String 1 (high E) through String 6 (Low E); this is not a physical routing observation. Controls declare −12..+12, while stored float units/range remain unproven. The standalone Windows x86 `L6SM.sys` PnP path also now has a static bounded removal trace: minor `0x02` conditionally detaches its saved lower device and deletes symbolic-link/device objects based on context flags. The matching standalone macOS driver is absent from the supplied set; device identity, interface/framing, complete disconnect cleanup and recovery remain unknown. The Variax USB driver INF identifies standalone bridge VID/PID `0x0E41:0x534D`; Workbench HD's manual describes VDI editing and names HD Pro/HD 500, X3 Live/Pro, and Vetta II hosts. Central's Variax rows are update compatibility metadata, not editor support. No device operation was performed; #61 remains partial. See [cross-product package evidence](docs/line6-cross-product-evidence.md).
- [x] PODxt Live USB audio routing: offline Record Send 1-2 planning and synthetic S24_3LE buffer checks are in [POD USB audio](docs/pod-usb-audio.md). GearBox modes are guide text. The PCM window is Linux `v7.2.15` source text, with the same integers present in the installed `snd-usb-pod` module file. No stream was opened — [#27](https://github.com/guirossibrum/VariaxWorkbench/issues/27).
- [x] Offline test recording in the same app: explicit synthetic or WAV input, Record Send route selection, integer peak meters, start/feed/stop/cancel, bounded chunks, and non-overwriting WAV plus sidecar saves. The source clock 78125/2 is kept and is not written as an integer WAV rate. Device capture stays locked. No DAW was opened. Explicit exact-clock 24-bit AIFF export preserves the rational and source PCM; eight files were independently decoded with SoX_ng. Other readers may round the rate; DAW imports are unverified. See [AIFF scope](docs/exact-clock-aiff.md) — [#69](https://github.com/guirossibrum/VariaxWorkbench/issues/69). Details: [recording files](docs/recording.md) — [#28](https://github.com/guirossibrum/VariaxWorkbench/issues/28).
- [x] Offline PODxt Live tone librarian: FORM/L6PA, FORM/L6FS, and FORM/L6AS files with PINF target `0x0003000A`, amp slot `0x00030000`, and cab slot `0x00030001` on L6AS only. Asset names are the strings both archived `GetAssetName` copies return for this device. A 12-byte MINF shows the signed Java ordinal and the `Read8() > 0` enabled flag. Reserved bytes stay raw. A 12-byte PARM shows its id, type, and raw bits. Unknown ids, other PARM lengths, and reserved bytes stay raw. No parameter unit or range was recovered, and values are not edited. Applied renames are tracked per tone, and quitting asks to save, discard, or keep editing before those edits are dropped. Nothing is sent. Details: [POD presets](docs/pod-presets.md) — [#29](https://github.com/guirossibrum/VariaxWorkbench/issues/29).
- [ ] Shared, device-specific firmware/backup/recovery/maintenance UI; real updates remain separately gated — [#30](https://github.com/guirossibrum/VariaxWorkbench/issues/30).
- [ ] Workbench HD/JTV-69 backends; other models added only with independent verification — [#31](https://github.com/guirossibrum/VariaxWorkbench/issues/31).
- [ ] POD HD500X preset, model, effect, firmware-image, and USB-audio routing recovery. Driver-text USB identity `0x0E41`/`0x4159` is recorded. Edit releases 9781 and 8559 answer HEAD with a login redirect, and HEAD of their release-id directories returns 404. No installer is local. A directory 404 can coexist with a public named object, and wider sources remain unknown. Two public 2013 manuals are hashed; their printed file suffixes disagree and are not a container. Independent H5EP and H5ES measurements are recorded, and their target bytes are not a registered product id. Details: [POD HD500X](docs/pod-hd500x.md) — [#32](https://github.com/guirossibrum/VariaxWorkbench/issues/32).

- [x] Unsaved offline recording takes are protected on quit alongside guitar/POD edits. Cancelled or failed saves retain audio; native-clock takes can be explicitly saved as exact-clock AIFF, kept or discarded. See [recording files](docs/recording.md) — [#72](https://github.com/guirossibrum/VariaxWorkbench/issues/72).

### Currently unavailable or not working

- [ ] The image-default Windows x87 math substitution fails the Mac generator's strict startup-constant check; its complete-generator replay stops. Historical Mac parity remains unresolved, and the optional portable profile is experimental.
- [ ] Guided check does not yet read switches/knobs, record audio or measure notes/bends/tuning. Future integration is [#34](https://github.com/guirossibrum/VariaxWorkbench/issues/34); the current app rehearses prompts only.
- [ ] **Live device access is intentionally disabled**, including identity/metadata reads. No runtime unlock exists.
- [ ] Sending edited presets to the guitar, firmware flashing and factory restoration are unavailable; no live writer is implemented or authorized.
- [ ] Live USB audio capture/playback is not implemented or validated; no audio stream has been opened. The offline test recorder does not open a device. See [recording files](docs/recording.md).
- [ ] PODxt Live parameter units, ranges, and value editing are not recovered. The archived Workbench jars name a Sharc editor that is not present, and the only `L6ParamDefinition` ranges belong to Variax. Firmware region-17 editing and sending a tone to a device are not implemented. See [POD presets](docs/pod-presets.md) — [#29](https://github.com/guirossibrum/VariaxWorkbench/issues/29).
- [ ] POD HD500X preset editing, firmware images, and live audio are not available. No preset container, model id, effect id, or firmware image was recovered. Printed manual suffixes are not a byte layout. Independent file bounds do not identify the product. The INF 48000 Hz / 24-bit values are driver defaults with `EngineFormatLock` 0. See [POD HD500X](docs/pod-hd500x.md) — [#32](https://github.com/guirossibrum/VariaxWorkbench/issues/32).
- [ ] Full PODxt Live/Variax firmware and audio/DSP emulation is not implemented. Existing host-library emulation is not device emulation.
- [ ] The earlier experimental full-DSP read timed out; no complete DSP/bank/calibration backup was obtained. The captured “Reso-5” file is one preset's settings only.
- [ ] Earlier Linux observations had no Variax ALSA port. The model-byte mismatch is diagnosed and an uninstalled patch is verified offline; live connectivity remains unverified.
- [ ] No complete authenticated Variax 600 factory bank or proven recovery path is available.
- [ ] The original Java reader rejects the tested odd-length unknown bank extensions with `EOFException`; Rust preserves them, but Java compatibility is unverified for such files. Entirely empty bank/collection GUI editing and bank/preset creation/deletion are unavailable. See [bank limits](docs/bank-editor.md).
- [ ] Native Wayland screenshot validation timed out inside Mesa buffer swap in this session; the Linux package launcher uses the tested X11 path. Wayland rendering and native Windows/macOS GUI checks remain unverified; see [package validation](docs/linux-package.md).
- [ ] Original-library reference tests and illustrated artwork need separately supplied private artifacts. The Linux package opens without them; Windows/macOS desktop packages are not implemented.

## Run the offline editor

```sh
cargo run --features gui --bin variax-workbench-gui -- --display x11
```

Open a file with **Open** or `--preset FILE`. Optional extracted artwork can be
selected with **Artwork folder** or `--artwork EDITOR_IMAGE_FOLDER`; see
[package paths](docs/linux-package.md). The editor supports
body/pickup selection and placement, tuning and electronics controls, undo/redo,
and saving a new copy. Originals and existing output files are protected.
A saved preset cannot yet be sent to the guitar. Offline host generation is implemented; historical-runtime, broader full-engine and audio validation remain pending.

The build's coefficient math profile can be inspected with `math-profile`.
The optional software profile and its offline checks are [documented](docs/math-profiles.md).

## Offline command line

```sh
cargo run --bin variax-workbench -- inspect INPUT.vwm
cargo run --bin variax-workbench -- rename INPUT.vwm COPY.vwm "My Guitar"
cargo run --bin variax-workbench -- set INPUT.vwm EDITED.vwm 02010001 200000 float 2.5
cargo run --bin variax-workbench -- inspect-bundle BANK.vwb
cargo run --bin variax-workbench -- extract-preset BANK.vwb EXTRACTED.vwm 0 0
```

Output paths must not already exist. `set` takes hexadecimal model-local IDs;
explicit edits use the [recovered parameter domains and body restrictions](docs/parameter-domains.md). Imported unknown and dormant values remain lossless.
`decode FIRST.syx SECOND.syx` decodes two captured metadata replies. `preview`
prints recovered request bytes without transmitting them.

## Verify

```sh
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
cargo run --bin variax-firmware-inspect -- --self-test
python scripts/verify_preset_compatibility.py
python scripts/verify_bundle_compatibility.py
.tools/python/bin/python scripts/verify_native_pack.py
.tools/python/bin/python scripts/verify_native_resonators.py
.tools/python/bin/python scripts/verify_native_pickups.py
.tools/python/bin/python scripts/verify_native_bodies.py
.tools/python/bin/python scripts/verify_body_rust.py
.tools/python/bin/python scripts/verify_body_plans.py
.tools/python/bin/python scripts/verify_body_interpolation.py
.tools/python/bin/python scripts/verify_pickup_combinations.py
.tools/python/bin/python scripts/verify_pickup_response.py
```

Reference checks require the locally extracted original application. REA was
built in `../rea`; `scripts/rea` selects the local Node/Java/Ghidra toolchain.
Original installers, proprietary extracts, artwork, captures and portable tools
remain local and ignored. No updater or installer was executed.

Details: [preset formats](docs/preset-files.md), [body definitions](docs/body-definitions.md),
[artwork](docs/ui-assets.md), [factory/firmware investigation](docs/factory-reference.md),
[initial investigation](docs/investigation.md), [native analysis](docs/native-analysis.md).

Pending work is tracked in [GitHub issues](https://github.com/guirossibrum/VariaxWorkbench/issues)
and the [dependency-linked backlog](docs/github-backlog.md). The default branch is `master`.

The [VariaxWorkbench project](https://github.com/users/guirossibrum/projects/17)
tracks execution order, phase and status, with native issue dependencies. Work
proceeds one issue at a time; hardware access stays locked.
