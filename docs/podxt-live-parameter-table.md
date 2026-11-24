# PODxt Live parameter table

The table describes Line 6 Edit 3.06.0 for PODxt Live firmware 3.01,
product `0x0003000a`. It covers
99 editor and codec selectors, two shared command constants, and four metadata
rows. Each row cites its source methods and carries an evidence label. The
seven model lists have 246 entries. Their labels were seeded from the Rust
catalog and checked against `L6Assets.GetAssetName` in the original classes.
[Versioned JSON table](../research/podxt-live-parameters.json)

## Method

Trace the Live branch in `L6PODxtDevice` and `L6SharcToneEditor`, model catalogs
in `L6PODxtParamMap`, and selector/slot/PARM lookups in `L6SharcToneParamMap`.
The effective host definitions come from `L6AX3ParamDefs.Init` and the display
conversions from `L6AX3ParamFormats.Init` and its format classes. The JSON
distinguishes editor selectors, raw host values, controller values, and IFF
fields. Model-specific rows resolve their meaning through the model layouts.
The editor's Tweak order is recorded separately from each model's PARM order.

`EncodePatch` writes the same 160-byte payload for an edit-buffer send and a
tone-bank send. Offsets are zero-based within that payload. Controller `n`
corresponds to byte `32 + n`. In a 168-byte read reply the payload begins at
byte 7, so Rust reads it at byte `39 + n`. The name decoder reads the first
16 payload bytes. The writer starts with 32 spaces, writes the supplied name
with byte-level sanitization, and initializes the rest to zero before writing
parameters. The table accounts for all 160 bytes, including 35 unassigned
bytes. Tone address is in the separate message prefix.

The harness runs the Windows Java classes from the local extraction. It
bypasses the device constructor, supplies only the Live product ID, and
records calls to an in-memory `L6MIDISender`. It feeds synthetic patches
through the original parameter map, `EncodeParamChange` and both
`EncodePatch` destinations. The independent Python predictions use the
table's offsets, encodings and domains. They check the whole payload for
companion writes, use every integer point on each scalar wire grid, and
exercise each enum choice and model-specific association key. They also probe
half steps on both 14-bit axes and quarter-dB points for Gate Threshold.

Run from the issue worktree with the artifact root pointing to the checkout
that contains the ignored extraction and JDK 17.

```sh
artifact_root=../..
python3 scripts/verify_podxt_parameters.py --self-test
python3 scripts/verify_podxt_parameters.py \
  --jdk-bin "$artifact_root/.tools/jdk/bin" \
  --classes "$artifact_root/analysis/extracted/rea-full/line6-edit-3.06.0-win/embedded-zip"
```

The oracle prints a JSON report and exits 1 when comparisons find
mismatches. The known report contains two groups for the compressor alias
below. The four self-tests pass without the vendor classes.

## Coverage

| Block | Rows | Model-specific knobs | Encoder and tone cases |
| --- | ---: | ---: | ---: |
| Amp | 7 | | 856 |
| Drive | 1 | | 128 |
| Cab and room | 2 | | 175 |
| Mic | 1 | | 188 |
| EQ | 14 | | 1,540 |
| Gate | 3 | | 515 |
| Compressor | 4 | | 386 |
| Stomp | 7 | 97 | 11,251 |
| Mod | 11 | 52 | 39,849 |
| Delay | 11 | 40 | 37,946 |
| Reverb | 7 | 42 | 5,523 |
| Wah | 3 | | 138 |
| Volume pedal | 3 | | 258 |
| Tempo | 1 | | 2,101 |
| Variax link | 14 | | 1,792 |
| System | 12 | | 415 |
| Metadata | 4 | | 258 name and 128 address cases |
| **Total** | **105** | **231** | **103,061 parameter cases** |

Coverage includes 84 model layouts, 352 present/absent layout definition
checks, 246 asset-name checks, 384 incoming Tweak-family CC cases, and 109
association cases checked against both destinations and the live sender. Amp
selection ran all 107 choices with defaults enabled and disabled.

## Encoder findings

- **Compressor amount alias.** The Bass-only `COMP_AMOUNT` selector still
  enters the shared encoder on the Live branch. Directly setting it in the
  original map changes the PARM shared with Compressor Threshold. In 127 of
  128 cases the writer emitted the change at byte 41 (CC 9) in each
  destination. The sender emitted CC 5. This accounts for all 254 mismatches
  against the table's expected Live tone ownership.
- **Mod Speed and Delay Time.** The live encoder rounds to the nearest
  14-bit value. The tone writer truncates. The paths diverged at all 16,383
  half-step inputs per field. Both paths reproduced every point on each
  integer wire grid. Gate Threshold uses two formulas that agreed on the
  whole- and quarter-dB samples. The tempo grid reproduced all 2,101 tenths.
- **Volume pedal.** The live encoder emitted all 128 requested levels. The
  tone writer emitted 127 for all inputs, so 127 cases differed across paths.
- **Model defaults.** Amp selection sends CC 11 with Load Amp Defaults on
  (the initial setting) and CC 12 with it off. Both send only the model
  ordinal. Other model selectors also send a single ordinal. Offline the
  original map changes the model ID and keeps its existing PARMs. When
  connected, the editor merges the edit buffer after Amp, Stomp, Mod, Delay,
  Reverb and Wah selection. Cab selection refreshes the Mic and Room UI.
  Pack-ineligible selector choices can fall back to the IDs in the table.

## Rust cross-check

Four tests in `line6-devices` check all 55 basic controllers, all 16 additional
settings, the 103-entry CC registry, and all 246 model IDs, positions, names
and pack flags. The 121 nonzero pack predicates match. The registry includes
four additional Bass-only EQ CC numbers (32, 68, 115, 118). The model names
also match all 246 corresponding names in the file editor's larger asset
lookup.

- **Live rows without a model definition.** Rust includes Mod Param 5 (CC 55)
  and Delay Param 5 (CC 86) in `Control::ALL`. Both have shared codec and
  payload support. All 24 Live Mod layouts and all 14 Live Delay layouts lack
  a definition at those positions. Several other raw knob positions apply
  only to specific models, as recorded in the JSON.
- **Additional settings.** Rust admits Tweak ordinals 0–13 for requests.
  Line 6 Edit builds a model-dependent menu from positions 0–20. Rust's mic
  display always uses the four guitar mic labels. The 22 Bass Pack cabs use
  a different label set at the same ordinals in Line 6 Edit. Amp selection
  exposes CC 12 only in Rust. For volume pedal level, Rust reads and sends
  the live scalar while the original writer forces the tone byte to 127.
  Rust accepts integer Gate Threshold values and direct 14-bit Mod Speed
  and Delay Time values. The Java host accepts float input before encoding.
- **Model layouts in the file editor.** A comparison of the existing Rust
  percent-control APIs against the JSON matched 215 model/selector/PARM
  entries after ignoring title case. On Stomp `0x020a0009`, Rust exposes
  Filter at selector `0x00030002` / PARM `0x00100002`, where Line 6 Edit uses
  Harmony 1457. Rust exposes Attack at selector `0x00030003` / PARM
  `0x00100003`, where Line 6 Edit uses Wave. Rust already implements the
  separate Harmony 1457 choice at the former address. The Wave control is
  absent from the Rust percent API. Delay Heads applies to both
  `0x02020005` and `0x0202000c` in Line 6 Edit. Rust admits only
  `0x02020005`. The other discrete layouts covered by Rust use the same
  selector/PARM locations.
- **File admission and changes.** The file editor accepts only 43 of 107 Amp,
  25 of 47 Cab, 14 of 31 Stomp, 11 of 24 Mod, and 9 of 14 Delay model IDs as
  requested choices. It accepts all 15 Reverb IDs and has no Wah model edit.
  Its ID changes keep the existing PARMs and require an existing record.
  The six Amp knob edits also exclude No Amp. The file editor's enable edits
  cover Amp, Stomp, Mod, Delay and EQ on canonical MINF records. Other
  writable Java map controls lack an equivalent file edit. IFF names use
  PINF's UTF-16 field, a different representation from the 16-byte reply
  name. The version floor and strict record guards are file-editor policy.
- **NOTE dependencies.** The original editor recomputes both active sync
  axes on a NOTE or tempo event. Rust's file NOTE edits require the other
  NOTE to be Off and, for a positive selection, require the canonical
  System model and existing Tempo. The live probe's per-NOTE companion
  tracking lists the corresponding axis. Its tempo tracking lists both.
  The table records the original editor's dependency across the two axes.

## Unknowns

Per-model firmware companion values are unknown for the full catalog. The
encoder sends a selector, and the connected edit-buffer response supplies the
result. Existing owner observations cover selected models only. The physical
interpretation of normalized percentages and the 35 unassigned tone bytes
is unresolved. The WHAMMY interval formatter provides a signed display
range and quantization without a printed unit.

The shared codec handles FX loop, Bass-only EQ and compressor fields, and
Bass-model D.I. controls. The Live has no FX loop. It uses four EQ bands and
can show the D.I. controls with Bass Pack amp models. The Variax family fields
use wrapped messages and separate stored bytes. The editor definitions and
encoder results do not establish which linked commands a particular guitar
and firmware combination accepts. This run used synthetic patches, local
original classes and an in-memory sender.

## Local checks

All-features offline tests passed in the app (736 passed, 1 ignored),
`line6-devices` (86 passed, 2 ignored), and `variax-probe` (4 passed).
The ignored tests are existing Variax oracle tests that require separate
Workbench inputs. Formatting and all-feature, all-target Clippy with
warnings denied passed for all three crates.
