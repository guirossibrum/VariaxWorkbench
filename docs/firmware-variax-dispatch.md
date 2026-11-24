# Variax 300/600 3.10 command dispatcher

This is the first Phase A slice of [#181](https://github.com/guirossibrum/VariaxWorkbench/issues/181).
The [dispatch inventory](../research/variax-310-dispatch.json) covers all 110
table entries. The [handler records](../research/variax-310-read-handlers.json)
cover the session flag and preset-buffer read. The
[store trace](../research/variax-310-store-handlers.json) covers opcode `0x61`.
Findings are static. Firmware execution and live device checks remain outside
this work.

## Coverage

| Scope | Confirmed static result | Remaining work |
| --- | --- | --- |
| Line 6 SysEx table | `P:0x14316` indexes 110 words at `P:0x14326–0x14393`, for opcodes `0x10–0x7D`. All targets are present in the main-region P load records. | The shared header parser and receive state machine still need end-to-end analysis. |
| Direct exits | 64 entries target the common stack-unwind return at `P:0x1483E` without calling a command-specific handler. | Follow the outer parser after this return to establish what happens to remaining input. |
| Command-specific targets | 46 distinct entries. Traced here: `0x2A`, `0x3C` and `0x61`. | The other 43 remain. Track each group through its callees. Historical traces for string levels and updater paths are in [factory extraction](factory-extraction.md) and [update research](update-recovery.md). |
| Image bytes | The 110 pointer words classify 330 bytes as dispatch data in the 442,402-byte decompressed Variax image (0.075%). | Whole-image classification, MIDI CC and program-change handling, universal SysEx, and PODxt Live coverage remain open. |

The dispatcher subtracts `0x10` at `P:0x1431A`. It branches on underflow
before the table access, then fetches the P-space word at `P:0x14322` before
the branch at `P:0x14323` that rejects indices above `0x6D`. The upper
comparison guards the indirect jump. The consequences of the preceding fetch
on the physical processor are unknown.

## Session flag and preset-buffer read

| Opcode | Confirmed input and action | Confirmed reply path |
| --- | --- | --- |
| `0x2A` | `P:0x144DD` takes bit 0 of one byte, stores it to `X:0x17F4` and reads one more byte before returning. The store precedes the trailing read, which does not check for `F7`. The flag controls outgoing program-change and CC helpers at `P:0x14A0B` and `P:0x14A1D`. | No immediate acknowledgment in the handler. |
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
over the physical link, whether every selection path yields an exact
stored-image match for all 1,092 words, and what the processor does for
exceptional count values. The compact copy alone leaves the tail's provenance
unresolved. There is no surviving full-DSP request or reply from the
2026-11-14 timeout described in [#119](https://github.com/guirossibrum/VariaxWorkbench/issues/119),
so its cause remains unknown.

## Host risks from this slice

| Trigger | Static evidence and possible consequence | Existing app boundary |
| --- | --- | --- |
| A variable read span extends beyond the preset buffer or queues an excessively long reply. | `P:0x144AE–0x144DA` uses the supplied address and count without a span guard. Possible unrelated Y-memory reads or a stalled response. Exact behavior outside the traced buffer is unknown. | The request enum exposes only the two fixed 32-word metadata spans. |
| A session command is interrupted after its argument, or cleanup fails. | `P:0x144E0` changes the flag before `P:0x14565` reads the trailing byte. Session state may persist, affecting outgoing MIDI reports. | The Rust session wrapper attempts session off even when the body or initial send fails and reports an off-send failure after a successful initial send. |

`0x2A` and `0x3C` identify no direct persistent write. The store path below
is the first flash trace in this series. A wider `0x3C` probe needs the
owner's separate procedure approval.

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
and never reaches `0x71`. The meaning of the bit is **unknown**.

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
through Y at the same numeric address. The schematic names one SST39LF040.
The two spaces aliasing onto that chip is **likely**. The zero byte is
written through Y immediately before the erase of that same address, so a
completed erase removes it when the spaces alias. It is not a flag the
loader tests.

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
Sectors 60–62 are outside the 60-slot preset region of the archive, so
whether a spare arrives erased is **unknown**. The first store into a group
programs that spare with no preceding erase.

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
  leaves the buffer uncopied. The effect of that flag value is **unknown**.
  The mismatch branch does not read the spare.

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
| After step 2, before step 3 finishes | Zero at the first byte if X and Y alias, or an erase already in progress | New image if step 1 finished | Checksum fails, so the copy is skipped and `Y:0xA48` becomes `0x100000`. The spare is not copied in. |
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
| Spare left programmed, then any later store in the same 20 slots. | `P:0x0FB9` before `P:0x0FCE` | The next store programs the spare without erasing it. A NOR-style poll can hang before the new slot is touched. Power-on does not erase sectors 60–62. | Yes, if an earlier `0x61` in that group was interrupted or its spare was not blank. | Treat the whole bank group as off limits after an ambiguous store. The groups are banks 0–3, 4–7 and 8–11. |
| Normal `0x61` of a host program whose words 767–770 and 1,089–1,090 are zero. | `P:0x1473A`–`P:0x14744` | Words 767–770 become `0x30 0x33 0x31 0x30` and word 1,091 increases by `0xC4`. A full-program compare disagrees with the host buffer. | Yes. This is what a successful store does. | Expect the stamp. Compare settings from word 851. Do not retry to force byte-identical flash. Audio effect of the stamp is unknown. |
| Power or link lost after the zero byte or during the slot erase or slot program. | `P:0x0FC1`–`P:0x0FC8` | That slot's sector is incomplete. The spare is not copied back. The next full-path load skips the copy and sets `Y:0xA48` to `0x100000`. | Yes, if the owner's store is interrupted. | Keep the verified backup. Do not retry the group. This image has no spare fallback. |
| `0x71` missing after the slot program has returned. | `P:0x0FCA`, then `P:0x0FCE`, then `P:0x14805` | The slot can already hold the new image while the spare erase or the reply is still outstanding. | Yes. The 15 s wait can time out after step 4. | Read the slot before any further store. A timeout does not mean the slot is unchanged. |
| Successful store erases the whole `0x1000`-byte step and programs 3,276 bytes. | `P:0x0F63`, `P:0x0F17` | Bytes 3,276–4,095 are left erased. Factory tails are `0xFF`. Owner tails are unknown and are not in the settings backup. | Yes, on a completed store. | Do not assume a non-`FF` tail survives. It is not part of the 1,092-word program the loader copies. |
| Fitted flash erases a block larger than 4 KB. | `P:0x0F63` steps by `0x1000` | A neighbour preset could be erased with the target slot. | Yes, the editor does send in-range `0x61`. | **Likely** avoided on the schematic's SST39LF040. **Unknown** if the fitted part differs. #177 re-reads the neighbouring presets and stops if one changed. |

The same store routine is also called from `P:0x1186`, `P:0x141A4`,
`P:0x145E2`, `P:0x145F3` and `P:0x1481E`. Those callers are not the editor's
`0x61` message. Other helpers program ranges outside the preset slots,
including `P:0x1016`. This slice identifies those sites so a later pass can
block them. It does not describe how to invoke them.

## Reproduction

Use the original local archive with SHA-256
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`:

```sh
python3 scripts/map_variax_dispatch.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-dispatch.json
python3 scripts/map_variax_store.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-store-handlers.json
python3 -m unittest discover -s scripts -p test_variax_store_trace.py
```

The scripts reconstruct raw tag-0 program loads from region 1 using the
existing [firmware parser](../scripts/map_firmware.py). The store checker
compares call order, branch targets and the address words with
[the store trace](../research/variax-310-store-handlers.json). The formula
tests do not read the archive and do not launch an emulator. The group
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
