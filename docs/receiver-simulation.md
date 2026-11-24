# Bounded receiver and session simulation

The Rust `simulation` module is an in-memory protocol/control-flow model for
issue #18. It has no device handle, physical transport, device firmware CPU,
flash image, OS adapter, real timer or recovery implementation. It cannot open
USB, MIDI or audio. The existing physical locks and absent live backend remain
unchanged. This is not a complete PODxt Live or Variax emulator.

## Evidence and independent checks

The source map is [update/recovery research](update-recovery.md). The Variax
application receiver was statically traced in the archived 3.10 image. Selected
boot-code receive loops were traced separately. The original updater's host
packing was exercised in bounded private x86 emulation with intercepted calls;
its derived reference is `research/updater-oracle-reference.json` (46 cases).
The Rust tests compare independently generated synthetic stream hashes, send
counts and relevant fragment lengths to that reference. They do not copy vendor
code, firmware, saved packets or user settings into the test suite.

The sender descriptions and receiver parser have separate traversals. The host
reference corroborates packing, not all receiver semantics. Receiver intent
ordering comes from static firmware instructions, not firmware execution or an
observed physical state. The test-only JSON reader uses the already locked
`serde_json` version; it adds no production core dependency.

## Implemented model

| Component | Supported scope | Limits |
| --- | --- | --- |
| Container candidate | Existing L6FF inspector, embedded MD5, declared product/version, source and expanded-image SHA-256 | No signature, exact archived-file requirement, installability or preservation claim |
| Variax main code | Command 0x60, six low-first count nibbles, low-first 24-bit words, additive 24-bit checksum, terminator | Nonempty aligned input; model cap 32768 words; programming helper effects unmodeled |
| Variax boot region | 0x6F operand 2, 16384 high-first encoded bytes, additive byte checksum, terminator | No entry into boot code, ROM, bank switching or reboot |
| Variax user patches | 0x62, 60 payloads of 3276 bytes and three 16-byte group trailers | 4096-byte source-slot profile only; internal preset checksums/semantics not validated |
| Variax factory templates | 0x67, 50 payloads of 2619 bytes | Internal preset loading, calibration and audio unmodeled |
| Explicit erases | 0x63/0x68 application-mode commands; intent before terminator | No physical sectors, erase timings, polling or success proof |
| POD host segment | 0x6D, up to 64 big-endian words per packet, 6/6/4-bit packing, all-FF child-writer skip | Hash/count summary only; no POD DSP receiver, acknowledgment or flash model |
| Virtual POD/guitar metadata | Logical interface 1, identity, enable, two ordered blocks, disable, fragmented/realtime framing | Logical routing only; no USB endpoint/driver or audio behavior |

The Variax stage is selected explicitly. Application mode is the default scenario
assumption. Boot-code mode admits only main/boot receive loops; it rejects preset
and full-bank erase operations because its sector handler differs. Neither mode
is detected from hardware. `recover()` always returns `UnsupportedRecovery`.
Unmodeled POD receive operations fail explicitly.

This model handles one selected transfer stage at a time. It does not orchestrate
a complete host update or infer which regions to install. The original parent
can erase before a transfer, preserve or replace user regions according to flags,
and visit factory/user regions in table order. The archived Variax image places
factory before user. Ordinary recovered asynchronous flags do not select the
boot stage merely because the image contains one. A blank POD child writer
sending nothing does not establish that its parent skipped erase.

## Intent, replies and faults

Main/boot payloads are buffered and checked before recording programming intent.
User/factory handlers call programming-like helpers during reception, so a
truncated or damaged stream can already have programming intent. Explicit erase
handlers are entered before their final terminator check. A malformed terminator
is therefore not a general promise of no prior effect.

`programming_intent_bytes` counts input associated with programming-like calls,
not completed writes. `erase_intent` covers explicit erase commands only; false
does not exclude erase inside programming helpers or earlier host stages.
`possible_persistent_change()` reports intent within this selected model stage;
false is not evidence of an unchanged physical device.

`ModeledReply` and `modeled_acknowledgment` describe the conditional normal path
if unmodeled peripheral helpers return. They are not observed replies. The model
never sets `hardware_safe`, `durability_known` or `peripheral_helpers_modeled`.
Timeout, disconnect and power loss retain prior intent and do not roll it back.
A fault after a modeled reply retains that reply as history while changing the
phase to interrupted; it does not promise durable data or successful recovery.

Input bounds, strict headers/stages and canonical-nibble checks protect the
model. They are not evidence that hardware has the same guards; some original
paths mask noncanonical values. The additive checksums are not authentication:
a compensating payload change can preserve them. User/factory transfer paths
have no equivalent outer packet checksum in this model; that says nothing about
internal DSP-program checksums or whether altered content can load or sound.

## Virtual metadata session

`VirtualControl` supplies synthetic metadata and a synthetic identity revision
ASCII `0001`, deliberately distinct from a saved unit identity. It rejects the
POD logical role and all operations outside the existing read-session allowlist.
The scenario can drop or duplicate metadata, disconnect, cancel, fail cleanup,
and split framed bytes with realtime interleaving. Pauses advance a counter;
there is no real scheduler, latency measurement or delayed delivery model.

Duplicate-response testing exposed an ambiguity in `read_current_on`: the two
metadata replies have no usable address/sequence correlation, so a duplicated
first payload could be mistaken for the second. The client now declines
identical decoded blocks and still attempts disable. Legitimately equal blocks
are also declined because they are indistinguishable. This does not establish
freshness or atomicity for different stale, reordered or mixed replies, and it
does not turn current metadata into a complete backup. No additional request or
opcode was added, and physical access remains disabled.

## Workbench application write transcript replay

The [#175 differential harness](variax-write-transactions.md#offline-reproduction)
replays its 180 original Workbench transcripts (60 each for edit, store and bank
name). A test-only reader reassembles the original send fragments and routes
session on/off (`0x2A`) through the existing virtual control model. The fixture
starts identified because #175 starts with a known target. The replay checks
the host wire shape and requires a complete `0x43` in the same transaction
before `0x61`. A short full payload, wrong count, absent `F7` and store without
a preceding full upload are rejected by the replay. These are host transaction
checks. The firmware's handling of these failures is unknown.

The application-mode updater receiver reports `WrongStage` for all six write
opcodes on every replay that contains them. The
[3.10 dispatcher inventory](firmware-variax-dispatch.md) establishes entry
points and traces `0x2A` and `0x3C`. The write handler semantics remain outside
this model:

| Opcode | Replay scope | Receiver gap |
| --- | --- | --- |
| `0x43` full buffer | Count 1,092, exact body length and nibble encoding | Edit-buffer application, checksum handling and partial-frame effects |
| `0x3B` live difference | Nibble and word alignment | Run interpretation, including the original terminal order, and DSP application |
| `0x4C` DSP control | Single on/off byte | DSP state and audio |
| `0x61` store | One slot byte in 0–59 and prior full frame | Flash operations, `0x71` generation, interruption and recovery |
| `0x6C` / `0x6D` bank name | 18-byte set and empty read body | Bank selection, name persistence and reply |

The PODxt bridge messages, tone CC and bank-select Program Change remain
outside receiver emulation. Timing and original chunk boundaries are verified
by the existing #175 differential comparison. Store and bank-name replies in
that comparison are synthetic.

## Validation scope

Sixteen module tests include independent host-reference hashes at word/chunk
boundaries, a complete synthetic boot transfer, full user/factory shapes,
checksum corruption and checksum collisions, wrong product/version declarations,
truncation, bounds and stage errors, partial programming intent under all three
fault kinds, unsupported recovery, cleanup failures and duplicate ambiguity.
A 26-byte main transfer is also tested under all 208 single-bit mutations.
This is a bounded regression set, not the late-project fivefold expansion in #33.

No DSP audio, sustain, bending, alternate tuning, knob/switch measurement,
physical flash, persistent backup, complete update sequence or ROM recovery was
validated here. Those limits remain explicit in the readiness review and later
issues. Passing this model never authorizes connecting, reading or writing the
irreplaceable hardware.
