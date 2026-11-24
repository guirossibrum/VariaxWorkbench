# Variax 300/600 firmware 3.10 program parity

Issue [#174](https://github.com/guirossibrum/VariaxWorkbench/issues/174)
compares all 60 programs from the [fingerprinted factory archive](factory-extraction.md).
Standard Rust and the original Java/native host reference agree byte for byte
on all 60 presets. All 60 archived programs differ from both Rust profiles and
the original host reference. The comparisons used local files and CPU emulation.

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

## Decision

**Step 2 encoder ([#175](https://github.com/guirossibrum/VariaxWorkbench/issues/175))
is a no-go under its current dependency on a passing #174.** Firmware parity
remains unresolved.

**Step 3 first edit-buffer test ([#176](https://github.com/guirossibrum/VariaxWorkbench/issues/176))
is a no-go.** Every archived slot still has unexplained differences, so the
stop rule also holds steps 4 and 5. The remaining offline evidence gap is the
factory generator, its exact inputs and the runtime that produced those words.
