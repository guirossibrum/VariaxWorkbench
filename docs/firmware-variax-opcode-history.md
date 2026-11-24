# Variax 3.10 opcode history

This is the offline attribution for [#226](https://github.com/guirossibrum/VariaxWorkbench/issues/226).
It names who builds each Variax 300/600 3.10 command, from the bytes of that
message. It does not send any command. Firmware installation stays out of
product scope. The handler behavior is the static trace in
[the dispatch note](firmware-variax-dispatch.md).

Evidence is **confirmed**, **likely**, or **unknown**. A row with no sender is
a confirmed result of the search below. It is not a claim that no program
anywhere builds the command.

## How a sender was matched

A family command is the opcode byte after `F0 00 01 0C 07` and the product
member. This image accepts member 0 or member 4. Any other member returns
before the opcode table. **Confirmed.** Workbench's MIDI layer prepends the
header stored on the open device. The electric open stores member 0. The
acoustic open stores member 1, so acoustic payload bytes are recorded and
are not 3.10 senders.

A match is the builder that writes those bytes: a Java payload whose first
byte is the opcode, a complete SysEx array, or a static template whose
address is referenced by the function that sends it. A nearby status string
supports the purpose. It is not a match by itself.

The formula checked by `scripts/test_variax_opcode_history.py` is: the
opcode is byte 6 of a message that starts `F0 00 01 0C 07`, or byte 0 of a
payload sent under that header, and the member is 0 or 4.

## Sources

Local copies were used first. No installer was downloaded for this pass.
Vendor files stay ignored. Hashes identify the bytes that were scanned.

| Source | Version | SHA-256 | URL |
| --- | --- | --- | --- |
| Variax Workbench 1.75.dmg | 1.75 | `d70b73a1b0e441e7a640c387f13e42f44284388063b155fa162fcc771a614fa1` | not captured in the local inventory |
| Variax Workbench v1.75 Installer.exe | 1.75 | `a6870ad2792fb0ee70683be0402cbc828ac316dcd1aa66abb2813cdc1df40f99` | not captured in the local inventory |
| Line6-Monkey-1.77.exe | 1.77 | `9e4e487226f44ae8b58b67e67c55cf460447af5d290de1e1fd02ab3dce49b590` | not captured in the local inventory |
| Line 6 Monkey 1.78.dmg | 1.78 | `96dbb7f11635a736a84def227a76dd99aa8028cff4d01f047774e2d1f3b3b3a0` | https://line6.com/getrelease?rid=8688 recorded for this hash and not contacted again |
| Variax300_3.10.vxf | 3.10 | `e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a` | not captured; user-supplied local file, catalog release 1062 |
| Line 6 Monkey.exe extracted from the 1.77 installer | 1.77 | `eb06a7ffaed9cd13119ca8fd4783fddfd03d4e22e23f2fbb3aa9524e272b2cde` | local extract of Line6-Monkey-1.77.exe; the same bytes are in the Workbench HD 2.15 Windows bundle |
| Line 6 Monkey Mac executable from the 1.78 disk image | 1.78 | `47417308c9697456eeed59720ec97281ec226139c707347e4cad3ce787491a37` | local extract of Line 6 Monkey 1.78.dmg; the same bytes are in the Workbench HD 2.15 Mac bundle |
| Line 6 Monkey.exe bundled with GearBox 3.72 | 1.33 | `a419574af0358e291a2a282cb8b993ac775fcf257d9e98dd0ff272f37d3366e8` | local GearBox 3.72 extract; installer URL not recorded for this pass |
| Line 6 Monkey Mac executable bundled with GearBox 3.72 | 1.33 | `512ccb8b54aee4a2eb48babe37e6fa1870f1f1c99fdf51f03718626b91f9b035` | local GearBox 3.72 extract; installer URL not recorded for this pass |
| Line 6 Monkey.exe bundled with POD HD500X Edit 2.27 | 1.73 | `58ff577512f42407b352c8685188792b8d73820641ca2a407cfa91b9a3a00e4c` | local POD HD500X Edit 2.27 extract |
| Line 6 Monkey Mac executable bundled with POD HD500X Edit 2.27 | 1.77 | `93a110f7a97a127456f8eacec63b3356d29dc51c22ac9b81de402b06b0f735e4` | local POD HD500X Edit 2.27 extract |
| Line 6 Monkey Mac executable bundled with POD Farm 2.58 | 1.68 | `3c83714186a4685a596aa2194f8f11dedda90333733a7560ae72afaf30a21211` | local POD Farm 2.58 extract |
| Variax Workbench.exe native launcher | 1.75 | `ed00f5fa55fa6f09a41ec68bf137c160462fc22a92235473ad1d5925cf819e06` | local extract of the 1.75 Windows installer; it contains no family-7 message |

Also scanned, with no member-0 or member-4 opcode template: Line 6 Edit
3.06 Windows and Mac, the extracted POD driver trees, and the Workbench
1.75 native launcher (its hash is in the table). Workbench HD 2.15's own
executable has a member 5 opcode `0x45` message and a repeated member 0
opcode `0x00` pattern. Opcode `0x00` is below this table. Member 5 does not
enter it.

Monkey also contains member 5 templates for opcodes `0x18`, `0x5D`, and
`0x64`. Those are a different product. Windows Monkey 1.77 embeds one
FORM/L6FF container, product `00190014`, which is not this guitar and was
not compared.

## Firmware versions

No older Variax 300/600 image was in the searched local set. The saved
catalog lists 300/600 release 1062 as version 3.10 only. Its 500/700 rows
are a different product and were not substituted. Monkey 1.33's bundled
catalog names `Variax300_3.10.vxf` and, in a note, a 300 firmware 3.07.
That older image is not local. When a handler appeared, changed, or was
dropped is **unknown**.

## What Workbench 1.75 does send

The issue's unused list includes `0x54`, `0x56`, and `0x60`. All three are
built by the 1.75 electric module. `0x54` and `0x56` are the String Volume
Setup set and write. `0x60` is built only by the shipped debug command
`upload_code`, not by a normal editor action. Whether that debug console
can be opened in a normal session was not checked. The payload layout is
omitted.

The same module also builds `0x29`, `0x2A`, `0x3B`, `0x3C`, `0x43`, `0x4C`,
`0x57`, `0x61`, `0x6C`, and `0x6D`. It builds three opcodes this image
treats as direct returns: `0x1B`, `0x3D`, and `0x48`. Windows and Mac
decompiles agree on these bytes.

The acoustic module builds `0x3E` and `0x3F` under member 1. No member-0 or
member-4 builder of either opcode was found.

## Findings

Monkey's flash opcodes are the ones whose static member-0 template has a
code reference in Windows Monkey 1.33, 1.73, and 1.77. The same templates
are present in Mac Monkey 1.68, 1.77, and 1.78, except `0x6F`. Mac absolute
references were not recovered for every template; the Windows reference is
the one cited. None of those opcodes disappears between Monkey 1.33 and
1.78 on Windows. `0x67` gains a second factory-segment caller by 1.73.
`0x6F`'s contiguous template is in Mac 1.33 and is absent from Mac 1.68,
1.77, and 1.78. A non-contiguous Mac builder was not found.

`0x63` and `0x68` are passed together by the function whose status text
names both factory and user patch erases. Pairing each opcode with one of
those two names is **likely**. A second `0x63` template, with one byte
before F7, sits in the function whose status text is "Erasing flash
sectors." Whether that byte is overwritten before the send was not
determined.

`0x1D` is an 18-byte message in the function whose status text is
"Writing Variax patch name:". The handler stores six RAM words. Calling
that region the patch name is **likely**. The same function also sends
`0x61`.

`0x44` is not in Workbench 1.75 or Monkey. The #225 slot-read request at
commit `43437dd` builds it: family header, member 0, opcode `0x44`, the
index, then F7. The third read uses index `0x3E`. That index is not opcode
`0x3E`. The tool waits for each reply to end in F7. Opcode `0x45` is the
guitar's reply to `0x44`, and it is also a separate request handler. No
searched host sends the request.

| Opcode | Sender | Evidence | What 3.10 does | #181 risk |
| --- | --- | --- | --- | --- |
| `0x10` | none in the searched set | no sender; confirmed; purpose confirmed | Loads one byte, 24 words, and one byte, then runs a follow-up after F7. | The follow-up runs only after F7. The scanned window does not call the erase or program helpers. |
| `0x16` | none in the searched set | no sender; confirmed; purpose confirmed | Loads 60 words into X memory, then the shared trailer. | A trailer other than F7 takes the shared failure reply. No erase or program helper in the scanned window. |
| `0x17` | none in the searched set | no sender; confirmed; purpose confirmed | Loads 12 preset-buffer words and runs a follow-up after F7. | A missing F7 skips the follow-up. No erase or program helper in the scanned window. |
| `0x1C` | none in the searched set | no sender; confirmed; purpose confirmed | Loads six word pairs and one byte into RAM, then the shared trailer. | A trailer other than F7 sends the shared failure reply. No erase or program helper in the scanned window. |
| `0x1D` | Monkey 1.33, 1.68, 1.73, 1.77, and 1.78 | sender confirmed; purpose likely | Stores 18 raw bytes as six words in the RAM region that 0x1E reads back. | RAM store only. This is not the bank-name flash commit. The same Monkey function then sends 0x61. |
| `0x1E` | none in the searched set | no sender; confirmed; purpose confirmed | Requires F7, then replies with the six words 0x1D stores. | A missing trailer sends nothing. No erase or program helper in the scanned window. |
| `0x23` | none in the searched set | no sender; confirmed; purpose confirmed | Loads 52 preset-buffer words and runs a follow-up after F7. | A missing F7 skips the follow-up. No erase or program helper in the scanned window. |
| `0x29` | Workbench 1.75 | sender confirmed; purpose confirmed | Requires F7 and replies with the DSP-usage opcode. | The handler is the instruction after the 0x7D reset call. That fall-through is not shown to run. |
| `0x2A` | Workbench 1.75 | sender confirmed; purpose confirmed | Stores bit 0 of one argument as the program-change and CC session flag before reading the trailer. | The flag changes before the trailer check. Reset startup stores 0. |
| `0x35` | none in the searched set | no sender; confirmed; purpose confirmed | Stores nine preset-buffer words and runs a follow-up after F7. | No erase or program helper in the scanned window. |
| `0x36` | none in the searched set | no sender; confirmed; purpose confirmed | Stores one X-memory word, then joins the 0x35 trailer. | No erase or program helper in the scanned window. |
| `0x37` | none in the searched set | no sender; confirmed; purpose confirmed | Stores one preset-buffer word, then joins the 0x35 trailer. | No erase or program helper in the scanned window. |
| `0x3B` | Workbench 1.75 | sender confirmed; purpose confirmed | Writes changed edit-buffer words. | The difference path can send 0x71. A count of 0 is covered by the buffer trace. |
| `0x3C` | Workbench 1.75 | sender confirmed; purpose confirmed | Reads a word count from a start word in the active preset buffer. | No address or count clamp. The transmit helper waits while its ring is full. |
| `0x3E` | none in the searched set | no sender; confirmed; purpose unknown | Stores words through the X:0x39E pointer. On F7 it makes an indirect call, then sends 0x71. | The indirect target was not found, so whether that call stores is unknown. |
| `0x3F` | none in the searched set | no sender; confirmed; purpose confirmed | Reads a bounded range through the X:0x39E pointer and replies with opcode 0x3F. | Out of range sends 0x72. The reply does not wait for F7. |
| `0x40` | none in the searched set | no sender; confirmed; purpose confirmed | Replaces the preset/DSP flag word. After F7 it copies the flag. | The RAM update precedes F7. A value with bit 20 selects the short mix only after the copy. |
| `0x41` | none in the searched set | no sender; confirmed; purpose confirmed | Requires F7, then replies with one DSP state word. | No erase or program helper in the scanned window. |
| `0x43` | Workbench 1.75 | sender confirmed; purpose confirmed | Replaces the full edit buffer and reloads DSP. | The full-buffer trace covers the reload. The editor send on this branch is a separate follow-up. |
| `0x44` | #225 slot read | sender confirmed; purpose confirmed | Reads one stored slot, or the current buffer when the index is 0x7F, and replies with opcode 0x45. | A trailer other than F7 returns before the walk. The flash walk sets PE4 and clears it after the walk. A host that stops reading stalls in the transmit ring with PE4 set. Index 0x7F does not set PE4. |
| `0x45` | none in the searched set | no sender; confirmed; purpose confirmed | Receives 1,092 words and, on a checksum match, stores before it reads the trailer. | There is no index clamp. A checksum miss sends 0x72 and does not store. The words are already in the edit buffer. No searched host sends this request. The 0x44 handler replies with opcode 0x45, and the #225 tool accepts that reply. |
| `0x47` | none in the searched set | no sender; confirmed; purpose confirmed | Reads 8,195 raw bytes with no trailer check, then sends 0x71. | A shorter transfer leaves the reader taking the following MIDI. A store wait can treat an 8-byte 0x71 as success. |
| `0x49` | none in the searched set | no sender; confirmed; purpose confirmed | Updates the preset/DSP flag word, clearing bit 20, then copies it after F7. | The RAM update precedes F7. The copy follows F7. |
| `0x4C` | Workbench 1.75 | sender confirmed; purpose confirmed | Toggles DSP modeling bypass. | An interrupted store can leave the bypass flag set. The flag is separate from PE4. |
| `0x4E` | none in the searched set | no sender; confirmed; purpose confirmed | Requires a one-byte gate and F7, then calls the session-gated tone-controller helper. | There is no SysEx reply. The helper transmits only when the session flag is 1. |
| `0x54` | Workbench 1.75 | sender confirmed; purpose confirmed | Stores six global string levels in RAM. The flash commit is 0x56. | No flash call in this handler. Workbench 1.75 does send it, contrary to the issue's unused list. |
| `0x56` | Workbench 1.75 | sender confirmed; purpose confirmed | After F7, erases and programs the string-level window with PE4 left clear. | The handler does not clear PE4. It relies on the bit already being clear. A missing trailer does not erase. Workbench 1.75 does send it. |
| `0x57` | Workbench 1.75 | sender confirmed; purpose confirmed | Requires F7 and replies with the six string levels. | No flash call. |
| `0x58` | none in the searched set | no sender; confirmed; purpose confirmed | After F7, copies factory templates into one slot or into slots 5 through 54. | Argument 0x7F rewrites that whole range, including the slot the Reso-5 test uses. Other arguments at or above 50, except 0x7F, do not store. |
| `0x59` | none in the searched set | no sender; confirmed; purpose confirmed | After F7, reads stored model-bank sectors and byte-programs the compact template region. There is no erase. | A missing trailer sends 0x72 and does not copy. PE4 is set only around each Y load and cleared before the byte program. |
| `0x5E` | none in the searched set | no sender; confirmed; purpose confirmed | Loads the first 240-word X-memory block, then the shared trailer. | No erase or program helper in the scanned window. |
| `0x5F` | none in the searched set | no sender; confirmed; purpose confirmed | Loads the second 240-word X-memory block, then the shared trailer. | No erase or program helper in the scanned window. |
| `0x60` | Workbench 1.75; Monkey 1.33, 1.68, 1.73, 1.77, and 1.78 | sender confirmed; purpose confirmed | Code-load command. The erase and program run only after the trailer and a matching sum, and the window clears PE4 before the erase. | Out of product scope. A checksum mismatch does not erase. The payload layout is intentionally omitted. |
| `0x61` | Workbench 1.75; Monkey 1.33, 1.68, 1.73, 1.77, and 1.78 | sender confirmed; purpose confirmed | Stores the active buffer into one regular preset slot. | The store sets PE4 and clears it before return. A poll can spin with PE4 set and does not parse MIDI. |
| `0x62` | Monkey 1.33, 1.68, 1.73, 1.77, and 1.78 | sender confirmed; purpose confirmed | Programs the expanded user-slot region while receiving, with PE4 set. There is no erase. | The payload wait and the byte-program poll can hold PE4 set. Sectors not yet entered are not addressed. Out of product scope. |
| `0x63` | Monkey 1.33, 1.68, 1.73, 1.77, and 1.78 | sender confirmed; purpose likely | Erases the high index walk with PE4 set, before the trailer. | The erase poll can spin with PE4 set and does not parse MIDI. Monkey also has a second template, with one byte before F7, in the function whose status text is 'Erasing flash sectors.' Whether that byte is overwritten before the send was not determined. |
| `0x67` | Monkey 1.33, 1.68, 1.73, 1.77, and 1.78 | sender confirmed; purpose confirmed | Programs the compact factory-template region. There is no erase. | Addresses stay in the low region named by the flash trace. Out of product scope. The payload layout is omitted. |
| `0x68` | Monkey 1.33, 1.68, 1.73, 1.77, and 1.78 | sender confirmed; purpose likely | Erases the 32–63 index walk before the trailer and does not write PE4. | The walk relies on PE4 already being clear. The numeric range includes the spare indices. Physical overlap with a store stays unresolved. |
| `0x69` | none in the searched set | no sender; confirmed; purpose confirmed | Requires a one-byte gate and F7, then replies with opcode 0x69. | Any other first bytes return without a reply. |
| `0x6A` | none in the searched set | no sender; confirmed; purpose confirmed | Requires the same gate, then replies with opcode 0x6A. | Any other first bytes return without a reply. |
| `0x6B` | none in the searched set | no sender; confirmed; purpose confirmed | Requires the same gate, then replies with opcode 0x6B. | Any other first bytes return without a reply. |
| `0x6C` | Workbench 1.75 | sender confirmed; purpose confirmed | Assembles a bank name and commits it before reading the trailer. The commit leaves PE4 clear. | The commit relies on PE4 already being clear. Delivering the name bytes and then stopping still erases, because the erase precedes the trailer. Stopping earlier does not. |
| `0x6D` | Workbench 1.75 | sender confirmed; purpose confirmed | Requires F7 and replies with the bank-name table. | No flash call in the reply. |
| `0x6F` | Monkey Windows 1.33, 1.73, and 1.77; Mac 1.33 | sender confirmed; purpose confirmed | Maintenance dispatch. One branch reads. The commit branch erases and programs only after the trailer and a matching sum, and it clears PE4 before the erase. | Out of product scope. Any other selector returns without either branch. The selector byte and the payload layout are omitted. |
| `0x75` | none in the searched set | no sender; confirmed; purpose confirmed | Masks one index nibble and, after F7, replies from X memory. | No erase or program helper in the scanned window. |
| `0x7D` | none in the searched set | no sender; confirmed; purpose unknown | Calls the peripheral reset before it reads the trailer. | Inside this image that path does not erase the spare sectors. Whether the boot ROM erases flash is unknown. A transfer that never reaches F7 has already taken the reset. |

## No sender found in the searched set

No sender found in Variax Workbench 1.75 Windows and Mac decompiles, the
1.75 acoustic module, the 1.75 native launcher, Line 6 Monkey 1.33, 1.68,
1.73, 1.77, and 1.78, Workbench HD 2.15, Line 6 Edit 3.06, the extracted
POD driver trees, and the #225 slot-read source, for these commands:
`0x10`, `0x16`, `0x17`, `0x1C`, `0x1E`, `0x23`, `0x35`, `0x36`, `0x37`, `0x3E`, `0x3F`, `0x40`, `0x41`, `0x45`, `0x47`, `0x49`, `0x4E`, `0x58`, `0x59`, `0x5E`, `0x5F`, `0x69`, `0x6A`, `0x6B`, `0x75`, `0x7D`, `universal-6e-reply`.

The acoustic builders of `0x3E` and `0x3F` are in that 1.75 tree and use
member 1, so they stay in this list.

## Universal header

The first byte after F0 selects this path when it is `0x7E`. The identity
request `F0 7E 7F 06 01 F7` is built by `crates/line6-devices`
`identity.rs`, by every scanned Monkey from 1.33 through 1.78, and by the
#225 procedure. It takes the reply at `P:0x14840` and does not set PE4.
**Confirmed.**

The other universal reply, `P:0x14869`, runs when the saved byte is `0x6E`.
Table opcode `0x6E` is a direct return and is not this path. No sender
found in the searched set builds a message that selects `P:0x14869`. The
entry condition stays in the dispatch note.

## Direct returns Workbench 1.75 still builds

| Opcode | Builder |
| --- | --- |
| `0x1B` | L6VariaxDevice.java tuning-preset request, first payload byte 0x1B |
| `0x3D` | L6VariaxDevice.java StoreTuningPresets sets the first payload byte to 0x3D |
| `0x48` | L6VariaxDevice.java TUNING_OVERRIDE_REQUEST, first payload byte 0x48 |

Each of these targets the common return. They are not part of the 46
command handlers.

## PODxt Live

#211 will add this section: PODxt Live commands that Line 6 Edit 3.06 does
not send, with the same kind of source list and table. Nothing is
attributed here.

## Variax update through a PODxt Live

This is the route question left open by #226. It does not add rows to the
section above. No device was opened, and no image was executed.

The PODxt messages are family 3, opcode `0x23`, one data byte:

| Direction | Bytes |
| --- | --- |
| On, before the flash segments | `F0 00 01 0C 03 23 01 F7` |
| Off, on the way out | `F0 00 01 0C 03 23 00 F7` |

**Confirmed.** Monkey's Variax flash writer sends the on frame when the
connected device name contains `PODxt` and a device-info bit of value 2 is
set, then waits 1,000 ms. It sends the off frame on the way out only when
that same condition was taken. The writer's log calls the pair enable and
disable of large buffers in PODxt Live. The sends are the eight bytes at
the referenced templates, not a nearby string.

| Build | SHA-256 | Call |
| --- | --- | --- |
| Monkey 1.77 Windows, also the Workbench HD 2.15 Windows bundle | `eb06a7ffaed9cd13119ca8fd4783fddfd03d4e22e23f2fbb3aa9524e272b2cde` | `FUN_004577d0` sends `DAT_005cff3c`; `FUN_00457ff3` sends `DAT_005cff44` |
| Monkey 1.33 Windows, bundled with GearBox 3.72 | `a419574af0358e291a2a282cb8b993ac775fcf257d9e98dd0ff272f37d3366e8` | `FUN_0044c000` sends `DAT_00529868` for the on frame |
| Monkey 1.78 Mac, also the Workbench HD 2.15 Mac bundle | `47417308c9697456eeed59720ec97281ec226139c707347e4cad3ce787491a37` | `FUN_000668a2` sends `DAT_00160c38` and `DAT_00160c48` |

The 1.78 disk-image URL is the one already recorded for that hash in
`research/variax-opcode-history.json`. The other two URLs were not captured.
The same two frames are present in the other Monkey builds named in that
file (1.33 Mac, 1.68 Mac, 1.73 Windows, and 1.77 Mac). Those four were
identified by the byte string, not by reading each writer again.

The writer compares the product id with `0x00070004`. Monkey's `default.cfg`
names that id Variax 300/600, and names USB id `0x4650` PODxt Live. Decimal
18000 is that USB id. The same writer also handles `0x00070000` and
`0x00070005`. **Confirmed.**

**Confirmed.** After a failed update whose product id is in the
`0x0007xxxx` range, Monkey adds one sentence when a short in the connection
record equals 18000. The sentence says a Variax update failure can come
from PODxt Live flash memory older than version 2.11. It is in Monkey 1.33,
1.68, 1.73, 1.77, and 1.78. The 1.77 Windows record reads the short at
offset 6. The 1.33 Windows record reads it at offset `0x106`. The compared
value is the same.

**Unknown.** What sets the device-info bit of value 2. The name test is the
substring `PODxt`, so the send itself is not limited to USB id `0x4650`.

**Confirmed.** Workbench 1.75 stores the off frame as
`XTLIVE_ENABLE_VARIAXMESSAGES_MSG` and writes byte 6 to 1 or 0.
Connection mode 1 sends the on frame from `OnInit` and the off frame from
`OnClose`. That is an editor session. The opcode table above already
records that Workbench 1.75 has no sender for the Variax flash opcodes.

**Confirmed.** Line 6 Central 1.06 Windows `UpdaterV3EngineDLL.dll`,
SHA-256 `d24bb48b0f13d9b46c6dd514e712c347ff9a00d4763555614f700c22634bebc7`,
has the same writer shape for product `0x00070004`. The name needle is
`PODxt`, the bit test is the same, `FUN_101f9f60` sends `DAT_10464600`
(the on frame) and waits 1,000 ms, then sends `DAT_1046462c` (the off
frame) on the way out. That DLL does not contain the version 2.11
sentence. Its URL was not captured. No file named as a separate Variax
updater was in the local Monkey, Workbench, GearBox, or Central extracts.

**Unknown.** Which PODxt processor acts on opcode `0x23`. The SHARC map
already records that the 3.01 image has no host family header. This section
does not add a device-side reading.

Two other writers in Monkey 1.77 Windows send the same pair under the same
name and bit test (`FUN_00454310` and `FUN_00454b80`). They are not the
function that compares `0x00070004`. Their products were not identified.

The relay note left these large-buffer bytes unknown because the log
strings only sat near the templates. The calls in the table are the
cross-reference. That note was not edited from this pass.

## Out of scope

Using any of these commands on the rig is a separate trace, a separate
owner approval, and a read-only test first. This pass did not open USB,
MIDI, or audio, and it did not load an image into an emulator.
