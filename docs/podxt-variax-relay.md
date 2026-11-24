# PODxt Live Variax relay

Offline notes for [#232](https://github.com/guirossibrum/VariaxWorkbench/issues/232).
The relay questions were moved here from [#211](https://github.com/guirossibrum/VariaxWorkbench/issues/211)
after the SHARC map merged as `b03b9c4`. Issue #232 has no comments of its own.
The questions are comments on #211, and the owner run is on
[#225](https://github.com/guirossibrum/VariaxWorkbench/issues/225).

Labels mean: **confirmed** (bytes, disassembly, or a recorded owner observation),
**likely** (a reading that fits those bytes and is not proven), **unknown**
(not established). Where a host opcode is still being read as a SHARC table
index, the phrase stays: **contradicted by owner hardware runs; host-to-SHARC
translation unknown**. This file does not add that kind of reading. The relay
is a host-to-guitar path in front of the SHARC program. No host message is
shown to reach the SHARC table, index `0x71`, or the half erases. Firmware
was not executed, and no device was opened.

## Answers

| Question | Label | What the searched sources show |
| --- | --- | --- |
| The PODxt relays only a set of Variax opcodes | **unknown** | No allow-list was found in the 3.01 image, the driver trees, Workbench 1.75, Line 6 Edit 3.06, or the MIDI libraries. Those libraries forward a family-`07` message once it is in the host buffer. |
| The PODxt caps guitar-to-host reply size | **unknown** | No 200-byte or 256-byte device cap was found. The 256-byte value is a host send splitter. Monkey's updater text names a PODxt Live "large buffers" command, and the bytes of that command were not recovered. |
| Another `03 23` value, or a firmware-update pass-through, relays `0x44` | **unknown** as a device mode. **Confirmed** that the host templates write only `0` and `1` into `03 23`. | Workbench, the Rust bridge, and the Monkey binaries contain `F0 00 01 0C 03 23 01 F7` and `F0 00 01 0C 03 23 00 F7`. No third value of that opcode was found. |

One silent `0x44` is compatible with an opcode filter and with a reply-size
cap. The run does not separate them. The guitar still working afterwards makes
a stuck guitar less likely. It leaves open whether the guitar saw the request.

## The #225 owner run

**Confirmed** as the owner's comment on #225, 2026-11-24 15:10 UTC. This pass
did not open the local log
`backups/reso5-tests/reso5-slot-read-20261124T150807Z-bEfNKN`.

PODxt Live 3.01 and Variax 3.10 identities were verified on the live route.
`03 23 01` and `2A 01` were sent. `F0 00 01 0C 07 00 44 7F F7` was sent at
0.181 s. The owner recorded no byte in 10 seconds. Cleanup sent `2A 00` and
`03 23 00` and released the driver. Indices `0x32` and `0x3E` were never
requested. Nothing was stored. A later check the same minute found the guitar
working: knob changes and sound were normal. The owner's conclusion in that
comment is that the PODxt Live most likely did not relay `0x44`.

The same comment says `0x43`, `0x3C`, `0x2A`, and `0x6D` have worked on this
route. The sizes that belong with those opcodes are below. The three complete
owner backup folders audited for #227 contain no bank-name reply files. That
inventory and the comment's `0x6D` sentence are both on record. They describe
different captures.

The request the tool sends is the nine-byte frame above. The reply it waits
for is opcode `0x45`, 6,561 bytes (`3,276 × 2 + 9`), inside a 10-second
window (`crates/line6-devices/src/family/variax/slot_read.rs`). The USB read
on this route is a 16-byte interrupt packet
(`crates/line6-devices/src/family/variax/transport.rs`). The reader keeps
polling until the window ends or a complete frame arrives. A missing complete
`0x45` frame produces "Timed out waiting for a complete 0x45 reply ending in
F7." An assembled message that grows past 8,192 bytes before `F7` produces
"unexpectedly large SysEx message" (`crates/line6-devices/src/midi.rs`,
`SYSEX_LIMIT`). A 6,561-byte reply fits under that limit. The owner's "no
byte" report is the silent-timeout path. It is a different outcome from the
assembler's oversize error.

The same USB write path has already carried a larger frame the other way.
**Confirmed** from the #176 owner ledger: the Rust `0x43` program, 6,566
bytes host to guitar, was sent through PODxt Live 3.01, and the guitar read
back the edit. A 200-byte `0x3C` reply is the ordinary current-preset read on
this route. The 16-byte packet size and the 256-byte host send chunk are both
smaller than those working transfers, so neither one accounts for a missing
6,561-byte reply.

## Setup and release

### Workbench connection modes

**Confirmed** from Workbench 1.75 `L6VariaxDevice.GetConnectionMode`, in the
Windows and Mac decompilations of that class.

| Mode | Constant | When the method sets it | What `OnInit` / `OnClose` send |
| --- | --- | --- | --- |
| −1 | `THRU_UNKNOWN` | The MIDI port state is absent | Nothing from these two methods |
| 0 | `THRU_SEAMONKEY` | The port is Line 6 hardware and the hardware id is 21325 | Direct guitar session. These methods do not send `03 23` |
| 1 | `THRU_XTLIVE` | The port is Line 6 hardware and the id is any other value | `SetSysexChunkSize(256)`, then `03 23` with byte 6 set to 1 on open and 0 on close |
| 2 | `THRU_VETTA` | The port is not Line 6 hardware | The Vetta pair below, byte 6 set to 1 on open and 0 on close |

The #225 comment grouped mode 1 as "PODxt Live/Vetta" and mode 2 as "plain
MIDI". The method above splits them the other way: mode 1 is the non-Variax
Line 6 hardware path and sends `03 23`; mode 2 sends the Vetta pair.
`CheckConnectionStatus` sends the Vetta pair twice more when the identity
product is a Vetta, then reads identity again.

`EnableVariaxMIDI` runs after the relay enable. It sets the guitar header
`F0 00 01 0C 07 00`, sends `2A 01`, and sleeps 100 ms. That is the guitar
session. The Rust bridge helper writes the same two `03 23` frames
(`bridge_message` in `crates/line6-devices/src/family/variax.rs`).

### The two enable pairs

**Confirmed** template bytes. `SetXTLiveVariaxMessagesEnable` overwrites only
byte 6:

```text
F0 00 01 0C 03 23 01 F7
F0 00 01 0C 03 23 00 F7
```

`SetVettaVariaxMessagesEnable` writes the same boolean into both frames and
sends them as two raw MIDI messages:

```text
F0 00 01 0C 03 0F 01 F7    F0 00 01 0C 03 10 01 F7
F0 00 01 0C 03 0F 00 F7    F0 00 01 0C 03 10 00 F7
```

The electric and acoustic Workbench device classes each contain these
templates. Line 6 Edit 3.06's `WrapVariaxMessage` is a different `0x10`:
under the PODxt header it sends family opcode `0x10`, then a model controller
and a value (`{16, 0, cc, value}`). Callers pass controllers 101, 120, 121,
96, 97, 98, 122–127, 99, and 100. That wrapped model edit is the message used
in the F-Spank observation below. It is a separate opcode from Vetta's
`03 10`.

`L6EditorApp.DisplayUpdateFirmwareMessage` opens a dialog that can launch
Line 6 Monkey. The method contains no Variax relay frame.

### Host send chunk and host receive buffers

**Confirmed** in the i386 `L6MIDIJava` library, SHA-256
`902713ed5e4819021a73ff8a4f2d6a35521a99a96af7253dc53ef153e3789a8f`.

`MIDIEngineBase::SetSysexChunkSize` at `0x38a6` stores the argument at
`this+0xc` and returns. `MIDIEngineBase::SendSysex` at `0x3a0a` sends the
whole buffer in one piece when that field is 0. Otherwise it loops, and each
iteration hands `min(remaining, chunk)` to `SendSysexNoChunking`. Mode 1
`OnInit` stores 256. `OpenL6HW` stores 256 when the Mac device name is
"Variax USB Interface". The field is a host send splitter. The same offset
`this+0xc` appears on unrelated objects elsewhere in the library; those uses
are not this chunk size.

`MIDIEngineBase::Init` at `0x407e` constructs `L6MIDIDeviceIO` with
`0x20000` (131,072 bytes). The constructor at `0x972a` allocates that many
bytes. `ProcSysexMessage` at `0x8e62` copies incoming bytes into the buffer
until `F7`, wraps at the allocated length, and returns `0x80009009` if the
write index meets the read index. The Windows libraries already decompiled
for this tree construct the same object with `0x20000`. A 6,561-byte reply
fits in that buffer.

`CheckNextSysexPacket` at `0x9054` returns immediately when the length is
under 6. It peeks at most 21 bytes. For `F0 00 01 0C` and family `07` it
forwards the opcode and the full length to the callback. Family `03`
opcodes `0x56`, `0x57`, `0x50`, `0x22`, and `0x0F` select callback id
`0x3007F`; other family-`03` opcodes take a different id. Both arms call
the same callback with the full length. `GetSysexPacket` at `0x3b4e` copies
an event when the caller buffer is at least the event length.
`SysExCallback` at `0x3bc6` stores the offset and length in a 512-slot
event ring and prints "MIDI event buffer overflow" when that ring laps
itself. `L6MIDIJava::OnSysexEvent` at `0x2f3e` returns without reading the
event. Java `L6MIDIManager` keeps `MAX_SYSEX_MESSAGES = 32`, a queue count.
The Java coalesce buffer starts at 4,096 bytes, `m_coalesce_sysex` defaults
to false, and the coalesce path is inside `SendSysex`.

Workbench's own waits, from `L6VariaxPatchManager` and `L6VariaxDevice`, are
sized to the replies it requests:

| Opcode | What the editor waits for |
| --- | --- |
| `0x3C` | Two reads of 32 words from address 851. Each reply must be `32 × 6 + 8` = 200 bytes, with opcode byte 60 |
| `0x1B` | Twelve tuning-preset reads. A reply of exactly 45 bytes with opcode byte 27 is copied |
| `0x6D` | A bank-name reply of at least 26 bytes with opcode byte 109 |
| `0x43` | Outbound only. `EncodePatch` builds opcode byte 67, 1,092 words, length `13 + 1,092 × 6 + 1` = 6,566, and calls `SendSysex` |

Workbench 1.75 does not send `0x44`. That is a statement about this editor.
It is not a device allow-list.

## Monkey, GearBox, and the driver

### `03 23` in the updater

**Confirmed** by a byte search of the local GearBox 3.72 and Monkey trees.
The complete `03 23` frames are only the two values above.

| File | `03 23` value `01` | `03 23` value `00` | Also contains |
| --- | --- | --- | --- |
| GearBox 3.72 Windows `Line 6 Monkey.exe`, 2,228,224 bytes, SHA-256 `a419574af0358e291a2a282cb8b993ac775fcf257d9e98dd0ff272f37d3366e8` | 3 | 3 | The Vetta pair, `03 0E 00`, and `03 20` |
| Monkey 1.78 Mac `Line 6 Monkey`, 2,378,528 bytes | 3 | 3 | The Vetta pair and `03 0E 00` |
| Monkey inside the GearBox 3.72 Mac package, 3,898,712 bytes | 6 | 6 | The Vetta pair and `03 0E 00` |
| GearBox 3.72 Windows `GearBox.exe`, SHA-256 `e643d4c497dd7030b55649b644550a4394ff78440bc06cef4513e3731f297d7d` | 0 | 0 | The Vetta pair |
| GearBox 3.72 Mac `GearBox`, `L6TWXG`, and `Line6AudioMIDI` | 0 | 0 | `03 0F 01` was present. `03 23 01` was absent |

The Windows Monkey executable also contains the error strings "Failed to
send command to disable large buffers in PODxtLive" and "Failed to send
command to enable large buffers in PODxtLive". The same two strings are in
both Mac Monkey binaries. The same Windows file says a Variax update failure
can come from PODxt Live flash older than 2.11, in the sentence "Check to
make sure your PODxt Live is at version 2.11 or higher." Those strings are
**confirmed**. The SysEx bytes of the large-buffer command are **unknown**.
In the Windows file the strings sit near the `03 23` templates and near
short family-`07` templates (`07 00 61`, `07 00 63`, `07 00 68`, and
`07 00 1D` plus spaces). Neighbourhood in the file is not a cross-reference,
so none of those templates is identified as the large-buffer command.

That text is the updater's own evidence of a size-related PODxt Live command
on the Variax firmware-update path. It fits the #225 silence beside a working
200-byte reply and a working 6,566-byte host-to-guitar send. It does not
identify the command, and it does not show that firmware 3.01 still honors
it. The owner unit is 3.01, which is a later version number than 2.11. The
2.11 image is not in the local trees, so the relay code of that older flash
was not compared.

### Controller image

**Unknown.** No separate controller executable was found in this pass.

Searched, with no PODxt 2.x container and no `CY7C`, `EZ-USB`, `8051`,
`.bix`, or `.iic` image:

- `vendor/installers/PODxtLive_3_01.xtf` is the only `.xtf` in that directory.
- Monkey 1.78 `default.cfg` has one non-empty `<filename>` in the whole file,
  `GearBox v1.00 Installer.exe`. The USB Firmware row and the Flash Memory
  row that list PODxt Live (`0x0003000a`) are version `0.00` with an empty
  filename, as already recorded in [the USB-firmware note](pod-usb-firmware.md).
- The PODxt driver directories under `pod-drivers-windows` and
  `drivers-4.2.7.7-win` (`l6podxt.dll`, the Live and Pro siblings, the `.sys`
  files, and `l6podxt.inf`). `l6podxt.dll` is 151,040 bytes, SHA-256
  `c9ef3db97f10935fc4f28aa69a639aa4684bce6697dde506ca83592e2b41c69a`. Those
  files also contain no `00 01 0C 03` and no "Firmware" or "Variax" string.
- GearBox 3.72 Windows `L6DriverControlPanel.cpl`, SHA-256
  `85d69b1f33ea4f47fc330a935d624b8004e514f884307a47339f130e0570493c`, has
  three `00 01 0C 03` templates, opcodes `0x56`, `0x57`, and `0x00`, plus
  the strings "Error reading PODxt firmware version." and "Error reading
  PODxt firmware status." It has the C++ type `L6XTVariaxMIDISvcImpl` and
  no `03 23` frame.
- `L6GPInst.dll` names "PODxt USB Firmware" and "Variax Flash" and contains
  no `00 01 0C 03` bytes.

The 3.01 main-flash image still has no copy of the host header, as recorded
in the [#211 host path](firmware-podxt-dispatch.md). Sheets 1–4 of drawing
35-00-0134, and the controller IC, remain unknown. The POD HD500X image in
the extracted-firmware directory is a different product and was not used here.

## What each #227 observation proves

The observations are the owner ledger in `README.md` and mismatch 6 in
[the capabilities audit](variax-300-600-capabilities.md). #227's own issue
comment is the handoff. It does not add a separate measurement.

| Observation | Proves | Leaves open |
| --- | --- | --- |
| With computer USB unplugged, the last EDIT page shows the Variax MODEL field. Reconnecting USB reports computer control, with VARIAX set to ON. | The front panel followed USB presence on that occasion, and the owner saw Variax control ON. | The internal ownership bit. A displayed page is the observation. |
| Standalone recall, 2026-11-23. The owner selected channel 1B and returned to 5B with computer USB unplugged during each switch. After USB returned, guitar metadata reported Chime-1 (body `0x0310000c`), then T-Model-1 (body `0x03100000`). Chime-1 matched the verified guitar backup apart from the live tone field. T-Model-1 differed from the manually selected Telecaster capture only in the preset name. No preset was saved. | The PODxt Live can recall those two stored linked F- models, and the guitar follows, while the computer is off the USB route at selection time. | U- models, Custom models, and keep-current. The USB linked-recall test stopped at the baseline read (JazzBox-3) before a channel change, so USB-triggered recall is unverified. |
| Standalone release, 2026-11-23. The probe capture shows one identity request and one `SetXTLiveVariaxMessagesEnable(false)` on the PODxt MIDI port. The owner reported that the last EDIT page continued to show computer control. | One release send left the display unchanged on that route. The write confirms that the host dispatched the message. | An internal ownership bit, a device acknowledgement, and any later release. The capture has no device acknowledgement of the release. |
| Wrapped model edit, 2026-11-23. After the baseline passed, Line 6 Edit's wrapped command selected F-Spank 1, waited 10 seconds, attempted to restore F-T-Model 1, and waited 10 seconds. All three PODxt dumps and all three guitar metadata pairs matched byte for byte. The linked value stayed 10 and the guitar stayed Telecaster. | That wrapped audition had no observed effect on those two dumps on the tested USB route. The PODxt MIDI port accepted both host writes. | Whether the PODxt can audition a linked model while USB is absent. The standalone F- recall above is the positive case for stored F- models without the computer. |
| Three verified library backups, 30/50 metadata matches, and the #176 `0x43` edit with readback. | Identity, current metadata, and one full edit-buffer program transfer work through this PODxt Live. | A stored-slot `0x44` read. Bank-name reply files are absent from those three backup folders. |

## What this note leaves in place

The merged slot-read code stays available for a possible direct Variax USB
interface, Workbench mode 0. This note does not add a retry of `0x44` over
the PODxt, a new `03 23` value, or a send of the unidentified large-buffer
command. A bounded owner check for #221 and #222 still needs its own
approval. Keep-current, F-, U-, and Custom stay distinct, as #218 recorded.
The two F- models above are the ones with standalone recall evidence.
