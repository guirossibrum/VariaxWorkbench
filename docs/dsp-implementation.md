# Remaining DSP implementation

Issue #3 is In Progress. [The recovered inventory](dsp-inventory.json) lists
17 DSP blocks, 59 hardware-word procedures and shared string-EQ helpers, bound
to the original library hash. Most block math and word ownership remain pending.
A procedure count is not test or audio coverage.

`src/dsp/patch.rs` now provides a logical 1,091-word `NativePatch` with no device
transport. It implements 15 scalar/control setters, native VX float bit encoding,
and the existing packer/checksum. The body-resonator conversion was already
verified separately. No complete coefficient engine or uploadable host patch is
claimed; Java metadata and host orchestration are separate later stages.

New original-machine-code evidence: 1,012 IEEE bit patterns (including signed
zeros, subnormals, infinities and NaNs) match VX encoding exactly. 297 scalar
setter cases span three seeded surrounding-word states and compare the entire
1,091-word array. Synthetic fixtures record native changed words while tests
also assert that every other word remains unchanged.

Scalar fields include block masks, coil counts/apertures/scale, resonator and EQ
counts, low-cut order, analog level, reverb delays/control sources, drone counts
and saved tone position/function. Supported index/count/normalized-control
bounds are checked before mutation. Those bounds are not comprehensive model
validation. The raw VX encoder preserves bit patterns rather than rejecting
nonfinite values because it performs only integer bit transformations.

Next stages are fixed-point coefficient/table setters, block-specific math,
tuning/string processing, an explicit word-ownership map and full all-block
assembly. Native saturation and asymmetric negative rounding must be preserved;
unknown behavior stays unimplemented. Hardware locks remain unchanged.

Reproduce the new primitive evidence with:

```sh
.tools/python/bin/python scripts/verify_dsp_scalar.py
cargo test --all-features
cargo test --release --lib dsp::
```
