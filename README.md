# Variax Workbench for Linux

A Rust replacement under development for the original Line 6 Variax 600,
connected by VDI through a PODxt Live. The current milestone is an **offline
preset editor and reverse-engineering toolkit**, with the original guitar and
pickup artwork recovered from Workbench 1.75.

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
work, and [TODO.md](TODO.md) for the hardware boundary and next steps.

## Implementation and validation checklist

Keep this checklist current as issues progress. Checked items passed the stated
offline checks; they do not certify physical safety or hardware/audio equivalence.
Detailed evidence and limitations: [body interpolation](docs/body-definitions.md),
[generator results](docs/offline-generator-results.md), and [backlog](docs/github-backlog.md).

### Verified and passing

- [x] Rust CLI and offline GUI build, including optimized release builds.
- [x] **40 Rust tests**, Clippy with warnings denied, and formatting checks.
- [x] Individual preset parsing/editing and protected copy saves: **6 original-Java compatibility checks**.
- [x] L6BA/L6CO bank inspection and preset extraction: **both formats compared with original Java**.
- [x] Offline GUI with recovered artwork, preset controls, undo/redo and guarded document operations; selected headless tests and rendering smoke checks pass.
- [x] All **43 pickup definitions** and **28 body definitions** verified against original native serialization.
- [x] Complete typed Rust body decoding and **748 bit-exact pickup-normalization comparisons**.
- [x] Rust body interpolation routing: **600 native routing-plan comparisons**.
- [x] Actual Rust body-field interpolation: **252 vendor-body outputs + 84 synthetic outputs** match every serialized field in debug and release under the documented host-libm oracle.
- [x] Original computer-side coefficient generator runs in isolated CPU emulation: **110 cases**, **98 distinct outputs**, plus **4 JNI comparisons** across 53 fields. This is a reference oracle, not the completed Rust generator.
- [x] Rust DSP packing/checksum and selected resonator conversions match original-machine-code fixtures.
- [x] Firmware inspection: both supplied images unpack with matching embedded MD5 and complete region coverage; **396 corrupt/truncated synthetic inputs rejected**.
- [x] Simulated read-session fragmentation, cancellation, timeout, disconnect and cleanup failures; physical-access locks remain tested and enabled.

### Pending implementation or validation

- [ ] Complete Rust pickup/electronics response calculations — [#2](https://github.com/guirossibrum/VariaxWorkbench/issues/2).
- [ ] Remaining DSP blocks, tuning and complete patch assembly, including Java adjustments and metadata — [#3](https://github.com/guirossibrum/VariaxWorkbench/issues/3), [#4](https://github.com/guirossibrum/VariaxWorkbench/issues/4).
- [ ] Historical math-runtime parity and complete Rust/native generator validation — [#5](https://github.com/guirossibrum/VariaxWorkbench/issues/5), [#6](https://github.com/guirossibrum/VariaxWorkbench/issues/6).
- [ ] Model-specific parameter validation, complete bank GUI workflows and broader GUI interaction tests — [#7](https://github.com/guirossibrum/VariaxWorkbench/issues/7)–[#9](https://github.com/guirossibrum/VariaxWorkbench/issues/9).
- [ ] Reproducible fresh-clone tool/artifact setup, Linux packaging and provenance completion — [#10](https://github.com/guirossibrum/VariaxWorkbench/issues/10)–[#12](https://github.com/guirossibrum/VariaxWorkbench/issues/12).
- [ ] Firmware architecture/region mapping, factory/calibration extraction and trustworthy factory comparison — [#13](https://github.com/guirossibrum/VariaxWorkbench/issues/13)–[#15](https://github.com/guirossibrum/VariaxWorkbench/issues/15).
- [ ] Static recovery/update reconstruction, hardened Rust firmware inspection and virtual device/update simulation — [#16](https://github.com/guirossibrum/VariaxWorkbench/issues/16)–[#18](https://github.com/guirossibrum/VariaxWorkbench/issues/18).
- [ ] Firmware-improvement feasibility, separate POD USB-firmware verification and Linux bridge investigation — [#19](https://github.com/guirossibrum/VariaxWorkbench/issues/19)–[#21](https://github.com/guirossibrum/VariaxWorkbench/issues/21).
- [ ] Readiness review, comprehensive backups and any future authorized hardware integration/write design — [#22](https://github.com/guirossibrum/VariaxWorkbench/issues/22)–[#25](https://github.com/guirossibrum/VariaxWorkbench/issues/25).

### Currently unavailable or not working

- [ ] **Live device access is intentionally disabled**, including identity/metadata reads. No runtime unlock exists.
- [ ] Sending edited presets to the guitar, firmware flashing and factory restoration are unavailable; no live writer is implemented or authorized.
- [ ] Full PODxt Live/Variax firmware and audio/DSP emulation is not implemented. Existing host-library emulation is not device emulation.
- [ ] The earlier experimental full-DSP read timed out; no complete DSP/bank/calibration backup was obtained. The captured “Reso-5” file is one preset's settings only.
- [ ] Linux did not expose this guitar's Variax ALSA port in earlier observations; the identity-matching issue remains unresolved and no driver patch is installed.
- [ ] No complete authenticated Variax 600 factory bank or proven recovery path is available.
- [ ] A fresh clone lacks proprietary artwork, vendor binaries and personal captures by design; original-library reference tests and the illustrated GUI need separately supplied local artifacts. No packaged standalone distribution exists yet.

## Run the offline editor

```sh
cargo run --features gui --bin variax-workbench-gui -- --preset backups/current-variax-600.vwm
```

The local extracted artwork lives in `assets/original-ui/`. The editor supports
body/pickup selection and placement, tuning and electronics controls, undo/redo,
and saving a new copy. Originals and existing output files are protected.
A saved preset cannot yet be sent to the guitar; full DSP generation is unfinished.

## Offline command line

```sh
cargo run --bin variax-workbench -- inspect INPUT.vwm
cargo run --bin variax-workbench -- rename INPUT.vwm COPY.vwm "My Guitar"
cargo run --bin variax-workbench -- set INPUT.vwm EDITED.vwm 02010001 200000 float 2.5
cargo run --bin variax-workbench -- inspect-bundle BANK.vwb
cargo run --bin variax-workbench -- extract-preset BANK.vwb EXTRACTED.vwm 0 0
```

Output paths must not already exist. `set` takes hexadecimal model-local IDs;
its values are type-checked but not validated against every model's ranges.
`decode FIRST.syx SECOND.syx` decodes two captured metadata replies. `preview`
prints recovered request bytes without transmitting them.

## Verify

```sh
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
python scripts/verify_preset_compatibility.py
python scripts/verify_bundle_compatibility.py
.tools/python/bin/python scripts/verify_native_pack.py
.tools/python/bin/python scripts/verify_native_resonators.py
.tools/python/bin/python scripts/verify_native_pickups.py
.tools/python/bin/python scripts/verify_native_bodies.py
.tools/python/bin/python scripts/verify_body_rust.py
.tools/python/bin/python scripts/verify_body_plans.py
.tools/python/bin/python scripts/verify_body_interpolation.py
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
