# POD Farm 2.58 concrete plugin-state provider

This finite pass closes the AU provider binding left open by the
[setter/state trace](pod-farm-setter-state.md) and compares the supplied full
i386 VST/RTAS state paths. The provider payload is an **XFP6 state envelope**,
not a FORM/L6P2 document. Its tone sections reach the patch-reader family, but
that connection does not establish whole-session preservation or format
interchangeability. #40 remains partial/open and #29 remains held.

Everything below is static source evidence or an explicitly labeled owned
reconstruction. No component, vendor library, initializer, authorization or DSP
routine ran. No additional CPU emulation or hardware/audio access occurred.

## Pinned inputs and coordinate spaces

| Input | SHA-256 | Coordinates |
| --- | --- | --- |
| Full AU x86_64 slice | `715d30b8fed77d4ea18150050131db759b8e8a27eeaf4e3d9b3872b6bca6fbe0` | AU VA; fat offset `0x282000` |
| Full VST i386 thin file | `e68b3d63d0cda33d6d3e3443bfd472fa412ac1fa5ae7636d7d532ed16369a0c4` | VST VA/file offset |
| Full RTAS i386 thin file | `950cc1522d9cfe4fbbba0be5f8f9e3f5825fe6278d45a1f16baebc62fdc81c88` | RTAS VA/file offset |

All are from the unchanged supplied 2.58 installer and were checked against its
private inventory before static import. VST/RTAS were copied without executing
or modifying their originals. Ghidra 12.2 analysis was bounded to 60 seconds per
new program; follow-up selections used `-noanalysis`. GNU objdump supplied
independent raw-instruction checks. No conclusion here transfers to Windows,
Elements, other architectures/releases, DAW behavior or a physical device.
Names such as “provider” and “state envelope” describe recovered roles; original
class names have not been recovered for these stripped paths.

## AU registration resolves the indirect receiver

AU constructor `0x9433c` constructs a secondary interface at AU object + `0x210`
through `0x759a0`. Its +8 field is the provider pointer previously seen at AU
object + `0x218`. It begins null. Derived AU vtable `0x244e50` has initialize
slot `+0x28` → `0x94020`; that path invokes secondary slot `+0x70` through
vtable `0x2451e0` → thunk `0x297f4` → body `0x29382`.

For the admitted configured count pairs (1,1), (1,2), (2,2), that body allocates
`0x7d0` bytes and calls constructor `0x436ee`. It stores this concrete pointer
at AU + `0x218`. The constructor installs primary vtable `0x246950`:

| Provider slot | Concrete target | Static role |
| --- | --- | --- |
| `+0x10` | `0x3cdb2` | State save adapter |
| `+0x18` | `0x3ce34` | State restore adapter |
| `+0x20` | `0x44688` | Initialization with the prepared host configuration |
| `+0x28` | `0x4164a` | Teardown callback |
| `+0x30` | `0x699a6` | Ready iff provider + `0x10` differs from -1 |

Existing providers take a reconfiguration branch instead of allocating again.
Unsupported count pairs and initialization/parameter-list failures affect the
secondary ready flag at + `0x18`; allocation alone is not observed readiness.
`0x75a3e` calls provider teardown then its deleting destructor, clears the
pointer and flag. This identifies a concrete reachable receiver, not an instance
created during this analysis.

The save/restore adapters construct a temporary state object with `0xc8c9a` →
`0xc8c4c`, binding provider as context at state object + `0x100`. They invoke
`0xc7f5a` / `0xc8ccc`, copy a result/detail word to provider + `0x31c`, then
destroy the temporary. The earlier opaque-provider edge is therefore resolved
through constructor, stored pointer and vtable dispatch, not by proximity to a
two-tone serializer.

## XFP6 prefix and sections

AU prefix constructor `0xc769a` initializes a 244-byte (`0xf4`) record. Reader
`0xc8a20` requires an actual 244-byte read, recognizes integer signature
`0x36504658`, or swapped `0x58465036` followed by `0xc8588`, and requires the
declared header length to be at least 244. On these Intel files the corresponding
signature bytes are `XFP6` and `6PFX`.

The following offsets are **within the serialized prefix**, not the temporary
C++ object (whose header starts at +8 in AU and +4 in i386 copies):

| Prefix offset | Recovered role |
| --- | --- |
| `0x00` | Signature |
| `0x04` | Declared header length, initialized/written as 244 |
| `0x08` | Advertised total, updated from stream position |
| `0x0c` | Save timestamp word |
| `0x10`, `0x14` | Context-sourced words; no new product-identity interpretation assigned |
| `0x18`, `0x1c` | First tone offset/length |
| `0x20`, `0x24` | Optional second tone offset/length |
| `0x28`, `0x2c` | Optional mapping section offset/length |
| `0x30`, `0x34` | Additional host/settings section offset/length |
| `0x6c`–`0xeb` | Fixed 128-byte name area; writer zeroes it and copies at most 127 bytes |
| `0xec`, `0xf0` | Final context words; initialized to 1,1, later replaced from provider context |

Other scalar words are retained as raw numeric fields here; not all their
semantics are proved. Byte swapping covers 27 prefix words and the two final
words, leaving the 128 name bytes unchanged. Acceptance of the swapped prefix
does not prove compatible embedded chunks from every architecture.

Save `0xc7f5a` first writes a placeholder prefix, builds a temporary dual-tone
object through `0xca07c`, and invokes **each child patch's** writer separately.
It does not call the dual-tone outer serializer `0x1a17fc` here. It then writes
mapping/settings sections, records positions/lengths and rewrites the fixed
prefix at absolute stream position zero. The observed callers provide fresh
memory streams; arbitrary embedding at a nonzero initial position is not a
supported contract established by this path. Timestamp, fixed name handling,
host-context words and rewritten headers preclude a byte-identical state claim.

Restore `0xc8ccc` resets a temporary prefix, seeks zero and reads it. On header
failure it restores only that temporary prefix. After a valid prefix it applies
context/scalar state (`0xc79c8`), then tone restoration (`0xc7d64`), settings
(`0xc77cc`) and mapping (`0xc783c`). Later failure does not roll back earlier
host-state changes. `0xc78b8` obtains a substream for each declared tone span,
constructs a patch and calls its reader; these are the same native patch-reader
paths previously traced. Some downstream failures are recorded in a detail word
while the adapter returns success, including the tone-application path.

The prefix reader does **not** prove advertised total equals actual length,
declared header extensions are present, or every span is nonnegative,
nonoverlapping and contained. Larger declared header lengths are accepted after
reading only 244 bytes. Stream methods mediate later reads. The AU borrowed
memory reader at `0xfc844` caps a read at remaining length, but seek `0xfc8f6`
does not reject an out-of-range position. Subview builder `0xfc9e4` uses
signed/unsigned size arithmetic and truncates a constructed length to 32 bits;
it is not a general validation barrier for arbitrary hostile offsets. No such
input was executed and no malformed-state compatibility/safety claim is made.

## AU ownership, identity, failure and deferred state

The outer [property-0 dictionary checks](pod-farm-setter-state.md#full-au-dictionary-state-is-another-boundary)
still apply. They are distinct from XFP6's signature/minimum-header check; a
recognized XFP6 prefix alone does not identify a product or authorize restore.

Save `0x9382c` creates a fresh owned memory stream and calls the now-bound
provider. It obtains pointer/length through `0xfc0a4` and copies the bytes into
CFData. It can still return a dictionary with empty data and success after a
negative provider save result. This outer behavior is not repaired by resolving
the receiver.

Restore `0x93a4a` obtains CFData pointer/length. Its immediate memory-view
constructor `0xfc978` stores the source pointer and a 32-bit length; deleting
that view clears its pointer without freeing the source (`0xfc77a`). The call
is synchronous while the dictionary owns the CFData. Length conversion to this
view does not establish support for CFData larger than 4 GiB. Ready-provider
restore calls the provider and then refreshes wrapper parameters; its return is
ignored, so outer success remains weaker than successful application.

The deferred branch copies into an owned growable stream using `0xfc286` and
caches it at AU + `0x238`. It does not retain a borrowed CFData pointer. An
eventual cached-state consumption/application edge has **not** been proved in
this bounded pass: identified direct uses cover creation, save/restore and
destruction, and an aliased/indirect consumer remains possible. Required missing
evidence is the consumer/callback binding and host readiness/call sequence, not
another installer. Do not label cached bytes as applied or conclude that they
can never be applied. No host/plugin execution was used to resolve that gap.

## VST and RTAS comparisons preserve ABI differences

VST `_VSTPluginMain` at `0x2c654` → factory `0x29cac` → constructor `0x29a6e`
returns its AEffect at object + `0x20`. The callback `0x2a2bc` routes through
virtual dispatcher `0x2bed6` and base dispatcher `0x2a44c`. Opcodes `0x17` and
`0x18` invoke slots `+0x54` / `+0x58` → `0x970ba` / `0x971d0`.
Initialization `0x96f08` → secondary-interface thunk `0x29f30` → `0x2982a`
allocates `0x59c` bytes through `0x49392` → `0x4908a`, stores provider at
wrapper + `0xb4`, and installs provider vtable `0x21d3a8`. Its slots `+8` /
`+0xc` resolve to `0x41808` / `0x4187e`; these call XFP6 save `0xce36a` /
restore `0xcf20e`. Prefix reader is `0xcef38`, initializer `0xcd9ac`.

VST getter deletes its previous cached stream, serializes anew and returns
its buffer pointer and low 32-bit length. That pointer is backed by the retained
stream, invalidated at least by the next getter; it is not caller-owned memory.
On provider failure it deletes the stream and returns length zero. Setter
requires nonzero provider, data pointer and byte count (not a proved positive
signed-size check), creates a borrowed view and returns 1 for a nonnegative
provider status, 0 for a negative status. A positive informational status is
therefore treated as success. This differs from AU's ignored provider status.
The examined handlers do not establish distinct bank/preset chunk semantics.

RTAS export `_NewMachOPlugIn` at `0x33626` creates a plugin controller through
`0x2b74a` / `0x2b616`. A concrete process-side factory `0x2b3e6` allocates
`0x59c` bytes with `0x5885e` → `0x58556`, stores provider at process + `0x2c8`
and installs vtable `0x2c6148`. Its save/restore slots `+8` / `+0xc` resolve
to `0x50cd6` / `0x50d4c` → XFP6 `0xe4ef6` / `0xe5d9a`; prefix reader is
`0xe5ac4`, initializer `0xe4538`.

The RTAS outer chunk adapters are also concrete, not inferred from XFP6 alone:

| RTAS method VA | Examined behavior for matching chunk id (process + `0x2f4`) |
| --- | --- |
| `0xac1c2` | Serialize into cached stream at + `0x324`, return payload size + 56; propagate negative provider/size result as failure and delete failed cache |
| `0xabf98` | Copy cached payload into caller chunk after its 56-byte prefix; update total length; cache/data-access failure returns an error |
| `0xac034` | Wrap bytes after the 56-byte prefix and call provider restore; negative provider result returns failure; successful payload is copied to owned cache |
| `0xac304` | Compare incoming payload with cached state using provider + `0x7c`, not a whole-buffer byte equality contract |

Nonmatching ids delegate to SDK/base methods. The id's producer/complete outer
prefix semantics and host-supplied buffer lifetime/capacity contract are not
resolved here. Restore checks only total length >=1 before subtracting 56 in
the examined branch; that is not proof it rejects every short outer chunk.
The copy path trusts the caller's destination capacity. Host execution and a
successful RTAS roundtrip have not been observed. `__PI_GetRoutineDescriptor`
is descriptor data; an attempted code selection there failed and is retained as
rejected evidence, not a recovered executable state entry point.

These three products demonstrably contain related XFP6 readers/writers. Their
different wrappers, context words, errors, architecture widths and host inputs
prevent a claim of interchangeable or losslessly preserved plugin state.

## Focused verification and terminating gaps

The owned [prefix helper](../scripts/pod_farm_state_prefix.py) models only fixed
prefix discrimination/word projection and the VST sign-bit status projection.
Eight [synthetic checks](../scripts/test_pod_farm_state_prefix.py) pass: little/
swapped-endian scalar handling with unchanged name bytes, short read, signature,
minimum header length, unvalidated advertised total, raw span/tail handling and
VST status classification. They deliberately demonstrate how prefix success is
weaker than whole-state validity. There is no native oracle, whole codec,
deferred-state simulation or original-state fixture.

```sh
python3 -m unittest discover -s scripts -p test_pod_farm_state_prefix.py -v
```

The owned Ghidra selector now accepts `refs:ADDRESS` for known static data/code
references in addition to the existing write-only selector. For already imported
hash-pinned programs, reproduce with headless `-process NAME -noanalysis
-scriptPath scripts -postScript PodFarmEdge.java --exact` and these selections:

```text
AU: 000297f4 000436ee 0003cdb2 0003ce34 000c7f5a 000c8ccc 000c8a20
VST i386: 0002982a 0004908a 0002a44c 000970ba 000971d0 000cef38
RTAS i386: refs:002c6148 0002b3e6 000ac1c2 000abf98 000ac034 000e5ac4
```

Static reports and raw ranges remain in ignored
`analysis/pod-farm-258/native-edge/reports/`; nothing original-derived is used
as a tracked fixture. Rejected mid-function/data selections are retained and
distinguished from the accepted paths. This pass resolves the provider/payload
edge but stops short of deferred AU consumption, complete RTAS outer identity/
ownership contracts, all section semantics/conversions and cross-product
preservation. No new source artifact is declared missing.

Private manifest (58 new reports, including failed selections and test log):
`provider-evidence-7b681afa555e7441790b9526b9504f312cee016a1413ea88975c7a0784b06f77.json`.
Focused eight-test log SHA-256:
`66b0b0129c6a0a41ea28c8258d9a1a4574e0f0f52fa0662e4f446606e3fc5584`.

The previous conditional registered-control clamp, unnamed third scalar word,
PINF/name losses and L6P2 unknown/duplicate/order losses remain in force. No active
pack/registry state, default/step/unit meaning or PODxt applicability follows.
No #29 code, production dependency, default math or guard changed. Publication,
full command-v1 checks and independent exact-head review remain held/required
under the existing coordinator instruction; this is not acceptance or release.
