# Archived Variax 3.10 factory reference

This is a file-only extraction of the exact Variax 300/600 3.10 archive recorded
in [the firmware map](firmware-map.md). It is not the owner's current guitar,
an earlier shipping-state reference, a calibration backup or a restoration
procedure. No device was contacted. All live-access locks remain enabled.

## Complete slot boundaries

The 50 records of 2,619 bytes are compact prefixes. Independently examining the
60 records of 4,096 bytes shows a complete 3,276-byte program followed by 820 FF
bytes in every slot. Reverse bytes within each three-byte word to obtain the
host's canonical big-endian word/metadata representation.

All 60 final words match the sum of the first 1,089 canonical words modulo
2^24, consistent with original Workbench Java `GeneratePatch`. Summing 1,091
words also matches these particular files because words 1,089 and 1,090 are zero
in every archived slot. The archive alone cannot distinguish those coverage
rules; the original code and an independent synthetic nonzero-reserved-word
test establish the implemented 1,089-word interpretation.

Fifty-four slot programs also equal a compact template plus zero extension and
the validated checksum. All 50 templates occur at slots 5–54. The six other
programs differ from template expansion; the final five are named Open G,
Open C, Open D, Open A and Drop D. These are archive-derived names and settings,
not an audio validation of their tuning or a reset recommendation.

This resolves the earlier 2,619-versus-3,276 size question at the **record
boundary** level. It does not establish that a newly generated coefficient
program sounds identical to a factory program or is safe to upload.

## Rust extractor and private outputs

`factory_reference::FactoryReference::parse` first uses the bounded FORM/L6FF
inspector, checks declared product/version, then requires this exact source
SHA-256:
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`.
It checks the four region shapes, names, slot padding and every program checksum.
A correct product/version declaration cannot substitute for that fingerprint.
The fingerprint identifies archived bytes; it is not publisher authentication.

```sh
cargo run --bin variax-firmware-inspect -- \
  --variax-reference /EXISTING/PARENT/NEW_REFERENCE_DIRECTORY /ABS/Variax300_3.10.vxf
```

The destination must not exist. The extractor saves 50 original compact prefixes,
60 original complete 4,096-byte slots, and 60 decoded `.vwm` model/settings
files. It writes `manifest.tsv` last, with source/image hashes, original file
offsets, raw and derived file sizes and SHA-256 hashes. The manifest marks a
complete extraction. A failure leaves partial evidence in the new directory;
retry into another new directory. No existing file or directory is replaced.
Do not publish these private firmware-derived outputs.

The `.vwm` files can be inspected and opened by the offline editor. They retain
decoded model/settings information; the separate raw slot files retain the
original coefficient program and padding. Neither represents a complete current
unit backup. File offsets are not approved physical flash destinations.

## Independent Java validation

`scripts/DecodeFirmwareReference.java` allocates the original metadata transcoder
without its device constructor, presents only slot metadata to pure `DecodePatch`,
and saves settings to a new directory. Its nibble array is a local parser input;
it is never sent or received through MIDI. No JNI coefficient library, device
constructor, UI or firmware is executed.

Compile/run with the previously extracted original Java classes and a local JDK:

```sh
javac -cp /ABS/EXTRACTED_MAC_JAR -d /ABS/NEW_CLASSES scripts/DecodeFirmwareReference.java
java -Djava.awt.headless=true -cp /ABS/NEW_CLASSES:/ABS/EXTRACTED_MAC_JAR \
  DecodeFirmwareReference /ABS/REFERENCE_DIRECTORY /ABS/NEW_JAVA_OUTPUT_DIRECTORY
```

Use `;` instead of `:` for the Java classpath separator on Windows. These are
research-only tools, not application runtime dependencies. All 60 original-Java
outputs agreed with the Rust outputs in every summarized model, parameter type
and raw bit. The private source-bound report is
`.tools/issue-14-java-agreement-8cfdad809b0f22e6.json`. The extraction and original
Java outputs are retained separately under `.tools/issue-14-*` in the main
checkout. They are not default test fixtures.

Five synthetic Rust layout/export tests cover full-byte preservation, metadata,
checksum coverage, correct declarations with a different fingerprint, rewrapped
corruption, unexpected layouts, prefix/expanded-program differences, and refusal
to replace existing directories. Two CLI tests exercise the fingerprint gate
before any export directory is created and reject a missing directory argument.

## Known global string-level structure; unit calibration remains separate

The original Java `L6VariaxDevice.GetStringVolumes` reads six signed 24-bit
fractional values in reversed string order. `VXFrac2Float` divides signed values
by 8,388,607; alternating signs and a remembered inversion flag affect the UI
interpretation. `SetStringVolumes` changes those global levels, while
`WriteStringVolumes` is a separate persistence action. These are static code
observations, not operations implemented or authorized here.

The exact firmware independently identifies the same six-value structure:

| Static edge | File-derived observation |
| --- | --- |
| `P:0x14316..0x14325` | Reads a command, subtracts 0x10, indexes the program-word table at 0x14326 and jumps through the result |
| Command 0x54, table word 0x1436A | Target 0x144E3 parses six words into circular `Y:0x2D8..0x2DD` |
| Command 0x56, table word 0x1436C | Target 0x14583 checks the terminator and calls 0x14142 |
| Command 0x57, table word 0x1436D | Target 0x1458E checks the terminator and emits a header, 0x57 and six words from the same buffer as low-to-high nibbles |
| `P:0x14142` | Supplies argument 4 to 0xF63, then sends six words from Y:0x2D8 to bus address 0xD04000 through 0xF17 |
| `P:0x14150` | Reads six groups of three low-byte values from that bus address into Y:0x2D8 |
| `P:0x14101..0x1410B` | A conditional initialization branch copies six fallback values from Y:0x14D01 to Y:0x2D8 and calls 0x14142 |

The called helpers contain erase/program-like command sequences and polling
loops. These are static branch observations, not executed operations. The bus
address **must not be equated to file offset 0x4000 or any approved physical
flash location**. The installed memory part, actual runtime branch conditions,
unit contents, complete erase effects, power-loss behavior and update-preservation
rules are not established by this trace.

The fallback comes from load record 31, raw memory tag 2, at image byte 31,804.
Its six values alternate positive maximum and signed minimum. It is a generic
initialization table, **not the owner's calibration**. Its raw 18 bytes remain
private, SHA-256
`08adb3ccdda44cda3e17b1bfca8cc40416cdcd2c67ca106ea61869737b5ab9b6`.
The source-bound trace receipt is the private
`analysis/reports/calibration-static-cb05ee0045954259.json` in the issue worktree.
It records the three independently read dispatch-table entries, file offsets,
source hash and corrected disassembly digest. Program loads were combined into
five contiguous ranges before decoding so instruction operands can cross load
record boundaries. Explicit hexadecimal starting PCs avoid the decoder CLI's
hexadecimal interpretation of bare address strings. Earlier exploratory logs
with decimal PC strings or artificial record boundaries are retained as rejected
address/flow evidence; the contiguous-range log is the cited trace.

These global levels are not per-preset virtual pickup levels. They are not
proven to cover every device-unique calibration value. The extractor intentionally
exports no substitute global-level table and does not mix it into factory
presets. Earlier shipping defaults, the owner's real values, other possible
calibration fields and preservation/recovery behavior remain unknown. The known
structure and factory reference satisfy the offline extraction scope; full-unit
backup and update/readiness gates remain on #16/#22–#25. No factory extraction
invents, zeroes, normalizes, merges or writes unit calibration.
