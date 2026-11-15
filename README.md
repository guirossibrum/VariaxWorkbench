# Variax Workbench for Linux

A Rust replacement under development for the original Line 6 Variax 600,
connected by VDI through a PODxt Live. The current milestone is an **offline
preset editor and reverse-engineering toolkit**, with the original guitar and
pickup artwork recovered from Workbench 1.75.

## Experimental software — use entirely at your own risk

This is an independent, reverse-engineered project developed with AI assistance.
It is not an official Line 6 product and is not affiliated with, endorsed by, or
supported by Line 6, Yamaha, or any equipment manufacturer. No technical support,
maintenance, recovery service, or continued development is promised.

The software is provided **“as is”, without warranty of any kind**. Its output,
sound, timing, controls and other behavior may differ from the original software.
Errors, incomplete reverse engineering, AI-generated mistakes or incompatibilities
may cause lost presets or calibration data, unexpected or damaging audio output,
unusable devices, or **permanent damage to or complete destruction of connected
hardware**. Tests, backups and the current hardware locks do not guarantee safety
or recoverability. Anyone who downloads, modifies, builds or uses this project
does so entirely at their own risk and assumes responsibility for that decision.

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
