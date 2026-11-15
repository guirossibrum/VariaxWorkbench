> Historical milestone notes. See [the consolidated report](report.md) for current
> features, hardware locks, validation results and limitations.

# Offline native analysis and Rust implementation

## Environment

REA 6.9.0 was built successfully in the supplied sibling checkout at commit
`79cc07ecf191e44ff4e280fa843e11a574e20f77` using `npm ci` and
`npm run build:cached`. The checkout has no tracked changes after the build.
Build logs are in `analysis/reports/rea-npm-ci.log` and `rea-build.log`.

The project-local `scripts/rea` wrapper selects Node 24.22.1, npm's pinned
toolchain was 11.16.0, and native analysis uses Ghidra 12.2 and Temurin JDK
25.0.5. No system Node or Java replacement and no client MCP registration was
needed. `analysis/reports/rea-doctor.json` reports the Ghidra provider healthy;
unrelated providers are not installed. Ghidra accepts 12.2 but REA's bridge
reports its verified baseline as 12.1.4, an explicit coverage limitation.

Portable upstream packages live in `.tools/`; REA dependencies and build outputs
live in `../rea`. The user authorized dependency installation for this work.
The local scripts speak MCP over standard input/output, keeping each batch's
target session open until its close and evidence export steps complete.

## Extraction coverage

Both Workbench 1.75 installers and Monkey 1.77 for Windows were extracted.
Monkey 1.78 for Mac was additionally downloaded from the official release
endpoint `https://line6.com/getrelease?rid=8688` and extracted, including the
bundled License Manager payload. Mac installer scripts were extracted for
inspection, never executed. Windows Monkey PE resources were also extracted.

`analysis/reports/inventory.json` holds installer hashes and Java comparisons.
`analysis/reports/extracted-files.json` inventories 2,666 extracted files,
including intermediate archives and derived native slices; that is not a count
of distinct application features. Both Workbench Java variants have been
decompiled with CFR. Native execution of the original apps has not been attempted.

`scripts/split_macho.py` validates universal headers, architecture tables and
slice bounds before splitting libraries; Java classes share the `CAFEBABE`
magic and must not be mistaken for universal binaries. Eight native slices
were extracted, with source offsets and hashes in `macho-slices.json`.

## DSP engine: static results

REA/Ghidra selected the i386 Mac coefficient-engine slice, SHA-256
`3aa232329d467101637764397b77a7b8ffa6af0ccd72ec380d02f145477ba9fe`.

The complete discovered function inventory contains 1,069 procedures. REA
exported **940 decompiled function bodies**, with 129 external functions and
zero reported failures. Discovery coverage remains unknown: this is complete
for Ghidra's inventory, not proof of perfect function recovery or correctness.

Export evidence:
`ev_36603b8c5893bdf1f9b81cfeeda522e9cf8acf90a1ccff6bb905392f42c611ce`,
saved in `analysis/reports/native-coef-export.json` and the portable
`analysis/reports/native-evidence.json` bundle. Function output and its digest
index are in `analysis/decompiled/native-coef/`.

Recovered conversion path:

1. JNI `nativeCreateHWPatch`, address `0x4790`, reads Java `VWPatch` fields.
2. `L6VariaxCoefEngine::GenerateHWPatchFromVWPatch`, `0x2c40`, looks up body
   and pickup models and constructs intermediate DSP state.
3. DSP component `UpdateHWPatch` methods populate hardware words. For example,
   `L6VariaxBodyResonance::UpdateHWPatch`, `0x16f40`, iterates 12 resonators
   and invokes `SetBodyResonatorCoefs`, `0x18670`.
4. `L6VariaxHWPatch::Pack`, `0x1bd10`, packs 1,092 words into 3,276 bytes,
   taking three bytes per word in big-endian order. JNI copies those bytes to
   Java; Java subsequently overlays metadata and recomputes its checksum.

The coefficient setter shows saturation, scaling and signed fixed-point
conversion. It is pseudocode, not yet a verified Rust algorithm. In particular,
floating-point precision, rounding, native word layout, metadata overlays and
checksum boundaries need reference comparisons before implementing edits.
The Java checksum loop and native packing loop have different bounds, so a
single assumed checksum rule would be premature.

## MIDI and Monkey

REA/Ghidra discovered 2,955 procedures in the i386 Mac MIDI library, SHA-256
`902713ed5e4819021a73ff8a4f2d6a35521a99a96af7253dc53ef153e3789a8f`.
Overview evidence:
`ev_77b7bf62127550467eb974b14adea27f07fbf7c9505e64ebf9d980950a8f6edf`.
Five Variax-named function bodies were exported separately, evidence
`ev_12b172f2238d816c8c6621fb64b684c68830cc736d574d9952c7f9b4d2b8a261`.
This is a focused export, not a decompilation of the entire MIDI library.

Eleven additional MIDI routines were decompiled in a focused batch, including
JNI open/send, device discovery and driver-backed transmission. Evidence:
`ev_b330702b4e16d2dd6a367e8b6ab7733fd1b1dc4fb7555e743b17c8e39f158e9f`,
saved in `analysis/reports/midi-detail-decompile.json` and the subsequent
`midi-monkey-evidence.json` bundle. Device discovery sends the native inquiry
constant and handles downstream device families; driver transmission has its own
packing and pacing. These paths are not yet validated as a Linux implementation.

The native backend contains CoreMIDI and Line 6 driver interfaces. Its
`L6DirectMidiMac_SPSI::BuildAndSendVariaxMidi` path builds driver-specific
messages; that path must not be confused with ordinary Variax SysEx framing.
Which route the Linux PODxt driver exposes still requires hardware inspection.

Monkey's Windows PE imports, resources and exports were inspected through REA;
results and evidence bundle are `analysis/reports/monkey-inspect_*.json` and
`pe-static-evidence.json`. Mac Monkey's packaged `default.cfg` is XML and
contains hardware definitions, including:

| Device | USB product ID | MIDI product ID |
| --- | --- | --- |
| PODxt Live | `0x4650` | `0x0003000a` |
| Variax 300/600 | none declared | `0x00070004` |
| Variax USB Interface | none declared in this configuration | `0x00700001` |

These are shipped configuration declarations, not hardware observations. The
configuration also names historical server endpoints; endpoint strings do not
establish that those services still work. Selected definitions are saved in
`analysis/reports/monkey-hardware-definitions.json`. Firmware updating remains
unimplemented and untested.

REA/Ghidra also inspected Mac Monkey's native executable: 8,893 procedures and
16,198 strings in the discovered inventory. Overview evidence:
`ev_dd37bf3626bbc1806eb5473efbdbc8a88e757e438acd10ae04cdb3e22fe5ee8d`.
The search found named update entry points such as `L6CamelSystem::UpdateFirmware`
at `0x1000a6` and `MNKYUpdateFirmware` at `0xfe360`. This is an inventory and
search pass, not a verified reconstruction of firmware updating. The complete
results are in `monkey-native-overview.json`, `monkey-native-functions.json`,
`monkey-native-strings.json`, and `midi-monkey-evidence.json`.

The model definition catalog contains 28 body definitions and 43 pickup
definitions, all with resolved numeric IDs and hashed binary source files.
It is saved in `analysis/reports/model-catalog.json`. These are DSP model
definitions; their count differs from the number of distinct UI images.

## Rust implementation

The replacement application uses Rust. Python/Java scripts remain analysis
helpers, not the product runtime. The crate has no third-party dependencies yet.

The subsequent [preset-file milestone](preset-files.md) adds lossless file
inspection/edits, verification against the original Java parser/writer, and a
native DSP packing primitive verified against emulated original machine code.
That report supersedes the initial Rust feature/test counts below.

Implemented in `src/lib.rs`:

* 24-bit Variax nibble encoding/decoding, with encoder fixtures produced by the
  original Java class.
* PODxt routing/MIDI enable and current-preset metadata request construction.
* Strict complete-reply validation, including length, header, opcode, nibble
  range and SysEx terminator.
* Decoding both metadata blocks into a preset name and typed parameter map:
  body, pickups, wiring, tuning, detune and mix. Original metadata bytes and
  unused padding are retained.

`src/main.rs` exposes offline `preview` and `decode` commands. Metadata replies
must arrive in request order; the recovered reply format does not identify their
addresses. Internal string numbering differs from the original display labels.
Metadata is not a full DSP backup, and decoding is not an editing implementation.

Validation: `cargo test` passes four tests covering original Java encoder vectors,
asymmetric nibble order, malformed packets, and typed metadata/padding.
`cargo clippy --all-targets -- -D warnings` passes. `cargo run -- preview` prints
the recovered bytes without opening any device. No live transport, graphical UI,
native coefficient port, preset writes, or firmware writes have been implemented.
The Rust CLI also successfully decoded a pair of synthetic binary reply packets
and rejected a malformed second input; the decoded output is saved in
`analysis/reports/rust-synthetic-decode.txt`.

## Remaining work

Use the saved native output to recover the model-file and DSP coefficient
algorithms, verify metadata decoding against original Java outputs and real
captures, then implement the Rust model/file layer and Linux transport. Hardware
availability is needed only for integration testing, not continued static work.
