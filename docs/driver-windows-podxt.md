# PODxt Live Windows driver

Evidence labels describe the strength of each result. **Static fact** is a file, instruction or previously documented offline observation. **Inference** is an interpretation of that evidence. **Unknown** requires more evidence. Addresses are virtual addresses unless marked RVA. Scope is the locally extracted PODxt-family 4.2.7.7 package.

## Inventory

**Static fact.** `l6podxt.inf` declares `DriverVer=03/15/2019,4.2.7.7` and `CatalogFile=l6podxt.cat`. Its only manufacturer decorations are `NTx86` and `NTamd64`. Both match VID `0x0e41`, with PIDs `0x5044` (PODxt), `0x5050` (PODxt Pro) and `0x4650` (PODxt Live). These are whole-device IDs, without an `MI_XX` suffix. The Live install sections select the demand-start kernel service `L6PODLV` and use the bitness-specific service binary.

| Binary | Static fact — PE machine, version and external interface | Inference — loader |
| --- | --- | --- |
| `L6PODLV.sys` | i386 PE32 native, 4.2.7.7, entry RVA `0x8a10c`, no PE exports | Windows PnP via x86 service |
| `L6PODLV64.sys` | AMD64 PE32+ native, 4.2.7.7, entry RVA `0xbaba4`, no PE exports | Windows PnP via x64 service |
| `l6podlv.dll` | i386 PE32 DLL, 4.2.7.7, COM exports below | 32-bit ASIO host via COM on x86 or WOW64 |
| `l6podlv_x64.dll` | AMD64 PE32+ DLL, 4.2.7.7, COM exports below | 64-bit ASIO host via COM |

**Static fact.** Both SYS files import `NTOSKRNL.exe`, `HAL.dll`, `STREAM.sys` and `USBD.sys`. The USB imports include `USBD_CreateConfigurationRequestEx` and `USBD_ParseConfigurationDescriptorEx`. Both DLLs import WinMM, SetupAPI, Version, Advapi32, Kernel32, User32 and Ole32. Their export ordinals 1–5 are `DllRegisterServer`, `DllUnregisterServer`, `DllCanUnloadNow`, `DllGetClassObject` and `DllMain`. The DLLs are registered as in-process servers for `ASIO PODxt Live` and have no independent USB hardware match. No ARM64 image or INF match appears in this package.

### Catalog and installation

**Static fact.** The catalog's PKCS#7 SignedData shows SHA-1 and a countersignature signing time of 2019-03-15 15:12:53 UTC. Embedded certificates include Line 6, DigiCert's High Assurance Code Signing CA-1 and a DigiCert EV Root cross-certificate issued by Microsoft Code Verification Root. Both SYS binaries also carry PE certificate tables.

**Inference.** The catalog is evidence of a legacy signed distribution. Its age, SHA-1 and cross-signing make installation on a current Windows 10/11 x64 configuration a compatibility question. The local records alone do not establish acceptance by a particular build, security configuration or driver store.

**Unknown.** Catalog member verification against these extracted copies, current policy, trust and revocation, actual PnP ranking, and installation or loading on a current Windows system.

**Static fact.** The saved [Linux transport evidence](linux-transport.md) splits PID `0x4650` across two interface drivers. **Unknown.** The Windows whole-device INF match alone gives neither the selected alternates nor the interface used for Variax traffic.

**Static fact.** Inventory can be reproduced with `llvm-readobj --file-header --coff-imports --coff-exports --coff-resources`, `strings -el`, `openssl pkcs7 -inform DER -print_certs`, `openssl asn1parse -inform DER` and the INF.

## MIDI, kernel streaming and ASIO

| Evidence | Finding |
| --- | --- |
| **Static fact** — Live INF install and interface sections | The x64 section includes `ks.inf,wdmaudio.inf`, needs `KS.registration,WDMAUDIO.registration`, registers audio/render/capture categories, supplies `Line 6 PODxt Live` as the Wave and UART friendly name and MIDI description, and associates the MIDI subclass with `wdmaud.drv`. |
| **Static fact** — SYS | `0x14290` calls the `StreamClassRegisterAdapter` thunk. MIDI subtype GUID `1D262760-E957-11CF-A5D6-28DB04C10000` occurs at file offset `0xa0340`. Code at `0x17d27`, `0x17db1`, `0x17df1` and `0x271e3` references the internal names `MidiOut`, `MidiOutBridge`, `MidiInBridge` and `MidiIn`. |
| **Inference** — WinMM presentation | WDM/KS and the Windows MIDI mapper layer expose the driver's MIDI ports. `Line 6 PODxt Live` is the registered name to look for. |
| **Unknown** — WinMM display name | Actual `midiInGetDevCaps` and `midiOutGetDevCaps` strings, possible suffixes and how many ports a particular host enumerates. The internal SYS object names are separate evidence. |
| **Static fact** — ASIO | Both ASIO registration views use CLSID `{E3F0119A-B92E-4618-ACA5-2CCCC58D4B92}` and the display name `ASIO PODxt Live`. The x64 DLL enumerates a SetupAPI device interface at `0x180009a51`–`0x180009b03`, opens a path via `CreateFileA` at `0x180008e50` and invokes `DeviceIoControl` with KS property IOCTL `0x2f0003` at `0x180008eb6`. Its property-set GUID at `0x18001e4a0` is `CE62E3F1-C013-4D0A-B46A-36DE15F8EC58`. Both DLLs' WinMM imports are timing-related (`timeGetTime`). |

### Original editor and downstream guitar

**Static fact.** The local Windows editor JNI (`L6MIDIJava.jnilib.win`) both imports and calls WinMM MIDI functions. Examples are `midiInOpen` at `0x10005ee8`, `midiOutLongMsg` at `0x100060d8`, `midiOutGetDevCapsA` at `0x1000630f` and `midiOutOpen` at `0x10006482`. Its other paths also call `DeviceIoControl`. Observed codes include `0x222010` (`0x1001b0df`), `0x222018` (`0x1001e3ab`) and `0x222008` (`0x1001e989`). The image contains `L6SMDirectMidiServiceWin` and `L6XTVariaxMIDISvcImpl` RTTI names. **Unknown.** Which, if any, of those private IOCTL callers reach this release of `L6PODLV64.sys` for PID `0x4650`. The JNI supports multiple device families.

**Static fact.** The [original editor traces](investigation.md) identify the PODxt Live routing command `F0 00 01 0C 03 23 01 F7`. The [transaction trace](variax-write-transactions.md) records that the editor opens the bridge before the guitar session and emits a corresponding release. The Mac native application's `L6DirectMidiMac_SPSI::BuildAndSendVariaxMidi` at i386 `0x3462e` builds driver-specific messages, as described in [native analysis](native-analysis.md). That symbol and its packing are evidence for the Mac backend.

**Inference.** The best Windows application candidate is the Line 6 MIDI endpoint with the original application-layer bridge sequence. On that route the host sends PODxt MIDI SysEx, the PODxt changes its routing, and the host exchanges the bounded guitar message through the PODxt. ASIO is an audio path. **Unknown.** The exact Windows JNI dispatch selected for each original editor build, whether that dispatch uses a special Line 6 Variax command, and the corresponding physical USB interface. The [Linux transport](linux-transport.md) can address the guitar directly through interface 1, whose relationship to these Windows host ports still has to be established.

**Inference — device risk.** A WinMM MIDI endpoint can carry state-changing SysEx. Routing enable/release can affect downstream control. An ASIO open and the undocumented private requests have independent side effects to account for.

## Attach and start traffic

**Static fact.** In `L6PODLV64.sys`, the constructor at `0x149a8` reads the USB device descriptor and feeds its PID to `0x8d038`. The compiled subtraction ladder at `0x8d372`–`0x8d390` resolves `0x4252 + 0x3f0 + 0x0e = 0x4650`, taking `0x8d417`. That profile sets the second-interface branch flag and retains interface-0 alternate 1. The constructor builds interface registrations from these flags. `0x15834` is referenced by the function pointer at `0xa0978` and performs configuration and subsequent bring-up.

| Phase or path | Static fact | Inference or unknown — device risk |
| --- | --- | --- |
| Device descriptor | `0x93ee8` submits URB function `0x0b`, type 1, index 0, length 18 via `0x937f8`. Callers include the constructor at `0x150e8`, configuration routine at `0x941b5` and later initialization at `0x15bde`. | **Inference.** Standard descriptor reads have low impact on device settings. |
| Configuration descriptor | `0x93f4c` submits type 2 with requested length `0x400`. `0x941dd`–`0x9420d` iterates indices until descriptor configuration value 1. The saved Live descriptor has one configuration. | **Inference.** The Live configuration read uses index 0. |
| Configuration selection | `0x94058` parses the registered interface/alternate pairs with `USBD_ParseConfigurationDescriptorEx`. `0x9463b` constructs a select-configuration URB with `USBD_CreateConfigurationRequestEx`. `0x94736` submits it and the following code stores the returned interface/pipe data. The Live profile registers interface 0 and, through the branch at `0x15286`–`0x1534d`, interface 1 with alternate 1. | **Inference.** Windows USB stack configuration can activate both interface alternates and their endpoints during driver startup. Exact packet order emitted by the USB stack is unknown. A concurrent raw claim on interface 1 would contend with this path. |
| Pipe registration | The PID-specific branch registers `03/84` and `05/86` at `0x1530e`, `0x15323`, `0x15338`, `0x1534d`. The same profile registers `01/82` at `0x1540b` and `0x15420`. Pipe matching at `0x943a1` onward uses descriptor endpoint addresses. The saved descriptors place `01/82` (isochronous) and `03/84` (interrupt) on interface 0 alt 1, and `05/86` (interrupt) on interface 1 alt 1. | **Unknown.** Which MIDI IN URBs begin before application open versus on stream open, their first payload and exact completion/requeue sequence. Pipe allocation establishes ownership and does not by itself establish a transfer. The `87` and alternate audio endpoints in other constructor branches are not the Live profile. |
| Vendor control OUT | `0x159e6` passes index 2 to `0x24c98`. Its builder `0x1ac78` sets URB function `0x17`, vendor/device request `0x67`, value `0x0201`, index `0x0002`, direction OUT and length zero. It submits at `0x24d1a`. | **Unknown.** Meaning and persistence of the device-side change. This is a control write that startup can issue before a MIDI application opens a port. |
| Standard feature OUT | `0x15a12`–`0x15a3d` submits URB function `0x0d` (SET_FEATURE to device), feature selector 1, index 0, no payload. The saved configuration reports self-powered and has no remote-wakeup capability bit set. | **Inference.** Feature selector 1 requests device remote wakeup. **Unknown.** Stack/device handling and whether the request succeeds on the captured revision. Startup branches on the returned status. |
| Later interface changes and cleanup | `0x94ce0` constructs a select-interface URB and submits at `0x94dfd`. Callers include `0x1fb80` and `0x20198`. Teardown routines call `0x94a58` with a null configuration and submit abort-pipe URBs on existing handles. | **Unknown.** The complete conditions, interface/alternate values for later stream or power transitions, and effects of default OS audio clients. |

**Static fact.** Windows' Live branch contains explicit setup for the same interrupt endpoints that the [Linux driver](linux-transport.md) matches on interface 1. Linux can bind that interface independently of interface 0. The Windows driver has a whole-device INF match and includes both interfaces in its setup. **Inference.** The operating-system ownership models differ even though the descriptor endpoint addresses agree. **Unknown.** Whether Windows workloads can safely share this interface and whether the static bring-up list captures every power, reset or error-recovery path. No actual USB timing or device response was measured.

### MIDI polling and the second interface

| Static fact — follow-up to the attach and MIDI traces | Inference or unknown |
| --- | --- |
| `0x813d4` calls `0x17448` while its reader count is nonzero. That routine submits bulk/interrupt URB function 9 through `0x94f80` using the pipe object at `+0x358`, which the Live constructor registers as `0x84`. The first-reader branch at `0x811b4` calls `0x17530` (abort of that input pipe) and `0x243a8` with argument 1, then signals the worker. `0x243a8` constructs vendor OUT `40 67 01 01 00 00 00 00`. The last-reader branch at `0x81330` can call the same helper with argument 0. | **Inference.** Stream activation can change USB state before any application SysEx. **Unknown.** Exact binding/stream timing under a current Windows host. |
| `0x28488` creates the second-interface MIDI context after profile bring-up. The later path `0x286b8 → 0x28840 → 0x81d8c` submits a 16-byte async input through the pipe object at `+0x480`, registered as `0x86`. Its callback `0x289fc` has a requeue branch. `0x28574 → 0x28f28 → 0x17810` has a path building a bulk/interrupt request from supplied bytes and the pipe object at `+0x460`, registered as `0x05`. | **Inference.** The second interface has its own MIDI transport inside the driver. **Unknown.** Which application-facing selector starts it and how the historical JNI chooses it. The OUT-associated helper still supplies transfer flags 3 at `0x94fc9`, which includes the WDM direction-IN bit. The descriptor specifies OUT. Static inspection has not resolved Windows stack treatment of that combination. |

**Static fact.** The `0x15834` bring-up also contains a conditional call at `0x15bff` to `0x243a8` with argument 3. This constructs vendor OUT `40 67 01 03 00 00 00 00`. The flags tested around that call start clear in the profile initializer and are unchanged by the Live PID branch. **Unknown.** Whether a later initialization path enables them. The `0x0101` first-reader request matches the corresponding macOS MIDI enable. It differs from the Windows attach request `0x0201` at index 2 and the Linux identity/serial startup sequence.

**Static fact.** The original Windows editor JNI has a PID-specific private candidate in addition to its WinMM imports. Its factory at `0x10009770` checks PID `0x4650` and the MIDI identity family field. For family 7 it reaches the `L6XTLiveVxControl` constructor via `0x10009a0b`. That constructor calls the `L6XTVariaxMIDISvcImpl` constructor at `0x100129a7`, passing GUID `FFD88D0F-54CE-4150-90A3-67542EDEB4B8`. The base service stores the GUID and resolves a device path. The same GUID is at file offset `0x92b18` in the x64 SYS and `0x705b8` in the x86 SYS. It differs from the ASIO property set. **Inference.** This is evidence for a dedicated editor-to-driver Variax service, alongside ordinary WinMM and the application-level bridge. **Unknown.** The complete property dispatch and data path tying that GUID to the second interrupt pair and the exact policy determining which editor session uses it.

## Adapter and gate equivalents

**Inference — recommended Windows x64 adapter.** Use the installed Line 6 WDM/KS MIDI ports through WinMM. The vendor service supplies the mapping and owns both USB interfaces. Use the editor's `03 23` MIDI bridge as the candidate public route for the Variax. Treat direct use of the private second-interface channel as a separate research path. ASIO belongs to a future recording adapter. **Unknown.** Whether the catalog will admit the legacy driver on a given current x64 host and whether the public MIDI route reaches both downstream operations successfully. The supplied package has no native Windows ARM64 driver.

| Equivalent | Inference — proposed app operation and system effects |
| --- | --- |
| G1 — recognition | With an already installed driver, read the OS PnP device record for `USB\\VID_0E41&PID_4650` and cached or OS-exposed port descriptions. This has no app MIDI handle or app-originated USB command. Driver staging is a separate prerequisite with persistent INF/catalog, service and registry effects. Connecting the unit can trigger PnP binding and the startup traffic above even when the app is closed. |
| G2 — identify the Variax | After a reviewed G1, open the selected Line 6 WinMM input/output, allocate and queue host MIDI buffers, then use a bounded, separately approved routing and identity exchange. Opening input may trigger vendor enable and polling. A bridge exchange can change the POD routing state and needs a defined release and failure case. Close both handles and return host buffers. Interface ownership stays with the vendor driver. |
| G3 — read current settings | After successful G2, use the same approved transport to read current Variax metadata and the current POD tone. Bound each response and clean up the routing and host session. The proposed app operations require no installer, PnP rebind, audio stream or stored preset write. |

**Unknown.** Exact WinMM name and instance association, editor selection of the public or private route, behavior after an interrupted bridge session, all stream/power/error transitions, and downstream response behavior on Windows. The Linux gate results supply a reference for request scope only. These proposed equivalents require their own owner-reviewed procedures before use.
