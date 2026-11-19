# PODxt model selection: source edge and bounded contract

This is a static source trace for VariaxWorkbench issue #29. It does not add a
model selector or claim behavior for any connected POD. The current exact-head
implementation continues to edit only already-present, source-admitted fields.

## What the supplied Line6Edit source establishes

The earlier claim that PODxt-specific source was unavailable was incorrect:
the classes are absent from the older `analysis/decompiled/mac` and
`analysis/decompiled/windows` trees, but present in the supplied Line6Edit 3.06
class payload. The shared `L6EditorAppDeviceMap` maps product `0x0003000A`
(PODxt Live), along with other PODxt product ids, to
`modules.sharctone.device.podxt.L6PODxtDevice` and
`modules.sharctone.editor.L6SharcToneEditor`. `L6PODxtDevice` branches on the
Live id and supplies `L6PODxtParamMap(product)`; a generic Sharc-family check
alone would not establish this binding.

Static inspection of the supplied `L6PODxtParamMap`,
`L6SharcToneParamMap`, `L6SharcToneEditor`, `L6Editor`, and
`L6PatchManager` class files establishes this selector path:

- `L6PODxtParamMap.GetAmpModelList` and `GetCabModelList` return static
  PODxt-family model-id arrays (the supplied code does not define a separate
  Live-only array). The shared map turns those arrays into
  association-format definitions with names resolved by
  `L6Assets.GetAssetName(product, id)`. `SetupAmpCabPanel` registers combos for
  AMP selector id `0x10010000` and CAB selector id `0x10020000`;
  `SetupMicPanel` registers a MIC combo for id `0x00020001`.
- AMP and CAB selectors refer to slots `0x00030000` and `0x00030001`.
  `GetModelForGroup` obtains those models with creation enabled. If a slot is
  missing, `GetSingleSlotModel` creates a model with id `-1`, enabled `true`,
  ordinal `0`, then `SetParam` assigns the selected id. For an existing model,
  the model-selector branch calls `L6Model.SetID` only; it does not seed,
  remove or replace PARMs. Existing PARMs and the other in-memory model fields
  remain in place on that direct path.
- MIC selection is a parameter on the CAB model, not a separate model slot:
  selector id `0x00020001` maps to CAB `PARM` id `0`. Its association format
  depends on whether the selected cab is classified as a bass cab. The supplied
  parameter-format class defines PODxt mic choices. On a PODxt cab change the
  editor refreshes the MIC control from the current patch. This establishes the
  binding, not physical acceptance of every cab/mic pairing.
- The control router forwards changed control ids and raw values to
  `L6SharcToneEditor.OnParamChange`. Before the base handler, that method checks
  AMP, CAB and effect-model choices with `IsModelInstalled`; an unavailable
  model is replaced with a fallback id. The map writes a model id through
  `SetID`. The broader `CheckPatchForInstalledModels` path can replace several
  unavailable models, and for unavailable Mod/Delay models also clears their
  enable flags. These are pack-eligibility substitutions, not model-specific
  default PARMs.
- `L6Editor.HandleParamChange` calls `L6PatchManager.ChangeParam`; that method
  marks the patch dirty and calls the target param map. No equality guard is
  visible in this path, so an emitted same-value event is still passed to the
  setter and dirty-status path. The supplied payload omits `L6ComboBox` and
  `L6ParamRouter`, so it does not establish whether the widget emits a change
  event when the user reselects its current item.
- After AMP changes, the Sharc editor refreshes controls and invokes
  `MergeEditBuffer`; the CAB branch refreshes the room overlay and, for XT,
  refreshes the MIC control. The common merge method returns if the device is
  busy or not connected; no live operation was invoked during this static
  trace. This work establishes no physical behavior or permission to exercise
  the connected-device path.
- `L6Patch.UpdateMetaAmpName` derives the displayed amp name from slot
  `0x00030000`; `UpdateMetaNotesForSetup` reads cab slot `0x00030001` for
  type-2 (`L6AS`) setup patches. The inspected selection callback does not call
  these metadata helpers directly. This establishes the metadata inputs, not a
  selector-specific save or rename effect.

These are positive static bytecode findings, not vendor execution. Private
installer extracts and `javap` reports remain in ignored coordinator inputs and
are not published. The earlier common-class findings remain relevant to
serialization and translation, but do not support a claim that selector source
is missing. In particular, generic writer reserved-byte normalization and
cross-family translation defaults do not describe PODxt model selection.

Historical combo same-choice event emission remains unknown because
`L6ComboBox` and `L6ParamRouter` are not present. It is not a blocker for the
explicit Apply action specified below: if the selected id already matches the
stored model id, this offline file operation is a byte/decoded/dirty no-op. That
is this application's file-edit policy, not a claim about Line6Edit widget
events or runtime dirty notifications.

## Concrete AMP operation record for technical admission

This is one conservative existing-slot AMP model-id operation admitted by
Gatekeeper. Its complete allowed choice set is intentionally smaller than
either the 107-entry `AMP_MODELS` array or the separate 106-model numeric-
control set:

| Exact allowed AMP id | Static source evidence | Eligibility evidence |
| --- | --- | --- |
| `0x00020000` | `L6PODxtParamMap.GetAmpModelList`; `AMP_MODELS[0]` is integer `131072` | `L6SharcToneEditor.IsModelInstalled(int)` does not list this id in its pack-gated switch; its `default` returns `true` |
| `0x00020001` | `L6PODxtParamMap.GetAmpModelList`; `AMP_MODELS[2]` is integer `131073` | Same default-true branch in `IsModelInstalled(int)` |

For these two choices the original editor's static pack guard returns available
independently of the four `IsPackInstalled` flags it reads. This establishes
the source application's eligibility decision for those ids. It does not
establish physical device support or a hardware result. No runtime installed
boolean is accepted by this file operation, and no other array member is
implicitly admitted.

The exact source payload is pinned by class-file SHA-256:

- `modules/sharctone/device/podxt/L6PODxtParamMap.class` —
  `3092741706674148563731965226cc7b56e8d1d1db55bbd1ddacf2f2bc3f4888`
- `modules/sharctone/device/L6SharcToneParamMap.class` —
  `761b59f407496ca5924f66e6fd8e3b7f728feb0771702ab9cc2484c6b2e72dcc`
- `modules/sharctone/editor/L6SharcToneEditor.class` —
  `131c4167a01d02b684f9e522bd8182424ff6af1c26bde50f544331ddcedae060`

**Source operation path.** The PODxt device map is created with product
`0x0003000A`. `SetupAmpCabPanel` binds selector `0x10010000` to the AMP combo.
`OnParamChange` checks an emitted model id with `IsModelInstalled` and only
substitutes the fallback for ids that fail that check. `SetParam` dispatches
model-id parameters to `L6Model.SetID`. `GetModelForGroup` maps AMP group
`0x00010000` to slot `0x00030000`; it asks `GetAmpModel` to create a missing
model, and `GetSingleSlotModel` would create it with id `-1`, enabled `true`,
ordinal `0`. This proposed Rust operation deliberately rejects a missing slot
instead. On an existing model the static setter changes only its id and retains
all existing PARMs. No model-specific defaults or PARM rewrites are admitted.

**Admission preconditions and mutation.** Require one parsed `FORM/L6PA` tone,
PINF layout version word `1`, target `0x0003000A`, the existing project's
conservative device-version floor `0x03000000`, exactly one AMP slot
`0x00030000`, one `MINF` for that slot, and a complete MINF body of at least 12
bytes. The version floor is the Rust offline editing profile; the traced
selector setter itself does not read PINF and this is not claimed as a vendor
selector gate. The parser rejects duplicate slots/chunks and validates the
outer FORM length. Admit only the two ids in the table above. The current
stored model id may be any 32-bit value: requesting the same id is a no-op;
requesting either admitted id changes only the first four bytes of the selected
MINF body, big-endian, and updates the decoded `ModelSlot.model_id`. Preserve
the complete MINF remainder, slot, ordinal, enabled and reserved bytes, every
PARM (including opaque/noncanonical records), all other model data, IFF pads,
chunks and metadata. Wrong target/version/container, absent/duplicate AMP slot,
missing/duplicate/truncated MINF, unsupported choice, or any failed
precondition rejects before bytes, decoded state, dirty state or save baseline
change. The operation never creates a model or silently substitutes another
choice.

**Literal synthetic whole-buffer example.** This 178-byte `FORM/L6PA` fixture
has PINF v1 / PODxt Live / device-version word `0x03000000`, one AMP model with
id `0x00020001`, a full MINF, an opaque three-byte PARM and an unknown model
child. The only byte difference on Apply of `0x00020000` is the final byte of
the big-endian MINF model-id word at file offset `0x93` (`01` to `00`):

```text
before (178 bytes; FORM length 0x000000aa):
0000: 46 4f 52 4d 00 00 00 aa 4c 36 50 41 4a 55 4e 4b
0010: 00 00 00 03 11 22 33 a5 50 49 4e 46 00 00 00 50
0020: 00 00 00 01 00 03 00 0a 00 41 00 00 00 00 00 00
0030: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0040: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0050: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0060: 00 00 00 00 00 00 00 00 03 00 00 00 5a a5 96 69
0070: 4c 49 53 54 00 00 00 3a 50 41 54 43 4c 49 53 54
0080: 00 00 00 2e 4d 4f 44 4c 4d 49 4e 46 00 00 00 0c
0090: 00 02 00 01 00 03 00 00 fe 12 34 01 50 41 52 4d
00a0: 00 00 00 03 de ad be a5 58 54 52 41 00 00 00 02
00b0: 55 66

after (178 bytes; FORM length 0x000000aa):
0000: 46 4f 52 4d 00 00 00 aa 4c 36 50 41 4a 55 4e 4b
0010: 00 00 00 03 11 22 33 a5 50 49 4e 46 00 00 00 50
0020: 00 00 00 01 00 03 00 0a 00 41 00 00 00 00 00 00
0030: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0040: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0050: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0060: 00 00 00 00 00 00 00 00 03 00 00 00 5a a5 96 69
0070: 4c 49 53 54 00 00 00 3a 50 41 54 43 4c 49 53 54
0080: 00 00 00 2e 4d 4f 44 4c 4d 49 4e 46 00 00 00 0c
0090: 00 02 00 00 00 03 00 00 fe 12 34 01 50 41 52 4d
00a0: 00 00 00 03 de ad be a5 58 54 52 41 00 00 00 02
00b0: 55 66
```

**Technical admission decision.** Gatekeeper admitted this two-choice,
file-only AMP operation on the static source evidence above. A stronger
physical pack/device guarantee is not required for this bounded offline edit;
physical applicability remains unverified. No hardware query is needed or
permitted. The explicit Apply and same-choice no-op behavior remain this
application's file policy, not a claim about vendor combo events.

## Proposed operation contract for Gatekeeper admission

This proposes a boundary for a future offline implementation; no selection
operation is implemented by this document.

1. **Target:** only a parsed PODxt Live file with PINF target `0x0003000A` and
   supported individual-tone container `L6PA`, `L6FS` or `L6AS`. Never admit a
   product merely because it satisfies `IsSharcTone` or has a matching asset
   name.
2. **Operations:** the source establishes AMP and CAB model selectors and a MIC
   association parameter on CAB `PARM` id `0`. The current parser exposes the
   cab slot as the tone's cab only for `L6AS`; the Gatekeeper must review that
   file-container applicability against the source path before any Rust edit is
   admitted. Do not infer a separate mic model slot or broader cab applicability.
3. **Choice admission and side effects:** accept only an exact model id present
   in the supplied PODxt association list. The direct model-selection setter
   changes the model id and retains existing PARMs; it does not apply per-model
   PARM defaults. The vendor map can create an absent AMP/CAB model with id
   `-1`, enabled `true`, ordinal `0` before assigning the requested id. A
   conservative Rust edit may continue to reject an absent slot, but must not
   claim that this matches vendor creation behavior. Review model-specific
   control definitions and connected edit-buffer effects before admitting a
   Rust operation; do not invent defaults or remove PARMs.
4. **Atomicity and preservation:** prevalidate target, container, unique existing
   target slot, source-table choice, MINF/PARM structure and the complete
   source-defined change set before applying anything. Commit the set together
   or leave the bytes, decoded state, dirty state and save baseline unchanged.
   Preserve all unrelated chunks and bytes, unrelated models/PARMs, ordinals,
   enable state and reserved fields unless the recovered selection code
   explicitly changes a field and that side effect is admitted.
5. **Rejection and scope:** reject wrong product/container, absent or ambiguous
   target slot, unsupported choice, malformed or duplicate relevant fields,
   and incomplete source mappings. No model/slot creation or deletion, bank
   membership changes, reset/send, device communication or audio claim is part
   of this contract. Any required model creation/deletion or change to other
   slots must be returned to Gatekeeper as a separate bounded scope request.
6. **Verification before delivery:** synthetic tests cover every admitted
   source-table choice, expected default/PARM words, unrelated-byte retention,
   unknown-field preservation, wrong-target and malformed/duplicate rejection,
   same-choice no-op, and byte/decoded/dirty failure atomicity through tone,
   library, bundle, session and GUI paths as applicable. A source-backed
   contract must precede these tests; synthetic fixtures cannot establish
   vendor applicability.

**Admission status:** the concrete two-choice AMP record above is admitted and
implemented in the current PR #95 branch for existing-slot FORM/L6PA MINF model
IDs only. Its exact-head configured checks and independent review remain
pending. The separate CAB/MIC operations and broader AMP choices remain out of
scope until their own contracts are reviewed. Historical GUI same-choice event
emission is unknown, but does not block this explicit Apply/file-diff policy.
Existing admitted operations, including AMP/EQ stored model-enable flags, do
not need renewed per-leaf scope approval.
