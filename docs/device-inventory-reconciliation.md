# Issue #114 device inventory review checkpoint

The schema-2 snapshot at `508cd000cc7f0cb074cde0056a8b1a52fab4cbdf:supported_devices.txt`
hashes to `e808fe477b4d1c8a976bdaf11369e711e131317164f20d4140b816f69dd26c7b`,
which is the original-source pin in
[`research/audio-inventory-source.json`](../research/audio-inventory-source.json).
Projecting the snapshot to `name` and `role` produces the exact bytes of
[#96's frozen projection](../research/audio-inventory-schema2.tsv), including
order, duplicate rows and LF endings. The projection SHA-256 is
`c3ed43ad76a0486ef327cb0cf1c42d389124c765418b0065b29f2d1db37ba790`.
The row totals are 186 aliases, 29 products, five guitars, two bridges and
two family labels, for 224 rows and 223 exact spellings. All 186 alias
rows have empty USB and Line 6 identifier fields. These are historical
catalog classifications. Device identity, capabilities and hardware
permissions require separate evidence.

## Reconciliation

Both drafts are read from `309f948ceeda368123649f848526c7bc320632cf`.
The underscore draft contains 216 unique names. The hyphen draft contains
176 unique names, with 172 `referenced-only` tags and four `partial` tags.
Their exact-name intersection contains 172 names, for a union of 220.
Every draft spelling occurs in the snapshot.

| Difference | Finding |
| --- | --- |
| In the hyphen draft only, relative to the underscore draft | `Variax 300`, `Variax 500`, `Variax 600` and `Variax 700` are already snapshot aliases. The generator explicitly adds these four spellings when absent from the underscore input. |
| In the snapshot only, relative to both drafts | `POD HD 300` and `POD HD 400` have the product role. `Workbench HD` has the family role. |
| Exact repeat | `TonePort` occurs once on underscore line 162 and matches two snapshot product rows, at ordinals 164 and 165. Preserve both ordinals. |
| Spelling collisions | Case-folding and removing non-alphanumeric characters yields 19 groups of distinct spellings. The JSON report lists every group, along with each spelling's exact source rows and roles. These groups flag candidates for review. |

The report accounts for every input row, including draft-only spelling
variants relative to the other draft. Each exact spelling has its snapshot
ordinal or ordinals and both drafts' original line numbers. Snapshot ordinals
start at one after the two header lines. Names are matched exactly before
the spelling-collision pass. For example, `Variax 300 600` and
`Variax 300/600` share a collision key. The separate `Variax 600` alias
keeps its own ordinal and empty identifiers; hyphen draft line 152 carries
a historical family reference. The comparison does not import the draft's
numeric fields or infer equivalence from a collision.

## Reproduce the checkpoint

Run from this repository with both pinned commits present. The script uses
Git blob bytes, so Git checkout newline settings do not affect the result.
It emits JSON to stdout with source digests and row references.

```sh
set -o pipefail
python3 scripts/reconcile_device_inventory.py | sha256sum
```

Expected SHA-256
`2595a15cc64f5af580c47e9cc1c545ad7aab305eb2f89f1d935a66c71a01522e`.

## Public inventory candidate

[Schema 3](../research/device-inventory.tsv) uses the approved #96 name and
role projection in its original order. The generator pins the SHA-256 of that
projection and both drafts as tracked at
`a55a70a854f56fcfd3c9e3582ce746416f8a3884`.
[The identifier input](../research/device-inventory-identifiers.tsv) cites a
line or range at that same revision for each of the 27 populated identifier
fields. These are published catalog and driver references. They carry no new
claims about live identity, alias equivalence, audio routes or access capability.

`ordinal` is the one-based row in the frozen #96 projection. The draft columns
give one-based line numbers joined by exact spelling. A draft line can map to
multiple ordinals. Underscore line 162 maps to both `TonePort` product rows.
All 392 draft lines remain traceable. Draft identifier and status fields stay
in their source files. All 186 aliases have `-` in each identifier column.
Elsewhere `-` means the candidate carries no value or draft reference in that
cell. `unresolved` marks a known gap in the reviewed identifier inputs.

| Unresolved field | Frozen ordinals and names |
| --- | --- |
| USB PID | 18 Bass PODxt Live |
| Line 6 product ID | 17 Bass PODxt, 19 Bass PODxt Pro, 50–51 Flextone II and III, 56–57 GuitarPort 1 and 2, 59 HD147, 84–86 POD 1.x, POD 2.0 and POD 2.0 Pro, 112 PODxt, 114 PODxt Pro, 164–165 both TonePort rows, 209–211 Vetta II, Vetta II Combo and Vetta II Head |

Run from the repository root. Generation uses tracked checkout files. The
citation test also requires the pinned revision in the local Git history.

```sh
python3 scripts/build_device_inventory.py --check
python3 -m unittest discover -s scripts -p test_build_device_inventory.py
```

The candidate SHA-256 is
`cd6a6182fb45145654a734ea30709f4f943e6e20751a4d223498ff0a312ff61c`.
Running the generator without `--check` writes canonical LF bytes to stdout.

## Gatekeeper review

The historical generator at
`508cd000cc7f0cb074cde0056a8b1a52fab4cbdf:scripts/build_supported_devices.py`
uses two preserved decompiled Workbench `L6ProductConstants` inputs and a
local extract of Monkey hardware definitions. The private rebuild matched
the frozen schema-2 hash. Publication approval for historical identifier
inputs remains open. The schema-3 candidate carries the reviewed public
identifiers and preserves the frozen names, roles, and ordinals. Historical
capability and device-test columns remain in the private snapshot.

The existing [#96 projection](../research/audio-inventory-schema2.tsv),
source pin, and consumers remain in place. After Gatekeeper review, #96
adoption can reference the reviewed path and output hash while preserving the
historical source hash and ordinal mapping.
