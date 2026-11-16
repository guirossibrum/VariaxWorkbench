# Rust preset-file support and compatibility checks

## Implemented behavior

`src/preset.rs` reads individual Workbench `FORM/L6PA` files, including the
electric and acoustic default `.vwm` presets extracted from Workbench 1.75.
A separate `src/bundle.rs` reader supports big-endian `L6BA` banks and `L6CO`
collections, preset extraction, and lossless embedded-preset replacement.
Little-endian variants and other FORM types remain unsupported. These file operations do not contact the guitar. Separate offline coefficient rendering is documented in [host pipeline](host-pipeline.md).

The reader validates file/container lengths and the required information chunks,
then exposes the preset name, device ID/version, model slots/IDs, enabled flags,
ordinals, parameter types, and raw value bits. Unknown chunks and untouched bytes
remain in the original buffer. It preserves nonzero odd-chunk padding and does
not reorder models, parameters, or metadata. Ambiguous duplicate required chunks,
model slots, or parameter IDs are rejected rather than silently edited.

`rename` replaces only the fixed 32-code-unit UTF-16BE name field. Names exceeding
that capacity, containing NUL, or splitting surrogate pairs are not written.
`set` modifies only the four stored value bytes of an existing parameter, retaining
its integer/float type. Parameter editing is limited to legacy electric Variax
device IDs `0x00070000` and `0x00070004`; explicit edits enforce the recovered
[parameter domains and original UI body restrictions](parameter-domains.md).
Imported unknown/out-of-domain fields remain lossless until explicitly edited. There is no device transmission.

A PINF target of `0x0003000A` is a PODxt Live tone, not a Variax preset. The
guitar editor does not open or rename it. That librarian is documented in
[POD presets](pod-presets.md).

CLI commands:

```sh
cargo run -- inspect INPUT.vwm
cargo run -- roundtrip INPUT.vwm COPY.vwm
cargo run -- rename INPUT.vwm RENAMED.vwm "My Guitar"
cargo run -- set INPUT.vwm EDITED.vwm 02010001 200000 float 2.5
```

Output files are written to a temporary sibling, synced, and atomically persisted
without replacing existing paths. Existing paths are refused. The
parameter command uses hexadecimal **file-local** slot/parameter IDs, which differ
from MIDI metadata IDs. Example slot `02010001` / parameter `200000` denotes
pickup 1 position. Its stored numeric units are preserved.

## Recovered file layout

Derived from the original bytecode/CFR output:

| Container or chunk | Meaning |
| --- | --- |
| `FORM L6PA` | Individual preset; 32-bit big-endian chunk lengths |
| `PINF` | Format version (1), target MIDI product ID, 32 UTF-16BE name units, device version |
| `LIST PATC` | Model collection |
| `LIST MODL` | One model instance |
| `MINF` | 32-bit model ID, 32-bit slot, ordinal byte, two reserved bytes, enabled byte |
| `PARM` | 32-bit parameter marker/ID, type, raw value bits; effective ID is low 24 bits |
| Other chunks | Preserved without reinterpretation |

The original format reader uses word alignment, and parameters use type 0 for
integers/booleans and type 1 for IEEE-754 float32 bits. The Rust reader keeps unknown
value types as raw data and refuses to edit them.

Source references, relative to `analysis/decompiled/mac/`:
`cmn/l6java/io/iff/L6IFFParser.java`, `L6IFFUtils.java`,
`cmn/l6java/patch/L6Patch.java`, `L6Model.java`, `L6ModelParam.java`.

## Verification against the original application code

`scripts/InspectPreset.java` loads files using Workbench's own
`L6PatchUtils.LoadPatch` and can reserialize them using `L6IFFUtils.SaveFile`.
It runs headless from the original JAR with no editor UI or hardware connection.

`python scripts/verify_preset_compatibility.py` runs six checks:

1. Electric default: Rust and Java summaries agree; Rust roundtrip is byte-identical.
2. Acoustic default: same comparison and preservation checks.
3. Java reads the Rust-renamed preset, including an emoji encoded as a surrogate pair.
4. Java reads a Rust pickup-position edit with the expected float bits.
5. Java re-saves that edited preset, and Rust reads the same models/parameters back.
6. Attempting to overwrite an existing output fails and preserves its bytes.

All six passed. The report is `analysis/reports/preset-compatibility.json`.
The initial electric reference summaries are `default-preset-java.txt` and
`default-preset-rust.txt`; their contents match exactly.

Rust unit tests additionally exercise unknown chunk/padding preservation,
type mismatches, nonexistent parameters, Unicode bounds, every truncated prefix,
and corrupted lengths. These checks establish compatibility for the tested
files and edits; they do not establish that arbitrary models are valid on hardware.

## Native DSP packing primitive

`src/dsp.rs` implements the native engine's intermediate `L6VariaxHWPatch::Pack`
routine: 1,091 data words contribute their low 24 bits to an additive checksum;
the routine emits those words plus the checksum as 1,092 big-endian 24-bit words,
3,276 bytes total.

This was verified by executing the **original i386 routine** at `0x1bd10` in
Unicorn 2.1.4 on deterministic synthetic inputs. The harness resolves the code
through Mach-O segment mappings and binds it to REA's target digest and function
extent. Only routine code, private object/output memory, and stack are mapped;
no OS API stubs or hardware interfaces are supplied. Emulation has time and
instruction limits and must return to a sentinel address.

Reproduce with `.tools/python/bin/python scripts/verify_native_pack.py`.
Its source is based on the [official Unicorn Python API tutorial](https://www.unicorn-engine.org/docs/tutorial.html).
The report is `analysis/reports/native-pack-emulation.json`; synthetic input/output
fixtures are in `tests/fixtures/native-pack-*.bin`. `cargo test` compares the Rust
output byte-for-byte to the original machine-code result, including high-bit and
overflow cases. A separate boundary test checks zero and wrapping sums.

The Java layer overlays metadata and applies its own checksum pass afterward.
This primitive is therefore **not a completed uploadable patch generator**.
Body/pickup coefficient calculations, their rounding behavior, and the final
patch assembly remain to be recovered and validated.

## Bank reference checks

`python scripts/verify_bundle_compatibility.py` asks the original Java writer to
create both fixed L6BA and non-fixed L6CO containers with a Unicode bank name.
Both original Java and Rust summaries agree, and Rust extracts a preset with the
same model/parameter summary as the input. Both checks passed. Embedded replacement
has synthetic Rust coverage; exhaustive vendor-bank variants remain unverified.
The GUI now browses and edits existing presets within banks/collections and saves protected whole-container copies; see [bank editing and reference checks](bank-editor.md).
