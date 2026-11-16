# PODxt Live tone files

Issue #29 adds an offline librarian for individual PODxt Live tone files.
It does not edit a connected POD, send MIDI or USB, switch presets, or write
firmware. Effect parameters and asset names were not recovered, so those
payloads stay undecoded.

## What was recovered

The original Workbench common code stores a Sharc tone with the same IFF patch
class used for other FORM tones. Local sources, kept out of git:

- `analysis/decompiled/mac/cmn/l6java/patch/L6ToneFileIFFMap.java` maps
  `FORM/L6PA` to `L6Patch` type 0, `FORM/L6FS` to type 1, and `FORM/L6AS` to
  type 2. `L6TE`, `L6BA`, `L6CO`, `L6GP`, and SoundDiver containers are not
  individual tones and are rejected here.
- `analysis/decompiled/mac/cmn/l6java/device/L6ProductConstants.java` sets
  `PODXT_LIVE` to 196618 (`0x0003000A`). `IsVariaxFamily` is the high word
  `0x00070000`. Other Sharc product ids, including Bass PODxt Live 196619,
  are not this target.
- `analysis/decompiled/mac/cmn/l6java/patch/L6Patch.java` method
  `UpdateMetaAmpName` reads model slot 196608 (`0x00030000`) for a Sharc tone.
  `UpdateMetaNotesForSetup` reads slot 196609 (`0x00030001`) only when
  `GetType()` is 2. That is the L6AS container. The stored model id is passed
  to `L6Assets.GetAssetName`. That name table was not recovered, so the
  librarian shows the raw id.
- `analysis/decompiled/mac/cmn/l6java/midi/L6MIDIJava/L6MIDIJava.java` defines
  `L6HW_ID_PODXTLIVE` as 18000. That MIDI hardware id is not the PINF target.

The PINF layout used here is the shared Workbench layout already accepted for
guitar files: version word 1, target id, 32 UTF-16BE name units, then the
device-version word. The device-version word is preserved. It is not required
to be the archived firmware catalog value `0x03010000`. Bytes outside the name
field, including unknown chunks, odd-length padding, short PARM chunks, and
the PINF tail, are kept. Duplicate model slots are rejected because
`GetModel(slot)` would be ambiguous.

Vetta slots `0x01030000` and `0x01040000` are not used. Guitar slots
`0x02010000` through `0x02010005` are not applied to a POD tone.

## What this librarian does

`src/pod_preset.rs` accepts one `FORM/L6PA`, `FORM/L6FS`, or `FORM/L6AS` file
whose PINF target is `0x0003000A`. A Variax-family id is a cross-family
rejection. Any other id is rejected and is not treated as a decoded PODxt Live
tone. The in-memory library is a list of those files. It is not a new bank
format.

The only edit is the 32-unit PINF name, with the same NUL and length rule as
the guitar renamer. `document::save_new` writes a new copy and refuses to
replace an existing path. The guitar document refuses to open or rename a
PODxt Live target, including a bank that contains one.

The session operations `OpenPodPreset` and `EditPodPreset` are offline for the
PODxt Live role and unsupported for the Variax 600 role. Physical operations
stay locked. `LIVE_BACKEND_INITIALIZER` stays `None`. The GUI window is
"Offline PODxt Live tones". A load reads a regular file only, and only after
the metadata check. A directory, pipe, or device node is not opened.

Files above 1 MiB are refused. That ceiling is an application bound, not a
recovered Line 6 limit.

## What stays undecoded

- Amp and cab asset names.
- The meaning of every model slot other than the two named above. A cab slot
  inside L6PA or L6FS is counted, not presented as a cab.
- PARM and MINF bytes beyond the 8-byte model id and slot. No ranges, effect
  ids, or bypass rules were recovered.
- `L6BA` / `L6CO` libraries, `L6TE` templates, and `L6GP` patches.
- Firmware region 17 and the 486-name resource directory in
  [firmware map](firmware-map.md). Those are image structures, not this tone
  file. They are not edited.
- Live transmission, preset switching, and device backup.

The firmware resource names "No Amp" and "Bypass" are not the user-preset
asset table.
