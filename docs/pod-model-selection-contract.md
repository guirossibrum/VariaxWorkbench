# PODxt model selection: source contract and bounded implementation

This is a static source trace and offline implementation record for VariaxWorkbench
issue #29. It does not claim behavior for any connected POD.

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

## Mod model selector: admitted static source contract and offline edit

Static inspection of `L6PODxtParamMap.GetModModelList` and the Mod model
catalog identifies 24 current Mod IDs. `L6SharcToneEditor.IsModelInstalled`
classifies 13 of these as Pack 4 gated; the remaining 11 IDs (`0x02030004`
through `0x0203000e`) take its default-available branch. This is source-level
eligibility evidence, not device support or installed-pack evidence. The
application therefore exposes only those 11 as typed requested choices while
allowing any of the 24 recognized current IDs to transition to an admitted
choice. Unknown current IDs remain readable but cannot be edited.

The offline setter is scoped to one existing model in FORM/L6PA for PODxt Live
(PINF layout 1, target `0x0003000A`, and the existing application profile floor
`0x03000000`). Exactly one model must occupy either primary slot `0x00020004`
or fallback slot `0x00050003`; absent, both-present, duplicate, or malformed
model/MINF structures reject. A unique MINF must contain at least 12 actual
bytes. The edit changes only its first four big-endian model-ID bytes; it
preserves all remaining MINF, PARM, enable/reserved, padding, chunk, and
unrelated bytes. It neither seeds nor resets parameters. The full clone is
reparsed before publishing. All context checks precede the same-ID no-op.

The static source supports the narrow choice admission, but the four-byte
serialization strategy is the application's conservative policy. It does not
claim full callback parity or prove vendor save behavior. A successful model
change clears staged Mod NOTE/SPEED values tied to that model context; a
rejected or same-ID Apply retains them. GUI rendering of a gated current ID is
read-only and displays it outside the choices so the user can directly select
one of the admitted IDs.

**Literal synthetic whole-buffer example.** The complete fixture below is a valid
`FORM/L6PA` tone with PINF v1 / PODxt Live / host-profile word `0x03000000`,
one Mod model in slot `0x00020004`, an opaque PARM and an unknown model child.
Applying `0x02030005` changes only the MINF ID bytes. The vector is independent
of the setter and also exercises save/reparse preservation.

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
0090: 02 03 00 04 00 02 00 04 fe 12 34 01 50 41 52 4d
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
0090: 02 03 00 05 00 02 00 04 fe 12 34 01 50 41 52 4d
00a0: 00 00 00 03 de ad be a5 58 54 52 41 00 00 00 02
00b0: 55 66
```

## CAB/MIC source trace and proposed bounded operation

This is a source-backed proposal for Gatekeeper technical admission. It adds no
Rust behavior and does not establish that any listed model is installed or
accepted by a physical unit.

**Source pin and symbols.** The source is the staged Line6 Edit 3.06.0 installer
already recorded above (SHA-256
`426ddbb1559f445bfc8d0c29c316c80fe19f9bf8ff2e71d61b065342783dbe24`). Static
class/method pins are `L6PODxtParamMap.GetCabModelList` and class initializer;
`L6SharcToneParamMap.GetCabModel`, `GetCabParam`, `GetCabParamDef`,
`IsBassCab`, `SetParam`, and `IsBassModel`; `L6SharcToneEditor.SetupAmpCabPanel`,
`SetupMicPanel`, `OnParamChange`, and `IsModelInstalled`; and
`L6AX3ParamDefs.MIC_MODEL_PODXT` / `MIC_MODEL_BASSPODXT`. Existing recorded
class digests for the provider, shared map and editor are in the source pin
section above. The supplied parameter-definition class is part of the same
private payload. All evidence is static bytecode inspection; vendor code was
not run.

**Selector and storage mapping.** `SetupAmpCabPanel` binds CAB selector
`0x10020000` to model slot `0x00030001`; the model setter calls `L6Model.SetID`
on that slot and does not seed or rewrite its PARMs. `L6PODxtParamMap` returns
one 47-id CAB list. Its first 25 ids are exactly
`0x01020000..=0x01020016`, `0x01020018`, and `0x0102001a`; the remaining 22 are
`0x01030001..=0x01030016`. The source `IsBassModel` predicate classifies the
second range as bass cabs and the first range as non-bass. When a CAB model is
missing, the original map can create it with model id `-1`, enabled `true`, and
ordinal `0` before setting the selected id. This proposal rejects missing
slots rather than copying that creation behavior.

**Eligibility and fallback.** `IsModelInstalled` sends the listed
`0x01030001..=0x01030016` CAB ids to pack index 8. CAB selector handling
substitutes `0x01020000` when that check fails. The 25 corrected unconditional ids do not have CAB cases in that switch and reach
its default-true branch. An offline file contains no authoritative pack state.
The admitted CAB-choice operation therefore accepts exactly the 25
default-true ids and rejects pack-gated CAB choices while their eligibility is
unknown. It never silently applies the source editor's pack fallback, because
the app cannot evaluate the source's installed-pack condition. `0x01020000` is
itself an explicit listed choice; that is distinct from implicit substitution.
This is an offline admission boundary, not an entitlement claim.

**CAB-dependent MIC definition.** `SetupMicPanel` registers MIC selector
`0x00020001`. In the shared map that selector belongs to the existing CAB model
and maps to CAB `PARM` id `0`; it is not a model slot. `GetCabParamDef` selects
the PODxt MIC association definition when `IsBassCab` is false and the
bass-PODxt definition when it is true. The two definitions each enumerate
integer choice ids `0..=3`, but those ids have different meanings between the
two definitions. For PODxt Live, `IsBassCab` obtains the existing CAB model and
classifies its id through `IsBassModel`. Thus the UI must derive MIC choices
from the current CAB id and refresh that interpretation after a CAB Apply. A
CAB change itself changes only the CAB model id; it leaves an existing MIC
value word untouched, so its displayed meaning can change when the cab class
changes. A separate explicit MIC Apply can then choose an id under the new
definition. Static `OnParamChange` pack fallback applies to the CAB model
selector, not to MIC selector `0x00020001`.

**Admitted Rust boundary.** Admit only FORM/L6AS, PINF layout 1, PODxt Live
target `0x0003000A`, and the established conservative profile floor
`0x03000000`. Require one existing CAB slot `0x00030001` and one complete MINF
body of at least 12 bytes. A CAB Apply accepts only
the 25 corrected unconditional ids, changes only the first four CAB MINF bytes in
big-endian order, and updates the decoded CAB id. It does not require, create,
or change MIC PARM id `0`; if present, that entire record is preserved. The UI
refreshes the MIC selector's choice definition from the new CAB id.

For MIC Apply, require an existing CAB id in the source's 47-id list, one
existing CAB `PARM` id `0`, an exact 12-byte recorded PARM with canonical
integer type `0`, and a requested id in `0..=3`. Interpret the id through the
definition selected by the existing CAB class; patch only the PARM value word
and update the decoded integer. Do not normalize the type word, create a
missing CAB/MIC record, or change the CAB model. A MIC edit on a currently
stored pack-gated bass-cab id uses the source's bass MIC definition; it does
not infer installed-pack eligibility and does not select that cab.

For either operation, validate target, container, source allowlist, unique
slot/chunk/record, actual parsed ranges and field type before comparing the
requested value. Same-value Apply is a byte/decoded/dirty no-op under the Rust
file policy, not a claim about vendor widget events. Failed validation leaves
bytes, decoded state, dirty state and baseline unchanged. Preserve CAB ordinal,
enabled/reserved fields, all MIC metadata, every unrelated PARM/chunk/pad and
outer byte. No model or parameter defaults, slot creation, fallback, GUI send or
device communication are part of this operation.

Synthetic verification scope: iterate all 25 admitted
CAB ids; test rejection of all pack-gated selector ids and non-list ids; cover
both CAB classes' `0..=3` MIC meaning without publishing vendor labels; assert
CAB Apply retains the raw MIC word while the UI derives the refreshed class;
test CAB and MIC same-value no-op, unknown existing CAB rejection for MIC,
missing/duplicate/truncated CAB MINF or MIC PARM, wrong target/container/type,
exact raw write sets, failure atomicity, library/bundle/session/GUI dirty-save-
reopen-revert and quit safeguards. These tests establish Rust behavior only.

**Admission erratum (Gatekeeper review, PR #95).** CAB selection supports only
`0x01020000..=0x01020016`, plus `0x01020018` and `0x0102001a`; holes
`0x01020017` and `0x01020019` reject. The exact `0x01030001..=0x01030016`
set is pack-gated and rejects with no fallback. MIC class derives from the
current exact CAB id. CAB changes preserve the raw MIC integer; a separate
explicit MIC Apply changes it. Gatekeeper admitted this scoped offline operation.

## Delay model selector: source trace and proposed offline boundary

This is a static, research-only contract for the next review. It does not add a
Delay model selector or authorize a writer. The source is the already-retained
Line6Edit 3.06 class payload in ignored `analysis/issue29-line6edit306-source/`;
no new source was acquired and no vendor code was executed.

**Source identity and pins.** The retained source container
`analysis/issue29-line6edit306-source/Line 6 Edit.exe` has digest
`02e46be124274617496f6aaa93ab8251b3ff60217bc08a6f3b5ed48943a112ac`. Relevant
class SHA-256 values are:

- `modules/sharctone/device/podxt/L6PODxtParamMap.class` —
  `3092741706674148563731965226cc7b56e8d1d1db55bbd1ddacf2f2bc3f4888`
- `modules/sharctone/device/L6SharcToneParamMap.class` —
  `761b59f407496ca5924f66e6fd8e3b7f728feb0771702ab9cc2484c6b2e72dcc`
- `modules/sharctone/editor/L6SharcToneEditor.class` —
  `131c4167a01d02b684f9e522bd8182424ff6af1c26bde50f544331ddcedae060`
- `cmn/l6java/patch/L6Model.class` —
  `20a965fd09e5cdcb11ef810fea2babd67eea67079d65dd7ebef38973e527ca05`
- `modules/sharctone/device/podxt/L6PODxtDevice.class` —
  `fcdff53cf943283ba43d2ccd9967b36780a3e71571e4976112de2f736ced12cc`
- `line6editbase/app/L6EditorAppDeviceMap.class` —
  `de0a93656cf05e61155971b51610bbba16454f1a3b3a7ee85554f418dfb225d8`

**Selector and storage path.** `L6SharcToneEditor.SetupDelayPanel` registers
the Delay combo under selector id `0x10050000`. `L6PODxtParamMap.GetDelayModelList`
returns 14 PODxt-family ids (not a Live-only list):
`0x02020002` through `0x0202000F`, inclusive. The shared
`L6SharcToneParamMap.CreateModelFormat` resolves their displayed labels through
`L6Assets.GetAssetName(product, id)`. Exact display strings are not independently
listed by these retained class files and are not inferred here.

`GetModelForGroup` routes the Delay group `0x00050000` to
`GetDelayModel(patch, true)`. That method calls `GetDualSlotModel` with primary
slot `0x00020005` and fallback slot `0x00050004`. The shared helper queries the
primary first and uses the fallback only when the primary is absent. If both
are absent and creation is enabled, the call constructs and loads a model with
id `-1`, primary slot `0x00020005`, `enabled=false`, and ordinal `0`. This is
source behavior; the proposed file operation below rejects a missing slot and
never copies that creation behavior.

`L6SharcToneParamMap.SetParam` recognizes model selector words by their high
byte (`0x10000000`); for an existing returned model it calls
`L6Model.SetID(requested_id)` and returns. That branch does not clear, seed,
remove, or rewrite the model's in-memory PARM vector, and it does not itself
validate the candidate against `GetDelayModelList`. The UI association list
provides the finite choice table, while `L6SharcToneEditor.OnParamChange`
performs the installed-model guard before dispatching the choice.

**Optional-pack fallback.** The editor's `IsModelInstalled` returns true by
default for Delay ids `0x02020002`–`0x0202000A`. The five ids
`0x0202000B`–`0x0202000F` are mapped to
`L6PODxtDevice.IsPackInstalled(4)`. In the pre-base portion of
`OnParamChange`, selector `0x10050000` is paired with fallback id
`0x02020002`; if `IsModelInstalled(requested_id)` is false, the editor
substitutes that fallback before calling the base handler. This is an
installation-state branch, not a model-specific parameter reset. The current
offline tone has no authenticated pack-installation state, so a Rust choice
list cannot claim those five optional-pack models are available. The
conservative proposed choices are exactly `0x02020002`–`0x0202000A`; the five
pack-gated IDs and any non-listed id reject as requested choices. This is an
offline policy inferred from the source guard, not a claim about entitlement or
physical support.

**Event side effects and parameter retention.** After the base parameter
change, the selector branch calls `UpdateAllControls()` and then
`MergeEditBuffer(send)`. Static bytecode shows control refresh and a merge
attempt, but this research did not execute either method or follow a connected
device path. No live result or GUI same-choice event behavior is claimed. The
direct model-map branch has no call to a model-default, reset, clear, or
parameter-creation method. Existing PARMs stay in the model object and will be
interpreted under the newly selected model layout. Missing expected PARMs are
not created; old PARMs are not reconciled to the new model's parameter set.

`L6Model.WriteIFFContainer` reconstructs a MINF record from model id, slot,
ordinal, two zero bytes, and enabled state, then writes stored PARM objects.
The `L6Model` fields do not retain arbitrary MINF reserved bytes or unknown
model children, so this class alone cannot establish their preservation by a
full surrounding writer. Its canonical output can differ outside the selected
id. The proposed Rust operation must edit the original parsed buffer directly,
changing only the existing MINF model-id word and retaining all other source
bytes, including unknown PARM records, model tails, sibling chunks, and pads.
Opaque PARM data need not be interpreted for that one-word operation; malformed
or opaque target MINF structure must reject. This is a proposed Rust
preservation rule, not vendor behavior.

**Admitted bounded Rust operation.** For a
first finite operation, limit writes to one existing individual PODxt Live
`FORM/L6PA` tone with PINF layout 1, target `0x0003000A`, and the established
offline editing-profile floor `0x03000000` on the PINF device-version word. The
version floor is an application profile, not a selector-source precondition.
Resolve the existing primary slot `0x00020005` or fallback slot `0x00050004`,
but require exactly one across both slots. This is a conservative application
policy, narrower than the legacy primary-precedence resolver. If neither, both,
or a duplicate exists, reject rather than creating or relocating a model. The
selected current model id must be in the full source list, and the requested
model id must be in the nine unconditional choices above. Unknown current or
requested ids, pack-gated choices, wrong target/container/profile, missing or
ambiguous model slot, truncated MINF, or non-unique MINF reject before mutation.
An opaque/unknown PARM or unrelated MODL child remains uninterpreted and
byte-preserved; it does not expand the accepted model-choice set. The target
MINF body must be at least 12 bytes; shorter bodies reject even if following
padding resembles the missing bytes. Validate all
guards before treating same-id Apply as a no-op. A same-id valid request leaves
the full buffer, decoded model, dirty state, and save baseline unchanged. Any
failed guard likewise leaves all of them unchanged.

For a valid changed request, the offline raw write set is exactly the
first four bytes of the selected existing MINF body, big-endian. It does not
alter slot id, ordinal, reserved bytes, enabled state, any PARM id/type/value,
the fallback model, or any outer IFF length. This avoids importing the source
serializer's normalization and omission behavior.

The Rust implementation is in `PodTone::set_delay_model_choice`, with typed
choice admission, library and bundle entry points, the POD-role session gate,
and an explicit per-tone GUI Apply/Discard selector. The GUI labels this as an
ID-only edit that retains stored parameters and performs no reset. Successful
apply reparses the candidate before publishing it. A successful actual model
ID change also invalidates staged Delay parameter edits; rejected or same-ID
operations preserve them. Selector interaction, discard, tone navigation,
failed apply, bundle apply, save collision, save/reopen, revert, and unsaved-quit
guard are covered offline. This does not claim that the entire legacy
control-refresh/merge callback is reproduced. Issue #29's three parent
acceptance criteria remain partial/open.

**Literal synthetic full-buffer vector.** These independent 188-byte
`FORM/L6PA` buffers contain a valid PODxt Live PINF, a primary Delay model in
slot `0x00020005`, one stored float PARM, an odd unknown top-level chunk with
pad, and an unknown MODL child. The only difference is byte offset `0x93`:
the requested source-listed id changes from `0x02020002` to `0x02020003`.
They specify the proposed raw-buffer edit, not bytes emitted by the vendor
serializer and not a captured preset.

```text
before (188 bytes):
0000: 46 4f 52 4d 00 00 00 b4 4c 36 50 41 4f 44 44 21
0010: 00 00 00 03 17 28 39 a5 50 49 4e 46 00 00 00 50
0020: 00 00 00 01 00 03 00 0a 00 41 00 00 00 00 00 00
0030: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0040: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0050: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0060: 00 00 00 00 00 00 00 00 03 00 00 00 5a a5 96 69
0070: 4c 49 53 54 00 00 00 44 50 41 54 43 4c 49 53 54
0080: 00 00 00 38 4d 4f 44 4c 4d 49 4e 46 00 00 00 0c
0090: 02 02 00 02 00 02 00 05 00 00 00 01 50 41 52 4d
00a0: 00 00 00 0c 3f 10 00 00 00 00 00 01 3f 00 00 00
00b0: 58 54 52 41 00 00 00 03 de ad be a5

after (188 bytes):
0000: 46 4f 52 4d 00 00 00 b4 4c 36 50 41 4f 44 44 21
0010: 00 00 00 03 17 28 39 a5 50 49 4e 46 00 00 00 50
0020: 00 00 00 01 00 03 00 0a 00 41 00 00 00 00 00 00
0030: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0040: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0050: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
0060: 00 00 00 00 00 00 00 00 03 00 00 00 5a a5 96 69
0070: 4c 49 53 54 00 00 00 44 50 41 54 43 4c 49 53 54
0080: 00 00 00 38 4d 4f 44 4c 4d 49 4e 46 00 00 00 0c
0090: 02 02 00 03 00 02 00 05 00 00 00 01 50 41 52 4d
00a0: 00 00 00 0c 3f 10 00 00 00 00 00 01 3f 00 00 00
00b0: 58 54 52 41 00 00 00 03 de ad be a5
```

This is static Java bytecode and a synthetic byte vector only. Offline tests
cover every admitted choice and byte-isolated full-buffer output, opaque-byte
preservation, MINF bodies of 8–11 bytes, missing/duplicate MINF and slots,
primary/fallback resolution, wrong PINF layout/target/profile/container,
unknown current/requested models, all five pack-gated requested IDs, same-ID
validation, atomic rejection, and direct-tone/library/bundle/GUI lifecycle
paths. GUI interaction additionally verifies direct selection of the first
allowed model from each of the five pack-gated current IDs, plus discard and
same-ID/rejected staging preservation. These tests establish Rust behavior
against the admitted static contract; they do not establish vendor-runtime or
hardware behavior.

## Admitted CAB/MIC operation contract

This records the admitted boundary and implemented offline operations. It does
not claim connected-device behavior.

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

**Admission status:** AMP and the corrected CAB/MIC operations are admitted. CAB
Apply is limited to existing-slot FORM/L6AS MINF IDs; MIC Apply is a separate
existing CAB PARM id 0 integer edit. The current implementation revision is
under exact-head configured validation; the issue remains partial because its
three parent acceptance criteria include broader POD format/control, library,
and compatibility work. Historical GUI same-choice event emission remains
unknown; the offline same-value policy is a byte/decoded/dirty no-op.
Existing admitted operations, including AMP/EQ stored model-enable flags, do
not need renewed per-leaf scope approval.
