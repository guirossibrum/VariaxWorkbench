# Variax 300/600 3.10 command dispatcher

This is the first Phase A slice of [#181](https://github.com/guirossibrum/VariaxWorkbench/issues/181).
The [dispatch inventory](../research/variax-310-dispatch.json) covers all 110
table entries. The [handler records](../research/variax-310-read-handlers.json)
cover the session flag and preset-buffer read. The
[store trace](../research/variax-310-store-handlers.json) covers opcode `0x61`.
The [buffer trace](../research/variax-310-buffer-handlers.json) covers `0x43`
and `0x3B`. The [session trace](../research/variax-310-session-handlers.json)
covers `0x4C` and the writers of the session flag. The
[read-limit trace](../research/variax-310-read-limits.json) covers the
`0x3C` reply length, the transmit queue, and the two editor spans. The
[flash trace](../research/variax-310-flash-handlers.json) covers the erase
step, the slot read, the other program and erase handlers, and the two
code-load opcodes as a list. The
[remaining-handler trace](../research/variax-310-rest-handlers.json) covers
the last 24 command entries, the `0x7E` header branch, and the unsolicited
send sites. Findings are static.
Firmware execution and live device checks remain outside this work.

## Coverage

| Scope | Confirmed static result | Remaining work |
| --- | --- | --- |
| Line 6 SysEx table | `P:0x14316` indexes 110 words at `P:0x14326–0x14393`, for opcodes `0x10–0x7D`. All targets are present in the main-region P load records. The `0x7E` header branch is traced below. | The rest of the receive state machine remains. |
| Direct exits | 64 entries target the common stack-unwind return at `P:0x1483E` without calling a command-specific handler. | Follow the outer parser after this return to establish what happens to remaining input. |
| Command-specific targets | All 46 distinct entries are traced. The last 24 do not call the erase or program helpers directly. `0x3E` ends in an indirect call whose target was not found. | None of the 46 command handlers remain. PODxt Live is a separate pass. |
| Image bytes | The 110 pointer words classify 330 bytes as dispatch data in the 442,402-byte decompressed Variax image (0.075%). Program change, CC and the `0x7E` header branch are traced below. | Whole-image classification and PODxt Live coverage remain open. |

The dispatcher subtracts `0x10` at `P:0x1431A`. It branches on underflow
before the table access, then fetches the P-space word at `P:0x14322` before
the branch at `P:0x14323` that rejects indices above `0x6D`. The upper
comparison guards the indirect jump. The consequences of the preceding fetch
on the physical processor are unknown.

## Session flag and preset-buffer read

| Opcode | Confirmed input and action | Confirmed reply path |
| --- | --- | --- |
| `0x2A` | `P:0x144DD` takes bit 0 of one byte, stores it to `X:0x17F4` and reads one more byte before returning. The store precedes the trailing read, which does not check for `F7`. The flag controls outgoing program-change and CC helpers at `P:0x14A0B` and `P:0x14A1D`, and `P:0x135E` sends `0x28` only when the flag equals 1. Reset startup at `P:0x14282` stores 0. Queued words `0x070001` and `0x070002` also store 0, through `P:0x179C`. None of those writers touches the DSP-bypass flag. | No immediate acknowledgment in the handler. |
| `0x3C` | `P:0x144A9` decodes two 24-bit words, count first and start word second. `P:0x1268` masks each encoded byte to four bits and assembles six low-first nibbles. The read starts at `Y:0xA47 + start_word` after the `F7` check at `P:0x144B4–0x144B9`. Each iteration emits one 24-bit word and advances the pointer. There is no explicit address, count or combined range check in the handler. | The normal path queues the family header, `0x3C`, six low-first nibbles per word, and `F7` at `P:0x1472B`. A 32-word reply totals 200 bytes with framing. No address or count is echoed. An unexpected trailing byte at the terminator check branches to the `0x72` reply at `P:0x147ED`. |

**Confirmed, static:** Receive helpers at `P:0x1257` and `P:0x1261` poll
for a queued byte. The nibble reader consumes six bytes without an early
terminator check. A truncated request can leave the parser polling for input,
and the `0x2A` flag can already be changed while the trailing read is pending.
The transmit helper at `P:0x1241` waits for queue space. These loops do not
establish elapsed-time bounds. Effects of a partial frame on subsequent
traffic and the effective count limit on the installed processor remain unknown.

**Confirmed, host:** Workbench 1.75
`L6VariaxPatchManager.RequestDevicePatch` requests two 32-word blocks
starting at 851 and 883 and waits up to 2,500 ms per wait call. It changes
program first only when supplied a bank and slot. Its bulk-get wrapper also
uses DSP disable/enable. The Rust family path uses the same address and
count pairs, uses a three-second reply window and wraps reads in session
on/off. The owner observed metadata reads through this path, which sends no
DSP bypass for the read. The firmware handler itself does not test the
session flag or change DSP state.

## Answer for #119

**Confirmed, static:** The handler's source is the current Y-memory preset
buffer. It takes no slot number and has no firmware address guard excluding
words 0–850. At `P:0x1FA7`, one preset-load branch copies 873 words to that
buffer (`P:0x1FB8–0x1FCD`). The successful slot-load branch copies all 1,092
words (`P:0x1FD8–0x1FF9`). Both copy spans include words 0–850. The longer
span ends at `Y:0xE8A`, and the preset checksum routine at `P:0x209D` sums
the preceding 1,091 words. To inspect an inactive preset via this handler,
the preset would first have to be loaded into the current buffer.

**Unknown:** Whether readback outside the two observed metadata blocks works
over the physical link, and whether every selection path yields an exact
stored-image match for all 1,092 words. A count of 0 is not tested in this
image, and what the instrument does with it is unknown. The static reply
length and the missing span check are in the limits section below. The
compact copy alone leaves the tail's provenance unresolved. There is no
surviving full-DSP request or reply from the 2026-11-14 timeout described
in [#119](https://github.com/guirossibrum/VariaxWorkbench/issues/119), so
its cause remains unknown.

## Host risks from this slice

| Trigger | Static evidence and possible consequence | Existing app boundary |
| --- | --- | --- |
| A `0x3C` span reads past word 1,091, or the host stops taking the reply. | `P:0x144B1` applies the start with no span check. `Y:0xE8B`, the first address past the buffer, is initialized through `Y:0xEA6`. `P:0x1241` waits while the 63-byte transmit queue is full. The limits section records the reply length and the zero-count gap. | The request enum exposes only the two fixed 32-word metadata spans, and the reader requires 200 bytes. |
| A session command is interrupted after its argument, or cleanup fails. | `P:0x144E0` changes the flag before `P:0x14565` reads the trailing byte. Session state may persist, affecting outgoing MIDI reports. | The Rust session wrapper attempts session off even when the body or initial send fails and reports an off-send failure after a successful initial send. |

`0x2A` and `0x3C` identify no direct persistent write. The store path below
is the first flash trace in this series. A wider `0x3C` probe needs the
owner's separate procedure approval.

## Preset-buffer read limits (`0x3C`)

`P:0x144A9` reads the count, then the start. It loads `0xA47` into `r6`,
puts the start in `n6`, and applies `(r6)+n6`. **Confirmed.** The handler
does not write `m6`. The same image does write `m6` in other handlers,
including an immediate `5` at `P:0x14154` and a copy from `m7` at
`P:0x144F2`, both outside this opcode. A plain sum is what `(r6)+n6` does
when `m6` is the linear mask. This handler never writes that mask, so the
modulus in force at entry is **unknown**. The two editor starts are the
spans the host decodes, and the owner has completed those metadata reads.
That those starts land on words 851–882 and 883–914 is **likely**.

Nothing in `P:0x144A9`–`P:0x144DC` compares the count or the end with
1,092, and nothing there calls `P:0x0F17`, `P:0x0F45`, `P:0x0F63` or
`P:0x0FA6`. **Confirmed.** This opcode reads Y memory. It is not a flash
read.

The byte after the two words must be `F7`. Any other value jumps to
`P:0x147ED` and sends `0x72` without entering the data loop.
**Confirmed.**

On `F7`, `P:0x1367` queues `F0` and then five bytes from `Y:0xA42`. Those
bytes are `00 01 0C 07 04`. **Confirmed.** The fifth byte is the device id.
The host accepts either `00` or `04` in that position. The handler then
queues `0x3C`. For each counted word it queues six low nibbles. The inner
loop's pointer update is discarded. `P:0x144D8` advances the saved pointer
by one word. The save at `P:0x144D9` is the last instruction of the count
loop. The next instruction, `P:0x144DB`, jumps to `P:0x1472B`, which queues
`F7`.
**Confirmed.** The reply length is `8 + 6 × count` bytes. A count of 32 is
200 bytes. A count of 1,092 from start 0 is 6,560 bytes.

Under a plain sum, a nonzero count stays inside words 0–1,091 when the
start is at least 0 and `start + count` is at most 1,092. The largest such
count from start 0 is 1,092. Start 1,092 reads `Y:0xE8B`. That address sits
in an initialized Y record that continues through `Y:0xEA6`. **Confirmed**
that the word is initialized image data. What the words after `Y:0xE8A`
mean is **unknown**. The load is still a Y read.

The transmit queue at `X:0x141B` holds 63 bytes. The sender at `P:0x1245`
and the drain at `P:0x19D3` both mask the index with `0x3F`. `P:0x1241`
stores its byte only when `(write + 1) & 63` differs from the read index.
Otherwise it branches back to the test. That loop has no deadline, does not
drop the byte, and does not call the receive parser. **Confirmed.** The
only program word equal to `0x19B7` is the extension of the `jsr` at
`P:0x0058`, the vector slot after `jsr $20D2` at `P:0x0056`. No short
branch targets the drain. Which peripheral uses that vector is unknown.
The drain takes a queued byte only when `X:0x1801` equals 9 and the queue
is not empty. Other entries shift the byte already in `X:0x17FF` toward
`x:$FFFFC9`. **Confirmed.**

A 9-word reply is 62 bytes and fits in one fill. A 10-word reply is 68
bytes and does not. The editor's 200-byte replies already take more than
one fill. The owner has completed that read, so the normal path does drain.
If the host stops accepting MIDI, `P:0x1241` keeps waiting. Bytes that
arrive on the receive queue stay queued until that wait returns. This wait
loop does not mask interrupts. **Confirmed** for the loop itself.

A count of 0 is not tested in this image. The count word is passed to
`do y1` with no mask in the handler. **Confirmed.** The local toolkit skips
a DO body when the loop count is 0. That was not measured on a guitar, and
it is a different instruction from the family manual's REP count of 0.
**Unknown** on the instrument. If the body is skipped, `P:0x144DB` still
runs, so the frame already started is closed with `F7` and no preset word
is read. The request enum sends no zero count. **Confirmed** for that
source.

### Trap list (preset read)

| Trigger | Address | Consequence | Our editor can send it | How the editor avoids it |
| --- | --- | --- | --- | --- |
| A start and a nonzero count whose last word is above 1,091, under a plain sum. | `P:0x144B1` | The handler reads Y past `Y:0xE8A` and still sends the normal `0x3C` frame. The first such address is initialized image data through `Y:0xEA6`. This case does not send `0x72`. | No. `Metadata1` and `Metadata2` are the only `0x3C` requests, and they end at words 882 and 914. | Keep those two shapes. Do not add a start or count the firmware does not bound. |
| A count of 0. | `P:0x144C2` | The image does not test it. A skipped body would send an eight-byte frame and read no Y word. A count that instead runs the body would walk far past the buffer and hold `P:0x1241`. The instrument's choice is unknown. | No. The request bytes contain no `0x3C` count of 0. | Do not send a zero count. |
| The host stops reading a reply longer than 63 bytes. | `P:0x1241` | The sender waits with no deadline. Queued input is not parsed until the wait returns. This loop does not mask interrupts. A count of 32 already produces 200 bytes. | Yes, if the host stops reading. The normal metadata read waits for all 200 bytes. | Keep reading the reply through `F7`. Do not add an unbounded `0x3C`. |

## Preset store (`0x61`)

`P:0x14731` reads one index byte, saves it, and reads the following byte.
That byte must be `F7`. Any other value branches to `P:0x1482D` and replies
`0x66` (`P:0x1482E`) without calling the store. On `F7` the handler copies
four words from `Y:0x151BB` into preset words 767–770 (`Y:0xD46`), sums
1,091 words at `Y:0xA47` through `P:0x209D`, and stores that sum at word
1,091 (`Y:0xE8A`). It then calls `P:0x0FA6` with the saved index still in A.
The branch to the `0x71` reply at `P:0x14805` is the next instruction after
that call returns. **Confirmed.** The handler does not call `P:0x1FA7` or
`P:0x1E9C`, so this command does not reload the DSP working set through
those routines.

The four copied words are `0x30`, `0x33`, `0x31`, `0x30`. **Confirmed** from
the tag-2 record at `Y:0x151BB`. Their sum is `0xC4`. Every archived factory
slot has words 767–770 and words 1,089–1,090 equal to zero, and both the
host sum (words 0–1,088) and the firmware sum (words 0–1,090) match word
1,091 (0 mismatches in all 60 slots). A store of such a buffer therefore
raises word 1,091 by `0xC4`. Settings metadata starts at word 851, so the
two blocks the editor already reads (851 and 883) do not include the stamp
or word 1,091. Whether the DSP uses words 767–770 as sound coefficients is
**unknown**.

There is no compare of the index with 59 in `P:0x14731` or `P:0x0FA6`.
**Confirmed.**

### Ordered flash steps

`P:0x0FA6` sets bit 4 of `Y:0xFFFF9D`, divides the index by 20 at `P:0x001A`,
and then makes these calls, in this order. **Confirmed** by the call list
and by the address words that sit between the calls.

| Step | Call | What it changes |
| --- | --- | --- |
| 1 | `P:0x0F17` | Programs 1,092 words from `Y:0xA47` through X at the spare address. No erase first. |
| 2 | `P:0x0F45` | Programs one `0x00` byte at the slot base through Y. A is cleared immediately before the call. |
| 3 | `P:0x0F63` | Erases the slot sector. |
| 4 | `P:0x0F17` | Programs the same 1,092 words through X at the slot address. |
| 5 | `P:0x0F63` | Erases the spare sector. |

The normal return clears bit 4. A hang inside any helper leaves the bit set
and never reaches `0x71`. The bit is PE4, programmed as a GPIO output.
What that pin selects on the board is **unknown**.

Addresses, with the index in A:

- Slot sector: `0xD00000 + index × 0x1000`
- Spare sector: `0xD3C000 + (index // 20) × 0x1000`, which is sector `60 + index // 20`

`P:0x001A`–`P:0x0034` matches a typed divide-by-20 program word for word.
Measured quotients are 0 for 0–19, 1 for 20–39, 2 for 40–59, 3 for 60–63,
6 for 127 and 12 for 254. `mac +S,#12,D` then shifts that quotient by 12.
A dirty upper accumulator word does not change the low word when the shift
does not carry, which holds for every byte-sized index and group.
**Confirmed** on the synthetic program. The only `0xD3C000` literal in the
image is at `P:0x0FB0`. The only `add #$3c,a` is at `P:0x0FCC`, and the next
call is the spare erase. `0xD3D000` and `0xD3E000` do not appear as literals.

| Flash indices | Spare | Workbench banks |
| --- | --- | --- |
| 0–19 | sector 60 at `0xD3C000` | Custom 1, T-Model, Spank, Lester (banks 0–3) |
| 20–39 | sector 61 at `0xD3D000` | Special, R-Billy, Chime, Semi (banks 4–7) |
| 40–59 | sector 62 at `0xD3E000` | Jazzbox, Acoustic, Reso, Custom 2 (banks 8–11) |

The editor index is `bank × 5 + (4 − slot)`. Inside a bank the flash index
runs opposite the Workbench slot: bank 0 slot 4 is index 0, and bank 0 slot
0 is index 4. The 60 results are exactly the indices 0–59, once each.
**Confirmed** against `Preset::new` and `Message::Store` in the family crate.
`program_number()` adds 55 for banks 1–10 and is the MIDI program-change
index. The store message does not send it. Those middle-bank program numbers
are all at least 60.

One sector step is 4,096 bytes. The program is 3,276 bytes. The archived
tail of every factory slot, bytes 3,276–4,095, is `0xFF` (0 slots differ).
The owner's tails are **unknown** and are outside the settings backup.

The store programs the slot image through X and the loader reads flash
through Y at the same numeric address. While bit 4 of `Y:0xFFFF9D` stays
set, both accesses match AAR1 and drive the same AA1 and the same A0–A17.
The DSP56367 has no separate X/Y pin, so those two steps are the same
external cycle. **Confirmed** for the pin list and the register words in
[which pins a sector address drives](#which-pins-a-sector-address-drives).
The zero byte is written through Y immediately before the erase of that
same address, so a completed erase removes it. It is not a flag the loader
tests. The schematic names one SST39LF040.

### Polls have no overall timeout

**Confirmed** from the helper control flow:

- `P:0x0F17` reads the byte it has just written and branches back until the
  low 8 bits match. The branch has no counter.
- `P:0x0F45` counts 10,000 polls (`P:0x0F59` is `0x2710`) and then restarts
  the byte program. The restart has no attempt limit.
- `P:0x0F63` waits until bit 7 of the sector address reads as set three
  times. A clear bit waits again. When the tick at `X:0x179C` passes 488
  (`P:0x0F82` is `0x1E8`), the erase starts over, including the count of
  three. There is no attempt limit and no failure return.

The elapsed time of 488 ticks is **unknown**. A poll that never matches
does not return, so `0x71` is not sent. On a NOR part, programming cannot
turn a 0 bit back to 1. That hang is **likely** for the SST39LF040 named on
the schematic and **unknown** for the chip fitted on the owner's board.
Spare indices 60–62, read with bit 4 of `Y:0xFFFF9D` set, are outside the
60-slot preset region of the archive, so whether a spare arrives erased is
**unknown**. The first store into a group programs that spare with no
preceding erase.

### The spare is not a recovery copy

**Confirmed** for the image's own address math:

- `P:0x1FA7` and `P:0x1E9C` contain neither the spare literal nor `add #$3c`.
- `P:0x1E9C` does not call `P:0x0FA6` or `P:0x20A5`. It reloads DSP state
  from flag bits at `Y:0xA48`.
- `P:0x1FA7` calls `P:0x20A5`, which forms `0xD00000 + index × 0x1000`.
  Arguments 0–4 and 55–59 keep that index. Arguments above 59 subtract 55
  first (`P:0x1FD0` compares with 59, `P:0x1FD2` subtracts 55), so argument
  60 reads sector 5. Arguments 5–54 copy 873 words from the template at
  `0xD20000` and return. The selection helper at `P:0x11A0` adds 55 before
  calling `P:0x1FA7` for indices 5–54, which takes the slot-sector read.
- On a checksum match, `P:0x0FEF` returns 1 and `P:0x1FA7` copies 1,092
  words into `Y:0xA47`. On a mismatch it stores `0x100000` at `Y:0xA48` and
  leaves the buffer uncopied. The reload copies that value to `Y:0xD0`.
  Bit 20 selects the short mix. Listening level is **unknown**.
  The mismatch branch does not read the spare. The
  [checksum-failure answer](#6-corrupted-slot-at-boot-or-on-load) records
  the path.

`P:0x148B9` can form a sector address from a supplied index through
`P:0x20A5`. It does not add 60, and the checksum-failure branch does not
call it. Giving it index 60 would read the spare. That is a separate
command, not a fallback, and this note does not describe how to send it.

### Power loss

The instruction order is **confirmed**. What a cut leaves in the cells is
**likely** for an SST39LF040 sector erase and **unknown** for any other
fitted part. `0x71` is absent in every row except the last.

| Cut is here | Slot sector | Spare of that group | Next full-path load of that slot |
| --- | --- | --- | --- |
| After the RAM stamp and checksum, before step 1 | Unchanged | Unchanged | Loads the old image. The RAM buffer already holds the stamp. |
| During or after step 1, before step 2 | Unchanged | Partial or complete new image | Loads the old image. The spare is dirty. |
| After step 2, before step 3 finishes | Zero at the first byte, or an erase already in progress. The zero and the erase drive the same pins while bit 4 stays set. | New image if step 1 finished | Checksum fails, so the copy is skipped and `Y:0xA48` becomes `0x100000`. The spare is not copied in. |
| During step 3 or step 4 | Erased, or a partial new image | New image | Same checksum failure. Nothing reloads the spare. |
| After step 4, during step 5, before the reply | New 1,092 words, with the stamp and the firmware checksum | Erase in progress, or already erased if the helper returned and only the reply was lost | Loads the new image. A missing `0x71` does not mean the slot is unchanged. |
| After `0x71` | New image | Erased | Loads the new image. Bytes 3,276–4,095 of the sector are the erased state. |

An in-range store erases only that one slot sector and the one group spare.
Neighbour presets are other `0x1000` steps. The schematic's SST39LF040 uses
4 KB sectors, so a neighbour preset is **likely** left alone. A different
fitted part with a larger erase block is **unknown**. Calibration data has
not been located. Sectors 63–127 are outside the preset region; their
contents are **unknown**.

Index 63 is the only index below 256 whose slot address equals its spare
address (`0xD3F000`). The same five steps then program that sector and the
final erase clears it. Indices 60–62 are themselves spare sectors 60–62;
each uses sector 63 as its spare. Index 64 and above address `0xD40000`
upward. Index 127 is `0xD7F000`, the last 4 KB step of a 512 KB linear map.
Past that is **unknown**.

### What this means for the first stored-bank test

[#177](https://github.com/guirossibrum/VariaxWorkbench/issues/177) stores one
slot and waits up to 15 seconds for an eight-byte reply whose status byte
is `0x71`. The reply carries no slot id. After `0x61` has been sent, the
existing transaction finishes that wait and does not send `0x61` again on
an ambiguous result. The firmware trace extends that rule to every slot
that shares the spare, not only the slot that was just attempted.

- A missing `0x71` has two settled explanations that the reply cannot tell
  apart. The spare program can hang before the slot changes, or the slot
  program can finish and the spare erase can hang afterwards. Read the
  target slot before deciding which one happened. Do not store any of the
  other 19 slots in that group until the spare is known to be blank.
  The only `add #$3c` in the image is the spare erase at the end of
  `P:0x0FA6`. No startup path traced here erases sectors 60–62. The wide
  erase `0x68` (`P:0x146C0`) does erase sectors 32–63, which includes those
  spares and presets 32–59. `0x63` (`P:0x146BD`) erases sectors 64–127.
  The editor sends neither command. `0x68` erases a whole half of the upper
  map, so it is not a way to repair one slot.
- A full 1,092-word compare with the bytes the host sent fails after a
  successful store, because words 767–770 and word 1,091 change. Compare
  the settings metadata. Do not retry because of that difference.
- Re-read the neighbouring presets. Stop if one changed.
- The host checksum is checked before the transaction opens. The firmware
  recomputes its own sum after the stamp, so that check does not freeze
  the checksum that ends up in flash.

### Trap list

| Trigger | Address | Consequence | Our editor can send it | How the editor avoids it |
| --- | --- | --- | --- | --- |
| Store index 60 or above. Index 63 erases the only copy of that sector. | `P:0x14731`, `P:0x0FA6` | Programs and erases spare sectors and addresses at `0xD40000` and above. No range check. | No. `Preset::new` allows banks 0–11 and slots 0–4, and `0x61` sends `bank × 5 + (4 − slot)`, which is 0–59 once. | Keep that formula. Do not send `program_number()` as the store index. |
| Program or erase poll never matches. | `P:0x0F17`, `P:0x0F45`, `P:0x0F63` | The handler does not return, `0x71` is never sent, and bit 4 of `Y:0xFFFF9D` stays set. | Yes, if the part does not finish. The first store of a group can hit this when the spare is not blank. | After an ambiguous `0x61`, do not send `0x61` again for any slot in that group of 20. |
| Spare left programmed, then any later store in the same 20 slots. | `P:0x0FB9` before `P:0x0FCE` | The next store programs the spare without erasing it. A NOR-style poll can hang before the new slot is touched. Startup in this image does not erase sectors 60–62. A measured power cycle is unknown. | Yes, if an earlier `0x61` in that group was interrupted or its spare was not blank. | Treat the whole bank group as off limits after an ambiguous store. The groups are banks 0–3, 4–7 and 8–11. |
| Normal `0x61` of a host program whose words 767–770 and 1,089–1,090 are zero. | `P:0x1473A`–`P:0x14744` | Words 767–770 become `0x30 0x33 0x31 0x30` and word 1,091 increases by `0xC4`. A full-program compare disagrees with the host buffer. | Yes. This is what a successful store does. | Expect the stamp. Compare settings from word 851. Do not retry to force byte-identical flash. Audio effect of the stamp is unknown. |
| Power or link lost after the zero byte or during the slot erase or slot program. | `P:0x0FC1`–`P:0x0FC8` | That slot's sector is incomplete. The spare is not copied back. The next full-path load skips the copy and sets `Y:0xA48` to `0x100000`. The reload copies that value to `Y:0xD0`, and bit 20 selects the short mix. Listening level is unknown. | Yes, if the owner's store is interrupted. | Keep the verified backup. Do not retry the group. This image has no spare fallback. |
| `0x71` missing after the slot program has returned. | `P:0x0FCA`, then `P:0x0FCE`, then `P:0x14805` | The slot can already hold the new image while the spare erase or the reply is still outstanding. | Yes. The 15 s wait can time out after step 4. | Read the slot before any further store. A timeout does not mean the slot is unchanged. |
| Successful store erases the whole `0x1000`-byte step and programs 3,276 bytes. | `P:0x0F63`, `P:0x0F17` | Bytes 3,276–4,095 are left erased. Factory tails are `0xFF`. Owner tails are unknown and are not in the settings backup. | Yes, on a completed store. | Do not assume a non-`FF` tail survives. It is not part of the 1,092-word program the loader copies. |
| Fitted flash erases a block larger than 4 KB. | `P:0x0F63` steps by `0x1000` | A neighbour preset could be erased with the target slot. | Yes, the editor does send in-range `0x61`. | **Likely** avoided on the schematic's SST39LF040. **Unknown** if the fitted part differs. #177 re-reads the neighbouring presets and stops if one changed. |

The same store routine is also called from `P:0x1186`, `P:0x141A4`,
`P:0x145E2`, `P:0x145F3` and `P:0x1481E`. Those callers are not the editor's
`0x61` message. The flash section names which messages reach them and which
ones the editor sends. `P:0x1016` is one code-load range. That section lists
the opcode and the sectors and does not describe how to invoke it.

## Full buffer (`0x43`) and difference (`0x3B`)

Both handlers write the current preset buffer at `Y:0xA47`. Neither compares
the count or the start with 1,092. The word reader at `P:0x1268` assembles
six low nibbles and has no `F7` test, so an early terminator is payload and
a short message leaves the reader waiting. **Confirmed.** A typed `do a1`
program on the DSP56300 emulator skipped its body for a count of 0 and ran
it for a count of 2. The loop counter in that emulator is 16 bits.

### `0x43` at `P:0x14668`

The first word is the count. The handler then stores that many words at
`Y:0xA47`. Each word is committed when its six nibbles have arrived, so a
transfer that stops after K complete words leaves those K words replaced and
the rest of the buffer as it was. **Confirmed.**

The byte after the counted words must be `F7`. Any other value replies `0x66`
at `P:0x1482D` and does not reload. The counted words are already stored.
`P:0x2095` then sums 1,091 words and compares the sum with word 1,091. A
mismatch returns at `P:0x1483E` with no reply and no call to `P:0x1E9C`.
The rejected image stays in the buffer. A match jumps to `P:0x1483A`, which
calls `P:0x1E9C`, and sends no `0x71`. **Confirmed.** That is why the
original editor waits for no `0x43` reply.

The host check in `Program::new` sums words 0–1,088. The firmware sum also
includes words 1,089 and 1,090. A host-valid program with either of those
words nonzero fails `P:0x2095` after the buffer has been replaced.
`store()` does not read a `0x43` reply. When the send itself returns
success, it waits 100 ms and sends `0x61`. The store handler then overwrites
word 1,091 with its own sum, so a rejected buffer can still be the image
that goes to flash. A send error returns before `0x61`. Whether 100 ms is
always enough for the guitar to finish the counted loop is **unknown**.

Our editor's full-buffer message carries the count 1,092, which fills words
0–1,091 and stops at `Y:0xE8A`.

### `0x3B` at `P:0x14433`

The handler repeats until `F7`. Each run is a count word, a start word, and
then that many data words, written at `Y:0xA47 + start`. There is no
checksum. `F7` replies `0x71` at `P:0x14466` and falls into the same reload
at `P:0x1483A`. **Confirmed.** The original editor does not wait for that
`0x71`.

Workbench's ordinary runs are count then start, and they match this parser.
A run whose last word is 1,089 still uses that order. The formula tests
lock both cases. A run that includes word 1,090 is the exception in
[the write contract](variax-write-transactions.md): the editor sends the
start and then the count. `P:0x14433` does not swap those words.

| Editor run | Words it sends first | What `P:0x14433` does |
| --- | --- | --- |
| Words 10–13 | count 4, start 10 | Writes those four words. |
| Words 1,000–1,089 | count 90, start 1,000 | Writes those 90 words. |
| Word 1,090 only | start 1,090, count 1, and one data word | Takes 1,090 as the count and 1 as the start. Stores the one data word at word 1, then waits for 1,089 more words. |
| Words 0–1,090, the unknown-baseline run | start 0, count 1,091, and 1,091 data words | Takes the count as 0. A zero count writes nothing on the emulator used here. The data words are then read as another run. |

`Edit::Difference` emits the reversed header whenever the changed run
includes word 1,090, including `previous: None`. `store()` does not send
`0x3B`. It sends `0x43` with count 1,092.

### Trap list (edit buffer)

| Trigger | Address | Consequence | Editor before #208 | Mitigation in #181 | Can the editor still trigger it after #208? |
| --- | --- | --- | --- | --- | --- |
| `0x43` ends before the counted words, or its bytes arrive while the reader is still inside `P:0x1268`. | `P:0x14668` | The completed prefix of the buffer is replaced. The reader waits. There is no reply and no reload. | Yes, if the transfer stops after `0x43` has started. `store()` sends `0x61` only when the `0x43` send returns success. | Keep the backup. Do not send `0x61` after an incomplete `0x43`. The 100 ms pause does not prove the guitar has finished. | Yes. An interrupted transfer can still partially overwrite the buffer. |
| Host checksum matches and word 1,089 or 1,090 is nonzero. | `P:0x2095` | The whole buffer is replaced, the firmware sum fails, and there is no reply and no reload. `0x61` can still store that buffer, with a newly computed checksum. | Yes. `Program::new` checks the 1,089-word sum and does not force those two words to zero. Factory slots have them zero. | Keep words 1,089 and 1,090 at zero, or do not store a program whose firmware sum disagrees. Do not treat a missing `0x43` reply as success or as rejection: success sends none. | No. A checksum disagreement is rejected in `Program::new`. A nonzero pair is accepted only if it sums to zero modulo 24 bits. |
| Count above 1,092, or a `0x3B` start and count that pass word 1,091. | `P:0x14670`, `P:0x14436` | Writes continue past `Y:0xE8A`. What occupies the following addresses is unknown. | The normal `0x43` count is 1,092. An in-order `0x3B` run stays inside words 0–1,089. | Keep those message shapes. The reversed run below is the case that does not. | No. The encoder fixes the `0x43` count at 1,092 and bounds each count/start run to words 0–1,090. |
| A `0x3B` run includes word 1,090. | `P:0x14433` | The firmware reads start as the count and count as the start. The single-word case stores that word at word 1 and then waits. The all-words case uses a zero count, writes nothing for that header, and parses the data as a new run. `F7` still replies `0x71` and reloads whatever was written. | Yes, through `Edit::Difference` when word 1,090 changed or the baseline is unknown. `store()` does not send it. | Send a full `0x43` for a program whose changed run includes word 1,090. Do not send that `0x3B` shape. | No reversed run. The editor uses count/start through word 1,090. The unknown-baseline path uses a full upload. |
| A `0x3B` reply is still unread when a store wait begins. | `P:0x14466`, then the store wait | `0x3B` transmits `0x71`. The store wait accepts the first eight-byte `0x71` and the reply carries no slot id. | Not from `store()` itself. A caller can leave the reply on a shared stream. | Finish or discard the difference reply before waiting for a store. | A completed difference wait consumes its reply. A session that allows edits refuses storage if a reply is queued at either check. A failed or ambiguous exchange requires a fresh link. |

## DSP bypass (`0x4C`) and what clears the two flags

`P:0x14561` reads one byte, stores that whole byte at `X:0x17A0`, and falls
into `P:0x14565`. The shared tail reads one more byte and does not compare
it with `F7`. The handler sends no reply. **Confirmed.** Host
`Message::DspEnabled` sends byte 1 for DSP off and byte 0 for DSP on. Any
nonzero stored value selects the bypass branch. A later `0x4C` clears the
flag only when its byte is 0.

The only instructions whose address extension is `0x17A0` are four reads
and two stores. The stores are `P:0x14563` (the `0x4C` byte, with no mask)
and `P:0x14023` (the value 0). The only long `jsr` of that clear is
`P:0x1A25`, inside startup `P:0x1A13`. The reset vector reaches startup
through `P:0x0000`, `P:0x0DBC` and `P:0x14004`. No scanned instruction
writes the flag from a counter. **Confirmed.**

`0x2A` stores only bit 0 of its byte at `X:0x17F4`. It does not write
`X:0x17A0`. **Confirmed.** Session off therefore leaves DSP bypass as it
was. The other session stores are `P:0x14282` (0, from reset startup; the
only long `jsr` is `P:0x1A21`) and `P:0x179C` (0). `P:0x179C` runs for
queued words `0x070001` and `0x070002` after `P:0x174D` subtracts one and
takes table entries 0 and 1. The only long `jsr` of that table is
`P:0x10D6`, which is the high-byte-7 arm of the queued-word decoder. A
repeat of the same low 16 bits is skipped when it matches `X:0x17A6`.
Those two words are enqueued at `P:0x1DF0` and `P:0x1DFA` when `X:0x1810`
is below `0x3B` or below `0x86`. What `X:0x1810` represents is **unknown**.
The arm does not write `X:0x17A0`.

Readers of `X:0x17F4` are `P:0x135E` (`0x28` only when the flag equals 1,
called from the timer at `P:0x0DDE`), `P:0x14A0D` (outgoing program change)
and `P:0x14A20` (outgoing CC). The SysEx header, `0x43` and `0x61` do not
read it. **Confirmed.** Neither command requires the session flag.

Program-change status `0xC0` reaches `P:0x14285`, which sets bits 16 and
18. The high byte of that queued word is 5. The decoder sends high byte 5
to `P:0x1114`, which stores the low 16 bits at `X:0x17A4` and calls
`P:0x11A0`. High byte 6 at `P:0x111B` is a different arm: it calls
`P:0x1FA7` and `P:0x1E9C` and does not add 55. Neither arm writes
`X:0x17A0`. **Confirmed.** `P:0x1E9C` calls `P:0x2001`,
which reads the bypass flag and skips its wait when the flag is nonzero.
The reload still does not store 0 to `X:0x17A0`.

The peripheral `reset` at `P:0x1ABA` clears the stack and jumps to
`P:0xFF0000`. Inside this image it does not write `X:0x17A0` and it does
not re-enter `P:0x0000`. "Reset entry" below means the vector path that
does write the flag. A measured power cycle is a separate question and is
**unknown**.

### What a set bypass flag does in the audio interrupt

`P:0x0DE4` calls `P:0x03F2` twice, and `P:0x03F2` calls `P:0x0593`.
**Confirmed.** `P:0x0593` keeps bit 20 of `Y:0xD0`, then ORs in `X:0x17FC`,
`X:0x17ED` and `X:0x17A0`. Any one of those four sends the interrupt to
`P:0x062B` and skips the model dispatch. At `P:0x062B` the test is bit 20,
`X:0x17FC` and `X:0x17A0`. Any one of those three jumps to `P:0x03DD`.
`X:0x17ED` is read after that jump, so it does not select `P:0x03DD`.
**Confirmed.** A nonzero bypass flag is sufficient for the short mix.
Clearing it leaves the short mix in place while bit 20 or `X:0x17FC` is
still set, and it leaves the model dispatch skipped while `X:0x17ED` is
still set.

`P:0x03DD` runs 16 groups. Each group clears the accumulator, multiplies
six input words by the immediate `0x055555`, and stores that accumulator
four times. The stores are not zeros. Listening level was not measured.
**Confirmed** for the instruction sequence. The audible result is
**unknown**.

`0x4C` and `P:0x0FA6` do not write the other three terms. **Confirmed**
for those windows. Startup `P:0x1A1F` calls `P:0x14BFC`, which writes 0
to `Y:0xD0`, and later `P:0x1A30` calls `P:0x14A64`, which writes `0x881`
(bit 20 clear). The same constant is written at `P:0x14B30`, `P:0x14B61`
and `P:0x14BA8`. `P:0x14BFD` writes 0. `P:0x201D` writes 0 at the end of
the reload wait, unless `X:0x17A5` is 1, in which case `P:0x2001` returns
before that store. Startup's following call at `P:0x1A32` stores 0 to
`X:0x17A5`. One queued word, high byte 8 and low 16 bits `0xFF`, stores 1
there at `P:0x112D`. Whether that word occurs during an editor session is
**unknown**.

The audio interrupt can set bit 20 after the mix returns. It divides a
saved value at `P:0x001A` and, when the quotient is at least `0x7F`,
executes `bset #20` at `P:0x0E6D` and writes 1 to `X:0x179F`. What the
dividend represents is **unknown**. That bit can become set while bypass
is already on, because the test runs after `P:0x03F2` returns.

`X:0x17FC` is outside the reset flag clear. `P:0x14A64` stores 1 and calls
`P:0x20AB` only when `X:0x1812` is above `0x80`. `P:0x20BC` then stores 0
when the value is 1. The other branch leaves `X:0x17FC` unchanged. Reset
entry therefore does not unconditionally clear it. `X:0x17ED` is not
written by `0x4C` or the store window.

### An interrupted store leaves the flag set

`store()` opens the transaction with DSP bypass, so the host sends
`0x4C 01` before `0x43` and `0x61`. Cleanup always attempts `0x4C 00`,
then session off, then bridge off, including when the body fails. That
attempt is a host send. `P:0x0FA6` does not write `X:0x17A0`.

The flash polls do not call the MIDI parser and do not mask interrupts
for the whole poll. **Confirmed.** The receive interrupt still queues
bytes at `X:0x131B`. The main thread does not parse that queue until the
poll returns, so a cleanup `0x4C 00` stays queued. The audio interrupt
still reaches the short mix. Sound returning requires the poll to return
and the bypass term to be clear, and the other terms above to be clear
as well. This image does not show that a stuck poll clears the flag.

While the trailing read of `0x4C` or `0x2A` is waiting, an empty queue
returns `0xFF` from `P:0x1234` and `P:0x1257` calls `P:0x1A97` before
trying again. `P:0x1A97` can transmit `0x74` and `0x4D`. The next byte
other than `0xFF` is consumed and is not compared with `F7`. A missing
trailer does not stick the way a flash poll does. One later byte unblocks
the reader, and that byte is not parsed as the start of a new message.
The flag is already stored before this wait.

### Trap list (bypass and session)

| Trigger | Address | Consequence | Our editor can send it | How the editor avoids it |
| --- | --- | --- | --- | --- |
| `0x4C` with a nonzero byte, then the link stops before a later byte 0, while the parser is still running. | `P:0x14563` | `X:0x17A0` stays nonzero until a later `0x4C` byte 0 or the reset entry. Session off and program change leave it unchanged, and no counter stores 0. Model dispatch is skipped and `P:0x03DD` keeps running its scaled sums. There is no reply. | Yes. `store()` sends `0x4C 01`. Cleanup sends byte 0 only when the host can still transmit. | Cleanup already sends DSP on before session off. A failed cleanup needs a later parsed `0x4C` byte 0, or the reset entry. A missing reply is the normal handler result. Bit 20 or `X:0x17FC` can keep the short mix after the flag is 0. |
| `0x4C 01` was accepted and a flash poll has not returned. | `P:0x0F17`, `P:0x0F45`, `P:0x0F63` | Cleanup bytes sit in the receive queue. The poll does not parse them, so the bypass flag stays set. The audio interrupt still runs the short mix. | Yes, when the part does not finish the poll. | The store rule already forbids another `0x61` in that group of 20. A queued `0x4C 00` applies only after the poll returns. Reset entry writes the bypass flag to 0. It does not unconditionally clear `X:0x17FC`. |
| The transfer stops after the `0x4C` or `0x2A` argument and before the trailing byte. | `P:0x14565`, `P:0x144E2` | The flag is already stored. The reader waits, calls `P:0x1A97` on each empty read, and consumes the next non-`FF` byte without an `F7` test. | Yes, if the transfer stops after the argument. | Send the closing `F7`. If it was lost, the next byte unblocks the reader and is not itself dispatched. |

## Flash erase, slot read, and the other program paths

The [flash trace](../research/variax-310-flash-handlers.json) pins these
handlers against the same archive. It records sector ranges and reply
opcodes. It does not record a code-load payload.

### One erase call addresses one sector

`P:0x0F63` forms `0xD00000 + index × 0x1000` and issues one erase at that
address. The stride is 4,096 bytes. One call does not walk into the next
sector. **Confirmed** for the command in this image. The schematic names
an SST39LF040, whose sectors are 4 KB. Whether a different fitted part
erases a larger block remains **unknown**.

The erase poll restarts the command when `X:0x179C` exceeds 488. The
byte-program helper restarts its command when its counter reaches 10,000.
Neither helper returns a failure code. **Confirmed.**

### Which pins a sector address drives

`P:0x0DBC` writes AAR0 `0x000539`, AAR1 `0xD00431`, AAR2 `0x000001` and
AAR3 `0x000001`. Nothing later in this image writes those four registers.
**Confirmed.** `P:0x14004` writes the bus control register `0x1F27C2` once.
That register sets wait states. It does not pick a different pin for one
of the paths below than for another.

The DSP56367 manual puts the address to compare in bits 23–12 and the
number of high bits to compare in bits 11–8. Bit 5 enables Y, bit 4
enables X, and bit 3 enables P. Bit 2 clear makes the pin active low.
Access type 01 is SRAM, and the AA pin is then a chip select rather than
an extra address bit. A compare count of 0 matches every address in the
enabled spaces.

AAR1 therefore matches X and Y, not P, for `0xD00000`–`0xDFFFFF`, and
asserts AA1 active low. AAR0 matches X, Y and P for `0x000000`–`0x07FFFF`
and asserts AA0. AAR2 and AAR3 enable no space, so they assert nothing. The external pin
list brings out AA0–AA2. **Confirmed** from the register words, those bit
fields, and the DSP56367 pin list. Every flash address below is in the
AAR1 window.

`P:0x0F63` and `P:0x20A5` form `0xD00000 + index × 0x1000`. Neither adds
an offset or a bank bit. **Confirmed.** The store sets bit 4 of
`Y:0xFFFF9D` at `P:0x0FA7` before it calls the helper, and clears it at
`P:0x0FCF`. The bit is not part of the address word.

`Y:0xFFFF9D` is the Port E data register. Bit 4 is pin PE4. The DSP56367
manual names that pin ESAI_1 FST when the port is a peripheral.
`P:0x1A13` calls `P:0x141CD`, which writes direction `0x000010` to
`Y:0xFFFF9E` and control `0x000000` to `Y:0xFFFF9F`, then clears the data
bit. The pin is a GPIO output. **Confirmed.** The Variax schematic notes
do not name the net, so what the pin selects on the board is **unknown**.

Port A brings out A0–A17, AA0–AA2, RD and WR. It has no X/Y pin. An X
access and a Y access that match AAR1 assert the same AA1 and the same
A0–A17. **Confirmed** for that pin list. Address bit 18 is not on A0–A17,
and AA1 is a chip select, so index and index + 64 present the same A0–A17
and the same AA1. When bit 4 is also the same, the pins this firmware
drives match.

| Path | Space | Bit 4 | Address |
| --- | --- | --- | --- |
| Names `P:0x1416B`, and boot's copy of that table | X for the erase and the 132-word program. Y for the marker byte and for boot's marker read. | Clear. `0x6C` does not set it. Boot clears it at `P:0x140E8` before the call. | Index 8, `0xD08000` |
| String levels `P:0x14142` | X for the erase and the program. Y for boot's three-byte compare. | Clear. `0x56` does not set it. | Index 4, `0xD04000` |
| Store `P:0x0FA6` and the loader copy at `P:0x1FE1` | X for the erase and the word program. Y for the zero byte and for the loader read. | Set for the whole store, and set around the loader copy. | `0xD00000 + index × 0x1000`, and the spare at index + `0x3C` |
| `0x44` flash walk | Y | Set around the walk. Index `0x7F` does not set it. | The same formula. Index `0x7F` reads `Y:0xA47`. |
| `0x62` | Y byte program | Set | Indices 0–59 |
| `0x63` | X erase | Set | Indices 64–127 |
| `0x68` and both code-load erases | X erase | Clear. The code-load windows clear it and do not set it. `0x68` does not write it. | Indices 32–63, 16–31, and 0–3 |

The name path and the loader's index 8 drive the same AA1 and the same
A0–A17, and a different level on PE4. Whether that level reaches a
different device is **unknown**. Calling them one physical sector is
unresolved; contradicted by owner backup (program 63 loads a distinct
valid program). The editor's program number for flash index 8 is 63.
**Confirmed** from `Preset::program_number`. Under the integer reading
already recorded for `P:0x11A0`, that request reaches the loader as
argument 63, subtracts 55, and reads index 8 with the bit set.
**Likely** for that idiom. The owner's 2026-11-24 backup, 60 of 60
programs, read program 64 as T-Model 1 and then program 63 as Telecaster
Thin, body `0x03100002`. A checksum miss would have kept the previous
buffer.

The same label covers the other clear-versus-set pairs: string-level
index 4 against preset index 4, the code-load ranges against preset
indices 0–3 and 16–31, and `0x68`'s indices 32–63 against the store's
indices 32–63 and spares 60–62. `0x62` sets the bit, so indices 0–59
drive the same pins as the store. `0x63` sets the bit, and index `64 + n`
drives the same A0–A17 as index `n`. The editor sends none of `0x62`,
`0x63`, `0x68`, or the code-load opcodes.

### Clear PE4, and who leaves it set

PE4 is bit 4 of `Y:0xFFFF9D`. Four windows erase or program while that bit
is clear: bank names, string levels, `0x68`, and both code-load erases.

| Path | What this image does with PE4 | Label |
| --- | --- | --- |
| Bank names `0x6C`, commit `P:0x1416B` | No set and no clear in the handler or the commit. Boot clears the bit at `P:0x140E8` before its own call. | **Confirmed.** Relies on the bit already being clear. |
| String levels `0x56`, commit `P:0x14142` | No set and no clear. The same boot clear precedes the string-level compare and that boot call. | **Confirmed.** Relies on the bit already being clear. |
| `0x68` | The 32–63 erase walk does not write the bit. | **Confirmed.** Relies on the bit already being clear. |
| Both code-load erases | Each window clears the bit, then erases. Neither window sets it. | **Confirmed.** Clears explicitly. |

The image sets this bit at the store, the loader copy, the `0x44` flash
walk, `0x62`, `0x63`, the two checksum Y reads, and the one-byte Y read
inside `P:0x141AA`. Each of those clears it before its own return. The
five paths below are the ones that hold it across a poll.

| Path | Return to the parser | Stall with PE4 still set | Label |
| --- | --- | --- | --- |
| Store `P:0x0FA6` | Clears at `P:0x0FCF` on the only path back. | The program and erase polls spin until the part answers. They do not read the MIDI queue. | **Confirmed.** |
| Loader copy `P:0x1FE1` | Clears at `P:0x1FF9` after the copy. A checksum miss never sets the bit. | The copy is a RAM loop. It does not poll the part or the host. | **Confirmed.** No stall, and no return with the bit set. |
| `0x44` flash walk | Clears at `P:0x148D2` after the walk, then queues the trailer. Index `0x7F` never sets the bit. A trailer other than `F7` returns before the walk. | `P:0x1241` spins while its 64-entry ring is full. That spin does not call the parser. The transmit interrupt drains the ring and returns without parsing. The receive interrupt only enqueues. A host that stops reading leaves the walk in that spin with the bit set. | **Confirmed.** |
| `0x62` | Clears at `P:0x14796` after the sector walk. | The payload wait spins without the idle parser. The byte-program poll can restart with the bit still set. | **Confirmed.** |
| `0x63` | Clears at `P:0x0F97` after its walk. | The erase poll can spin with the bit still set, and it does not parse MIDI. | **Confirmed.** |

The family write API sends `0x6C` from `rename_bank`. That is the only
clear-PE4 write among its commands and the #225 slot-read commands. The
editor also sends `0x61` and program change. Both set the bit and clear
it before they return. The editor does not send `0x44`, `0x56`, `0x62`,
`0x63`, `0x68`, or either code-load opcode.

The #225 tool sends a universal identity request (`F0 7E 7F 06 01 F7`),
bridge on, session on, `0x44` for `0x7F`, then `0x32`, then `0x3E`, then
session off and bridge off. The identity request matches `F7` at
`P:0x142E3` and replies without setting PE4. The tool waits until each
slot reply ends in `F7` before the next request. It does not send `0x6C`,
`0x56`, `0x68`, or a code-load opcode.

A host that stops reading during a flash `0x44` stalls in the ring wait
with the bit set. A rename queued behind that reply is not parsed until
the walk returns, and that return has already cleared the bit. A rename
queued during a store poll waits the same way. **Confirmed:** neither
sequence enters the name commit while PE4 is set.

No sequence built only from those two senders' commands enters a
clear-PE4 write while the bit is set. **Confirmed.** `0x6C`, `0x56` and
`0x68` would keep a set bit if some earlier command returned with it
set. These senders do not have such a command. Startup in this image
clears the bit at `P:0x141CD` and again at `P:0x140E8` before the name
and string reads. A power cycle was not measured.

### What erases sectors 60–62

A completed `0x61` still erases one spare at the end of `P:0x0FA6`, after
`add #$3C`, while bit 4 is set. **Confirmed** from the store trace. For
slot 50 that spare is index 62 at `0xD3E000`.

`0x68` at `P:0x146C0` erases numeric indices 32–63 before the trailer,
and it does not set bit 4. The numeric range includes 60–62.
**Confirmed** for that DSP loop. It does not drive the pins the store
uses for those indices. The physical overlap is unresolved; contradicted
by owner backup (program 63 loads a distinct valid program). No opcode
erases spare 62 alone while the bit is set.

`0x63` at `P:0x146BD` erases numeric indices 64–127 before the trailer,
with the bit set. The numeric range leaves 60–62 alone. **Confirmed.**
Index `64 + n` drives the same A0–A17 as index `n`, with the same bit
level as a store. A matching trailer then sends `0x70`. The editor does
not send `0x63`.

The erase-helper callers in this image are the restart branch inside
`P:0x0F63`, the 64–127 walk, the 32–63 walk, the slot erase and the spare
erase inside `P:0x0FA6`, the two code-load ranges, numeric index 4, and
numeric index 8. **Confirmed.**

`0x62` walks numeric indices 0–59, with the bit set, and calls the
byte-program helper only. It does not erase, and it does not address
60–62. Those indices drive the same pins as the store. **Confirmed.**
`0x67` does not erase. Its program addresses stay below `0x21000`.
**Confirmed.** `0x59` requires `F7` first, reads sectors 5–54, and
byte-programs below `0x21000`. It does not erase. **Confirmed.** The
`0x59` read sets the bit only around each Y load and clears it before
the byte program.

Startup `P:0x1A13` calls `P:0x140E8` and does not call either wide erase.
Inside that boot function, a `0xFF` byte near the end of sector 31 selects
a four-byte program of the factory version bytes onto that same end of
sector 31. There is no erase, and the addresses are not sectors 60–62.
**Confirmed.** The peripheral `reset` at `P:0x1ABA` leaves this image. It
was not shown to erase 60–62 here.

The editor's family write API does not send `0x63` or `0x68`. The names
in `src/simulation/receiver.rs` are a receiver model. The historical
receive order is in [update research](update-recovery.md).

### Read of one stored slot (`0x44`)

`0x44` at `P:0x147F2` reads one index byte and one trailer byte. A trailer
other than `F7` returns without a read. `F7` enters `P:0x1488C`, which
sends reply opcode `0x45` and then the saved index. **Confirmed.**

Index `0x7F` sends 1,092 words from `Y:0xA47`, six nibbles each. Any other
index calls `P:0x20A5` and walks 3,276 byte addresses. Each step reads the
low nibble and the high nibble of the same byte, then advances. Those
3,276 bytes are the first bytes of the sector and are the complete
1,092-word stored program. The last 820 bytes of the 4,096-byte sector
are not sent. **Confirmed.**

The only special case is index `0x7F`. That arm reads `Y:0xA47` and does
not set bit 4. Index 50 and index 62 both take the flash walk. Index 50
is `0xD32000`. Index 62 is `0xD3E000`. Both reads set the bit, so they
drive the same pins as the store of slot 50 and as that slot's spare.
**Confirmed** for the pins. Past index 127 the same walk still runs. What
the part returns is **unknown**.

Index 62 is a read of the 1,092 words the next store programs into the
spare. The walk does not erase or program, and it does not require the
sector to be blank. A host can compare those bytes with the erased state
before storing. The firmware does not make that comparison. The last 820
bytes of the sector are not returned, and the store does not program
them. Index 50 returns the 1,092 words now stored in the slot, including
the stamp and the firmware checksum written by `0x61`. It is not a
byte-for-byte check of the host image. The editor does not send `0x44`.

The `0x44` body and `P:0x1488C` do not call `P:0x0F17`, `P:0x0F45`,
`P:0x0F63` or `P:0x0FA6`. **Confirmed.** In the flash walk the only
peripheral-register write is bit 4 of `Y:0xFFFF9D`, set at `P:0x148BC`
and cleared at `P:0x148D2`. Reply bytes are queued at `X:0x141B` by
`P:0x1241`. That helper and the header helper `P:0x1367` do not write a
peripheral register. The trailer wait before the walk can call
`P:0x1A97`. That is the idle sender already listed, and it runs before
the flash read.

The family write API does not send `0x44`. One branch of `0x6F` transmits
a bulk read that starts at `0xD00000` and does not erase. Its length and
its selector stay in the older update note. The code-load erases hold
bit 4 clear, so they do not drive the pins of the preset indices with
the same numbers.

### Conditional store `0x45`

`P:0x1480A` receives 1,092 words into `Y:0xA47` and then checksums them.
A match calls `P:0x0FA6` before the trailer is read, sends `0x71`, and
only then reads the trailer. There is no index clamp. A trailer other
than `F7` after that store sends `0x66`. A matching trailer reloads
`X:0x1825`, the index that was already current, rather than the index
just stored. **Confirmed.**

A checksum mismatch sends `0x72` and does not store. The new words are
already in `Y:0xA47`. A matching trailer then reloads the old slot over
them. **Confirmed.** The editor does not send `0x45`. The byte `0x45`
programmed at `0xD081FE` by the bank-name helper is flash data, not this
opcode.

### Bulk store `0x58`

After `F7` (otherwise `0x66`), `P:0x145BD` stores `0x100000` at `Y:0xA48`
and copies that flag before any store. Argument `0x7F` loads and stores
indices 5–54. Those loader arguments are the compact copy, and the store
still runs. An argument below 50 stores the one sector at argument + 5.
An argument of 50 or more, other than `0x7F`, does not store and does not
send `0x71`. The `0x7F` and below-50 paths send `0x71` and then reload.
**Confirmed.** Spares 60–62 are outside both ranges. One `0x7F` message
can rewrite sectors 5–54, including slot 50. The editor does not send
`0x58`.

### Received programs `0x62` and `0x67`

`0x62` at `P:0x1474A` sets the same flag, copies it, and then programs
sectors 0–59 one byte at a time while it is still receiving. There is no
erase, and sectors 60–62 are not addressed. After indices 19, 39 and 59
it reads 32 further bytes before the next sector, which the update note
already records. An interrupted transfer has already issued program
commands for the bytes it received. The poll restarts rather than
returning. Sectors not yet entered are not addressed. **Confirmed.**

`0x67` at `P:0x14798` sets the flag, copies it, and programs bytes whose
addresses stay below `0x21000`. There is no erase, and the walk does not
reach `0xD00000`. **Confirmed.** The editor does not send either opcode.
On both of these paths the bytes that arrive during the program calls are
the payload.

### Copy `0x59`

`P:0x147D8` requires `F7` first. A missing trailer sends `0x72` and does
not copy. A matching trailer sets the flag, copies it, reads sectors
5–54, and byte-programs below `0x21000`. There is no erase. It then
reloads the saved index and sends `0x71`. **Confirmed.** The editor does
not send `0x59`.

### Bank names `0x6C` and `0x6D`

`0x6C` at `P:0x14623` loads `X:0x1825`, divides by 5, and applies a bare
`maci`. Which six RAM words receive the 18 name bytes is **unknown**:
that scale was not measured. The handler assembles six Y words, calls
`P:0x1416B`, and only then reads the trailer. **Confirmed.**

`P:0x1416B` erases numeric index 8 through X, programs 132 words from
`Y:0x6D5` to `0xD08000` through X, and programs one marker byte `0x45` at
`0xD081FE` through Y. Bit 4 stays clear. The flash destination does not
depend on the unmeasured RAM offset. The whole table is written. 132
words are 396 bytes, and the marker sits past that program.
**Confirmed.** The loader reads preset index 8 through Y with the bit
set. That overlap is unresolved;
contradicted by owner backup (program 63 loads a distinct valid program).

The rename does not use a spare and does not call `P:0x0FA6`. The sector
immediate is 8 for every call, including a rename while bank 0 is
current. Editor index 8 is bank 1, slot 1, and its program number is 63.
A raw loader argument of 8, with the bit set, is the compact path. The
name commit does not take that path.

`0x6D` at `P:0x14650` checks `F7` and replies opcode `0x6D` from the RAM
table. That window has no flash call. **Confirmed.** When the marker byte
is `0x45`, boot copies `0xD08000` back into `Y:0x6D5` through X and does
not erase. When the marker is some other value, startup erases numeric
index 8 with the bit clear and writes default names. A completed rename
leaves the marker, so a later boot copies. **Confirmed.** Not every boot
erases that window. The marker read is a Y read with the bit clear. It
is not the loader's bit-set read of index 8.

`rename_bank` sends `0x6C` after a program change to that bank's slot 0.
A transfer that delivers the 18 name bytes and then stops still erases
the name window, because the erase is before the trailer. Stopping
before those 18 bytes leaves the handler in the byte reader and does not
erase. **Confirmed.** The current editor does not avoid the name-window
erase. It does not, on this evidence, erase preset index 8.

### String levels `0x54`, `0x56` and `0x57`

`0x54` at `P:0x144E3` receives six words into `Y:0x2D8` and jumps to the
shared trailer used by `0x6C`. That window has no flash call.
**Confirmed.** `0x57` checks `F7` and replies opcode `0x57` plus those
six words. It does not touch flash. **Confirmed.**

`0x56` at `P:0x14583` checks `F7` before `P:0x14142`. A missing trailer
sends `0x66` and does not erase. The commit erases numeric index 4
through X, with bit 4 clear, and programs six words from `Y:0x2D8` to
`0xD04000`. **Confirmed.** Preset index 4 is the same formula with the
bit set. That overlap is unresolved; contradicted by owner backup
(program 63 loads a distinct valid program). The erase clears the window
this access drives. Only those six words are programmed back. There is
no spare.

Boot at `P:0x140E8` clears bit 4, then assembles three Y bytes from
`0xD04000` and compares them with `m0`. An equal compare copies six
defaults and calls the same erase. The image's `movec m7,m0` sites are
`P:0x0E00`, `P:0x1EF0` and `P:0x14148`. None of those is before the
compare inside `P:0x140E8`. Startup calls `P:0x04CF` before `P:0x140E8`.
Whether that call writes `m0` was not followed. **Unknown.** A family
manual gives reset `m0` as `$FFFFFF`. That value is not stored in this
image. If `m0` is `$FFFFFF`, the compare matches three `0xFF` bytes of
this bit-clear read. **Likely** for that reset value, and **unknown** if
a board boots with a different `m0`. This read is not the loader's
bit-set read of preset index 4. `P:0x14150` copies the bit-clear window
into `Y:0x2D8` through X and does not erase.

`P:0x14BC9` also calls the sector-4 commit. Nothing in this image calls
or jumps to that address. The word before it is a short conditional
branch, so the call is reached by fall-through when that branch is not
taken. The message that falls through was not identified. The family
write API does not send `0x54` or `0x56`.

### Code load, listed only

`0x60` at `P:0x14687` erases and programs numeric indices 16–31 only after
the trailer and a matching sum, with bit 4 clear. A mismatch sends `0x73`
and does not erase. After the reply it calls the peripheral reset at
`P:0x1ABA`. The range does not include 60–62. Those indices do not drive
the pins of preset indices 16–31. The overlap is unresolved; contradicted
by owner backup (program 63 loads a distinct valid program). The editor
must not send `0x60`.

`0x6F` at `P:0x1468B` has a read branch that does not erase and does not
reset, and a commit branch that erases and programs numeric indices 0–3
only after the trailer and a matching sum, with bit 4 clear, then resets.
Any other value returns without either branch. Neither range includes
60–62. The same overlap label applies to indices 0–3. The editor must
not send `0x6F`. The selector byte and the payload layout stay out of
this note. The older update note already records the historical order.

### Reset `0x7D`

`P:0x1469B` calls `P:0x1ABA` before it reads the trailer. Inside this
image that path does not erase sectors 60–62. Whether the boot ROM erases
flash is **unknown**. The editor does not send `0x7D`. A transfer that
never reaches `F7` has already taken the reset.

### Version stamp

The only program word that addresses `Y:0xD46` is the four-word copy in
`0x61` at `P:0x1473B`. No program word addresses `Y:0xD47`, `Y:0xD48` or
`Y:0xD49`. The checksum adds those words with the rest of the buffer and
does not compare them with the version text. Other uses of the factory
source at `Y:0x151BB` copy those bytes into replies or into the sector-31
marker program. They do not read the stamp back from a stored slot.
**Confirmed.**

### Checksum failure and one queued store

On a checksum mismatch, `P:0x1FA7` stores `0x100000` at `Y:0xA48` and
returns without the 1,092-word copy. The previous buffer stays. That
failure path does not call `P:0x2090`. The reload at `P:0x1E9C` saves the
flag, clears bit 1, copies through `P:0x2090`, restores the saved flag,
and copies again. `P:0x2090` moves `Y:0xA48` to `Y:0xD0`. A failure value
of only bit 20 therefore reaches `Y:0xD0`. Bit 20 selects the short mix
at `P:0x03DD`. Listening level remains **unknown**. The buffer is
unchanged, and model dispatch is skipped once the flag has been copied.
**Confirmed** for that flag path.

Arguments 5–54 of `P:0x1FA7` take the compact copy and do not checksum.
Arguments 0–4 and arguments above 54 checksum a slot sector. Arguments
above 59 subtract 55 first. Startup `P:0x1A13`–`P:0x1A34` does not call
`P:0x1FA7`. What boot plays for a corrupt slot was not found.
**Unknown.** `0x58`, `0x59`, `0x62`, `0x67` and the code-load helpers also
store `0x100000` at `Y:0xA48` on purpose. Those stores are not the loader
failure.

`P:0x11A0` forms a value from `X:0x17A8` and `X:0x17A4` with `asl`,
`maci #5` and `asr`. It adds 55 when that value is 5 through 54. It then
always calls `P:0x1FA7`, `P:0x1E9C`, the outgoing program-change helper
and the three-byte MIDI helper. A formed value of 50
becomes loader argument 105, which is above 59, subtracts 55, and
checksums sector 50. Bare `maci` was not measured. The integer reading
`5 × X:0x17A8 + X:0x17A4` is **likely**. A different scale is
**unknown**. Startup clears `X:0x17A8`. The editor's program number for
flash index 50 is 105. **Confirmed** from `Preset::program_number`.
Whether that program change produces helper value 105 is **likely** only
under the integer reading, with `X:0x17A8` still 0. Direct high byte 6
does not add 55, so an argument of 50 there is the compact path.

`P:0x1186` is fall-through in the queued-word decoder, not a SysEx
opcode. For one low-16-bit case it writes `0x100000` to `Y:0xD0`,
checksums, and calls `P:0x0FA6` when the formed index is below 5 or is
55–59. Indices 5–54 skip that store. That arm is high byte 8 with the
low 16 bits equal to 1, and it runs only when `X:0x17A5` is already 1.
Program-change status `0xC0` is queued as high byte 5 and enters
`P:0x1114`. Controller `0x4F` and controller 7 go to `P:0x1428E`.
**Confirmed.** Neither path enters `P:0x1186`. The family write API sends
program change and controller 79, and it does not build the high-byte-8
word. `P:0x14197` would store indices 5–54 and has no call, branch or
literal reference in this image. It is not a dispatch entry.

### MIDI that arrives during a flash poll

Inside `P:0x0FA6` the helpers are the divide, `P:0x0F17`, `P:0x0F45` and
`P:0x0F63`. Those polls do not parse the MIDI queue. Bytes stay at
`X:0x131B` until the poll returns, and the parser then treats them as the
next MIDI message. **Confirmed** for `0x61`. Whether a full receive queue
drops a byte was not measured again. **Unknown.** `0x62`, `0x67`, and the
code-load receives consume arriving bytes as payload while they program.

### Trap list (flash commands)

| Trigger | Address | Consequence | Our editor can send it | How the editor avoids it |
| --- | --- | --- | --- | --- |
| `0x6C` with its 18 name bytes, including a transfer that stops before `F7`. | `P:0x14645`, `P:0x1416B` | Erases numeric index 8 with bit 4 clear and programs 132 words plus the marker. The spare is not used. The erase is before the trailer. Preset index 8 is the bit-set read. That overlap is unresolved; contradicted by owner backup (program 63 loads a distinct valid program). | Yes. `rename_bank` sends `Message::BankName`. The preset-index consequence is not established. | The current code does not avoid the name-window erase. Stopping before the 18 data bytes does not erase. A completed write leaves the marker, so the next boot copies the table instead of erasing again. |
| `0x56` after `F7`. | `P:0x1458A`, `P:0x14142` | Erases numeric index 4 with bit 4 clear and programs six words. The overlap with preset index 4 carries the same unresolved label. | No. The family write API does not send `0x54` or `0x56`. | Do not send it. A missing `F7` sends `0x66` and does not erase. |
| Fall-through into `P:0x14BC9`. | `P:0x14BC9` | Calls the same sector-4 erase. The message that reaches the fall-through was not identified. | Unknown. | Do not claim the editor avoids a path that was not identified. |
| `0x45` whose 1,092-word checksum matches. | `P:0x1481E` | Runs the full `0x61` store before `F7`, with no index clamp. A later bad trailer sends `0x66` after the store. | No. | Do not send `0x45`. |
| `0x58` with argument `0x7F`, or with an argument below 50. | `P:0x145BD` | `0x7F` stores sectors 5–54. An argument below 50 stores argument + 5. The flag is set before the store. | No. | Do not send `0x58`. |
| `0x68` or `0x63`, including a transfer that stops before `F7`. | `P:0x146C0`, `P:0x146BD` | `0x68` erases numeric indices 32–63 with bit 4 clear. The overlap with the store's indices 32–63 and spares 60–62 is unresolved; contradicted by owner backup (program 63 loads a distinct valid program). `0x63` erases numeric indices 64–127 with the bit set, and index `64 + n` drives the same A0–A17 as index `n`. Both erases precede the trailer. | No. | Do not send either opcode. Neither is a single-sector repair. |
| `0x62` while bytes are still arriving. | `P:0x1474A` | Programs numeric indices 0–59 with bit 4 set, the same pins as the store, and without an erase. Bytes already received have been programmed. Later sectors are not addressed. | No. | Do not send `0x62`. |
| `0x60` or the commit branch of `0x6F`, after a trailer and a matching sum. | `P:0x14687`, `P:0x1468B` | Erases sectors 16–31 or 0–3, programs them, and then takes the peripheral reset. A sum mismatch sends `0x73` and does not erase. | No. | Do not send either opcode. Both ranges miss sectors 60–62. |
| `0x7D`, including a transfer that never reaches `F7`. | `P:0x1469B` | Calls the peripheral reset before the trailer. This image does not erase 60–62 on that path. Boot-ROM flash behavior is unknown. | No. | Do not send `0x7D`. |
| The queued word that falls through `P:0x1186` for a formed index of 0–4 or 55–59. | `P:0x1186` | Writes bit 20 to `Y:0xD0`, checksums, and stores that index. | No. Program change is high byte 5. The editor's tone CC is controller 79 and goes to `P:0x1428E`. | Do not build a high-byte-8 word whose low 16 bits are 1. |

## Remaining command handlers

The [remaining-handler trace](../research/variax-310-rest-handlers.json)
pins the last 24 command entries against the same archive. The checker
scans fixed windows for a direct `jsr` to `P:0x0F17`, `P:0x0F45`,
`P:0x0F63`, `P:0x0FA6`, `P:0x0F8C`, `P:0x0F9A` or `P:0x1ABA`. Those windows
contain none. A branch that leaves a window is not followed. **Confirmed**
for the scanned windows.

`0x3E` is inside its window and still unresolved. On `F7`, `P:0x14430`
is `jsr (r6)`. The pointer comes from `X:0x1A88`. That table was not
recovered, so whether the call stores is **unknown**. When the call
returns, `P:0x14431` jumps to `P:0x14805`, which sends `0x71`.

| Opcode | Handler | Confirmed action in this image |
| --- | --- | --- |
| `0x69` | `P:0x14394` | The first byte must be 1 and the next byte must be `F7`. Otherwise the handler returns at `P:0x1483E` and sends nothing. A match jumps to `P:0x1483D`, which calls the reply at `P:0x1493A`. The reply opcode is `0x69`. |
| `0x6A` | `P:0x143A4` | The same two-byte gate, then `P:0x14997` replies with opcode `0x6A`. |
| `0x4E` | `P:0x143B5` | The same two-byte gate, then `P:0x149CE` calls the three-byte MIDI helper with immediate `0x4F`. There is no SysEx reply. The helper transmits only when the session flag is 1. |
| `0x6B` | `P:0x143C6` | The same two-byte gate, then `P:0x149D5` replies with opcode `0x6B`. |
| `0x3E` | `P:0x143D7` | Words are stored through a pointer from `X:0x39E` before `F7`. On `F7` the indirect call runs. When it returns, `0x71` is sent. Outside the compared ranges the handler drains more words and loops. The indirect target is unknown. |
| `0x3F` | `P:0x1446C` | Three words are read and range-checked. Out of range sends `0x72`. In range the reply opcode is `0x3F`, followed by nibbles from the `X:0x39E` pointer. The reply does not wait for `F7`. |
| `0x1C` | `P:0x144F6` | `P:0x1378` receives six word pairs and one byte into RAM, then the shared trailer. A trailer other than `F7` sends `0x66`. |
| `0x16` | `P:0x144FA` | `P:0x170E` receives 60 words into `X:0x185A`, then the shared trailer. |
| `0x5E` | `P:0x144FE` | `P:0x16F6` receives 240 words into `X:0x1896`, then the shared trailer. |
| `0x5F` | `P:0x14502` | `P:0x1702` receives 240 words into `X:0x1986`, then the shared trailer. |
| `0x17` | `P:0x14506` | `P:0x1AC6` receives 12 words into `Y:0xCEF` before the trailer. After `F7`, `P:0x1AEC` runs. A missing `F7` sends `0x66` and skips that call. |
| `0x10` | `P:0x1451D` | `P:0x14A34` receives one byte, 24 words and one more byte before the trailer. After `F7`, `P:0x13B0` runs. |
| `0x49` | `P:0x1452A` | Two words update `Y:0xA48` before `F7`, and the update clears bit 20. After `F7`, `P:0x2090` copies the flag to `Y:0xD0`. |
| `0x40` | `P:0x14545` | One word replaces `Y:0xA48` before `F7`. After `F7`, the same copy runs. A replacement with bit 20 selects the short mix only after that copy. |
| `0x41` | `P:0x14554` | `F7` is required first. The reply opcode is `0x41`, followed by six nibbles of `Y:0xD1`. |
| `0x36` | `P:0x14513` | One word is stored at `X:0x1827`, then the handler joins the `0x35` trailer. |
| `0x37` | `P:0x14518` | One word is stored at `Y:0xA6B`, then the same trailer. |
| `0x35` | `P:0x14569` | Nine words are stored from `Y:0xC69`. After `F7`, `P:0x14B0` runs. |
| `0x1D` | `P:0x145FE` | Six groups of three raw bytes are stored at `Y:0xD9A`, then the shared trailer. |
| `0x1E` | `P:0x14618` | `F7` is required. A missing trailer returns at `P:0x1483E` and sends nothing. The reply opcode is `0x1E`, followed by the six words at `Y:0xD9A`. |
| `0x23` | `P:0x1465B` | `P:0x14A58` receives 52 words into `Y:0xCBB` before the trailer. After `F7`, `P:0x17CE` runs. |
| `0x29` | `P:0x1469D` | `F7` is required. The reply opcode is `0x29`. This handler is the next instruction after `0x7D`'s reset call. The reset leaves this image, so that fall-through is not shown to run. |
| `0x75` | `P:0x146CF` | The first byte is masked to four bits. `F7` is required. The reply opcode is `0x75`, followed by nibbles from X memory. |
| `0x47` | `P:0x147FE` | A count of 8,195 raw bytes is read through `P:0x1257`. There is no `F7` test. The loop ends at `P:0x14804` and execution falls into the shared `0x71` at `P:0x14805`. |

The family write API does not send any of these 24 opcodes. The editor
header is `F0 00 01 0C 07`, so its first data byte is `0x00`.

### The `0x7E` header branch

At `P:0x142B7` the first byte after `F0` is compared with `0x7F` and then
with `0x7E`. A byte of `0x7F` returns at `P:0x1483E`. Any byte other than
`0x7E` takes the ordinary family path at `P:0x142F5`. **Confirmed.** On
the `0x7E` path, a later byte that is `F7` continues. The saved value
`0x6E` calls `P:0x14869`, whose reply opcode is `0x6E`. Any other saved
value calls `P:0x14840`, which starts `F0 7E`. Both are replies to that
header. A byte other than `F7` at `P:0x142E3` takes `P:0x142E6`, and that
instruction branches to itself. **Confirmed.**

### Trap list (remaining handlers)

| Trigger | Address | Consequence | Our editor can send it | How the editor avoids it |
| --- | --- | --- | --- | --- |
| `0x47`. | `P:0x147FE` | Reads 8,195 raw bytes with no trailer check, then sends `0x71`. A shorter transfer leaves the reader taking the following MIDI. A store wait treats an 8-byte `0x71` as success. | No. | Do not send `0x47`. |
| `0x3E` that reaches `F7`. | `P:0x14430` | Indirect call whose target was not found, then `0x71` when that call returns. | No. | Do not send `0x3E`. |
| `0x40` or `0x49` with a matching `F7`. | `P:0x14547`, `P:0x1452A` | Copies the updated flag to `Y:0xD0`. `0x40` can set bit 20. `0x49` clears bit 20 before the copy. The RAM update precedes `F7`. The copy follows `F7`. | No. | Do not send either opcode. |
| A first header data byte of `0x7E`, then a byte other than `F7` at `P:0x142E3`. | `P:0x142E6` | The instruction branches to itself. | No. The editor header's first data byte is `0x00`, so `P:0x142BF` takes the ordinary family path. | Do not send a first data byte of `0x7E`. |

## Answers for #177

[#177](https://github.com/guirossibrum/VariaxWorkbench/issues/177) will store
the Reso-5 factory program at flash index 50. That index shares spare
sector 62 with flash indices 40–59. Each answer below cites the static
evidence and a confidence of confirmed, likely, or unknown. The
preset-read limit pass does not close another question. `0x3C` reads the
current Y buffer. The session pass answers questions 1, 9 and 11. The
flash pass answers questions 2, 3, 4, 5, 6, 7, 8, 12 and 13. Question 10
is answered with the remaining handlers. Audible boot, the short-mix
listening level, the bare `maci` scale, receive-queue overflow and the
`0x3E` indirect target were looked for and stay unknown. No question was
left unread.

### 1. What clears DSP bypass, and can an interrupted store leave the guitar silent?

`0x4C` byte 0 stores 0 at `X:0x17A0` (`P:0x14563`). **Confirmed.** The
handler does not mask the byte, so any other byte leaves the flag nonzero.
`0x2A 00` writes bit 0 of `X:0x17F4` only. **Confirmed.** It does not clear
bypass. No counter in the handler or elsewhere in the `0x17A0` extension
list writes the flag, so this image has no timeout for it. **Confirmed.**
Program change sets high byte 5 at `P:0x14285`. That arm enters `P:0x1114`
and does not write `X:0x17A0`. High byte 6 at `P:0x111B` is a different arm
and also does not write the flag. **Confirmed.**
When the reload at `P:0x1E9C` does run, it can clear `Y:0xD0` when `X:0x17A5` is 0, which is the
value startup stores at `P:0x1A32`. That still leaves the bypass flag set,
so the short mix stays selected. Reset entry stores 0 at `P:0x14023`.
**Confirmed** for that path. The peripheral `reset` at `P:0x1ABA` does not
write the flag inside this image. A measured power cycle is **unknown**.

An interrupted store does not itself write `X:0x17A0`. If the host has
already sent `0x4C 01` and a flash poll has not returned, the flag stays
set and a queued `0x4C 00` is not parsed. **Confirmed.** The audio
interrupt still runs `P:0x03DD`, which stores scaled accumulator values,
not zeros. Whether that is silence was not measured. **Unknown.** Sound
through the model path returns when the bypass flag is 0 and bit 20 of
`Y:0xD0`, `X:0x17FC` and `X:0x17ED` are clear. `0x4C` byte 0 clears only
the bypass term. The audio interrupt can set bit 20 on its own.

### 9. Does `0x61` require the session flag? Does `0x43`?

No. The SysEx header, `P:0x14668`–`P:0x14690` and
`P:0x14731`–`P:0x1474A` plus `P:0x0FA6`–`P:0x0FD2` do not read `X:0x17F4`.
**Confirmed.** The flag gates the three outgoing helpers listed above.
The editor's transaction turns the session on before either command anyway.

### 11. `0x3B`: when is `0x71` sent, and what does a count of 0 do?

From the buffer trace already checked against this archive. `F7` at the
run loop branches to `P:0x14466`, which transmits `0x71` and continues to
the reload at `P:0x1483A`. **Confirmed.** A count of 0 writes nothing on
the typed emulator used for that trace. The following bytes are parsed as
another run. **Confirmed** for that emulator. `store()` does not send
`0x3B`.

### 2. Which opcode returns a stored flash slot, and is that read only?

Request opcode `0x44` at `P:0x147F2` produces reply opcode `0x45`.
**Confirmed.** A trailer other than `F7` returns before any read. Index
`0x7F` sends 1,092 words from `Y:0xA47` as six nibbles and does not set
bit 4. Every other index, including 50 and 62, walks the first 3,276
bytes of `0xD00000 + index × 0x1000` through Y while bit 4 is set. That
is the complete 1,092-word program. The last 820 bytes of the sector are
not sent. **Confirmed.** Index 50 is `0xD32000`, the slot the store
writes for editor slot 50. Index 62 is `0xD3E000`, the spare that store
erases. Both drive the same pins as that store. Index 62 is a read-only
check of the 1,092 words the next store programs into the spare. It does
not test blankness itself. Index 50 returns the stored words, including
the stamp and the firmware checksum, so it is not a byte-for-byte check
of the host image. There is no further clamp. Past index 127 the walk
still runs, and what the part returns is **unknown**.

The handler and `P:0x1488C` do not call the program or erase helpers.
**Confirmed.** The flash walk's only peripheral-register write is bit 4
of `Y:0xFFFF9D`. Reply bytes are queued in X RAM by `P:0x1241`, which
does not write a peripheral register. The pin is PE4, a GPIO output, and
its board net is **unknown**. Nothing in this note is a request to send
`0x44`. The family write API does not send it.

### 3. Does anything other than a completed store erase sectors 60–62?

A completed `0x61` erases one spare at the end of `P:0x0FA6`, with bit 4
set. For slot 50 that is index 62 at `0xD3E000`. **Confirmed.** `0x68`
erases numeric indices 32–63 before `F7` with the bit clear. The numeric
range includes 60–62. It does not drive the pins of the store's spare.
That overlap is unresolved; contradicted by owner backup (program 63
loads a distinct valid program). `0x63` erases numeric indices 64–127
with the bit set. The numeric range leaves 60–62 alone, and index
`64 + n` drives the same A0–A17 as index `n`. **Confirmed** for the
loops and the pins. The two code-load ranges are numeric indices 16–31
and 0–3, with the bit clear, so they do not drive the preset indices of
those numbers. Startup `P:0x140E8` does not call either wide erase. The
four-byte version program near the end of sector 31 does not erase. The
peripheral reset leaves this image and was not shown to erase 60–62
here. **Confirmed** for those windows. No opcode erases spare 62 by
itself while the bit is set.

### 4. Where does a store stop when the spare is not blank?

The first step of `P:0x0FA6` programs the spare through `P:0x0F17` with
no erase. On a mismatch, `P:0x0F45` counts and, at 10,000, restarts the
program command. It does not return. **Confirmed.** The slot is
untouched until that first program returns. The erase helper restarts
when `X:0x179C` exceeds 488. **Confirmed.** Startup in this image does
not erase 60–62, and the loader does not read the spare, so a non-blank
spare is still there for the next store of that group. A stuck poll
cannot continue across a reset that re-enters this image. `P:0x1ABA`
leaves the image, so a measured power cycle is **unknown**. Audible
output after that reset was not measured. **Unknown.** The missing
`0x71` still has both explanations from the store trace: the spare
program can hang before the slot changes, or the spare erase can hang
after the slot is written. Read the slot before deciding.

### 5. Does an erase assume 4 KB, and can it touch the next slot?

`P:0x0F63` addresses `0xD00000 + index × 0x1000` and issues one erase.
The command does not loop across a neighbour, and the address helper
adds no bank bit. **Confirmed.** Neighbour safety for a fitted part
other than the schematic's 4 KB SST39LF040 is **unknown**. `0x6C` erases
numeric index 8 with bit 4 clear. `0x56` erases numeric index 4 with the
bit clear. Preset indices 8 and 4 are the same formula with the bit set.
That overlap is unresolved; contradicted by owner backup (program 63
loads a distinct valid program). Neither erase steps into the next index.

### 6. What does a corrupted slot play?

For a loader argument outside 5–54, a checksum mismatch at `P:0x1FA7`
stores `0x100000` at `Y:0xA48` and skips the 1,092-word copy. The
previous buffer stays. The reload copies the flag to `Y:0xD0` through
`P:0x2090`. Bit 20 selects the short mix, which stores scaled sums.
**Confirmed.** Listening level was not measured. **Unknown.** This is
not "the previous model keeps playing": model dispatch is skipped once
the flag is copied. It is also not a measured silence.

Program change enters `P:0x1114` and `P:0x11A0`, not `P:0x111B`. The
helper adds 55 when its formed value is 5–54. It then always calls the
loader, the reload, the outgoing program-change helper and the three-byte
MIDI helper. Flash index 50 is editor
program 105. **Confirmed** from `Preset::program_number`. A formed value
of 50 would checksum sector 50. The `maci #5` scale was not measured, so
that mapping is **likely** for the integer idiom while `X:0x17A8` is 0,
and **unknown** if the scale differs. Direct high byte 6 with argument
50 takes the compact path and does not set the flag. Startup does not
call the loader. What boot plays was not found. **Unknown.**

### 7. Does any path read the version stamp in words 767–770?

No instruction addresses `Y:0xD46` except the four-word copy in `0x61`
at `P:0x1473B`. No program word addresses `Y:0xD47`, `Y:0xD48` or
`Y:0xD49`. **Confirmed.** The checksum adds the words and does not
compare them with the version text. Bulk copies that start at `Y:0xA47`
include the words without a version test.

### 8. What happens to MIDI that arrives during a store?

On the `0x61` path the flash polls do not parse the queue. Bytes stay
at `X:0x131B` and are parsed as the next MIDI message after the poll
returns. **Confirmed.** They are not consumed as program data. Whether
a full receive queue drops a byte was not measured. **Unknown.** `0x62`
and `0x67` call the byte reader while they program, so bytes that arrive
there are the payload. The editor does not send those opcodes. `0x47`
also consumes a fixed run of 8,195 raw bytes and then sends `0x71`. That
`0x71` is a reply to `0x47`. The editor does not send `0x47`.

### 12. Where is a bank name stored, and does that write use the spare?

`0x6C` commits at `P:0x1416B` before `F7`. The commit erases numeric
index 8 through X with bit 4 clear, programs 132 words to `0xD08000`,
and writes marker `0x45` at `0xD081FE`. **Confirmed.** The spare is not
used. Preset index 8 is the loader's Y read of the same numeric address
with the bit set. That overlap is unresolved; contradicted by owner
backup (program 63 loads a distinct valid program). Which six RAM words
receive one bank's name is **unknown**, because the bare `maci` after
the divide by 5 was not measured. The flash write does not depend on
that offset: the whole table is programmed. `rename_bank` sends this
opcode. The current editor can still erase the name window. It is not
shown to erase preset index 8.

### 13. Where are the string levels stored, and what else is in that sector?

`0x54` only fills six words at `Y:0x2D8`. `0x56`, after `F7`, erases
numeric index 4 with bit 4 clear and programs those six words at
`0xD04000`. **Confirmed.** Preset index 4 is that formula with the bit
set. That overlap is unresolved; contradicted by owner backup (program
63 loads a distinct valid program). There is no spare. `0x57` replies
with the six RAM words and does not touch flash. The family write API
does not send `0x54` or `0x56`. Boot compares three Y bytes at
`0xD04000` with the bit clear, and erases only when they match `m0`.
If reset `m0` is `$FFFFFF`, three `0xFF` bytes match. **Likely** for
that manual reset value. Whether `P:0x04CF` writes `m0` before the
compare was not followed. **Unknown.** This trace does not show that
compare reading preset index 4.

### 10. Which other messages can the guitar send on its own?

Besides the three notices already named, two session-gated channel
messages can leave the guitar without a host request for that message.

`0x28` is sent at `P:0x1363` only when `X:0x17F4` equals 1. The only call
of that sender in this scan is inside `P:0x0DCE`, and the only `jsr` to
`P:0x0DCE` is `P:0x119C`, one arm of the queued-word decoder.
`P:0x0DCE` counts in `X:0x1798`. On the fifth call, when `X:0x179D` is
nonzero, it sends `0x28` and clears `X:0x179D`. **Confirmed** for that
counter. What writes `X:0x179D` was not found. **Unknown.** This scan
does not show a separate hardware-timer entry for the notice.

`0x74` is sent at `P:0x1AA6` when `P:0x1A97` runs and `X:0x17EF` is
nonzero. `0x4D` is sent at `P:0x1AB2` from the same function when
`X:0x179F` is nonzero. `P:0x1A97` is called from the byte reader when the
queue is empty (`jsr` at `P:0x1258`) and from the main loop (`bsr` at
`P:0x1A45`). **Confirmed.**

`P:0x14A0B` sends the byte at `X:0x17F0` with bits 6 and 7 set, then one
data byte. It returns without sending when the session flag is not 1.
The only `jsr` is `P:0x11B9`, after the loader and the reload in the
selection helper. **Confirmed.** Bits 6 and 7 are the program-change
status class. The other bits of `X:0x17F0` were not traced, so the
channel nibble is **unknown**.

`P:0x14A1D` sends `X:0x17F0` with bits 4, 5 and 7 set, then two data
bytes, and only when the session flag is 1. Bit 6 is not set in this
helper. The callers are `P:0x10EE` with immediate `0x07`, `P:0x1106`
with immediate `0x4F`, `P:0x11C1` with immediate `0x4F`, and `P:0x149D3`
on the `0x4E` reply path, also with immediate `0x4F`. **Confirmed** for
those call sites. The channel nibble is **unknown**.

None of these send sites is called from the flash poll inside `P:0x0FA6`.
None is called between that poll's return and the `0x71` at `P:0x14805`.
**Confirmed** for the `0x61` path. The same sites can run while a byte
read is waiting, including the `F7` test that precedes the store, because
an empty read calls `P:0x1A97` and that function calls the queued-word
decoder. They can also run from the main loop. The editor turns the
session on, so the two channel helpers transmit when their callers run.

The store wait skips SysEx opcodes `0x28`, `0x4D` and `0x74`. Any other
event, including a program-change-shaped byte or a three-byte message,
fails a store. **Confirmed** from `wait_for_confirmation`.

Replies that follow a request are separate from those sends. The reply
opcodes seen in this pass are `0x71`, `0x72`, `0x66`, `0x70`, `0x73`,
`0x45`, `0x57`, `0x3C`, `0x3F`, `0x29`, `0x75`, `0x1E`, `0x6D`, `0x69`,
`0x6A`, `0x6B` and `0x41`. Opcode `0x6E` is the reply at `P:0x14869`.
`P:0x14840` starts `F0 7E`. Both of those run only after the first header
data byte was `0x7E` and a later byte was `F7`. The editor header's first
data byte is `0x00`. **Confirmed.** A `0x71` after `0x47`, or after a
completed `0x3E`, is a reply to that request. The editor sends neither
request.

No question in this list was left unread. Audible boot output, a measured
power cycle, the short-mix listening level, the bare `maci` scale,
receive-queue overflow, the six bank-name RAM words, whether `P:0x04CF`
writes `m0`, the message that falls through to `P:0x14BC9`, the `0x3E`
indirect target, and the writer of `X:0x179D` stay **unknown**.

## Reproduction

Use the original local archive with SHA-256
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`:

```sh
python3 scripts/map_variax_dispatch.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-dispatch.json
python3 scripts/map_variax_store.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-store-handlers.json
python3 scripts/map_variax_buffer.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-buffer-handlers.json
python3 scripts/map_variax_session.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-session-handlers.json
python3 scripts/map_variax_read_limits.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-read-limits.json
python3 scripts/map_variax_flash.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-flash-handlers.json
python3 scripts/map_variax_rest.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-rest-handlers.json
python3 -m unittest discover -s scripts -p 'test_variax_*.py'
```

The scripts reconstruct raw tag-0 program loads from region 1 using the
existing [firmware parser](../scripts/map_firmware.py). The store checker
compares call order, branch targets and the address words with
[the store trace](../research/variax-310-store-handlers.json). The buffer
checker does the same for
[the edit-buffer trace](../research/variax-310-buffer-handlers.json). The
session checker compares the bypass and session flag references with
[the session trace](../research/variax-310-session-handlers.json). The
read-limit checker compares the `0x3C` reply shape, the queue mask and the
editor spans with
[the read-limit trace](../research/variax-310-read-limits.json). The
flash checker compares sector ranges, call targets and reply opcodes with
[the flash trace](../research/variax-310-flash-handlers.json). The
remaining-handler checker compares the last 24 entries, the `0x47` count,
the unsolicited send sites and the `0x7E` header branch with
[the remaining-handler trace](../research/variax-310-rest-handlers.json). The
formula tests do not read the archive and do not launch an emulator. The group
quotients recorded in those tests were measured on a typed divide-by-20
program whose words at `P:0x001A`–`P:0x0034` match this archive. The
measurement used the local MIT DSP56300 toolkit and loaded no firmware
routine.

Cross-check the listed addresses against the local DSP56300 disassembly
described in [factory extraction](factory-extraction.md). The retained
private files used here are the contiguous disassembly, SHA-256
`01f82b434175ae68049a8887c29fe5d7e03ae0e45caf0316bc843dd70e28caca`, and the
two worktree extracts `variax-main-278-22c2.dis.txt` SHA-256
`9c99ead85db5ec67d4751911468fa8d84d9eee73dea01f0e8fe64b7e35a6d9b4` and
`variax-main-14004-14c2c.dis.txt` SHA-256
`9ea92a61c4af64d696d4f01c197d15ce1fa75fd93840c8bbc3b00c67ed912326`.
The committed inventories contain derived addresses only.
