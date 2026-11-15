# Legacy electric preset edit domains

`src/edit.rs` defines explicit edits shared by `PresetFile::set_parameter`, the
CLI and GUI. It implements the original Workbench 1.75 UI domains for legacy
electric devices `0x70000`/`0x70004`. Opening, copying and renaming a file do not
normalize existing values. Unknown chunks, fields, types, model IDs and dormant
settings remain lossless. Explicit edits of unknown parameters/models are refused.
Failures occur before buffer mutation; grouped document edits are transactional.

Recovered from `L6VariaxParamDefs.Set`, `L6VariaxParamFormats.Set`,
`L6VariaxParamMap` and `L6VariaxEditor.UpdateBodyImage`, `UpdateTuningPanel`,
`UpdateToneKnobPanel` and the controls' parent panels. Static `javap -c -p`
checks of the original classes agree with the local decompilation. Original JAR
SHA-256: `053b1a9c469472d4733711a102a3bddb9f4a105d1cdd70c3936a5885139f16ba`.

| File slot / parameter | Stored edit domain | Units |
| --- | --- | --- |
| `02010001`/`02010002`, `200000` | float 0.1..9.5 | inches, virtual position |
| Same, `200001` | float -45..45 | degrees; Java converts to native distance |
| Same, `200003` | float -18..6 | dB |
| `02010002`, `200002` / `200004` | integer 0 or 1 | phase reverse / series |
| `02010004`, `200000` / `200002` | 25, 50, 100, 250, 470, 500, 1000 | float kΩ, volume/tone resistance |
| Same, `200001` | integer 0 or 1 | linear/audio volume taper |
| Same, `200003` | 0.0011, 0.0022, 0.0047, 0.011, 0.022, 0.047, 0.1, -0.047 | float µF; negative value is the original Filter choice |
| Same, `200004` | float 0..1 | saved tone fraction |
| Same, `200005` | integer 0 or 1 | save tone position |
| `02010000`, `200000` | float -20..6 | output trim dB |
| `02010005`, `200000`..`200005` | integer -12..12 | semitones, low E to high E |
| Same, `200006`..`20000b` | float -100..100 | cents, twelve-string detune |
| Same, `20000c`..`200011` | float 0..1 | twelve-string mix fraction |
| Same, `200012` | float 0..25 | stored spread; original percent formatter does not establish physical units |

All bounds are inclusive. New floats must be finite. Discrete new float edits
must equal a canonical float32 choice; imported 24-bit-truncated representations
are retained until an explicit selection replaces them. The original association
formatter can display the nearest choice; that display behavior is not evidence
that arbitrary electrical values are selectable UI choices.

Model selection exposes the 28 body IDs `03100000`..`0310001b` and 32 virtual
pickup IDs `03110000`..`03110015`, `03118000`..`03118009` from the recovered UI
catalog. Additional native pickup definitions and internal negative IDs support
the coefficient engine; they are not user-selectable models. Only pickup 1,
pickup 2 and alternate tuning have editable enable flags.

Acoustic bodies `03100012`..`03100017`, `0310001a`, `0310001b` disable virtual
pickup selection/enables/parameters and electrical controls. Output trim remains
editable: its original panel is separate. Sitar body `03100018` disables tuning.
Detune, mix and spread require a twelve-string body: `0310000d`, `03100013` or
`03100015`. This follows the original legacy default with `ENABLE_ALL_12STR`
disabled; no developer-switch or Tyler tuning-override behavior is inferred.
Tone position requires the saved-tone flag to be enabled. Body changes retain
dormant pickup/electrical/tuning data instead of clearing it. Partial imported
documents without a body can still edit known existing scalar fields; no body
restriction is inferred when the body is absent. These rules do not certify an
unknown imported model for coefficient generation or hardware use.

Virtual R/C/pole/coil model-definition resources are not additional preset UI
fields. No independent pickup-height setting was found; the guitar uses piezo
sensing. See [pickup coverage](pickup-coverage.md).

The GUI uses edit-only clamping and leaves imported out-of-range values intact
when rendered. Nonfinite slider values are displayed as stored data with an
explicit reset action; they are not passed to an egui slider, which otherwise
changed a NaN position during the first regression run. Drag values also avoid
clamping on display. Body restrictions disable editing; GUI rendering creates no
document history. Twelve-string detune/mix/spread use the same validation path.

## Offline verification

`tests/edit_domains.rs` covers all 34 known scalar parameters: accepted endpoints
and all electrical choices, adjacent out-of-range floats, NaN/infinities, invalid
integers and type mismatches. It checks rejected edits preserve bytes and undo/
redo state, dormant-data restoration across body changes, invalid model choices,
CLI failures before output creation and all 1,792 body/pickup/slot selections.
Additional tests preserve imported unknown types/parameters and nonfinite values;
a headless egui regression renders every tab without changing imported bytes.

`python scripts/verify_edit_domains.py` obtains domains from the original Java
definitions at runtime, rather than copying Rust's table. It edits only a
committed synthetic preset with separate electric and twelve-string seeds. All
**84 endpoint/choice cases across 34 parameters** pass the original Java reader;
Java reserialization and Rust rereading retain identical summaries. Reports
record JAR, input, helper, driver and Rust executable hashes in ignored
`analysis/reports/edit-domains-*.json`. This helper constructs the parameter map
and file reader only; no editor, native library or device API is opened.

These checks establish supported edit/file behavior. They do not exhaust
coefficient combinations, test sustain/bending/tuning audio, prove equivalence
to physical piezo sensing, establish firmware compatibility or authorize device
traffic. Broader generator parity remains #6 and synthetic signal coverage #33.
