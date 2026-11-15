# Coefficient math profiles

Normal builds retain the `host-std-v1` coefficient profile. An explicit
`portable-math` build selects the experimental
`software-wide-v1/libm-0.2.16` profile. This is not a hardware-access switch.
Both profiles keep all USB/MIDI/audio access locked.

```sh
# Identify the profile in the current binary.
cargo run --bin variax-workbench -- math-profile

# Build the experimental profile, with no GUI or USB feature needed.
cargo build --locked --features portable-math --bin variax-workbench
target/debug/variax-workbench math-profile
```

Selection is fixed at build time, including for stateful host generation. The
same preset parameters can produce different coefficient words under different
profiles; record the profile when comparing outputs. `src/math.rs` is the single
boundary for coefficient transcendental functions and square root. A source
guard rejects direct calls that bypass it. Formats, block ordering, field types,
quantization, checksums and hardware-access policy do not change.

## Software profile contract and dependency

The profile preserves float32 arguments, evaluates the function with software
float64 algorithms, and rounds back to float32. Float64 inputs/results remain
float64. This covers the 44 recovered math call sites, including square root.
No fused operation or expression regrouping is introduced. Wider intermediates
do not guarantee correct rounding for arbitrary float32 inputs: double rounding
remains possible. Tests assume normal nearest rounding with subnormal flushing
disabled, as do the reference audits.

The optional dependency is exactly `libm` 0.2.16, with default features disabled
and `force-soft-floats` enabled. The algorithms compile into the program; they
do not require Linux libm, MPFR, Python, Java, Monkey or an original Line 6
library at runtime. Cargo records the archive checksum. Research helpers verify
the archive and all 155 archived files, and independently check Cargo's resolved
source. The crate has no production transitive dependencies. License and embedded
source notices are preserved in [third_party](../third_party/README.md).

Architecture-specific and intrinsic paths are disabled by this feature even if
another dependency also enables the crate's default `arch` feature. This is the
behavior of the pinned crate's checked build configuration. Ordinary float
arithmetic and program I/O still use the target CPU and OS. Replacing/updating
the math implementation requires a new profile ID and repeated reference checks;
the rest of the coefficient engine calls only this small boundary. Complete
source vendoring/reproducible distribution remains #10/#11 work.

## Offline evidence

The original instructions and the Rust implementation share the selected math
adapter in these comparisons. This independently checks their surrounding
algorithms, not the math library against itself. Separate MPFR and embedded
Windows CRT comparisons provide additional mathematical/runtime evidence;
see [the runtime audit](math-runtime-audit.md).

- All **108 Rust tests** pass with `--all-features` (software profile), and all
  **108 pass with `--features gui,usb`** (host profile). Clippy with warnings
  denied and formatting pass. The six profile-specific expected-output files
  preserve exact comparisons; the original host-profile fixtures are unchanged.
- Both debug and release product binaries match **110 complete numeric patches**
  under their corresponding original-instruction math adapters. The host profile
  retains all existing canonical outputs. The software profile matches the
  independent original-instruction software candidate, MPFR-256 candidate and
  initialized Windows-math candidate for that corpus.
- Both profiles' debug/release binaries match **48 complete synthetic-resource
  cases**. These are existing inputs, not additional distinct samples for #33.
- The software product passes **144 complete original Java/native preset-file
  comparisons** in debug and release, including 24 special-combination cases,
  original reader/ParamMap adjustments and final metadata/checksum overlay. A
  host-profile replay also retains all 144 existing references. The two special
  response snapshots are byte-identical across the tested profiles.
- **828 component cases** pass against original instructions with the matching
  software adapter in debug and release: 600 biquads, 64 pickup tables, 80
  smoothed responses and 84 body interpolations. There are 59 changed raw cases
  versus the host-profile expectations. The initial generated software variant
  therefore failed four old-profile fixture tests (88/92 library tests passed).
  Those failures remain recorded; the new profile's independent exact reference
  values resolve the profile mismatch without adding numerical tolerances.
- Undefined-symbol checks of the software CLI binaries find no imports of the
  tested host transcendental, square-root or FMA functions. The host-profile
  control binaries retain their expected math imports. This is not a claim that
  the executable has no ordinary OS runtime dependencies.

Evidence above is local Linux x86_64 evidence. The new `Offline core` workflow
checks the software profile's synthetic-only core/file tests in debug and release
on Ubuntu, Windows and macOS runners with a pinned Rust compiler. Cross-target
results are pending until that workflow actually completes. It does not compile
USB/GUI features, obtain vendor artifacts, or contact a device.

## Reproduction and remaining limits

```sh
python scripts/check_coefficient_math.py
cargo test --all-features
cargo test --features gui,usb
cargo clippy --all-features --all-targets -- -D warnings
cargo clippy --features gui,usb --all-targets -- -D warnings
.tools/python/bin/python scripts/audit_product_math.py
.tools/python/bin/python scripts/audit_engine_math_components.py
```

The optional audits require separately supplied local original modules/resources
and the research math helpers. They never load original libraries as host
processes or provide USB/MIDI/audio services to emulated code. Private receipts,
binary copies, original-derived outputs and resources stay under ignored
`.tools/` and `analysis/` paths. `product-engine-math-v1/<source-fingerprint>/`
receipts bind compiler, source/manifest/lock hashes, binaries, profile and outputs.
The generated-engine experiment remains reproducible from the explicitly pinned
pre-profile Git revision in `build_engine_math_variant.py`; it is a control
experiment, not a copy of future production changes.

Historical Mac libSystem behavior is not authenticated. Full Windows
coefficient/JVM execution, broader model/parameter boundaries, other compiler/CPU
settings, audio/DSP behavior and hardware safety remain unverified. The software
feature is experimental and does not authorize a preset upload or update. Issue
#5 remains open while its remaining runtime/platform limits are tracked; #6
continues broader engine validation after its dependencies are satisfied.
