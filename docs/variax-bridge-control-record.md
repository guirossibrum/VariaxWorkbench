# Standalone bridge: bounded Windows x86 control-record evidence

Partial next-step contribution to guirossibrum/VariaxWorkbench issue #61.
This establishes a selected driver dispatch/buffer boundary and an independent
Rust offline snapshot inspector. It is not a driver, connector session, USB
frame builder, instrument codec or physical compatibility result. Issue #61
and its wider acceptance criteria remain incomplete.

## Current assignment, originals and sources

The assigned clean `b/issue-61` was checkpointed locally at
`refs/checkpoints/issue-61-f96e703d-7d42487`, preserving original head
`7d42487a4a23332a4f5c33824a1f7ddf70c47008`. Normal fetch and ancestor checks
proved it and predecessor PR #87 merge `2d1a084fe1ce4ca2094869940caf836981fd3ddb`
are ancestors of fetched master `699b4b0b28c52a05efffb921ffed0935a1f55a2c`.
The owned branch advanced with `merge --ff-only`; no rebase/reset/history rewrite.
The coordinator prepared the current assignment's base input inventory
`ae6f747b2dcf4d52244e6106f11465eadfdb4654983467310695bd9e31c59184` with
policy `e34ecaafc47dc2385eb9fa281d6bd431151c671b03fbb7e1704ce0715eb7d34c`.
No preceding assignment's supplemental authority was reused.

The inventoried standalone installer remains 3,336,616 bytes, SHA-256
`fb376b29ed264aaf65c4c71aa4dfe27fd29ec0fef4048d95ee55df634fc733ae`.
Workbench HD 2.15 Windows original SHA-256 is
`7d197848079a73fda2c8160ae065dd42af9159125d2f9c9b63aa4f386f4dccf3`.
Both originals were verified before static member extraction through trusted
7-Zip 26.06. Initial exclusive copies were verified and atomically published using temporary
files, fsync and non-overwriting hard links under `verified-inputs/` in ignored
`analysis/issue-61-next/`; no installer, CPL, SYS, DLL or service was launched.
All native bytes, disassembly/imports and derived source reports stay private.

| Extracted member | Bytes | SHA-256 |
| --- | ---: | --- |
| Standalone x86 L6SM.sys | 39,296 | `b109c86093a54a33b8eff7f677053f3ca1960c3c304a137ed6ea776f4f17c5d7` |
| Standalone x64 L6SM64.sys | 66,176 | `9310af69e22afd33a3e044bafcdd0a0fd142b1631f7dc679d234deeca2a0c774` |
| WBHD-bundled x86 L6SM.sys | 39,296 | `a6bc1c679e2cc5f551b6f4d6a2eff0c2f96aafc4cb0bf9832cf4265a26919dc7` |
| WBHD-bundled x64 L6SM64.sys | 66,176 | `46fa39b889ed159aa47a346c76b01e712537feeb0c796de831e5683821b7dc8a` |

The sizes match across packages but hashes differ. This pass does not promote
matching size/INF/version labels into behavioral equivalence. The extracted
standalone INF (2,578 bytes) hashes to
`a7109a7b404adde549fd0cfd347c34a924202e522751c27f8d939a3525d9d0f8`.
All following x86 addresses are PE VAs at image base `0x10000`.

## Connected static dispatch and selected buffer contract

The earlier common callback `0x105c4` conditionally indexes a major-function table
at `0x17a30`; entry 14 is wrapper `0x12d3d`, which dispatches vtable slot `+0x48`.
The previously identified concrete vtable at `0x17400` places `0x11c78` in that
slot. The separate zero-global alternative at `0x105d2` still uses a different
vtable path: this is not proof every runtime request reaches this handler.

Owned `research/bridge/inspect_static_tables.py` hash-pins and independently reads
the two table words as data. GNU objdump static disassembly connects wrapper and
handler; the tool's ignored nonpaged-section-flag warnings were retained. Neither
check runs a driver or establishes live dispatch.

At `0x11c78`, the selected handler reads the control word through IRP-like object
`+0x60` then stack-like object `+0x0c`. The comparison/subtraction tree selects:

| Control | Leaf VA |
| --- | --- |
| `0x222000` | `0x10cb8` |
| `0x222004` | `0x10d4e` |
| `0x222008` | `0x10cfc` |
| `0x22200c` | `0x11970` |
| `0x222010` | `0x119f4` |
| `0x222014` | `0x11356` |
| `0x222018` | `0x1144e` |
| `0x22201c` | `0x10dee` |
| `0x222020` | `0x10dba` |

Other values enter `0x11cf4` with status word `0xc000000d`. At `0x11d30`,
return word `0x103` follows a separate branch; other results call completion
helper `0x1081a`. This is a static status/dispatch distinction, not an implemented
pending/cancellation contract.

For **only** `0x22200c` and `0x222010`, leaves `0x11970`/`0x119f4`:

- Inspect context byte `+0x286` and the buffer pointer at request `+0x0c`.
- Compare a length from the stack-like object `+4` against 16.
- Read a little-endian 16-bit word at buffer offset 0 and a one-byte count at 2.
- Compare count against 8, then pass the word, count and pointer `buffer+3`
  to separate helpers `0x11708` and `0x1181e` respectively.

The request/stack offset names are structural interpretation of these Windows
x86 paths; the buffer shape and integer comparisons are directly disassembled.
The counted bytes' protocol meaning and the two operations' directions remain
unresolved. Neither word nor bridge PID `0x534d` is an attached-guitar model ID.
No instrument response or on-wire packet was captured.

The length check concerns the caller's declared length, not an independently
validated actual allocation. The native short-length branch can leave a zero
result; do not claim native strict rejection or safe malformed-input handling.
Our inspector's minimum actual slice length is deliberately stricter.

Standalone x64 `0x12810` separately subtracts `0x222000` at `0x12830`, checks a
range ending at `+0x20`, then uses indexed tables at `0x1298c`/`0x12964`.
That is bounded corroboration of a control-selection region, **not** recovery of
x64 record layout, full case membership or cross-architecture ABI parity.
The WBHD driver copies and macOS path are not behaviorally covered here.

## Independent Rust subset and verification

`variax_bridge_record::ControlRecord` borrows a supplied offline snapshot for the
two selected control values only. It requires at least 16 actual bytes and count
at most 8, then exposes a neutral raw word, counted bytes and unchanged original
slice including all slack/extensions. It never manufactures or submits a request.
It neither registers a hardware backend nor associates a bridge with a guitar.
Other controls, short slices and invalid counts get distinct rejection results.
This is an inspection boundary, not an implementation of native IOCTL behavior.

Six owned synthetic tests cover both controls, counts 0–8, every count 9–255,
every shorter slice, wrong/adjacent control values, little-endian words and
uninterpreted slack/extensions with byte/pointer preservation. A standalone
`rustc --edition 2024 --test src/variax_bridge_record.rs` run using the inventoried
Rust 1.99.1 passed all six methods. No dependency, vendor library or original
record is used. Static table checks are separate from these behavioral tests;
neither is a native oracle or transport/session compatibility test.

## Capability boundary and remaining next steps

| Family / connector / OS / operation | This pass | Still unknown |
| --- | --- | --- |
| Standalone bridge / Windows x86 / selected control buffer inspection | Source-qualified dispatch and two neutral buffer shapes; owned Rust inspection tests | Endpoint/interface choice, framing meaning, actual session, timeouts/cancel/refcount/disconnect beyond earlier PnP trace |
| Same bridge / Windows x64 and bundled WBHD drivers | Separate hashes and bounded x64 control-selection lead | Complete ABI/case/behavior parity; no generic fallback to x86 |
| Guitar/bass behind bridge / any OS / recognition or editing | No new applicability claim | Exact attached identity and family/version constraints; raw word cannot fill them |
| Central / Windows/macOS / discovery, maintenance | Prior matrix retained | Config/callback producers, scanner construction and indirect dispatch targets unresolved |
| Workbench HD / tuning/editor/library | Prior static records retained | Stored tuning units/domains and instrument-specific applicability; GUI ranges are not device limits |
| PODxt Live/HD500X as WBHD host / control/audio | Remains unverified | No newly established connection or audio route; no issue #31/#32 capability release |

Full issue coverage remains partial: original-package inventory is reused with
new member checks (criterion 1); selected x86 control shape advances criterion 2;
topologies, Workbench HD and Central contracts (3–6) are not completed; the matrix
and independent Rust boundary (7–8) cover only this subset; offline verification
(9) has no native/device oracle; delivery (10) is a focused partial PR, without
new follow-up acceptance or parent closure. Central/tuning work was not duplicated
from other active tracks. No placeholder session API or fake physical-success
simulation is added. Further source-backed recovery remains necessary before
routing/lifecycle fixtures, codecs or capability registration can be justified.
