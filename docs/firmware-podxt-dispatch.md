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
(not established). The trap list is a later section of this file.

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
`0x9380` selects which stored half is active. The store section records
the four paths. The boot section records the checksum and the two spins
on the reset path.

The transmit loop at `0x80D9` has no counted timeout. **Confirmed.** Elapsed
time is **unknown**.

CC status and program-change status are in the group this parser consumes
without a table jump. The decoded program contains no immediate `0x45` and
no register load of `0x13`. The only store to `DM(0x00200000)` is `0x80DA`.
Every call or jump to the transmit helpers `0x80D6`, `0x80D8`, and `0x80D9`
is inside `0x80D6–0x81A5`. The opcode byte placed between `F0` and the
payload is one of `0x50`, `0x51`, `0x54`, `0x56`, `0x58`, `0x5C`, `0x5E`,
`0x6D`, `0x71`, `0x72`, or `0x74`. **Confirmed.** This program does not
write CC 69, CC 64, CC 19, program change, or SysEx opcodes `0x0F` and
`0x24` to that address. Whether another component emits them is **unknown**.

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

## Command handlers

CC status and program change still have no handler. Every byte from
`DM(0x00200000)` enters the ring, and the parser returns without a table jump
for every status other than `F0` and `F7`. **Confirmed.** A front end that
turned a CC or a program change into a SysEx opcode before the pipe is
**unknown**; nothing in this image does that conversion.

Ring reads in the handlers below do not compare the read pointer with the
write pointer and do not test for `F7`. A short or truncated message therefore
supplies whatever bytes are already in the ring. **Confirmed** from the
addressing. Every transmit waits on `FLAG1_IN` at `0x80D9` with no counted
timeout. The flash poll loops below also have no counted timeout.

The state word `DM(0x1B910)` selects a preamble at `0x93DF` through the six
words at `0x1B917`. The archived initial word at `0x1B910` is 0, and slots 0,
1, 2, 4, and 5 of that table are `RTS` at `0x81FA`. Slot 3 is `0x93E3`, which
writes `0xB0` to `DM(0x01000000)` and waits until `DM(0xC2D7)` changes.
**Confirmed.** The live value of the state word after boot is **unknown**.

### Editor opcodes

| Opcode | Entry | Confirmed action | Reply | Flash |
| --- | --- | --- | --- | --- |
| `0x75` | `0x8187` | Sends 160 bytes from `DM(0x1B9D8)`. Reads no argument. | `F0 74` + 160 bytes + `F7` | no |
| `0x74` | `0x818F` | Stores 160 ring bytes into `DM(0x1B9D8)`, clears `DM(0xC2D7)`, jumps to `0x94BA`. | none in the entry | no |
| `0x73` | `0x8166` | Calls the `0x76` body, then sends `F0 72 F7`. | `0x72` after the body | no |
| `0x76` | `0x816B` | Reads two 14-bit pairs. `0x81A8` turns the first into an address (`I0` = pair × `0x50` + `DM(0xC2DF)`) and returns that pair in `R1` (likely; see below). The second pair is the count. Count 0 sends one `F0 74` frame of 160 bytes from that address. A non-zero count sends that many `F0 71` frames, each with the returned pair's high and low 7 bits, then 160 bytes, then adds one to the pair. | `0x74` or repeated `0x71` | no |
| `0x71` | `0x9420` | Runs the state preamble, takes the same 14-bit origin, toggles bit 15, and programs 80 sixteen-bit words from the next 160 ring bytes. | none | program |
| `0x57` | `0x812D` | Echoes the current ring byte and sends the word at `DM(0x1B9A8 + index)` as four nibbles. The index is not masked. | `F0 56` + index + 4 nibbles + `F7` | no |
| `0x56` | `0x8124` | Stores four following nibbles at `DM(0x1B9A8 + index)`. A non-zero high nibble clears `DM(0xC2D7)`. Then jumps through the word at `0x1B978 + index`. | none in the entry | no |
| `0x0E`, `0x0F`, `0x10`, `0x11`, `0x23`, `0x24` | table word 0 | No in-program handler. | none | no |

`0x81A8` does not compare the 14-bit value with a maximum. **Confirmed.**
The `0x76` loop count is a second 14-bit pair and is likewise not compared.
A count of 16383 would transmit 16383 frames. **Confirmed** as the
instruction; the elapsed time is **unknown**.

At `0x81AC` the multiply and the stack store are one instruction:
`R0 = R0 * R1, DM(I7, 0) = R0`. The next instruction reloads that slot into
`R1`. **Likely** the store keeps the 14-bit pair from before the multiply,
because `0x8175` then shifts the returned `R1` by −7 and masks `0x7F`. The
address left in `I0` is the product plus `DM(0xC2DF)`. **Confirmed** as
those instructions.

`0x0F` at `0x8E88` is an argument to `CALL 0x8638`, the same shape as the
`0x0E` arguments at `0x9048` and `0x9087`. `R3 = 0x24` at `0x8803` is
multiplied by the following word. `0x24` at `0x9578` and `0x9583` is passed
to `0x81DF`, and `0x81FB` adds that value to `0x1AA38` and loads the word
there. `R0 = 0x40` at `0x8886` is followed by `RTS`. `R0 = 0xC0` at
`0x93BF` is stored to `DM(0xC2E1)`. None of these is a write to
`DM(0x00200000)`. **Confirmed.**

`0x74` does not call `0x930F` or `0x9324`. Its tail `0x94BA` calls `0x95DE`,
`0x943E`, and `0x95E7`. `0x943E` walks external memory from `0x01018000`.
Whether that walk changes stored sound is **unknown**. It is not the program
or erase sequence below.

The four-nibble reader at `0x80E6` shifts and adds each byte. It does not
mask a byte to four bits, so a value above 15 is still accumulated.
**Confirmed.**

The jump at `0x95A0` indexes `0x1B978` with the raw byte. Of those 256 words,
49 are inside the loaded program, 158 are zero, and 49 are outside it. Indexes
`0x30`, `0x3B`, `0x3C`, and `0x3D` hold `0x0301`, `0x1000`, `0x1000`, and
`0x7FFF`. The other outside words are small integers, including a run of
`0x0020`. **Confirmed.** A jump to any of them leaves the loaded program. The
effect is **unknown**.

### Program and erase sequence

**Confirmed** stores. **Likely** an AMD-style flash command sequence, because
the addresses and the data bytes are the familiar unlock pair. The chip and
the sector size are **unknown**.

| Routine | Stores | What follows |
| --- | --- | --- |
| `0x932E` | `0xAA` at `DM(0x01000555)`, `0x55` at `DM(0x010002AA)` | returns |
| `0x930F` | unlock, then `0xA0` at `DM(0x01000555)`, then one 16-bit word at `DM(0xC2D6)`, then advances that pointer | polls the same word until the read matches; no timeout |
| `0x931C` | unlock, `0x80` at `DM(0x01000555)`, unlock, `0x30` at the word addressed by `DM(0xC2D6)` | returns |
| `0x9324` | calls `0x931C` | polls that word until it reads `0xFFFF`; no timeout |
| `0x9333` | unlock, `0x20` at `DM(0x01000555)`, `0xA0` at `DM(0x01000000)`, one word copied, then `IMASK` bit `0x10` | the `0x8010` vector continues the copy and later writes `0x90` / `0x00` |

`0x71` reaches `0x930F` eighty times and does not call `0x9324`. It also
does not transmit `0x50` or `0x51`. Those two replies exist: `0x80DC` sends
`F0 50 F7`, and `0x80E0` sends `F0 51` plus one data byte plus `F7`. The
jumps to `0x80DC` are at `0x9369` (`0x60`), `0x943D` (`0x67`), `0x81A7`
(`0x6C`), `0x941F` (`0x72`), and `0x941C` (the body at `0x940A`). The jump
to `0x80E0` is at `0x8165` (`0x68`). **Confirmed.**

| Opcode | Entry | Confirmed path | Reply |
| --- | --- | --- | --- |
| `0x60` | `0x9365` | Address from the ring through `0x81A8`; programs 80 words from `DM(0x1B9D8)` through `0x9428`, then `0x936A`. | `F0 50 F7` |
| `0x67` | `0x9431` | Two `0x81A8` results and a third 14-bit pair multiplied by `0x50`. That product is the length passed to `0x9333`, then `0x936A`. | `F0 50 F7` |
| `0x6C` | `0x81A6` | `0x9324` at the current `DM(0xC2D6)`. No argument read. | `F0 50 F7` |
| `0x6D` | `0x80FB` | First byte is a loop count. Each pass assembles three bytes into one word and calls `0x930F`. No maximum check. | none |
| `0x6E` | `0x80F9` | Assembles eight nibbles, adds `0x01000000`, and stores the sum at `DM(0xC2D6)`. | none |
| `0x72` | `0x941D` | `0x936A` only. | `F0 50 F7` |
| `0x68` | `0x8161` | `0x93F2` writes `0x100000` words at `DM(0x02000000)`, reads them back, then the handler sends `0x51` with data byte 0 when the mismatch count stayed 1 and 3 otherwise. | `F0 51` + byte + `F7` |

`0x936A` scans `0x180` addresses and calls `0x9333` when a shifted read equals
`0xFFFF0000`. It then writes `0x1969` through `0x930F` and jumps to the erase
poll `0x9324`. **Confirmed.** `0x60`, `0x67`, and `0x72` therefore both program
and erase. `0x6C` erases and does not program. `0x6D` programs and does not
erase. `0x68` does not use the unlock sequence. What `DM(0x02000000)` is on
the board is **unknown**. Which half is erased, and what a power loss leaves,
is in the store section.

`0x940A` calls `0x9324` at `0x01018000`, `0x01020000`, `0x01028000`,
`0x01030000`, `0x01038000`, and `0x01040000`, then jumps to `0x80DC`. No
decoded call or jump targets `0x940A`, and the 16-bit load record has no word
equal to `0x940A`. Whether some other computation enters it is **unknown**.

The main-loop table at `0x1B911` can also reach the erase command. Slot 2 is
`0x93B4`, which stores an address to `DM(0xC2D6)` and jumps to `0x931C`.
**Confirmed.** When that slot runs is **unknown**.

### Other loaded entries

These do not call `0x930F`, `0x9324`, or `0x93F2` on the instructions listed.
Callees beyond the ones named here are not fully expanded. **Confirmed** for
the entry instructions; further effects of an unnamed callee stay open.

| Opcode | Entry | Entry behavior |
| --- | --- | --- |
| `0x52` | `0x961A` | Calls `0x95DE`, `0x9628`, `0x9610`, and `0x95E7`, then jumps to `0x9495`. |
| `0x53` | `0x9630` | Calls `0x95DE`, `0x9628`, `0x9609`, and `0x94BA`, then jumps to `0x95E7`. |
| `0x54` | `0x8104` | Stores two argument bytes through `DM(0x1B9F8 + index)` and jumps through the word at `DM(0x1BB78 + index)`. No range check. Words 0 through 127 at `0x1BB78` are inside the loaded program. From 128 on, the words include 0 and addresses outside that program. |
| `0x55` | `0x811A` | Replies `F0 54`, the current ring byte, the byte at `DM(0x1B9F8 + index)`, `F7`. |
| `0x59` | `0x814C` | Replies `F0 58` and 32 bytes from an address taken through `DM(0x1BA78)`. |
| `0x5C` | `0x8137` | Copies 32 ring bytes to `DM(0x1B9D8)` and returns. |
| `0x5D` | `0x813C` | Replies `F0 5C`. Bit 6 of the current ring byte selects one of two 32-byte sends. |
| `0x5E` | `0x9600` | Two argument bytes. Clears or sets one bit of `DM(0x2F)`. The bit number is the first byte minus 4. No range check. |
| `0x5F` | `0x8159` | Replies `F0 5E`, the current ring byte, and 0 or 1 from `0x95F4`. |
| `0x61` | `0x9499` | Copies 80 source words into `DM(0x1B9D8)` as 160 masked bytes and jumps to `0x94BA`. |
| `0x63` | `0x9650` | Reads one byte, stores 0 to `DM(0x1B910)`, and either jumps to `0x960B` or calls `0x9609` and jumps to `0x9394`. |
| `0x65` | `0x94A6` | Joins `0x66` with a count of `0x17` and a pointer of `0x1B932`. |
| `0x66` | `0x94A9` | Count `0x2F`, pointer `0x1B949`. Both scatter masked bytes into `DM(0x1B9D8)` and jump to `0x94BA`. |
| `0x69` | `0x9638` | One argument byte. Calls `0x95E7` and either `0x90CC` or `0x90C0`, then calls `0x954A` twice. |
| `0x6F` | `0x8195` | Replies `F0 6D`, the count byte, then three bit fields per word for that count. The count is not checked. |

Opcodes `0x40–0x51`, `0x58`, `0x5A`, `0x5B`, `0x62`, `0x64`, `0x6A`, `0x6B`,
and `0x70` are the `RTS` at `0x80CE`. They read no argument and send no reply.
**Confirmed.**

## Store and storage

The [store record](../research/podxt-301-store.json) locks 77 instruction
lines and the archived half measurements.
`python scripts/map_podxt_store.py` checks them against the container.
Nothing in this work was sent to a device.

### Slot window

**Confirmed** arithmetic. A slot is `0x50` words (160 bytes). `0x936C` walks
`0x180` slots, offsets `0` through `0x77FF`. The sentinel offset `0x7FFF` is
not a slot start. Offsets `0x7800` through `0x7FFE` are between the last slot
and the sentinel.

`0x81A8` builds `I0 = (14-bit index × 0x50) + DM(0xC2DF)` and does not
compare the index. **Confirmed.** The host addresses from `Location::address`
(Line 6 Edit `TranslateAddr`) are tones 0–63 and 192–255, amp setups 64–127
and 256–298, and FX setups 128–191. The maximum is 298, and
`298 × 0x50 + 0x50` stays inside the 384 slots. **Confirmed** as that formula
and that fit. The firmware does not branch on bank kind. **Likely** the host
index is the firmware slot index. That is not a captured trace.

Index 409 starts at word offset `0x7FD0`. Word `0x2F` of that slot is
`0x7FFF`, the sentinel. The 80-word program continues through offset
`0x8020`, `0x20` words past an `0x8000`-word half. **Confirmed** arithmetic.
The host addresses do not include 409.

### Where the two halves sit

**Confirmed** file facts. The width-2 load record contains
`DM(0xC2DF) = 0x01070000` and `DM(0xC2D6) = 0`. Region 17 is `0x10000` bytes
at file `0xE0000`. Region 18 is `0x10000` bytes at file `0xF0000`. Those
offsets equal `(address − 0x01000000) × 2`. The firmware map uses that same
formula for the resource directory. Region 18 is entirely `FF`. Region 17's
first two bytes are `50 6c` and its last two bytes are `19 69`. Of the 384
slots, the lower half has 0 whose first two bytes are `FF FF`, and the upper
half has 384.

**Likely** a CPU read of those addresses returns those bytes, and a 16-bit
big-endian read makes the upper first word `0xFFFF` and the lower last word
`0x1969`. The installed flash can differ from this archive. The load-record
value being present at reset is **likely** on the same basis as treating
`0x8000` as the loaded program. It is not a confirmed CPU start state.

Bit 15 of `0x01070000` is clear. Bit 15 of `0x01078000` is set. The two
bases differ by `0x8000` words.

### Opcode 0x71

**Confirmed.** `0x9420` calls the state preamble `0x93DF`. `0x9421–0x9423`
copy `B4`/`I4`/`L4` into `B5`/`I5`/`L5`. `0x9424` advances `I5` by 2.
`0x9425` calls `0x9428`. `0x9426` clears `L5`. `0x9427` is `RTS`. The body
`0x9420–0x9430` does not call `0x9324`, `0x931C`, or `0x936A`, and it does
not jump to `0x80DC`. There is no `0x50` and no `0x51`.

`0x81A8` consumes the two address bytes from `I4`. `0x9428` toggles bit 15
of the returned address, stores it at `DM(0xC2D6)`, and calls `0x930F`
eighty times. Each pass reads two bytes from `I5` and assembles
`second | (first << 8)`. There is no range check and no test that the
destination word is `0xFFFF`. The only callers of `0x9428` are `0x9367`
(opcode `0x60`) and `0x9425` (opcode `0x71`).

`0x930F` programs one masked 16-bit word and polls until the read matches.
`0x931A` has no other exit. **Confirmed.** A second program of a word that
never reads back as the new value stays in that loop. Whether the part can
change a programmed word is **unknown**. The main loop does not return while
the poll runs.

Opcode `0x71` does not write `DM(0xC2DF)`. The only assignments are
`0x9378`, `0x9387`, and `0x9390`. **Confirmed.** The `0x76` read at
`0x816B–0x8186` contains no `BTGL`, so it uses the untoggled base. A lone
`0x71` is not the half a following read addresses until something else flips
the base.

With the load-record base `0x01070000`, the toggled address is the upper
half. **Likely** when path A below has left that base in place.

### Commit at 0x936A

**Confirmed.** The only callers are `0x9368` (opcode `0x60`, after
programming one slot from `DM(0x1B9D8)`), `0x943C` (opcode `0x67`, after
`0x9333`), and `0x941E` (opcode `0x72`, which calls `0x936A` and then sends
`0x50`). `0x60`, `0x67`, and `0x72` call `0x93ED` first. Opcode `0x71` does
not.

`0x936A` saves `DM(0xC2DF)` and loops `0x180` times. Each pass toggles bit
15 of the running address, reads the toggled address, shifts that read left
16, and calls `0x9333` when the difference from `0xFFFF0000` is zero.
`0x9333` stores `R0` at `DM(0xC2DA)`, `R1` at `DM(0xC2D6)`, and `R2` at
`DM(0xC2DB)`. At the call, `R0` is the untoggled address, `R1` is the
toggled address, and `R2` is `0x50`. A spare slot whose first word has high
half `0xFFFF` is copied from the active slot. The running address then
advances by `0x50`.

After the loop, `0x9377–0x9378` store the toggled base at `DM(0xC2DF)`.
`0x9379` loads `0x7FFF`. `0x937A` is
`R0 = R0 + R1, R1 = DM(I7, -0x01)`. **Likely** the add uses `0x7FFF` and
the load overwrites `R1`, the same reading as the parallel store at
`0x81AC`. `0x937B` stores that sum at `DM(0xC2D6)`. `0x937C–0x937D` program
`0x1969` through `0x930F`. `0x937E` restores `DM(0xC2D6)` to the original
base. `0x937F` jumps to `0x9324`, which issues `0x30` and polls until that
one word reads `0xFFFF`.

The code issues one erase command at one address. It does not erase per
slot. **Likely** the following code treats the `0x8000`-word half as the
unit, because the scan covers that many slots and the sentinel is written
at `+0x7FFF`. The sector length is **unknown**.

### Reset selector 0x9380

**Confirmed.** Reset at `0x825E` calls `0x9380` before `0x8080` and `0x9609`.

| Path | Test | `DM(0xC2DF)` | Erase |
| --- | --- | --- | --- |
| A | `DM(0x01078000)`, shifted left 16, equals `0xFFFF0000` | not written | none |
| B | otherwise, and `DM(0x01070000)`, shifted left 16, equals `0xFFFF0000` | set to `0x01078000` | none |
| C | otherwise, and `DM(0x01077FFF)`, shifted left 16, equals `0x19690000` | set to `0x01070000` | `0x9324` at `0x01078000` |
| D | otherwise | set to `0x01078000` | `0x9324` at `0x01070000` |

Path C toggles `R2`, which holds `0x01078000`, before storing it. Path D
does not. `0x9391` toggles again into `DM(0xC2D6)`, so the erase address is
the other half. The routine reads the upper first word, the lower first
word, and the lower last word. It does not read `0x0107FFFF`.

On the archived bytes, under the likely mapping, the upper first word is
`0xFFFF`, so reset takes path A and leaves the load-record base
`0x01070000`. Reads use the lower half. Opcode `0x71` writes the upper
half. **Likely.**

If both first words fail the `0xFFFF` test, path C or D erases one half.
The erase address is one of the two bank bases, not the program block at
file `0x10000`. Whether a chip erase stays inside the addressed half is
**unknown**.

### Power loss

These are instruction consequences. They are not the current device state.
They also depend on the flash matching this archive, which is **unknown**.

During `0x71` the path issues no erase. A word whose poll at `0x930F` has
completed matches the value written. The word in progress may not. Later
words are not written. `DM(0xC2DF)` is unchanged, so the write address is
the opposite half from the one reads use. That pointer is a DM location in
the 32-bit load record. Opcode `0x71` does not store it to flash.

If the opposite half is the upper one and index 0 completes, the upper
first word is no longer `0xFFFF`. The next reset is not path A. If the
lower sentinel still reads as `0x1969`, path C keeps the lower half and
erases the upper half, which discards the `0x71` words. **Likely** under
the big-endian reading and the file mapping.

A non-zero index leaves the upper first word `FF` when slot 0 was not the
target, so path A can remain and the partial slot stays. A later `0x936A`
does not copy over a spare slot whose first word fails the `0xFFFF` test.
It still flips the base and erases the previous half, so the torn slot can
be the one that remains. The torn word's value is **unknown**.

During the `0x936A` copy, before the sentinel program finishes, some spare
slots may already be programmed, `DM(0xC2DF)` is not yet flipped, and the
old half is not yet erased. If slot 0 was copied, both first words fail the
`0xFFFF` test and the old sentinel is still present. The next reset uses
that sentinel to choose which half to erase.

After the sentinel poll completes, `0x9324` erases the old half and waits
until its first word reads `0xFFFF`. A reset in that window can see neither
first word as `0xFFFF`. Path C or D then erases one half, which can be the
half that still holds the copy. **Confirmed** as the control flow. The
physical cell state is **unknown**.

### What this path does not place

**Confirmed** for the instructions named. The only lines that name
`0x1B9A8` or `0x1B9A9` are `0x812A` (opcode `0x57`), `0x8133` (opcode
`0x56`), and `0x8C73`. `0x8C73` can store one word at `0x1B9A9` and returns
at `0x8C7B`. A 40-instruction straight line from `0x8C73`, and from each
in-program word of the `0x1B978` table, does not reach `0x930F`, `0x931C`,
`0x9324`, `0x9333`, or `0x936A`. An indirect callee is not expanded.
Whether another pointer aliases `0x1B9A8` is **unknown**. This does not
show that globals are stored in flash. Regions 1 and 2 are not named by
the store instructions traced here.

Opcodes `0x10`, `0x11`, and `0x23` are table word 0. This store path does
not name a separate Variax-link pointer. Whether a link value sits inside
the 80-word record is **unknown**. The record is 160 bytes, the same size
as the factory records in region 17. **Likely** the same record size. The
field layout is **unknown**.

Opcodes `0x6E`, `0x6D`, and `0x6C` remain the raw pointer path from the
command section. `0x6E` stores an address at `DM(0xC2D6)` with no slot
multiply. That address is outside this 384-slot map.

`0x940A` still calls `0x9324` at six addresses `0x8000` apart, starting at
`0x01018000`. Under the file-offset formula those addresses land on the
resource regions. No decoded call or jump targets `0x940A`. Reachability
is **unknown**. The body is a confirmed erase sequence if it is entered.

## Boot and validation

The [boot record](../research/podxt-301-boot.json) locks 40 instruction lines.
`python scripts/map_podxt_boot.py` checks them against the container.
Nothing in this work was sent to a device.

### Reset order

**Confirmed.** `0x825E` calls `0x9380`, `0x825F` calls `0x8080`, and `0x8260`
calls `0x9609`. The checksum at `0x8266` runs after those three return. The
main loop is `0x8271` (the parser), `0x8272` (`0x93AA`), `0x8273`
(`0x95C5`), and `0x8274` back to `0x8271`. No instruction jumps or calls
`0x8266`. The sum runs once per entry to reset, not on every pass of the
loop.

### The sum

**Confirmed.** `I0` starts at `0x01000000`. The loop count is `0x00080001`
(524,289). Each pass reads `DM(I0)` and post-increments. The reads are
`0x01000000` through `0x01080000` inclusive. That window includes both
stored halves. The last address is one word past `0x0107FFFF`. What
`0x01080000` is on the board is **unknown**.

The body is `R2 = FDEP R5 BY 0:8`, `R1 = FEXT R5 BY 8:8`, and
`R2 = R2 + R1`. The disassembler takes the two numbers from the low 6 bits
and the next 6 bits of the immediate. **Likely** each pass adds those two
8-bit fields of the word just read. `0x826E` stores `R0` at `DM(0x1B9AB)`.
`0x8270` stores `R0` shifted right by 16 at `DM(0x1B9AA)`.

No instruction from `0x823C` through `0x8267` contains `R2`. The sum starts
with `R0 = -R2`. The value of `R2` at that point is **unknown**.
**Likely** the first parallel add, `R0 = R0 + R2`, cancels it, and the
field sum of the last word stays in `R2` and is not stored. This program
was not executed, so no numeric total is claimed.

The only instructions whose text names `0x1B9AA` or `0x1B9AB` are those
two stores. **Confirmed.** Nothing between the stores and `CALL (0x000080B2)`
branches. A sum that would not match an expected value does not divert
this path, because this path does not compare it.

Opcode `0x56` sets `M1` from the raw argument byte and writes
`DM(0x1B9A8 + M1)`. Opcode `0x57` reads the same address. Neither masks the
byte. **Confirmed.** Indexes 2 and 3 are `0x1B9AA` and `0x1B9AB`. That is
the global access, not a boot check. An indirect read of the same words
through some other pointer is **unknown**.

### What reset does test

**Confirmed** for the reset line `0x823C` through `0x8274`. That line calls
`0x9380` and does not itself read the stored halves. The three-word test
inside `0x9380` is the startup check recorded in the store section. A tone
that leaves those three words unchanged does not change that choice. This
boot record does not add a per-slot checksum. Callees of `0x9609` beyond
the call instruction are not expanded here. A partial `0x71` that changes
one of the three words can make the next reset erase a half, as the store
section records.

`0x8080` does not call `0x930F`, `0x931C`, `0x9324`, or `0x936A`.
**Confirmed.** Before the zero fill it polls bit 30 of `DM(0xE1)`:
`0x808B` jumps to `0x8089` while the tested bit is clear. There is no
other exit. What that bit is on the board is **unknown**. If it stays
clear, reset does not reach the checksum or the parser. The following
fill writes `0x100000` zeros at `DM(0x02000000)`. That is not the unlock
sequence.

### Main-loop counter

**Confirmed.** The width-2 load record has `DM(0xC2E5) = 0`. Each main-loop
pass adds one and tests bit 19. `0x95C9` jumps to itself when that bit is
set. The first value with bit 19 set is `0x80000`. The only other writer
is `0x910E`, which stores zero. The only instruction that names `0x9105`,
the start of that interrupt body, is `0x8028`. If that interrupt does not
clear the counter, the parser loop stops after `0x80000` increments. The
spin does not call a program or erase routine. How long one main-loop pass
takes is **unknown**.

### What this does not show

A bad sum, a torn tone that leaves the three `0x9380` words in a path-A or
path-B state, and the `0x95C9` spin do not by themselves erase the program
block. The address `0x9380` passes to `0x9324` is one of the two half
bases. The sector length is still **unknown**, so whether that erase can
extend into the program block is **unknown**. The hangs at `0x808B` and
`0x95C9` stop the instructions that follow them. They are availability
failures on this path. They are not a claim about the current device.

## Device-facing notes from this slice

Nothing in this work was sent to a device.

| Trigger | Evidence | Consequence established here |
| --- | --- | --- |
| A SysEx opcode whose table word is 0, including `0x00–0x3F` and the editor opcodes `0x0E`, `0x10`, `0x11`, `0x23`, `0x0F`, and `0x24` if they arrive as the first byte after `F0`. | `JUMP (M15, I15)` has no range check. Those words are `0x0000`. | Target address 0 if `M15` is still 0. Address 0 is not in this load image. Further effect **unknown**. |
| A SysEx opcode whose table word is outside `0x8000–0x96B7` (86 opcodes, including `0x77–0x7F`). | The inventory lists the words. | Where execution would continue is **unknown**. |
| Opcodes `0xBF`, `0xCE`, `0xDD`, and `0xFB`. | The word is `0x8020`, and `0x8020` jumps to the receive routine that ends in `RTI`. | Entering that routine from the SysEx jump is **unknown** in effect. |
| Transmit with `FLAG1_IN` held clear. | `0x80D9` jumps to itself. | The loop has no timeout in the instruction. |
| Opcode `0x74` with fewer than 160 bytes after the opcode. | `0x818F` reads 160 ring words and does not test `F7`. | The read can take bytes already in the ring. |
| Opcode `0x71`, `0x60`, `0x67`, `0x6C`, `0x6D`, or `0x72`. | Those entries reach `0x930F` or `0x9324`. `0x930F` writes the unlock pair and `0xA0`. `0x9324` writes the unlock pair, `0x80`, and `0x30`, then polls for `0xFFFF`. | **Likely** flash program or erase. No timeout on the poll. `0x71` sends no `0x50` or `0x51`. |
| Opcode `0x68`. | `0x93F2` writes `0x100000` words at `DM(0x02000000)` and the handler replies `0x51`. | The address is **unknown**. The reply byte is the same opcode the host uses for a failed store. |
| Opcode `0x56` with index `0x30`, `0x3B`, `0x3C`, `0x3D`, or another outside slot of `0x1B978`. | `0x95A0` jumps through that word. | The target is outside the loaded program. Effect **unknown**. |
| Opcode `0x54` with an index of `0x80` or above. | `0x8111` jumps through `DM(0x1BB78 + index)` with no range check. | Words in that upper half include 0 and addresses outside the loaded program. Effect **unknown**. |
| Opcode `0x71`. | `0x9420–0x9427` returns after `0x9428`. That body does not call `0x9324`, `0x931C`, or `0x936A`. `0x9429` toggles bit 15 before `0x930F`. | Programs 80 words on the half opposite `DM(0xC2DF)`. No erase, no flip of `DM(0xC2DF)`, and no `0x50` or `0x51`. A word that never reads back stays in `0x931A`. |
| Opcode `0x60`, `0x67`, or `0x72`. | The only callers of `0x936A` are `0x9368`, `0x943C`, and `0x941E`. | Copies spare slots whose first word has high half `0xFFFF`, programs `0x1969`, flips `DM(0xC2DF)`, and erases the previous half. |
| Power loss during `0x71`. | That body issues no erase. `0x930F` polls each word before the next. | Completed words match. The word in progress and the words not started are not established. `DM(0xC2DF)` is unchanged. A completed index 0 on the spare makes the next reset take a path other than A. |
| Reset while both half-base words fail the `0xFFFF` test. | `0x9380` then uses the lower sentinel and jumps to `0x9324`. | Erases one of the two halves. The choice follows the lower sentinel. Sector length is **unknown**, so the erase may cover more than that half. This is the instruction path, not the current device. |
| Store index 409. | `409 × 0x50 + 0x2F = 0x7FFF`. `0x81A8` does not compare the index. | The 80-word program includes the sentinel and continues `0x20` words past the half. Host addresses stop at 298. |
| Globals `0x56`/`0x57`, and opcodes `0x10`, `0x11`, `0x23`. | The only lines naming `0x1B9A8` or `0x1B9A9` are `0x812A`, `0x8133`, and `0x8C73`. The three opcode words are 0. | No flash routine on the straight-line paths checked. No separate Variax-link slot was identified on this store path. |
| Reset reaches `0x808B` while bit 30 of `DM(0xE1)` stays clear. | `0x808B` jumps to `0x8089`. `0x8080` does not call `0x930F` or `0x9324`. | Reset does not reach the parser. What the bit is on the board is **unknown**. |
| The `0x8266` sum. | The result is stored at `0x1B9AB` and `0x1B9AA`. No other instruction names those addresses, and the next call is the main loop. | Reset does not branch on the sum. |
| A tone that leaves the three `0x9380` words unchanged. | The reset line calls `0x9380` and does not itself read the halves. | The boot choice is unchanged. This boot record has no per-slot checksum. |
| `0x80000` main-loop passes without the `0x8028` interrupt. | `DM(0xC2E5)` starts at 0, `0x95C5` adds one, and `0x95C9` jumps to itself once bit 19 is set. `0x910E` stores 0 and is reached from `0x8028`. | The parser loop does not resume. The spin does not program or erase. |
