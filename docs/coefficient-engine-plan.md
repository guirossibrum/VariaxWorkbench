# Completing the coefficient engine

Updated 2026-11-15. This plan does not authorize any USB/MIDI traffic.

## Current reference limitation

The previous whole-generator/JNI output baseline is invalidated: the loader omitted
native module initialization, leaving the tone/volume taper constants zero.
All 26 native initializers now execute; 110 versioned host-libm generator cases
and four JNI/direct comparisons pass. Historical runtime rounding remains open
in #5. See [current evidence](native-runtime.md). Standalone
body, circuit combination, complex transfer and primary filter-fit checks retain
their documented scope. See [the finding](pickup-response.md#whole-generator-reference-invalidation).

## Inputs already available

Workbench 1.75 Java code, original i386 coefficient library, all bundled model
resources, native function bodies, preset parsers, selected machine-code oracles,
and one real captured current-preset metadata example. Firmware is useful for
understanding the receiving DSP and defaults, but is not a prerequisite for
porting the host-side coefficient engine. Additional live presets are not a
prerequisite either: offline files and synthetic parameter variations supply
reference inputs.

## Remaining implementation and acceptance criteria

1. **Build an end-to-end original-library oracle.** Reproduce the in-memory
   model/patch objects and required math/runtime imports for
   `GenerateHWPatchFromVWPatch` (0x2c40) / `GenerateHardwarePatch` (0x23a0).
   Existing emulation covers selected primitives, not this complete call chain.
   Fail on unresolved dependencies; no USB, JNI device constructors or OS device
   services. Record artifact hashes, input bytes and every reference output.
2. **Finish body schema and interpolation.** Type the remaining 2,710 bytes per
   variant, establish units and interpolation modes/weights from original code,
   reproduce `L6VWBodyModel::Interpolate` (0x9d60) and
   `L6VariaxPatchInterpolation::Interpolate` (0x94f0). Verify endpoints, transitions
   and all 28 bodies, including bodies without a middle pickup.
3. **Port pickup and electronics calculations.** Reproduce response generation
   (`GeneratePickupResponseTables`, 0x25d40), position/angle, string/pole geometry,
   pickup combinations, phase/series wiring, volume/tone resistance, capacitance,
   taper and output scaling. Establish sample-rate/constants and lookup-table
   behavior from native code. Compare intermediate tables as well as final bytes.
4. **Complete hardware-patch assembly.** Account for every emitted DSP word,
   coefficient conversion, fixed-point range/rounding rule, string tuning/detune,
   metadata overlay, checksum and packing order. Packing alone is insufficient.
   Trace which settings belong to which stage instead of assuming all are DSP
   coefficients. Reject unsupported/nonfinite inputs deliberately.
5. **Differential validation in Rust.** Compare complete reference outputs for
   all selectable bodies/pickups, representative pairings and wiring states,
   documented endpoints, near-boundary values and deterministic parameter sweeps.
   Exact final bytes are the target; investigate each mismatch and do not hide
   quantization differences behind a broad float tolerance. Add malformed-input
   and repeatability checks. This is finite test coverage, not proof over every
   possible float or a physical audio-equivalence test.
6. **Integrate only offline.** Editor saves can produce inspectable local patch
   artifacts once validated. Add dry-run packet transcripts, size/product checks
   and simulated receiver rejection cases before any separate hardware proposal.
   Physical access remains locked and there is no upload/flash permission.

## Settings captures

No further hardware capture is required for steps 1–5. Original factory presets,
if extracted reliably from the supplied firmware, would provide additional real
reference inputs. Later, a separately authorized full current-bank capture would
help preserve the user's instrument and compare modified slots against factory
references. That backup is a distinct task, not a requirement to derive the
coefficient mathematics and not an automatic consequence of this plan.

## Progress after initial plan

The complete native generation/packing call chain now runs in the private oracle
with all 71 model definitions and explicit bounded memory/host-libm adapters.
Four original-JNI mapping cases agree with direct generation. See
[offline-generator-results.md](offline-generator-results.md) for scope and the
validation ledger. Java ApplyHacks/metadata overlay and historical-libm rounding
remain outside this reference milestone. Rust interpolation/electronics/complete
assembly remain unported. Both firmware files now unpack with matching embedded
image MD5; physical access stays locked.

Issue #1 progress: all body fields are now typed, pickup normalization matches
748 native comparisons, and routing/nesting matches 600 traced native plans.
The plan deliberately does not execute per-field blending; implementing and
validating InterpolateBetween remains the next sequential task. See
[body-definitions.md](body-definitions.md).

## Body interpolation completed

Issue #1 now includes actual field evaluation, verified bit-for-bit for 252
vendor-body outputs and 84 synthetic outputs in debug and release. All 17 sections,
disabled transitions and native arithmetic order are covered; the host-libm
limitation remains explicit. See [body-definitions.md](body-definitions.md).
Next sequential implementation topic: pickup/electronics response (#2).
