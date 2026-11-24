# Variax 300/600 3.10 command dispatcher

This is the first Phase A slice of [#181](https://github.com/guirossibrum/VariaxWorkbench/issues/181).
The [dispatch inventory](../research/variax-310-dispatch.json) covers all 110
table entries. The [handler records](../research/variax-310-read-handlers.json)
cover the session flag and preset-buffer read. Findings are static. Firmware
execution and live device checks remain outside this slice.

## Coverage

| Scope | Confirmed static result | Remaining work |
| --- | --- | --- |
| Line 6 SysEx table | `P:0x14316` indexes 110 words at `P:0x14326–0x14393`, for opcodes `0x10–0x7D`. All targets are present in the main-region P load records. | The shared header parser and receive state machine still need end-to-end analysis. |
| Direct exits | 64 entries target the common stack-unwind return at `P:0x1483E` without calling a command-specific handler. | Follow the outer parser after this return to establish what happens to remaining input. |
| Command-specific targets | 46 distinct entries. This slice traces two, `0x2A` and `0x3C`. The other 44 are outside this slice. | Track each group through its callees. Historical traces for string levels and updater paths are in [factory extraction](factory-extraction.md) and [update research](update-recovery.md). |
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

This slice identifies no direct persistent write in either handler. The
flash, recovery and DSP traps remain on #181. A wider `0x3C` probe needs
the owner's separate procedure approval.

## Reproduction

Use the original local archive with SHA-256
`e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`:

```sh
python3 scripts/map_variax_dispatch.py /path/to/Variax300_3.10.vxf \
  --check research/variax-310-dispatch.json
```

The script reconstructs raw tag-0 program loads from region 1 using the
existing [firmware parser](../scripts/map_firmware.py). Cross-check the
targets and listed addresses against the local DSP56300 contiguous disassembly
described in [factory extraction](factory-extraction.md). The retained private
disassembly used here has SHA-256
`01f82b434175ae68049a8887c29fe5d7e03ae0e45caf0316bc843dd70e28caca`.
The inventory contains derived addresses only.
