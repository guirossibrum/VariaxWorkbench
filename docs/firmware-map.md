# Static firmware layout research

Issue #13 records a static baseline map with the unknowns listed below. All
findings come from archived files; no device was contacted and no firmware was executed or installed. Source firmware and
disassembly remain private. Image offsets are file offsets, and candidate
loader destinations are word addresses, not approved flash offsets.

The [schematic reference](rig-schematics.md) records the processor, memory, and
signal paths shown on the Variax electronics and the standard PODxt SHARCPOD
board. It includes page and revision identifiers for checking the local PDFs.

## Variax 300/600 archive 3.10

Source SHA-256:
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`.
The existing L6FF inspector verifies product `00070004`, version `03100000`,
the 442,402-byte decompressed image and its embedded MD5. That does not establish
publisher authenticity. The outer region tags remain numeric.

| Region | Raw tag | Bytes | Observed structure |
| --- | --- | --- | --- |
| 0 | 5 | 16,384 | Initial prefix, second 41-record stream at 0x400 and FF padding |
| 1 | 0 | 49,308 | 117 three-u24-header load records and an exact terminator |
| 2 | 2 | 130,950 | 50 contiguous records of 2,619 bytes |
| 3 | 1 | 245,760 | 60 slots of 4,096 bytes with a 2,619-byte program prefix |

The main-region candidate header is little-endian 24-bit count, destination and
memory tag, followed by `count` words. A zero count ends with one destination
word. Tags 0/1/2 appear 88/7/22 times, with no overlapping destinations within
a tag. The exact terminator's nominal entry is word `0xDBC`. Destination zero
contains a DSP56300-compatible absolute jump to that entry. The entry initializes
registers and jumps to a separately mapped block at `0x14004`.

Phase A of #181 maps all 110 SysEx dispatch entries in the main region and
traces the session flag, DSP bypass, the current-preset buffer read and its
reply limits, the full-buffer and difference handlers, the preset-store
flash sequence, the slot read, the wide erases, the bank-name and
string-level sectors, the other program and code-load handlers, and the
remaining 24 command handlers. Those 24 have no direct call to the erase
or program helpers in the scanned windows. `0x3E` ends in an indirect
call whose target was not found. The guitar can send `0x28`, `0x74` and
`0x4D` on its own, and, when the session flag is on, a program-change-shaped
status plus one data byte and a three-byte status plus two data bytes.
See the [Variax 3.10 dispatcher inventory](firmware-variax-dispatch.md).

Independent static decoding used the MIT-licensed Rust DSP56300 toolkit v1.0.0,
commit `aab3649fc452daf03da8b5e36fdc0c308d874101`, built separately with
Rust 1.99.1, `cargo build --locked --release -p dsp56300-disasm`. It is an optional
research tool, not a production dependency. This supports a DSP56300-compatible
ISA interpretation; the exact installed chip is unproven. The manufacturer's
[DSP56364 manual](https://www.nxp.com/docs/en/user-guide/DSP56364UM.pdf) describes
24-bit program/data words and separate program/X/Y spaces. The file parser keeps
the numeric memory tags rather than claiming a complete hardware memory map.

Reversing bytes within each three-byte word reveals `Variax 300/600` in the
main region. It also reveals 18-byte names at record-relative byte 2,553 in both
data regions. All 50 template records occur byte-for-byte at slots 5 through 54.
Four other slot prefixes also match templates exactly; six differ. This is
evidence of a template/slot relationship, not a backup of the owner's current
guitar. The current host renderer emits 3,276-byte outputs, so these 2,619-byte
records must not be treated as interchangeable upload artifacts. Subsequent
[factory extraction](factory-extraction.md) identifies complete 3,276-byte
programs inside the 4,096-byte slots and validates their checksums. This resolves
the record-boundary question while leaving upload behavior unverified.

The initial prefix view of region 0 consumes 168 bytes. Later static update
research identifies 856 FF bytes, a second 41-record stream at file offset
0x400 consuming 11,850 bytes, and 3,510 FF bytes through the end of the region.
The second stream has nominal entry 0x26C, with a program-word reset jump to that
address. These are file-derived loader candidates. Their apparent headers must
not be assumed to be the chip ROM's boot protocol; see
[update/recovery research](update-recovery.md). Record roles, slot tails, calibration/preservation behavior, checksums
inside the records and the exact IC variant remain unresolved. No restoration
or patching operation follows from this analysis.

## PODxt Live archive 3.01

Source SHA-256:
`738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5`.
The existing inspector verifies product `0003000A`, version `03010000`, the
1,048,576-byte image and embedded MD5, with 19 contiguous regions. Seven regions
are entirely `0xFF`; their values do not prove they are safe to overwrite.
The following internal boundaries are derived from the image, independently of
outer region tag meanings:

| File range or region | Observed role | Remaining limit |
| --- | --- | --- |
| Regions 1/2, 0x4000/0x6000, 8,192 bytes each | Similar settings/name-bearing data | Persistence, calibration and redundancy rules unverified |
| Region 3, 0x8000, 32,768 bytes | 28 startup display frames followed by padding | Frame format verified offline. Runtime selection and LCD transfer remain unverified. |
| Region 4, 0x10000 | Length/checksum wrapper and four complete load records | Loader implementation and hardware behavior unverified |
| Region 5, 0x20000 | Length/checksum wrapper, 65,528-byte opaque payload | Payload ISA/encoding unverified; high entropy is not proof of encryption |
| Regions 6–11, 0x30000–0x90000 | Resource directory, named records and padding | Record bodies, licensing and execution semantics unverified |
| Region 17, 0xE0000 | Factory channel and setup records | Firmware programming and live device behavior remain unverified |
| Regions 0, 12–16, 18 | Entirely FF | Their values do not establish permission or safety to overwrite |

Regions 4 and 5 begin with a big-endian u32 count of 16-bit words, then a
big-endian u32 additive checksum. Summing the payload's unsigned big-endian
16-bit values modulo 2^32 gives `0x10D752E2` and `0x3F7F2359`, respectively.
Region 4 contains 24,534 words, followed by 16,460 FF padding bytes. Region 5
contains 32,764 words and no padding. These are integrity checks, not signatures;
there is no established publisher authentication mechanism in this analysis.

Region 4's 49,068-byte payload parses exactly as four records: big-endian u32
destination, u32 count, u16 width tag, then `count * tag * 2` bytes. This is the
observed file structure, not a claim that it is the generic Analog Devices
boot-stream format or a physical flash address map.

| Destination word | Count | Width tag | Payload bytes | Static interpretation |
| --- | --- | --- | --- | --- |
| 0xC000 | 742 | 2 | 2,968 | Data candidate |
| 0x1A900 | 5,582 | 1 | 11,164 | Data candidate |
| 0x8000 | 128 | 3 | 768 | 48-bit program/vector block |
| 0x8080 | 5,688 | 3 | 34,128 | 48-bit program block |

The program blocks occupy words `0x8000..0x96B8` without overlap. Big-endian
six-byte instruction decoding produces an IDLE at `0x8004`, followed by a jump
to `0x823C`; another vector jumps to `0x9348`. Both destinations are inside the
loaded program, and decode to interrupt-mask initialization and a data-memory
store, respectively. This and coherent arithmetic/call sequences support a
SHARC-compatible ISA. The reset-vector convention matches the ADSP-21065L entry
in the manufacturer's [loader manual](https://www.analog.com/media/en/dsp-documentation/legacy-software-manuals/438698055513140_loader_man.pdf)
(Table 4-2 and reset-vector discussion). It is an inference about compatibility,
not identification of the installed IC or proof that the entire image executes
correctly. A later pass still finds no second instruction set in the 19
regions. Region 5 stays an unidentified high-entropy payload. Regions 6–11
are the named resource directory. The host-facing processor and its firmware
are unknown. The relay opcode filter, reply-size cap, and pass-through
questions are recorded in [the Variax relay note](podxt-variax-relay.md).
Owner runs contradict reading a host opcode as a SHARC table index. The phrase for that reading is: contradicted by owner hardware runs;
host-to-SHARC translation unknown. The
[PODxt Live 3.01 entry, command, store, boot, and coverage map](firmware-podxt-dispatch.md)
follows the region-4 byte pipe, the SHARC-internal handlers, the two
`0x8000`-word halves used by SHARC opcode `0x71` and `0x936A`, the reset
checksum, and the trap list. Those flash routines and boot checks apply to
the SHARC command set. No host message is shown to reach them. Region 5's
encoding stays unidentified.

Independent static decoding used Ville Linde's BSD-3-Clause MAME SHARC
disassembler at tag `mame0285`, commit
`f379cd060ac3b2bd567ebbd0957aed059d6895ff`, files
`src/devices/cpu/sharc/sharcdsm.cpp` and `sharcdsm.h`. A local C++17 compatibility
shim supplies formatting, the disassembler interface and a big-endian six-byte
reader; no MAME CPU/device implementation is built or executed. This is a private
research tool, not an application dependency. The source pin, shim and private
decoder logs are retained in the issue worktree's ignored `.tools/sharc-research`
and `analysis/reports` directories.

The 128-word vector block decodes without unsupported markers. Of the 5,688
words in the following block, 28 lines contain an unsupported/TODO/register
marker; those may reflect decoder limitations or embedded data. They remain
unclassified. These counts do not certify every word as valid executable code.
Private decoder-output SHA-256 values are
`4efd704b9efd085a3d8c85572fe5d8ef79893ce5c0165b18d2b42dc0e52b222d`
(vectors) and
`d91cadf0891b4c8774e61d554ac69e31c46eba3d41df427103d6d8e240b18e1f`
(following block).

The resource directory has 486 big-endian key/pointer pairs plus a final
`FFFFFFFF` key carrying the end pointer. Converting each pointer to a file
offset with `2 * (pointer - 0x01000000)` yields strictly increasing record
boundaries from `0x31000` through `0x844EA`. Every record starts with two 32-byte
UTF-16BE name fields; all 486 pairs decode as ASCII text with zero padding.
The first record is named "No Amp"/"Bypass". The directory ends at `0x30F38`,
with FF padding up to the first record. The end pointer equals the first byte
after the region's final non-FF data. This is strong evidence of a word-indexed
resource relationship, not a proven DSP/flash memory mapping or license grant.

No USB-component firmware, installed component version, calibration-safe update,
or recovery sequence is established by this map. These unknowns remain on #20,
#14, #16 and the later safety/readiness issues.

### PODxt Live 3.01 component checkpoint

[#282](https://github.com/guirossibrum/VariaxWorkbench/issues/282) remains open.
The following findings come from the archived 3.01 image, static SHARC
analysis, bounded memory models, and the Live service drawing. The stock image
spans all 19 mapped regions.

| Area | Established offline | Still unknown |
| --- | --- | --- |
| DSP resources | A bounded model of the stock loading path reconstructs four base resources with the expected end marker. The Room record and resident tables account for three subcomponents and all 38 trailing parameter halfwords. The model follows initialization, parameter copying and event registration. The interrupt dispatcher can reach the registered Room graph under the conditions below. | The route and interrupt state in a running unit, whole-graph output, and audio equivalence. Other resource groups need independent verification. |
| Display | Region 3 encodes 28 bitmapped frames in 512-byte records on a 128 × 32 canvas. They contain the startup logo sequence, the standard model-set screen and 15 pack-status combinations. The Live drawing specifies a 122 × 32 visible LCD. | The code that selects and sends each frame, its visible crop, and the source of the reported pack state. The graphics establish which states can be displayed. |
| Board interfaces | The Live drawing puts the LCD and switch matrix on the MCU side and sends potentiometer and pedal readings to the MCU through a separate ADC. Variax digital audio reaches a dedicated receiver, and Variax MIDI has its own MCU serial channel. The service theory describes clock selection for internal, Variax and USB audio operation. | Firmware routing and ownership rules, panel scan and debounce behavior, and the persistent calibration fields. |
| Flash and persistence | For the documented bottom-boot flash option in word mode, the settings pair aligns with two independent 8 KiB sectors and the preset pair aligns with two independent 64 KiB sectors. A model of the load-time decision logic distinguishes the two bank selectors using archived inputs and 10 synthetic cases. | The populated flash part on a given unit, behavior during an interrupted write or erase, calibration and pack preservation, and end-to-end recovery. |

The device-side producer and storage of the installed-pack status remain
unassigned. The first-stage bootstrap and Region 5 execution remain open.
The flash and graph models establish behavior for their stated inputs.
They provide no hardware, update safety or recovery verification.

The tracked archive checks below can be rerun against an authorized local
copy of the firmware. The additional component verifiers, result logs and
checksum index are retained locally under the ignored
`backups/podxt-301-audit/` area. See its `SHA256SUMS` entries for the Room
graph, panel data and flash geometry checkpoints. The Room sample-path and
second-child checkpoints each have a private checksum index in the same area.

#### Room sample path

The archived interrupt vector targets a handler that walks a table of
registered callbacks when its global graph flag is set. The stock Room
load fills one row in that table. Its callback walks a second table with
three child processing callbacks. Each child template supplies distinct
parameter, initialization and processing entries. Earlier private notes
reversed the parameter and processing labels. The archived table before
registration has no Room callback, and the alternate dispatch with the
global flag cleared skips the Room row in the checked fixture.

The first child reads 16 samples, takes its filter coefficients from the
loaded Room state, and writes a 16-sample block and four history values.
A bounded interpretation of its decoded instructions matched an
independent scalar calculation for silence, an impulse and an alternating
input with seeded history, using synthetic memory and binary32 arithmetic.
The second child takes that output block and rewrites it in place. The third
child reads the two adjacent blocks and writes its mix into the original
input block. That last relationship comes from the decoded reads and stores.

The next runtime evidence needed is a route-labelled interrupt-entry
snapshot for Live 3.01, or matching code that reconstructs it. It must
identify the interrupt source and buffer phase, the enabled callback rows
and order, and the producer of the first child's input block for the
selected analog, Variax or USB route. Matching input, state and output
vectors are needed to check DSP arithmetic and audio equivalence.

#### Room second child

The stock loader clears the second child’s 4,096-word delay allocation. The
initializer saves the ring base and cursor in the child’s working state. The
child advances and saves the cursor once per 16-sample block. Four passes
then gather delay values into 16 words of shared scratch, rewrite the sample
block and store updates at tap-relative ring positions. These passes advance
a temporary pointer. Other decoded paths use the same scratch base.

An instruction replay and an independent binary32 block recurrence agreed
bit for bit in 63 synthetic comparisons. Each compared the 16 output words,
the entire ring, the saved cursor and the final scratch span. Cases cover
silence, consecutive blocks fed by the first child, seeded delay, cursor
wrap and ring-write wrap. Clearing a revisited ring span changed the later
output. Two address interpretations for the final lookahead produced the
same samples and saved state because the lookahead value is discarded. The
physical read address, DSP precision and interrupt timing remain unverified.

The comparison bounds the stock 16-sample entry. The alternate entry and
the third child's arithmetic still need replay. Route-labelled interrupt
state and matching input, state and output vectors are still needed for
whole-graph verification and audio equivalence.

### PODxt Live 3.01 factory channels

The archived main-flash image contains the 128 factory channel tones in
160-byte records. Line 6 Edit 3.06's `L6SharcTonePatchManager.TranslateAddr`
maps channel indices 0–63 to region-17 records 0–63 and channel indices 64–127
to records 192–255. The address mapping excludes the amp and effect setup
records. The original `L6SharcToneMIDITranscoder.DecodePatch` decoded the
channel records in memory, using the PODxt Live map with a null MIDI manager.
The original IFF writer then produced a local `FORM/L6BA` bundle arranged as
32 numbered banks with four tones each.

The Rust POD bundle parser reopened all 128 tones. Each name, target, version,
and all 512 Amp, Stomp, Mod, and Delay model IDs and enabled states matched the
offline decode. The bundle is a host-format reconstruction of the archived
3.01 defaults. The numbered bank labels were added during export. Earlier
factory revisions, current device presets, and live restore remain unverified.
The source firmware and derived tone files remain local and ignored.

## Reproduction and retained evidence

```sh
python scripts/test_firmware_map.py
python scripts/test_podxt_entry.py
python scripts/test_podxt_commands.py
python scripts/inspect_firmware.py --self-test /ABS/VARIAX.vxf /ABS/POD.xtf
python scripts/map_firmware.py /ABS/VARIAX.vxf /ABS/POD.xtf
python scripts/test_podxt_store.py
python scripts/test_podxt_boot.py
python scripts/test_podxt_coverage.py
python scripts/map_podxt_entry.py /ABS/POD.xtf --check research/podxt-301-entry.json
python scripts/map_podxt_commands.py /ABS/POD.xtf --check research/podxt-301-commands.json
python scripts/map_podxt_store.py /ABS/POD.xtf --check research/podxt-301-store.json
python scripts/map_podxt_boot.py /ABS/POD.xtf --check research/podxt-301-boot.json
python scripts/map_podxt_coverage.py /ABS/POD.xtf --check research/podxt-301-coverage.json
```

Ten synthetic layout tests cover every truncated prefix of a two-space loader,
invalid tags, destination overflow/overlap, unexpected extra data and a retained
unknown tail; POD record widths, exact load-record consumption, additive checksums,
word-pointer directory bounds, invalid names and padding. The template/slot test
distinguishes prefix matches from complete-slot equality. No vendor bytes are
embedded in those tests. The mapper reads the
outer envelope using the existing strict inspector and writes only derived
metadata to content-addressed private reports. It does not execute its inputs.

Fresh-worktree extraction exposed a missing report directory in the older
inspector. Extraction succeeded, but its final aggregate write failed. The helper
now creates the report directory and saves a content-addressed report through
the existing non-overwriting save function. Old fixed-name reports are retained.

Initial source-bound reports in the issue worktree are
`firmware-map-ef64014c86d3f7db.json` and `firmware-map-49039640c18c4aa8.json`.
The initial Variax report's architecture-status wording was subsequently narrowed:
the structural parser itself does not validate instructions. Separate private
decoder logs remain supporting evidence, with no claim of physical behavior.

The current content-addressed reports are `firmware-map-38e608f90ff9d73c.json`
(Variax) and `firmware-map-684b874c98efd94d.json` (POD). SHA-256 binds every
region and candidate payload to its source. The synthetic layout tests and the
396-case outer-container rejection test also run in the three-platform core CI.
