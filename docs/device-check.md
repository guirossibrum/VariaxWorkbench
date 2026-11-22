# Device check (staged probe)

Issues [#134](https://github.com/guirossibrum/VariaxWorkbench/issues/134),
[#137](https://github.com/guirossibrum/VariaxWorkbench/issues/137) and
[#157](https://github.com/guirossibrum/VariaxWorkbench/issues/157); parent
[#113](https://github.com/guirossibrum/VariaxWorkbench/issues/113).
**G1 is authorized by the owner's direct permission (2026-11-22) and is open in this build only.**
G2–G8 are not authorized. See [the G1 procedure](hardware-gates/g1.md).
The window still opens in **Simulation**. **This computer (G1)** is offered only
when step 1's live form is available, and only step 1 can run in that mode.

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

`ProbeBackend` has two window backends.

**Simulation** (the default) uses `SimulatedRig`:

- Stage 1 reads `SyntheticSysfs`, an in-memory copy of the sysfs/procfs
  layout through the `OsMetadataSource` trait. Choosing Simulation never
  constructs `LinuxSysfs`.
- Stages 2–4 use the existing in-memory `simulation::VirtualControl` and the
  shared `read_session::read_current_on`. Identities and metadata are
  synthetic (revision `0001`, preset name `SIMULATED`).
- The **Pretend setup** list picks a scenario: connected, nothing connected,
  two PODxt Live units, unreadable information, a silent guitar, or tuning
  left unchanged.

**This computer (G1)** uses `HostRig` around `LinuxSysfs`. It is offered only
when `live_stage_available(Stage::UsbRecognition)` is true (G1 open, and step 1
is the only live stage). The reader is built only when that mode is chosen.
It reads only the [g1.md section 1](hardware-gates/g1.md#1-what-the-app-reads)
paths under a configurable root (the app uses `/`; tests inject a temporary
fake tree) with `read_dir`, `read_to_string` and `read_link`, fills the same
in-memory map and reuses its parser. Both trait methods run their reads inside
`run_if_unlocked(Gate::OsDiscovery, …)`. In this mode steps 2–4 are locked:
they have no Yes/No buttons, and the backend refuses them without sending
anything. A passed step never starts the next one. The answer stays **No**
until the person chooses Yes.

## Tests

- Core simulation (`src/device_probe.rs`): recognised, absent, two devices,
  malformed metadata (vendor/product, interface number, card list), missing
  card list, decline, cancel, no automatic advance, full ladder, silent guitar.
  Simulation mode never holds a `LinuxSysfs`.
- Real reader (`src/device_probe.rs`, Unix tests on temporary fake trees, never
  the real `/sys` or `/proc`): PODxt Live with `snd-usb-pod` and
  `snd-usb-variax` bound through relative `driver` symlinks; absent (including
  another Line 6 product), an unbound interface, two units; malformed
  `idProduct`, non-UTF-8 `idVendor`, bad `bInterfaceNumber`, bad card list,
  missing card list, empty tree; `serial`/`descriptors` present but never
  read. `real_reader_reads_only_its_injected_root_with_g1_open` checks that
  the default root constant is `/` and then reads only an injected temporary
  tree. No test calls `LinuxSysfs::new()` and reads from it. This-computer
  mode runs step 1 only; decline and cancel run nothing; the report contains
  no serial number. G2–G7 each stay locked (`src/hardware_gates.rs`).
- GUI (`src/gui.rs`): default-No via Enter, decline button, no automatic
  advance, stop on failure and start over, all in Simulation with zero real
  readers built. The mode test checks that **This computer (G1)** is present
  only on Linux and that the step-1 indicator does not say the real reader is
  open on any other platform. On Linux, this-computer mode uses an injected
  fake tree, runs only step 1, and a decline reads nothing. Enter on the
  focused Yes button reads that tree
  (`device_check_enter_on_focused_yes_reads_the_injected_tree`). Closing the
  window in the same frame as that Yes
  (`device_check_close_in_the_same_frame_as_yes_reads_nothing`) cancels through
  the window dispatch and the injected reader records zero calls.

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

- The owner has not run the real step 1. The card for that run, after #158 is
  on `master`, is [phase 2](owner-tests/phase-2.md). Results belong on #125.
- Replay of the private 2026-11-14 identity captures (#113) is not included;
  committed fixtures stay synthetic.
- The inventory table (`supported_devices.txt` schema 2, `device_inventory.rs`)
  from `508cd00` is not part of this change.
