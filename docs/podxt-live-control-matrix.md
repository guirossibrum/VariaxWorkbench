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
`GetAmpParamDef`; `L6AX3ParamDefs.Init` and its float definitions; and the shared
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

## Source-backed operations

| Operation | Source contract | Rust operation and exact admission | Rejection and preservation |
| --- | --- | --- | --- |
| Open/rename a PODxt Live tone | `L6ToneFileIFFMap` recognizes FORM/L6PA, FORM/L6FS, FORM/L6AS; PINF product `0x0003000a`; name is fixed-width | Offline parser and librarian; name edit is bounded to its existing PINF name field | Wrong family/target, malformed files, and unsafe paths reject. Unrelated bytes remain copied from the retained source. New copies refuse overwrite. |
| Amp Drive | Amp slot `0x00030000`; Plexi Lead 100 `0x0002001b`; semantic PARM `0x00100003`; `AMP_DRIVE` float definition and `L6SharcTonePercentFormat` | Slider/API admit only this model, device-version word `>=0x03000000` under the Rust offline profile, present last matching PARM, raw prefix `0x3f`, type 1, finite current/new value in `[0,1]` | Missing/opaque/unsupported record, wrong model/target/version, or invalid float rejects before mutation. Only the four big-endian value bytes change. |
| Amp Bass | Same product/model/slot path; `GetAmpParamDef` selects `AMP_BASS`, semantic PARM `0x00100000`, float range `0..1` | Same finite host scale, prefix/type, last-match and version gates as Drive | No default, step, device-accepted range, physical effect, or installed entitlement is claimed. Missing or malformed PARM stays uneditable. |
| Amp Mid | `GetAmpParamDef` selects `AMP_MID` for the PODxt product branch; semantic PARM `0x00100001`, float range `0..1` | Same existing-record patch contract | Same rejections and four-byte write set. Bass PODxt's alternate Lo Mid mapping is not used for product `0x0003000a`. |
| Amp Treble | PODxt branch selects `AMP_TREBLE`; semantic PARM `0x00100002`, float range `0..1` | Same existing-record patch contract | Same rejections and four-byte write set. |
| Amp Presence | PODxt branch selects `AMP_PRESENCE`; semantic PARM `0x00100004`, float range `0..1` | Same existing-record patch contract | Same rejections and four-byte write set. |
| Amp Channel Volume | PODxt branch selects `AMP_CHANVOL`; semantic PARM `0x00100005`, float range `0..1` | Same existing-record patch contract | Same rejections and four-byte write set. |

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

## Remaining editor and librarian workflows

| Workflow/source area | Current disposition | Admission still needed before editing |
| --- | --- | --- |
| Other amp models in `L6PODxtParamMap`'s model list | Parsed and labeled where known; controls stay unavailable | Per-model target applicability and control-presence/serialization contract. The supported model list is not copied into Rust and does not prove pack installation. |
| Cab models and microphone selection | Model IDs may be displayed; no numeric edit is exposed | Target-specific cab/mic definition, associated parameter ID/type/domain, model-dependent mapping, and exact preservation behavior. |
| Stomp, modulation, delay, reverb and EQ models | Parameter chunks remain raw/visible as decoded records; numeric editing unavailable | Exact PODxt Live model-to-slot and parameter mapping, float/integer/enum representation, domain/units/scaling/steps, callbacks and failure behavior. Shared AX3 names or IDs alone are insufficient. |
| Amp enable, model selection, setup slots and model replacement | No numeric/model replacement API | Boolean/association contract, optional-pack policy, replacement side effects and transaction/failure semantics. `CheckCanWritePatch`'s installed-pack check is not an edit-domain or serializer contract. |
| Tone setup, bank/collection organization and POD Farm interchange | Tone-file list/name workflow only; no bank/collection editor or POD Farm conversion | Concrete container schema, target/version gate, exact serializer, unknown-record retention and safe-copy workflow. The negative POD Farm 1.12 receiver result is scoped to that package; see [its historical investigation](pod-farm-112-target-applicability.md). |
| Send, switch, firmware, backup, audio and maintenance operations | Disabled or unimplemented | Separate future requirements and explicit authorization. No such live operation follows from the offline contracts here. |

The current Rust implementation covers six controls on one amp model. Other
controls and workflows remain rejected or unimplemented until their own source
contract and synthetic compatibility tests are admitted. This matrix is not an
issue-acceptance claim. The broader criteria for formats/models/effects,
inspection/edit/library workflow, and compatibility/preservation/rejection
testing remain partial. Lack of physical testing is not an offline criterion
gap; hardware remains locked independently.
