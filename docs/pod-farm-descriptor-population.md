# POD Farm 2.58 descriptor population: one system-control path

This report records an earlier research pass. Its publication/check hold statements
are historical and superseded by the [current publication and check status](pod-farm-acceptance-coverage.md#current-publication-and-check-status).
Scientific limits, incomplete acceptance and the #29/hardware boundaries remain.

This continues the [property lookup trace](pod-farm-parameter-edge.md) for #40.
It establishes a concrete named descriptor and new-patch seed, and the producer
of its generic scalar metadata. The subsequent
[cache/serialization trace](pod-farm-cache-serialization.md) resolves concrete
metadata endpoints in the supplied resources. This does not establish a complete control domain
or PODxt Live applicability. #40 stays partial/open; #29 stays held.

All addresses here use the previously documented **x86_64 slice** coordinate
system, SHA-256
`dced9e6bee1ac57ba9135c1b8751d26141be82155122f7aa151b2cb457bd55a1`.
The source framework hash remains
`1b6acb3aa012d9b884f00daeacc948aa80884c81f513871be740055cd629a924`.
These are static instruction/xref findings, checked with Ghidra 12.2 and GNU
objdump 2.47. No new vendor CPU execution occurred, including emulated execution.
The earlier two-leaf oracle was not expanded. No i386 producer correspondence is
claimed in this continuation.

## From instance construction to a named descriptor

The examined instance initialization path at `0x43270` calls table population
`0x5d460`. For its raw kind argument `0x7d080006`, the latter reaches `0x5d030`,
which calls `0x537a0(table_root, selector, 0, 0)` at `0x5d078`.
This is a traced call path, not a product identity assigned from that kind word.

`0x537a0` zeroes a `0x198`-byte local descriptor template. It installs system
records into the same five groups and ordinal layout consumed by `0x50b00`.
The builder around `0x53a17`–`0x53a58` associates:

| Field | Proven value on this path |
| --- | --- |
| Descriptor name at record + `1` | `Sys-In Mute` |
| Tagged identifier at record + `0x108` | `0x3f20002a` |
| Logical module at record + `0x110` | `0` (third builder argument) |
| Logical ordinal | `3` (fourth builder argument + 3) |

The producer copies the template into a selected group, or all groups for selector
`-1`, and marks the record present. When replacing a present record, its old name
and old current word can be retained: the saved group-relative `+0x148` is
**record-relative `+0x130`**, because records start at group + `0x18`.
The corresponding generic merge at `0x50d80` matches **both module and tagged
identifier** before replacing a descriptor. A matching ordinal alone is not a
stable identifier, and the low 24 bits of a tagged word are not an ordinal.

The consumer resolves the logical module through instance vtable + `0x200`
(`0x235390` → `0x3f800`), called from `0x3fb70`. For the examined instance-kind
branch `0x1000`, module 0 resolves to model slot `0x04010000`. This is a slot in
the native host patch model, not a physical preset/bank location.

## One typed new-patch seed and its consumer

The patch-building function at `0x49fe0` prepares a typed value and calls its
parameter insertion/update helper at `0x175e7c`. The raw instructions give a
specific initialization record:

| Call at `0x4a0ed` | Value |
| --- | --- |
| Model-slot argument | `0x04010000` |
| Tagged parameter argument | `0x3f20002a` |
| Value type tag at local typed-value + 0 | `0` |
| Value payload word at local typed-value + 8 | `1` |

`0x175e7c` copies the typed value into an existing parameter record or appends a
new one, keyed by identifier, and marks it present. `0x1767f4` subsequently finds
the record and returns those typed-value words. The descriptor getter reaches
that read through `0x2bd10`. The separately traced scalar builder treats type 0
as a signed integer representation; this is a typed integer seed, not a floating
point sample interpreted from its magnitude.

The system-slot consumer `0x63a50` compares the same slot/identifier pair. Its
path at `0x63b60`–`0x63bab` reapplies cached byte fields at object + `0x477` and
`0x478` through virtual offset `0x3f0`. This corroborates use of the named system
control in the host state path. It does not prove a complete set of accepted
values, boolean clamping, transfer direction to physical equipment, or Linux
audio behavior. The constructor's seed value 1 is not the current value of a
loaded user preset and not a recovered descriptor default-range field.

## Where scalar properties actually come from

Template names and zeros do not supply domains. The metadata population function
`0x41240` obtains a model-definition pointer through `0x24ba0`, walks its parameter
definition array at + `0xe8` with bounds + `0xf4`/`0xf8`, and reads the current
typed patch value. It requires that value's type tag to match definition +
`0x128` before constructing the descriptor. It invokes instance slot `0x310`
at `0x413f0`; vtable entry `0x2354a0` resolves to `0x40d80`.

That producer copies a concrete definition's scalar words into the descriptor:

| Definition source | Descriptor destination | Supported interpretation |
| --- | --- | --- |
| `+0x128` | flags at `+0x118` | Type 0 → `0x10000000`; type 1 → 0; type `0x10` → `0x20000000` |
| `+0x12c` | `+0x120` | Lower endpoint used by generic display normalization |
| `+0x130` | `+0x128` | Upper endpoint used by generic display normalization |
| `+0x134` | `+0x138` | Copied metadata word; default/step/other meaning still unverified |
| Current typed-value argument | `+0x130` | Current value, distinct from all three metadata source words |

For type 0 the producer sign-extends 32-bit words; type `0x10` zero-extends them.
For type 1 it copies raw 32-bit float words into the previously zeroed descriptor,
without a host transcendental-math call. Other native tags use another branch,
whose semantic category remains unclassified; the owned research decoder rejects
them. A model-specific branch can further override the generic flags/callbacks.

The builder installs formatting/parsing callbacks `0x414b0` and `0x41670`.
The former uses the two endpoints for percentage normalization; the latter uses
the inverse interpolation. This substantiates endpoint roles, not a unit for
`Sys-In Mute` or any other specific control. After descriptor merge, `0x41240`
also calls instance slot `0x2f8`, so this generic projection is not proof of final
model-specific display behavior. It does not validate interpolation for malformed
endpoints or establish cross-platform math compatibility.

The model-definition cache returned by `0x24ba0` uses the tree at cache +
`0x38`/`0x40`. The subsequent cache trace follows its construction to the actual
`LOUT`/`LOBJ` resource records and resolves the final override for this control.
The descriptive cache label remains inferred; original class identity is unverified.
That later result establishes endpoint words without inventing values from
template zeros, and leaves the third scalar component's default/step meaning open.
No additional artifact or live query is inferred necessary.

## Focused checks and boundaries

The owned offline helper [metadata words](../scripts/pod_farm_metadata_words.py)
reconstructs only the three classified scalar-copy branches on synthetic input.
It is separate from the Rust app and does not parse tone files or call a library.
Six [synthetic tests](../scripts/test_pod_farm_metadata_words.py) pass: signed
extension with distinct current/metadata words, unsigned extension, exact float
bit preservation (including negative zero and a NaN payload), unknown-tag
rejection, truncation and input-word bounds. This is a reconstruction check,
not execution of the native producer or a compatibility oracle.

```sh
python3 -m unittest discover -s scripts -p test_pod_farm_metadata_words.py -v
```

Five additional static checks confirm the actual seed call's identifier, slot,
type tag, payload and call target in objdump output. Private content-addressed
validation report:
`analysis/pod-farm-258/native-edge/reports/population-validation-d1d0eab68d36f464c05f7c554c0104018c86788db37c1bc6906b5699abababe1.json`.
Its check log SHA-256 is
`89572fddc4e9cf8fc3d567c16f5604fe8f011a90d194b631dd8f0f8c7551aa8e`.
New `population-<hash>.log/.asm` reports preserve source decompilation and raw
instruction evidence privately. An initial selection at mid-function `0x40da7`
was rejected as a complete function; a fresh project traced the actual entry
`0x40d80`. The failed/incomplete selection remains preserved.

No application runtime, USB/MIDI/audio adapter, hardware lock, dependency or
default math changed. No vendor initializer, plugin, authorization or DSP routine
ran, and no hardware or stream was contacted. Publication and full required CI
remain held by the existing coordinator instruction; independent review remains
required. The named host control and seed do not authorize #29 editing, establish
PODxt applicability or make the full #40 acceptance criteria complete.
