> Runtime correction: [whole-generator reference invalidation](pickup-response.md#whole-generator-reference-invalidation). Earlier execution counts do not validate initialized-library behavior.

> Later milestone: [offline generator and POD download results](offline-generator-results.md).
> Both firmware images are now acquired/unpacked, and original native generation
> runs with runtime adapters. The initial-report limitations below are historical;
> the full Rust coefficient port and hardware/recovery validation remain incomplete.

# Variax 600 / PODxt Live: offline engineering report

> Acquisition update: the user subsequently supplied the Variax 300/600 3.10
> firmware, verified against catalog size/MD5. See the acquisition addendum in
> [factory-reference.md](factory-reference.md). Download limitations below describe
> the initial report; unpacking/recovery remains pending.

Date: 2026-11-15. Workspace: `/home/grbrum/Documents/Code/VariaxWorkbench`.

## Result

There is now a buildable **Rust offline editor**, recovered original artwork,
lossless preset-file support, bank-file inspection/extraction, an isolated native
reference-testing setup, and a documented firmware/factory-reference investigation.
This is a substantial working milestone, **not yet a complete replacement for
Workbench's device programming and coefficient engine**.

The guitar and POD remain protected by a source-level hardware lock. No preset
upload, flash write, firmware update, factory reset or bank switching was performed.
No live access was attempted after the user imposed the offline-only boundary.
The USB cable may remain disconnected for all functionality described here.

## Software recovered and tools built

- Recovered and extracted Workbench **1.75** for Windows and macOS, Monkey
  **1.77 Windows / 1.78 macOS**, and Central **1.00 Windows**. These packages
  were inspected as data; no vendor installer or updater was executed.
- Built the user's sibling REA checkout, version **6.9.0**, commit
  `79cc07ecf191e44ff4e280fa843e11a574e20f77`, with its pinned local Node/npm.
  Added wrappers for repeatable REA jobs and saved their evidence outputs.
- Installed project-local Java, Ghidra, 7-Zip, Node and Python/Unicorn tools in
  `.tools/`. No global desktop configuration, permanent USB rule or updater
  installation was made.
- Extracted Java classes and native libraries from both platforms. Of 661 shared
  Java classes, 659 are identical; the differences are application/version glue.
- Split and validated eight Mach-O architecture slices. Analyzed the i386 native
  coefficient engine, MIDI library and Monkey executable in Ghidra through REA.
- Exported **940 coefficient-engine function bodies**, with 129 external entries
  and no export failures among the discovered functions. This is a discovered
  function inventory, not proof that every algorithm or entry point is understood.
- Inspected selected native MIDI discovery/open/send routines and located Monkey's
  firmware-related symbols and strings. The update protocol is not reconstructed.

Installers, extracted/decompiled proprietary files, original artwork, captures,
and portable tools remain local and ignored. Nothing was published or uploaded.
Hashes and source/evidence records are under `analysis/reports/`; the native
investigation is described in [native-analysis.md](native-analysis.md).

## Original images and editor

Recovered **396 original raster images** unchanged, including guitar bodies,
pickups, neck/fretboard/tuning imagery, controls and help artwork. Copies were
checked against their source hashes. The local visual catalog is
`analysis/ui-assets/index.html`; contact sheets are in the same directory.

The Rust GUI uses original body and pickup images and recovered placement geometry.
It supports body and pickup selection, pickup position/angle, alternate tuning,
wiring and electronics controls, preset naming, undo/redo and saving a new copy.
The selector exposes 28 bodies and 32 user-facing pickup definitions; the other
11 native pickup definitions are internal combinations, not additional user models.
The original discrete resistance/capacitance and taper choices were recovered.

File saves write a temporary sibling, sync it, and persist it without replacing an
existing destination. Failed edit transactions roll back. Drag edits group into
one undo operation. The UI protects dirty documents, blocks mutations while a
read worker is active, caches missing artwork failures, and reports invalid CLI
arguments without a panic. Live reading is disabled and guarded before worker
creation. Headless tests cover specific guards and failure cases; they are not
an exhaustive automated click-through of every dialog or control.

Launch the offline editor with:

```sh
cargo run --features gui --bin variax-workbench-gui -- --preset backups/current-variax-600.vwm
```

Release binaries: `target/release/variax-workbench` and
`target/release/variax-workbench-gui`. They currently depend on the local artwork
and extracted default preset paths; this is not a packaged distributable app.
Screenshots: `analysis/ui-assets/rust-editor.png`,
`rust-editor-refined-controls.png`, and `rust-editor-final-pickups.png`.

## File and protocol implementation

- Parses individual big-endian `FORM/L6PA` `.vwm` presets, retaining original
  bytes, unknown chunks, ordering, padding and parameter bit patterns.
- Supports Unicode naming, typed parameter changes, model selection and enabled
  states. Unsupported types/ambiguous required chunks fail explicitly. CLI values
  are not checked against every model-specific hardware range.
- Parses big-endian `FORM/L6BA` banks and `FORM/L6CO` collections; CLI commands
  inspect banks and extract individual presets. The library can replace an
  embedded preset while rebuilding lengths and preserving other bytes. Replacement
  has synthetic coverage; arbitrary historical bank variants remain unverified.
  The GUI currently operates on individual presets rather than a bank browser.
- Implements Variax 24-bit nibble encoding, metadata decoding, identity parsing,
  fragmented SysEx framing and real-time MIDI-byte handling.
- Separates the read-session algorithm from its physical transport so the same
  session flow can be tested with a fake transport. The retained USB transport is
  locked before enumeration/opening and has a second exact-message allowlist.
- No device preset writer, firmware writer or live arbitrary-memory command
  interface is implemented. Unsupported experimental full-DSP reading was removed
  from the product interface after an earlier timeout.

## Native model and DSP work

All **43 pickup definitions** were decoded: electrical parameters, volume,
pole offsets and coil spacing. All **28 body definitions** were decoded with
original bytes retained. Each 17,483-byte body definition includes its header and
six variants. Original native serialization confirms the full schema, including
1,421 field operations per body. Some nested data remains exposed as opaque
payload, and the meaning/interpolation of every coefficient is not established.
See [body-definitions.md](body-definitions.md).

Rust implements the native intermediate packing routine: 1,091 low-24-bit words
plus an additive checksum produce 3,276 bytes. Rust also implements a tested
body-resonator coefficient conversion, including native ordering, clamping and
asymmetric rounding. Original i386 instructions were run in Unicorn in isolated
memory, with bounded execution and no OS/device services. Serialization harnesses
supply an in-memory byte reader and bypass formatting-only labels; these are
reference tests with disclosed adapters, not the complete application executing.

**The full body/pickup coefficient engine, metadata overlay, final patch assembly,
and end-to-end uploadable output are not complete.** Native packing agreement
alone cannot establish an edited patch will sound correct or be safe to upload.

## Earlier hardware observations and preservation

Before the current hardware prohibition, identity replies reported:

| Equipment | Product ID | Observed main firmware |
| --- | --- | --- |
| Variax 600, shared 300/600 identity | `0x00070004` | **3.10** |
| PODxt Live | `0x0003000a` | **3.01** |

The Linux Variax interface issue is consistent with a driver matching an older
product identity while this guitar replies with model ID 4. This conclusion is
supported by the observed reply and kernel source; no driver patch was installed.
The direct probe temporarily claimed only the guitar-control interface, requested
identity, enabled a MIDI session, read two metadata blocks, disabled the session,
and released/reattached the driver. A temporary per-device permission ACL was used;
no persistent udev rule was installed. Read queries transmit data and session
commands, so the report does not describe that earlier interaction as passive.

The captured current preset is **“Reso-5”**. Raw replies are in
`analysis/captures/variax-usb-20261114-235059/`; the Rust settings file is
`backups/current-variax-600.vwm`. The original Java metadata decoder and Rust
produce matching model/parameter summaries from that capture.

**This is one current preset's model/settings metadata, not a complete stored-bank,
DSP, calibration, flash or recovery backup.** An experimental broader DSP read
attempt timed out and yielded no DSP backup. Nothing justifies treating the
current capture as sufficient protection for future device writes.

## Firmware versions, downloads and factory presets

The observed main firmware versions match the newest applicable official listings:
[Variax 300/600 3.10](https://line6.com/software/index.html?hardware=Variax%20300%2F600&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show),
released January 6, 2006, release 1062; and
[PODxt Live 3.01](https://line6.com/software/index.html?hardware=PODxt%20Live&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show),
released April 20, 2006, release 1224. These listings indicate no newer main flash
version for either observed device. POD USB firmware is a separate component and
has **not** been verified.

Actual firmware binaries have **not been acquired**. Official release downloads
returned HTTP 403/account access requirements; Central's image endpoint requires
a token. Public metadata for the exact 300/600 image records 124,208 bytes and MD5
`8b64eddd73f1d4a1eaa72c8dbd98ece5`, but this is not a verified local payload.
Central's public catalog and statically extracted product map corroborate model
applicability. No credentials, browser cookies or user account tokens were read.

Workbench's “Default Patch” and physical model assets are **not a complete factory
preset bank**. No authenticated Variax 600 factory bank was recovered. Matching
firmware versions cannot establish that the user's presets equal factory presets,
or that a version's defaults equal the guitar's exact original shipping state.
Line 6 documents factory restoration through reflash, but that procedure was not
run and is not recommended for the current phase.

Firmware unpacking, firmware disassembly and offline recovery simulation remain
pending the exact artifacts. No firmware improvement or safe recovery capability
has been established. See [factory-reference.md](factory-reference.md) for exact
sources, hashes, acquisition failures and product distinctions, and [GitHub issues](https://github.com/guirossibrum/VariaxWorkbench/issues)
for the preserved firmware/recovery tasks.

## Validation ledger

All commands below are offline. Final Rust checks completed successfully:

| Check | Result and scope |
| --- | --- |
| `cargo test --all-features` | **32 tests passed**: 25 library, 4 GUI, 1 document integration, 2 malformed-input integration |
| `cargo clippy --all-features --all-targets -- -D warnings` | Passed, no warnings |
| `cargo fmt --check` | Passed |
| `cargo build --release --all-features` | CLI and GUI optimized builds passed |
| Original Java preset compatibility | **6 checks passed**: electric/acoustic read and exact roundtrip, Unicode rename, typed edit, Java reserialization, overwrite refusal |
| Original Java bank compatibility | **Both formats passed**: Java creation, reader-summary agreement and Rust extraction for L6BA/L6CO |
| Saved real metadata capture | Original Java DecodePatch output and Rust capture-derived preset summaries agree for “Reso-5” |
| Native pickup serialization | **43 vendor assets plus synthetic fixture passed** |
| Native body serialization | **28 complete vendor assets plus synthetic fixture passed**, all 1,421 field calls per body |
| Native DSP packing | Original machine-code output matches the Rust synthetic fixture; checksum/boundary tests pass |
| Resonator conversion | **16 original machine-code vectors**, with zero/negative/saturation/rounding cases, match Rust fixtures |
| Malformed-input robustness | 4,096 deterministic mutated/arbitrary inputs do not panic; truncated files/length errors and oversized SysEx/recovery also covered |
| Simulated session faults | Wrong identity, timeout, disconnect, partial enable failure, cancellation, cleanup failure, noise and fragmented metadata tested through shared session flow |
| Hardware lock | Tests confirm the physical path rejects before enumeration/opening and GUI starts no live worker |
| GUI smoke check | Linux GUI rendered the saved preset and original artwork; screenshots saved; no hardware action |

Logs and machine-readable reference reports are in `analysis/reports/`:
`all-feature-tests.log`, `clippy.log`, `release-build.log`,
`preset-compatibility.json`, `bundle-compatibility.json`,
`captured-preset-agreement.json`, `native-body-serialization.json`,
`native-pickup-serialization.json`, `native-pack-emulation.json`, and
`native-resonator-emulation.json`.

During development, a malformed-bundle test initially changed a valid HEAD
payload rather than a chunk length and therefore failed to provoke rejection.
The fixture was corrected to corrupt the actual bank length; the final suite
passes. The earlier physical DSP timeout and firmware download failures remain
unresolved limitations, rather than being counted as successful tests.

## Risks and next steps

Offline tests provide evidence for specific behavior; they cannot prove zero risk
on aged physical hardware. USB driver detach/reattach failures, device-side timing,
bootloader behavior, power loss, calibration handling and recovery remain
unvalidated in this phase. No emulator currently represents the guitar's complete
DSP/firmware or the POD's bootloader. The current native oracle covers selected
original host-library routines only. Ghidra decompilation and inferred meanings
can be wrong; the Ghidra bridge accepts the local 12.2 installation but its baseline
was verified on 12.1.4. No months-long fuzzing or full formal verification was done.

The application remains an offline prototype. File correctness for tested examples
is not a guarantee of hardware-valid model combinations or audio equivalence.
There is no complete backup and no proven recovery path, so persistent device
changes remain out of scope.

The next useful work is to acquire the exact authorized firmware artifacts,
recover a trustworthy 300/600 factory reference, port and compare the remaining
coefficient calculations against native outputs over broader parameter ranges,
and build an offline device/update model with corruption and recovery fixtures.
Full-bank GUI support and packaging can proceed independently. These pending
items mean the user's preconditions for considering writes are **not satisfied**.
Hardware access stays locked; any future live operation requires a separate,
precise review and explicit authorization. General permission to continue
engineering does not unlock it.
