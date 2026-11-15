# DSP coefficient implementation

All 17 recovered DSP block parameter-to-word paths are implemented in Rust.
This completes issue #3's component port. It does not complete the preset-to-patch
host pipeline (#4), historical-runtime parity (#5), full-engine validation (#6),
parameter-domain validation (#7), or independent rendered-audio tests (#33).
No hardware access is available or authorized.

`NativePatch` is a logical 1,091-word buffer with no device transport. The
[inventory](dsp-inventory.json) lists 17 blocks, 59 hardware-word procedures and
shared helpers bound to the original library hash. 54 hardware-word procedures
have direct primitive comparisons. The remaining five constructor/checksum/name/
getter procedure entries are explicitly labeled without that claim; zeroed
construction, packing/checksum and status access have Rust implementations, while
host name placement remains #4. Procedure counts are not coverage percentages.

## Original-instruction comparisons

All fixtures use synthetic inputs and compare complete seeded buffers, including
untouched neighbors, unless the row explicitly describes a scalar/helper output.

| Scope | Cases |
| --- | ---: |
| VX bit encoder, arbitrary IEEE patterns | 1,012 |
| Scalar/control setters, six-band EQ capacity | 315 |
| Table/scalar coefficient writers | 927 |
| Block coefficient writers | 1,218 |
| Pickup geometry / response assembly | 360 / 960 |
| Volume, mix, decay, drone, pitch, microphone | 1,008 |
| Generic biquad helper | 600 |
| Low-cut, pick-filter, compressor, reverb | 864 |
| Mixed-precision EQ band coefficients | 2,304 |
| Six-band / twelve-string EQ assembly | 216 / 216 |
| Sitar, including zero Q and signed zero | 288 |
| Body resonance / string-tone assembly | 216 / 216 |
| Complete raw-body all-block assembly | 216 |

The final row covers all 28 local vendor bodies × six variants, plus eight
synthetic bodies × six variants. Only the synthetic inputs and 48 synthetic
outputs are committed. Every one of 1,091 logical words matches. All eight
combinations of optional pick-filter/compressor/microphone overlays appear in
the synthetic corpus, retaining original call order and enabled-only conditions.
This finite corpus does not establish arbitrary-parameter or audio equivalence.

78 all-feature Rust tests pass, including full synthetic all-block regression,
malformed-input atomicity and nonfinite rejection. Clippy with warnings denied,
formatting, optimized DSP fixtures and the release build are required checks.

## Constants, precision and error behavior

The original engine constructor sets its default internal rate to **39062.5 Hz**
(bits `0x47189680`). Rust names this `LEGACY_SAMPLE_RATE`. Earlier 31250/44100/
48000 Hz vectors remain useful synthetic rate variations; they are not measured
hardware or USB-audio sample rates.

Arithmetic grouping and narrowing follow original instructions. Body resonance
uses double tangent/power while string-tone uses float tangent/power. Sitar uses
double exponential after a float rate-duration product, and double TAU operations
after a float frequency subtraction. String-tone's gain multiplication retains
`gain * (-damping * inverse)`. Initial one-word differences exposed incorrect
decompiler expression grouping; disassembly and native vectors resolved them.

Q23 preserves original saturation and asymmetric negative rounding. The trim
writer always sets bit 0x800000. Raw VX conversion accepts arbitrary IEEE bit
patterns as integer encodings. Public numeric block APIs reject nonfinite input
and unsupported ranges before committing a replacement buffer. Native zero-Q
sitar division can produce infinity; the original saturating output is retained
for finite zero-Q input rather than rejecting stock definitions. These guards
are a deliberate safe subset, not a claim that the original rejected bad data.

## Word ownership and scope

The [observed word map](dsp-word-map.md) indexes every word with overlapping
writers retained: 748 have CPU instruction-write evidence; 343 stayed zero in
the 110-case host-generator corpus and remain semantically unknown. They are
not claimed reserved, reassigned or used for new data. Raw all-block generation
starts from zero, so this corpus checks the resulting unknown words as well.

`NativePatch::from_body_variant` reproduces raw typed body update/generation order
with explicit optional pickup-response/geometry overrides. It excludes body
selection/interpolation orchestration, Java conversions/model hacks, names,
metadata and final host masks. `dsp-body-variant` writes an inspectable local
1,091-word file using non-overwriting saves, not an uploadable host preset.

The original i386 library runs in private CPU emulation with initialized v2
runtime and disclosed host-libm adapters. Matching that reference does not prove
historical macOS libm parity. These are host coefficient calculations, not a
firmware/audio DSP emulator. Sustain, bends and tuning sound remain #33.

## Reproduction

Run ordinary regressions without vendor files or equipment:

```sh
cargo test --all-features
cargo clippy --all-features --all-targets -- -D warnings
cargo fmt --check
cargo test --release --lib dsp::
```

With separately obtained local vendor artifacts and ignored tools/analysis,
reference scripts are `verify_dsp_scalar.py`, `verify_dsp_tables.py`,
`verify_dsp_coefficients.py`, `verify_dsp_pickup_blocks.py`,
`verify_dsp_simple_blocks.py`, `verify_dsp_filters.py`,
`verify_dsp_equalizer.py`, `verify_dsp_eq_blocks.py`, `verify_dsp_sitar.py`,
`verify_dsp_resonance.py`, `verify_dsp_body_variants.py`, and
`verify_dsp_word_map.py`. Analysis snapshots are private numeric layouts rather
than a supported preset format. Hardware locks remain unchanged.
