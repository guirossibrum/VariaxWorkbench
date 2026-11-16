# POD HD500X offline identity

Issue [#32](https://github.com/guirossibrum/VariaxWorkbench/issues/32) records
what the local driver packages and the public Line 6 catalog establish for
POD HD500X. This is not a preset editor, a firmware image, a record route, or
permission to connect a device. Workbench HD and JTV-69 stay on their own
profiles. PODxt Live file id `0x0003000A` and USB product `0x4650` stay on the
initial rig.

`Session::pod_hd500x` returns this record only for `ProductProfile::PodHd500x`.
Workbench HD, JTV-69, and `Session::initial_rig` do not inherit it. Shared
open, edit, record-route, and firmware operations stay `Unverified`. Physical
operations stay `Locked`. No conversion or cross-flash function is provided.

## Driver packages

The hashes are the installer archives already in
[artifact provenance](artifact-provenance.md). This pass listed them with 7-Zip
and extracted text only. The Windows installer was not run. No `.sys`, DLL,
kext, or Mach-O was loaded.

| Archive | Artifact id | Bytes | SHA-256 |
| --- | --- | --- | --- |
| Line 6 Drivers Windows 4.2.7.7 | `driver-windows-4.2.7.7` | 29821648 | `dd2f3a787006ecb0341b0ff2f4a8699c00df274b494eb6ed2c6beae4e13c7b92` |
| Line 6 Audio-Midi Driver macOS 7.6.8 | `driver-macos-7.6.8` | 2869592 | `48099370936bea11e09b9d8ee4828e2c9050dd1a1da1586828b7f5a72b559436` |

7-Zip lists `Tools/Driver Archive/POD HD 500X/l6PODHD5SE.inf` inside the Windows
archive. A data extract of that member is 10484 bytes, MD5
`37f0d8c24dc286d2a629b8e6d7a2a261`, SHA-256
`50080c9e9efb7b104e2520ea577db3eb2c13f9c16fb0cf90c1628ef31f5b1790`.
The same directory lists `L6PODHD5SE.sys`, `L6PODHD5SE64.sys`,
`l6podhd5se.dll`, `l6podhd5se_x64.dll`, and `l6PODHD5SE.cat`. Those binaries
were not opened.

The macOS DMG lists `Line 6 Audio-Midi Driver.pkg` at 2786707 bytes, SHA-256
`34e32e3efc3f96330470576673c2252580d726f3d1e5e84c5fe5faaeb6ad0c4d`.
Inside that pkg, `Line_6_Audio_Driver.pkg/Payload` is 768396 bytes of gzip
whose uncompressed size is 2712064. That payload is a cpio archive. Both
`Line6AudioSigned.kext/Contents/Info.plist` and
`Line6AudioUnsigned.kext/Contents/Info.plist` in the cpio are 47343 bytes, MD5
`889903adc3cb5db09c17a9b4be9316b1`, SHA-256
`17f83de2fcfc818e7ea50699406fcbe7ab15ae52dd74db735a5f33e695c14b9a`.
The kext Mach-O files were listed and not loaded. No codesign check was made.
These member hashes are not the archive hashes.

## USB match

`l6PODHD5SE.inf` is dated `$Date: 2019/03/15 14:06:41 $`, `$Revision: 1.30 $`,
with `DriverVer=03/15/2019,4.2.7.7`. Both `Line6Mfg.NTx86` and
`Line6Mfg.NTamd64` match `USB\VID_0E41&PID_4159`. The strings name the device
"POD HD500X" and the ASIO entry "ASIO POD HD500X".

The Mac personalities `PODHD500X`, `PODHD500XBulk`, `PODHD500XDevice`, and
`PODHD500XClassic` use `idVendor` 3649 (`0x0E41`) and `idProduct` 16729
(`0x4159`). `PODHD500X` is interface 0, configuration 1, class
`com_line6_audio`. `PODHD500XBulk` is interface 1, configuration 1, class
`com_line6_driver_bulkpipecomm`. The device and classic personalities do not
state a configuration or interface number. This is a driver match table. It is
not a live enumeration and not a firmware product id.

The same Windows package has other INF hardware ids. They are not this
profile. POD HD500, without the X, is `USB\VID_0E41&PID_414D`. The Mac
`PODHD500` personalities use `idProduct` 16717, the same value. POD HD is
`0x4156`, POD HD Pro is `0x4157`, POD HD Pro X is `0x415A`, POD HD 300 is
`0x5057`, and POD HD 400 is `0x5058`. PODxt Live on the initial rig remains
`0x4650`.

## Audio defaults

`L6PODHD5SE_NT_AddReg` and the amd64 copy set:

| Value | INF number |
| --- | --- |
| DefaultWordSize | 24 |
| DefaultSampleRate | 48000 |
| EngineFormatLock | 0 |
| ASIOPerfBufferSize | 512 |
| ASIOWordSize | 24 |

`EngineFormatLock` 0 means 48000 Hz and 24-bit are defaults. They are not a
negotiated format, not a PODxt Live Record Send route, and not an opened
stream. `SelectRecordRoute` on this profile stays `Unverified`. Capture,
playback, and driver bind stay `Locked`.

## Catalog downloads that were not obtained

A GET of
`https://line6.com/software/index.html?hardware=POD%20HD500X&os=All`
returns the page shell. That response does not contain the release rows. The
page script loads rows only after `submit_form=set` and `submit_query=Show`.
The listing request is a GET of
`https://line6.com/software/index.html?hardware=POD%20HD500X&os=All&submit_form=set&submit_query=Show`.
On 2026-11-16 that request returned HTTP 200 and `text/html`, 315367 bytes,
SHA-256
`048b200988464d1e73942a36afe657e2af7e9aa933236b6000a452de48fe3657`.
The ignored copy is
`analysis/reports/pod-hd500x-public-catalog-2026-11-16.html` in the primary
checkout. No account and no download token were used.

That saved page lists POD HD500X Flash Memory v2.62 as release 6260. The
sidebar prints Version 2.62.00 and Released 4/9/15. The estimated size is
892 KB. The notes say to update USB Firmware (1.04) and Flash Memory (2.62).
Those are separate labels. No USB-firmware release id or image was obtained.
The same page also lists Flash Memory v2.61 as release 6237, estimated
892 KB, and those notes also name USB Firmware (1.04). Release 6237 is not
the archived-container row. On 2026-11-16, without an account,
`https://line6.com/software/readeula.html?rid=6260` returned HTTP 302 to
`/account/login.html?user_attempt=download` and then HTTP 200 login HTML.
`https://line6.com/getrelease?rid=6260` returned HTTP 403. No image was saved.

Neither flash label is a FORM/L6FF product id or an installed-unit read.
`archived_firmware_product` stays empty. Version queries stay
`NotEstablished`.

The same saved page lists POD HD500X Edit release 9781 (macOS, sidebar
Version 2.27, Released 8/4/20, estimated 35 MB) and release 8559 (Windows,
sidebar Version 2.27, Released 9/10/18, estimated 37 MB). Neither file was
downloaded. The macOS notes mention an installer customization screen. That
text is not a driver procedure and was not followed. Older rows on the same
page, including Edit 2.26, are not registered here. One older description
calls Edit an editor/librarian and says one build adds HD Model Pack support.
That text does not include a preset container or a model id.

The v2.62 notes name three optional amp/cab packs and eight setlist display
names, including VARIAX. They also say selecting a preset from that setlist
may recall a guitar model on a connected Variax. Those sentences are release
notes. They are not a file format, not model or effect ids, and not JTV or
Workbench HD compatibility. This crate does not store the names as a bank and
does not recall or convert presets.

No POD HD500X preset container, file product id, model id, effect id, or
parameter range was recovered from these archives. PODxt Live tones and
original Variax files opened through this profile stay `Unverified`. They are
not rewritten into another format.

## What remains open

Preset format, model and effect definitions, a local editor, a local flash
image, a local USB-firmware image, and USB-audio routing beyond the INF
defaults are still unresolved. Issue #32 stays open. Passing these tests does
not authorize enumeration, streaming, flashing, or a driver install.
