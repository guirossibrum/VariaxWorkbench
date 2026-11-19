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

The precise remaining selector edge is the supplied GUI widget implementation:
`L6ComboBox` and `L6ParamRouter` are not present in the class payload, so
same-choice event emission cannot be determined. The PODxt classes, model-id
arrays and association definitions are present; the unresolved question is not
source presence. No acquisition, vendor execution or hardware observation is
needed to state this limit.

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

**Admission status:** no new selection operation is admitted by this research
correction. The target-specific classes and selector maps are present, and the
direct setter behavior is now source-backed. Gatekeeper review is still needed
before implementation, including `L6AS` cab applicability and the GUI
same-choice event edge. Existing admitted operations, including AMP/EQ stored
model-enable flags, do not need renewed per-leaf scope approval.
