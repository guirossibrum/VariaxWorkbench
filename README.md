# Variax Workbench for Linux

A Rust replacement under development for the original Line 6 Variax 600,
connected by VDI through a PODxt Live. The current milestone is an **offline
preset editor and reverse-engineering toolkit**, with the original guitar and
pickup artwork recovered from Workbench 1.75.

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
