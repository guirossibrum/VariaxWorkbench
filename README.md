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
Detailed evidence and limitations: [body interpolation](docs/body-definitions.md),
[pickup response](docs/pickup-response.md), [generator results](docs/offline-generator-results.md), and [backlog](docs/github-backlog.md).

### Verified and passing

- [x] Rust CLI and offline GUI build, including optimized release builds.
- [x] Short guided-check **offline rehearsal** app: five prompts, cancellation/skip handling and protected report saving. Run `cargo run --bin variax-check -- plan`; see [usage and limits](docs/manual-check.md).
- [x] **131 Rust tests per math profile** (host configuration and experimental software configuration), Clippy with warnings denied, and formatting checks on Linux.
- [x] **15 headless GUI tests** cover actual dirty-prompt/bank-row clicks, save cancellation/failure/retry, grouped slider drags, operation cancellation/error recovery, artwork recovery and document preservation. Native OS dialogs/window behavior still need manual platform checks; see [GUI scope](docs/gui-validation.md).
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
- [x] Original embedded Windows math instructions: **206,228 inputs in each of three explicit CPU/precision modes** complete offline. Initialized SSE math substitution into the Mac generator matches all 110 MPFR-256 packed candidates. This isolates math effects; it is not full Windows generator/JVM parity.
- [x] Experimental `portable-math` build: pinned Rust algorithms, **44 centralized math call sites**, **110 complete numeric patches**, **48 synthetic-resource cases**, **828 raw component cases**, and **144 complete Java/native preset-file cases** match corresponding original-instruction software-math references in debug/release. Software CLI binaries import none of the tested host math functions. The default profile remains host math; [profile contract, dependency and limits](docs/math-profiles.md).
- [x] Fresh offline research setup: digest-checked archive extraction, isolated hash-locked Python environment, 28-body/43-pickup catalog, eight native slices, derived procedure metadata and an ordered 39-stage reference recipe. Nine synthetic setup guards pass on Linux/Windows/macOS. A fresh uninterrupted replay passed all 39 stages without fixture changes. Replay evidence and retained bounded-run failures are [documented](docs/reproducing-references.md); the Linux research environment is separate from application dependencies.
- [x] Linux offline package builder, explicit digest manifest, desktop/MIME integration and protected install/removal. Both math-profile packages passed staged checks, all four tools on synthetic inputs and native X11 welcome/preset captures; eight package guards pass. Vendor resources are excluded; [build/install and limits](docs/linux-package.md) — [#11](https://github.com/guirossibrum/VariaxWorkbench/issues/11).
- [x] Software-profile core/file CI: **115 tests in debug and release per target** on Linux x86-64, Windows x86-64 and macOS ARM, with Clippy clean. Cross-target checks found and fixed Windows CLI stack exhaustion and ARM NaN-sign differences; GUI/USB integration is outside this CI scope.
- [x] Raw Rust pickup circuit combination: **3,826 bit-exact comparisons** with the native routine.
- [x] Pickup electronics: **2,322 raw responses + 387 primary fits + 282 secondary fits**, **900 biquad checks**, **280 complete tables** and **252 smoothed-response comparisons**. Synthetic fixtures pass debug/release; host integration is covered below.
- [x] Rust DSP packing/checksum, selected resonator conversions, **1,012 VX encodings and 315 scalar-setter cases** match original-machine-code fixtures. **2,145 coefficient-writer cases**, **1,320 complete pickup block cases** and **1,008 simple-block cases** pass. A further **600 biquad checks and 864 filter-block cases** pass. All **17 DSP block paths** now have parameter-to-word implementations: a further **2,304 EQ coefficient cases**, **432 EQ block cases**, **288 sitar cases**, **432 body/string-tone cases**, and **216 complete raw-body assemblies** match the original instructions under the documented host-libm adapters. Scope is [documented](docs/dsp-implementation.md); the [observed word map](docs/dsp-word-map.md) distinguishes 748 written words from 343 still-unknown words.
- [x] Pickup geometry and electric level primitives: **261 pole-offset outputs, 120 projection traces, 129 bundled-model placements, 72 synthetic host cases and 216 level/trim cases** match the documented native paths — [#2](https://github.com/guirossibrum/VariaxWorkbench/issues/2).
- [x] Offline Rust preset renderer: **240 Java mapping/overlay cases**, **110 complete stateful native cases**, **144 complete Java/native final-byte comparisons** (including 24 special-combination cases), and **48 synthetic full-engine outputs** pass. Extra regressions cover **300 fractional EQ mixes**, **48 derived-frequency sitar cases**, state retention, atomic errors and protected file output. External model resources remain private. Historical libm/audio parity remains pending; see [host pipeline scope](docs/host-pipeline.md) — [#4](https://github.com/guirossibrum/VariaxWorkbench/issues/4).
- [x] Firmware inspection: both supplied images unpack with matching embedded MD5 and complete region coverage; **396 corrupt/truncated synthetic inputs rejected**.
- [x] Offline Rust FORM/L6FF inspector (`variax-firmware-inspect`): bounded decompression, chunk and stream termination checks, region coverage, MD5 integrity and SHA-256 reporting. Synthetic rejection covers the same **390 prefixes and 6 damaged fields**, plus wrong-product, wrong-version, oversized, overlapping, duplicate, and corrupted-image fixtures. MD5 agreement is not authenticity. No flashing or device API. Details: [firmware containers](docs/firmware-container.md) — [#17](https://github.com/guirossibrum/VariaxWorkbench/issues/17).
- [x] Offline artifact provenance: product, version, size, MD5, SHA-256, and a source URL only where one was captured. Variax 300/600 3.10 size and MD5 agree with Central release 1062. That agreement is not a signature. Details: [artifact provenance](docs/artifact-provenance.md) — [#12](https://github.com/guirossibrum/VariaxWorkbench/issues/12).
- [x] Simulated read-session fragmentation, cancellation, timeout, disconnect and cleanup failures; physical-access locks remain tested and enabled.

### Pending implementation or validation

- [ ] Historical macOS math parity and full Rust/native validation remain pending. The old uninitialized whole-generator/JNI baseline stays invalidated; use only the versioned candidates described in [native runtime evidence](docs/native-runtime.md).

- [ ] Historical math-runtime parity, cross-target/default-rollout review of the experimental portable profile, and broader complete Rust/native validation — [#5](https://github.com/guirossibrum/VariaxWorkbench/issues/5), [#6](https://github.com/guirossibrum/VariaxWorkbench/issues/6). The synthetic-only core now passes Windows/macOS/Linux CI; other targets and full application integration remain pending.
- [ ] Firmware architecture/region mapping, factory/calibration extraction and trustworthy factory comparison — [#13](https://github.com/guirossibrum/VariaxWorkbench/issues/13)–[#15](https://github.com/guirossibrum/VariaxWorkbench/issues/15).
- [ ] Static recovery/update reconstruction and virtual device/update simulation — [#16](https://github.com/guirossibrum/VariaxWorkbench/issues/16), [#18](https://github.com/guirossibrum/VariaxWorkbench/issues/18). Rust container inspection is [#17](https://github.com/guirossibrum/VariaxWorkbench/issues/17).
- [ ] PODxt Live USB-component artifact and installed version remain unmet. A macOS Monkey userspace builder shape was recorded and was not sent. USB PID `0x4650` constructs an `L6XTAudioControlMac` and, when control slot `+0x18` succeeds, virtually calls description slot `+0x0c` with that control. No symbol stub binds the setter. A later `AcquireService(0)` miss stores an `L6KernelBrandiServiceMac`, and the base acquire call returns that cache slot. Kernel `GetVersion` calls `IOConnectCallScalarMethod` selector 6. Sharedxt selector 6 builds the same five fields in a device request and forwards that pointer. That handler is not `GetIOPVersion` and does not identify a firmware image. The forwarded callee at slot `+0x978` is not in the local archive, so the three returned bytes stay uninterpreted. That is a static branch. The five fields stay unapproved. The partial record is [PODxt Live USB firmware](docs/pod-usb-firmware.md). The installed USB version is unknown — [#20](https://github.com/guirossibrum/VariaxWorkbench/issues/20).
- [ ] Firmware-improvement feasibility and the Linux Variax bridge/identity investigation — [#19](https://github.com/guirossibrum/VariaxWorkbench/issues/19), [#21](https://github.com/guirossibrum/VariaxWorkbench/issues/21). The installed variax module matches USB `0x4650` interface 1. Its identity bytes match MIDI product `0x00070000`, and startup progress advances to 1 only while that progress is still below 1. Saved product `0x00070004` and version `0x03100000` are derived scalars. An offline patch also accepts model `0x0004` and is not installed. A detached rebuild against the installed headers and the recovered Line 6 headers reproduced the installed module `.text`; with that patch applied, the same rebuild keeps the progress gate and accepts model bytes `0x00` and `0x04`. The rebuilt module was not installed. An offline interpreter runs the compiled Linux prefix at `[0x160, 0x240)` for every model-low value, the family and high bytes, and the progress boundaries. Those instruction bytes are GPL-2.0-only test reference data from the pinned `variax.c` commit. The buffers fed to them are synthetic. Scheduling calls stay stubbed, and the module is not executed. Notice: [third_party](third_party/README.md). Details: [Linux transport](docs/linux-transport.md).
- [ ] Late-stage synthetic test expansion to **at least 5x distinct samples**, including body-dependent sustain/note decay, continuous string bends/vibrato, bends during decay and alternate/custom tuning in combination — [#33](https://github.com/guirossibrum/VariaxWorkbench/issues/33). Audio behavior stays unverified without a suitable independent DSP/audio reference.
- [ ] Readiness review, comprehensive backups and any future authorized hardware integration/write design — [#22](https://github.com/guirossibrum/VariaxWorkbench/issues/22)–[#25](https://github.com/guirossibrum/VariaxWorkbench/issues/25).

- [ ] Shared multi-device application architecture and capability registry — [#26](https://github.com/guirossibrum/VariaxWorkbench/issues/26).
- [ ] PODxt Live USB audio routing, in-app recording and external DAW interoperability — [#27](https://github.com/guirossibrum/VariaxWorkbench/issues/27), [#28](https://github.com/guirossibrum/VariaxWorkbench/issues/28).
- [ ] PODxt Live preset/effect editing and librarian workflows — [#29](https://github.com/guirossibrum/VariaxWorkbench/issues/29).
- [ ] Shared, device-specific firmware/backup/recovery/maintenance UI; real updates remain separately gated — [#30](https://github.com/guirossibrum/VariaxWorkbench/issues/30).
- [ ] Workbench HD/JTV-69 and POD HD500X backends; other models added only with independent verification — [#31](https://github.com/guirossibrum/VariaxWorkbench/issues/31), [#32](https://github.com/guirossibrum/VariaxWorkbench/issues/32).

### Currently unavailable or not working

- [ ] The image-default Windows x87 math substitution fails the Mac generator's strict startup-constant check; its complete-generator replay stops. Historical Mac parity remains unresolved, and the optional portable profile is experimental.
- [ ] Guided check does not yet read switches/knobs, record audio or measure notes/bends/tuning. Future integration is [#34](https://github.com/guirossibrum/VariaxWorkbench/issues/34); the current app rehearses prompts only.
- [ ] **Live device access is intentionally disabled**, including identity/metadata reads. No runtime unlock exists.
- [ ] Sending edited presets to the guitar, firmware flashing and factory restoration are unavailable; no live writer is implemented or authorized.
- [ ] Live USB audio capture/playback and recording controls are not implemented or validated; no audio stream has been opened.
- [ ] Full PODxt Live/Variax firmware and audio/DSP emulation is not implemented. Existing host-library emulation is not device emulation.
- [ ] The earlier experimental full-DSP read timed out; no complete DSP/bank/calibration backup was obtained. The captured “Reso-5” file is one preset's settings only.
- [ ] Linux did not expose this guitar's Variax ALSA port in earlier observations; the identity-matching issue remains unresolved and no driver patch is installed.
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
