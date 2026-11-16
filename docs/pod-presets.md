# PODxt Live tone files

Issue #29 adds an offline librarian for individual PODxt Live tone files.
It does not edit a connected POD, send MIDI or USB, switch presets, or write
firmware. Asset names and the 12-byte model and parameter records recovered
below are shown. Parameter ranges and value editing are not recovered.

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
  to `L6Assets.GetAssetName`. For device 196618 that call skips the debug
  table, the Vetta arm, and the bass-only arm. The mac and Windows copies
  return the same 348 names on the remaining path, including later
  fall-through switches. An id that path does not return stays raw. The
  fallback phrase "Unknown model" is not stored as a name.
- `analysis/decompiled/mac/cmn/l6java/patch/L6Model.java` reads a MINF as
  model id, slot id, ordinal, two ignored bytes, and an enabled byte.
  `L6Buffer.Read8` returns a Java signed byte (`(byte)(get() & 0xFF)`,
  mac and Windows lines 345–349). That byte is stored in `int m_ordinal`,
  so `0x80` is -128 and `0xFF` is -1. Enabled is `Read8() > 0`, so those
  same bytes are disabled. The Windows `L6Model.AddIFFChunk` copy matches
  the mac lines 212–215. A chunk shorter than 12 bytes keeps the model and
  slot only. The file bytes, including the two ignored bytes and any bytes
  after the first 12, stay as stored. The enabled byte is not a bypass
  flag, and this librarian does not write it.
- `analysis/decompiled/mac/cmn/l6java/patch/L6ModelParam.java` reads a PARM
  as three 32-bit words: the low 24 bits of the first word are the id, the
  second word is the value type (0 int, 1 float), and the third word is the
  raw value. The stored high byte is kept and is not required to be `0x3F`.
  Any other length stays opaque. No PODxt knob minimum, maximum, or unit
  was found. `L6ParamDefinition` is constructed only for Variax and Variax
  Acoustic.
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
the guitar renamer. Model names, the enabled byte, ordinals, reserved bytes,
and parameter values are displayed from the recovered records and are not
edited. `document::save_new` writes a new copy and refuses to
replace an existing path. The guitar document refuses to open or rename a
PODxt Live target, including a bank that contains one.

An applied name is unsaved until that tone's copy is written. Quitting asks
to save, discard, or keep editing, one tone at a time. Cancel, or a save that
is cancelled or refused, keeps the edit and leaves the application open.
Saving or discarding one tone does not clear another. Closing the tone window
keeps the in-memory library. The guitar unsaved-changes dialog is unchanged,
and confirming it does not close while a tone is still unsaved.

The session operations `OpenPodPreset` and `EditPodPreset` are offline for the
PODxt Live role and unsupported for the Variax 600 role. Physical operations
stay locked. `LIVE_BACKEND_INITIALIZER` stays `None`. The GUI window is
"Offline PODxt Live tones". A load reads a regular file only, and only after
the metadata check. A directory, pipe, or device node is not opened.

Files above 1 MiB are refused. That ceiling is an application bound, not a
recovered Line 6 limit.

## Parameter domain boundary

No PODxt Live parameter unit, minimum, maximum, or knob name was recovered.
`parameter_domain` therefore returns no domain, and
`PodTone::reject_parameter_edit` leaves every file byte unchanged. Raw PARM
bits are not knob positions. The searches were read-only checks of the local
archives. No DLL was loaded, no installer was run, and no device was opened.

- `new L6ParamDefinition(` occurs only in
  `analysis/decompiled/mac/modules/variax/device/L6VariaxParamDefs.java`,
  `analysis/decompiled/mac/modules/variaxacoustic/device/L6VariaxAcousticParamDefs.java`,
  and the matching Windows files. Those ranges are the Variax editor in
  [parameter domains](parameter-domains.md). They are not applied to a PODxt
  PARM id. `extends L6ParamMap` occurs only in the two matching `ParamMap`
  classes. `GetParamMap` implementations were found on `L6VariaxDevice` and
  `L6VariaxAcousticDevice` only.
- Mac and Windows `L6EditorAppDeviceMap` are identical. Device 196618 is
  named `modules.sharctone.device.podxt.L6PODxtDevice` and
  `modules.sharctone.editor.L6SharcToneEditor`. No class or Java file with
  either name is present under `analysis/extracted/windows-app`,
  `analysis/extracted/mac-jar`, or `analysis/decompiled`. `unzip -l` of
  `analysis/extracted/windows-workbench.jar` and the mac
  `VariaxWorkbench.jar` lists no `sharctone` entry. The packed `modules`
  trees contain only `variax` and `variaxacoustic`. The named editor was
  not available to recover a knob table from.
- Mac and Windows `L6ModelParam.java` are identical. `ReadIFFChunk` keeps
  the low 24 bits of the first word, the type word, and the raw value bits.
  The file defines id-class constants and no minimum, maximum, unit, or
  name. `GetBooleanValue` is `m_value != 0`. That comparison is not a
  recovered knob domain and is not used as one. The `0x3F` write prefix is
  a stored tag, not a range. `L6ParamDefinition.java` and
  `L6ParamFormat.java` also match across mac and Windows.
  `L6ParamFormat`, `L6SimpleFormat`, `L6FormatPercent`, `L6TwoStateFormat`,
  and `L6AssociationFormat` are generic formatters. No PODxt id is passed
  to them. `L6PatchTemplate` returns `L6TE` and contains no PODxt domain.
- The firmware resource directory in [firmware map](firmware-map.md) has
  486 name pairs. The first record is "No Amp"/"Bypass". Those are resource
  names, not PARM units or ranges. This milestone did not re-decode firmware
  and does not edit region 17.
- Product icons, driver archives, and PNG panels were not treated as
  parameter tables.

## What stays undecoded

- Parameter ranges, units, knob names, and value editing. A decoded PARM
  shows its id, type, and raw bits, and the display says that no range is
  known. A requested value edit is refused.
- PARM chunks whose length is not 12 bytes.
- Model ids the PODxt Live name path does not return. They stay raw.
- Slot ids outside `kGPXTSlot_*`. A known slot can still hold an unnamed
  model. A cab slot inside L6PA or L6FS keeps the Cab role and is not
  presented as the tone's cab model.
- The MINF enabled flag is the signed `Read8() > 0` result and is not
  rewritten. Bytes `0x80` through `0xFF` display as disabled. It is not a
  bypass meaning. Model id `0x7fffffff` has the asset name "Bypass" from
  `GetAssetName`; that name is a different field. The displayed ordinal is
  the signed Java value. The stored MINF bytes stay unchanged.
- `L6BA` / `L6CO` libraries, `L6TE` templates, and `L6GP` patches.
- Firmware region 17 and the 486-name resource directory in
  [firmware map](firmware-map.md). Those are image structures, not this tone
  file. They are not edited.
- Live transmission, preset switching, and device backup.

The firmware resource names "No Amp" and "Bypass" are not this asset table.
Some names in the table, including POD 2.0 ordinals, `BX-` model ids, and
acoustic-body ids, are returned by fall-through switches in `GetAssetName`.
They are lookup results for those ids. They are not a claim that PODxt Live
implements every one of those models.
