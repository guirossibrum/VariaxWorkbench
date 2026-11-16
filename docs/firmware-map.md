# Static firmware layout research

Issue #13 is in progress. All findings below come from archived files; no device
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
Other regions include high-entropy data and recognizable preset/model text.
Their executable ISA, internal packaging and code/data boundaries have not yet
been established. High entropy is not proof of encryption. No USB-component
firmware or installed component version is identified by this map.

## Reproduction and retained evidence

```sh
python scripts/test_firmware_map.py
python scripts/inspect_firmware.py --self-test /ABS/VARIAX.vxf /ABS/POD.xtf
python scripts/map_firmware.py /ABS/VARIAX.vxf /ABS/POD.xtf
```

Three synthetic layout tests cover every truncated prefix of a two-space loader,
invalid tags, destination overflow/overlap, unexpected extra data and a retained
unknown tail. No vendor bytes are embedded in those tests. The mapper reads the
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
