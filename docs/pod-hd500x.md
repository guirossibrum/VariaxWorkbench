# POD HD500X offline identity

Issue [#32](https://github.com/guirossibrum/VariaxWorkbench/issues/32) records
what the local driver packages, public Line 6 catalog, public manuals, and
static Editor research establish for POD HD500X. The offline HD Edit file
library described below is not proof of device compatibility, a firmware image,
a record route, or permission to connect a device. Workbench HD and JTV-69 stay
on their own profiles. PODxt Live file id `0x0003000A` and USB product `0x4650`
stay on the initial rig.

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

## Public catalog routes and private static materials

A GET of
`https://line6.com/software/index.html?hardware=POD%20HD500X&os=All`
returns the page shell. That response does not contain the release rows. The
page script loads rows only after `submit_form=set` and `submit_query=Show`.
The listing request is a GET of
`https://line6.com/software/index.html?hardware=POD%20HD500X&os=All&submit_form=set&submit_query=Show`.
On 2026-11-16 that request returned HTTP 200 and `text/html`, 315367 bytes,
SHA-256
`048b200988464d1e73942a36afe657e2af7e9aa933236b6000a452de48fe3657`.
An ignored local capture was used for the offline inspection. Its private
storage location and contents are not published. No account and no download
token were used.

That saved page lists POD HD500X Flash Memory v2.62 as release 6260. The
sidebar prints Version 2.62.00 and Released 4/9/15. The estimated size is
892 KB. The notes say to update USB Firmware (1.04) and Flash Memory (2.62).
Those are separate labels. No USB-firmware release id or image was obtained
through this public catalog.
The same page also lists Flash Memory v2.61 as release 6237, estimated
892 KB, and those notes also name USB Firmware (1.04). Release 6237 is not
the archived-container row. On 2026-11-16, without an account,
`https://line6.com/software/readeula.html?rid=6260` returned HTTP 302 to
`/account/login.html?user_attempt=download` and then HTTP 200 login HTML.
`https://line6.com/getrelease?rid=6260` returned HTTP 403. No image was saved
through that public route.

These results describe only the unauthenticated public routes checked on that
date. Owner-supplied private Editor, PHF, model/default, and firmware-image
materials have since been examined statically. Their files, extracted
contents, hashes, and private locations are intentionally not published here.
They supersede the earlier inference that no such artifacts were available;
they do not establish linkage to the public release ids, product applicability,
or an admitted application schema. No vendor process was executed.

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

## Public Edit 2.27 routes and private static materials

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

A plaintext search of the locally recorded `monkey-windows-1.77`, both archive names of
`monkey-macos-1.78`, and `central-windows-1.00` found none of `HD500X`,
`9781`, `8559`, `POD HD500X Edit`, `l6c-acdn`, `media/software`, `.5xe`, or
`.h5e`. Those scanned archives were not run. Separate owner-supplied Editor
materials have since been examined statically; no vendor process was run, and
tests do not reopen either private or scanned source artifacts.

## Public manuals name files and do not define them

Two PDFs were captured locally for offline inspection and were not executed.
Their private storage location is not published.

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
`POD HD500X Edit.mpkg` names describe the guide only; the public catalog route
did not supply those named payloads. Private Editor materials are separate,
owner-supplied evidence.

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

Static Editor 2.27 resource metadata associates H5EP presets, H5ES setlists and
H5EB bundles with the HD500X Edit file group. The offline library parses their
fixed nested framing, preserves unknown header/context and trailing bytes,
extracts child records as separate items, and saves new copies without
overwriting. H5EP parsing also validates a bounded subset of model classes and
tail flags. Its reversible word conversion is limited to statically traced
fields. The library edits source-identified primary type-1 and selected numeric
rows using recovered model/key mappings. The corrected full descriptor matrix
has 364 populated primary type-1 model/key rows across 108 models; all are
exposed through the common type-1 path. The earlier 184-row mapping covered a
subset and is superseded. The library also exposes 79 non-type-1 numeric rows.
Type-1 values use the editor's shared integer 0..100, step-1 setup and
normalized float path. These are editor values, not physical units or device
defaults. Auxiliary widgets and the separate EDI widget path remain opaque;
non-value bytes are preserved. General model defaults and replacement
semantics remain unavailable except for the three earlier typed Octave Fuzz
replacements and the newly admitted finite twelve-target batch documented
below. None of these editor values establish physical device availability.

### Bounded Octave Fuzz to Vintage Pre defaults

The standalone file library and GUI expose one explicit model replacement:
an existing valid Octave Fuzz model (`0x0205000C`) in any of the eight FX slots
can be replaced with Vintage Pre (`0x020C000E`) and its source-derived defaults.
The API accepts a typed FX slot, not an arbitrary model id or a vendor session
context. H5ES/H5EB children are extracted as independent H5EP library items;
the parent container remains unchanged.

Before mutation, the operation requires validity byte 1, enable byte 0 or 1,
row count 5 through 12, the five known default keys exactly once in the first
five rows (any order), and all twelve assignment-marker bytes in 0/1/2. It
rejects malformed input without changing file bytes or library dirty state.
On success it writes the model DWORD, five ordered key/value pairs
(`3F100002`, `3F100003`, `3F100004`, `3F100005`, `3F100001`, each with zero
value), count 5, and clears only the first byte of each of the twelve marker
DWORDs. Enable, reserved fields, the remaining bytes of marker DWORDs, rows
5–11 (including later duplicates), H5EP framing, trailing bytes and all other
slots remain unchanged. Changed DWORDs use the existing H5EP big-endian file
mapping. This is a conservative supported-input policy derived from the
source-backed standalone operation; it does not claim universal vendor
acceptance or physical applicability. The operation is an explicit user
action and does not run on selection or rendering. Save-copy remains
non-overwriting; library revert and the GUI quit guard retain the existing
dirty-state flow. Focused checks passed on implementation head
`9bb4d617dcac0f4c50209ae06b53913a302b4929`.

### Bounded Octave Fuzz to Screamer defaults

The same shared file library/session and explicit GUI expose a second,
independently typed operation: an existing valid Octave Fuzz (`0x0205000C`)
in any FX slot can be replaced with Screamer (`0x0205000E`) and its source
defaults. The retained Editor 2.27 static initializer confirms descriptor row
1 at `0x00520124`; its first five defaults are keys `0x3F100001` through
`0x3F100005` in ascending order, each selector 1 with zero value bits. Rows
5–10 have zero keys and are skipped by the builder. This is descriptor and
initializer evidence, not vendor execution or callback parity.

Admission is limited to a standalone H5EP or an independently extracted H5ES/
H5EB child and a typed slot 0–7. The conservative checks require the exact
old model, validity 1, enable 0/1, count 5–12, each of the five keys once in
the first five rows in any order, and all twelve marker bytes in 0/1/2.
Rejection is atomic. Success writes model `0x0205000E`, the ordered five
key/zero-value rows and count 5; it clears only the first marker byte in each
of twelve rows. Validity and enable/reserved bytes remain unchanged, as do
rows 5–11 (including later duplicates), all unrelated slots, file framing,
trailing bytes and an H5ES/H5EB parent. The candidate is reparsed before
commit. This is not a generic model/default setter and does not run on
selection or rendering. Both explicit replacements share rejection coverage
for invalid validity, enable, count, key prefix, duplicate prefix, old model,
and marker inputs, checking bytes plus library baseline/dirty/revert state.
The GUI lifecycle exercises each replacement through cancel, failing and
colliding save destinations, successful save/reopen, and quit protection.
Extracted H5ES and H5EB child tests check parent preservation; the H5EB case
also checks dirty/revert state.

### Bounded Octave Fuzz to Sub Octave Fuzz defaults

The third separately typed operation replaces an existing Octave Fuzz
(`0x0205000C`) in public FX slot 0–7 with Sub Octave Fuzz
(`0x02050014`, SFUZ). The retained POD HD500X Edit 2.27 PE32 i386 image
(SHA-256 `871c86da6b6810307dc201297d8411f2b2f7eec7e4c3e150f0aa881f8c89afe2`)
identifies SFUZ as descriptor row 13 at `0x00522674`. Its initialized prefix
is keys `0x3F100001` through `0x3F100005` in ascending order, each selector 1
with selected zero DWORD bits. Six remaining zero-key candidates are skipped
by the admitted standalone builder. This is static initializer evidence, not
vendor execution or callback parity.

The operation uses the same H5EP-only guards and atomic candidate/reparse
policy as the two prior transitions. Source runtime slots 4–11 correspond to
public/file slots 0–7; the body block is `0x428 + 0x100 * slot`, excluding the
0x28-byte H5EP header. It writes model `0x02050014`, the ordered five
key/zero-value rows, count 5, and clears only the first byte of each of twelve
marker DWORDs. It preserves enable, validity, reserved bytes, marker bytes
1–3, rows 5–11 (including later duplicates), unrelated slots, framing,
trailing bytes and any H5ES/H5EB parent. Malformed inputs and direct container
edits are rejected without mutation; extract a child before editing. The GUI
exposes a separate explicit action and does not mutate on selection or
rendering. Physical compatibility remains unknown.

### Admitted batch of twelve Octave Fuzz defaults

The closed `HdDistortionDefault` choice and one explicit batch Apply action
cover exactly these targets. The retained POD HD500X Edit 2.27 PE32 image has
SHA-256
`871c86da6b6810307dc201297d8411f2b2f7eec7e4c3e150f0aa881f8c89afe2`; the
descriptor rows below were joined to their retained eleven-candidate default
records and five key/value initializer stores. The joined default matrix hash
is `11730db4a0e04fa176422a1e6518d5afcc49735dbef510918b2612b330455877`.
Those hashes identify private evidence and do not include vendor material in
this repository.

| Target | Model ID | Descriptor row / address |
| --- | --- | --- |
| Jet Fuzz | `0x0205000A` | 2 / `0x00520440` |
| Buzz Saw | `0x02050015` | 3 / `0x0052075C` |
| Classic Distortion | `0x0205000B` | 4 / `0x00520A78` |
| Color Drive | `0x02050018` | 5 / `0x00520D94` |
| Facial Fuzz | `0x02050011` | 6 / `0x005210B0` |
| Fuzz Pi | `0x0205000F` | 7 / `0x005213CC` |
| Heavy Distortion | `0x02050016` | 8 / `0x005216E8` |
| Jumbo Fuzz | `0x02050017` | 9 / `0x00521A04` |
| Line 6 Distortion | `0x02050012` | 10 / `0x00521D20` |
| Line 6 Drive | `0x02050013` | 11 / `0x0052203C` |
| Overdrive | `0x02050010` | 12 / `0x00522358` |
| Tube Drive | `0x0205000D` | 14 / `0x00522990` |

Each retained initializer record has eleven candidates. The first five active
rows use keys `0x3F100001` through `0x3F100005` in ascending order, selector 1,
and selected zero DWORD values; six remaining candidates have zero keys and
are skipped. This is a finite source-backed editor-default operation, not a
general model catalog or physical capability claim. It replaces values with
the target defaults; it does not migrate the Octave Fuzz's values or labels.

The operation reuses the standalone H5EP contract above: slot 0–7, exact old
model `0x0205000C`, validity 1, enable 0/1, count 5–12, the five known source
keys exactly once in the first five rows in any order, and twelve first-byte
assignment markers in 0/1/2. It rejects out-of-set targets by construction and
rejects failed guards before mutation. The candidate write changes only the
target model DWORD, five ascending key/zero-value rows, count 5, and the first
byte of each of twelve markers. It preserves all other bytes, later duplicate
rows, marker tails, the remaining rows, enable/reserved fields, other FX slots,
H5EP framing/context/trailer and extracted H5ES/H5EB parents. Candidate bytes
are reparsed before commit. The batch has been implemented on the current PR
branch; exact-head focused validation is pending. GUI target selection and
rendering do not apply it. No vendor execution, product applicability or
callback parity is established.

### Bounded FX enable/disable flag

The shared library admits enable/disable only for an existing valid H5EP FX
block whose model id appears in the current source-joined typed control tables.
That limited set is not a complete model catalog and does not add model
selection. The POD HD500X session is required for this write; H5EP parsing
requires file version 1, while unknown internal header words remain opaque and
physical target applicability remains unknown.

The source-traced byte is `B+8`, where public FX slot 0–7 maps to body block
`B = 0x428 + 0x100 * slot`. A nonzero byte reads as enabled, but a write
requires the existing byte to be canonical `0` or `1`; both requested values
reject a noncanonical byte without changing it. A canonical no-op preserves
the whole file, while a real state change writes only the requested `0` or `1`
at `H5EP_HEADER_BYTES + B + 8`. The operation rejects the empty
`0x02000000` block, unknown models, invalid slot validity and unsupported
H5EP versions before mutation. Candidate bytes are reparsed before publishing.
Extracted H5ES/H5EB children are independent library entries, so their parents
stay byte-identical. The GUI exposes a single offline action only for an
admitted current selection; save remains a non-overwriting copy operation and
unsaved changes stay protected by the existing quit prompt.

Seven new synthetic tests cover both requested states across all eight slots,
canonical no-ops, both requests against noncanonical bytes, unknown/empty/invalid
blocks, version rejection, dirty/revert state, H5ES/H5EB parent preservation,
target profile checks, and GUI save collision/failure/reopen/quit behavior.
These tests do not validate a physical product or original-editor event parity.

### Bounded type-9 three-choice controls

The offline H5EP library exposes four exact revision-3 Editor 2.27 type-9
model/key associations: `0x020A0014/0x3F100004`,
`0x020A0010/0x3F100004`, `0x020A000F/0x3F100004`, and
`0x020A0012/0x3F100003`. Three additional tuples were admitted from the
initialized descriptor stores in the private revision-3 static analysis; this
bounded list is not an exhaustive descriptor catalog. All four use neutral UI
labels `Choice 0`, `Choice 1`, and `Choice 2`, with exact serialized big-endian
f32 words `00000000`, `3F000000`, and `3F800000`. The typed file slot is
`HdFxSlot(0..=7)` and maps to body block `0x428 + slot*0x100`; descriptor
ordinals and vendor runtime selectors are not converted into file slots. These
source/applicability observations do not prove physical HD500X compatibility.

The setter first relies on the full H5EP v1 parser, then requires the exact
exact model/key association, validity byte `B+0x0b == 1`, canonical enable byte `B+8` in 0/1, row
count `B+0x0f` in 1..=12, exactly one matching key among all twelve rows (even
outside the count), target row below the declared count, and zero assignment
marker at row `+0x10`. The count rule is a conservative application guard;
the legacy parser does not itself validate that byte. Disabled-but-valid
effects remain editable and the enable byte is preserved. The operation rejects
non-finite current values, while getters retain their raw bits as a distinct
non-finite state; unknown finite values remain lossless on load and can be
replaced only by an explicit choice. Same-word no-ops occur only after every
guard succeeds. A candidate changes only the unique value DWORD, is reparsed as
a complete H5EP record, then becomes the in-memory edit. H5ES/H5EB children
must be independently extracted; the enclosing parents are never mutated.

The GUI stages a choice per selected library entry and file FX slot across
frames. Apply and Discard are explicit; changing file/slot/model, loading or
reverting clears stale staged state. Staged choices participate in the quit
guard and are never applied implicitly. The shared Session write requires the
POD HD500X extension profile; other profiles and non-H5EP entries reject.
Synthetic tests exercise all eight slots, rows 0/5/11, all choices, both enable
states, unknown finite/no-op/non-finite values, malformed/count/model/validity/
marker/missing/duplicate rejection, full-file preservation, session/library
gates, GUI callback staging across frames, save/reopen, and staged/ordinary
quit decisions. The exact-head configured check result is recorded on PR #109;
older check totals do not cover this operation. None of these tests establish
physical compatibility or audio behavior.

### Additional selected numeric control contracts

The retained Editor 2.27 callback matrix has 80 active non-type-1 numeric rows
across 17 widget tags. The offline library now admits 79 source-joined rows
across 16 tags and 65 descriptor model IDs. It exposes a known control label
where an earlier static join established one; otherwise the UI uses the raw
parameter key so it does not invent a name. Model IDs and labels are editor
internal descriptor values, not physical-product capability claims.

The exact integer arguments passed to the source widget setup are listed by
raw tag below. Each admitted row also maps through configured float endpoints
0.0 and 1.0. The Rust editor requires exact round-tripping through that integer
grid, a matching model and key, and a zero row flag; edits change only the first
matching four-byte float value. These bounds and labels do not establish
physical units, audible ranges, device defaults, or availability on a connected
POD. Auxiliary widget data remains opaque.

The retained Edit 2.27 disassembly provides the conversion order for the
admitted rows: integer `value - min` and `max - min` are formed in 32-bit
registers, the numerator is loaded to x87, divided by the integer width, mapped
through the configured 0.0..1.0 endpoints, then stored as a DWORD float. Tag
`0x10` follows a separate converter with the same observed endpoints. The
independent fixed-word regression fixtures record interior inputs and edited
outputs for all 16 admitted tags, including negative and nonzero minima; these
test raw-word decoding, direct/shared edits, unrelated-byte preservation and
saved-file reparse. This is a source-derived offline contract, not a claim of
audio equivalence, physical behavior or full editor parity.

| Raw widget tag | Active rows | Editor integer bounds |
| --- | ---: | --- |
| `0x05` | 9 | 0..240 |
| `0x06` | 2 | 0..220 |
| `0x07` | 1 | 0..360 |
| `0x08` | 2 | -96..0 |
| `0x09` | 1 | -80..0 |
| `0x0A` | 1 | 0..24 |
| `0x0D` | 25 | 0..1400 |
| `0x0E` | 1 | 0..1400 |
| `0x0F` | 1 | 0..3520 |
| `0x10` | 20 | 20..2000 |
| `0x11` | 11 | 0..200 |
| `0x12` | 1 | 0..800 |
| `0x13` | 1 | 0..4000 |
| `0x14` | 1 | -240..240 |
| `0x16` | 1 | 0..200 |
| `0x1A` | 1 | 20..500 |

### Selected primary type-1 effect controls

The corrected source matrix contains 364 active type-1 primary rows across 108
model IDs. The GUI/session exposes each source-joined model/key/resource-label
row. This reconciles the previous 184-row mapped subset with the 180 additional
rows in the corrected full matrix. The primary knob setup uses source integer
arguments 0..100 with step 1 and a normalized float mapping; these are editor
values, not physical percentages or device defaults. The separate auxiliary
EDI widget path is not included. Each edit requires a matching model/key, zero
assignment flag and an exact percent-grid value, and changes only the first
matching float value word.

The active tag `0x1B` row on Vintage Pre is admitted only for its primary
widget: model `0x020C000E`, key `0x3F100005`, integer editor range 0..15,000,
normalized float 0..1. The constructor, getter/converter and virtual setter
trace show that this primary knob clamps to its configured range and stores
the converted normalized float in the first matching existing keyed row. Its
separate EDI edit-widget setup uses 0..150,000 and is not used for this row;
the row's auxiliary coordinates are sentinels. The post-write root `+0x40`
receiver and virtual `+0x28` callback remain unidentified, so this operation
does not claim full event aftermath or original-editor equivalence. Its UI
label remains the key; no physical unit, domain, or model-specific default is
inferred.

Together with the 80 selected non-type-1 numeric controls, the library exposes
444 source-identified controls across 143 descriptor model IDs (364 primary
type-1 rows across 108 IDs and 80 numeric rows across 65 IDs, covering 17
widget tags). No active primary type-1 model/key row in the corrected matrix
remains unexposed. This remains a bounded subset: auxiliary widgets, option
controls and other type families remain unimplemented. General model defaults
and replacements remain unavailable beyond the three earlier bounded Octave
Fuzz operations and the exact twelve-target batch above. The batch is awaiting
exact-head focused validation. Original-editor parity is not established.
Unknown, assigned, or
off-grid values stay unchanged.

### Source erratum: Screamer and Tube Drive

An earlier private Editor 2.27 join misattributed model ID `0x0205000D` to
Screamer. The corrected Windows Edit 2.27 static descriptor/registry join is:
descriptor row 1 at `0x00520124`, model `0x0205000E`, Screamer with TONE; and
descriptor row 14 at `0x00522990`, model `0x0205000D`, Tube Drive with MID.
Octave Fuzz is descriptor row 0 at `0x0051FE08`, model `0x0205000C`, with MID.
The controls share the five primary type-1 key positions, but labels and model
gates are model-specific. This erratum supersedes the earlier model name
assignment; it does not change the common type-1 primary numeric callback,
first keyed-row lookup, float32 value write, dirty-state behavior, normalized
grid restriction adopted by this Rust subset, or four-byte-only edit. It does
not admit auxiliary widgets, general effect defaults, a complete effect
catalog, or physical product applicability. The three separately typed bounded
Octave Fuzz replacements use source-confirmed ordered-prefix defaults and
conservative file guards; they are not a general extension of this value edit.
Earlier private reports
remain preserved as historical evidence; the private correction note records
which model-name conclusions they got wrong.

This Edit file association does not prove device target compatibility. In
particular, H5EP byte `0x0b` belongs to a context dword supplied by the writer;
the H5EP reader checks the magic/version but does not compare that byte as a
product id. Third-party source comments assigning `0x2d` or `0x2e` to device
families therefore do not establish target identity. Product attribution,
physical compatibility, broad model/effect semantics, other control ranges,
and original-editor parity remain unresolved. The GUI exposes this path as a
separate offline HD Edit library, while the shared session's product capability
remains `Unverified`. PODxt Live tones and original Variax files are not
rewritten into another format. Synthetic fixtures validate only the
implemented subset and do not reopen independent captures or private source
materials.

### Writer context and the limits of byte `0x0b`

The Editor 2.27 static source trace follows `FUN_0040dd80` through
`FUN_00410730` to `FUN_004108f0`. `FUN_0040dd80` bounds the setlist and preset
indices, selects the patch pointer from the table at state offset `0x44`, and
passes that patch into the copy/write path. The raw writer reads header word 2
from the nested context object's `+0x2c` field and word 3 from `+0x1950`,
placing them at file offsets `0x08` and `0x0c`. It emits ten big-endian 32-bit
words: words 0–1 are magic/version, words 2–5 (`0x08`–`0x17`) come from
context, and words 6–9 (`0x18`–`0x27`) are zeroed. Thus byte `0x0b` is the
fourth byte of the copied `+0x2c` field. The upstream store/constructor that
assigns this field, and any relationship to a product/profile selector, remain
untraced. Reader `FUN_00410a50` converts the body back and checks only
magic/version; it does not compare or retain header words 2–9 as identity.

The field read is established; its semantic producer is not. A separate
Central static product key (`0x00140005`) has no
proven join to this H5EP field or reader. Consequently, neither the writer
trace nor the file-group association establishes a POD HD500X target ID. The
often repeated third-party `0x2d`/`0x2e` labels remain unregistered claims.
These narrow edit-domain conclusions are based on private static reports
identified by SHA-256 `04be402d5444356e0d31ab0bbb26e09b4bb75fcb0723d285f67b3be783d828c1`,
`0087b55afb53b293f4c79136423ff607cab21f2325bbfc78b204947b4001c2a6`, and
`9a0c2576398272b486fb3c1a4c523157369da1d00349cadebdcf0c316d0824df`; the
Editor image identity is `871c86da6b6810307dc201297d8411f2b2f7eec7e4c3e150f0aa881f8c89afe2`.
The reports and binaries remain local; hashes are provenance pins, not a
public reproduction package.

The independent Mac Edit 2.27 trace resolves a different source for the same
H5EP offsets. Its writer serializes `Memory+0x18` and `Memory+0x1c` at file
offsets `0x10` and `0x14`. The `Memory` constructor initializes `+0x18` to
`0x00140005`. `Memory::Init` skips the mapping when its communication argument
is null. Otherwise `L6GBCIComm::GetDeviceID` at `0x10001afc8` returns zero
for a null provider pointer, or calls `0x10001a620`, which reads the 16-bit
field at provider `+0x10a`. A zero result
keeps the default, recognized nonzero results map to editor-internal 32-bit
values, and an unrecognized nonzero result stores `0xffffffff`. The mapping
switch and tables are at `0x10006ea0c–0x10006ed3d`; for example, provider value
`0x4241` maps to `0x00210001` and `0x5741` maps to `0x00190014`. This chain
joins only the comm provider's stored field to the editor's internal switch; it
does not assign that value a USB, firmware, or physical product namespace.
AppShell's static provider-selection path calls the same getter and reads the
same provider field (`0x1000024a5–0x1000024ba`), but this source was not executed
and does not establish a connected unit. The paired `+0x1c` value is filled
from a firmware-version call's successful result, or remains zero on error.
This trace used the x86_64 Mac Editor 2.27 executable (SHA-256
`34148643512ad6fc7e8415cfa10eb767d93d0aa457fd3c517b908677129d1ee0`), extracted
statically from the retained DMG (SHA-256
`7988e77f8ae615b5bd081893c64f582fa0f38a84ad5f6be88a8643b820f74e8e`). The
coordinates above are original executable virtual addresses; no executable
was launched.

Mac `Patch::ReadFromStream` delegates to its H5EP reader, which reads 40 header
bytes, byte-swaps all ten words, checks only H5EP magic and version 1, then
reads and validates the 4096-byte body. It ignores words at `0x08` through
`0x24`, including the two `Memory` context words. The shared Rust offline
session interface now imports the H5EP/H5ES/H5EB file group, extracts child
presets, supports the already traced controls, preserves the raw context,
tracks dirty state through the library, and writes non-overwriting copies. Its
summary labels the file family separately and reports physical target
applicability as unknown. A PODxt FORM tone is rejected by the HD Edit library;
no device-profile capability is promoted. Synthetic coverage checks
default-like, mapped-like, and arbitrary unknown context values as opaque file
data; it does not emulate or verify the vendor's mapping. The existing HD Edit
window now routes import, extraction, supported control edits, and copy-save
through the shared Session interface; this closes the concrete gap between the
standalone window and common library boundary. This header trace establishes
no additional model/control or audio contract; broader admitted Editor-source
investigation remains open under criterion 1.

The five control edits are intentionally narrow. In the pinned Editor image,
`FUN_00411130`/`FUN_004127e0` resolve the first row matching the parameter key;
the row flag is passed into widget assignment, and `FUN_00407ef0` gates the
write callback through `expc` before `FUN_0040a610`. The complete assignment
state path is unresolved. For each of the five statically joined controls,
Rust therefore admits only a zero row flag and an exact integer-percent float value, preserves the remaining row bytes, and
rejects nonzero/unknown flags without mutation. This is a conservative offline
editing rule, not proof that a physical device or original editor accepts the
result.

### Criterion-to-operation boundary

The admitted evidence was checked against the shared operations before adding
more HD500X behavior. The result is a bounded offline file feature and explicit
unsupported states for the other product operations; physical recording,
flashing, and full editor parity are not prerequisites for offline progress.

### Validation on the reviewed implementation head

The four configured exact-head checks passed on
`9bb4d617dcac0f4c50209ae06b53913a302b4929`: all-features
(`4611dc7c-154d-46a8-afcf-4210db78f3ce`), GUI/USB
(`37d0e7ac-edef-446b-a7d5-41a4356a0909`), Clippy
(`23e455c9-0a1b-4490-9298-8a8beebd5b63`), and formatting
(`58b79dbc-7f7e-415c-9f96-16658c10cd59). Each functional profile passed
277 tests: 223 library, 28 GUI, and 26 integration. GitHub Actions run
[44510026483](https://github.com/guirossibrum/VariaxWorkbench/actions/runs/44510026483)
passed Linux, macOS, and Windows jobs on that same implementation head.
This documentation refresh changes no product code; these results are
reported for their recorded head rather than attributed to the later docs-only
commit.

The Screamer implementation at `d2f625637c6a10c28576e6aebb5912bc531e3dde`
also passed all four configured focused checks: all-features
(`eae4fa8c-9ed5-44b9-a5c6-744e22562c0a`), GUI/USB
(`a568ad1e-8dd9-482e-a1d0-91ab3164344a`), Clippy
(`7b67a681-3437-49fc-a14a-fe407323cf85`), and formatting
(`395bb495-d6be-4504-b818-167a2c82f769`). Each functional profile passed
279 tests: 225 library, 28 GUI, and 26 integration. These results apply to
the Screamer implementation head; no GitHub Actions run is claimed for it.
The review-requested test-only additions to rejection and lifecycle coverage
are not covered by the `d2f6256` run. Their current-head result is recorded
through the PR and coordinator handoff, avoiding a docs-only head change.

| Operation | Shared entry point and current result | Product-specific evidence and remaining relation |
| --- | --- | --- |
| Import, inspect, extract, edit, save HD Edit files | `Session::import_hd_editor_file`, `extract_hd_editor_preset`, the three retained typed Octave Fuzz operations, the closed twelve-target `replace_hd_octave_fuzz_with_admitted_defaults`, selected type-1/numeric setters, and `save_hd_editor_copy` use the offline file library. 444 selected controls across 143 descriptor model IDs use model/key gates, canonical integer grids and four-byte-only writes. | Editor 2.27 file-group, row type/key, model descriptor, widget-constructor, MPRE/Screamer/SFUZ and twelve-target default initializer joins support these bounded paths. The previous three replacements passed exact-head focused checks on `a2fc16839d18066866e9522a74297a9ef8d2b491` (280 tests/profile: 226 library, 28 GUI, 26 integration); the twelve-target batch is awaiting focused checks. Each replacement validates the old model, validity/enable/count bytes, complete known-key prefix and recognized marker bytes before staging. It changes only the model word, five ordered key/zero-value rows, count 5 and first marker bytes; other bytes remain preserved. The 364 type-1 rows use the shared primary editor widget grid; the 80 numeric bounds are primary-widget integer arguments, not physical units or defaults. The active tag `0x1B` entry admits only Vintage Pre model `0x020C000E`, key `0x3F100005`, primary range 0..15,000 and normalized float conversion. The separate EDI range and post-write UI callback are not included. These joins do not establish that H5EP identifies the physical HD500X. The Windows writer reads context `nested+0x2c` into header word 2, but the bounded caller, constructor, reader, and setter joins did not find the store or initializer for that exact field. |
| Select an HD500X audio route | `Session::select_record_route` remains `Unverified` for the HD500X; the PODxt-only planner is not reused and no route plan is admitted. | Driver text supplies match IDs, interface roles, and 48 kHz/24-bit defaults. The Advanced Guide supplies documented routing behavior. Neither connects HD500X audio endpoint descriptors to driver callbacks, negotiated formats/timing, and a selected product identity. Device capture/playback remain `Locked`. |
| Inspect HD500X firmware offline | `Session::inspect_extension_firmware` performs bounded FORM/L6FF structure inspection for the HD500X session and returns declared header values, generic catalog classification, image digest/length, checksum result and region count. `physical_target` remains `Unknown`; this is not target acceptance. | The owner-supplied P25-2.62.00 PHF is task-scoped and its original release declaration is available. The existing Rust reader parses it as `0x00140005` / `0x02620000`, completes zlib decompression and MD5 comparison, and reports one covering region; the Rust offline product catalog classifies `0x00140005` as unknown. Central's metadata `id` mapper maps the same number to “POD HD500X”; separately, parser callback `0x101f3e70` reads a descriptor ID, factory `0x101f50e0` constructs the `0x00140005` object family, then that callback invokes the FORM/HEAD reader `0x101fe200` and loader `0x101f8af0`. The preserved traces do not show the PHF HEAD +8/+4 values being compared with that metadata ID and a product version. The resolved HEAD comparator `0x101aa540` instead compares HEAD +8 with a handler value supplied by an owner field whose observed producer yields `0x00200001..=0x00200007`; no join connects that handler to the `0x00140005` factory object or offline update path. The filename, release label, numeric match and adjacent call sequence do not by themselves settle that consumer relation. |
| Plan maintenance | No HD500X maintenance plan is registered. Read-installed-version, flash, reset, install, and bind operations remain `Locked` for every profile; the offline capability matrix does not promote static image inspection into a plan. | The available listing, driver-match evidence, and statically inspected PHF do not define a product-qualified maintenance sequence or join the image header consumer to the product. A release manifest plus an independently admitted maintenance protocol/source would be needed even for an offline plan; no physical operation is authorized here. |

This matrix records the current offline boundary, not a promise that the listed
missing joins are sufficient for physical compatibility. The exact H5EP writer
field producer remains a separate source join; a Mac field with a similar role
does not resolve the Windows object's producer or semantics. Generic PHF
inspection does not classify its declared product for HD500X, authenticate the
image, validate a connected unit, or authorize flashing.

The bounded enable flag is part of the current Session/library/UI surface. It
does not establish a physical target from the selected profile or opaque H5EP
context words, and it does not broaden the existing finite default operations.

### Issue #32 acceptance status

| Original acceptance criterion | Current status |
| --- | --- |
| Establish HD500X identity, editor/driver/firmware, preset/model/effect and audio evidence from primary sources. | **Partial.** Static Editor file-group association and a bounded writer/reader trace are recorded. Driver text matches USB `0x0E41:0x4159`, with the Mac audio personality on interface 0 and bulk personality on interface 1; the Advanced Guide names a processed stereo host feed, **1-2 Main Out**, plus playback return and 48 kHz native processing with documented conversion rates. This does not supply the device's USB audio descriptors/endpoints or connect them to driver callbacks, formats, and selected identity. Product attribution, complete model/effect catalog, firmware applicability and physical identity remain unresolved. |
| Implement an independent offline backend integrated with shared interfaces and tests. | **Partial.** The shared offline Session/library interface and GUI import bounded H5EP/H5ES/H5EB files, extract children, preserve context and save new copies. They edit all 364 source-identified primary type-1 rows plus 80 selected non-type-1 numeric controls across 143 descriptor model IDs with canonical-grid and byte-preservation tests, and expose three earlier Octave Fuzz replacements, the closed twelve-target defaults action, the one-byte FX enable/disable operation, and four exact type-9 three-choice model/key rows: `0x020A0014/0x3F100004`, `0x020A0010/0x3F100004`, `0x020A000F/0x3F100004`, and `0x020A0012/0x3F100003`. The type-9 setter requires an explicit file slot, valid/enable/count/assignment-marker guards, a unique existing key, finite current value and complete-candidate reparse; its GUI stages per entry/slot and applies or discards explicitly. Expanded synthetic dirty-state, extracted-child isolation and GUI lifecycle tests cover the admitted catalog; configured checks are submitted on the final head through the coordinator. The HD500X session also exposes generic bounded FORM/L6FF inspection without applying header identity to a physical profile. Product-qualified route planning and maintenance remain unavailable; other type-family semantics, physical recording and original-editor parity are not implemented. |
| Keep HD/JTV separate from PODxt/original Variax; prohibit cross-family conversion, flashing and live operations. | **Met as written.** Profiles remain separate; no cross-family conversion, upload, flash or live path is exposed, and hardware access remains locked. This criterion does not establish physical compatibility. |

Issue #32 remains open. Synthetic tests do not complete source-identification,
device-compatibility or physical-safety criteria.

## Independent files were measured and do not name the product

No original Edit application or installer was executed. A public author page,
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
remain in an ignored local location that is not published. Tests do not reopen
them.

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
remain false as product-admission flags. Static Edit resources and writer paths
support the bounded file-group and one-control implementation above, but do not
register a product-attributed device schema or complete model/effect catalog.
`Session::open_pod_preset` for the HD500X profile still returns `Unverified`;
the separate offline library does not change that identity gate.

## What remains open

The unauthenticated Edit 2.27 readeula HEAD requests redirected to login, and
the release-id directory HEAD requests returned 404; those public routes did
not supply their listed payloads. Owner-supplied private Editor/PHF/model-default
and firmware-image materials have since been statically examined. They remain
private and are not published in this report. This supersedes the earlier
acquisition-limit inference, while provenance, public availability, target
attribution, and applicability remain separate questions. This crate does not
store credentials, retry the routes already requested, or flash release 6260.

An admitted product-specific preset format, supported model/effect/default
schema, semantic editing, firmware release/product applicability, installed
versions, and USB-audio routing beyond INF defaults remain unresolved. Issue
#32 stays open. Passing these tests does not authorize enumeration, streaming,
flashing, or a driver install.
