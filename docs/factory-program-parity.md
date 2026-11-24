# Variax 300/600 firmware 3.10 program parity

Issue [#174](https://github.com/guirossibrum/VariaxWorkbench/issues/174)
compares all 60 programs from the [fingerprinted factory archive](factory-extraction.md).
Standard Rust and the original Java/native host reference agree byte for byte
on all 60 presets. All 60 archived programs differ from both Rust profiles and
the original host reference. The comparisons used local files and CPU emulation.

The [Windows precision follow-up](factory-program-windows-precision.md) reproduces
part of the gap with the original PE32 engine and an isolated Q23 rounding test.

## Exact results

Each comparison covers 196,560 bytes across 60 slots. Words are 24-bit and the
indices are zero-based. The table counts changed words once per slot and pair.
[Per-slot results](factory-program-parity.json) give the exact word indices by
category and SHA-256 hashes of the canonical programs. An empty difference
object is an exact match.

| Comparison | Exact slots | Different bytes | Coefficient rounding | Settings or metadata | Checksum | Unknown words |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Rust `host-std-v1` vs firmware | 0/60 | 3,188 | 61 | 0 | 59 | 2,893 |
| Rust `software-wide-v1` vs firmware | 0/60 | 3,191 | 64 | 0 | 59 | 2,893 |
| Original Java/native vs firmware | 0/60 | 3,188 | 61 | 0 | 59 | 2,893 |
| Rust `host-std-v1` vs original Java/native | 60/60 | 0 | 0 | 0 | 0 | 0 |
| Rust `software-wide-v1` vs original Java/native | 57/60 | 8 | 5 | 0 | 3 | 0 |
| Rust `software-wide-v1` vs Rust `host-std-v1` | 57/60 | 8 | 5 | 0 | 3 | 0 |

- All 60 firmware slots differ at volume-curve word 551. This is point 6 of the
  nine-point curve. The host converts 0.625 to Q23 by multiplying by 8,388,607,
  adding 0.5 and truncating. Its float32 product and sum yield a result one
  count above the firmware. Carrying the double intermediate through the
  addition reproduces the exact archived word in all 60 slots. This explains
  the numeric difference. The original factory toolchain is unidentified.
- The two Rust profiles differ at slot 23 word 127 and slots 30 and 31 words 147
  and 149. The original native word map attributes these to the pickup response
  Tone2 Q and Gain tables. Firmware agrees with the wide value at slot 23 and
  with the standard values at slots 30 and 31. Both profiles use the same
  volume-curve arithmetic. [Observed native word map](dsp-word-map.md)
- All name and settings bytes written by the host overlay agree. Every final
  checksum independently validates the same 1,089-word modular sum. The 59
  changed checksum words follow from earlier word changes. Slot 37 has the same
  sum despite different program data.
- The 2,893 remaining differences occur at the same slot and word positions for
  both profiles and the original host. Of those, 2,742 are one count apart in
  the 24-bit representation and 151 are two or more counts apart. This describes
  the differences without applying a tolerance or assuming their source.
  The archived metadata retains three bytes of each relevant float. Any fourth
  byte in factory authoring inputs is unavailable from these settings files.
  Authoring precision, model resources, toolchain and runtime remain possible
  contributors. Exact agreement between standard Rust and the host shows the
  remaining firmware gap is shared by both implementations on these inputs.


Two alternate original-native runs matched wide Rust across all 60 slots. One
uses the software-wide adapter and the other uses the pinned emulated Apple
88.3.9 i386 math runtime. Both used the same model resource hashes as the
baseline. Neither recovered a firmware match or changed the 2,893 unknown
positions. The Apple runtime is a historical candidate for the host
instructions and does not identify the factory build environment.
[Apple candidate scope](apple-tiger-math.md)

## Reproduction

The harness verifies the exact archive SHA-256 and runs the Rust extractor into a
fresh private directory. It reverses each three-byte word in the first 3,276
bytes of the matching raw slot and checks the remaining 820 bytes of slot
padding. Both binaries use the ordinary offline `render-preset` command, with a
fresh engine per slot and default host options. The recorded run used Linux
x86-64 debug builds with rustc 1.99.1 and USB/GUI features excluded.

The original column uses the Workbench 1.75 Java reader and parameter map,
initialized Mach-O i386 coefficient instructions with host libm adapters
(`initialized-host-libm-v2`), and the original Java final overlay. The harness
restores the initialized coefficient object before each call. Original device
constructors are bypassed. Vendor machine code runs in private CPU emulation
with the existing bounded imports. The original-reference result has the scope
and historical-runtime limits in [native runtime](native-runtime.md).

From the issue worktree, set `artifact_root` to the absolute path of the local
checkout holding the private research inputs. It needs the extracted Java classes,
local JDK and Python environment with Unicorn, native library slice and index,
and model catalog and resources used by the existing host harness. Use new
report paths under the ignored build directory.

```sh
CARGO_TARGET_DIR=target/issue-174/std cargo build --offline --locked \
  --no-default-features --bin variax-workbench --bin variax-firmware-inspect
CARGO_TARGET_DIR=target/issue-174/wide cargo build --offline --locked \
  --no-default-features --features portable-math --bin variax-workbench

"$artifact_root/.tools/python/bin/python" scripts/verify_factory_programs.py \
  --artifact-root "$artifact_root" \
  --archive "$artifact_root/vendor/installers/Variax300_3.10.vxf" \
  --extractor target/issue-174/std/debug/variax-firmware-inspect \
  --host-std target/issue-174/std/debug/variax-workbench \
  --software-wide target/issue-174/wide/debug/variax-workbench \
  --report target/issue-174/replay.json
```

The optional `--oracle-software-helper` and `--oracle-apple-runtime` flags
repeat the original host render with the existing locally built Rust math
research helper or the pinned Apple 88.3.9 i386 runtime slice. Supply a separate
new report path for each run. Only counts, word indices and hashes are retained
in the report. The extracted settings, raw slots, model resources and packed
programs stay private.

## Decision after #174

The initial exact-firmware comparison held back the encoder and edit-buffer
test. The owner subsequently allowed offline encoder work in
[#175](https://github.com/guirossibrum/VariaxWorkbench/issues/175) using the
original Java transcript as its reference. The owner set a separate
device-output criterion for #176 in
[#179](https://github.com/guirossibrum/VariaxWorkbench/issues/179).

## Device-output reference and firmware bounds (#179)

The owner selected original Workbench parity as the device-output reference on
2026-11-23. **`host-std-v1` is the default profile for that reference.** On
this Linux x86-64 replay it matches all 60 initialized Mac Workbench 1.75
host-reference renders byte for byte. The original Windows PE32 engine runs
with 53-bit x87 precision as an independent comparator. A Windows Rust profile
is optional. Historical Mac libm authentication and wider input coverage
remain separate work. [Math-profile decision](https://github.com/guirossibrum/VariaxWorkbench/issues/5)

**Under the proposed limits below, the offline prerequisite for [#176](https://github.com/guirossibrum/VariaxWorkbench/issues/176)
fails.** The [owner decision](#owner-decision-2026-11-23) replaces that gate. Slot 52 exceeds the proposed word and response limits. In nine
reverb-enabled slots, at least one Rust feedback coefficient moves one count
toward unity. Windows also moves individual feedback coefficients toward
unity. The owner decides whether and how to proceed.

[Per-slot results, observed-range distributions, and reverb positions](factory-program-response.json)
contain derived measurements only. Slot and word indices start at zero.
The replay validates the firmware archive, both previous reports' program
hashes, the Mac and Windows library and model fingerprints, and the Java class
hashes. It renders every Rust output using the offline CLI and checks it against
the corresponding original Mac output before measuring differences.

### Packed-word comparison

Distances are the shortest absolute distance modulo 2²⁴. A distance of 1
across a signed zero boundary remains 1. VX exponent and mantissa words are
counted individually. A distance in a mantissa word alone does not measure
a change in the decoded coefficient. The report provides per-slot frequency
counts for nonzero distances by DSP block and by exact range from
[`dsp-word-map.json`](dsp-word-map.json). Missing per-slot histograms represent
zero changes. Aggregate histograms include the distribution of the number of
changed words per slot, including zero. The ranges preserve overlapping writer
ownership through the original map. The checksum is reported independently.

Each cell gives changed words across all 60 slots / maximum count distance.

| DSP block | Rust vs firmware | Mac vs Windows 53-bit | Windows 53-bit vs firmware |
| --- | ---: | ---: | ---: |
| Pickup geometry / microphone | 106 / 1 | 106 / 1 | 0 / 0 |
| Pickup response / pick filter / compressor | 522 / 3,177,429 | 491 / 3,177,429 | 405 / 11 |
| Body | 34 / 1 | 39 / 1 | 43 / 1 |
| Low-cut | 41 / 1 | 52 / 1 | 25 / 1 |
| String tone / resonance | 755 / 1 | 747 / 1 | 28 / 1 |
| Global decay | 40 / 1 | 38 / 1 | 2 / 1 |
| String EQ | 826 / 2 | 519 / 2 | 554 / 2 |
| Sum to mono | 68 / 1 | 65 / 1 | 5 / 1 |
| Volume curve | 60 / 1 | 60 / 1 | 0 / 0 |
| Sitar and auxiliary word | 12 / 1 | 7 / 1 | 5 / 1 |
| Drone and unobserved | 10 / 1 | 7 / 1 | 9 / 1 |
| Reverb | 111 / 1 | 115 / 1 | 60 / 1 |
| 12-string EQ and gains | 360 / 18 | 288 / 2 | 199 / 18 |
| Coil gain and unobserved | 9 / 76,223 | 0 / 0 | 9 / 76,223 |
| Checksum | 59 / 6,102,381 | 59 / 6,102,421 | 59 / 76,228 |

Slot 52 accounts for the largest packed-word differences. Seven Tone2 Q
mantissa words differ by more than 100,000 counts, and five of those words
have a changed companion exponent. Both original editors generated these
outputs from the same decoded preset. The largest coil-gain gap is 76,223
counts (0.0090865 on a Q23 unity scale) at word 772 in slots 30 and 31,
shared by both editors. Those changes warrant checks beyond counting
last-bit rounding differences.

### Frequency-response comparison

The harness evaluates coefficient-only transfer functions on a linear and
logarithmic grid of 20,481 frequencies from 20 Hz through 20 kHz, including both endpoints and
Nyquist. The coefficient engine rate is 39,062.5 Hz. Frequencies above
19,531.25 Hz are the mirrored part of the discrete-time response.
The reported metric is the largest sampled absolute difference in magnitude
dB with both magnitudes floored at −120 dB. The report also gives the maximum
absolute linear magnitude difference to expose the scale of the change near a
response minimum. The data include each slot and path counts. The table
reports peaks for enabled paths. Stored coefficients also get measured for
bypassed blocks where they can be decoded as the requested filter.

The peak slot is in parentheses. The slot count follows the block name.

| DSP block | Rust vs firmware | Mac vs Windows 53-bit | Windows 53-bit vs firmware |
| --- | ---: | ---: | ---: |
| String EQ (51) | 0.021912 dB (49) | 0.021912 dB (49) | 0.017883 dB (4) |
| String tone / resonance (49) | 0.273874 dB (30) | 0.273874 dB (30) | 0.007151 dB (49) |
| 12-string EQ (9) | 0.000430 dB (57) | 0.000430 dB (57) | 0.000155 dB (59) |
| Pickup response (50) | 5.798641 dB (52) | 5.798645 dB (52) | 0.000022 dB (52) |
| Low-cut (18) | 0.029958 dB (20) | 0.037720 dB (48) | 0.037720 dB (48) |
| Body (57) | 0.002609 dB (20) | 0.000323 dB (12) | 0.002609 dB (20) |

The analysis uses the packed Q23 scaling and coefficient order in
[`src/dsp/coefficients.rs`](../src/dsp/coefficients.rs) and the VX format in
[`src/dsp/patch.rs`](../src/dsp/patch.rs). With `z = exp(−j·2πf/fs)`, each
ordinary section uses `(b0+b1·z+b2·z²)/(1+a1·z+a2·z²)`.

- **String EQ.** Cascade the indicated number of packed bands per string and
  apply its string gain.
- **String tone and body.** Sum the reconstructed bandpass resonator responses
  per string or body bank. For the body, also evaluate the two mix gains,
  the treble shape and rolloff individually. String hold and decay are
  time-dependent and are covered by word counts.
- **12-string EQ.** Cascade each string's two sections. Evaluate the same
  cascade with the wet gain and the independent dry gain. Pitch-shift timing
  is outside the transfer model.
- **Pickup response.** Evaluate the volume lowpass and primary bandpass, and
  the tone primary gain and secondary bandpass at the nine stored points.
  Use the decoded VX frequency inverse, Q and gain with the host
  [bandpass equations](../src/pickup/biquad.rs).
  The ten slots with pick-filter and compressor overlays have no active
  pickup-response table to decode. Their overlapping words are included in
  the word comparison.
- **Low-cut.** Use the packed denominator and the high-pass numerator
  `1−2z+z²` with one or two sections from the order flag.

The host writers establish the packed layouts. The path equations above
infer the filter response from those layouts and do not reconstruct the
firmware's signal routing, control interpolation, fixed-point execution or
time-varying behavior. The numbers characterize this archive and sampled
grid. Physical behavior and audio equivalence remain unverified.

### Reverb and the proposed bound

The ten enabled reverb slots are 2, 45–51, 54 and 55. Per slot the replay
checks all 36 tank feedback words, nine allpass density words, four allpass
delay words and the reverb control and pre-delay words. Tank and allpass
tables are signed Q23 with a unity magnitude of 8,388,608 counts. For each
word, the study compares absolute distance from unity with firmware.

| Candidate vs firmware | Changed tank words | Tank words toward unity | Changed allpass density words | Density words toward unity |
| --- | ---: | ---: | ---: | ---: |
| Rust / Mac reference | 73 | 32 in 9 slots | 38 | 38 in 10 slots |
| Windows 53-bit | 60 | 5 in 4 slots | 0 | 0 |

Every movement toward unity is one count. The smallest tank margin over all
three sources is 3,572,457 counts (0.42587 below unity). The smallest
allpass density margin is 1,082,131 counts. All four delay words and the
control and pre-delay words match in all ten slots. The Reso-5 candidate in
#176 (slot 50) has four Rust tank words one count closer to unity at
indices 643, 646, 651 and 676.

**Proposed comparison limits.** Use at most 32 modular counts per packed
program word except checksums and coil-gain words 771–774. Cap the coil-gain
distance at 83,886 counts (about 0.01 on the Q23 unity scale). Require at most
0.10 dB in every enabled modeled filter path on the grid. These screening
values round above the Windows comparator's observed ordinary word and
response peaks and account for the shared coil-gain offset separately.
The reverb criterion independently requires every tank value to stay below
unity and none to move toward it.

| Firmware comparison | 32-count word limit | 83,886-count coil limit | 0.10 dB enabled response limit | Strict tank criterion |
| --- | --- | --- | --- | --- |
| Rust / Mac reference | Exceeds, 3,177,429 | Pass, 76,223 | Exceeds, 5.798641 dB | Fails in 9 slots |
| Windows 53-bit | Pass, 18 | Pass, 76,223 | Pass, 0.037720 dB | Fails in 4 slots |

The Rust/Mac secondary pickup path at slot 52, tone point 7 (zero-based),
changes from about −45.93 dB in firmware to −51.73 dB in Rust/Mac near
Nyquist. The largest absolute linear difference across modeled pickup
paths in slot 52 is 0.15742. Thirteen slots also exceed 0.10 dB in enabled
string tone. The peak is 0.273874 dB at 84.15 Hz in the sixth string’s
resonator bank in slots 30, 32 and 34. The proposed limits are
an archive regression screen. The observed Windows fit and the remaining
margin to unity do not override the stop rule.

### Owner decision (2026-11-23)

After reviewing these measurements, the owner decided:

1. **Editor compatibility.** Exact agreement with the original Mac Workbench
   output satisfies the device-output reference. `host-std-v1` meets it on all
   60 slots here and on the 2,329-preset corpus ([#180](host-wide-corpus.md)).
   The Windows-derived screening limits above flag outliers. They do not veto
   the Mac-compatible profile.
2. **Firmware comparison.** This remains a separate check, applied to the
   preset actually sent.
3. **Reverb, provisional and offline.** For the preset sent, each tank feedback
   and allpass density coefficient may differ from firmware by at most 2
   counts and must stay at least 10 % below unity (838,861 counts). This checks
   individual coefficients. It does not establish stability of the full reverb
   network. All 60 slots meet it; the smallest tank margin is 42.6 %.
4. **#176 prerequisite cleared for Reso-5 (slot 50) only.**
   - Every differing word is one count.
   - The largest modeled enabled-path change is 0.067 dB (string 5 resonators, 115 Hz).
   - Reverb changes are one count, with a tank margin of 45.7 %.
   - The clearance holds only if a same-day backup shows identical settings and
     the generated program equals the Mac reference byte for byte.
   - The owner's explicit go for stage 1 is still required separately.
5. **Slot 52 is excluded** from this decision. Besides the 5.80 dB peak at tone
   point 7 near Nyquist (−51.73 vs −45.93 dB), the secondary pickup bandpass at
   tone point 4 is **2.113 dB lower at 18,730.52 Hz**: Rust/Mac 0.57172
   (−4.856 dB) vs firmware 0.72914 (−2.744 dB). These are modeled gains of the
   isolated bandpass, and the original Mac Workbench produces the same values.
   The original Windows engine matches the firmware there. An optional
   Windows-precision Rust profile would address it.

### Reproduction

Use the local artifact root described above and a dedicated build directory
for the standard-math binaries. Building another feature set in that same
directory can replace the CLI while the comparison is running. Use a fresh
report path.

```sh
CARGO_TARGET_DIR=target/issue-179/std cargo build --offline --locked \
  --no-default-features --bin variax-workbench --bin variax-firmware-inspect

"$artifact_root/.tools/python/bin/python" scripts/verify_factory_response.py \
  --artifact-root "$artifact_root" \
  --archive "$artifact_root/vendor/installers/Variax300_3.10.vxf" \
  --extractor target/issue-179/std/debug/variax-firmware-inspect \
  --host-std target/issue-179/std/debug/variax-workbench \
  --report target/issue-179/response.json

"$artifact_root/.tools/python/bin/python" scripts/test_factory_response.py
```

Rust validation passed 708 all-feature tests, Clippy with warnings denied,
and formatting. The five synthetic tests cover coefficient order and scaling,
VX and bandpass reconstruction, count distance, the reverb stop rule and the
frequency grid. The replay used local files and CPU emulation.
