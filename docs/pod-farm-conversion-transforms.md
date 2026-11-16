# POD Farm 2.58 direct conversion transforms

This report records an earlier research pass. Its publication/check hold statements
are historical and superseded by the [current publication and check status](pod-farm-acceptance-coverage.md#current-publication-and-check-status).
Scientific limits, incomplete acceptance and the #29/hardware boundaries remain.

This is one bounded static pass over the direct transformations left by the
[tone coverage report](pod-farm-tone-coverage.md). Source: full AU x86-64 slice
SHA-256 `715d30b8fed77d4ea18150050131db759b8e8a27eeaf4e3d9b3872b6bca6fbe0`.
Addresses are AU virtual addresses/slice offsets. Ghidra 12.2 exact selections,
raw x86-64 decoding and static table-range hashes provide **source evidence**.
Owned synthetic reconstructions are separately identified below. No native vendor
code, imports, initializer, plugin, authorization or new CPU emulation ran.

**Conversions rebuild selected fields, can drop others, and mutate before errors.
Malformed-input permissiveness and unenforced mapping/settings span lengths from
the preceding reports remain material. Nothing here establishes a full-state
roundtrip, hardware parameter domain, units/defaults/steps or PODxt applicability.**
#40 stays partial/open and #29 held; production Rust, math, dependencies and guards
are unchanged. Publication/full command-v1 checks/independent review remain held
and required under the current coordinator instructions.

## Legacy projections are data-dependent, not entirely unavailable

`0x193fbc` clears the destination, replaces product/version with `0x80000`/
`0x1000000`, copies selected metadata, and maps fixed source document fields into
model slots and tagged parameters. For example it writes six selected float words
from source +`0xd0`..`0xe8` into slot `0x30000`, and uses source enable flags.
Several later parameter subsets depend on source model IDs. Unknown source fields
have no opaque retention route. This is source-defined projection from a supplied
legacy document, not evidence that those values are actual defaults or units.

`0x1947fc` clears/rebuilds and sets product/version `0x300`/`0x2000000`. It trims
a 16-byte name, caps two model selectors at 31 and 15, doubles several input bytes,
left-shifts selected bytes by six, divides a signed source integer by six, and
selects model/parameter subsets from a source mode byte. An unhandled mode simply
omits that subset; the routine still returns zero. Several scale/truncate steps
are therefore intrinsically lossy. Neither projection requires contacting a
current target patch to define these direct arithmetic steps. The initial
legacy document/MIDI parser and original-state equivalence remain unvalidated;
those are validation boundaries, not missing files or proof of unavailable code.

`0x1915fc` clears and selects the legacy record layout. Its signature/length caller
`0x191e8a` admits particular 168/170-byte records; this routine itself takes the
168 layout or its else layout. For the 170 layout, bank = byte[7] × 128 + byte[8]:
64..127 or 256..319 selects category `0x400003`, 128..191 or 320..383 selects
`0x400002`, and other values retain `0x400005`. Bytes are read as full unsigned
bytes; this path does not first enforce seven-bit MIDI values. The 168 layout
keeps `0x400005`. Names move by two bytes between layouts and are trimmed.

It creates a system model at slot `0x10000`. The `0x400003` branch keeps only
parameters `0x3f200017`/`0x3f200018` there; `0x400002` keeps only `0x3f200004`.
Other category-specific models and enable flags depend on input bytes and on
patch product/version words. For example an additional slot is gated by product
`0x30002`/`0x30005`/`0x3000a` with version >=`0x1060000`, or the masked product
family 8 with version >=`0x2490000`. These words are not inferred live identities.

## Table dispatch and scalar arithmetic

`0x18fbd6` uses 24-byte descriptors: two source byte offsets, parameter ID, a
store flag, and a 64-bit conversion callback. It selects record payload +39 or
+41 by layout, invokes the callback in decode direction and stores when the flag
is nonzero. Callback success is not checked. Thus even a model lookup failure's
fallback value can be used. A null callback supplies integer-tagged zero.

The examined 21 descriptor ranges start at `0x257a40`..`0x2582e0`; their explicit
counts/strides/hashes and proprietary rows remain in the private table report.
They resolve 13 distinct scalar callbacks, not 13 native test executions.
The following are derived contracts; the complete tables are not reproduced:

| Callback | Decode branch (byte input → tagged scalar) | Encode branch/limit |
| --- | --- | --- |
| `0x1a5722` | float tag 1, byte / 127 | Multiply by 127, add 0.5, truncate, keep low byte |
| `0x1a5764` | As above except byte 40 maps to raw float word `0x3ea3d70a` | Additional hundredth-step quantization before byte conversion |
| `0x1a57da` | byte 127 → 1; otherwise byte / 128 | Scale by 128, round/truncate; low-byte 128 is replaced by 127 |
| `0x1a5838` / `0x1a5892` | (first byte ×128 + second byte) / 16383 or /10 | Round/truncate then split into two masked seven-bit bytes |
| `0x1a58ec` | integer tag 0, full unsigned byte | Signed 32-bit word clamped to 0..127 |
| `0x1a591e` | bytes 0/1/2/other → integers 2/3/1/0 | words 0/1/2/3 → bytes 3/2/0/1; negative →3; signed >3 →1 |
| `0x1a5988` | byte <42 →0; <86 →1; otherwise 2 | 32-bit shift left six, then signed clamp to 0..127 |
| `0x1a59d2` | byte >63 → integer 1, otherwise 0 | Zero word →0, every nonzero word →127 |
| `0x1a59fc` | negative byte as float, capped at -96 | Sign flip/round/truncate to low byte then cap at 96 |
| `0x1a5a5a` | byte/127 + stored float near 0.01 | Subtract that stored constant before byte quantization |
| `0x1a5aee` | (byte/127) ×787.5 +6.25 | Inverse affine byte quantization |
| `0x1a5cc2` | Clamp byte into 16..112, divide by127 | Convert to low byte, then clamp into16..112 |

These are encoding arithmetic, not device range definitions. Float operations
round at their original instruction widths; they do not supply a portable host
math contract. Some encoders narrow **before** capping, so arbitrary input words
cannot be treated as a saturating real-number API. Exceptional floating-to-integer
inputs are not modeled by the owned helper. The four integer callback ranges
`0x1a58ec`..`0x1a59fb` were independently checked in raw instructions, including
the wrapped 32-bit shift before signed comparison.

Model-ID callbacks `0x1a615a`, `0x1a6286`, `0x1a634c`, `0x1a63ae`, `0x1a5d30`,
`0x1a5de0`, `0x1a5e48` index bounded tables and return a fallback plus false when
out of range. Reverse searches select the first matching table entry; no generic
bijectivity follows. Provenance includes these ranges (values stay private):

| Table address | Entries × stride | Selected by |
| --- | --- | --- |
| `0x1c57c0`, `0x1c57a0`, `0x1c5840` | 29×4, 7×4, 107×4 | `0x1a615a` family selector |
| `0x1c5680`, `0x1c56e0` | 23×4, 47×4 | `0x1a6286` family selector |
| `0x1c5640`, `0x1c5620` | 15×4, 8×4 | `0x1a634c`, `0x1a63ae` |
| `0x1c5320`, `0x1c50c0`, `0x1c4f60` | 31×24, 25×24, 14×24 | Final three callbacks, first word of each entry |

One special branch maps bytes126/127 directly to IDs `0x2000009`/`0x200000a` in
`0x1a5d30`. The bounded lookup/fallback structures are recovered; no proprietary
table is committed as an owned replacement or promoted to an active model list.

## Direct helpers selected by the conversion matrix

`0x191252` replaces a model ID and marks every existing parameter entry absent.
`0x19b3b6` first copies old parameters, preserves the enable flag, replaces and
clears the target parameter array, then seeds selected entries via `0x19b2d6`.
Only explicitly transformed/reinserted old values survive. The setter `0x18fb00`
updates the first matching parameter, appends a missing one, and can preserve an
absent entry's stored value when the final flag is set. It returns zero for a
missing model; many callers ignore that result. This is not transactional copying.

`0x19b2d6` scans an on-disk data region at `0x288180`: 544 rows of 20 bytes,
partitioned into 83 model-header/entry groups. It filters entries for the requested
slot or wildcard -1, choosing a float field for tag1 and the integer field for
other tags. That region is available static evidence, not an entirely missing
runtime source. It does not establish actual registration/pack availability.
The full group headers/values and per-range hashes remain private.

| Matrix helper | Established direct effect / branch condition |
| --- | --- |
| `0x19db9e` | One direction removes slot `0x30004`, rescales a selected float by16/31 then clamps0..1; the other may seed that model, insert selected zero/half words and rescale by31/16 conditional on another float > stored ~0.999 threshold. Positive integer mode collapses to1 or2 by direction. Model fallback dispatch uses ten24-byte entries at`0x258cc0`. |
| `0x199794` | Nonzero direction does nothing; zero direction uses eleven24-byte fallback descriptors at`0x258ba0`, replacing models beyond callback-reported limits. |
| `0x19e364` / `0x19987a` | Direction-gated insertion of specific tagged seed words if absent, plus selected enable/reset behavior; conversion-local seeds, not product defaults. |
| `0x19e406` | Moves selected system parameters into slots`0x2000d`..`0x2000f` or back, changes system model`0x3e700000`↔`0x3e700002`, replaces other slots, collapses/reinterprets flags and drops unselected parameters. Product`0xc0001`/`0xc0002` selects one inserted integer differently. |
| `0x19edde` | Saves one selected parameter, calls`0x19e406`, then writes it under a different ID (`0x3f200029`↔`0x3f20001f`); an ignored missing lookup leaves its initialized seed. |
| `0x199a9c` | Removes destination slots and remaps source slots into`0x6010000`/`0x6020000`..3/`0x6030000`..1/`0x6050000`..3. Slot allocation depends on which old models are present/enabled. Product`0x600` selects distinct arithmetic, including a (-96 - selected scalar) and another /1000 rather than ×stored~3.15. Writes product/version`0x130011`/`0x1000000`. |
| `0x19b2bc` | Direction dispatch to`0x19a3aa` or`0x19ab90`; forward has <=10 grouped-model capacity conditions, reverse can remove/reassign selected slots before a later false result. No rollback. |

The legacy upgrader `0x19d914` calls direct per-model transformations and fills
missing slot pairs from thirteen16-byte fallback descriptors at`0x1c4d40`.
It resets/updates scratch global`0x288160`; that is conversion state, not an
observed device setting. Selected concrete remaps include:

- `0x19d608`: slot`0x20003`, model`0x2000008`→`0x2000007`, parameter`0x3f300031`→`0x3f100001`, signed input converted to float /127.
- `0x19d70c`: selected slot`0x20001` or`0x50002`, model`0x2070001`→`0x2070000`, parameter`0x3f30002e`→`0x3f000004`, signed input /127; if neither source model matches, both slots are removed.
- `0x19d850`: only model`0x2060001` at slot`0x20002` is rebuilt as`0x2060000`, disabled with a selected half-word seed; another model there is removed.
- `0x19c6a4`: selected integer parameters become float tags with threshold or reciprocal affine arithmetic; unhandled fields are not reinserted after replacement.
- `0x19cca6`: selects among`0x2040001`/`0x2040000` successors, changes parameter IDs and tags, and applies square-root arithmetic in selected branches. Availability can alter the chosen successor.

The other direct branches `0x19b538`/`0x19cfe2`/`0x19d374`/`0x19ba22`/
`0x19bc48`/`0x19c294`/`0x19c8d0` contain source-defined ID switches, normalization,
selected fallbacks and dropped unmatched fields. Several choose a replacement
using availability predicate `0x19a204`. That predicate walks the **runtime list
at `0x2ab020`**, comparing model IDs; it does not use names as domain proof.
No active contents of that list were read or fabricated. These selected outcomes
therefore remain conditional, even though the branch code is statically present.

Nonlinear subbranches call imported historical `_log`, `_exp` or `_log10`, use
private interpolation curves and sometimes clear exponent-all-ones/subnormal
results. Their instruction/runtime math agreement is an explicit unvalidated
boundary, not a reason to adopt current host transcendental functions. No curve
or proprietary lookup table is reproduced here. The shared `0x19eea2` common exit
can still overwrite target metadata on error; direct helper mutations are not
rolled back. Wrapper status projection remains weaker than whole-state success.

## Owned checks and final boundaries

Eight new test methods in `test_pod_farm_legacy_scalar.py` check table-free integer
scalar rules and legacy bank category selection, including signed word limits,
32-bit shift overflow, all 256 byte values for three decoders, canonical roundtrips,
loss outside the canonical set and category boundary neighbors. These are owned
reconstructions from static facts, **not original-state or native oracle results**.
The helper restricts legacy record length to 168/170 as a research precondition;
it does not claim the native callee rejects all other lengths. It includes no
vendor table or production codec. Existing 22 focused methods are retained:
now **30 methods = 22 reconstruction + 8 stricter audit**, with nested byte/edge
cases not counted again as methods. Evidence-file counts are separate.

```sh
python3 -m unittest discover -s scripts -p test_pod_farm_legacy_scalar.py -v
python3 -m unittest discover -s scripts -p test_pod_farm_tone_class.py -v
python3 -m unittest discover -s scripts -p test_pod_farm_state_prefix.py -v
python3 -m unittest discover -s scripts -p test_pod_farm_section_audit.py -v
```

The pass stops at these precise input/validation boundaries:

1. Runtime model-list`0x2ab020` contents and initialized target product/version/current patch remain unobserved; TWXY imports are not called.
2. Historical nonlinear runtime math, interpolation curves, full legacy decoding and original-state roundtrips lack oracle validation. Static arithmetic/branches do not fill that gap.
3. All parameter definitions, third scalar component/default/step/unit semantics and PODxt-specific serialization/applicability remain missing; #29 is not implemented.
4. Earlier AU deferred-cache consumer and RTAS outer-ID producer remain unresolved; no new reachable xref was found or wrapper search repeated.
5. License storage/schema and active pack state remain unverified; no credential/authorization inspection was performed.
6. Four required exact-head command-v1 worker checks and independent review remain mandatory after explicit publication unblock. No push/PR/polling/new decision or unpublished full CI occurred.

Static reproduction uses the existing pinned AU project and `PodFarmEdge.java
--exact` with`001915fc 0018fbd6 001a5722 001a5764 001a58ec 001a591e 001a5988
001a59d2 001a615a 001a6286 001a634c 001a63ae 001a5d30 001a5de0 001a5e48
0019db9e 00199794 0019e364 0019e406 0019987a 0019edde 00199a9c 0019d914
0019a3aa 0019ab90 0019a204 0019b3b6 0019b2d6`. Direct child selections and
provenance ranges are recorded privately; no broad inventory/exhaustion claim is
made. Original bytes/code/disassembly/resources/captures remain ignored/local.

Private manifest: `transforms-evidence-9dad7f6924a7ee9385b5594c429302fcac6a23433630d109683546eb84b2505f.json`.
Its **nine evidence files** comprise five Ghidra reports, two private table/
provenance JSON reports, one raw integer-instruction range and one combined
focused-test log. The **30 test methods** are a separate count. Combined test-log
SHA-256: `7698597b0a0290a59902f6259d9b532ac2233ef8c944aff7e239f1fe7d557ef0`.
