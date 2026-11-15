> Correction: earlier whole-generator/JNI output validation is invalidated as a
> reference baseline because a required native module initializer was missing.
> Generation is now guarded. See [the runtime finding](pickup-response.md#whole-generator-reference-invalidation).
> Standalone body/circuit/primary-fit checks are independently validated and do
> not use the affected globals. Historical execution counts below are not proof
> of correct native startup.

> Subsequent issue #1 progress: complete typed body decoding, native pickup
> normalization, interpolation routing and full field blending are now verified. See
> [body-definitions.md](body-definitions.md). The full pickup/DSP/assembly engine
> remains incomplete.

# Offline generator tests and PODxt Live downloads

Date: 2026-11-15. No hardware access, driver installation or updater execution.
The Rust, GUI and Python hardware locks remain unchanged.

## Original coefficient generator now executes

The test harness loads the original i386 Workbench coefficient library into
Unicorn private memory and runs its constructors, all model registrations,
`GenerateHWPatchFromVWPatch`, `GenerateHardwarePatch`, and the original packer.
All 28 body and 43 pickup definitions are loaded using offsets previously
verified against original native serialization. Original AddModel routines insert
into private, preallocated hash-bucket/vector storage.

Imports resolve to shipped symbols or explicit in-memory allocation, bounded
memory/string operations and the local system math library. Unrecognized imports
stop execution; no file/network/device APIs are supplied. Calls are bounded by
both instruction count and elapsed execution time. The original binaries are
unchanged; import binding modifies only the private emulator image.

The sweep covers every recovered body and pickup definition, pickup position,
normalized angle, level, volume/tone resistance, capacitance, tone position,
saved tone position, trim, two-pickup parallel/series/phase wiring, and alternate
tuning. Every case must return a 3,276-byte patch with a valid additive checksum.
The initial baseline must recur byte-for-byte after all sweeps and in a fresh
emulator instance. Baseline-valued inputs may correctly reproduce the baseline.
Tone position alone also does not alter the tested output while save-tone is
false; three saved-tone positions must produce three distinct outputs.

The complete result and per-case input/output hashes are recorded in
`analysis/reports/coefficient-generator-tests.json`, with the execution log beside
it. Local binary reference candidates remain ignored under
`analysis/extracted/coefficient-reference/` and are never transmitted to hardware.

A separate JNI cross-check executes the **original JNI input extraction and output
copying code**, with a synthetic JNIEnv that supplies named scalar fields and a
private byte array. Four cases—baseline, saved tone, series plus phase reversal,
and alternate tuning—produce identical output through the JNI and direct native
paths. This checks the manually prepared native struct against the original JNI
mapping. See `analysis/reports/coefficient-jni-agreement.json`.

## Validation result

- **110 generator cases passed**, with **98 distinct complete patch outputs**.
- All packed outputs have valid additive checksums; warm/fresh baseline outputs
  agree byte-for-byte. Saved-tone positions produce distinct outputs.
- **4 JNI/direct comparisons passed**, exercising **53 named fields**.
- Both firmware containers passed bounded unpacking, MD5 and region-coverage checks.
- The synthetic firmware test rejected **396 corrupt/truncated inputs**.
- The existing **32 Rust tests**, Clippy with warnings denied, formatting checks,
  and all four native primitive/serialization reference scripts passed again.
- The optimized fixture naming includes both case name and input bytes, so the
  11 identically displayed internal pickup combinations retain distinct files.
  An earlier naming collision correctly refused to replace a different fixture;
  this was a test-artifact naming problem, not a coefficient calculation failure.

## What these results establish and what they do not

These are original-library execution and reference-generation tests, **not yet
Rust coefficient-engine parity tests**. They establish a repeatable offline source
of complete reference outputs, enabling development without capturing additional
guitar slots. Existing tests of Rust's packer/resonator primitives still pass.

The historical macOS libm is not emulated; local host libm supplies external math
functions. Differences in transcendental-function rounding could affect final
quantization. Original Java `ApplyHacks`, model-specific capacitance overrides,
pickup-combination selection, preset-to-native unit conversions, name/metadata
overlay and the final Java checksum pass are outside this sweep. These must be
incorporated and independently checked before claiming complete Workbench parity.
The negative-ID internal pickup combinations are also exercised as synthetic
primary pickups, which is not a normal user-facing configuration. Successful
execution/checksum alone does not prove valid audio, valid physical combinations,
or safe hardware programming. No firmware/DSP or bootloader emulator is claimed.

An initial sweep stopped on an unsupported `strncpy` runtime import; a bounded
private-memory implementation was added. The original one-million-instruction
limit was insufficient for the complete generator, so full-generator calls use
a documented maximum of twenty million instructions / ten seconds, while small
oracle calls retain their original lower limits. Generic per-instruction tracing
was replaced by hooks only at adapter entry points to reduce Python overhead.
These changes neither bypass model calculations nor enable OS/device services.

## Firmware inspection

Both downloaded firmware files are **FORM/L6FF** containers with a 52-byte HEAD,
region-information chunks and zlib-compressed image data. A strict offline parser
checks root/chunk bounds, supported header/compression values, bounded expansion,
stream termination, embedded uncompressed-image MD5, and contiguous region coverage.
A synthetic fixture plus **396 corrupt/truncated inputs** exercise rejection.
The MD5 is an integrity check, not an independent signature/authenticity proof.

| Download | Declared product | Declared revision | Expanded image | Regions |
| --- | --- | --- | --- | --- |
| `PODxtLive_3_01.xtf` | `0x0003000a` | `0x03010000` = 3.01 | 1,048,576 bytes | 19 |
| `Variax300_3.10.vxf` | `0x00070004` | `0x03100000` = 3.10 | 442,402 bytes | 4 |

Both declared identities/revisions agree with the earlier observed devices, and
both expanded images match their embedded MD5 values. PODxt Live source SHA-256:
`738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5`.
Variax source SHA-256:
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`.
User-provided download URLs were not captured.

Images and individually hashed regions are extracted under
`analysis/extracted/firmware/<source-sha256>/`, with manifests. Detailed evidence:
`analysis/reports/firmware-containers.json`. Numeric region-type meanings,
executable architectures, bootloader/recovery behavior, and factory preset tables
are not yet established. No complete factory bank has been extracted.

The **Variax firmware** is a useful lead for factory defaults and the guitar's
receiving DSP/data layout. The **POD firmware** is useful for later study of the
VDI/USB bridge and update/recovery implementation. Neither is necessary to start
host coefficient generation, and downloading/unpacking neither authorizes flashing.

## Driver usefulness

The Windows **4.2.7.7** installer was extracted as data. Its PODxt INF reports
`DriverVer=03/15/2019,4.2.7.7` and explicitly maps PODxt Live to
`USB\VID_0E41&PID_4650`. It includes x86/x64 POD Live kernel drivers and companion
DLLs, separately named from PODxt and PODxt Pro. Static imports show USB
configuration/descriptor support; exact transport semantics require disassembly.

The macOS **7.6.8** DMG/pkg and all five nested payloads were extracted without
running their install/uninstall scripts. Its audio driver's Info.plist explicitly
matches vendor 3649 (`0x0e41`), product 18000 (`0x4650`), configuration 1,
interface 0. Its native symbols include several implementations named
`L6FeatureHandlerBase::BuildAndSendVariaxMidi`, alongside MIDI routing/SysEx code.
These are concrete leads for bridge behavior, not proof of their complete protocol.

Both packages help compare device identification, interface ownership, buffering
and Variax message routing with Linux behavior. They are **OS-specific reference
material**, not drivers that can be loaded into Linux and not the coefficient
engine itself. No driver was installed or loaded. Manifests and provenance hashes:
`analysis/reports/pod-user-downloads.json` and `pod-driver-static-summary.json`.

## Reproduction

```sh
.tools/python/bin/python scripts/test_coefficient_generator.py
.tools/python/bin/python scripts/verify_coefficient_jni.py
python scripts/inspect_firmware.py --self-test \
  vendor/installers/PODxtLive_3_01.xtf vendor/installers/Variax300_3.10.vxf
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
```

No additional guitar settings capture is required for this reference-testing
stage. A future complete bank backup remains a separate preservation task requiring
specific authorization. Next coefficient work is to port and differentially test
body interpolation and pickup/electronics calculations in Rust, then integrate
the omitted Java transformations and full patch assembly. The physical path stays
locked throughout.
