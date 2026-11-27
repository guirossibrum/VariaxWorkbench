# Rig Check: the device connection on Windows and macOS

[#262](https://github.com/guirossibrum/VariaxWorkbench/issues/262) · research
report only, no code. Evidence labels: **Static fact** (a decompiled line or
an existing repository finding), **Inference**, **Unknown**.

Sources. The decompiled Workbench 1.75 Java trees under
`analysis/decompiled/windows/` and `analysis/decompiled/mac/` and the native
MIDI library functions under `analysis/decompiled/native-midi/functions/`
are local and stay uncommitted; citations give their path and line. Driver
evidence comes from the committed studies
[driver-windows-podxt.md](../driver-windows-podxt.md),
[driver-macos-podxt.md](../driver-macos-podxt.md) and
[podxt-variax-relay.md](../podxt-variax-relay.md). No device, driver or
network was used.

## 1. How the original editor reaches the rig

**Static fact.** The Java device layer is identical on both platforms:
`diff -rq analysis/decompiled/windows/cmn/l6java/midi analysis/decompiled/mac/cmn/l6java/midi`
reports no difference. All I/O goes through the native `L6MIDIJava` library,
which offers two kinds of port:

| Port kind | Entry point | Evidence |
| --- | --- | --- |
| Plain MIDI in/out port pair (WinMM, CoreMIDI) | `native_Open(instance, in, out)` | `cmn/l6java/midi/L6MIDIJava/L6MIDIJava.java:81`, `:273-293` |
| "Line 6 hardware" (the Line 6 driver's private interface) | `native_OpenL6HW(instance, index)` | `L6MIDIJava.java:83`, `:312-329`; `L6MIDIManager.java:79-82` |

**Static fact.** `L6VariaxDevice.GetConnectionMode` (`modules/variax/device/L6VariaxDevice.java:191-197`)
picks the guitar route from the port kind: Line 6 hardware with id 21325
(the stand-alone "Variax USB Interface") is mode 0, any other Line 6 hardware
(the PODxt Live) is mode 1, a plain MIDI port pair is mode 2 (Vetta).
`OnInit` (`:94-106`) for mode 1 sets the host send chunk to 256 bytes and
sends the PODxt relay enable `F0 00 01 0C 03 23 01 F7`, then opens the guitar
session (`EnableVariaxMIDI`, `:120`). The full table is in
[podxt-variax-relay.md](../podxt-variax-relay.md#workbench-connection-modes).

**Inference.** On Windows and macOS, Workbench talks to the PODxt Live as
*Line 6 hardware* through the driver, and the guitar's SysEx travels in the
POD's own MIDI stream, relayed by the PODxt after `03 23 01`. It does not
use the plain MIDI ports for the PODxt Live. On Linux, Rig Check and the
editor instead claim the PODxt Live's **interface 1** (`snd-usb-variax`'s
interface, endpoints `0x05/0x86`) for the guitar and use the ALSA rawmidi port
of interface 0 for the POD. The two routes carry the same SysEx bytes but are
different transports.

**Static fact (macOS native).** The native library has a separate direct
guitar sender, `L6DirectMidiMac_SPSI::BuildAndSendVariaxMidi`
(`native-midi/functions/0x3462e.c`), which rewrites a 4-byte header (byte 3 =
`'B'`, byte 2 = `0x81`, a length word) and calls the driver's command
interface with selector `0x90000`; `L6DrvMsgServiceMac::SetVariaxDirectMidi`
(`0x2fefa.c`) installs it, and `L6DeviceManagerMac::RegisterForVariaxInterface`
(`0x2ad70.c`) registers for the guitar interface. The guitar-present check
used by `IsGuitarConnectedToUSBInterface` is
`native_SpecailCheckVariaxConnection` → `L6SeaMonkeyHW::IsGuitarPresent`
(`0x2572.c`). **Inference.** These serve the stand-alone Variax USB interface
(mode 0, "SeaMonkey"), not the PODxt Live relay.

## 2. Does the same SysEx work through the driver's MIDI ports?

- **Static fact.** The bytes Rig Check sends are the bytes Workbench builds:
  identity, `2A 01/00`, `0x3C` reads, `0x43` full program (6,566 bytes), `0x61`
  store, `0x71` wait ([podxt-variax-relay.md](../podxt-variax-relay.md#host-send-chunk-and-host-receive-buffers)).
- **Static fact.** In mode 1 Workbench splits every outgoing SysEx into
  256-byte pieces (`MIDIEngineBase::SendSysex`, chunk field set by
  `SetSysexChunkSize(256)`, `L6VariaxDevice.java:99`). The macOS kernel's
  `WriteMIDI` accepts at most 256 bytes per call
  ([driver-macos-podxt.md](../driver-macos-podxt.md), table "Framing").
- **Inference.** The relay route works for reads and stores as long as the
  host keeps the 256-byte send split, because that is the route Workbench used
  on both platforms for the PODxt Live. The device-side limit seen on Linux
  (guitar replies over about 256 bytes are dropped by the relay, #225/#234) is
  in the PODxt, so Rig Check must keep reading in 32-word chunks (200-byte
  replies) on every OS.
- **Unknown.** Whether a *third-party* program that opens the driver's
  *plain* MIDI ports (WinMM "Line 6 PODxt Live", the CoreMIDI device the
  `Line6MIDI` plug-in creates) reaches the same relay with the same results
  as Workbench's Line 6 hardware path. The kernel MIDI path on macOS runs on
  interface 0 ([driver-macos-podxt.md](../driver-macos-podxt.md)), which is the
  POD side where the relay lives, so this is plausible but not proven. The
  WinMM port names, the exact per-message USB segmentation and whether the
  driver passes SysEx longer than its internal buffer unchanged are also
  unknown.
- **Unknown.** Whether macOS or Windows let a user-space program claim
  interface 1 while the Line 6 driver owns the device (libusb on Windows needs
  a WinUSB driver bound to that interface, which would replace the Line 6
  driver for it).

## 3. Buffer limits that matter

| Limit | Value | Evidence |
| --- | --- | --- |
| Host send split (mode 1) | 256 bytes | `L6VariaxDevice.java:99`; relay doc |
| macOS kernel `WriteMIDI` per call | at most 256 bytes | driver-macos-podxt.md |
| Native receive buffer | 131,072 bytes ring, 512-event ring | relay doc, `MIDIEngineBase::Init` |
| PODxt relay guitar reply | about 256 bytes (replies above are lost) | Rust memory of the #225/#234 owner runs; `span_read` uses 32 words |
| Largest Workbench write | `0x43`, 6,566 bytes | relay doc |

## 4. What Rig Check must detect in steps 0.2 and 1.x

**Windows**
- The Line 6 driver package and version (`pnputil /enum-drivers`, `l6podxt.inf`,
  4.2.7.7 is the last known), its signature era (SHA-1, 2019) and whether the
  device is started (`pnputil /enum-devices /connected`, instance
  `USB\VID_0E41&PID_4650`). Implemented as parsers in #251.
- The MIDI port names exposed by WinMM (expected to contain
  "Line 6 PODxt Live"), and the ASIO driver "ASIO PODxt Live" for step 8.1.
- Whether another program holds the ports (WinMM opens are exclusive).

**macOS**
- macOS version and CPU: the Line 6 7.6.8 package is Intel-only
  (`Line6Audio` kext x86_64, `Line6MIDI` plug-in x86_64/i386). On Apple
  Silicon there is no native driver; Rig Check must report "no supported
  driver for this Mac" rather than try.
- Whether the `com.line6.driver` kext or a system extension is loaded
  (`kextstat`, `systemextensionsctl list`), and the CoreMIDI device the
  plug-in creates.
- The USB location ID from `system_profiler SPUSBDataType`.

## 5. Recommended adapter design

1. **Keep `line6-devices` unchanged above the `Link` trait.** Add, per OS, one
   thin `Link` over the OS MIDI API for the PODxt Live's port:
   WinMM (`midiInOpen`/`midiOutLongMsg`) on Windows, CoreMIDI on macOS. Use
   the existing `Path::PodxtMidi` for POD messages.
2. **Add a separate path for guitar messages relayed in the POD stream**, for
   example `Path::VariaxViaPodxtMidi`, instead of reusing `VariaxViaPodxt`
   (interface 1). The procedures are the same; the transport differs, and the
   relay enable (`03 23 01` / `03 23 00`) must wrap every guitar session on
   this path, as Workbench's mode 1 does. Both paths need the 256-byte send
   split (already `BRIDGE_CHUNK` in `family/variax/write.rs`).
3. **Dependencies.** Evaluate `midir` (pure-Rust front over WinMM and CoreMIDI,
   maintained) against two small own bindings (`windows-sys` WinMM, the
   `coremidi` crate). Under AGENTS.md's portability rule, prefer the smallest
   pinned dependency with a replacement path; either keeps the adapter thin.
4. **Rig Check wiring.** `RealPort::podxt` and `RealPort::variax` gain
   Windows/macOS branches returning these links; nothing above changes.
   Steps 1.1/1.2 become "MIDI port present / opened and closed" on these
   systems.
5. **First owner tests, in order** (each needs the owner's approval, #267):
   identity of the PODxt on the MIDI port (G2); identity of the guitar through
   the relay; one 32-word `0x3C` read; the 35-chunk read; only then any live
   or save step. Stop if the relay path drops replies differently from Linux.

## 6. Open questions for the owner

- Is Windows or macOS first? macOS on Apple Silicon has no driver at all.
- May Rig Check depend on the Line 6 driver being installed on Windows and
  Intel macOS (AGENTS.md says normal operation must not *require* Line 6
  services; a test tool for those platforms may have to)?
- Should a WinUSB route to interface 1 (replacing the Line 6 driver for that
  interface) ever be considered? It would match Linux but changes the
  user's driver setup.

## 7. Windows PODxt Live endpoint association for the identity proof

**Static evidence.** The reviewed x64 Line 6 4.2.7.7 INF matches the whole
PODxt Live USB device, registers the `L6PODLV` service and declares WDM/KS
MIDI support. The archived Windows JNI libraries for Workbench and Line 6
Edit import WinMM long-message calls. The review reproduced the pinned
static checks at `a1b9e09c93e417ccf7eeee4e0c97cbeeb10f353d`. The 17-byte
PODxt identity reply is Linux reference evidence.
[Driver study](../driver-windows-podxt.md) · [Static identity review](https://github.com/guirossibrum/VariaxWorkbench/issues/262#issuecomment-7222575898) · [Endpoint-mapping review](https://github.com/guirossibrum/VariaxWorkbench/issues/262#issuecomment-7224144417)

**Proposed admission rule.** Resolve both directions before opening any
MIDI port.

1. Enumerate real WinMM input and output IDs independently with
   `midiInGetNumDevs` and `midiOutGetNumDevs`, excluding the mapper. Call
   `midiInMessage` or `midiOutMessage` with the directional ID
   cast to its handle type. Query `DRV_QUERYDEVICEINTERFACESIZE` followed by
   `DRV_QUERYDEVICEINTERFACE`. Accept only a stable byte count and a bounded,
   nonempty, NUL-terminated UTF-16 path. Treat each path as opaque.
   [WinMM path queries](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/obtaining-a-device-interface-name)
2. Resolve each path with `SetupDiOpenDeviceInterfaceW` and
   `SetupDiGetDeviceInterfaceDetailW`. Walk from the owning devnode with
   `CM_Get_Parent` to the whole USB device with hardware ID
   `USB\VID_0E41&PID_4650`. Count present PODxt Live units independently of the
   MIDI ports. Require exactly one present unit, `DN_STARTED` set and
   `DN_HAS_PROBLEM` clear, with service `L6PODLV` and driver version
   `4.2.7.7`. The version is OS-reported package metadata.
   [SetupAPI lookup](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendeviceinterfacew) · [Owning devnode](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdigetdeviceinterfacedetailw) · [PnP ancestry and instance IDs](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/retrieving-device-relations) · [Devnode status](https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_get_devnode_status) · [Service property](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-service) · [Driver version property](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-driverversion)
3. Require exactly one input ID and one output ID that independently resolve
   to the same full USB instance ID (`CM_Get_Device_IDW`), including its
   suffix. Paths may differ or be shared by several ports. Count IDs per
   direction. Names, capability
   fields, mapper eligibility, matching ordinals and container GUIDs are
   insufficient to establish the pairing. Stop before opening either port
   if a direction lacks a usable path to the target, the instance or either
   direction is ambiguous, or the target fails the readiness or driver
   checks. Recheck the inventory and both mappings immediately before open
   and again before any send. Refuse a changed snapshot. WinMM offers no
   atomic verified pair open.
   [WinMM numbering and updates](https://microsoft.github.io/MIDI/kb/moving-from-winmm-to-wms/#discovering-and-identifying-midi-sources-and-destinations) · [Device-ID query rules](https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/system-intercepted-device-messages)

**Windows MIDI Services is diagnostic.** Where available, examine each
`MidiLegacyPortDeviceInformation` row by `(Flow, Number)` and follow its
`DriverDeviceInterfaceId` or the associated UMP endpoint's
`ParentDeviceInstanceId` into the PnP tree. This can corroborate the USB
ancestry or explain incomplete publication and disagreement. Legacy port
publication and WinMM numbering can change independently. A service-only
match cannot supply a missing or conflicting WinMM path. A valid WinMM pair
can qualify when the service API or its legacy rows are unavailable.
[Legacy port properties](https://microsoft.github.io/MIDI/sdk-reference/Enumeration/Legacy/MidiLegacyPortDeviceInformation/) · [Endpoint properties](https://microsoft.github.io/MIDI/sdk-reference/Enumeration/MidiEndpointDeviceInformation/) · [WinMM integration limits](https://microsoft.github.io/MIDI/kb/moving-from-winmm-to-wms/#discovering-and-identifying-midi-sources-and-destinations)

**Native verification pending.** The pinned driver's Windows x64 loading,
WinMM names, counts and directional paths, service rows, identity exchange
and long-message teardown remain unverified. The association would identify
the physical PODxt unit. Actual routing to PODxt USB interface 0, the guitar
relay over the POD stream and the private guitar interface 1 need separate
evidence. The Windows package remains offline for device actions.
[Windows package](../windows-package.md) · [Endpoint-mapping review](https://github.com/guirossibrum/VariaxWorkbench/issues/262#issuecomment-7224144417)

### Proposed synthetic fixtures (eight cases, all unrun)

Use injected API results. Admission selects a pair for a later proof.
Every refusal yields zero MIDI opens and zero sends.

| Fixture | Expected result |
| --- | --- |
| Input 2 and output 5 resolve through different child nodes to started Live A with the pinned driver. Repeat with a shared path and renamed ports. | Admit A using the full instance ID and directional IDs. |
| Identical-name Live units A and B share a hardware ID, with input on A and output on B. Repeat with a complete pair on A and an unresolved port on B. | Refuse. Distinguish split ancestry from multiple present units. |
| Two input IDs share A's filter path, or a second output also resolves to A. | Refuse the ambiguous direction, counting IDs even when paths match. |
| A direction is missing, or a path is empty, unsupported, malformed, unresolvable or resolves only to a software node with no Live ancestor. Caps, mapper eligibility or container GUID agree. | Report the missing direction or unavailable association. A positively resolved unrelated port does not contribute to the pair. |
| Paths reach A, but the unit is absent, stopped, has a PnP problem, or reports a different service or a missing or different driver version. | Refuse with the specific readiness or driver result. |
| Service rows alone map both directions through a UMP endpoint to A. Repeat with incomplete rows, a renamed port, changed indices and a legacy mapper offset. | Report service ancestry where supported and flag disagreement or incomplete publication. Refuse without independent qualifying WinMM paths. |
| The service API is absent, in Legacy mode, or has no PODxt row in Hybrid mode. Valid WinMM paths map to A. | Admit using WinMM. |
| A rescan changes a path, instance ID, readiness or index. Include unplug and replug with the same label. | Refuse the stale selection. |
