# PODxt model selection: source edge and bounded contract

This is a static source trace for VariaxWorkbench issue #29. It does not add a
model selector or claim behavior for any connected POD. The current exact-head
implementation continues to edit only already-present, source-admitted fields.

## What the supplied Line6Edit source establishes

The shared macOS and Windows `L6EditorAppDeviceMap` entries map product id
`196618` (`0x0003000A`, PODxt Live) to
`modules.sharctone.device.podxt.L6PODxtDevice` and
`modules.sharctone.editor.L6SharcToneEditor`. Those classes are not present in
the supplied `analysis/decompiled/mac` or `analysis/decompiled/windows` source
trees. The class map establishes the intended product/editor binding, but does
not reveal selector choices, defaults or state changes. `IsSharcTone` includes
several other product ids, so a generic Sharc branch is not PODxt Live proof.

The available common classes expose primitives, not a PODxt selection contract:

- `L6Patch.LoadModel` stores a model in a map keyed by slot and replaces the
  existing value at that key. It returns the displaced model and only records
  that value when `CaptureDisplaced` has installed a vector. `UnloadModel`,
  `MoveModel` and `SwapModels` have additional removal/collision behavior. These
  are common data operations, not selector policy.
- `L6Model.SetID` changes only the in-memory model id. `FindParam(id, true)` can
  create an integer zero parameter; `SetParamFloat` may create a missing
  parameter when its flag is true. The generic writer serializes the current
  MINF fields with reserved bytes set to zero and emits stored parameters. It
  does not preserve opaque MINF tails or establish what a PODxt selection
  should do to model-specific parameters.
- `L6ModelParam` declares generic `MODEL_SELECT`, `SLOT_SELECT` and
  `MODEL_ENABLE` id-class constants. It supplies no target/model bindings.
  `L6ParamMap.SetModelIndex` maps an index through a caller-supplied `int[]`;
  the helper has no built-in list, and the supplied decompiled sources have no
  call site for it.
- `L6TonePatcher` performs versioned compatibility migrations; its Sharc path
  applies broadly across products. `L6ToneTranslationUtils` translates
  TonePort-family patches and can change model ids, move/unload models, clear
  parameters and seed values such as `0.5`. These paths demonstrate that
  conversions may have side effects; they are not the PODxt Live editor's
  selection behavior and their defaults must not be copied into this feature.
- `L6Patch.UpdateMetaAmpName` reads slot `0x00030000` for Sharc tones. Its
  `UpdateMetaNotesForSetup` reads cab slot `0x00030001` only for type-2
  (`L6AS`) setup patches. This is generated metadata display, not proof that a
  selector exists or that every L6AS cab id is valid. No supplied PODxt Live
  source identifies a microphone selector or microphone model slot.

Useful local source references (private originals remain outside Git):

- `analysis/decompiled/mac/line6editbase/app/L6EditorAppDeviceMap.java:19`
  (product-to-class mapping); mac and Windows copies match.
- `analysis/decompiled/mac/cmn/l6java/device/L6ProductConstants.java:39-55`
  (PODxt/Sharc product families).
- `analysis/decompiled/mac/cmn/l6java/patch/L6Patch.java:101-178,347-390`
  (generic slot map and generated amp/cab metadata).
- `analysis/decompiled/mac/cmn/l6java/patch/L6Model.java:50-57,107-148,233-247`
  (id mutation, optional parameter creation and writer behavior).
- `analysis/decompiled/mac/cmn/l6java/patch/L6ModelParam.java:14-24`
  (generic parameter id classes).
- `analysis/decompiled/mac/cmn/l6java/patch/L6ParamMap.java:184-192`
  (caller-supplied model list mapping).
- `analysis/decompiled/mac/line6editbase/tonetranslation/L6TonePatcher.java:27-70`
  and `L6ToneTranslationUtils.java:18-111` (versioned fixes and cross-family
  translation, not an editor selector contract).

The precise missing source edge is the PODxt-specific implementation and
resource data referenced by `L6PODxtDevice` and `L6SharcToneEditor`: its
selector event/handler, per-product model-id lists, and the functions or
resources that initialize, remove or retain PARMs on a choice. For cab and mic,
the corresponding target-specific selector bindings and applicability list are
also required. No acquisition, execution or hardware observation is needed for
this request; if these members are absent from the supplied originals, the
selection edge remains unresolved.

## Proposed operation contract for Gatekeeper admission

This proposes a boundary for a future offline implementation; no selection
operation is implemented by this document.

1. **Target:** only a parsed PODxt Live file with PINF target `0x0003000A` and
   supported individual-tone container `L6PA`, `L6FS` or `L6AS`. Never admit a
   product merely because it satisfies `IsSharcTone` or has a matching asset
   name.
2. **Operations:** separate typed requests for selecting an AMP model at slot
   `0x00030000` and a CAB model at slot `0x00030001`. The existing parser exposes
   the cab slot as the tone's cab only for `L6AS`; the Gatekeeper must require
   the recovered selector contract to confirm whether that is also its edit
   applicability. A microphone operation is excluded unless the missing
   PODxt-specific source explicitly identifies its control, target, slot and
   mapping; it must not be inferred from Vetta, POD HD or asset labels.
3. **Choice admission and side effects:** accept only an exact model id present
   in a recovered PODxt Live selector table. The table must also state, for each
   choice, its model-specific PARM additions, replacements, removals, defaults,
   and any other changed fields. Do not invent defaults or assume changing only
   MINF model id is equivalent. If the source does not fully describe a choice,
   reject it without mutation.
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

**Admission blocker:** the available common primitives cannot fill the
per-choice model lists or parameter side effects. Gatekeeper admission is
requested for this bounded investigation and contract, not for implementing
model selection, cab/mic selection, or model-specific PARM replacement without
the named target-specific source evidence. Existing admitted operations,
including AMP/EQ stored model-enable flags, do not need renewed per-leaf scope
approval.
