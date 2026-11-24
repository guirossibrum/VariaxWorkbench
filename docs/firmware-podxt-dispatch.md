# PODxt Live 3.01 firmware dispatch

This is the PODxt half of [#181](https://github.com/guirossibrum/VariaxWorkbench/issues/181),
tracked as [#211](https://github.com/guirossibrum/VariaxWorkbench/issues/211).
The [entry inventory](../research/podxt-301-entry.json) is the 256-word table
and the region-5 measurements. Findings are static. The archived image and the
disassembly listing stay private. Firmware was not executed, and no device was
opened.

Instruction text comes from the pinned MAME SHARC disassembler recorded in the
[firmware map](firmware-map.md). The tool is a private research build. It is
not an application dependency.

Labels mean: **confirmed** (bytes or disassembly at the address given),
**likely** (a reading that fits those bytes and is not proven), **unknown**
(not established). Handler arguments, the store, boot checks, and the trap
list are later sections of this file.

## Which program sees the bytes

**Confirmed.** MIDI-shaped SysEx in this image is the region-4 program, not
region 5. The loaded 48-bit words occupy `0x8000` through `0x96B7` (5,816
words). The vector block `0x8000–0x807F` is NOP or RTI except:

| Words | Instruction | Role established here |
| --- | --- | --- |
| `0x8004–0x8005` | `IDLE`, then `JUMP (0x0000823C)` | Reset. Already recorded in the firmware map. |
| `0x8010` | `JUMP (0x00009348)` | Compares two shifted data words and can clear IMASK bit `0x10`. It does not read the byte pipe. |
| `0x8020` | `JUMP (0x000080CF)` | Byte receive. The only jump to `0x80CF`. |
| `0x8028` | `JUMP (0x00009105)` | Reads `DM(0x60)`, toggles bit 5 of USTAT1, and walks or clears a buffer. It does not read the byte pipe. Role unknown. |

**Confirmed.** Receive at `0x80CF` loads `R0` from `DM(0x00200000)`, stores
that word in the ring, and returns from interrupt. Transmit at `0x80D6–0x80DB`
waits while `FLAG1_IN` is clear, then stores `R0` to the same address. Those
are the only two decoded references to `0x00200000`. The address is absolute
external memory. Which pin or peripheral decodes it is **unknown**. The
[schematic reference](rig-schematics.md) shows the ADSP-21065L handshaking with
a controller and shows separate MIDI circuitry. This image does not say which
of those paths feeds `0x00200000`.

**Confirmed.** The parser at `0x80B2` is called from the reset main loop at
`0x8271` and from nowhere else in the decoded program. It returns immediately
when the ring read pointer `DM(0xC1F0)` equals the write pointer `DM(0xC1F1)`.
Otherwise it sets `B4 = 0x1A900` and `L4 = 0x100`, reads one ring word, keeps
the low eight bits, and advances the read pointer.

- A byte with bit 7 clear is consumed and the parser returns.
- `F0` stores the already-advanced pointer in `DM(0xC1F2)` and returns. That
  pointer addresses the first byte after `F0`.
- Any other byte with bit 7 set, including `B0` and `C0`, is consumed and the
  parser returns.
- `F7` reloads `I4` from `DM(0xC1F2)`, reads the first byte after `F0` into
  `M0`, loads `I15` from the 16-bit word at `0x1A9C1 + that byte`, and executes
  `JUMP (M15, I15)`. There is no delay slot and no range check.

The only decoded writes of `M15` are `M15 = 0` at `0x824F` and `0x8259`. The
only decoded writes of `L0` are `L0 = 0` at `0x823D`, `0x8254`, `0x8A08`, and
`0x9123`. The dispatcher does not write either register.

A second `F0` before `F7` replaces `DM(0xC1F2)`. The first message is then not
dispatched. With no `F7`, the parser never takes the indirect jump. **Confirmed.**

Reset at `0x823C` clears the length registers, sets the same ring base in `B6`
with length `0x100`, and calls `0x9380`, `0x8080`, and `0x9609` before enabling
the main loop. `0x8080` writes `0x100000` zero words starting at `DM(0x02000000)`.
**Confirmed** as instructions. What that address is on the board is **unknown**.
`0x9380` compares shifted reads from `0x01078000`, `0x01070000`, and
`0x01077FFF` with `0xFFFF0000` and `0x19690000`. Its role is **unknown** here.

The transmit loop at `0x80D9` has no counted timeout. **Confirmed.** Elapsed
time is **unknown**.

CC status and program-change status are in the group this parser consumes
without a table jump. The decoded program contains no immediate `0x45`. That
does not search every data table for the tuner. **Confirmed** for the parser
and the immediate; the rest of the tuner question stays open for the handler
pass.

## Dispatch table

**Confirmed.** The index byte selects a big-endian 16-bit word in the width-1
load record whose destination is `0x1A900`. Index 0 is word `0x1A9C1`. All 256
values are in [the inventory](../research/podxt-301-entry.json). `0x80CE`
disassembles to `RTS`, so a table word equal to `0x80CE` returns to the caller
of `0x80B2`, which is the main loop. Class counts:

| Class | Count | Meaning |
| --- | --- | --- |
| zero | 100 | Word is `0x0000`. With `M15` still 0 the jump target is address 0, which this load image does not contain. |
| return | 26 | Word is `0x80CE` (`RTS`). |
| program | 44 | Word is inside `0x8000–0x96B7` and is not `0x80CE`. |
| outside program | 86 | Word is neither zero nor inside the loaded program. |

Opcodes `0x00–0x3F` are all zero. Opcodes `0x40–0x76` are all either `0x80CE`
or a loaded-program address. Opcodes `0x77–0x7F` are outside the loaded
program. The return opcodes are `0x40–0x51`, `0x58`, `0x5A`, `0x5B`, `0x62`,
`0x64`, `0x6A`, `0x6B`, and `0x70`.

Loaded entries at and below `0x76`:

| Opcode | Entry |
| --- | --- |
| `0x52` | `0x961A` |
| `0x53` | `0x9630` |
| `0x54` | `0x8104` |
| `0x55` | `0x811A` |
| `0x56` | `0x8124` |
| `0x57` | `0x812D` |
| `0x59` | `0x814C` |
| `0x5C` | `0x8137` |
| `0x5D` | `0x813C` |
| `0x5E` | `0x9600` |
| `0x5F` | `0x8159` |
| `0x60` | `0x9365` |
| `0x61` | `0x9499` |
| `0x63` | `0x9650` |
| `0x65` | `0x94A6` |
| `0x66` | `0x94A9` |
| `0x67` | `0x9431` |
| `0x68` | `0x8161` |
| `0x69` | `0x9638` |
| `0x6C` | `0x81A6` |
| `0x6D` | `0x80FB` |
| `0x6E` | `0x80F9` |
| `0x6F` | `0x8195` |
| `0x71` | `0x9420` |
| `0x72` | `0x941D` |
| `0x73` | `0x8166` |
| `0x74` | `0x818F` |
| `0x75` | `0x8187` |
| `0x76` | `0x816B` |

**Confirmed** reply bytes at the entry, before the later handler pass:

| Entry | What the entry transmits or consumes |
| --- | --- |
| `0x812D` (`0x57`) | `F0`, `0x56`, one index byte, four nibbles, `F7`. |
| `0x814C` (`0x59`) | `F0`, `0x58`, then 32 bytes. |
| `0x813C` (`0x5D`) | `F0`, `0x5C`, then either 32 bytes or a branch into the 32-byte sender at `0x8155`. |
| `0x8159` (`0x5F`) | `F0`, `0x5E`, the current ring byte, then `R0` from `0x95F4` (that routine returns 0 or 1), then `F7`. |
| `0x8187` (`0x75`) | `F0`, `0x74`, 160 bytes from `0x1B9D8`, `F7` (163 bytes). |
| `0x818F` (`0x74`) | Reads 160 ring bytes into `0x1B9D8` with no `F7` test in that loop, then jumps to `0x94BA`. |
| `0x8161` (`0x68`) | Calls `0x93F2`, then sends `F0`, `0x51`, and a byte that is 3 or 0. |
| `0x9365` (`0x60`) and `0x81A6` (`0x6C`) | Reach `0x80DC`, which sends `F0`, `0x50`, `F7`. |

The host editor's SysEx frames are `F0 00 01 0C 03 <opcode> … F7`. A search
of the whole image finds zero occurrences of `F0 00 01 0C`. The SHARC send
and receive path above emits `F0`, then one opcode byte, then payload, then
`F7`. **Likely:** another stage removes `00 01 0C 03` before this parser and
inserts it on the way out. The reason for that reading is that the populated
table slots are the host opcodes (`0x57`, `0x71`, `0x73`, `0x74`, `0x75`) and
several of those entries transmit the reply opcode the host already pairs
with the request. If the four manufacturer and family bytes were still
present, the index would be `0x00` and the target would be address 0.
**Unknown:** which component does that, and why the host's 168-byte edit
buffer is five bytes longer than the 163-byte frame at `0x8187`. The
[USB component note](pod-usb-firmware.md) still has no USB-firmware image.
This main-flash image is not that artifact. **Confirmed.**

Nine occurrences of the three bytes `00 01 0C` are at file offsets `0x13CD2`,
`0x13D20`, `0x152C2`, `0x16270`, `0x180D6`, `0x183DC`, `0x18FC4`, `0x1BC58`,
and `0xE13D7`. Whether each one is a manufacturer id is **unknown**.

Opcodes above `0x76` are still indexed by the same instruction. Fifteen of
them land in the loaded program: `0xB5`, `0xB6`, `0xC4`, `0xC5`, `0xD3`,
`0xD4`, `0xE2`, `0xE3`, `0xF1`, and `0xF2` target `0x81FA` (`RTS`); `0xBF`,
`0xCE`, `0xDD`, and `0xFB` target `0x8020` (the receive vector, which ends in
`RTI`); `0xEC` targets `0x81CC`, which is the middle of a store sequence.
The other high slots are zero or outside the loaded program, including
`0x77–0x7F` at `0xAA76`, `0xAA85`, `0xAA94`, `0xAAA3`, `0xAAB2`, `0xAAC1`,
`0xAAD0`, `0xAADF`, and `0xAC1A`. Where an outside target would execute is
**unknown**.

### Bytes the editor uses

The table match below is **confirmed**. Treating the host opcode as the
SHARC index is the **likely** header-stripping reading above, not a captured
trace of `DM(0x00200000)`.

| Editor message | Table index if that byte is the opcode | Recorded word |
| --- | --- | --- |
| Edit-buffer request `03 75` | `0x75` | `0x8187` program |
| Edit-buffer send `03 74` | `0x74` | `0x818F` program |
| Stored read `03 73` | `0x73` | `0x8166` program |
| Store `03 71` | `0x71` | `0x9420` program |
| Globals `03 57` | `0x57` | `0x812D` program |
| Packs `03 0E` | `0x0E` | `0x0000` |
| Linked model `03 10` | `0x10` | `0x0000` |
| `03 11` | `0x11` | `0x0000` |
| Variax release `03 23` | `0x23` | `0x0000` |
| `03 0F` | `0x0F` | `0x0000` |
| `03 24` | `0x24` | `0x0000` |
| Value `0x40` (CC 64's controller number) | `0x40` | `0x80CE` return |
| Value `0x45` (CC 69's controller number) | `0x45` | `0x80CE` return |
| Value `0x13` (CC 19's controller number) | `0x13` | `0x0000` |

`0x0E` immediates at `0x9048` and `0x9087` are arguments to `CALL 0x8638`,
not a second status compare. `0x24` at `0x8803` is a length. They are not a
pack or save parser. Argument layout for the program entries starts in the
next section.

## Region 5

**Confirmed measurements, unknown identity.** Region 5 at file `0x20000` has
the same length-and-checksum wrapper as region 4: 32,764 big-endian 16-bit
words, checksum accepted by `pod_word_section`, and no `FF` padding (65,528
payload bytes). `pod_load_records` rejects that payload with `unsupported POD
word width or empty record`, so it is not a second region-4 load image.

| Measurement | Result |
| --- | --- |
| Shannon entropy | 7.9156 bits/byte, all 256 byte values present |
| zlib level 9 | 64,335 bytes (does not compress away) |
| zlib/gzip inflate | 0 successes in 11 attempts: offsets 0, 2, 4, and 8, the one `78 DA` at payload offset 5466, and the two `1F 8B` at 13493 and 51269 |
| 16-bit words | 23,203 distinct of 32,764; entropy 14.298 bits; most common word `0xF0A3` occurs 39 times |
| Aligned 16-byte blocks | 4,094 distinct of 4,095; the repeated block occurs twice |
| Printable ASCII runs of 5 or more | 222 runs; 15 are at least 8; the longest is 12. None contains `midi`, `usb`, `sharc`, `pod`, `line`, `variax`, `boot`, or `flash` |

**Likely.** The payload is not coherent SHARC code at the alignments that were
disassembled. The same marker rule that marks 28 of the 5,816 region-4 program
lines (a line whose disassembly text contains `?` or `TODO`) marks 5,371 of
10,921 words on the first 65,526 payload bytes. The first 2,000 words at byte
shifts 0 through 5 mark 958, 887, 1,021, 918, 958, and 953 lines. Two payload
bytes remain after the aligned 48-bit split.

**Unknown.** The measurements do not identify an ISA, a compression scheme, or
encryption. High entropy is not evidence of encryption. No 8051, Thumb, or
XOR sweep was run. No separate USB-controller executable was identified in
this image.

## Device-facing notes from this slice

Nothing in this work was sent to a device.

| Trigger | Evidence | Consequence established here |
| --- | --- | --- |
| A SysEx opcode whose table word is 0, including `0x00–0x3F` and the editor opcodes `0x0E`, `0x10`, `0x11`, `0x23`, `0x0F`, and `0x24` if they arrive as the first byte after `F0`. | `JUMP (M15, I15)` has no range check. Those words are `0x0000`. | Target address 0 if `M15` is still 0. Address 0 is not in this load image. Further effect **unknown**. |
| A SysEx opcode whose table word is outside `0x8000–0x96B7` (86 opcodes, including `0x77–0x7F`). | The inventory lists the words. | Where execution would continue is **unknown**. |
| Opcodes `0xBF`, `0xCE`, `0xDD`, and `0xFB`. | The word is `0x8020`, and `0x8020` jumps to the receive routine that ends in `RTI`. | Entering that routine from the SysEx jump is **unknown** in effect. |
| Transmit with `FLAG1_IN` held clear. | `0x80D9` jumps to itself. | The loop has no timeout in the instruction. |
| Opcode `0x74` with fewer than 160 bytes after the opcode. | `0x818F` reads 160 ring words and does not test `F7`. | The read can take bytes already in the ring. Whether those bytes are a previous message is **unknown** until the handler pass. |
