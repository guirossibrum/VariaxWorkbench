# Original Workbench body definitions

`src/body.rs` decodes the complete original body-resource header and all 17 variant sections in Rust. This is offline resource decoding. It does not communicate with a guitar, generate a device patch, restore factory presets or claim that body defaults equal the factory contents of a Variax 600.

## Native verification

Run `.tools/python/bin/python scripts/verify_native_bodies.py` from this checkout. It executes the original i386 `L6VWBodyModel::ReadWrite` at `0x99c0`, `ReadWritePDef` at `0x9930`, the nested `VCEVariaxPatch::ReadWrite` at `0xa1f0`, numeric stream readers and array loops in Unicorn. The Mach-O source digest is checked by `NativeOracle`; the reference SHA-256 is `3aa232329d467101637764397b77a7b8ffa6af0ccd72ec380d02f145477ba9fe`.

The emulator supplies bytes from bounded local memory through a synthetic stream's `ReadByte`. Its only patched original function is `StringX::Format` at `0xd500`, replaced by `ret` in private emulator memory because binary `SerialPersistance` ignores field/group labels. Original StringX construction/destruction still executes. This is a deliberate adapter, not a claim of unmodified whole-application execution. No OS service, hardware, original UI or running application is involved.

For all 28 bundled body files, the verifier observes 1,421 serialization calls, checks contiguous complete input coverage, reads every corresponding native destination field, reverses each multibyte value to canonical big endian order, and compares the result with the full input. Each file is 17,483 bytes. All 28 yield identical field layouts. A separately generated synthetic fixture with distinct values also passes the original reader. Its bytes are generated from the observed schema; the committed fixture does not contain an original vendor body resource.

Evidence:

- `analysis/reports/native-body-schema.json`: all field names, types, widths, counts, file offsets and native object offsets.
- `analysis/reports/native-body-serialization.json`: source digest, adapter disclosure, resource hashes and fixture digest.
- `tests/fixtures/native-body-definition.bin`: synthetic test input.

## Verified file layout

Integers and IEEE-754 floating-point values are big endian. Booleans occupy one byte and the native reader treats every nonzero value as true. There is no resource header signature or version field before these fields.

| File offset | Bytes | Field |
| --- | --- | --- |
| 0 | 1 | `has_middle` |
| 1 | 32 | bridge pickup defaults |
| 33 | 32 | middle pickup defaults |
| 65 | 32 | neck pickup defaults |
| 97 | 4 | interpolation mode, unsigned raw representation |
| 101 | 6 × 2,897 | variants named `bb`, `mb`, `nb`, `mm`, `nm`, `nn` by the original reader |

Each pickup default consists of position float32, angle float32, then six pole-offset float32 values. Defaults are serialized even for pickups the model does not contain. Interpolation-mode values 0 through 4 occur in bundled resources. Their selection/weight rules are now verified against the native routing helpers, as described below; the per-field operation is now implemented separately in `src/body/interpolation.rs`.

The native object's pickup defaults begin at `0x4b50`, `0x4b70`, `0x4b90`, its mode at `0x4b4c`, and `has_middle` at `0x4bb0`. Native variants begin at object offsets `4 + i * 0xc8c`. Native structure size/padding must not be used as serialized size.

Within each serialized variant:

| Relative offset | Bytes | Field |
| --- | --- | --- |
| 0 | 1 | body resonance enabled |
| 1 | 12 | twelve resonator enable flags |
| 13 | 48 | twelve float32 frequencies |
| 61 | 48 | twelve float32 Q values |
| 109 | 48 | twelve float32 gains |
| 157 | 4 | treble frequency |
| 161 | 4 | rolloff frequency |
| 165 | 4 | master |
| 169 | 4 | body mix |
| 173 | 4 | mid mix |
| 177 | 4 | treble mix |
| 181 | 1 | low-cut enabled |
| 182 | 4 | low-cut frequency |
| 186 | 1 | low-cut fourth-order flag |
| 187 | 745 | string EQ: six strings, each with a level and six typed bands |
| 932 | 241 | twelve-string EQ: six strings with two bands each |
| 1,173 | 541 | string tone: six strings, each with six resonators |
| 1,714 | 73 | global decay: hold-off, decay and minimum arrays |
| 1,787 | 24 | six sum-to-mono string levels |
| 1,811 | 45 | volume curve, raw analog gain and continuous gain |
| 1,856 | 101 | reverb delays, density/decay tables and control sources |
| 1,957 | 105 | drone count/level and twelve frequency/feedback entries |
| 2,062 | 121 | sitar-buzz per-string parameters |
| 2,183 | 97 | pitch-shift tune/delay/mix/level arrays |
| 2,280 | 261 | pickup-response tables and scalar parameters |
| 2,541 | 153 | pickup geometry: four coils, six poles/phase flags per coil |
| 2,694 | 49 | pick-filter frequency/Q/decay and gain table |
| 2,743 | 45 | compressor attack/release and post-gain table |
| 2,788 | 109 | microphone-position, notch-depth and random-level tables |

## Rust API and limits

`BodyDefinition::parse` requires exactly 17,483 bytes before any field access, returns three pickup defaults and six fully typed variants, and retains all input bytes. `original_bytes()` returns the original resource; `variant_bytes(index)` returns a bounded original variant. Changes to public decoded fields do not alter either byte view. `canonical_bytes()` serializes the typed view for offline comparison or an edited local copy; it normalizes true booleans to byte 1. `original_bytes()` retains noncanonical boolean bytes exactly. No method transmits to a device.

The parser follows native boolean behavior and retains floating-point bit patterns, including NaNs. Successful parsing verifies structure/length, not whether a resource is sensible for signal processing. Rust tests exercise distinct synthetic values across every exposed field and variant, every shorter input length, excess bytes, out-of-range variant indices, noncanonical true values and NaN payload preservation.

## Position normalization

`normalized_pickup_position` implements the original SSE float32 mapping at
`0x9b60`. It clips physical positions at bridge/neck to 0/1. Bodies without a
middle pickup use linear interpolation across that interval; bodies with a middle
pickup map the middle to 0.5 with a separate linear segment on each side. The
original UI uses inches for pickup position; this API preserves the stored units
and introduces no additional conversion. Invalid/nonfinite anchors, overflowing
spans and nonfinite inputs are rejected rather than propagated into calculation.

`verify_body_rust.py` compares every typed field against the original native
serializer for all 28 vendor bodies and a synthetic fixture, and makes **748
bit-exact normalization comparisons** including anchors, adjacent float values,
outside positions and bodies with/without a middle pickup. Normalization executes
original instructions without math/runtime adapters. An x87 return-capture stub
runs only in private memory. No original normalization instruction is patched.

## Interpolation routing and field evaluation

`BodyBlendPlan` represents the exact nested calls selected by original helpers:

| Mode | Native routing rule |
| --- | --- |
| 0 / unknown | Copy `bb` |
| 1 | Blend `bb` to `nn` at the average of the two normalized positions |
| 2 | Blend `bb` to `mm`, then `mm` to `nn`, according to the position sum |
| 3 | Sort the positions and use triangular interpolation over `bb`, `nb`, `nn` |
| 4 | Sort/double positions; use a lower triangle, a middle bilinear region, or an upper triangle over the six variants |

The exact nested operation order is retained even when a weight is zero or one;
nonlinear field interpolation may round differently from a direct endpoint copy.
The public normalized-plan API rejects nonfinite values or coordinates outside
0..=1. `BodyDefinition::interpolation_plan` first normalizes physical positions.

`verify_body_plans.py` executes original mode selection and triangle/bilinear
helpers, with **explicit trace adapters replacing InterpolateBetween and CopyFrom**
in private emulator memory. **600 plans** match Rust, including all modes,
unknown mode 37, both position orders, signed zero and adjacent values around
transitions. This proves routing/nested float32 weights for those cases, **not the
numeric interpolated body coefficients**. The synthetic fixture contains only
coordinates, variant indices and traced weights.

Frequency/Q/gain names, arrays and integer discriminants are preserved from the
native serializer. Their complete DSP-specific units, enums and blending rules
must still be established per block; typed decoding is not an assertion that all
physical semantics are known. The native `L6VariaxPatch::InterpolateBetween` operation is now ported, including
logarithmic frequency interpolation and enabled/disabled transitions. The separate
full-output comparisons below cover all 28 definitions. Host-library compatibility
does not establish device DSP execution or physical audio equivalence.
Factory presets and calibration remain separate evidence/recovery topics.

## Reproduce

```sh
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
.tools/python/bin/python scripts/verify_body_rust.py
.tools/python/bin/python scripts/verify_body_plans.py
.tools/python/bin/python scripts/verify_body_interpolation.py
.tools/python/bin/python scripts/verify_body_interpolation.py --release
```

Local reports: `analysis/reports/body-rust-native-agreement.json` and
`analysis/reports/body-interpolation-plan-agreement.json`. Ordinary Rust tests
replay the committed synthetic fixtures without any vendor files or hardware.
Analysis CLI commands `body-roundtrip`, `normalize-body` and `body-plan` expose
the same code for reference checks. File output uses the shared atomic,
non-overwriting save function; it never writes to an input file or device.

## Complete field interpolation milestone

`BodyBlendPlan::evaluate` and `BodyDefinition::interpolate_normalized` evaluate
all selected fields. An analysis CLI can save an interpolated variant locally:
`interpolate-body INPUT.bin OUTPUT.bin X_BITS_HEX Y_BITS_HEX`. `blend-body` exposes
a direct two-variant comparison. Both use atomic, non-overwriting saves and have
no device-access path.

The port preserves the native left-copy rule for discrete flags/control sources,
frequency interpolation, gain/mix transitions, filter-type mismatch behavior,
integer conversions and the native single-wrap pickup-coil rule. It does not
replace those behaviors with a generic arithmetic average. A disabled low-cut
transitions through the native constant 20; a fading resonator uses the native
stored-gain target -100. Other integer/table units are preserved as represented
by the original code; unconfirmed physical interpretations are not invented.

Disassembly corrected an ambiguous decompiler result: logarithms and weighted
products remain **float64**, with the complement weight first computed in float32.
The sum rounds to float32 before `expf`. Prematurely rounding each logarithm to
float32 caused measurable mismatches and was corrected. Linear interpolation
uses separate float32 subtraction, multiplication and addition.

`verify_body_interpolation.py` executes the actual original field interpolation
and routing; unlike the routing-only tracer, it does **not** replace field
calculations. Its explicit private memory/libm adapters also capture original
filter-type mismatch diagnostics without invoking host stdout or device APIs.
The CPU's x87 control/tag and MXCSR are initialized explicitly for reproducibility;
the earlier full-generator baseline and all 110 regression cases remain unchanged.

Validation in both debug and optimized release builds:

- **252 full outputs** from all 28 vendor bodies, every serialized field bit-exact.
- **84 full synthetic outputs** varying all sections, disabled transitions,
  signed-integer boundaries, filter types, coil counts and zero-frequency cases.
- **40 Rust tests**, Clippy with warnings denied and formatting checks pass.
- The **110-case** original generator regression, **4 JNI** comparisons and
  **600 routing** plans still pass.

Only synthetic input/output fixtures are committed. Vendor-derived outputs and
per-case evidence remain local under `analysis/reports/`.

Compatibility deliberately includes native numerical quirks: zero-frequency
logarithms can produce NaNs at endpoints, and an interpolation endpoint need not
be a byte-identical source copy. Raw nonfinite data can still be decoded; weights,
normalized coordinates and coil counts are checked at the calculation API.
Future complete-patch validation must decide which results are acceptable for
its intended context. No result here is an upload authorization or evidence of
physical safety. Historical macOS libm parity remains the separate math issue;
these comparisons use the disclosed host-libm oracle on this Linux system.

Issue #1 acceptance is satisfied for this declared scope. Completing the remaining
pickup/electronics and DSP/assembly components is still necessary before a full
Rust Workbench coefficient engine exists.
