# Device check (staged probe)

Issues [#134](https://github.com/guirossibrum/VariaxWorkbench/issues/134),
[#137](https://github.com/guirossibrum/VariaxWorkbench/issues/137) and
[#157](https://github.com/guirossibrum/VariaxWorkbench/issues/157); parent
[#113](https://github.com/guirossibrum/VariaxWorkbench/issues/113).
**Simulation only.** No gate is authorized; see
[the G1 procedure](hardware-gates/g1.md).

## Stages

| Step | Name | Gate | What the live form would do |
| --- | --- | --- | --- |
| 1 | USB recognition | G1 | Read what Linux already holds (USB ids, cached descriptor fields, bound drivers, ALSA card list). Sends nothing. |
| 2 | Device identification | G2 | One identity request over PODxt Live interface 1 (temporary detach/reattach). |
| 3 | Settings read | G3 | MIDI enable, two metadata requests, MIDI disable. Partial current settings, not a backup. |
| 4 | Change check | G3 | The person changes `string1_tuning` on the guitar; the same read runs again. |

The exact action and risk text for each step is `Stage::action` / `Stage::risk`
in `src/device_probe.rs`; the window shows them verbatim before asking.

## Rules (`ProbeSession`)

- The default answer is **No** (`Consent::default()`); in the window the
  focused default button is **No, stop**.
- Only **Yes** runs the waiting step. No, cancel (closing the window), a
  failure, no device, several devices or unreadable metadata stop the whole
  sequence.
- A passed step makes the next step *wait*; it never runs it.
- The report is plain text with fixed keys (`stage`, `gate`, `decision`,
  `status`, `detail`). It carries no serial number, preset name or path.

## Backends

`ProbeBackend` has one implementation, `SimulatedRig`:

- Stage 1 reads `SyntheticSysfs`, an in-memory copy of the sysfs/procfs
  layout through the `OsMetadataSource` trait. No real adapter exists
  (`LIVE_BACKEND_AVAILABLE = false`).
- Stages 2–4 use the existing in-memory `simulation::VirtualControl` and the
  shared `read_session::read_current_on`. Identities and metadata are
  synthetic (revision `0001`, preset name `SIMULATED`).

The window's **Pretend setup** list picks a scenario: connected, nothing
connected, two PODxt Live units, unreadable information, a silent guitar, or
tuning left unchanged.

## Tests

- Core (`src/device_probe.rs`, 8 tests): recognised, absent, two devices,
  malformed metadata (vendor/product, interface number, card list), missing
  card list, decline, cancel, no automatic advance, full ladder, silent guitar.
- GUI (`src/gui.rs`, 4 headless tests): default-No via Enter, decline button,
  no automatic advance, stop on failure and start over.

## Decision on `preview/device-check-visual` (#137)

**Integrated into the main GUI.** The prototype crate from the preservation
commit `508cd00` (branch `grok/issue-93-device-probe`, its own 2,985-line
lockfile) is replaced by the **Device check…** window in the main app. Its
layout ideas were kept: steps listed down the side, the risk text shown before
the buttons, **No, stop** first. The crate was not brought to `master`; there
is no `preview/` directory and no second Cargo package or workspace member,
and no build output is committed. The preservation branch remains as history
only.

## Not done here

- The real `OsMetadataSource` adapter and the G1-only lock change belong to
  [#158](https://github.com/guirossibrum/VariaxWorkbench/issues/158), after the
  owner signs the G1 form.
- Replay of the private 2026-11-14 identity captures (#113) is not included;
  committed fixtures stay synthetic.
- The inventory table (`supported_devices.txt` schema 2, `device_inventory.rs`)
  from `508cd00` is not part of this change.
