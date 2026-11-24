# Legacy host coefficient corpus

Issue [#180](https://github.com/guirossibrum/VariaxWorkbench/issues/180) extends
the original-host comparison beyond the earlier single-setting sweeps. It
uses synthetic Workbench files and locally extracted model definitions. Each
result compares the final 3,276 bytes, including the original name/settings
overlay and checksum.

## Results

Both runs below compared Rust `host-std-v1` with the original Java reader,
`initialized-host-libm-v2` Mac native generator and original final overlay.

| Engine lifetime | Exact presets | Bytes compared | Differing words |
| --- | ---: | ---: | ---: |
| Fresh engine per preset | 2,329/2,329 | 7,629,804 | 0 |
| One ordered engine for the entire corpus | 2,329/2,329 | 7,629,804 | 0 |

Retaining engine state changed 1,506 outputs in each implementation relative
to fresh rendering. A smaller 2,041-case run repeated the first 2,040 cases in
the same order and matched the corresponding full-run results. Rust rejected
all three invalid probes and preserved the subsequent packed result.
The test used Linux x86-64, Rust 1.99.1 and Python 3.14.8. All 708 Rust
tests passed, along with formatting and Clippy with warnings denied.

The exact result applies to this corpus and the emulated Mac host reference.
Firmware and transmission checks retain their separate criteria.
[Sanitized results and hashes](host-wide-corpus.json)

## Corpus

The generator uses Python's seeded `random.Random(180600175)` and writes each
float once as an IEEE-754 binary32 value. The 1,000 random cases use the original
editor's continuous ranges and discrete choices. Body assignment cycles across
the entire catalog, which guarantees a sample for every body. The deterministic
groups cover the following cases.

| Group | Cases | Coverage |
| --- | ---: | --- |
| Active pickup pairs | 640 | 20 non-acoustic bodies × 32 rotating pairs. Each available pickup appears in each slot on each body. The rotation covers series, parallel and both phase states. |
| Active single pickups | 80 | Pickup 1 or pickup 2 alone with both phase settings on each non-acoustic body. |
| Acoustic bodies | 8 | One baseline with dormant pickups disabled for each acoustic body. |
| Dormant pickup pairs and singles | 288 | The same 36 paired and single setups for each acoustic body, representing an electric preset after changing its body. Pickup and electrical fields remain stored while the original UI disables their controls. |
| Continuous endpoints | 32 | Both endpoints and the immediately adjacent representable float inside each bound for pickup positions, angles and levels, trim and saved tone. |
| Twelve-string endpoints and default behavior | 159 | All three twelve-string bodies, each string's detune and mix endpoints and inward neighbours, spread endpoints and neighbours, and tuning off. |
| Tuning endpoints | 96 | Every string at −12, −11, +11 and +12 semitones on four representative electric, acoustic and twelve-string bodies. The sitar keeps alternate tuning off. |
| Electrical choices | 24 | Every volume and tone resistance, every capacitance including Filter, and both tapers. |
| Seeded combinations | 1,000 | Bodies, wiring, pickup controls, trim and available tuning controls with the recovered editor domains. |
| Repeated anchor | 2 | An identical acoustic file opens and closes the ordered run. |
| **Total** | **2,329** | |

Source ranges and body-specific availability come from [parameter domains](parameter-domains.md).
The corpus includes combinations alongside targeted one-field boundaries.
It samples pairings and random inputs without enumerating the full Cartesian
product of models and parameter values.

## Reproduction and scope

The Java harness reads every synthetic preset and maps its parameters. The
`initialized-host-libm-v2` reference executes the original Mac coefficient
instructions in Unicorn. Fresh workers reset the initialized engine object
for each file. A separate worker runs the identical sequence with one
persistent engine. Both use the original Java final overlay.

Rust fresh renders use the ordinary `render-preset` command. The research
example uses the same Rust parser, host mapping, engine and final overlay for
each file in one ordered batch. Its three rejection probes verify missing
parameter, nonfinite parameter and out-of-range saved tone position rejection.
After each set of rejected inputs, the succeeding valid stateful output must
equal the control run byte for byte. No hardware or audio path participates.

Use the ignored artifact root described in the [factory comparison](factory-program-parity.md).
Use a new report path and a separate target directory from all-feature tests,
which select the software-wide profile for their executable.

```sh
CARGO_TARGET_DIR=target/issue-180/host cargo build --offline --locked \
  --no-default-features --bin variax-workbench --example render_preset_corpus

"$artifact_root/.tools/python/bin/python" scripts/verify_host_wide_corpus.py \
  --artifact-root "$artifact_root" \
  --binary target/issue-180/host/debug/variax-workbench \
  --batch target/issue-180/host/debug/examples/render_preset_corpus \
  --jobs 8 \
  --report target/issue-180/replay.json
```

Optional `--start` and `--limit` select a contiguous slice for diagnosis. A
slice starts its own ordered engine and has that slice's state history. Raw
vendor data, native settings and packed outputs stay in temporary private
storage. Reports retain counts, synthetic case indices, differing word
indices and SHA-256 hashes. A run with an incomplete or differing case
retains its report and returns a failure status.
