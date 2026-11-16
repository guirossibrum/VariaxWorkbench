# POD Farm 2.58 tone conversion and #40 coverage

This finite static continuation follows the concrete XFP6 tone reader, not a new
inventory sweep. Source: full AU x86-64 slice SHA-256
`715d30b8fed77d4ea18150050131db759b8e8a27eeaf4e3d9b3872b6bca6fbe0`.
All addresses in the tone sections below are AU virtual addresses/slice offsets.
Ghidra 12.2 noanalysis selections and GNU objdump raw decoding supplied static
evidence; no vendor routine, native library or new CPU emulation was executed.
The original artifact and original-derived reports remain private/ignored.

**Restore is not atomic. Mapping/settings advertised lengths do not cap their
reads. A successful prefix or child parse does not establish whole-state validity,
lossless preservation, actual host application or PODxt compatibility.** #40 stays
partial/open, #29 held; publication/full CI/review holds remain in force.

## Concrete tone dispatch

Both XFP6 tone spans enter `0xc78b8`, which constructs `0x190a1a` via `0x190b98`.
The resulting vtable `0x2579b0` selects read +`0x38` = `0x19523c` and write
+`0x40` = `0x19009a`. The reader calls format detection `0x18eb2c`, whose
container discriminator `0x1a0a6c` has these relevant results (both FORM byte
orders are recognized):

| Form type bytes | Internal category | Reader selected by `0x19523c` |
| --- | --- | --- |
| L6GP | `0x400001` | Legacy document `0x18e0cc`, then projection `0x193fbc` |
| L6FS | `0x400002` | `0x19303c` |
| L6AS | `0x400003` | `0x19303c` |
| L6PA | `0x400005` | `0x19303c` |

The L6P2 form itself yields `0x410003`; it is **not** accepted by this child-reader
switch. A separate MIDI/container detector `0x1971cc` can yield `0x400004`,
leading to `0x197bda`, child selection at index zero and `0x1947fc` projection.
Categories `0x400006`..`0x400008` read at most 256 bytes through a signature-based
legacy parser `0x191e8a` (specific lengths 168/170 and product discrimination).
Unknown category retains `0x80001019`. These legacy branches are concrete routes,
not completed legacy codecs: field mappings at `0x193fbc`/`0x1947fc` and decoding
at `0x1915fc` have not been validated with an authorized original-state oracle.
The two observed projections rebuild selected model/parameter fields and replace
product/version metadata; they are not opaque preservation paths.

## Nested L6PA/L6FS/L6AS reconstruction

`0x19303c` initializes the container stack (`0x1a0b46`), then clears existing
models, metadata and mappings (`0x18f25e`) **before** reading PINF. It copies a
prefix of PINF into a zeroed 76-byte local, decodes product (+4), version (+72),
a scalar format word (+0), and at most 31 UTF-16 name units. The 32nd unit is
forced to zero. This connects the earlier PINF preservation limits to XFP6 tones.

| Nested input | Read behavior | Preservation consequence |
| --- | --- | --- |
| PINF | First matching chunk, fixed 76-byte projection | Duplicate/extended data and name tail are not retained |
| LIST/PATC | First matching list, iterate LIST/MODL | Original outer ordering is not retained |
| MODL/MINF | First MINF, 12-byte projection, model keyed by slot | Repeated model slot returns positive `0x1003`; first model survives and outer loop continues |
| MODL/PARM | Repeated 12-byte entries, append to the parameter array | No key deduplication at append; local array capacity behavior still applies |
| LIST/UNFO | Preferred over INFO; 16 known tag searches | Unknown tags and duplicate occurrences are not generically retained |
| LIST/INFO | Fallback only when UNFO absent; narrow strings | Conversion to metadata, not original byte retention |
| MIDM | First matching chunk, known L6ME reader | Inherits mapping version/count/partial-load behavior |

`0x190eb2` changes model id `0x3e000000` to `0x3e700000`, normalizes flag bytes,
and appends PARM entries via `0x195d9c`. `0x190478` subsequently looks up the
**first** matching enabled parameter entry. Thus a retained duplicate in the
array does not establish last-value-wins semantics. Models are stored in a tree:
`0x195ac6` compares unsigned slot keys, and `0x195ed8`/`0x195f52` walk that order.
The writer therefore emits model key order, not original model chunk order.

Container search `0x1a0380`/`0x1a04e4` skips unmatched chunks using `0x19fe28`:
next = current + 8 + unsigned declared length, **without odd-byte padding**.
The skip path checks stream length < 2^32 and strict next-position bounds against
stream/container limits; it does not validate every matched chunk's full payload.
Index zero restarts at the parent payload; index -1 continues from the current
position. Repeated singleton metadata selects the first match. Search failures
can mean end/not-found or malformed input; `0x1910f6` treats any nonzero next-MODL
search result as successful termination. It propagates negative model-reader
results, while duplicate-slot `0x1003` is nonfatal. The PARM loop similarly stops
on any nonzero search result. Neither is a full nested integrity check.

The prefix-copy helper `0x1a063c` reads min(declared size, destination capacity)
but returns the **search status**, without turning a short payload read into a
new error. When sizes differ it seeks to chunk end; that seek result is ignored.
Consequently native success must not be described as strict exact-length PARM,
MINF or PINF validation. UNFO's separate reader `0x18f4a8` does check its selected
UTF-16 read; it uses floor(chunk bytes/2), so an odd trailing byte has no retained
character. The INFO branch `0x18f018` shares the prefix-copy behavior; malformed
length handling there is not validated by an owned strict codec.

Writer `0x19009a` rebuilds FORM with PINF, LIST/PATC models, LIST/UNFO known tags,
and MIDM. `0x18f75a` emits fixed 12-byte MINF and fixed 12-byte PARM only for
entries marked present. `0x18f100` emits known nonempty metadata in table order;
unknown chunks, extra fixed-record bytes, duplicate singleton fields, original
order and trailing opaque data have no generic storage/re-emission path.
The writer also updates timestamp metadata (`0x18b7be`) and may replace creator
metadata with configured globals; if version is zero it substitutes a runtime
global. It is a reconstruction with source-established losses, not a roundtrip
codec. Several decompiler signatures infer void for writer helpers; negative
caller tests do not independently prove every internal failure propagates.

## Product/version conversion boundaries

After reading, `0x18f480` calls `0x19eea2` with configured target globals
`0x2880f0`/`0x2a4da0`. Product and version come from patch +`0xcc`/+`0xd0`
(getters `0x18efb6`/`0x18f496`). The selected class function `0x19a242` has this
unsigned 32-bit discrimination, cross-checked against raw instructions through
its return at `0x19a36f`:

| Product word | Version intervals → internal class |
| --- | --- |
| `0x300`, `0x400`, `0x500`, `0x600`, `0x80000` | Any → 0 |
| `0x30002`, `0x30005`, `0x3000a` | <`0x1060000` → 1; <`0x2100000` → 2; <`0x2900000` → 3; otherwise 4 |
| `0x80001` | <`0x2490000` → 1; then shared intervals below |
| `0x80002`..`0x8000e`, `0xfffffff0` | <`0x2530000` → 2; <`0x2540000` → 4; otherwise 6 |
| `0x80001` after its early interval | Same shared intervals → 2/4/6 |
| `0xc0000`..`0xc0002` | Any → 5 |
| `0x130001` / `0x130011` | Any → 7 / 8 |
| Other words | -1 (unclassified) |

Classes are dispatch labels, not recovered product names or a device-compatibility
allowlist. `0x19eea2` selects a source/target-class conversion matrix, can reorder
legacy slots when the scalar format word is zero, sets that word to 3 and inserts
selected tagged parameters for source classes <7. Crucially those mutations
precede conversion success, and the common exit writes target product/version
even for `0x8000a009` errors. Class identity can return positive 4 rather than zero.
All conversion helpers, model substitutions and numeric equivalence have **not**
been validated. This finite pass identifies the matrix and mutation sites, not
an unrecoverable function or an exhausted artifact set.

The later apply adapter `0xc9ae8` treats L6FS/L6AS specially: it gets the target's
current patch through `0xc96d6` → `0x83e10` → imported `TWXYGetPatchData`, then
merges through `0x192262`. That merge requires destination category `0x400005`,
selects product families and copies/removes selected model slots. Some branches
are version-gated (e.g. `0x1060000`/`0x2490000`), others convert a temporary patch
before copying. Its boolean result is ignored by `0xc9ae8`. Unselected fields can
remain from the current target patch: input bytes alone do not define the output.
The serialized result then reaches `0x851aa` → imported `TWXYSetPatchData`.
These imports plus the actual initialized target globals/current patch are the
specific runtime-input boundary for application; neither import was called.

## Error and partial-mutation scope

First-tone parse/conversion failure in `0xc7d64` returns early. Second-tone failure
can leave the first eligible for application. Successful child parsing changes
only temporary documents at this point; context restoration has already happened.
When application starts, both tone slots and rebuilt mappings can change before a
later failure; there is no transactional rollback. `0xc7d64` can put a negative
apply result into its optional detail output and return zero after setting the
name. AU/VST/RTAS wrappers then project results differently, as already documented.
Settings/mapping adapters seek the original stream and use advertised length only
as a nonzero presence test; they do not limit their reader to that span. The prior
mapping reader's clear-before-validation and partial-record retention stand.

## Consolidated original-scope coverage and remaining definitions

| Original research area | Established contract/evidence | Specific remaining boundary/input |
| --- | --- | --- |
| Package/architecture/API inventory | Hash-pinned 2.58 extraction, thin-slice/import inventory, [inventory](pod-farm-258.md) | Other product/version equivalence requires independently pinned inputs; no broad source-exhaustion claim |
| Parameter lookup/domains | [Two lookup leaves](pod-farm-parameter-edge.md); [one named host control](pod-farm-descriptor-population.md); [resource endpoints](pod-farm-cache-serialization.md) | Runtime registry/pack population remains conditional; descriptor third component has no proven default/step/unit definition; no PODxt parameter mapping |
| Setter semantics | [Signed registry clamp](pod-farm-setter-state.md), typed float/integer branches | Actual target registration and all control-domain definitions absent; cannot promote host metadata into hardware ranges |
| Preset/dual-tone serialization | PINF fixed projection; L6P2 outer rebuild; nested tone reconstruction above | Unknown-field/order/duplicate losses established; no complete Rust codec or authorized native state roundtrip fixtures; per-model conversion matrix helpers remain unvalidated |
| Plugin state | [Concrete AU/VST/RTAS providers](pod-farm-plugin-provider.md), XFP6 prefix, [mapping/settings](pod-farm-xfp6-sections.md), child dispatch above | AU cached-state consumer and RTAS outer-id producer unresolved at earlier wrapper boundaries; actual host sequencing/current target patch not observed |
| Compatibility/maintenance | Historical host-DSP/controller/audio distinction in [inventory](pod-farm-258.md) | No verified host-to-POD editing protocol/domain or cross-family audio/firmware compatibility; no live access authorized |
| Legitimate offline license/model inventory | Historical package/authorization surface distinguished from model availability | Read-only storage schema and authenticated product/pack meaning have not been recovered; no user credentials/entitlements read, and no authorization routine executed |

The remaining definitions are not silently deferred to #29 implementation. This
is a bounded report of recovered paths, losses and concrete unknowns, not acceptance
of #40, permission to unlock hardware, audio equivalence or a recovery guarantee.

## Checks and count correction

The preceding handoff's numeric `count=18` measured **private evidence files** in
`sections-evidence-58294b49cebd9bc91d80f546ca8a5896315246fd45f98cb24fca33cc155a7392.json`:
17 static report/raw-range files plus one audit-test log. It did not count tests.
That handoff ran eight strict audit tests and eight retained prefix/status tests,
for **16 test methods**, with truncation cases nested inside methods.

This continuation adds six owned synthetic reconstruction methods for the product/
version class thresholds (including boundary neighbors, unknown product neighbors
and unsigned words). They test `scripts/pod_farm_tone_class.py`, not native code,
a conversion engine or a real preset roundtrip. The eight prefix/status methods
are also reconstructions; the eight section audit methods enforce deliberate
**research restrictions** (version 2 only, full frames, settings header exactly 20)
that are stricter than native behavior. Together the three focused commands run
**22 test methods: 14 reconstruction + 8 strict audit**, all synthetic. Historical
3,318 two-leaf CPU-emulated cases belong to the earlier parameter lookup only;
that emulation is neither rerun nor expanded and is not added to this test total.
Earlier 17 resource/setter methods likewise retain their own scope/count ledger.

```sh
python3 -m unittest discover -s scripts -p test_pod_farm_tone_class.py -v
python3 -m unittest discover -s scripts -p test_pod_farm_state_prefix.py -v
python3 -m unittest discover -s scripts -p test_pod_farm_section_audit.py -v
```

For static reproduction, use the hash-pinned AU project and owned
`PodFarmEdge.java --exact` selector with `00190a1a 0019523c 0019303c 001a0a6c
001a0380 001a04e4 001a063c 0019fe28 001910f6 00190eb2 0018f4a8 0019009a
0018f75a 0019a242 0019eea2 00192262 000c9ae8 00083e10 000851aa`.
Raw source-derived pseudocode/disassembly stays private, never a tracked fixture.
The initial truncated raw classifier range is retained as incomplete evidence;
the full range through the return supplies the accepted cross-check.
No production/default/math/dependency/guard changes or full product CI occurred.
Four exact-head command-v1 checks and independent review remain mandatory after
explicit Gatekeeper publication unblock. No push/PR/access polling/new decision
or no-PR acceptance request was made.

Private manifest: `tones-evidence-36d14e14ecce11479a1c26ef7c3b4b563ac580949d43e50157d67491f429f9f7.json`.
Its **11 evidence files** comprise eight static Ghidra reports, two raw classifier
ranges (one truncated) and one combined focused-test log. This count is separate
from the **22 test methods**. Combined test-log SHA-256:
`0bd70ea0dc7425e5bfed60e6390636a9da302335e76f40b05b2316e764e99661`.
