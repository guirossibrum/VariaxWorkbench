# Offline FORM/L6FF firmware inspection

The Rust inspector reads a FORM/L6FF container that is already on disk and
reports structure, checksums, region coverage, and product applicability.
`variax-firmware-inspect` and `variax_workbench::firmware` do not install
firmware, open USB, MIDI, or audio devices, or reset anything. Passing these
checks does not make an image safe to flash. Flashing is not implemented.

The reference parser is `scripts/inspect_firmware.py`. Both follow the same
container rules below. The Rust parser adds the applicability gate, the
container-size limit, and rejection of zero-length regions.

## Container checks

A container is a big-endian IFF file:

| Field | Rule |
| --- | --- |
| Root | `FORM` size must equal the file length minus 8, and the type must be `L6FF`. |
| Chunks | Each chunk is a 4-byte tag, a big-endian size, that many payload bytes, and one pad byte when the payload length is odd. The pad is not part of the payload. |
| Allowed tags | Exactly one `HEAD`, exactly one `data`, and one or more `dinf` chunks. Any other tag is rejected. |
| `HEAD` | 52 bytes: format version, version, product id, compression, image length, 16-byte MD5, 16 reserved bytes. |
| Format | Version word `0x00010000` and compression code `1` (zlib). |
| Image length | Greater than zero and at most 16 MiB. The whole container must also fit in 16 MiB plus 64 KiB. |
| `data` | zlib data. Decompression allocates at most one byte more than the declared length. The stream must finish, produce exactly the declared length, and leave no unused bytes. |
| MD5 | The digest of the uncompressed image must equal the digest in `HEAD`. |
| `dinf` | 16 bytes: raw type, version, offset, size. Regions must be non-empty, start at offset zero, and cover the image without gaps or overlap. |

The 16 bytes after the MD5 are preserved as `reserved_tail`. Archived PODxt Live
3.01 and Variax 300/600 3.10 files store zeros there. Their meaning is unknown,
so a non-zero tail is not treated as a different format.

`dinf` type values are reported as `type_raw`. This inspector does not assign
meanings, instruction sets, or load addresses to those numbers.

## Integrity is not authenticity

`header_md5_matches` means the uncompressed bytes match the digest stored beside
them. Anyone who can edit the image can recompute that digest. MD5 agreement is
not a signature, not a publisher check, and not proof that a file is the
archived download. The report field `integrity_is_not_authentication` is always
true on success so the limit stays attached to the result.

`source_sha256` and `image_sha256` are fingerprints of the bytes just inspected.
They are not an allowlist.

## Product applicability

Structural inspection succeeds for any product id and version that meet the
container rules. Applicability is a separate gate:

| Declaration | Classification | `require_target` / `--expect-product` plus `--expect-version` |
| --- | --- | --- |
| Product `0x0003000a`, version `0x03010000` | Recorded declaration for PODxt Live | Passes only that exact pair |
| Product `0x00070004`, version `0x03100000` | Recorded declaration for Variax 300/600 | Passes only that exact pair |
| Product `0x00070004`, any other version | Wrong version for Variax 300/600 | Fails the archived 3.10 target |
| Product `0x00070000` | Recognized Variax 500/700 identity, no archived image | Fails a 300/600 or PODxt target |
| Any other product id | Unknown product | Fails a specific target |

The version label `3.01` or `3.10` is only a display of the top two bytes when
the low 16 bits are clear. It matches the two archived HEAD words
(`0x03010000` and `0x03100000`). It is not a claim that every future version
uses that layout.

A recorded declaration means the header fields match the catalog. A synthetic
container can declare the Variax 300/600 3.10 identity without being the
archived file. The catalog does not compare file bytes.

Sources for the catalog:

- PODxt Live `0x0003000a` / `0x03010000` comes from the HEAD of the archived
  `PODxtLive_3_01.xtf` container. It is not USB PID `0x4650`.
- Variax 300/600 `0x00070004` / `0x03100000` comes from the HEAD of the archived
  `Variax300_3.10.vxf` container. Line 6 Central's map uses decimal 458756 for
  the same product id.
- Variax 500/700 `0x00070000` comes from Central's map, decimal 458752. No
  firmware container for that id is archived here, so it is not an applicable
  image target.

Those archived filenames stay local and are not committed. Default tests use
synthetic containers only.

## What this does not establish

Region types, CPU or DSP instruction sets, bootloader behavior, factory
presets, and update sequencing are out of scope. A successful inspection is not
permission to flash, and it is not evidence that a device is running the image.
The separate POD USB-firmware component is not this container; see issue #20.

## Checks

```sh
cargo test --all-features firmware
cargo run --bin variax-firmware-inspect -- --self-test
cargo clippy --all-features --all-targets -- -D warnings
```

The synthetic suite rejects 390 prefixes and six damaged fields of the same
390-byte layout used by `scripts/inspect_firmware.py --self-test`, plus
wrong-product, wrong-version, oversized, overlapping, duplicate-chunk, and
corrupted-image fixtures. The Python self-test's 396 rejections are the 390
prefixes and six damaged fields. Rust counts those 396 and the additional
classes separately.

Optional local confirmation, only when the ignored installer files are present:

```sh
cargo run --bin variax-firmware-inspect -- --json \
  vendor/installers/Variax300_3.10.vxf
cargo run --bin variax-firmware-inspect -- --json \
  vendor/installers/PODxtLive_3_01.xtf
python scripts/inspect_firmware.py --self-test
```

That command reads local files. It does not contact a device. Do not add a
runtime switch that weakens `LIVE_ACCESS_LOCKED`.

## Decisions left for review

- Unknown product ids remain parseable. They fail only when the caller supplies
  an expected product and version.
- Variax 500/700 is recognized from Central's map and is not an archived image.
- Containers larger than 16 MiB plus 64 KiB, and zero-length regions, are
  rejected. The Python inspector does not apply those two limits.
- `reserved_tail` stays opaque. Non-zero tails are accepted.
