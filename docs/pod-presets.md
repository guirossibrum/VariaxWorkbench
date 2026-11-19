# PODxt Live tone files

Issue #29 adds an offline librarian for individual PODxt Live tone files and
target-validated bank/collection editing in the Rust core.
It does not edit a connected POD, send MIDI or USB, switch presets, or write
firmware. This document separates historical findings from the later,
source-backed Line6Edit host-scale control subset. Neither those source scales
nor these offline edits establish device-accepted ranges or physical behavior.

## What was recovered

The original Workbench common code stores a Sharc tone with the same IFF patch
class used for other FORM tones. Local sources, kept out of git:

- `analysis/decompiled/mac/cmn/l6java/patch/L6ToneFileIFFMap.java` maps
  `FORM/L6PA` to `L6Patch` type 0, `FORM/L6FS` to type 1, and `FORM/L6AS` to
  type 2. `L6TE`, `L6BA`, `L6CO`, `L6GP`, and SoundDiver containers are not
  individual tones. `L6PatchBank.AddIFFContainer` accepts L6PA/L6FS/L6AS patch
  entries in a bank. Rust admits the three individual types in its bank reader;
  this is static format evidence, not a vendor save oracle.
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
  Any other length stays opaque. This Workbench-source audit did not find
  PODxt knob definitions in the inspected `L6ParamDefinition` classes; those
  classes construct definitions for Variax and Variax Acoustic. That bounded
  negative does not describe the separate Line6Edit source summarized below.
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
tone. The individual-tone GUI library is a list of those files.
`PodBundleLibrary` also opens existing Workbench L6BA/L6CO banks and collections
when every contained preset is a PODxt Live tone. It provides selected-tone
editing, dirty tracking, revert and atomic non-overwriting save-copy while
retaining the original bundle structure. Its POD-specific IFF framing validates
embedded tones with `PodTone` rather than routing them through the stricter
guitar `PresetFile` semantic parser; duplicate records and unrelated short
opaque PARM children therefore remain inspectable and preservable under the
POD contract. Ambiguous targeted controls still reject. It rejects empty and
mixed-target bundles. GUI bank navigation remains unimplemented; this API does
not synthesize bank membership or claim vendor save compatibility.

The PINF name edit uses the same 32-unit NUL and length rule as the guitar
renamer. Six host-scale edits are available for Drive, Bass, Mid, Treble,
Presence, and Channel Volume in amp slot `0x00030000`, for the 106 amp models
in PODxt Live `GetAmpModelList` after excluding its `0x00020000` No Amp
association. The source maps these controls to float PARM identifiers
`0x00100003`, `0x00100000`, `0x00100001`, `0x00100002`, `0x00100004`, and
`0x00100005`, respectively. Their finite host scale is 0 through 1; this does
not claim a device-accepted range or physical behavior. Each edit requires the
exact serialized prefix `0x3f`, type 1, a present parameter and a PINF
device-version word at least `3.00` under this offline profile. It patches only
the last matching parameter's four value bytes, following the source reader's
reverse lookup; it does not create a missing parameter or rewrite the
surrounding file. Unknown IDs, unsupported prefixes/types, opaque amp PARM
chunks, missing controls, out-of-domain values, No Amp and ids outside the
provider catalog remain uneditable. This source catalog does not establish
installed-pack entitlement or physical applicability. `document::save_new`
writes a new copy and refuses to replace an existing path. The guitar document
refuses to open or rename a PODxt Live target, including a bank that contains
one.

## Separate Line6Edit 3.06 host-scale contract — 2026-11-19

Static analysis of the separately supplied Line6Edit 3.06.0 original (11,742,080
bytes; SHA-256 `426ddbb1559f445bfc8d0c29c316c80fe19f9bf8ff2e71d61b065342783dbe24`)
found the PODxt device and parameter-map classes missing from the archived
Workbench application packages described below. The source chain and relevant
methods are recorded in the [source-to-operation matrix](podxt-live-control-matrix.md).
This positive result supersedes the earlier broad wording about unavailable
value editing; it leaves the historical Workbench-package observations intact.

Static Line6Edit 3.06 source maps selector positions in Stomp slot `0x00020003`
to semantic PARM ids `0x00100000 + position`. Rust exposes 86 source-derived
percent controls across all 31 PODxt Stomp model layouts. The original 16 are
`0x02000002` Sustain/Level,
`0x02000003` Level/Sustain, `0x02000004` Sustain/Level, `0x02000005`
Sensitivity/Level, `0x02000006` Amount/Level, `0x02000007` Ramp/Depth;
`0x02050000`, `02`–`05` Drive/Gain/Tone; `0x02050001` Drive/Treble/Gain/Bass;
`0x02050006` Drive/Contour/Gain/Mid/Mid Frequency; `0x02050007`
Gain/Bass/Treble/Mid/Mid Frequency; `0x02050008` Bass/Treble/Drive/Gain; and
`0x02050009` Drive/Tone/Blend, with selector position 3 left empty. The
additional layouts expose these scalar percent fields: `0x020a0000` Sensitivity
and Q at positions 1/2; `0x020a0001`, `03`–`06` Filter/Decay/Mix at positions
2/3/4; `0x020a0002` and `09` Filter/Attack/Mix at positions 2/3/4;
`0x020a0007` and `0c` Mix at position 4; `0x020a0008` and `0d` S1OG/S2OG/Mix
at positions 1/2/4; `0x020a000a` Sensitivity/Q at positions 2/3; `0x020a000b`
Decay/Sensitivity/Q at positions 1/2/3; and `0x020a000e` Position/Mix at
positions 1/4. The source's SYNWAVE association values and the special
WHAMMY_INTERVAL Heel/Toe values are not percent controls and remain unavailable.
All exposed definitions use finite host-scale floats `0..=1` with the percent
formatter; the source labels do not establish physical units. Rust applies the
same target/version/prefix/type/range checks and four-byte raw patch as the amp
controls. EQ and cab controls remain unavailable.
This allowlist does not establish installed-pack entitlement, device
acceptance or physical behavior.

The provider also maps 52 percent controls across its 24 Mod layouts in slot
`0x00020004`: Depth, Bass, Treble, Feedback, Manual, Pre-delay, Wave, Tone, Q,
Frequency, Flutter, and Distortion, only at the model-specific positions in
`SetupModLayouts`. These selected `FXParams` definitions are finite `0..=1`
floats using the percent formatter. Rust requires the matching existing model
and PARM record and edits only its four value bytes. The percent labels do not
assert physical units or device acceptance.

Its 15 Reverb layouts in slot `0x00020006` expose 42 percent fields: Dwell and
Tone at positions 1/2 on the first three models, and Pre-delay, Decay, and Tone
at positions 0/1/2 on the remaining twelve. Their selector IDs and semantic
PARM IDs use the family-specific mapping from `GetFXParamDef` and `GetFXParam`;
the Rust table preserves that distinction. The selected definitions are finite
`0..=1` PERCENT floats. Edits require the corresponding existing record and
change only its value word. These host scales do not establish physical units
or device acceptance.

The 14 Delay layouts in slot `0x00020005` expose 37 scalar percent fields.
`SetupDelayLayouts` selects Feedback, Mod Speed, Depth, Speed, Bass, Treble,
Flutter, Drive, Offset, Feedback Left/Right, Spread and Tone by model and
selector position. The selector family uses `0x00050002 + position`; its
semantic PARM ids are `0x00100000 + position`. Source association formats for
HEADS/BITS and the dedicated Delay time/note/mix/xover selectors are outside
this numeric allowlist. The exposed `FXParams` are finite `0..=1` PERCENT
floats and edits patch only the selected existing PARM value word.

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

For the admitted controls, static Line6Edit source identifies `L6PODxtDevice`,
`L6PODxtParamMap`, the PODxt amp model catalog and shared amp selectors, all 31
Stomp model layouts, the selected `L6AX3ParamDefs` definitions, and
`L6SharcTonePercentFormat`. The formatter maps finite host
values through percent text and its quantizer is a no-op; display labels are
derived through a 7-bit conversion. Rust edits the underlying finite float
without treating those display labels as a supported integer grid. Other
PODxt Live model/effect controls remain unresolved. This is a dated static
source result, not an executed vendor oracle. See the
[source-to-operation matrix](podxt-live-control-matrix.md) for the admitted
scope and remaining workflows. This is static source analysis plus independent
Rust behavior, not an executed vendor oracle, device acceptance test, or
installed-pack entitlement result. No DLL was loaded, no installer was run,
and no device was opened.

Three Stomp-specific Rust test methods cover the admitted fields across all 31
model layouts, rejection, and library revert/copy-collision/reopen behavior.
Separate Mod, Delay and Reverb tests cover source-listed layout mappings and
focused independent semantic-id, complete-file preservation, family-specific
transactional rejection, and saved/reparsed output cases. The headless egui
slider/quit-guard test exercises the admitted controls. These are synthetic Rust
behaviors, not a physical compatibility test or vendor runtime oracle.
Exact-head configured checks are tracked separately.

The [POD Farm 1.12 source-recovery milestone](pod-farm-112-source-recovery.md)
uses a separate original and remains a historical negative for that package's
control applicability. It does not negate the independently admitted
[Line6Edit 3.06 contract](podxt-live-control-matrix.md), and it grants no
hardware access.

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
  either name is present in the inspected Workbench `windows-app`, `mac-jar`,
  or decompiled trees; the corresponding Workbench archives list no
  `sharctone` entry. This is specific to those Workbench packages. The distinct
  Line6Edit 3.06 source supplies the positive, limited control contract above.
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

- Unmapped parameter ranges, physical units, device-accepted domains, and
  controls outside the admitted Line6Edit host-scale subset. A raw PARM still
  shows its id, type, and bits. Unsupported values are not edited. The admitted
  edits do not establish hardware acceptance or physical behavior.
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
