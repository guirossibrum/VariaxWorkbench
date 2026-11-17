# POD Farm 1.12: bounded PODxt source recovery (#29)

This partial research milestone follows the newly provisioned original through
one PODxt Live packed-tone consumer and one scalar conversion edge. It does not
recover an admissible editor domain or implement value editing. [#29](https://github.com/guirossibrum/VariaxWorkbench/issues/29)
remains open with incomplete acceptance. The [offline librarian](pod-presets.md),
hardware locks, default math and application dependencies are unchanged.

## Source identity and extraction

The owner-supplied file named `POD Farm1.12.dmg` is **137,347,009 bytes**, SHA-256
`086416c6c6b7e4471871bee4d44f4d8949362e57155677fd06d8288a344e399a`.
The supplementary task manifest digest is
`8b2891f34882ec02c88a0d558c29c0cf8cd7e9ecd5f398dfeaf31a673e3ae866`.
The original is read-only and its bytes match before and after extraction.
These pins establish identity to the supplied copy, not publisher authenticity,
signature trust, licensing, installed state or any device identity.

The separate [1.12 reader](../scripts/pod_farm_112_inventory.py) retains the
[2.58 reader's](../scripts/pod_farm_inventory.py) original pins unchanged. It
uses trusted 7-Zip 26.06 `l`/`x -so`, bounded gzip and the existing old-ASCII CPIO
reader. It requires a fresh destination below ignored `analysis/`, writes files
atomically without replacement, records links without materializing them and
does not restore executable permissions. It never mounts, installs, executes or
imports a vendor component. Installer scripts, drivers and updaters are not run.

The seven selected component payloads contain **832 regular files**, **three
unmaterialized links**, and **seven Mach-O files with fourteen slices**. This is
a selected-component inventory, not a survey of all DMG payloads. The selection
is the standalone app, common resources, TWXY/TWX frameworks and AU/VST/RTAS
plugins. Relevant `Info.plist` findings:

| Component | Package version | Embedded bundle version / architecture |
| --- | --- | --- |
| POD Farm standalone | 1.12 | `com.line6.podfarm`: 1.12; i386 + PowerPC |
| POD Farm Common | 1.12 | Shared resources; no separate native executable surveyed |
| POD Farm AU/VST/RTAS | 1.12 | Short version 1.12, build 1.1.2; i386 + PowerPC |
| Line 6 TWXY Framework | 5.02.0 | `com.line6.twxy`: 5.02; i386 + PowerPC |
| Line 6 TWX Framework | 5.02.0 | Separate dylib: i386 + PowerPC |

The TWXY payload also embeds a distinct `data/twx/L6TWX_u.dylib`; do not conflate
it with the separate TWX package. All surveyed slices are 32-bit. No surveyed
slice has an `LC_CODE_SIGNATURE` command; this is a header observation, not a
signature verification result. Only static Mach-O headers/imports/symbols were
read. Framework imports include macOS Carbon/CoreMIDI/CoreAudio/IOKit; those
libraries were neither loaded nor called.

| Selected component / private evidence | SHA-256 |
| --- | --- |
| Full `L6TWXY.framework/Versions/A/L6TWXY` | `81fdb247a010a23f5a0139abf490f93bf6c15d5a306eca9171110111e12ae703` |
| TWXY i386 slice, fat offset `0x1000`, size 4,013,896 | `db9253497fb715ae71a3c3c31adeabdb81878594fab486d8e621bbdf04e6fd30` |
| Full standalone `Contents/MacOS/POD Farm` | `0339f08a95e8a295e8746326b0ac7a73693c374a979edc7082107b414f3c9e63` |
| Separate full `L6TWX_u.dylib` | `c608ed3f52f02bf8e7de0338d7b0d57395998d52ff035f3019ef1b0ea86fed82` |
| Nested full `data/twx/L6TWX_u.dylib` | `cdf3dace8637c22e762f747f94ff6050baf774a24c035594ab575180cf6af103` |
| Private `inventory.json`, identical on fresh replay | `0cd608a2c9e13a9ced4d4a7c366c8c93be2cf8d452b84288e44579cece3e4e6d` |

## Target-specific consumer, separated from control definitions

All following addresses refer to the **1.12 TWXY i386 slice** above. Its `__TEXT`
and relevant `__DATA` file offsets equal their virtual addresses; this does not
apply to `__IMPORT` or `__LINKEDIT`. Ghidra 12.2/Temurin 25 supplied static
xrefs/pseudocode; GNU objdump supplied independent raw i386 instruction decoding.
Ghidra imported zero additional libraries. No native or emulated vendor execution
occurred. Some inferred function signatures are incomplete; argument widths and
the scalar branch below were checked against instructions.

The product word `0x0003000A` is not USB product `0x4650` or MIDI hardware
identity decimal 18000. In this framework `0x21bca6`, called at `0x21c05f`, maps
the latter 16-bit identity to `0x3000a`. That identity translation is not a control
table. Product checks in `0x46330` classify conversion compatibility, also without
supplying parameter domains.

The concrete packed-tone consumer is `0xf6508`:

1. It checks particular six-byte signatures and record lengths 168 or 170.
2. On its POD-family branch it forms the product word as `0x30000 | byte[6]`;
   the accepted checks include `0x30002`, `0x30005`, `0x3000a` and `0x30003`.
3. It stores that product word in patch object +`0x78`, then calls `0xf593a`
   at `0xf669e`. This is a target-connected **file conversion** consumer.
   No hardware communication is needed to inspect these instructions.

`0xf593a` clears/rebuilds patch state, selects the layout at payload +`0x27` or
+`0x29`, trims a legacy name and inserts selected models/parameters. The amp
selection calls `0x10e938` to translate packed model selectors. Its non-`0x30000`/
non-`0x40000` model-family branch selects six conversion records beginning at
`0x366a20`, then calls `0xf3c9e` at `0xf5d1a` for slot `0x30000`.
This is conditional on the decoded model and patch category; it is not a table
applicable to every amp/model or every file target.

`0xf3c9e` walks **20-byte conversion records**: two packed-source offsets, a
tagged parameter identifier, a store flag and a 32-bit callback pointer. It calls
the callback in decode direction, then `0xf3ba6` when the store flag is set.
The latter updates/appends the typed raw parameter keyed by slot/identifier and
marks the patch changed. It does not validate editor min/max/units here. A null
callback supplies integer-tagged zero. Callback results are not checked here.
These are conversion descriptors, not editor parameter definitions.

One derived edge is concrete: the first selected amp record yields parameter
`0x3f100003` through callback `0x10dda4`, reading the selected layout's parameter
base +`0x0d`.
The callback's decode direction sets type tag 1 and computes float32 byte/127;
its encode direction multiplies by 127, adds float32 0.5, truncates to signed
integer and keeps the low byte. The constants at `0x330a40`/`0x330a54` are 127
and 0.5. It reads a full unsigned byte and has no seven-bit validity check on
this path. Consequently the arithmetic itself cannot justify a `0..1` editor
range, a percent unit, a knob name, a default or a step. Exceptional conversion
inputs and the calling runtime's floating-point environment were not validated.
No proprietary conversion table is reproduced or installed into the Rust app.

The private six-record span `0x366a20..0x366a98` hashes to
`27cd7683181509530df401ea4d8c4e494525508a1eed422ba37530d18aa787ea`;
callback bytes `0x10dda4..0x10ddf4` hash to
`01b20d80c3660229bdf429b8525e2e42e0197511084210d7c0748fbfd903016b`.
These are end-exclusive coordinates, not published instruction bodies or tables.

## Separate property-provider boundary

Export `TWXYGetParameterProperties` is at `0x3e6d0`. It resolves the manager
pointer through `0x3b9068` to global `0x3318e0`, obtains an instance through
manager virtual slot +`0xa4`, then invokes instance virtual slot +`0xec` with
selector/ordinal and refresh flag 1. It copies selected descriptor words to the
caller only after successful lookup. `TWXYCreateInstance` at `0x3de90` delegates
construction to manager slot +`0x98`. None of these exported arguments alone is
a verified PODxt PARM identifier or a physical bank/slot.

The static constructor chain resolves that virtual dispatch further:

- Manager constructor thunk `0x3b420` reaches `0x3b350`, installing vtable
  `0x361968`. Its slot +`0x98` (`0x361a00`) points to `0x3b830`; slot +`0xa4`
  (`0x361a0c`) points to `0x3b9e0`. The latter bounds the unsigned handle below
  256 and reads manager +`0x558 + handle*4`, returning null for an empty slot.
- `0x3b830` allocates an instance and calls `0x4a9b0` → `0x4a8d0`; this installs
  vtable `0x361a48`. Its +`0xec` entry at `0x361b34` is `0x4b780`.
- `0x4b780` validates the selector through `0x61a60`, then calls `0x62550` with
  instance +`0x28c` as the descriptor-table root. The five accepted selectors
  are 0, 1, `0x1000`, `0x7000`, `0x7001`. Lookup requires unsigned ordinal <216
  and a present byte, with record stride `0x14c` and group stride `0x11834`.
  These are host lookup coordinates, not parameter IDs or hardware ranges.
- The export requests refresh. `0x4b780` then uses instance virtual +`0xfc` and
  typed patch accessors to refresh current words. A record's mutable current
  value is not evidence of its allowed range or default.

At this source-recovery milestone, population of the constructed instance's table
at +`0x28c` was unresolved. The subsequent
[target-applicability investigation](pod-farm-112-target-applicability.md) locates
its descriptor/cache producers and distinguishes the reference product key from
the processing destination. The examined concrete receivers select a different
product or encounter rejection; a generic product-forwarding leaf does not
establish an applicable PODxt branch through its preceding guards. No control
domain is admitted from host resources, reset seeds or the generic property API.
The remaining requirement is an independently applicable target-preserving
contract, not a claim that another artifact or live input is necessary, that this
package lacks every PODxt definition or that all entry points are exhausted.

For comparison, the already reviewed [2.58 conversion trace](pod-farm-conversion-transforms.md)
places the corresponding layout consumer at AU x86-64 `0x191e8a` → `0x1915fc`,
24-byte callback dispatch at `0x18fbd6` and byte/127 scalar at `0x1a5722`.
This is a concrete structural/arithmetic correspondence across different
components/architectures, not byte identity, complete semantic equivalence or
an independent editor-domain oracle. The [2.58 property trace](pod-farm-parameter-edge.md)
likewise distinguishes host instance handles and selector/ordinal lookup from
PODxt product applicability. No broad 2.58 inventory or missing-Sharc search
was repeated.

## Reproduction, validation and disposition

After supported task preparation, supply the exact immutable input path:

```sh
python3 scripts/research_setup.py attach-tools .tools/coordinator-inputs/tools
python3 scripts/pod_farm_112_inventory.py --source /PRIVATE/TASK_INPUT/data --output analysis/issue29-podfarm112-fresh
python3 -m unittest discover -s scripts -p test_pod_farm_112_inventory.py -v
```

Seven owned synthetic reader tests pass: wrong source size and same-size wrong
digest, refusal before archive invocation, existing-output preservation, root/
path/symlink containment, bounded/truncated gzip, no restored executable bits or
materialized CPIO links, and traversal/device-node rejection. They validate the
research extraction guards, not native control behavior or a Rust codec. A
separate rerun of the ten existing shared Mach-O/IFF reader tests also passes;
those ten are previously published tests, not new #29 cases. A
second fresh extraction produced the identical inventory hash above. All 832
regular payload files had no execute bits. The pinned original remained intact.

Private evidence under `analysis/issue29-podfarm112/` includes archive listings,
component metadata/payloads, the inventory, copied slices, Ghidra logs, raw
instruction spans and `source-consistency.json`. The latter records the source
pin, selected conversion-record edge, two scalar constants and replay identity;
it is static consistency evidence, not a native oracle. The initial broad
target-immediate lead selection and unrelated containing-function selections
remain private and are not evidence of complete function/source coverage.
Use `PodFarmEdge.java --exact` on the documented concrete entries for focused
inspection; extract the i386 slice from fat offset `0x1000` and verify its hash
before importing. Ghidra projects must use a private path without a leading-dot
component. Original binaries, instructions, resource tables, artwork and example
tones remain ignored and are not test fixtures.

No applicable tuple of **model/product + parameter ID + representation + accepted
domain + units/scaling/default/step** was recovered. The continuation states the
[remaining target-preserving contract](pod-farm-112-target-applicability.md#validation-scope-and-remaining-contract)
before value editing. A future recovered definition needs target/model mismatch,
endpoint and adjacent-invalid-value cases, exact serialization, unknown-byte
retention and atomic new-copy tests, backed by an independently applicable source
contract.
No placeholder API, fabricated units or disabled editing UI is added here.
The four configured Rust checks are submitted through the coordinator at the
actual committed head; reader tests do not replace them. Review and merge-result
CI remain required. Passing any offline check grants no hardware access.
