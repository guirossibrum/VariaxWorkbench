# Line 6 cross-product package evidence

Partial static research milestone for [#61](https://github.com/guirossibrum/VariaxWorkbench/issues/61).
Capability matrix version: `line6-static-capability-matrix-v1` (2026-11-16).

This records what the five supplied packages and their bundled documentation say.
It does not establish a connected device's identity, successful communication,
physical compatibility, firmware state, audio behavior, or recovery capability.
The Windows/macOS vendor binaries were not executed or loaded. No installer was
run, DMG mounted, driver installed, service launched, catalog contacted, or
USB/MIDI/audio interface opened.

## Artifact identity and integrity

The issue's supplied byte sizes and SHA-256 references were checked against both
private inventoried copies of each original (10 file checks). Every copy matched.
The AC-prepared private inventory SHA-256 was
`ae6f747b2dcf4d52244e6106f11465eadfdb4654983467310695bd9e31c59184`.
These checks establish byte identity with the supplied references, not publisher
identity, signature validity, or independent product provenance. The supplied
files remain private and ignored.

| Supplied package | Bytes | SHA-256 | Independently visible package identity |
| --- | ---: | --- | --- |
| Variax USB Interface Driver v1.0.7.2 Installer.exe | 3,336,616 | `fb376b29ed264aaf65c4c71aa4dfe27fd29ec0fef4048d95ee55df634fc733ae` | NSIS 2 wrapper; extracted INF says driver 1.0.7.2. |
| Workbench HD v2.15 Installer.exe | 21,693,496 | `7d197848079a73fda2c8160ae065dd42af9159125d2f9c9b63aa4f386f4dccf3` | NSIS 3 wrapper; embedded app resources identify Workbench HD 2.15. |
| Workbench HD 2.15.dmg | 20,774,114 | `236cd73563e7b47c569f472ca3ef99c1e50de31a0b1f079555b11b4330462dfb` | `Workbench_HD.pkg/PackageInfo` and app `Info.plist` both identify 2.15; app declares macOS 10.8 minimum. |
| Line 6 Central 1.06.dmg | 70,019,033 | `34c77824799f044f7e94631e2b1b31cad2535faea5687f8932f9194fce477457` | PackageInfo and app plist identify bundle 1.06 (package version 1.6.0); universal x86_64/arm64 app, macOS 11.0 minimum. |
| Line 6 Central 1.06 Installer.exe | 40,148,464 | `e9a7269e4ff670b5421c79f24196323cfcb0d663159857b3e7de85aad818ebfd` | NSIS 3 wrapper; embedded .NET assembly identifies Line 6 Central 1.6.0.0 / .NET Framework 4.8. |

Extraction used the inventory-pinned 7-Zip 26.06 tool. The five extracted trees,
nested macOS package metadata and gzip/cpio payloads are under ignored
`.tools/issue-61-*` paths. Extraction outputs were staged in new temporary
directories and published by a non-overwriting directory rename. No proprietary
payload was added to this repository.

## Versioned capability matrix

| Instrument / identity evidence | Connector or bridge | Editor/backend and host | Recognition / editing / library evidence | Maintenance evidence | Audio / physical status |
| --- | --- | --- | --- | --- | --- |
| Variax guitar; exact attached model identity is not established by the driver INF | Standalone Variax USB Interface, Windows USB VID `0x0E41`, PID `0x534D` | Workbench HD 2.15 package for Windows and macOS. The Pilot's Guide says to connect a Variax through its Variax Digital Interface (VDI); its general workflow describes model editing and upload/download. | The manual establishes intended Workbench HD editing. This pass did not recover a complete per-instrument USB identity table, protocol, or compatibility list. JTV names occur in editor resources, but the exact supported JTV model set is not recovered here. | Workbench HD installation bundles Line 6 Monkey and License Manager packages on macOS and driver files on Windows. This is installation composition, not an update result. | No audio path is specified by these sources. No hardware operation was performed. |
| Variax Tyler (`0x70005`), Variax 500/700 (`0x70000`), Acoustic (`0x70001`), Bass (`0x70002`), 300/600 (`0x70004`), Bass 705 (`0x70006`) | Central's `Variax.json` compatibility keys; they are not identified as USB VID/PID values | Line 6 Central 1.06 on Windows and macOS | Both platform packages contain semantically equal `Variax.json` compatibility data: each row has `minAppVersion=1.01.9f` and `minInstalledFirmwareVersion=0`. This is updater compatibility metadata, not proof that Central edits presets or successfully recognizes a connected instrument. | The same JSON declares a four-stage update process (`sendEula`, `sendKeepPresetsOption`, `sendReportEngineProgressOrder`, `sendUpdateCompletedHTML`). Static app strings include USB/flash update UI and firmware-version fields. No catalog was contacted and no update was started. | No audio capability established. A table row does not establish physical compatibility. |
| Variax attached through a VDI-input device | Manual-named VDI hosts: POD HD Pro, POD HD 500, POD X3 Live, POD X3 Pro, Vetta II | Workbench HD Pilot's Guide, Windows and macOS editions | Page 14 says other Line 6 devices with a VDI input work as computer interfaces and names those products; it says those hosts also power the guitar. This is a manual claim, not a tested connection. The list does not name PODxt Live or POD HD500X. Their Workbench HD routing remains unknown from this package set. | Not established by this topology statement. | Audio routing is not specified. No host or guitar was connected. |
| Variax 600 through the project's existing PODxt Live path | VDI → PODxt Live → USB host | Existing repository baseline and #26 capability registry | This is the established project target. The five #61 packages add no static evidence that Workbench HD 2.15 supports PODxt Live as its VDI host. Keep the two statements separate. | Not newly established here. | Live control/audio remain locked by project policy; no test was performed. |

Central's six decimal JSON keys convert to the hexadecimal values shown above.
They are sourced from `compatibilityData`; they must not be promoted to editor
support, transport identifiers, USB descriptors, or successful update claims.
The two platform `Variax.json` objects compare equal after JSON parsing. In
addition, 20 common product/update JSON resources and 10 common presentation
JSON resources compare equal across the Windows and macOS packages (30 paired
resource objects; 60 files). macOS packages an additional `SGTDISteps.json`.
The macOS package also places the ten presentation JSON files beside its other
resources, while Windows places them under `PresentationScripts/`.

## Recovered static details

### Standalone bridge versus attached guitar

The Windows driver INF at
`Tools/Driver Archive/Variax USB Interface/L6SM.inf` calls the device “Line 6
Variax USB Interface” and matches `USB\VID_0E41&PID_534D`. It declares the
`L6SeaMonkDev` kernel service, `MEDIA` class, demand start, and separate
`L6SM.sys` / `L6SM64.sys` driver files (the INF has an NTamd64 section).
The Workbench HD Windows installer includes the same INF bytes,
but its bundled `L6SM.sys`, `L6SM64.sys`, and catalog bytes differ from the
standalone driver installer's copies. The INF text alone does not explain those
binary differences.

Static imports and strings in `L6SM.sys` include USB configuration parsing and
configuration-request construction (`USBD_ParseConfigurationDescriptorEx`,
`USBD_CreateConfigurationRequestEx`), device-interface registration, and the
driver name “Variax Workbench USB Driver.” This supports a USB host-driver role.
It does not recover interface numbers, endpoint addresses/types, transfer
framing, IOCTL contract, attached-guitar identity, session lifecycle, or error
cleanup. The PID `0x534D` is the standalone bridge match; it is not a Variax
guitar product ID. No macOS standalone-driver package was present among these
five inputs, so its driver and transport remain unknown here.

### Workbench HD editor and formats

The Windows Workbench HD executable's embedded version strings identify 2.15.
The macOS package identifies bundle `com.line6.workbenchhd`, version 2.15,
minimum macOS 10.8. Its `Info.plist` registers three distinct file types:

| Type | Extension | Four-character type |
| --- | --- | --- |
| Workbench HD preset | `.whd` | `6WHD` |
| Workbench HD bundle | `.bun` | `6WHB` |
| Workbench HD bank | `.bnk` | `6WHK` |

The Pilot's Guide (Workbench HD User Manual, printed pages 2–4 and 12–14)
describes VDI connection/power requirements, offline preset/bank/bundle editing,
60-preset bundles, and explicit upload/download actions. It says a charged
Variax battery and a 1/4-inch cable are needed when using the standalone VDI.
Page 14 names the VDI-input hosts in the matrix above. The Windows and macOS
packages contain byte-identical copies of that manual, `FactoryPresets.bun`,
`WB2Base.ast`, and `wb2.imb`; several UI and language resources differ. These
package facts do not show that their file bytes or device protocol are
cross-compatible with original Workbench formats. No conversion is inferred.

### Central's firmware/update evidence

The Windows app is a .NET assembly with `UpdaterV3EngineDLL.dll`,
`EngineCLIWrapper.dll`, `libusb-1.0.dll`, and `cyusbserial.dll`; the macOS app
contains corresponding USB/serial dylibs and the same updater-step data. The
Windows/macOS Variax resources share the six compatibility rows and update
steps. This identifies maintenance-oriented app components and static UI/data,
not a reusable app API or verified bridge protocol. The cached resources also
contain EULA/update UI content; no remote endpoint was contacted.

## Explicitly unresolved

- The exact instrument identity reported behind bridge PID `0x534D`, supported
  Variax model/revision table for Workbench HD, and its host discovery/handshake
  are not recovered.
- Driver interface/endpoint numbers, transfer types, packet framing, IOCTLs,
  timeouts, disconnect cleanup, and Windows/macOS driver behavior are not
  recovered. The two Windows driver bundles' binaries differ despite sharing an
  INF with the same declared version.
- PODxt Live and POD HD500X are not named in the Workbench HD VDI-host list.
  The presence of generic VDI support does not settle either case.
- Central's Variax table does not establish whether each guitar/bass is
  recognized, editable, backed up, or updateable in a real session. The bass
  rows do not imply Workbench HD editor support.
- No Variax bass editor/backend, verified Central catalog/version payload, firmware
  image, firmware protocol, installed-unit version, or recovery path was
  established. No audio routing/capture support was established.
- No original-library oracle, CPU emulation, synthetic transport check, or
  connected hardware check was run for this milestone. All physical operations
  remain locked, and no product or family capability has been unlocked.

The issue remains a partial research milestone. The full connection/capability
map, protocol and package coverage, verified model-specific editor support,
additional offline compatibility work, and native dependent issues remain open.
