# PODxt Live source-to-operation matrix

This matrix separates the offline operations supported by this repository from
source discoveries that are not yet safe to edit. It records a narrow Rust
contract from static Windows Line6Edit 3.06.0 analysis. It does not establish
device acceptance, an installed pack, physical compatibility, or full editor
coverage. Issue [#29](https://github.com/guirossibrum/VariaxWorkbench/issues/29)
remains open and incomplete.

## Source pin and method — 2026-11-19 follow-up

The admitted installer is 11,742,080 bytes, SHA-256
`426ddbb1559f445bfc8d0c29c316c80fe19f9bf8ff2e71d61b065342783dbe24`. Static
archive listing/extraction and `javap` inspection used the provided read-only
copy. No installer, app, vendor class, native component, emulator, device, or
hardware transport was executed. The digest identifies supplied bytes; it does
not authenticate the publisher.

The relevant source chain is `L6EditorAppDeviceMap` registration of product
`0x0003000a` to `L6PODxtDevice` and `L6SharcToneEditor`; the Live version branch
in `L6PODxtDevice`; `L6PODxtParamMap.GetAmpParam` and
`GetAmpParamDef`; `L6PODxtParamMap.GetStompModelList`,
`L6SharcToneParamMap.SetupStompLayouts`, `GetStompModel`, `GetFXParamDef` and
`GetFXParam`; `L6AX3ParamDefs.Init` and its float definitions; and the shared
typed map accessors in `L6SharcToneParamMap`. For product `0x0003000a`, the
shared-map constructor sets the PODxt product branch and leaves the Bass PODxt
flag clear. The Drive definition uses `L6SharcTonePercentFormat`; the five
adjacent classic amp definitions use the same finite 0..1 float range and percent
format. The formatter's `Quantize` is a no-op; its display labels use a derived
7-bit conversion. The host source does not establish a useful default, a
quantization grid, a device-accepted range, or the physical meaning of this
continuous scale.

This is an independently recorded static source trace of the supplied original,
not a vendor runtime oracle. The source class/method chain above and the original
digest qualify the findings; private report files, extracted classes, code,
disassembly, resource tables, and local paths are not published here.

For PODxt Live, `L6PODxtParamMap.GetAmpModelList` returns 107 model ids. Rust's
amp-control allowlist mirrors those ids except `0x00020000` (No Amp). The
shared-map constructor takes the PODxt product branch, so `m_bassxt` remains
false. `GetAmpParam` maps the six amp selectors to semantic PARM ids independent
of model id; `GetAmpParamDef` selects the corresponding shared definitions.
Rust still requires each selected parameter to exist in the current model, so
catalog membership does not create missing controls or establish pack
entitlement.

## Source-backed operations

## Reverb model selector

| Operation | Exact offline contract | Rejections and evidence |
| --- | --- | --- |
| Existing-slot Reverb model ID | Typed choice is the 15 IDs 0x02040002 through 0x02040010. FORM/L6PA, PINF layout 1, target 0x0003000a, profile >=0x03000000, and exactly one existing record across primary slot 0x00020006 and fallback slot 0x00050005 are required. Candidate clone patches only the first four big-endian MINF bytes, reparses and verifies before publication. | Unknown current/requested IDs, missing/dual/duplicate slot, missing/duplicate or 8..11-byte MINF, wrong container/target/layout/profile reject. No model creation, pack/entitlement query, parameter reconciliation, reset or defaults. Same-ID validates guards before no-op. Synthetic tests cover the full 15x15 current/requested matrix in primary and fallback contexts and byte-preserving failures. This is not vendor serializer or physical-device validation. |

| Operation | Source contract | Rust operation and exact admission | Rejection and preservation |
| --- | --- | --- | --- |
| Open/rename a PODxt Live tone | `L6ToneFileIFFMap` recognizes FORM/L6PA, FORM/L6FS, FORM/L6AS; PINF product `0x0003000a`; name is fixed-width | Offline parser and librarian; name edit is bounded to its existing PINF name field | Wrong family/target, malformed files, and unsafe paths reject. Unrelated bytes remain copied from the retained source. New copies refuse overwrite. |
| Stomp model selector | `L6PODxtParamMap.GetStompModelList` supplies 31 IDs; `L6SharcToneEditor.IsModelInstalled` maps 17 IDs, including `0x02050006`, to the Pack 4 flag and returns `true` by default for the other 14. `GetStompModel` resolves slot `0x00020003`; `SetParam` calls `L6Model.SetID` on the resolved in-memory model. This does not establish vendor serialization behavior. | Typed `PodStompModelChoice` exposes only the 14 default-true IDs. Any of the 31 known current IDs can transition to one of those choices without pack inference. Offline explicit Apply requires one existing slot/MINF in FORM/L6PA, PODxt Live PINF target and profile floor; clone, patch the four-byte MINF ID, reparse, then publish. | Unknown current IDs remain raw and are not editable. Gated requested IDs, including `0x02050006`, cannot be represented by the typed API. Missing/duplicate slot or MINF, short MINF, wrong target/container/profile reject atomically. Same-ID validates all guards then no-ops. All other bytes and parameters are retained. This is a conservative Rust raw-file policy, not full Line6Edit callback parity or device acceptance. GUI staging is per tone with Apply/Discard; a successful change clears Stomp Harmony staging while retaining unrelated Delay/Mod staging. |
| Amp Drive | Amp slot `0x00030000`; `GetAmpParam` maps selector `0x00010002` to semantic PARM `0x00100003`; `GetAmpParamDef` selects `AMP_DRIVE`, finite float `0..1`, and `L6SharcTonePercentFormat` | Slider/API admit the 106 `GetAmpModelList` ids except No Amp, the device-version word `>=0x03000000` under the Rust offline profile, a present last matching PARM, raw prefix `0x3f`, type 1, and finite current/new value in `[0,1]` | Missing/opaque/unsupported record, No Amp, wrong model/target/version, or invalid float rejects before mutation. Only the four big-endian value bytes change. |
| Amp Bass | Same target/catalog/slot path; selector `0x00010003` maps to semantic PARM `0x00100000`; `GetAmpParamDef` selects `AMP_BASS`, float range `0..1` | Same finite host scale, prefix/type, last-match and version gates as Drive | No default, step, device-accepted range, physical effect, or installed entitlement is claimed. Missing or malformed PARM stays uneditable. |
| Amp Mid | PODxt product branch selects `AMP_MID`; selector `0x00010004`, semantic PARM `0x00100001`, float range `0..1` | Same catalog and existing-record patch contract | Same rejections and four-byte write set. Bass PODxt's alternate Lo Mid mapping is not used for product `0x0003000a`. |
| Amp Treble | PODxt branch selects `AMP_TREBLE`; selector `0x00010005`, semantic PARM `0x00100002`, float range `0..1` | Same catalog and existing-record patch contract | Same rejections and four-byte write set. |
| Amp Presence | PODxt branch selects `AMP_PRESENCE`; selector `0x00010006`, semantic PARM `0x00100004`, float range `0..1` | Same catalog and existing-record patch contract | Same rejections and four-byte write set. |
| Amp Channel Volume | PODxt branch selects `AMP_CHANVOL`; selector `0x00010007`, semantic PARM `0x00100005`, float range `0..1` | Same catalog and existing-record patch contract | Same rejections and four-byte write set. |
| Stomp percent controls on all 31 models | `GetStompModel` resolves slot `0x00020003`; `GetFXParamDef` selector positions are 1-based and `GetFXParam` maps position *n* to semantic PARM `0x00100000+n`. The first 16 layouts expose: `0x02000002` Sustain/Level; `03` Level/Sustain; `04` Sustain/Level; `05` Sensitivity/Level; `06` Amount/Level; `07` Ramp/Depth; `0x02050000`, `02`–`05` Drive/Gain/Tone; `01` Drive/Treble/Gain/Bass; `06` Drive/Contour/Gain/Mid/Mid Frequency; `07` Gain/Bass/Treble/Mid/Mid Frequency; `08` Bass/Treble/Drive/Gain; `09` Drive/Tone/empty/Blend. The remaining `0x020a` layouts expose only their percent definitions: `00` Sensitivity/Q; `01,03–06` Filter/Decay/Mix at positions 2/3/4; `02,09` Filter/Attack/Mix at positions 2/3/4; `07,0c` Mix at position 4; `08,0d` S1OG/S2OG/Mix at positions 1/2/4; `0a` Sensitivity/Q at positions 2/3; `0b` Decay/Sensitivity/Q at positions 1/2/3; `0e` Position/Mix at positions 1/4. `L6AX3ParamDefs` gives these selected fields finite float `0..1` definitions with the percent formatter. | Rust exposes only those source-listed scalar model/layout pairs in Stomp slot `0x00020003`, for the PODxt Live PINF target and host-profile version `>=3.00`. It requires the last matching PARM, canonical stored prefix `0x3f`, float type 1 and a finite existing value in `[0,1]`; each UI/API edit changes only that record's four raw value bytes. The discrete SYNWAVE association is handled separately for exactly Stomp models `0x020a0001`, `0x020a0002`, `0x020a0003`, `0x020a0004`, `0x020a0005`, `0x020a0006`, `0x020a0007`, and `0x020a000c`; `0x020a0009` has a distinct percent-valued WAVE definition. Special WHAMMY_INTERVAL Heel/Toe controls are implemented separately below. | The shared percent formatter has no quantization; integer labels are display conversions, not a supported input grid. Missing, opaque, wrong-prefix/type, wrong-model/slot/target/version and invalid-domain records reject before mutation. Known duplicate IDs follow the source's last-match lookup; opaque PARM records make the target ambiguous and reject. This allowlist does not assert installation, entitlement, device acceptance, physical units, or physical behavior. Mod, Delay and Reverb controls are handled by their own source mappings below. |

For each amp control, static `L6ModelParam.ReadIFFChunk` masks the first word
to a semantic ID and reads a value type; the Rust operation additionally
requires canonical serialized prefix `0x3f` and float type 1. Static
`L6Model.FindParam` scans from the end, so Rust edits the last duplicate and
rejects any opaque parameter in the amp slot that would make the ID ambiguous.
All other header bytes, padding, unknown child chunks, parameters, and earlier
duplicates remain untouched. A source-defined continuous host scale does not
turn the formatter's displayed integer labels into an input grid.

The source's visible model writer reconstructs MINF and known PARM records and
does not emit a raw unknown-child path; the enclosing unknown-chunk handling was
not fully traced. Rust therefore patches the original serialized bytes in place
instead of round-tripping through a reconstructed vendor-style model object.
Library dirty tracking compares against the original byte baseline, supports
revert, and writes atomic non-overwriting copies. These are Rust behaviors tested
with synthetic data, not a native vendor save/reopen oracle.

## Delay association control contract — implemented offline subset

The admitted Line6Edit 3.06.0 source defines `L6AX3ParamFormats.HEADS` and
`BITS` as `L6AssociationFormat` instances. `L6AX3ParamFormats.Init` sets fuzzy
mode `FUZZY_SNAP_LOW` and quantization on for both; `AddCCValuePair` maps each
integer controller value `v` to the floating key `(v - 0.5) / 127.0`. HEADS
associates controller values `0,16,32,48,64,80,96,112,127` with four-head
patterns; BITS associates those same values with labels `12` through `4`.
These labels are formatter objects, not integer PARM values or a percentage
scale. `L6AssociationFormat.ToObject` returns no object when there is no pair;
`FromString` rejects text. `FromObject` accepts an `ObjectValuePair` instance
directly, without checking membership, or looks up a registered object.
Fuzzy lookup can snap an unrecognized input to its lower neighbor for display,
so Rust retains the raw word unless a recognized finite selection is made.

| PODxt Live delay model | `SetupDelayLayouts` definition | Selector / definition position | Semantic PARM id | Source value representation |
| --- | --- | --- | --- | --- |
| `0x02020005` | `HEADS` | `DELAY_PARAM3` (`0x00050004`) / array position 2 | `0x00100002` | `L6ParamDefinition.IsFloat`; `L6ModelParam` float bits, type 1 |
| `0x0202000D` | `BITS` | `DELAY_PARAM4` (`0x00050005`) / array position 3 | `0x00100003` | `L6ParamDefinition.IsFloat`; `L6ModelParam` float bits, type 1 |

`GetModelForGroup` selects the Delay model, `GetFXParamDef` maps those selector
constants to the positions above, and `GetFXParamLayoutDef` indexes the array
registered for the model id. `GetFXParam` maps the selector to the semantic
PARM id and asks `L6Model.FindParam` for the record. `L6SharcToneParamMap.GetParam`
returns the raw value bits; `SetParam` updates those bits and assigns the value
type from the selected definition's `IsFloat` result. `L6ModelParam` serializes
PARM as id, type, and value words; `SetFloatValue` uses IEEE-754 float bits and
sets type 1. Thus the formatter's normalized float keys are the source-backed
stored representation for these two controls.

This contract applies to the PODxt Live product branch (`0x0003000a`) in the
supplied Line6Edit source. It does not establish firmware-version compatibility,
physical-device acceptance, or a device-side interpretation. The host map can
create missing model/parameter records when called with its create flag; Rust
only exposes edits for an existing matching typed PARM. The GUI and library
use finite typed choices, encode each controller key with the source float32
operation order (`i2f`, subtract `0.5f`, divide by `127.0f`), and patch only the
existing value word. Controller zero therefore encodes a negative value and
is not percent-clamped. Unknown finite raw values remain unchanged until an
explicit choice; imported NaN and infinity bit patterns remain lossless on
read but reject any attempted association edit. Duplicate ids use the source last-match behavior. Wrong target,
version, model, slot, prefix, type, missing/opaque records, and ambiguous slot
layouts reject before mutation. Synthetic Rust tests cover every choice,
byte preservation, unknown values, rejection, duplicate lookup, and library
dirty/revert/save/reopen behavior. This is offline Rust behavior checked
against the admitted static contract, not a vendor-runtime or hardware oracle.

## Mod NOTE/SPEED source contract proposal — research only

This is a proposed next bounded control contract, not an implementation or an
acceptance claim. It is based on static bytecode from the already admitted
Line6Edit 3.06.0 artifact (SHA-256
`426ddbb1559f445bfc8d0c29c316c80fe19f9bf8ff2e71d61b065342783dbe24`). No
vendor class was executed.

Relevant symbols are `modules.sharctone.device.L6SharcToneEditor` methods
`Note2TimeSpeed(int,float,boolean)`, `UpdateNoteValues()`, and
`OnParamChange(int,int,boolean)`; `modules.sharctone.device.L6AX3ParamFormats`
fields `NOTE`, `MODSPEED`, and `DELAYTIME`; `modules.sharctone.device.L6AX3ParamDefs`
fields `MOD_NOTE`, `MOD_SPEED`, `DELAY_NOTE`, and `DELAY_TIME`; and
`modules.sharctone.device.L6SharcToneParamMap` methods `GetModModel`,
`GetDelayModel`, `GetFXParam`, and `GetFXParamDef`. The shared conversion is
`cmn.l6java.patch.paramformats.L6RangeFormat.TranslateFromDisplayedValue(double)`;
the editor applies it via `L6Editor.SetDisplayedParam(int,double,boolean)`.

| Control | Source selector id | Existing semantic PARM | Type | Model lookup |
| --- | ---: | ---: | --- | --- |
| Mod SPEED | `0x00040001` | `0x00100000` | float | Mod slot `0x00020004`, else `0x00050003` |
| Mod NOTE | `0x00040002` | `0x00200000` | integer ordinal | Mod slot `0x00020004`, else `0x00050003` |
| Delay TIME | `0x00050001` | `0x00100000` | float | Delay slot `0x00020005`, else `0x00050004` |
| Delay NOTE | `0x00050002` | `0x00200000` | integer ordinal | Delay slot `0x00020005`, else `0x00050004` |
| System Tempo | `0x000c0001` | `0x00200001` | float BPM | System slot `0x00010000` |

`L6PODxtParamMap.GetModModelList` supplies 24 Mod ids:
`0x02030004`, `0x02030005`, `0x02030006`, `0x02030007`, `0x02030008`,
`0x02030009`, `0x0203000a`, `0x0203000b`, `0x0203000c`, `0x0203000d`,
`0x0203000e`, `0x02030015`, `0x02030012`, `0x0203001e`, `0x02030014`,
`0x0203001f`, `0x02030013`, `0x02030011`, `0x0203001a`, `0x0203001b`,
`0x02030017`, `0x02030018`, `0x02030019`, `0x0203001c`. The special Mod
NOTE/SPEED selector definitions and `GetFXParam` semantic ids are returned
independently of per-model percent-layout arrays. A bounded writer should
admit only one existing source-listed Mod model, one selected primary or
fallback slot, and unique existing canonical NOTE and SPEED records; it must
not synthesize missing models or PARMs. `NOTE` is the shared ordinal table
0–13. `MODSPEED` is displayed in Hz over `[0.1,15]`; `DELAYTIME` is displayed
in ms over `[20,2000]`; both `L6RangeFormat` definitions request clipping.

The existing-slot Mod model selector is a separate admitted offline edit. It
permits requested IDs `0x02030004..=0x0203000e` only, while recognizing all 24
listed IDs as valid current models. The other 13 current IDs are source Pack-4
gated and cannot be requested from an offline file. The writer requires one
primary (`0x00020004`) or fallback (`0x00050003`) model in FORM/L6PA for PODxt
Live, validates its unique full MINF, and changes only the four-byte ID. It
does not reset/reconcile parameters or claim device/pack applicability. The
literal full-container vector and exact tests are in the
[model-selection contract](pod-model-selection-contract.md).

`UpdateNoteValues()` reads Tempo as `f32`, then Mod NOTE and Delay NOTE as
integers. For each NOTE ordinal greater than zero it calls
`Note2TimeSpeed(note, tempo, true)` for Mod SPEED and
`Note2TimeSpeed(note, tempo, false)` for Delay TIME, then passes the returned
`f32` (widened to `double`) to `SetDisplayedParam`. The common NOTE multipliers
are `1.0f`, `0.75f`, `0.5f`, `0.33333334f`, `0.375f`, `0.25f`,
`0.16666667f`, `0.1875f`, `0.125f`, `0.083333336f`, `0.09375f`, `0.0625f`,
and `0.041666668f` for ordinals 1 through 13. The method multiplies the
multiplier by `4.0f` first. Mod SPEED is `tempo / (q * 60.0f)`; Delay TIME is
`(q / tempo) * 1000.0f * 60.0f`, with each operation in `f32`. `L6RangeFormat`
computes display-minus-min in `double`, computes `max - min` in `f32` before
promoting it, divides in `double`, then the editor casts the normalized result
to `f32` before writing. Source Tempo has range `[30,240]` BPM.

The callback also establishes separate edit behavior. A positive Mod NOTE
event updates Mod SPEED and updates Delay TIME if Delay NOTE is positive. A
manual Mod SPEED event clears a positive Mod NOTE and otherwise writes SPEED;
Mod NOTE Off does not recompute SPEED. System Tempo changes use the same
`UpdateNoteValues()` path. The bounded implementation requires Delay NOTE to
be the unique existing integer zero value before either Mod NOTE event,
preventing the cross-Delay TIME side effect. Positive Mod NOTE updates NOTE
and derived SPEED atomically; Off writes only NOTE and does not read SPEED or
Tempo. Manual SPEED is a separate Hz operation; it clears an active Mod NOTE
and requires no Delay or System context. Positive Mod NOTE validates unique
canonical Mod SPEED and Tempo. All touched values and required contexts are
validated before writing; unsupported, ambiguous, opaque, wrong-type,
noncanonical or unknown consumed values reject without changing bytes.

Literal synthetic expectations for NOTE `6` (Quarter) at 120 BPM are `q =
1.0f`, Mod SPEED `2.0f Hz`, normalized SPEED bits `0x3e0293c2`, Delay TIME
`500.0000305175781f ms`, and normalized TIME bits `0x3e783e11`. If Delay NOTE
is positive, the TIME bits are the independent cross-effect the source
callback would also write; the proposed Mod NOTE operation rejects that
fixture before mutation. With Delay NOTE zero, it writes Mod NOTE ordinal 6
and SPEED bits `0x3e0293c2`, preserving Delay TIME and all other bytes. Manual
3.0 Hz SPEED converts to `0x3e474d51` and clears positive Mod NOTE. Mod NOTE
Off preserves the prior SPEED word. These are synthetic expectations, not a
runtime or device oracle.

`GetSystemModel` uses default id `0x3e700000`, and `GetSingleSlotModel` returns
an existing slot model without enforcing that default id. The bounded
application deliberately admits positive NOTE edits only when exactly one
System model at slot `0x00010000` has the default id `0x3e700000`; existing
`0x3e800000` and unknown ids remain lossless but reject as Tempo context.
This conservative gate is an application policy, not a claim about legacy
getter rejection. Delay positive NOTE now uses the same gate; Delay Off and
explicit TIME keep their isolated dependencies.

## Issue #29 acceptance gap ledger

All three original guirossibrum/VariaxWorkbench issue #29 criteria remain
partial; this PR and its bounded controls do not complete the issue.

| Original criterion | Evidence already in this bounded implementation | Required work still missing | Explicitly outside the admitted contracts |
| --- | --- | --- | --- |
| Recover POD-specific formats, model/effect controls, source references, and synthetic fixtures | FORM/L6PA, L6FS, and L6AS parsing; source-backed existing-record controls and synthetic checks for listed scalar layouts, selected associations, EQ, and model flags; static Delay selector trace, implemented ID-only selector, and literal full-buffer vector in [model-selection contract](pod-model-selection-contract.md) | Cover per-model records, remaining association definitions, coupled side effects, domains, and precision | Untraced associations, unsupported families, model/PARM creation or replacement outside separately admitted operations, vendor runtime, and live behavior |
| Implement offline POD inspection/editing/library workflows alongside guitar without target confusion | Separate POD parser/backend, offline tone/session/library/bundle views, bounded existing tone/bank organization operations, target rejection, Apply/save-copy flows, and the admitted staged per-tone Delay model-ID selector for one existing L6PA primary-or-fallback MINF, retaining parameters and unknown bytes | Broader source-supported controls/workflows; bank/folder creation/deletion and tone synthesis; compatibility across remaining containers and model families | POD Farm conversion, unsupported container combinations, absent-record repair, and synthesis until their own contracts exist |
| Test compatibility, unknown-field preservation, backups, and target-specific rejection while live transmission remains separately gated | Byte-preserving bounded writes, malformed/unknown/duplicate rejection, dirty/save-copy/reopen paths, and POD/guitar target guards for exercised operations; Delay selector tests cover 9 choices × 14 current IDs, the independent 188-byte buffer, MINF lengths 8–11, missing/duplicate MINF and slots, target/profile/container failures, all five pack-gated requests, and tone/library/bundle/GUI lifecycle | Broader format/model compatibility and backup/recovery evidence remain | Physical acceptance, audio equivalence, recovery capability, hardware traffic, and lock overrides are not claims or substitutes for offline criteria |

This ledger records gaps rather than a new task queue. Unsupported operations
stay rejected until individually sourced and admitted; the hardware boundary
remains independently locked.

## Remaining editor and librarian workflows

| Workflow/source area | Current disposition | Admission still needed before editing |
| --- | --- | --- |
| Amp-model catalog outside the six shared controls | The 106 source-listed amp models are admitted for numeric edits only when a matching canonical existing PARM is present; No Amp is rejected. Separately, AMP model enable control `0x40010001` edits only MINF byte 11 on exactly one existing model in slot `0x00030000`, with no additional model-ID allowlist. Model selection/replacement remains unimplemented. | Source-defined model-selection association behavior, optional-pack transaction rules and model-specific defaults/side effects. Catalog membership does not prove installed entitlement or that a field is present in every model. |
| Cab models and microphone selection | Source-backed offline edits are admitted for existing L6AS CAB slots only. CAB Apply accepts the 25 unconditional ids (`0x01020000..=0x01020016`, `0x01020018`, `0x0102001a`), rejects the 22 pack-gated bass ids and holes, and changes only the MINF model-id word. MIC Apply is separate and accepts only an existing canonical integer PARM id `0` with prefix `0x3f`, on an admitted CAB; its four choices patch only the value word. Unknown MIC values are not shown as a valid current choice, remain raw during CAB Apply, and require explicit MIC Apply to replace. GUI selection is staged across frames and per-tone until Apply. | CAB replacement, absent-slot creation, noncanonical/missing/ambiguous MIC repair, installed-pack entitlement, and hardware acceptance remain unavailable or unverified. |
| Stomp SYNWAVE selection | `GetStompModel` slot `0x00020003`; selector `0x00030001` resolves definition index 1 and semantic PARM `0x00100001`. Line6Edit 3.06 maps `SYNWAVE_8` to models `0x020a0001`–`06`, `0x020a0007`, and `0x020a000c`; model `0x020a0009` instead uses percent-valued WAVE. | Typed `StompWaveSelection` exposes exactly eight options at keys 0, 16, 32, 48, 64, 80, 96 and 112 using the source f32 operation order. Existing unambiguous Stomp model/PARM, PODxt Live target and host version `>=3.00` are required. | Unknown raw float words read without snapping and remain unchanged until explicit choice; finite unknowns may be replaced, non-finite current words reject before mutation. Only the last matching existing type-1 PARM value word changes. Missing/opaque/wrong target/model/version/prefix/type reject. The first valid choice is negative; no percent clamp applies. |
| Stomp HARMONY_1M335 and HARMONY_1457 selectors | Line6Edit 3.06 `GetStompModel`/`GetFXParamDef` layout `0x020a0009` in slot `0x00020003`; selector positions 1/2 (`0x00030001`/`0x00030002`) map to semantic PARMs `0x00100001`/`0x00100002`. | Two separate typed nine-choice domains, each with keys `0,16,32,48,64,80,96,112,127` and float32 words `bb810204`, `3df9f3e8`, `3e7dfbf8`, `3ebf7efe`, `3f000000`, `3f204081`, `3f408102`, `3f60c183`, `3f7efdfc`. HARMONY_1M335 labels are `-1 OCT`, `-MAJ6TH`, `-MIN6TH`, `-4TH`, `UNISON`, `MIN3RD`, `MAJ3RD`, `5TH`, `1 OCT`; HARMONY_1457 labels are `-1 OCT`, `-5TH`, `-4TH`, `-2ND`, `UNISON`, `4TH`, `5TH`, `7TH`, `1 OCT`. GUI stages per tone/control and writes only after explicit Apply. | Requires exactly one existing Stomp model `0x020a0009`, target PINF `0x0003000a`, host version `>=3.00`, nonopaque model and exactly one recorded matching semantic PARM with canonical prefix `0x3f`, type 1 and finite float value. Duplicate semantic target records are preserved on read but refused by getters, setters, libraries, bundles, sessions and GUI admission; there is no last-match fallback for harmony. Unknown finite values display as unknown and remain raw until Apply. Nonfinite values and all failed guards reject without mutation; same-choice Apply still runs guards. Each apply changes only its selected existing four-byte word. No choice is added to SYNWAVE and no physical behavior is asserted. |
| Stomp WHAMMY_INTERVAL Heel/Toe knob positions | `L6ControlCore` normalized control route and source quantizer for model `0x020a000e`, Stomp slot `0x00020003`: Heel selector/PARM `0x00030002`/`0x00100002`; Toe `0x00030003`/`0x00100003`. The operation is distinct from the formatted-text range mapping. | Typed `StompWhammyKnob` accepts finite normalized position `p` in `[0,1]`. The source float32 operation is `trunc_f32(p * 63.75) as integer`, then float32 multiplication by reciprocal bits `0x3c808081`, with each round preserved. It does not define physical units or semitone values. | Existing target/model/slot/record, prefix `0x3f`, type 1, finite current value in `[0,1]`, and non-opaque model are required. Read/render preserves imported finite raw values; explicit editing changes only the selected existing four-byte word and refuses invalid values before mutation. Heel/Toe have GUI, library and bundle paths. Source examples, adjacent transition neighbors, fixed points, preservation and synthetic save/reopen tests cover this offline contract; no device behavior is claimed. |
| Non-scalar Stomp controls; Mod, delay, reverb and EQ models | All 31 Stomp layouts expose 86 source-mapped percent fields; 24 Mod layouts expose 52 fields; 14 Delay layouts expose 37 fields; 15 Reverb layouts expose 42 fields with family-specific selector and PARM mappings. Delay HEADS/BITS, Stomp SYNWAVE, and the distinct Stomp harmony associations are implemented only for their source-traced layouts. WHAMMY_INTERVAL normalized Heel/Toe knob positions are implemented as described above; their separate formatted-text mapping and other association layouts remain outside those operations. Static Line6Edit 3.06 source traces PODxt four-band EQ model `0x020c0002` in slot `0x00030004`; editor IDs `0x00090001`–`08` map to semantic PARMs `0x00100000`–`07`. Frequency uses four per-band nine-point tables and 127-step read/display conversion; graph writes interpolate continuously. Gain maps to `-12.8..12.6 dB` using source float32 endpoint arithmetic. EQ enable control `0x40090000` is separately admitted for the existing fixed model/slot on the PODxt Live target. | `PodxtEqBand` reads preserved normalized words and applies a pair update only to existing canonical type-1 records at the admitted target/model/slot and host version `>=3.00`. Both frequency and gain inputs/current words validate before either value word is patched; unknown finite imported words remain unchanged on read, while out-of-range or nonfinite words reject editing. AMP/EQ enable edits accept typed booleans and patch only the existing applicable MINF byte 11; they require current byte `00`/`01` and reject unsupported, missing, duplicate, wrong EQ model, short or noncanonical targets before mutation. The source read interpretation remains signed `Read8() > 0`; noncanonical bytes are displayed but the toggles are disabled. AMP uses no extra model-id allowlist for this structural flag. Tone, individual library, bundle, session and GUI paths are connected. Synthetic coverage exercises both directions, literal byte-only changes, all noncanonical byte classes, preservation, target rejection and save/reopen/dirty paths. EQ reset/send, missing-record creation and model replacement remain excluded. These are offline source-derived software results, not device acceptance or audio behavior; no PODxt serializer oracle is established. |
| Amp setup slots and model replacement | No setup-slot/model replacement API | Boolean/association contract for setup slots, optional-pack policy, replacement side effects and transaction/failure semantics. `CheckCanWritePatch`'s installed-pack check is not an edit-domain or serializer contract. |
| Tone setup, bank/collection organization and POD Farm interchange | Existing L6BA/L6CO PODxt-only banks/collections can be inspected, navigated, edited, reverted and saved as a new copy in the offline GUI. Existing tones can be duplicated, removed and reordered only within L6CO/FLDR. A separate count-preserving operation reorders only existing target `0x0003000a` L6PA tones within one nonempty L6BA/BANK, retaining complete raw tone spans, opaque sibling positions and enclosing lengths. Reads are limited to 16 MiB plus one sentinel byte during I/O; parse and replacement enforce the same total cap transactionally. Mixed-target inputs reject. Fixed-bank duplicate/remove, mixed L6FS/L6AS banks, other root/bank combinations, bank/preset creation/deletion and POD Farm conversion remain unavailable. | Source-backed container schema for creation/deletion, broader membership operations, and a target-specific POD Farm conversion contract. The admitted reorder does not establish physical bank-slot layout or device acceptance. The negative POD Farm 1.12 receiver result is scoped to that package; see [its historical investigation](pod-farm-112-target-applicability.md). |
| Send, switch, firmware, backup, audio and maintenance operations | Disabled or unimplemented | Separate future requirements and explicit authorization. No such live operation follows from the offline contracts here. |

The current Rust implementation covers six shared amp controls across 106
source-listed amp models (excluding No Amp), 86 source-mapped Stomp percent
controls across all 31 provider layouts, 52 source-mapped Mod percent controls
across all 24 provider layouts, 37 source-mapped Delay percent controls across
all 14 provider layouts, and 42 Reverb percent controls across all 15 provider
layouts. A separate typed eight-choice Stomp SYNWAVE selector applies only to
the eight models in the table above. The app can also navigate and edit existing
homogeneous POD banks and collections in its offline library view. Other controls and workflows remain
rejected or unimplemented until their own source contract and synthetic
compatibility tests are admitted. This matrix is not an issue-acceptance claim.
The broader criteria for formats/models/effects, editor workflows and
compatibility/preservation/rejection testing remain partial. Lack of physical
testing is not an offline criterion gap; hardware remains locked independently.

## Remaining source/input gaps — 2026-11-19

The admitted Line6Edit 3.06 trace and synthetic Rust coverage establish
Delay `HEADS` on `0x02020005`, `BITS` on `0x0202000D`, Stomp `SYNWAVE` on
exactly eight models, and normalized Stomp `WHAMMY_INTERVAL` Heel/Toe positions
on `0x020a000e` as editable offline controls under the restrictions above.
Other Delay association layouts, EQ reset/send, EQ model creation or
replacement, unsupported EQ definition paths, and amp model
selection/replacement remain unresolved. CAB/MIC edits are limited to the
existing-slot contract in the table above; missing-slot creation and invalid
MIC repair remain unresolved. Names, selector ordinals,
a shared AX3 id, or a generic float definition do not by themselves establish
the PODxt association, stored type, domain, unit, side effects, error behavior,
or preservation rules.

Bounded read-only searches of the retained archived Workbench trees
`analysis/decompiled/mac/line6editbase`,
`analysis/decompiled/windows/line6editbase`,
`analysis/extracted/mac-jar/line6editbase`, and
`analysis/extracted/windows-app/line6editbase` found no source member chain
that fills the still unresolved control contracts. Those historical negative
searches cover Workbench trees only and do not establish absence from the
separately admitted Line6Edit source. The trace above uses retained
`device-provider.txt`, `param-formats-followup.txt`, and
`patch-codec-formats.txt` outputs for the pinned original digest. No new source
was fetched or vendor code executed for this check.
