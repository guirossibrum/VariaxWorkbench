# Acoustic output-trim quantization checkpoint

Issue #5 checks four consecutive binary32 values of the editable output trim
(`02010000/200000`) near −3.000037 dB. The presets use acoustic body
`03100012`, disabled pickups and tuning, and the existing synthetic body
definition at index 2 in `tests/fixtures/native-dsp-body-inputs.bin`. The Rust
editor accepts the values and the original Java reader preserves all 32 bits
into the native trim field at offset `0xbc`. The four mapped settings differ
only in that field. None of these trim values occurs in the 2,329-preset
corpus. The two Mach-O trim `powf` argument pairs are also absent from the
206,228-input math trace. See the [parameter domains](parameter-domains.md),
[preset corpus](host-wide-corpus.md) and [runtime audit](math-runtime-audit.md).

## Results

The table gives zero-based packed word 555, the Q23 trim gain with its
`0x800000` flag. Inputs are ordered from less to more attenuation.

| Runtime or profile | `c0400099` | `c040009a` | `c040009b` | `c040009c` |
| --- | ---: | ---: | ---: | ---: |
| Rust `host-std-v1` and `software-wide-v1/libm-0.2.16` | `8bda34` | `8bda34` | `8bda33` | `8bda33` |
| Mach-O i386 with glibc or MPFR-256 math imports | `8bda34` | `8bda34` | `8bda33` | `8bda33` |
| Original Windows PE32, 53- or 64-bit x87 | `8bda33` | `8bda33` | `8bda33` | `8bda33` |
| Same PE32, 53-bit x87 with 24-bit precision inside Q23 conversion | `8bda34` | `8bda33` | `8bda33` | `8bda33` |

Both Linux x86-64 Rust debug CLI builds matched all four complete
3,276-byte, Java-overlaid programs from each Mach-O adapter. The adjacent pair
`c040009a`/`c040009b` changes only word 555 and the checksum at word 1091. The
outer neighbors confirm the two plateaus. Native packer checksums and final
Java checksums were checked with their respective word ranges. The PE32
programs matched Rust on zero of four complete files in each mode. Within
the narrowed PE32 run, the earlier crossing changes only word 555 and its
checksum.

The Mach-O trim path divides the adjusted dB value at binary32 width, passes
it to `powf`, stores a binary32 gain, and uses scalar SSE instructions in
the positive Q23 path. Its two observed gain inputs are `3dbda339` on the
first two presets and `3dbda335` on the last two. The PE32 trim converter
receives `3dbda339` on the first preset and `3dbda335` on the other three in
all three tested modes. At the shared gain `3dbda339`, widening the PE32
conversion alone retains `gain × 8388607 + 0.5 ≈ 776755.969904` before
truncation. With 24-bit precision the same calculation rounds to
`776756.0` before truncation. This accounts for the converter difference at
the same gain input. The earlier crossing with narrowed PE32 also exposes a
difference upstream of quantization. The follow-up trace below isolates
that difference to the first acoustic trim addition.

## Why PE32 changes gain one input earlier

Both generators use analog selector 2, an effective analog offset of exactly
−10.5 dB, and the same binary32 continuous-gain operand `c0e560e1`. Mach-O
loads −10.5 as binary32, adds the input trim with scalar SSE, then adds the
continuous gain with a second binary32 addition. PE32 forms the offset as
zero minus binary64 +10.5, then adds the input trim and continuous gain in
x87 before the first binary32 store of the sum. The additions on these inputs
are exact at both tested x87 precisions, 53 and 64 significand bits. The
zero-dB clamp passes all four sums through.

| Input trim bits | Mach-O first sum | Mach-O adjusted dB | PE32 adjusted dB |
| --- | ---: | ---: | ---: |
| `c0400099` | `c1580026` | `c1a5584b` | `c1a5584b` |
| `c040009a` | `c1580026` | `c1a5584b` | `c1a5584c` |
| `c040009b` | `c1580027` | `c1a5584c` | `c1a5584c` |
| `c040009c` | `c1580027` | `c1a5584c` | `c1a5584c` |

For input `c040009a`, the exact first sum is −28311629/2097152 dB.
Retaining it through the second addition gives −21672087/1048576 dB,
exactly halfway between `c1a5584b` and `c1a5584c`. The PE32 store rounds
to even, selecting `c1a5584c`. Rounding the first sum to `c1580026` gives
a second sum of −43344173/2097152 dB before rounding, one quarter of a
binary32 ULP toward zero from that midpoint. Mach-O therefore selects
`c1a5584b`.

| Adjusted dB bits | Binary32 exponent after division by 20 | Binary32 gain after power |
| --- | ---: | ---: |
| `c1a5584b` | `bf8446a2` | `3dbda339` |
| `c1a5584c` | `bf8446a3` | `3dbda335` |

Mach-O divides at binary32 width and passes binary32 base 10 and exponent to
`powf`. PE32 divides in x87, stores the exponent as binary32, and reloads it
for the original CRT intrinsic. The initialized SSE2 branch receives base 10
and the exactly widened exponent as binary64 and returns a binary64 result
through x87. Both paths store the gain as binary32 before Q23. For each shared
exponent the original PE32 and both Mach-O adapters arrive at the same gain
bits.

The follow-up replay used the retained Java-mapped settings and synthetic body.
Across the five baseline configurations, all 20 native outputs matched the
prior replay byte for byte. In 12 controlled executions, substituting each
platform's adjusted dB into the other moved the exponent and gain crossing
with that value. Rounding only PE32's first addition to binary32 moved its
crossing to the Mach-O input as well. The PE32 diagnostic runs used the prior
24-bit Q23 conversion to make the gain crossing visible in word 555.
Relative to each platform's own corresponding baseline, each intervention
changed only words 555 and 1091 on input `c040009a`.

## Provenance and replay

The existing `initialized-host-libm-v2` oracle ran the pinned Workbench
i386 Mach-O instructions in Unicorn 2.1.4 with x87 control `0x037f`, MXCSR
`0x1f80`, and glibc 2.44 math imports. The MPFR 4.2.2 comparison substituted
256-bit working precision with rounding to the original import widths. Each
case used a fresh initialized engine. The independent Windows run executed
the pinned Workbench 1.75 PE32 generator, embedded CRT and packer in the same
emulator, with initialized SSE2 dispatch and x87 control `0x027f` or `0x037f`.
The controlled rerun used `0x027f` except for the converter, where it used
`0x007f`. Original PE32 OS imports stayed trapped. The Java file reader and
overlay used the existing local harness with its fake coefficient engine.
See [native startup](native-runtime.md) and [PE32 controls](factory-program-windows-precision.md).

The vendor-free regression reopens the edited synthetic preset and checks the
four target words plus the complete packed-word difference between neighbors.
Run `cargo test --offline --no-default-features --test host_render` and
`cargo test --offline --no-default-features --features portable-math --test host_render`.
The original-instruction replay uses the existing `CoefficientOracle`,
`WindowsCoefficientOracle` and `VerifyHostPipeline` with the four fields
above. The assignment's ignored evidence directory holds the local replay
script, exact inputs, outputs and hash report.

The width trace is retained locally in
`backups/5-acoustic-trim-widths-assignment59/`. From the lane checkout, use
the main checkout as `main` and run:

```sh
PYTHONPATH=scripts PYTHONDONTWRITEBYTECODE=1 "$main/.tools/python/bin/python" \
  "$main/backups/5-acoustic-trim-widths-assignment59/trace.py" \
  "$main/backups/5-quantization-threshold-assignment54/delivery-e7badf6/replay" \
  "$main"
```

The trace reads the native arithmetic at the generator, the power call and
gain store, and the Q23 boundary. It checks the exact midpoint with rational
arithmetic, verifies all five prior packed baselines by bytes and hash, and
checks all three interventions against the appropriate same-platform packed
outputs. The private trace and prior artifacts use separate directories.

These results establish a local coefficient threshold under the stated
software and emulator controls. Historical Mac runtime authentication, the
controls of a live historical Windows JVM, other target/compiler settings,
and audio and physical equivalence remain open under the existing scope.
