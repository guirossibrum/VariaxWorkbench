# Static update and recovery research

Issue #16 remains in progress. Static analysis is now supplemented by selected
original host-updater methods running inside an isolated emulated i386 CPU.
Every transport, event and helper call is intercepted by a memory-only adapter;
unknown code or imports stop the experiment. No vendor code runs natively,
no DLL entry point or device firmware runs, and no device is contacted. These
findings do not authorize a live packet, reset, erase, update or recovery entry.
All hardware source locks remain enabled.

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
It has not been invoked. The separately selected internal methods below run
only under the bounded emulator, without a Windows loader or OS forwarding.

An independently checked native function at 0x100D4B10 has a product-0x00070004
branch and a test of object byte +0x44 bit 0 leading to a patch-erase abort log.
Another product branch is 0x00070001. The method's boolean argument selects
factory-patch versus user-patch erase data. Ghidra decompilation, caller flow and
the exact flag origin remain under review. This is evidence of a conditional
static guard, not an observation that the current unit is in boot code, proof of
preservation, or a reason to attempt an erase.

Original vendor files, packets, strings/disassembly and analysis databases stay
private. Product IDs, hashes and derived relationships are the public record.

## Bounded host-method oracle

`scripts/native_updater_oracle.py` requires the exact DLL fingerprint above. It
maps PE sections into private Unicorn memory, replaces imports and virtual
transport calls with traps, and permits only audited method ranges and explicit
helper adapters. It never loads the DLL into the host process. Guest instruction,
time, payload and accumulated output limits remain enabled. Unicorn is an
existing pinned research dependency, not an application dependency.

The code sender at 0x100D46F0 agrees with an independent Python description for
synthetic payloads of 0, 1, 2, 31, 32, 33, 64 and 65 24-bit words: transfer count
includes the checksum word, payload chunks contain at most 32 words, each word
uses six low-first nibbles, and the additive checksum wraps at 24 bits. Four
non-word-aligned inputs confirm that this host method ignores trailing bytes;
that observation does not approve such inputs for a receiver. Its caller's
input validation remains under review.

Two cases with the preservation flag clear exercise a simulated erase delegate:
a negative delegate result prevents sending, and zero permits the code transfer.
The concrete virtual-method binding and the owner's preservation choice are
not established by those cases. A negative send-status adapter reaches the
normal-body boundary and stops before unmodeled exception handling. The first
test expected a later exception import; the strict boundary stopped earlier at
0x100D4AA5. That expectation was corrected without expanding the allowlist, and
the interrupted guest CPU was discarded. The failed research log is retained.

Four erase-method cases cover user/factory selection, its boot-flag guard and a
simulated wait timeout. The method returns zero for the boot guard without
sending, and also returns zero after a timeout. Consequently its return value
alone cannot prove acknowledgment or completion. The event, status, erase
delegate, message pump and security-cookie helpers are simulated; this is not
an end-to-end updater or flash model. Windows documents zero as the signaled
wait result and 0x102 as timeout. [WaitForSingleObject reference](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitforsingleobject).

The source-bound private report `native-updater-oracle-b5912a17c67bddd1.json`
records these 19 passing cases. Output contains derived lengths, hashes and
adapter events; original binary bytes and emitted buffers are not published.

## Preset-slot traversal corroboration

The user-patch sender at 0x100D4BF0 traverses three groups of 20 slots. For
product 0x00070004 and versions above 0x0303FFFF, the source stride is 4096 bytes;
only the first 3276 bytes of each slot are encoded. Earlier versions use a
3276-byte stride and skip another 16 source bytes after each group. A generated
16-byte trailer follows each group. This independently supports the archived
3.10 slot interpretation in [factory extraction](factory-extraction.md); it
does not establish a physical switch-position map or safe preset replacement.

Two additional guest-CPU cases use distinct synthetic per-slot data for 3.10
and 3.03 layouts. Both agree byte for byte with an independent traversal,
including all 60 slots and three generated trailers: 2105 fake send calls per
case, each at most 192 bytes, under fixed guest and output limits. The MSVC stack
probe is another explicit guest-memory adapter; no host stack operation or
allocator is forwarded. The private report
`native-updater-oracle-9c8dbddab8d9f0da.json` records all 21 passing cases. These
cases contain no archived preset bytes.

This patch-writing method requests the simulated user-erase delegate regardless
of the code sender's preservation flag. That is a method-level observation:
which update stages the caller selects, and how it restores saved settings,
still require tracing. It would be incorrect to infer that the UI's keep-presets
choice is ignored or that calibration is preserved.

The oracle is an opt-in research command using the existing pinned environment:

```sh
.tools/python/bin/python scripts/native_updater_oracle.py /path/to/UpdaterV3EngineDLL.dll
```

The source file must be a bounded regular local file with the recorded exact
fingerprint. Reports are saved without overwriting under ignored
`analysis/reports`. No original DLL is distributed or required by the Rust app.

## Response callbacks and virtual-method evidence

Static constructor code at 0x100CEFBE installs vtable 0x1026AB50. For objects
using that table, slot +0x24 points to erase method 0x100D4B10 and slot +0x20 to
the update-sequencing method at 0x100D3D60. This closes the method-slot binding
for that class; it does not prove a connected unit or selected caller state.
The oracle still uses a simulated delegate to isolate the transfer methods.

Registration code at 0x100D3B80 associates response commands 0x70 and 0x71
with callback 0x100D5E40, commands 0x72 and 0x73 with 0x100D5E70, and command
0x6E with 0x100D5EA0, using selector 0x0007007F. The callbacks store status at
object +0x14C (respectively 0, -3022 and -3038) and signal its event if present.
The transport's selector matching remains outside this trace; these registrations
must not be treated as a verified standalone packet decoder.

Three additional isolated guest cases execute those callbacks against synthetic
objects and a fake SetEvent adapter. Each stores the expected status and emits
no transport bytes. The combined private report
`native-updater-oracle-00f50264c735f759.json` has 24 passing cases. They support
host callback behavior, not receiver state, durability, or a physical reset.

## Variax stage selection

Static decompilation of 0x100D3D60, checked against its direct call sites,
establishes this method-level order. Field meanings refer to this updater class,
not an observed device state:

| Stage | Selection and behavior |
| --- | --- |
| Preconditions | Requires an image object and object +8 equal to 0x10000. A separate product-0x70004/high-flag-byte-4 branch calls virtual slot +0x28 with arguments 0 and 3 and leaves the normal sequence. |
| Boot region (raw tag 5) | Selected by flags +0x54 bit 27; first matching region calls 0x100D56A0 before the main-code stage. A product/version-specific erase-range call follows for 0x70004 above 0x0305FFFF. Range units and physical mapping remain unverified. |
| Main code (raw tag 0) | First matching region calls the code sender 0x100D46F0 for 0x70004. |
| User patches (raw tag 1) | Calls 0x100D4BF0 only when flags +0x54 bit 0 is clear. |
| Factory patches (raw tag 2) | A separately computed applicability predicate selects 0x100D5060. Its full version/product predicate is not yet closed. |

The base entry at 0x100D0890 stores its first argument into object +0x54 and
later invokes update slot +0x20. Thus bit 0's skip-user-patches effect is tied
to an update argument, but the entire path from the Central UI choice to that
argument is still unverified. The sender's unconditional erase delegate is
inside a stage which this caller can skip; it does not prove the keep-presets
option is ineffective. No calibration copy/restore guarantee follows.

When a transport-description test identifies a PODxt-capable link, the method
also brackets Variax transfers with large-buffer enable/disable requests. These
are Variax transfer support through a POD link, distinct from updating POD main
flash. No such request has been sent.

## Separate POD main-segment evidence

The same DLL's product dispatch at 0x100CED00 sends product 0x0003000A through
its two-level jump table to 0x100CED9E. With the dispatcher's second argument
zero, constructor code installs vtable 0x1026AD24 at 0x100CEE0B. Its update slot
+0x20 points to 0x100D6D20, which calls segment writer 0x100D7330. The meaning of
the alternate dispatcher argument remains unverified; it must not be labeled a
USB-component path merely because it is an alternative.

Seven isolated segment-writer cases cover 1, 2, 63, 64, 65, 128 and 129 synthetic
big-endian 16-bit words. They corroborate at most 64 words per fake send, encoded
into groups of 6, 6 and 4 bits, a count byte and terminator, and one simulated
port-wait call per chunk. The payload/count checks are independent descriptions;
they do not validate an entire POD update or its packet routing. An eighth
all-FF case confirms that this writer skips such a span. The calling update's
erase behavior is separate, so this does not establish that FF archive regions
are preserved or safe to overwrite.

The combined source-bound private report
`native-updater-oracle-0c54da0d12b75ae6.json` records 32 passing cases over four
host methods and three callbacks. POD logging and port waits are explicit
memory-only adapters. This corroborates a different word encoding from the
Variax path, without executing either device's firmware. POD boot/recovery entry,
flash durability and preservation remain unverified; the USB-component artifact
and installed version remain separately unknown on #20.

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
