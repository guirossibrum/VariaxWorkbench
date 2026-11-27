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

## Gatekeeper review

The historical generator is
`508cd000cc7f0cb074cde0056a8b1a52fab4cbdf:scripts/build_supported_devices.py`.
It parses two matching decompiled Workbench `L6ProductConstants` inputs
and a local extract of Monkey hardware definitions before rendering the
full table. Those three inputs are absent from the pinned trees and fall
under the repository's local-material rules. The hyphen argument serves
only as a diagnostic report input. The comparison reproduces the frozen
names and roles from committed evidence. Full generator reproducibility
and a publication-ready path for the unified inventory remain pending.

For review, retain `research/audio-inventory-schema2.tsv` as the existing
#96 accounting reference. Preserve the two drafts and the frozen ordinals
through the review. A future unified path and generator require an approved
input set before adoption and any #96 path or pin update.
