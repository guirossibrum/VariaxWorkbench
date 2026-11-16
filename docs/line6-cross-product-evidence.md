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

#### Standalone Windows bridge driver: PnP removal edge

Static disassembly of the standalone installer's x86 `L6SM.sys` (SHA-256
`b109c86093a54a33b8eff7f677053f3ca1960c3c304a137ed6ea776f4f17c5d7`, PE
ImageBase `0x00010000`; `objdump` addresses below are image virtual addresses)
traces one removal path. Initialization at `0x18552` registers common callback
`0x105c4` for several major-function slots, registers AddDevice `0x1058a`, and
registers unload `0x17c06`. The common callback first checks global byte
`0x17aa4`: its zero branch takes a device-object vtable `+4` path; its other
branch indexes a major-function table. In that branch, PnP major-function
slot `0x1b` selects wrapper `0x12cd9`, which uses object vtable slot `+0x7c`.
The recovered vtable at `0x17400` has handler `0x14b1f` at that slot, and two
vtable-initialization sites (`0x109cd`, `0x11b41`) install that pointer. Handler
`0x14b1f` dispatches PnP minor values `0x00..0x17` through a table at
`0x153e9`; it has a separate `0x87` branch and sends other values above
`0x17` to its unsupported path. Minor `0x02` routes to `0x14e76`
(`IRP_MN_REMOVE_DEVICE`).

In that remove case, when context flag bit `0x10` is set and a saved pointer at
context offset `+0x30` is non-null, code calls helper `0x13030`, which invokes
the imported `IoDetachDevice` on the saved lower-device pointer, then clears
`+0x30`. Later conditional branches call `IoDeleteSymbolicLink` and
`IoDeleteDevice` based on context state. These are static paths, not an observed
disconnect or proof that every remove reaches all cleanup steps. The traced
unload routine also calls two indirect global-object methods and a cleanup
worker whose full session/refcount semantics are unresolved. The standalone
x64 driver and separate Workbench HD driver binaries are not behaviorally
covered by this x86 trace. Supporting private disassembly/imports and a compact
trace report are under `.tools/issue-61-analysis/`, including
`l6sm-driver-disassembly-standalone-x86-b109c860.txt`,
`l6sm-driver-imports-standalone-x86-b109c860.txt`, and
`l6sm-pnp-remove-trace-b109c860.txt`. Static analysis read extracted driver
bytes; it did not execute/load the driver. No matching standalone macOS driver
was in the supplied packages, so a platform-paired path cannot be established.

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

### Workbench HD 2.15 preset record structure (static, partial)

The Windows and macOS package copies of `FactoryPresets.bun` are byte-identical
(74,492 bytes, SHA-256
`aa39230491ceee7f49db8b1b7aca0d721ab6654d2b433b4e512b81d76fa48d1f`). The
macOS copy is at
`.tools/issue-61-payloads/00/Workbench HD.app/Contents/Resources/FactoryPresets.bun`;
the Windows extraction is at
`.tools/issue-61-analysis/01/Workbench HD/res/FactoryPresets.bun`. A bounded
IFF walker checked container boundaries and record counts without retaining
preset names or parameter values. It observed a `FORM/WB2B` bundle containing
12 `FORM/WB2K` banks, each with five `FORM/WB2P` presets (60 total). Every
preset starts with a 12-byte `PINF` and has a `LIST/PRST` with eight parameter
groups. Across all 60 records the group names and `PARM` counts are: `PUP1`
(5), `PUP2` (5), `PUP3` (5), `BODY` (11), `STRG` (6), `ALTT` (7), `PIFX`
(13), and `SYST` (1); every observed `PARM` chunk has a 12-byte payload. Each
factory record also has `LIST/META` with one `INAM` chunk; its payload is 8 or
12 bytes. These are measurements of this one factory bundle, not universal
limits for other versions or files. Recheck script and private output are
`.tools/issue-61-analysis/check_wb2_factory_platform_pair.py` and
`.tools/issue-61-analysis/wb2-factory-structure-platform-pair-aa39230491ceee7f49db8b1b7aca0d721ab6654d2b433b4e512b81d76fa48d1f.txt`.

Static analysis of the macOS Workbench HD executable (SHA-256
`531bbcf1a187afe4243c9e48af491acb64841f3986383bb099c69fd9a0be1fe8`, Mach-O
image base `0x100000000`) located `WB2Preset::FromStream` at `0x100020d24`,
`ToStream` at `0x100020994`, `VerifyFile` at `0x1000214a8`, `ReadParamGroup` at
`0x10002125c`, and `WriteParamGroup` at `0x100020c54`. The reader requires a
`FORM/WB2P` and first reads a 12-byte `PINF`; it proceeds only when its first
big-endian word is greater than 2. The writer emits version word 3 and
preserves two other 32-bit words. In the factory set those words are
`0x00070005` and `0x0200006A`. `0x00070005` also appears as the Tyler product
identifier in the Central compatibility data above, but the preset field's
meaning is not independently named by this code; the third word remains
opaque. Do not treat either value as a complete instrument identity or
Workbench HD support table.

The reader dispatches only the eight group IDs observed above, then reads
12-byte `PARM` payloads as an index, a type tag, and a 32-bit value field.
`WB2ParamGroup::SetParam` at `0x10000e6d8` rejects an index outside its
group's parameter count and rejects a type tag that differs from that
parameter's configured type. The writer walks configured parameter indices
and emits the same 12-byte shape. This establishes a bounded typed-record
contract, not names, ranges, units, or model/tuning meanings for the parameter
IDs or values. The reader skips unrecognized chunks/groups; `VerifyFile` is
only a shallow `FORM/WB2P` plus first-`PINF` check, not full validation. Static
decompilation report:
`.tools/issue-61-analysis/wb2-param-contract-531bbcf1a187afe4243c9e48af491acb64841f3986383bb099c69fd9a0be1fe8.txt`.

The preceding `WB2P` preset-handler evidence is from the macOS executable only;
resource equality does not prove Windows `WB2P` behavior. A separate,
narrowly matched tuning serializer is described next. Ghidra analysis was
offline and static. No vendor application/library code was executed and no
device, USB/MIDI/audio, service, or network was contacted. No Rust preset
backend, editor compatibility, transfer behavior, validation oracle, or
physical recovery claim follows from this finding. Issue #61 remains partial.

### Workbench HD tuning record (static Windows/macOS match)

Static analysis of the same macOS executable found named `WB2AltTuning`
handlers: `FromStream` (`0x100007ece`), `ToStream` (`0x100007d4e`),
`SetTune` (`0x100007c5a`), `GetTune` (`0x100007c92`), and
`WriteStringParams` (`0x100007e6e`). `ToStream` writes `LIST/TUNE` with a
`THDR` name chunk followed by six `STRG` chunks. Each `STRG` payload is exactly
8 bytes: a slot index byte, three zeroed padding bytes, and a big-endian
IEEE-754 float. `SetTune` accepts slot indices 0 through 5 and maps slot `i`
to internal group parameter index `i + 1` with type tag 1. `SetParam` rejects
indices outside the configured count (`0x80000008`) and a type mismatch
(`0x80000009`); `SetTune` rejects index 6 or higher (`0x80000002`). The stream
does not carry a type tag: the reader assigns type 1 to the float. The
constructor resolves parameter definitions for group ID `ALTT` and creates its
internal parameter group from that definition. The definition has seven
entries: index 0 is type 0 with default zero; indices 1–6 are type 1 with
default zero. Thus TUNE slot `i` maps to preset parameter `ALTT[i + 1]` in this
macOS object implementation. This is a static object/definition link, not a
claim that every preset or platform uses the same mapping.

The macOS reader skips out-of-range slot bytes; a wrong-sized `STRG` fails
with `0x80000007`. An unexpected child tag fails with `0x8000101b` rather than
being skipped. Duplicate in-range slots overwrite earlier values, so the last
record wins. The Windows reader uses the same wrong-tag and wrong-size errors,
but its slot setter rejects an out-of-range slot with `0x80000002` instead of
skipping it. These are source-code paths, not observed results from running the
vendor binaries.

The bundled `wb2.xml` lays out String 6 (Low E) through String 1 in descending
display order. Its panel IDs have low 16-bit indices 6 through 1, and the
strings-panel controller routes widget actions through `SetParamFromWidget`,
which uses that low 16-bit value as the parameter index. Combined with the
TUNE `slot i → ALTT index i + 1` mapping, the static UI association is slot 0
to String 1 (high E) and slot 5 to String 6 (Low E). This describes the
Workbench HD UI labels and static parameter wiring; no physical instrument was
observed. The note controls declare a display range of −12..+12, but their
recovered parameter curve only converts integer and float representations.
Neither the stream nor this curve establishes the float's musical unit or
permitted stored range. Treat semitone interpretation as unverified.

The Windows PE has no PDB or symbol table, so these function labels are
inferred from static instruction flow rather than trusted debug names. In the
Windows executable (SHA-256
`be268d1a7e22ce792e9055152a81bb013edd8047fb87c2236dd4bce057f8a3cd`, image
base `0x00400000`), `0x00405390` writes `LIST/TUNE` and six records via
`0x00405b60`; `0x004055a0` reads the same container and calls `0x00415540` for
each slot. The writer helper emits 8-byte `STRG` payloads; the reader helper
maps to internal index `i + 1`, enforces type 1, and stores the 32-bit float
bits. For index 6 or higher, Windows helper `0x00415540` returns
`0x80000002`, which the reader propagates as a failure; this differs from the
macOS reader's skip behavior. This is a bounded static comparison for the
tuning path only; it does not establish equivalence of other Windows and
macOS code or any device transfer behavior.

The Windows executable hash is for
`.tools/issue-61-analysis/01/Workbench HD/Workbench HD.exe`; the macOS
executable and hash are listed in the preceding section. Ghidra reports are
preserved privately under `.tools/issue-61-analysis/`. The reproducible,
wholly synthetic CPU reference and tests are
`scripts/wb2_tune_reference.py` and `scripts/test_wb2_tune_reference.py`; run
them with `python3 scripts/test_wb2_tune_reference.py`. Six checks cover writer
round trip, truncation/chunk lengths, missing/short header, record-size
mismatch, unknown child rejection, duplicate-last-wins and macOS
out-of-range-slot skip, plus setter index/count/type errors. These checks do
not execute or copy vendor code and are not an original-library oracle or
product parser tests. No vendor binary was executed. Issue #61 remains partial.

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
- Tuning analysis remains limited to the static Workbench HD object and UI:
  stored float units and accepted stored-value range are unknown, and the
  String 1/high E through String 6/Low E association has not been checked on a
  physical instrument. The XML's −12..+12 control range is a UI declaration,
  not a validated storage or instrument limit.
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
