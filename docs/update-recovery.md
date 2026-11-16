# Static update and recovery research

Issue #16 remains in progress. No updater, vendor DLL or device firmware has
been executed, and no device was contacted. The findings below are static
file/code relationships. They do not authorize any live packet, reset, erase,
update or recovery entry. All hardware source locks remain enabled.

## Variax boot-region structure

The exact 300/600 3.10 source fingerprint is recorded in
[the firmware map](firmware-map.md). Region 0, raw tag 5, decomposes as follows:

| Region-relative bytes | Observation |
| --- | --- |
| 0–167 | One 51-word load-record candidate plus terminator; nominal entry 0x200 |
| 168–1023 | 856 FF bytes |
| 1024–12873 | 41 load-record candidates plus terminator; nominal entry 0x26C |
| 12874–16383 | 3,510 FF bytes |

The second stream has 38 records with raw tag 0, one with tag 1 and two with
tag 2. Its tag-0 records form two contiguous program ranges. Independent static
DSP56300 decoding finds an absolute reset jump at P:0 to P:0x26C. That entry
initializes registers and jumps to P:0xE04. The original chip's boot-ROM protocol,
ROM contents and physical entry conditions remain unverified. A later reset path
references P:0xFF0000; that is not an archived ROM image or an approved entry
sequence.

`scripts/map_firmware.py` now reports both candidate streams and validates both
padding ranges. Ten synthetic layout tests include two new boot-stage cases:
whole-region accounting, malformed interstage/end padding, wrong shape and an
oversized second-stage record. No original firmware bytes are test fixtures.
The private source-bound mapper report is
`firmware-map-9a0de574c8b6818c.json` in the issue worktree.

The second stream contains receive paths checking manufacturer/family framing,
terminators, transfer sums, response values and reset branches. These paths are
still being traced against the updater. Counts, ordering, boundedness, timeouts,
flash destinations and failure/power-loss behavior are not established as a
complete protocol. No sendable update API follows from this partial decoding.

## Central is a static reference, not a runnable recovery plan

The archived Line 6 Central 1.00 installer has SHA-256
`6d7a4ba9efb2a6f9f45dd996cd2a7fe34eb7355ec77c8d4a6795b6c64bc9aee6`.
Static NSIS extraction, without executing the installer, recovers:

| File | SHA-256 |
| --- | --- |
| UpdaterV3EngineDLL.dll | 6786a97458a5a6e5d225e5c2d2804281a8e0a082a0d992f558ce041c144ae078 |
| res/Variax.json | 0212bb5832870d6d8f74d61445d1c0ce1043d7657b50892f561766277fef1b1e |

The JSON declares product 0x00070004 separately from 500/700 and other Variax
models. Its UI progression requests license acceptance, a keep-presets choice,
engine progress and a completion display. Progress labels distinguish main,
factory-patch and user-patch stages. These labels do not prove which bytes are
preserved or how calibration is handled.

The DLL is x86 PE, image base 0x10000000. REA 6.9.0 with Ghidra 12.2 is analyzing
it in source-immutable static mode. Its exported function named
`doOfflineFirmwareUpdate` is **not** a hardware-free analysis function; the
vendor's word "offline" must not be interpreted as permission to invoke it.
It has not been invoked, nor has any other vendor entry point.

An independently checked native function at 0x100D4B10 has a product-0x00070004
branch and a test of object byte +0x44 bit 0 leading to a patch-erase abort log.
Another product branch is 0x00070001. The method's boolean argument selects
factory-patch versus user-patch erase data. Ghidra decompilation, caller flow and
the exact flag origin remain under review. This is evidence of a conditional
static guard, not an observation that the current unit is in boot code, proof of
preservation, or a reason to attempt an erase.

Original vendor files, packets, strings/disassembly and analysis databases stay
private. Product IDs, hashes and derived relationships are the public record.

## Work still required

Cross-check updater product/image applicability, transfer framing and sequencing,
integrity coverage, acknowledgment/error paths, preservation choices and recovery
branches. PODxt Live main-flash and USB-component paths must remain separate;
the latter still has the unknowns on #20. Establish simulated behavior only for
states supported by independent evidence; unsupported behavior must fail rather
than invent a recovery success. Physical flash mapping, boot-ROM behavior,
calibration preservation, disconnection and power-loss consequences remain open.

This research does not implement a live updater. #18 owns the later memory-only
state simulation; #22–#25 retain readiness, backup and physical-operation gates.
