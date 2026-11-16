# Static firmware layout research

Issue #13 records a static baseline map with the unknowns listed below. All
findings come from archived files; no device
was contacted and no firmware was executed or installed. Source firmware and
disassembly remain private. Image offsets are file offsets, and candidate
loader destinations are word addresses, not approved flash offsets.

## Variax 300/600 archive 3.10

Source SHA-256:
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`.
The existing L6FF inspector verifies product `00070004`, version `03100000`,
the 442,402-byte decompressed image and its embedded MD5. That does not establish
publisher authenticity. The outer region tags remain numeric.

| Region | Raw tag | Bytes | Observed structure |
| --- | --- | --- | --- |
| 0 | 5 | 16,384 | A load-record-like prefix; the remainder is not classified |
| 1 | 0 | 49,308 | 117 three-u24-header load records and an exact terminator |
| 2 | 2 | 130,950 | 50 contiguous records of 2,619 bytes |
| 3 | 1 | 245,760 | 60 slots of 4,096 bytes with a 2,619-byte program prefix |

The main-region candidate header is little-endian 24-bit count, destination and
memory tag, followed by `count` words. A zero count ends with one destination
word. Tags 0/1/2 appear 88/7/22 times, with no overlapping destinations within
a tag. The exact terminator's nominal entry is word `0xDBC`. Destination zero
contains a DSP56300-compatible absolute jump to that entry. The entry initializes
registers and jumps to a separately mapped block at `0x14004`.

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
records must not be treated as interchangeable upload artifacts.

The partial prefix view of region 0 consumes only 168 bytes; the rest remains
unclassified. Its apparent header must not be assumed to be the chip ROM's boot
protocol. Record roles, slot tails, calibration/preservation behavior, checksums
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
| Region 3, 0x8000, 32,768 bytes | Repeated bit patterns consistent with display data | Not classified as executable; exact format unverified |
| Region 4, 0x10000 | Length/checksum wrapper and four complete load records | Loader implementation and hardware behavior unverified |
| Region 5, 0x20000 | Length/checksum wrapper, 65,528-byte opaque payload | Payload ISA/encoding unverified; high entropy is not proof of encryption |
| Regions 6–11, 0x30000–0x90000 | Resource directory, named records and padding | Record bodies, licensing and execution semantics unverified |
| Region 17, 0xE0000 | Preset-name-bearing data | Editing/checksum/preservation rules unverified |
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
correctly. No separate 8051 or USB-controller executable is identified here.

Independent static decoding used Ville Linde's BSD-3-Clause MAME SHARC
disassembler at tag `mame0285`, commit
`f379cd060ac3b2bd567ebbd0957aed059d6895ff`, files
`src/devices/cpu/sharc/sharcdsm.cpp` and `sharcdsm.h`. A local C++17 compatibility
shim supplies formatting, the disassembler interface and a big-endian six-byte
reader; no MAME CPU/device implementation is built or executed. This is a private
research tool, not an application dependency. The source pin, shim and private
decoder logs are retained in the issue worktree's ignored `.tools/sharc-research`
and `analysis/reports` directories.

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

## Reproduction and retained evidence

```sh
python scripts/test_firmware_map.py
python scripts/inspect_firmware.py --self-test /ABS/VARIAX.vxf /ABS/POD.xtf
python scripts/map_firmware.py /ABS/VARIAX.vxf /ABS/POD.xtf
```

Seven synthetic layout tests cover every truncated prefix of a two-space loader,
invalid tags, destination overflow/overlap, unexpected extra data and a retained
unknown tail; POD record widths, exact load-record consumption, additive checksums,
word-pointer directory bounds, invalid names and padding. No vendor bytes are embedded in those tests. The mapper reads the
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
