# POD HD500X offline identity

Issue [#32](https://github.com/guirossibrum/VariaxWorkbench/issues/32) records
what the local driver packages, the public Line 6 catalog, and two public
manuals establish for POD HD500X. This is not a preset editor, a firmware
image, a record route, or
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
sidebar Version 2.27, Released 9/10/18, estimated 37 MB). Each card's download
anchor is `readeula.html?rid=` plus that release id. The hidden input on each
card is `name="rid"`. Neither card publishes a CDN object name. A neighboring
Line 6 Monkey 1.78 card uses `/getrelease?rid=8688`. That request was not
repeated. The macOS Edit notes mention an installer customization screen.
That text is not a driver procedure and was not followed. Older rows on the
same page, including Edit 2.26, are not registered here. One older description
calls Edit an editor/librarian and says one build adds HD Model Pack support.
That text does not include a preset container or a model id.

## Edit 2.27 installers were not obtained

On 2026-11-16, HEAD requests without an account and without following the
redirect returned:

| Release | URL | Result | Date |
| --- | --- | --- | --- |
| 9781 | `https://line6.com/software/readeula.html?rid=9781` | HTTP 302 to `/account/login.html?user_attempt=download` | Mon, 16 Nov 2026 04:19:45 GMT |
| 8559 | `https://line6.com/software/readeula.html?rid=8559` | HTTP 302 to `/account/login.html?user_attempt=download` | Mon, 16 Nov 2026 04:19:46 GMT |
| 9781 directory | `https://l6c-acdn2.line6.net/media/software/9781/` | HTTP 404, content-length 213 | Mon, 16 Nov 2026 04:19:47 GMT |
| 8559 directory | `https://l6c-acdn2.line6.net/media/software/8559/` | HTTP 404, content-length 213 | Mon, 16 Nov 2026 04:19:48 GMT |

The login page was not requested. No cookie was stored. Flash release 6260
was not requested again. The known public CDN object in this crate is Central
1.00 at `media/software/12401/Line6CentralInstaller1.00.exe`. The Edit cards
do not name a file, so no Edit object name was guessed. A 404 from those
directory HEAD requests can coexist with a public named object.

Wayback CDX returned HTTP 200 and a 3-byte empty JSON array for
`line6.com/software/readeula.html?rid=9781` and for
`line6.com/data/*HD500X*Edit*`. The second pattern does not describe the
manual URLs below, which were fetched directly.

A plaintext search of the local `monkey-windows-1.77`, both archive names of
`monkey-macos-1.78`, and `central-windows-1.00` found none of `HD500X`,
`9781`, `8559`, `POD HD500X Edit`, `l6c-acdn`, `media/software`, `.5xe`, or
`.h5e`. Those installers were not run. Tests do not reopen them.

## Public manuals name files and do not define them

Two PDFs were saved under the ignored primary-checkout directory
`analysis/reports/pod-hd500x-manuals/`. They were not executed.

| Manual | Bytes | PDF | SHA-256 |
| --- | --- | --- | --- |
| Edit Pilot's Guide Rev B | 3017205 | 1.7 | `853bdf6b5bf361b56a17e938e0b7d4f4e6140ecbb093b574977380b0c4e19cac` |
| Edit Installer Guide Rev A | 4294046 | 1.6 | `7eef1aefbe5d16f30f04fabbc3c9cdc149854397b1f3a317bd2ea7ff855e5435` |

Pilot's Guide Rev B:
`https://line6.com/data/6/0a06434cb25651dddc9192349/application/pdf/POD%20HD500X%20Edit%20Pilot%27s%20Guide%20-%20English%20%28%20Rev%20B%20%29.pdf`.
Its coverage line says it covers POD HD500X Edit version 1.0 and Flash Memory
version 1.0 or later. It prints a computer preset as POD HD500X Edit Preset
(`.5xe`), a Bundle of the set lists as (`.5xb`), and a Set List file as
(`.5xs`).

Installer Guide Rev A:
`https://line6.com/data/6/0a06434c122752169db7bc869/application/pdf/POD%20HD500X%20Edit%20Installer%20Guide%20-%20English%20%28%20Rev%20A%20%29.pdf`.
The file is copyright 2013 and does not print an Edit 2.27 label. It says a
download from the software page is a `.dmg` opened to `POD HD500X Edit.mpkg`.
Its Windows association sentence prints `.h5e` Preset, `.h3b` Bundle, and
`.h5s` Set List. Those suffixes are not the Pilot's Guide suffixes. This
record keeps both printed sets and does not choose one. Neither guide gives a
byte layout, a file product id, a model id, or an effect id. The `.dmg` and
`POD HD500X Edit.mpkg` names are not local files.

The device Advanced Guide Rev A at
`https://line6.com/data/6/0a06434c6fb051e03e8ab63dc/application/pdf/POD%20HD500X%20Advanced%20Guide%20-%20English%20(%20Rev%20A%20).pdf`
was not downloaded during the earlier pass. A later bounded acquisition and
page-cited capability/routing study is now recorded in the
[Advanced Guide evidence](pod-hd500x-advanced-guide.md). It adds documented
behavior, not preset/model identities or a codec.

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

## Independent files were measured and do not name the product

No Edit installer was used. A public author page,
`https://www.jzab.de/content/pod-hd-preset-tool/`, returned HTTP 200 on
Mon, 16 Nov 2026 05:07:21 GMT, 23665 bytes, SHA-256
`84b3ba20ae3bbb407ac08b5f0220cd028c4abe290d96eea62cc7496cc8ce82e5`.
That page links `https://github.com/johanneszab/podhd/` and its releases.
The local no-checkout clone is pinned at
`58e95d5b514b44ae667dd5463ec63c8bafa6735a`. Its `LICENSE` file is the GNU GPL
version 2 text. `src/mainwindow.cpp` at that commit is SHA-256
`6656f6a6cfa4304b6a85164f9a3437c6bcf00b631809c82b1b582b77f0dfc597`.
None of that conversion code is copied here. The source was not built or run.
It is not an original POD HD500X Edit oracle.

The same commit's file filters name `.h5e` and `.h5s`. The fetched files use
`.5xe` and `.5xs`. Those suffixes are not a product id.

Four files were fetched from
`https://www.foobazaar.com/podhd/patches/HD500X/` on 2026-11-16. The copies
stay in the ignored `.tools/issue-32-continuation/` directory. Tests do not
reopen them.

| File | Bytes | SHA-256 | Fetched |
| --- | --- | --- | --- |
| `7S_Das_Pod_L.5xe` | 4136 | `4e651fe1830952993ff803720a566e76587a367159a4d774ebfd8843edeee797` | Mon, 16 Nov 2026 04:59:41 GMT |
| `BS_Clean_L.5xe` | 4136 | `d13d20d1fca31bbc699fde60be6c7af3ae6e5ed2a9aa25242c480469adb50714` | Mon, 16 Nov 2026 04:59:42 GMT |
| `CL_ChmpCln_N.5xe` | 4136 | `ca1e4f5dc81c2c78ffbe1fcee1878f102f2cf004877d0733f9748f0043691176` | Mon, 16 Nov 2026 04:59:42 GMT |
| `MAB_Final.5xs` | 264782 | `bcf81c9a495e959129e967c0840fc57ad1714903b20073eb9844b72997e1f400` | Mon, 16 Nov 2026 04:59:42 GMT |

Each `.5xe` file is 4136 bytes and begins with ASCII `H5EP`. The three directory files have byte 0x0b equal to 0x2e. At offsets `0x0b`, `0x0c`,
`0x0d`, `0x13`, `0x14`, and `0x15` they contain `2e 02 26 06 02 62`. The
pinned source writes those six values in the branch named `ProX`. The source comment at offset 0x0b labels `0x2e` as HD ProX and `0x2d` as HD 500X. Neither label is registered. The directory name and the .5xe suffix do not prove a POD HD500X product.

The `.5xs` file is 264782 bytes and begins with ASCII `H5ES`. Its header byte
0x17 is `0x2d`. That offset is not the preset offset 0x0b. The header byte at
0x0b is `0x05`. Sixty-four `H5EP` records start at offset 56 and repeat every
4136 bytes. `56 + 64 * 4136 + 22 = 264782`. Each embedded record has byte 0x0b
equal to `0x2d`. The six offsets above contain `2d 02 26 05 00 00` in every
embedded record. The source branch named `500X` writes `2d 02 26 05 02 62`,
so offsets `0x14` and `0x15` differ. The embedded records are not treated as
that branch's output.

`preset_container_recovered`, `model_ids_recovered`, and `effect_ids_recovered`
stay false. A synthetic buffer with these lengths is rejected by
`Session::open_pod_preset` as `Unverified` before any product parse.

## What remains open

No Edit 2.27 installer is available locally. The unauthenticated readeula HEAD requests redirected to login, and the release-id directory HEAD requests returned 404. A directory 404 can coexist with a public named object. An owner-supplied original installer would unblock this static-extraction approach. Wider availability and other independent sources remain unknown. This crate does not store credentials and does not retry the routes already requested or flash release 6260.

Preset format, model and effect definitions, a local editor, a local flash
image, a local USB-firmware image, and USB-audio routing beyond the INF
defaults are still unresolved. Issue #32 stays open. Passing these tests does
not authorize enumeration, streaming, flashing, or a driver install.
