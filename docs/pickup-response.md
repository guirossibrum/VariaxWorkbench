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
