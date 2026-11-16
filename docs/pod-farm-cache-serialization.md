# POD Farm 2.58 cache, override and serialization findings

This report records an earlier research pass. Its publication/check hold statements
are historical and superseded by the [current publication and check status](pod-farm-acceptance-coverage.md#current-publication-and-check-status).
Scientific limits, incomplete acceptance and the #29/hardware boundaries remain.

This consolidates the source chains following the
[descriptor producer trace](pod-farm-descriptor-population.md). One host control
now has concrete signed integer metadata endpoints, and the patch reader/writer
has a concrete PINF preservation limit. These are offline static findings and
owned reconstructions, not native execution or full issue #40 acceptance. #29
remains held: none of this establishes PODxt Live parameter applicability.

The subsequent [setter and outer-state pass](pod-farm-setter-state.md) resolves
conditional registry clamping and the separate L6P2 container path, and identifies
AU dictionary-state entry points and preservation/error limits. Its 17 focused
synthetic checks include the nine below; they are not additional native executions.

Native addresses use the x86_64 slice coordinate system, SHA-256
`dced9e6bee1ac57ba9135c1b8751d26141be82155122f7aa151b2cb457bd55a1`, from framework
SHA-256 `1b6acb3aa012d9b884f00daeacc948aa80884c81f513871be740055cd629a924`.
The original files are unchanged. New native CPU execution was zero; the earlier
two-leaf emulation scope was not expanded. Ghidra 12.2 and GNU objdump 2.47 were
used as static readers. Class-purpose labels below are inferred from consumers;
they are not recovered original class names.

## Cache construction reaches actual resource records

Instance initialization calls `0x27d40`. It reuses a cache when three interface
identity words match, or allocates one through constructor `0x27970`. That
constructor initializes the model lookup tree at cache + `0x38`/`0x40` and calls
`0x277c0`. The latter scans the configured resource directory through `0x274b0`.
Its file callback `0x23aa0` recognizes header category `0x800000`, which header
classifier `0x185e96` assigns to `FORM/L6RB`.

Loader `0x252b0` reads headers, applies selection/authorization filters and reads
the selected resource body. This analysis did not run or bypass those filters.
It follows the data construction statically; active selection, entitlement and
installed device/host state remain unknown.

The full resource reader is `0x189a26`, reached through document slot `0x38` by
`0x172386`. It calls layout reader `0x1885e8`. The earlier header-only path at
`0x189734` is distinct. Layout parsing walks `LIST/LAYO` → `LIST/LOUT`, reads
a `LOUT` header prefix of `0x9c` bytes and each `LOBJ` prefix of `0x114` bytes,
converts selected numeric words from file byte order, then builds typed scalar
objects. The model cache insertion path in `0x252b0` calls `0x20810` → `0x201d0`;
the latter constructs parameter definition objects with `0x20040` and populates
the same definition-pointer array consumed by `0x41240`.

This explains why the resource's UI/layout records matter: their parameter
metadata is actually copied into the cache. It is not an assumption based on a
matching label or a nearby constant. It does not make every UI object a physical
device control.

## Concrete `Sys-In Mute` metadata

The previous trace established logical module 0, ordinal 3, tagged identifier
`0x3f20002a`, host model slot `0x04010000`, and a typed integer new-patch seed 1.
The patch constructor also places model `0x3e700003` in that slot. Three supplied
resource records agree on this model/parameter pair:

| Resource in framework `Versions/A/data/res/` | SHA-256 | LOBJ payload offset |
| --- | --- | --- |
| `GP2.astwx` | `e8f81fec37a1f49ec9ec9fbef0e96c4fa9c7cf317f7d811ec411a3311f94d91c` | `0xe80c` |
| `ilxstd.astwx` | `eb1de8360f25e618e2856f3f2d53495497f1fe3af8e6ca7a470a39e88724783a` | `0x125dc` |
| `Z5Std.astwx` | `6fd22ba24520ba49a5c1279f0095ed19886cf3d27dc4c3dad7ec93256209c9b1` | `0x7b44` |

Offsets are file offsets within each resource, not native-code VAs. Each payload
is 276 (`0x114`) bytes, inside a layout whose model word is `0x3e700003`.

| LOBJ payload offset | Observed big-endian word | Traced role |
| --- | --- | --- |
| `+0x54` | `0x3f20002a` | Tagged parameter identifier |
| `+0x58` | 0 | Signed integer scalar branch |
| `+0x7c` | 0 | Lower endpoint |
| `+0x80` | 1 | Upper endpoint |
| `+0x84` | 1 | Third range component and initial scalar value; further meaning unverified |

Layout reader `0x1885e8` passes the last three words to typed scalar construction
`0x20e10` for tag 0. The typed object's range setter (vtable + `0x50`, target
`0x20d50`) stores lower/upper/third component at scalar + `0xc`/`0x10`/`0x14`;
its value setter (`0x20d20`) separately stores the current integer at + `0x18`.
Parameter definition construction places that scalar at definition + `0x120`.
Consequently the generic descriptor producer `0x40d80` sees signed type 0,
endpoints **0 and 1**, and word 1 at definition + `0x134`.

These are proven **metadata endpoints**, not proof that every setter rejects all
other integers. The producer initializes the third component and current value
from the same file word; this does not establish whether the third component is
an invariant default, step, reset value or another scalar attribute. No reset/
step consumer proving that meaning was recovered in this chain. The new-patch
seed, that component, and the later refreshed current word remain distinct.
Units and specialized scaling are also unverified. Resource names do not prove
PODxt support or license transfer; no active pack state was inspected.

## Final override is bounded to a different identifier

The final callback slot `+0x2f8` resolves through pointer `0x235488` to `0x40b00`.
After a conditional instance query it builds an auxiliary `Pedal` descriptor
with identifier `0x3f000003` and merges it via `0x50d80`. That merge requires
both module and tagged identifier to match. Thus this examined override does
not replace `Sys-In Mute` (`0x3f20002a`), even if their module words coincide.
This closes the previously unresolved override edge for the concrete definition;
it is not a statement about every callback or model-change path.

## The patch reader is a converter, not a lossless backup contract

Direct patch reader `0x17ac94` dispatches using format classification. Normal
`FORM/L6PA`, `L6FS` and `L6AS` classify as `0x400005`, `0x400002`, `0x400003`
and reach reader `0x178be0`. `L6P2` classifies as `0x410003` and has no matching
branch in this direct reader. That does not mean the full application cannot
open it: a separate bank/container path is required and remains untraced here.
Classifier categories are not product identities or device operation codes.

`0x178be0` zeroes a 76-byte PINF buffer and calls chunk-copy helper `0x185a70`
requesting 76 bytes. That helper reads the smaller of requested and declared
chunk lengths, and seeks past the rest when lengths differ. It returns the
search status; it does not impose equal lengths or propagate a separate short
stream-read failure at this point. This is static control-flow evidence, not a
native malformed-input test or permission to relax Rust's input validation.

The reader forces the last of 32 UTF-16 name units to zero. Writer `0x176416`
zeroes a fresh 76-byte PINF, writes the three stored words and copies at most
31 name units, stopping at the first NUL. If the stored device-version word is
zero, it substitutes the current global version word. It writes exactly 76
PINF bytes through `0x185c88`. Consequently a PINF extension after byte 76 has
no retained field on this path and cannot survive that PINF write. A 32nd
nonzero name unit and hidden name units after an embedded NUL are also outside
the serialized name. This is a concrete preservation limit; it is not evidence
that all unknown chunks elsewhere are lost or preserved.

The same reader explicitly recognizes target `0x0003000a` in a compatibility
branch shared with two other targets, and invokes model-state adjustment
helpers. Other branches swap selected old Line 6 Edit parameters or adjust
models for target `0x00130001`. These conversions are another reason not to
claim a semantic or byte-perfect whole-tone roundtrip from native import/export.
They establish file-target handling, not PODxt hardware parameter applicability.
Native MINF/PARM reads request 12-byte prefixes; full unknown-chunk behavior and
L6P2/plugin-state preservation remain unresolved.

## Focused verification and remaining scope

The owned [resource contract helper](../scripts/pod_farm_resource_contract.py)
checks the three exact resource hashes and selects the explicit model/identifier
pair. It confirms one signed 0/1 endpoint record in each source. It never runs
the native loader or its authorization filters. Its resource parser is stricter
than the native prefix-copy routine and is not presented as a compatible importer.

```sh
python3 scripts/pod_farm_resource_contract.py
python3 -m unittest discover -s scripts -p test_pod_farm_resource_contract.py -v
```

Nine owned synthetic checks pass: model/identifier selection, non-substitution,
truncation, short LIST types, opaque odd chunks, PINF extension loss in the
reconstructed field path, last-name-unit termination, embedded NUL/short-prefix
handling, and explicit synthetic context for zero-version substitution. The
PINF functions model only that local field projection, not a native execution
oracle, full serializer or semantic roundtrip. Source bytes remain unchanged.

Private resource result:
`resource-contract-cf929594acec8d0ad6025488bc3f93a70e07d5334d2f052456baccfc83e8a77d.json`.
Private consolidation manifest:
`consolidation-evidence-66987962bfd3dce11bf8df02b46454e654294689b5e84c3b8be18cb15eb7ea2e.json`.
Both are in ignored `analysis/pod-farm-258/native-edge/reports/`; the manifest
records 30 content-addressed reports, raw-instruction checks and the nine-test
log (SHA-256 `ba8b3e94b8ffa76dd12834787a4bdb8554680ecf2167b1e62570f2eb321e4845`).
No proprietary payloads, instructions, pseudocode or presets are committed.

The concrete cache and override chain is resolved to these supplied records;
there is no new missing-artifact request. Default/step meaning, units, strict
setter behavior, other model domains, pack selection, hardware applicability,
L6P2/plugin state and whole-document unknown preservation remain unmet. No
product APIs, runtime dependencies, math defaults or hardware guards changed.
Publication and required full checks remain held; independent review and full
#40 acceptance are still required. No device, audio stream, driver or licensing
operation occurred.
