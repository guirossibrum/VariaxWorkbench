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
