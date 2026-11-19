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

| Operation | Source contract | Rust operation and exact admission | Rejection and preservation |
| --- | --- | --- | --- |
| Open/rename a PODxt Live tone | `L6ToneFileIFFMap` recognizes FORM/L6PA, FORM/L6FS, FORM/L6AS; PINF product `0x0003000a`; name is fixed-width | Offline parser and librarian; name edit is bounded to its existing PINF name field | Wrong family/target, malformed files, and unsafe paths reject. Unrelated bytes remain copied from the retained source. New copies refuse overwrite. |
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

## Remaining editor and librarian workflows

| Workflow/source area | Current disposition | Admission still needed before editing |
| --- | --- | --- |
| Amp-model catalog outside the six shared controls | The 106 source-listed amp models are admitted for edits only when a matching canonical existing PARM is present; No Amp is rejected. Model selection/replacement remains unimplemented. | Source-defined model-selection association behavior, optional-pack transaction rules and model-specific defaults/side effects. Catalog membership does not prove installed entitlement or that a field is present in every model. |
| Cab models and microphone selection | Model IDs may be displayed; no numeric edit is exposed | Target-specific cab/mic definition, associated parameter ID/type/domain, model-dependent mapping, and exact preservation behavior. |
| Stomp SYNWAVE selection | `GetStompModel` slot `0x00020003`; selector `0x00030001` resolves definition index 1 and semantic PARM `0x00100001`. Line6Edit 3.06 maps `SYNWAVE_8` to models `0x020a0001`–`06`, `0x020a0007`, and `0x020a000c`; model `0x020a0009` instead uses percent-valued WAVE. | Typed `StompWaveSelection` exposes exactly eight options at keys 0, 16, 32, 48, 64, 80, 96 and 112 using the source f32 operation order. Existing unambiguous Stomp model/PARM, PODxt Live target and host version `>=3.00` are required. | Unknown raw float words read without snapping and remain unchanged until explicit choice; finite unknowns may be replaced, non-finite current words reject before mutation. Only the last matching existing type-1 PARM value word changes. Missing/opaque/wrong target/model/version/prefix/type reject. The first valid choice is negative; no percent clamp applies. |
| Stomp WHAMMY_INTERVAL Heel/Toe knob positions | `L6ControlCore` normalized control route and source quantizer for model `0x020a000e`, Stomp slot `0x00020003`: Heel selector/PARM `0x00030002`/`0x00100002`; Toe `0x00030003`/`0x00100003`. The operation is distinct from the formatted-text range mapping. | Typed `StompWhammyKnob` accepts finite normalized position `p` in `[0,1]`. The source float32 operation is `trunc_f32(p * 63.75) as integer`, then float32 multiplication by reciprocal bits `0x3c808081`, with each round preserved. It does not define physical units or semitone values. | Existing target/model/slot/record, prefix `0x3f`, type 1, finite current value in `[0,1]`, and non-opaque model are required. Read/render preserves imported finite raw values; explicit editing changes only the selected existing four-byte word and refuses invalid values before mutation. Heel/Toe have GUI, library and bundle paths. Source examples, adjacent transition neighbors, fixed points, preservation and synthetic save/reopen tests cover this offline contract; no device behavior is claimed. |
| Non-scalar Stomp controls; Mod, delay, reverb and EQ models | All 31 Stomp layouts expose 86 source-mapped percent fields; 24 Mod layouts expose 52 fields; 14 Delay layouts expose 37 fields; 15 Reverb layouts expose 42 fields with family-specific selector and PARM mappings. Delay HEADS/BITS and Stomp SYNWAVE are implemented only for their source-traced layouts. WHAMMY_INTERVAL normalized Heel/Toe knob positions are implemented as described above; their separate formatted-text mapping and other association layouts remain outside those operations. Static Line6Edit 3.06 source traces PODxt four-band EQ model `0x020c0002` in slot `0x00030004`; editor IDs `0x00090001`–`08` map to semantic PARMs `0x00100000`–`07`. Frequency uses four per-band nine-point tables and 127-step read/display conversion; graph writes interpolate continuously. Gain maps to `-12.8..12.6 dB` using source float32 endpoint arithmetic. | `PodxtEqBand` reads preserved normalized words and applies a pair update only to existing canonical type-1 records at the admitted target/model/slot and host version `>=3.00`. Both frequency and gain inputs/current words validate before either value word is patched; unknown finite imported words remain unchanged on read, while out-of-range or nonfinite words reject editing. The tone, individual library, bundle and GUI paths are connected. Synthetic tests cover all four bands, literal endpoint/interior/adjacent-boundary words, pair-only patching, failure atomicity, wrong/missing/opaque/prefix/type/version/model cases, and library/bundle/GUI save/reopen behavior. EQ enable, reset/send, missing-record creation and model replacement are excluded. These are offline source-derived software results, not device acceptance or audio behavior; no PODxt serializer oracle is established. |
| Amp enable, setup slots and model replacement | No numeric/model replacement API | Boolean/association contract, optional-pack policy, replacement side effects and transaction/failure semantics. `CheckCanWritePatch`'s installed-pack check is not an edit-domain or serializer contract. |
| Tone setup, bank/collection organization and POD Farm interchange | Existing L6BA/L6CO PODxt-only banks/collections can be inspected, navigated, edited, reverted and saved as a new copy in the offline GUI. Reads are limited to 16 MiB plus one sentinel byte during I/O; parse and replacement enforce the same total cap transactionally. Mixed-target and empty collections reject. Bank/preset creation/deletion and POD Farm conversion remain unavailable. | Source-backed container schema for creation/deletion, exact serializer rules for membership changes, and a target-specific POD Farm conversion contract. The negative POD Farm 1.12 receiver result is scoped to that package; see [its historical investigation](pod-farm-112-target-applicability.md). |
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
Other Delay association layouts, EQ enable/reset/send, EQ model creation or
replacement, unsupported EQ definition paths, amp enable/model replacement, and
cab/mic selection remain unresolved. Names, selector ordinals,
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
