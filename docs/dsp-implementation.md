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
zeros, subnormals, infinities and NaNs) match VX encoding exactly. 315 scalar
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

## Coefficient writers and block assembly

The next milestone passes 927 table/scalar coefficient cases and 1,218 block
coefficient cases (16 additional routines), all comparing the full seeded
1,091-word array. 54 of the 59 inventoried hardware-word procedures now have
implemented primitives with documented native comparisons. Constructor/checksum/
name/getter-specific procedure coverage remains separate from those counts.

Native word ranges overlap between some blocks; the Rust writers retain the
original offsets rather than assuming each named block has disjoint storage.
The native trim writer always sets bit 0x800000, including small negative values
whose Q23 rounding retains a carry bit. A failing native comparison identified
that detail and it is now preserved. The original six-band string-EQ capacity
also corrected the preliminary five-band analysis guard; the versioned scalar
fixture now includes the sixth band and passes 315 cases.

Complete pickup UpdateHWPatch assembly passes 360 geometry and 960 response
cases with unmodified original setters. Source snapshots are synthetic only;
all 1,091 words are compared, including surrounding state. Invalid block inputs
leave the previous Rust patch unchanged.

Volume, mono mix, global-decay, drone, pitch-shift and microphone-position
assembly/preparation pass a further 1,008 cases at 31250/44100/48000 Hz. These
include enabled/disabled states, volume trim/analog branches, drone-count
clamping, drone zero/negative frequencies, fractional/clamped semitone offsets,
minimum/large integer delays, wet/dry mixing and microphone-position scale
normalization. The microphone cases distinguish runtime prepared-field writes
from raw-position preparation; both stages have their own comparisons.

66 all-feature Rust tests, Clippy, formatting, release DSP fixtures and optimized
builds pass. Body resonance, low-cut, string EQ/tone, 12-string EQ, sitar, reverb,
pick filter and compressor DSP parameter calculations still require porting and
validation. Full word ownership, call-order overlays and all-block assembly remain
pending. Passing decay/pitch parameter-to-word tests is not audio sustain/bend
validation; that remains the independent audio matrix in #33.

Additional reference scripts are `verify_dsp_tables.py`,
`verify_dsp_coefficients.py`, `verify_dsp_pickup_blocks.py` and
`verify_dsp_simple_blocks.py`. Analysis snapshots are private numeric layouts,
not a supported preset or upload format. Hardware locks remain enabled.
