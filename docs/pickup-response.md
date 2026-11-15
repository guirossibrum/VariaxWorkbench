# Pickup-response engine: sequential progress

Issue [#2](https://github.com/guirossibrum/VariaxWorkbench/issues/2) is In Progress.
The passive circuit-combination primitive is implemented and verified; the full
pickup/electronics engine is not complete. GitHub is the work-tracking source of
truth. No hardware access or writer is involved.

## Implemented component

`src/models.rs` provides `PickupCircuit`, `PickupWiring` and `CombinedPickup`.
`PickupCircuit::from(&PickupDefinition)` copies the four stored electrical values.
`combine` follows the original `L6VariaxPickupResponse::CombinePickups` at `0x23b30`:

| Behavior | Native rule preserved in Rust |
| --- | --- |
| Disabled pickup | Inductance exactly zero, including negative zero |
| One disabled input | Copy the other circuit verbatim |
| Both disabled | Copy the right circuit and return inactive |
| Series | Sum DC resistance and inductance; sum absolute magnetic resistances; combine capacitances by product over sum |
| Parallel | Product over sum for DC resistance, inductance and absolute magnetic resistance; sum capacitances |
| Active result | Resulting inductance compares unequal to zero |

The sole-active-copy rule does not normalize the copied magnetic resistance's
sign. Arithmetic order is retained; no algebraic reassociation or broad float
tolerance is used. The analysis CLI `combine-pickups LEFT.bin RIGHT.bin
series|parallel` prints the active flag and exact float64 result bits.

This is a compatibility calculation on raw circuit values, not a claim that every
input describes a valid physical pickup. Zero denominators and nonfinite values
follow IEEE arithmetic. Parameter restrictions and the original generator's
pre-combination clamps belong to subsequent validation/pipeline stages. The raw
helper does not implement phase, magnetic geometry, response filters, control
knobs or device DSP execution.

## Verification

`scripts/verify_pickup_combinations.py` executes the original i386 combination
routine directly inside bounded Unicorn memory. It needs no memory, math, file,
network or device **runtime import adapters** for this routine; it only supplies
input/output objects and initializes MXCSR to `0x1f80`. It checks all four output
float64 bit patterns plus the returned active flag against Rust.

- All **43 × 43 × 2 = 3,698 vendor-definition pairings** pass.
- **128 synthetic cases** pass, including disabled inputs with nonzero stored
  fields, negative magnetic resistance, zero denominators and magnitude boundaries.
- The synthetic vectors are committed and replayed in Rust unit tests; no vendor
  model data or derived vendor-pair outputs are committed.
- The focused Rust combination tests also pass in optimized release mode.

Local evidence: `analysis/reports/pickup-combination-agreement.json`. Source native
library SHA-256: `3aa232329d467101637764397b77a7b8ffa6af0ccd72ec380d02f145477ba9fe`.

## Remaining implementation

Recover and port the response tables and filter-parameter fitting, then pickup
position/angle/pole/coil geometry, phase and wiring interactions, volume/tone
resistance/capacitance/taper/load behavior, and the original pipeline's clamps and
model-specific adjustments. Compare intermediate tables and complete outputs
against original code before treating the pickup engine as complete. Successful
raw circuit tests do not establish audio equivalence or physical safety.

```sh
.tools/python/bin/python scripts/verify_pickup_combinations.py
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
cargo test --release --lib models::
```

Keep equipment disconnected and every source-level hardware lock enabled.

## Complex transfer and primary filter fitting

`src/pickup.rs` now implements the complex circuit transfer (original `0x24be0`)
and the primary peak/half-power frequency/Q/gain search (`0x259a0`). The circuit
solver preserves native complex determinant grouping, explicit zero terms and
addition order. A one-bit matrix mismatch identified an association difference;
that was fixed using disassembly rather than a numeric tolerance.

New independent comparisons:

- **2,322 complex transfer outputs** across all 43 vendor definitions, six
  frequencies and nine volume/tone combinations: both float64 components exact.
- **387 primary filter fits**: frequency, Q and gain all bit-exact.
- **540 synthetic vectors** across different circuit/wiring values are replayed
  in Rust tests. Only generated values and their outputs are committed.

These routines execute original SSE arithmetic without external runtime imports;
they do not depend on the uninitialized globals described below. The primary fit
requires finite volume in (0,1] and tone in [0,1], preserves the original bounded
search and numerical edge behavior, and does not certify physical/audio validity.
CLI `pickup-response` and `pickup-filter1` expose the fixed-wiring analysis cases.

Secondary fitting, full response tables, control taper handling, geometry and
complete pipeline integration remain pending. Verify with
`.tools/python/bin/python scripts/verify_pickup_response.py`.

## Whole-generator reference invalidation

During the secondary/table investigation, static initializer `0x33780` was found
in the original Mach-O initializer table. It initializes `kMultVolume` to
`pow(10,-2)` and `kMultTone` to `pow(10,-3)`, as well as translation-unit constants.
The existing whole-generator oracle had not executed that module constructor;
these values remained zero. Its earlier 110 completed executions are therefore
**not valid initialized-library reference results**. The prior report is archived
locally and marked invalidated. The generate entry point now rejects zero knob
globals rather than silently emitting misleading reference data.

Issue #5 must audit/implement required native module initialization and rebuild
whole-generator/JNI reference evidence before those outputs are used as a baseline.
This finding does not affect the standalone body interpolation, raw combination,
complex transfer or primary-fit comparisons: their recovered code paths do not
use those globals. It also does not change any hardware state; all work remains
inside private emulator memory and offline files.

## Initialized reference follow-up

The startup defect above is now fixed by executing all 26 original initializer
entries. All 52 translation-unit complex constants and 53 captured destructor
registrations are checked. A new versioned 110-case host-libm generator sweep and
four original-JNI comparisons pass; the old outputs remain invalidated. See
[native runtime evidence and remaining limits](native-runtime.md). Historical
math parity is still pending in #5; complete pickup tables/geometry remain in #2.

## Complete electrical tables and smoothing

The Rust electrical-response stage now includes original bandpass transformation,
float32 complex response, residual secondary fitting, the 64-double response
tables and CalculateResponse smoothing/conversion. New differential passes:

- 180 bandpass transforms and 720 complex responses, synthetic inputs at three
  sample rates (900 comparisons).
- 282 secondary fits across 43 bundled definitions and four synthetic circuits.
- 248 complete 64-double tables: all 43 definitions, selected two-pickup pairs,
  four synthetic circuits, both wiring modes and both volume tapers.
- 188 calculated-response outputs, each comparing all 83 assigned float fields,
  across all 43 definitions, synthetic pairs, both tapers and positive/negative
  tone-capacitance handling (the original frozen-tone convention).

Only synthetic inputs/outputs are committed: 900 biquad, 24 secondary-fit,
32 table and 16 calculated-response vectors. They pass both debug and optimized
Rust regression tests. Native comparisons use the initialized v2 host-libm
harness and are not historical macOS rounding or audio-equivalence proof.

The native table generator's backward search can read before its gain array
when no gain exceeds 0.2. Rust bounds this search and preserves the resulting
skipped-fill outcome. Inactive circuits return None rather than emitting tables
from uninitialized stack data. Supported analysis sample rates are bounded to
1..=384000 Hz; this numerical bound is not a device support claim.

The first residual-fit comparison exposed reversed derivative bounds in the new
port. Correcting the branch direction restored all comparisons; no tolerance was
added. The original primary/secondary search quirks, float32 complex multiply,
positive-imaginary Reciprocal and smoothing behavior are retained.

Pickup placement/pole/coil geometry, phase projection, level/trim orchestration,
host model overrides and final packed engine integration remain pending. The
electrical module does not generate audio or authorize hardware traffic.

Reproduce with `scripts/verify_pickup_biquad.py`,
`scripts/verify_pickup_secondary.py`, `scripts/verify_pickup_tables.py`,
and `scripts/verify_pickup_calculated.py` using the local Python environment.
