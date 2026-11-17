# Preservation-only XFP6 subset and next-edge status

Bounded continuation of guirossibrum/VariaxWorkbench issue #40 after PR #84.
This adds a Rust envelope inspector/save-copy, not a plugin-state converter,
restore engine, PODxt control editor, pack entitlement query or DAW adapter.
The parent issue and its incomplete acceptance criteria remain open.

## Current source and reproducibility

The task branch was advanced by authorized fast-forward from `685e75153713041c6483e928ed7650e58c1137be`
to fetched master `0ae5c7ff034e7d157f97b5aec8f1ad79eeb08a3e`, after proving
ancestry and inclusion of PR #84 merge `05e03c75e69de69f6747634b02821d1b8f4d7bb1`.
The old head is preserved by local checkpoint
`refs/checkpoints/issue-40-a365af96-685e751`. No history rewrite occurred.

Current-assignment `prepare-worktree` accepted that base with the base private
inventory SHA-256 `ae6f747b2dcf4d52244e6106f11465eadfdb4654983467310695bd9e31c59184`
and policy SHA-256 `e34ecaafc47dc2385eb9fa281d6bd431151c671b03fbb7e1704ce0715eb7d34c`.
The inventoried original POD Farm 2.58 DMG was re-extracted statically with the
existing owned inventory helper and trusted 7-Zip 26.06. It remains 201,113,929
bytes, SHA-256 `483e4129f2d319665ab7273194818acf570181cac8eb8f75dc5f96581715217e`.
No old assignment's supplemental authority or original-derived trace was reused.
New output stays private under `analysis/issue-40-next/`.

Independent FAT-slice extraction matched the prior qualified hashes:

| Slice | SHA-256 |
| --- | --- |
| AU x86-64 | `715d30b8fed77d4ea18150050131db759b8e8a27eeaf4e3d9b3872b6bca6fbe0` |
| VST i386 | `e68b3d63d0cda33d6d3e3443bfd472fa412ac1fa5ae7636d7d532ed16369a0c4` |
| RTAS i386 | `950cc1522d9cfe4fbbba0be5f8f9e3f5825fe6278d45a1f16baebc62fdc81c88` |

Ghidra 12.2 / Temurin 25.0.5 static analysis selected AU `0x93a4a` and
constructor `0x29014`, plus their discovered callers. The original
`0x9382c` selection was not recognized as a function in this fresh database;
that is a tool-analysis gap, not evidence the function is absent. Import emitted
relocation/address-overflow warnings. Decompiled pseudocode and auto-created
function boundaries are therefore leads, not an independently validated oracle.
The VST constructor and selected map lookups were also re-read in a fresh i386
database; they do not establish final populated host IDs/count. The independently authored selector is `research/pod_farm/StateEdges.java`;
reports and databases are never published. Headless project directories cannot
contain dot-prefixed path elements: initial attempts in `.git-worktrees` stopped
before import. The eventual task-owned database is private at
`/tmp/variax-issue40-c87bd7d2-projects/`, with logs in ignored task storage.
No vendor instructions were executed.

## Finite contract edges: evidence and limits

| Edge | Current evidence | Remaining boundary |
| --- | --- | --- |
| AU deferred state | AU `0x93a4a` again shows an owned stream cached at object +`0x238`; fresh field references identify initialization at `0x9433c`, destruction at `0x92e44`, and cache replacement in restore. | Eventual application/consumption still not established. Other objects also use displacement `0x238`; matching offsets alone do not establish an AU receiver or absence of a consumer. |
| Descriptor/host table | Prior constructor count 25 versus guide's 40 controls remains in the [acceptance ledger](pod-farm-acceptance-coverage.md). | Actual final populated count/ID table and host-to-model bindings remain unresolved; no stable automation-ID table registered. |
| RTAS outer identity | Fresh i386 caller `0x2adfc` (and copy path `0x2affe`) passes numeric `0x36503250` through constructor `0xac9b4` to process +`0x2f4`; adjacent field +`0x2dc` receives `0x36503248`. Matching chunk methods compare the former. | This resolves the selected chunk-ID producer as a static numeric constant, not a PODxt product ID. Complete 56-byte outer prefix and host capacity/lifetime contract remain unverified; no RTAS import/application support added. |
| Target applicability | Current #29 disposition [comment 6933803124](https://github.com/guirossibrum/VariaxWorkbench/issues/29#issuecomment-6933803124) reports the examined 1.12 graph does not establish a PODxt Live numeric-control contract. | This 2.58 task does not duplicate or overturn that trace; names/ranges/shared IDs are not target admission. |

These unresolved edges prevent semantic conversion/automation/application, but do
not prevent a clearly labeled opaque preservation-only envelope copy. This PR
must be reviewed as that limited subset, not as completion of all next-step edges.

The RTAS caller supplies two distinct constants: `0x36503250` (chunk ID) and
`0x36503248` (adjacent constructor field). Their little-endian memory bytes are
`50 32 50 36` and `48 32 50 36`; this does not assign a cross-plugin FourCC
interpretation, outer-header layout or shared hardware namespace. Reproduce the
connected selection with `StateEdges.java 000ac9b4 0002adfc 0002affe` against the
pinned RTAS slice. AU selections are `00093a4a 00092e44 0009433c`; the VST
selection is `0002aa2c 0009f074 0009f238`. These are static disassembly/decompiler
coordinates, with all import limitations retained.

## Supported subset and explicit exclusions

`pod_farm_state::PreservedState::inspect` admits only an XFP6/byte-swapped
fixed-244-byte envelope whose declared total equals input length. Every present
section needs both offset and length, starts after the fixed header, fits the
input and is disjoint from every other section. A 16 MiB owned policy cap bounds
allocation; it is not a recovered vendor limit. Empty sections are represented
by a zero/zero pair. An empty envelope has no claimed native validity.

The four sections are inspected as opaque tone A, tone B, mapping and settings
spans. Out-of-header-order sections are supported. Prefix scalar/name bytes,
unknown gaps, nested bytes (including duplicate/unknown chunks or records),
trailing bytes and endian representation are retained exactly. No reconstruction,
sorting, deduplication, byte swapping or normalization occurs on save-copy.
`section` returns the exact source span and `as_bytes` returns the unchanged
whole envelope. Copies use the existing atomic non-overwriting `document::save_new`.

Header extensions, dangling half-present spans, alias/overlapping sections,
truncation, inconsistent totals and unsupported signatures receive distinct
`StateError` values. This intentionally strict policy is not native acceptance:
the original readers can accept extensions and mutate partially as described in
the prior ledgers. No source-derived native safety or permissiveness claim follows.

`UNVERIFIED` explicitly reports nested tone/pack compatibility, automation bindings,
positional settings application and host product/version interpretation for every
admitted envelope. Arbitrary product-like header words never become PODxt identity.
Unknown model/pack mismatches remain unverified, not silently substituted or
reported compatible. PINF, L6P2 and nested chunks have **no semantic edit/conversion
support** in this module: direct non-XFP6 input is rejected; embedded content is
opaque. This preserves bytes without promising semantic lossless interchange.
No native restore, plugin adapter or device path calls this module.

Packaged resource inventory remains distinct from installed/authorized state.
No user license schema, entitlement record, credentials, authorization service or
activation behavior was accessed. DSP/audio equivalence and current host/OS
compatibility remain unknown. A future adapter still needs independent DSP,
parameter/automation IDs and state/application references before ABI selection.

## Verification and issue acceptance coverage

Six owned Rust test methods cover both endians, out-of-order/absent sections,
opaque duplicate/unknown bytes and tails, every truncation of a synthetic frame,
appended-data disagreement, fixed-header extensions, dangling spans, arithmetic
bounds, overlap/alias rejection, direct foreign signatures, policy cap,
product-word non-admission, and actual save-copy/reopen, existing-path refusal,
missing-directory failure and original/state preservation. Synthetic bytes were
authored from the published prefix contract, not from vendor preset captures.
They test owned policy/roundtrip behavior, not permissive native compatibility.

Local focused Cargo invocation initially stopped during offline dependency
resolution (`eframe` absent from the local registry), before compiling tests.
No online fallback or duplicate raw full suite was used. Configured exact-head
checks must run through the coordinator; no pass is implied by the test inventory.
The local Rust tool is the inventoried 1.99.1 toolchain. Rustfmt and diff checks
are separate from actual compilation/testing. The historical 69 Python methods
and 3,318 two-leaf cases were neither rerun nor added to these six methods.

| Full issue criterion | This milestone / remaining gap |
| --- | --- |
| 1 Original/inventory | Original/source slices reverified; signature trust and runtime compatibility remain unknown. |
| 2 Code/control recovery | Bounded AU static corroboration; real populated host IDs and target control definitions unresolved. |
| 3 PODxt capability matrix | Prior matrix retained; no new PODxt operation admission. |
| 4 Packs | Packaged-vs-installed distinction retained; runtime entitlement schema absent. |
| 5 Formats | Real preservation-only Rust XFP6 envelope inspection/save-copy; nested/automation conversion, bank/slot codec and semantic interchange incomplete. |
| 6 Modern DAW | No adapter or independent DSP; finite host binding/RTAS gaps above remain. |
| 7 Offline validation | Six synthetic Rust methods added, requiring genuine exact-head check results; no native state/audio oracle. |
| 8 Delivery | Scoped owned code/tests/derived notes and README update; draft follow-up triage and parent acceptance remain incomplete. |

No acceptance checkbox changes, parameter edit API, hardware unlock, runtime
entitlement claim, production math profile or new dependency is included.
