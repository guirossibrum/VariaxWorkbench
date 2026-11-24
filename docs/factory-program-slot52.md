# Slot 52 secondary pickup Q trace

**Confirmed in the offline references:** x87 intermediate precision in the
primary biquad response changes the secondary peak search at tone point 4.
That discrete search decision changes the raw Q by about a factor of two.
The Q fill and smoothing propagate it through the table. VX encoding reflects
the changed Q values, including the five exponent changes among the seven
large mantissa differences. [Issue #184, stage 1](https://github.com/guirossibrum/VariaxWorkbench/issues/184)

## Where the calculations diverge

All indices start at zero. The Rust component replay uses the combined pickup
circuit and wiring observed at the original Mac fit entry. Its point-4 fit,
nine unsmoothed Q values, nine writer inputs and all nine packed pairs agree
with the Mac trace. Separately, the normal Rust CLI render agrees with the
complete Mac program and its published hash. The two PE32 baselines reproduce
the published 53- and 64-bit program hashes. Both use the initialized SSE2 CRT
dispatch. [Windows precision study](factory-program-windows-precision.md)

At tone point 4 the control value is 0.03162277660168379, volume is 1 and
the circuit resonance limit is approximately 989.850884 Hz. The adjusted
primary filter given to the Rust replay and both the Mac and 53-bit Windows
fits has frequency 8,750 Hz, Q 0.18891497754177505 and gain
10,298.98341175162. The 64-bit inputs differ in the last few bits. Both
53-bit Windows and Mac produce identical primary bandpass coefficients at
this fit.

The search measures the magnitude of the pickup circuit response times the
inverse primary-biquad response at the midpoint and at ±10 Hz. The first
observed numerical difference inside the biquad at the **decisive left
sample, 1.599815048538 Hz**, is in the reciprocal of the polar unit sample.
The original Mac instructions and Rust round the squares, sum and division
to binary32, whereas the original PE32 reciprocal carries those operations
in x87 before storing the scale as binary32. The two polar components are
identical at this sample. The wider sum of squares is 0.9999999470092111
at 53 bits and the binary32 sum is 0.9999999403953552. Taking the reciprocal
and storing the scale gives `1.0` and `1.0000001192092896`, respectively.
Both store the scaled components as binary32. The subsequent complex
multiplication also uses different intermediate rounding. Its imaginary
component uses the three-product expression
`(a + b)(c + d) − ac − bd`. Near DC, the small terms are sensitive to
rounding at the additions and subtractions.

| Primary biquad intermediate at 1.599815 Hz | Rust / Mac | PE32 x87, 53 and 64 bits |
| --- | --- | --- |
| Polar sample (real, imaginary) | (0.999999940395, 0.000257329550) | (0.999999940395, 0.000257329550) |
| After scaling by reciprocal squared magnitude, `z` | (1, 0.000257329579) | (0.999999940395, 0.000257329550) |
| Squared sample, `z²` | (0.999999940395, 0.000514798681) | (0.999999821186, 0.000514659041) |
| Biquad numerator | (0.000488281250, −3.833984375) | (0.001464843750, −3.832916975) |
| Biquad denominator | (0.463726043701, −0.000252895057) | (0.463726103306, −0.000252832950) |
| Complex biquad response | (0.005561820697, −8.267776489) | (0.007665359881, −8.265471458) |

The polar, scaled and squared samples through the numerator and denominator
are native instruction snapshots. Rust uses the same binary32 arithmetic as
Mac here, and its directly measured biquad output matches Mac at all sampled
frequencies they share for this fit. The Mac biquad and fit start at
`0x23d30` and `0x252b0`. The PE32 biquad, complex reciprocal and fit start
at `0x1000d5e0`, `0x100106c0` and `0x1000e310`.
[Rust biquad response](../src/pickup/biquad.rs) · [Rust secondary fit](../src/pickup/secondary.rs)

| Midpoint (Hz) | Engine | Residual before | Residual at midpoint | Residual after | Decision |
| ---: | --- | ---: | ---: | ---: | --- |
| 23.199630097 | Rust / Mac | 1.209290377 | 1.209131260 | 1.208887503 | Lower high bound |
| 23.199630097 | PE32 53/64 | 1.209271706 | 1.209129412 | 1.208903855 | Lower high bound |
| 11.599815049 | Rust / Mac | 1.208957247 | 1.209299226 | 1.209167595 | Accept local peak |
| 11.599815049 | PE32 53/64 | 1.209294148 | 1.209293683 | 1.209152878 | Lower high bound |
| 5.799907524 | PE32 53/64 | 1.209356840 | 1.209331851 | 1.209244107 | Lower high bound and exit on interval width |

The PE32 53-bit left sample is only about 0.000000465 above the center at
11.599815 Hz, but that is enough to keep bisecting. The next trial shrinks
the interval to 5.799907524 Hz, below the 10 Hz stopping width. The fit keeps
the last evaluated midpoint. For both engines the upper 0.707-of-peak search
runs to its Nyquist fallback, 19,531.25 Hz. The formula
`Q = peak / (upper − (peak / upper) × peak)` then gives
0.000593910739975110 for Rust/Mac and 0.000296955291428810 for both PE32 widths.
The fitted peak here is a search intermediate. The emitted secondary
bandpass uses the circuit resonance frequency, approximately 989.85 Hz.

### Controlled replays

- Replace the PE32 biquad output only at 1.599815 Hz in the point-4 fit with
  the measured Rust value. The midpoint becomes a local peak, the fit Q
  matches Mac and all nine final Q pairs reproduce the Rust/Mac result.
- Replace only the returned point-4 fit Q in PE32. The fill, smoothing,
  binary32 writer inputs and all nine final Q pairs reproduce Rust/Mac.
- Limit x87 to 24 significand bits only inside the complex reciprocal,
  multiply and divide routines for low-frequency biquad evaluations during
  the point-4 fit. The PE32 biquad intermediates at the decisive sample,
  the search decision and all nine packed pairs reproduce Rust/Mac. The
  surrounding fit and packing retain their original PE32 precision.
- Narrow only the complex multiply at `0x10010620` over the same scope.
  The residual triplet at 11.599815 Hz becomes (1.209265406, 1.209299149,
  1.209144776), sufficient to accept the peak and reproduce all nine Mac
  pairs. Narrowing only the reciprocal makes `z` match at the decisive
  sample but leaves the left residual (1.209321882) above the center
  (1.209293683) and all nine Windows pairs intact. The reciprocal is the
  earliest observed difference at that sample. The multiply control
  demonstrates a sufficient change to the branch.

This is a **confirmed float32 versus wider x87 intermediate-precision effect**
in the tested emulators, with branch amplification in the secondary fit. It
survives switching PE32 from 53 to 64 bits. The trace establishes the slot
behavior of the fingerprinted engines with their configured math adapters.
The historical Windows JVM control word and the original factory authoring
runtime remain unknown.

## How the seven words are produced

Point 4 is the last point whose raw secondary gain exceeds 0.2 (Rust/Mac
1.209299226, PE32 1.209331851). The table therefore copies its fitted Q
into points 4–8; points 0–3 have the same raw Q in Rust/Mac and PE32 53-bit.
Five passes of weighted log-domain smoothing update points 1–7 using
neighboring Q values. The two endpoints retain their unsmoothed values.
The table below gives the inputs and the resulting writer values. Both
native writers and Rust encode positive binary32 values with the IEEE
biased exponent plus one and drop the lowest bit of the explicit 24-bit
significand. All five exponent changes in the seven outliers are one step.

| Tone point | Exponent / mantissa word | Raw Q, Rust/Mac → PE32 | Binary32 writer Q, Rust/Mac → PE32 | VX exponent, Rust/Mac → PE32 | Mantissa distance, Rust vs firmware |
| ---: | --- | --- | --- | --- | ---: |
| 0 | 120 / 121 | 1.459935586 → same | 1.459935546 → same | 128 → 128 | 0 |
| 1 | 122 / 123 | 1.177869049 → same | 0.719408333 → 0.706817329 | 127 → 127 | 105,621 |
| 2 | 124 / 125 | 0.856158386 → same | 0.289605349 → 0.271240830 | 126 → 126 | 308,105 |
| 3 | 126 / 127 | 0.186179824 → same | 0.065080613 → 0.056214001 | 124 → 123 | 3,177,429 |
| 4 | 128 / 129 | 0.000593910740 → 0.000296955291 | 0.008761533 → 0.006082913 | 121 → 120 | 1,827,667 |
| 5 | 130 / 131 | 0.000593910740 → 0.000296955291 | 0.002352855 → 0.001414993 | 119 → 118 | 1,024,630 |
| 6 | 132 / 133 | 0.000593910740 → 0.000296955291 | 0.000997211 → 0.000536963 | 118 → 117 | 329,492 |
| 7 | 134 / 135 | 0.000593910740 → 0.000296955291 | 0.000705797 → 0.000362024 | 117 → 116 | 156,774 |
| 8 | 136 / 137 | 0.000593910740 → 0.000296955291 | 0.000593911 → 0.000296955 | 117 → 116 | 1 |

Point 8 has an additional exponent change with only a one-count mantissa
difference. The Windows widths give identical packed Q pairs. Relative to
firmware they match every Q exponent and seven mantissas. The two remaining
mantissas, at words 129 and 133, are each one count below firmware. This
accounts for the modeled secondary pickup response gap of roughly 2.11 dB
at tone point 4 and 5.80 dB at point 7 for Rust/Mac versus firmware.
[#179 modeled response](factory-program-parity.md#device-output-reference-and-firmware-bounds-179)

## Recommendation

Keep `host-std-v1` for device output and retain slot 52's exclusion from
the current clearance. Use the emulated Windows output as an offline
comparator. Slot 52 gives a concrete reason to prototype a Windows-compatible
profile if closer reproduction of this factory preset is a priority.
Evaluate that profile on the full reference corpora before considering any
preset-specific selection. The measured response is for a modeled filter
path. Audibility, the full device signal path and physical behavior are
unverified.

## Reproduction

Run from the issue worktree. Use the main checkout as the private artifact
root and choose a fresh report path under the ignored build directory.

```sh
artifact_root=/home/grbrum/Documents/Code/VariaxWorkbench
build=target/issue-184/pure
CARGO_TARGET_DIR="$build" cargo build --offline --locked --no-default-features \
  --bin variax-workbench --bin variax-firmware-inspect \
  --example trace_slot52_pickup

"$artifact_root/.tools/python/bin/python" scripts/trace_factory_slot52.py \
  --artifact-root "$artifact_root" \
  --archive "$artifact_root/vendor/installers/Variax300_3.10.vxf" \
  --extractor "$build/debug/variax-firmware-inspect" \
  --host-std "$build/debug/variax-workbench" \
  --rust-probe "$build/debug/examples/trace_slot52_pickup" \
  --report target/issue-184/slot52-trace.json
```

The report and the temporary Java, native and archive intermediates stay under
the ignored directory. The replay uses the same archive, models, Java overlay
and bounded original engines as #179. It checks one archive slot and traces
five secondary fits per native render, across one Mac baseline, two PE32
baselines and five PE32 interventions. The 60-slot and 2,329-preset parity
results are from the earlier studies.

Validation passed 708 Rust tests, five Python response tests, formatting and
Clippy with warnings denied. The trace checks all three original program hashes,
Rust/Mac equality and the controlled substitutions.
