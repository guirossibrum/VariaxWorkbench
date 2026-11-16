# POD Farm 2.58 native parameter-property edge

This follows the [static inventory](pod-farm-258.md) for
[#40](https://github.com/guirossibrum/VariaxWorkbench/issues/40). It traces an
actual descriptor lookup and its exported projection, rather than inferring knob
domains from names. It does **not** recover applicable PODxt Live control ranges,
units or serialization, and does not unblock #29. The Rust application is unchanged.

## Exact sources and method

The source is `Line_6_TWXY_Framework.pkg`'s
`L6TWXY.framework/Versions/A/L6TWXY`, full-file SHA-256
`1b6acb3aa012d9b884f00daeacc948aa80884c81f513871be740055cd629a924`, from the
previously verified DMG. Two complementary native representations were inspected:

| Slice | Offset in framework file | SHA-256 |
| --- | --- | --- |
| x86_64 | `0x1000` | `dced9e6bee1ac57ba9135c1b8751d26141be82155122f7aa151b2cb457bd55a1` |
| i386 | `0x2a9000` | `33db73ae144b7975de8c1b8bc1cdc22928d521e0c6825ee6c1067023b771914a` |

Addresses below are preferred virtual addresses **within the named slice**.
The cited `__TEXT` functions and `__DATA` pointer entries have matching slice file
offsets; add the slice offset above for framework-file offsets. Symbol names come
from the actual symbol table. Unnamed functions are identified by address only.

Ghidra 12.2/Temurin 25.0.5 provided static cross-references and pseudocode using
the owned [PodFarmEdge.java](../scripts/PodFarmEdge.java) script. GNU objdump 2.47
independently decoded the relevant instruction ranges as data. Nothing was loaded
as a host library. Ghidra loaded zero additional imported libraries.

Some Ghidra function boundaries/decodes were incomplete or erroneous, including
`TWXYCreateInstance` at `0x38860`. Its pseudocode is **not** a contract. The
factory path below uses the raw instructions for that export and its calls. An
initial Ghidra project creation failed because it rejects `.git-worktrees` in the
project path; a separate private `/tmp/issue40-ghidra-*` analysis project resolved
that tool restriction. It is not a Git checkout or another agent's worktree.

## Traced lookup, not a hardware ID table

In the x86_64 slice, exported `TWXYGetParameterProperties` at `0x39060`:

1. Reads the manager pointer at `0x26c060`, rejects a null output/invalid instance,
   and resolves its first argument through manager virtual offset `0x130`.
2. Calls instance virtual offset `0x1e8` with the other two selector/ordinal
   arguments, a descriptor-pointer output, and a final flag of `1`.
3. On a nonnegative internal result and nonnull descriptor, copies the twelve
   fields listed below. It returns through status translation at `0x37a40`.

The constructor at `0x362e0` installs manager vtable `0x234fd0`; the pointer at
`0x235100` (vtable + `0x130`) is `0x36800`. That leaf accepts unsigned instance
indexes below 256 and reads the pointer at manager + `0x9e0 + 8*index`; null or
out-of-range yields null. These are host-instance handles, not preset bank slots.

The raw `TWXYCreateInstance` instructions at `0x388b5` call manager slot `0x118`.
Its vtable entry points to `0x36650`, which allocates an object and calls constructor
thunk `0x43260` → `0x43170`. That constructor stores vtable `0x235190`. On this
constructed-instance path, vtable + `0x1e8` at `0x235378` points to `0x44c60`.
That getter validates the selector through `0x50430` and looks up the descriptor
through `0x50b00` using instance + `0x318` as its table root.

The two leaves at `0x50430` and `0x50b00` establish this bounded contract:

| Selector word | Descriptor group index |
| --- | --- |
| `0` | 0 |
| `1` | 1 |
| `0x1000` | 2 |
| `0x7000` | 3 |
| `0x7001` | 4 |

Other selector values are rejected. A record lookup requires unsigned ordinal
`< 0xd8` (216), and a **nonzero byte** at the record start. The returned address is
`root + 8 + group*0x15858 + 0x18 + ordinal*0x198`; otherwise it returns null.
The ordinal is an array index. Passing a stored tagged PARM word such as
`0x3f000003` as this ordinal fails the bound. This does not establish any mapping
from its low-24-bit id `3` to ordinal `3`. This table does not assign selector labels or
hardware identities, and 216 is storage capacity, not proof of 216 controls
available in every model. Neither groups nor instance handles establish hardware
bank numbering.

## A state-dependent projection with architecture-dependent widths

The getter `0x44c60` does more than return a constant descriptor. With the final
flag set, it calls instance slot `0x208` and dispatches using the low word at
descriptor + `0x108`. Observed comparisons include `0x3f000003` and
`0x7e021001`–`0x7e021003`. It queries other instance/model methods, and writes
descriptor + `0x130` on its successful path. For one flag-class branch it also
writes + `0x148`; for a failed availability flag both fields are cleared. The
export always requests this refresh. Thus its projected + `0x130` value cannot
be treated as an invariant default or static range endpoint. Exact numeric
interpretation and complete branch semantics remain unrecovered.

The i386 export at `0x3bc30` provides a complementary twelve-field projection.
The following are **byte-copy facts**, not declarations of C/Rust scalar types,
units, parameter IDs or valid ranges. Field order pairs the two implementations;
two x86_64 fields load eight bytes and store only the low four.

| i386 descriptor offset | Output offset / bytes | x86_64 descriptor offset | Output offset / bytes |
| --- | --- | --- | --- |
| `0x104` | `0x00` / 4 | `0x108` | `0x00` / 8 |
| `0x108` | `0x04` / 4 | `0x110` | `0x08` / 4 |
| `0x10c` | `0x08` / 4 | `0x118` | `0x0c` / 8 |
| `0x140` | `0x0c` / 4 | `0x180` | `0x14` / 8 |
| `0x110` | `0x10` / 4 | `0x120` | `0x1c` / 8 |
| `0x114` | `0x14` / 4 | `0x128` | `0x24` / 8 |
| `0x118` | `0x18` / 4 | `0x130` | `0x2c` / 8 |
| `0x11c` | `0x1c` / 4 | `0x138` | `0x34` / 8 |
| `0x128` | `0x20` / 4 | `0x150` | `0x3c` / 4 |
| `0x13c` | `0x24` / 4 | `0x178` | `0x40` / 8 |
| `0x144` | `0x28` / 4 | `0x188` | `0x48` / 8 |
| `0x148` | `0x2c` / 4 | `0x190` | `0x50` / 8 |

The observed write spans are 48 and 88 bytes. There is no output-size argument
checked by either wrapper. This is not a portable persisted property blob, and
none of these offsets are a recovered `PARM` chunk serialization layout. This
milestone adds no FFI call or live API to expose it.

## Patch serialization edge located, not decoded

The constructed-instance vtable also resolves `TWXYGetPatchSize`/`GetPatchData`/
`SetPatchData` to `0x3ed20`, `0x3ee20`, `0x3ef50` via slots `0x1a0`, `0x1a8`,
`0x1b0`. Size/get obtains a patch object through `0x2b9c0`, prepares a memory stream
and calls that object's slot `0x40`. Get checks caller capacity before copying
the serialized bytes. Set constructs a read stream and calls `0x17ac94`, then
instance slot `0x1b8`. Those are concrete next handlers for tracing FORM/PINF and
unknown-field behavior; the calls do not yet establish accepted targets, L6P2
versions, preservation rules or plugin-state equivalence. No roundtrip serializer
or Rust format acceptance is added from this partial trace.

## Focused validation and preserved evidence

The owned [lookup oracle](../scripts/verify_pod_farm_lookup.py) pins the complete
x86_64 slice SHA-256 before creating an emulator. Only two independently inspected
leaf ranges are mapped into emulated RX memory: `[0x50430,0x50487)` and
`[0x50b00,0x50bd1)`. They have no calls, imports or system instructions. An
instruction hook rejects execution outside the active leaf and each call has a
512-instruction budget. The only other mapped memory is a synthetic writable table
and stack; interrupts fail closed. There is no host-service bridge, initialization,
authorization path, DSP or device access. The vendor library is never executed
natively. This is bounded CPU emulation, not a complete library oracle.

With pinned Unicorn 2.1.4, **3,318 cases** passed with **0 mismatches**:
13 selector cases; 3,240 lookups covering all 216 ordinals across all five groups
and presence bytes `0`, `1`, `0x80`; 40 invalid-selector lookups; and 25 rejected
ordinal-boundary/tagged-word lookups. The trace executed 75,337 emulated instructions.
It proves these leaves' behavior on synthetic memory only, not descriptor
population, actual control domains, processing, hardware safety or audio parity.
A separate guard rejected a synthetic incorrect source hash before emulator
creation; it is not included in the 3,318 leaf cases.

After the existing offline Python setup, reproduce with:

```sh
.tools/python/bin/python scripts/verify_pod_farm_lookup.py
```

This expects the private hash-verified slice at
`analysis/pod-farm-258/native-edge/L6TWXY.x86_64.macho`. Recreate it from the
framework's fat-table entry at offset `0x1000`, size 2,782,904, and verify the
slice hash above. Ghidra analysis can use the same copied slice and
`-scriptPath scripts -postScript PodFarmEdge.java`; `--exact <VA>...` limits
the script's selected functions. Ghidra projects must use a path without leading
dot elements. No application/installer extraction switch is involved.

The content-addressed private oracle report is
`lookup-oracle-6d3d12d00e05457ca6b5b505e457705a14ff84dd3c66f35aa5045147e86ae0fb.json`.
Evidence manifest
`edge-evidence-03745c06d06582eba797d85f73f65067323498f7a8551c844a867a5be89c6b13.json`
preserves 24 logs/reports with content-addressed copies, including the initial
tool failure and inaccurate decompilation. Both remain under the ignored
`analysis/pod-farm-258/native-edge/`; no proprietary instructions, pseudocode,
examples or symbol dumps are committed. The framework hash remained unchanged.

The subsequent [descriptor population trace](pod-farm-descriptor-population.md)
maps one named system-control seed and the generic scalar metadata producer.
Complete domains, defaults, model applicability and PODxt target support remain
unknown. The located patch-object reader remains another source edge.
Publication is still separately held;
required command-v1 checks and independent review remain pending publication
authority. #40 remains open and #29 remains held.
