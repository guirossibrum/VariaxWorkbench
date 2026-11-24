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
