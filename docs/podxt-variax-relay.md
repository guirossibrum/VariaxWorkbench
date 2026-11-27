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
| The PODxt caps guitar-to-host reply size | **unknown** | No 200-byte or 256-byte device cap was found. The 256-byte value is a host send splitter. The Monkey large-buffer command is the existing `03 23` pair, recovered below. Its sender retries the outbound transfer and then sleeps. |
| Another `03 23` value, or a firmware-update pass-through, relays `0x44` | **unknown** as a device mode. **Confirmed** that the host templates write only `0` and `1` into `03 23`, and that those two values are the large-buffer enable and disable. | Workbench, the Rust bridge, and the Monkey binaries contain `F0 00 01 0C 03 23 01 F7` and `F0 00 01 0C 03 23 00 F7`. No third value of that opcode was found. |

One silent `0x44` is compatible with an opcode filter and with a reply-size
cap. The run does not separate them. The guitar still working afterwards makes
a stuck guitar less likely. It leaves open whether the guitar saw the request.
The large-buffer enable is the `03 23 01` frame that run already sent, so a
missing large-buffer command does not account for the silence.

## VDI presence, identity and continuity

**Static host trace.** In Workbench 1.75, the selected MIDI input reaches
`L6MIDIJava.native_WaitForMIDI`, then `L6MIDIManager` dispatches SysEx to
`L6VariaxPatchManager.OnSysex`. The 11-byte `03 11` branch uses only byte 9
for connection status (zero means disconnected) and notifies on Boolean
changes. The 12-byte `7F 03` branch reports disconnected. Neither status
branch checks a guitar identifier or attachment counter. Bridge-mode
`PollConnection` returns success without checking the downstream guitar.

On Linux, the app's passive path decodes the notices on the verified PODxt's
ALSA raw MIDI input. A changed notice queues an identity refresh. A successful
refresh can retain the guitar's existing host generation. Active Variax Follow
instead decodes the notices from PODxt USB interface 1 and can invalidate that
generation on a reported disconnect. Both inputs terminate at the PODxt. The
host routes do not establish where PODxt 3.01 generates or forwards a notice,
or whether every VDI interruption reaches either input.

**Existing owner evidence.** The Linux check saw no notices and needed manual
refresh. [#201](https://github.com/guirossibrum/VariaxWorkbench/issues/201)
Two macOS Rig Check 6.1 records from 2026-11-26 detected loss and return by
identity fallback. The packet-recorded run opened Follow windows with the
bridge enabled and captured a universal identity reply and family-`07`
`69`/`6A`/`6B` traffic during return. Neither `03 11` nor `7F 03` was decoded.
Those additional frames have no verified per-guitar identifier or session
counter. The 17-byte identity reply supplies manufacturer, family, member and
firmware. Its equality across a replug establishes the same reported identity
and a responding guitar at each successful read.

**Continuity remains unverified.** A same-model, same-firmware replacement can
answer identically while the PODxt USB attachment generation stays fixed.
Evidence would need a device-originated, non-reused VDI attachment token
bound to the operation, or a loss-detectable stream proven to report every
VDI break across listener gaps and handoffs. Per-unit identity would distinguish
two guitars at read time, but even the original guitar can be unplugged and
reinserted. The existing captures contain no comparison with a known same-model
replacement, and the firmware trace does not establish the notice producer,
delivery conditions or the meaning of the unused status bytes.
[Cache capture requirements](first-connection-cache.md#variax-capture-proof-240)

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

### Large-buffer command

**Confirmed.** The error strings "Failed to send command to enable large
buffers in PODxtLive" and "Failed to send command to disable large buffers in
PODxtLive" sit on the failure paths of the calls below. The bytes those calls
pass are the two `03 23` frames already counted in the table. Each of the
three builds contains that enable frame three times and that disable frame
three times. The file bytes at every call operand are:

| Action | Eight bytes |
| --- | --- |
| Enable | `F0 00 01 0C 03 23 01 F7` |
| Disable | `F0 00 01 0C 03 23 00 F7` |

| Build | Enable address | Disable address | Call |
| --- | --- | --- | --- |
| Windows Monkey 1.77, `Line 6 Monkey.exe`, 4,178,432 bytes, SHA-256 `eb06a7ffaed9cd13119ca8fd4783fddfd03d4e22e23f2fbb3aa9524e272b2cde` | `0x005CF868`, `0x005CFB50`, `0x005CFF3C` | `0x005CF870`, `0x005CFB58`, `0x005CFF44` | Object at `this+0x30`, virtual slot `+0x40`, which is `FUN_004a3380` |
| Windows Monkey 1.33, the GearBox 3.72 bundle in the table above | `0x0052954C`, `0x005296F8`, `0x00529868` | `0x00529554`, `0x00529700`, `0x00529870` | Same object, slot `+0x3C`, which is `FUN_0041f4a0` |
| Mac Monkey 1.78, `Line 6 Monkey`, 2,378,528 bytes, SHA-256 `47417308c9697456eeed59720ec97281ec226139c707347e4cad3ce787491a37` | `0x160B00`, `0x160B48`, `0x160C38` | `0x160B08`, `0x160B50`, `0x160C48` | Same object, slot `+0x44`, which is `L6MIDIDeviceIO::SendSysexMessage` |

The Windows 1.77 enable at `0x454543` pushes `0x1388` (5,000), `8`, and
`0x005CF868`, then calls slot `+0x40`. The Windows 1.33 enable at `0x449DDD`
pushes the same timeout and length with `0x0052954C`, then calls slot
`+0x3C`. The Mac 1.78 enable at `0x63164` stores timeout `0x1388`, length
`8`, and address `0x160B00`, then calls slot `+0x44`. The disable sites use
the next address in each pair, the same length, and the same timeout.

Order, **confirmed** from those three writers. The enable is `03 23 01`.
These functions have no earlier `03 23` and no second `03 23` between the
two calls. After a successful enable the writer sleeps 1,000 ms, then writes
code and patch segments. The disable, `03 23 00`, follows those writes. The
long writer's other 8-byte send, Windows 1.77 `0x005CFF54` and Mac 1.78
`0x160C40`, is `F0 00 01 0C 07 05 64 F7`. Its failure text is "Update
alt-tone presets failed!". That frame is a different command.

The send is gated, **confirmed** as control flow. The writer reads a device
record from the MIDI object, searches the wide string `PODxt`, and continues
into the enable only when that search hits and bit 1 of the flag method's
return value is set (`test al, 2` at Windows 1.77 `0x454486`; the Mac writer
masks that value with 2). The failure text names PODxt Live. What bit 1 selects
beyond that test is **unknown**.

Reply wait, **confirmed** for the host. Mac `SendSysexMessage` at `0xCC20`,
Windows 1.77 `FUN_004a3380`, and Windows 1.33 `FUN_0041f4a0` are the same
sender. When its trace flag is set it logs "MIDIDevIO Syx Tx:" with the
buffer and the length. It then calls the out port. A return of `0x9000`
sleeps 10 ms and tries again until the 5,000 ms argument is spent, and then
returns `-0x7FFF6FF4`. Any other out-port return is passed back. A negative
result on enable is logged and thrown, which stops the update. A negative
result on disable is logged, and the writer continues. The 1,000 ms pause
after a successful enable is `Sleep` on Windows (`FUN_00475b70` in 1.77,
`FUN_00407790` in 1.33) and `L6ThreadManager::Sleep` on Mac. The disable has
no following sleep. These call sites do not register the SysEx callback that
other Monkey sends use when they expect an ack. The sender's inbound wait,
and any device byte that answers `03 23`, stay **unknown**.

### The 2.11 dialog

**Confirmed.** The sentence "Check to make sure your PODxt Live is at
version 2.11 or higher." is dialog text on the Variax-update failure path.
The characters `2.11` are in that string. The branch does not compare a
firmware version with `2.11`, `0x211`, or a packed version word.

The dialog is reached when the update has failed, the target Line 6 product
id masked with `0x7F0000` equals `0x70000`, a nested device pointer is
present, and one unsigned 16-bit field equals 18,000. 18,000 is `0x4650`,
the PODxt Live USB product id recorded in the driver notes. The same short
is later passed with vendor `0x0E41`. Windows 1.77 `FUN_00434810` stores
that vendor at offset `0x28` and the short at offset `0x2A`.

| Build | Function | USB product-id load |
| --- | --- | --- |
| Windows 1.77 | `FUN_0041edf0` | `*(short *)(record[3] + 6) == 18000` |
| Mac 1.78 | `FUN_0001e710` | `*(short *)(record[3] + 6) == 18000` |
| Windows 1.33 | `FUN_004914c0` | `*(short *)(record[2] + 0x106) == 18000` |

The field this check reads is that USB product id, together with the Line 6
product-id family in the first word of the update record. It does not read
the MIDI identity version. The owner unit is 3.01. The 2.11 image is still
absent from the local trees, so this dialog supplies no relay listing for
that older flash. What firmware 3.01 does with `03 23` on the device stays
**unknown**.

### #225 and the #234 stepped read

**Confirmed** against the #225 record above: that run sent `03 23 01` before
`F0 00 01 0C 07 00 44 7F F7`, and it sent `03 23 00` during cleanup. Those
are the large-buffer enable and disable. **Likely:** a missing large-buffer
command does not explain the silent `0x44`. The host function retries an
outbound transfer, and the Monkey writers place the pair around outbound
code and patch writes. The #176 run already carried a 6,566-byte `0x43`
from host to guitar on firmware 3.01 after the same enable. **Unknown:**
whether the device changes guitar-to-host capacity when it sees the frame,
and whether the updater's 1,000 ms sleep changes that. The #225 request was
at 0.181 s, so the run did not copy that pause. One silent request still
leaves the opcode filter and a reply-size cap unseparated.

[#234](https://github.com/guirossibrum/VariaxWorkbench/issues/234)'s draft
stepped `0x3C` read, in `docs/reso5-full-read-test-card.md`, already sends
`03 23 01`, waits 100 ms, requests 64, 128, 256, 512, and 1,092 words
(expected replies 392, 776, 1,544, 3,080, and 6,560 bytes), and sends
`03 23 00` afterwards. Those two frames are the large-buffer pair, so the
stepped read needs no further command and no SysEx reply to `03 23 01`.
The updater behavior the draft does not copy is the 1,000 ms sleep after a
successful enable. That sleep is a thread delay. The 200-byte `0x3C` reply
already returns on this route without it. Whether the longer steps need the
longer pause is **unknown**. This note leaves the draft's stop at the first
timeout in place. It adds no sleep, changes no tool, and approves no live run.

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
interface, Workbench mode 0. This note adds no retry of `0x44` over the
PODxt and no new `03 23` value. The large-buffer frames are the `03 23`
pair already recorded here. Nothing in this note is sent to the rig. A
bounded owner check for #221 and #222 still needs its own approval.
Keep-current, F-, U-, and Custom stay distinct, as #218 recorded. The two
F- models above are the ones with standalone recall evidence.
