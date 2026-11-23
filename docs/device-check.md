# Device check (staged probe)

Issues [#134](https://github.com/guirossibrum/VariaxWorkbench/issues/134),
[#137](https://github.com/guirossibrum/VariaxWorkbench/issues/137) and
[#157](https://github.com/guirossibrum/VariaxWorkbench/issues/157); parent
[#113](https://github.com/guirossibrum/VariaxWorkbench/issues/113).
**G1 is authorized by the owner's direct permission (2026-11-22) and is open in this build.**
**G2 is authorized for any user of the same supported rig** as one Variax 600
identity request through the PODxt Live, recorded in [the G2 ledger](hardware-gates/g2.md).
Device-check step 3 reads the Variax current settings and the PODxt Live
current tone. Step 4 stays locked. Device check on This computer also offers a
Variax library backup and a PODxt Live library backup. The Variax backup
changes presets while it reads, then tries to switch back. The PODxt Live
backup does not change the current tone. Nothing is written back to either
device. G4 and G6–G8 are not authorized.
See [the G1 procedure](hardware-gates/g1.md).
The window still opens in **Simulation**. **This computer** is offered only
when a live step is available. Its label, banner, and report name the gates
this build can actually run. Steps 2 and 3 are offered on Linux only in a binary
built with the `usb` feature, and step 2 only after a fresh step 1 pass. A build
without `usb` still offers only step 1. The corner shows build 16. On This
computer, under the steps, Back up the Variax and Back up the PODxt Live each
ask first. No is the default. If the guitar does not answer the first Variax
question, that backup asks again on the same connection, and if it is still
quiet it gives the driver back and tries the connection once more.

## Stages

| Step | Name | Gate | What the live form would do |
| --- | --- | --- | --- |
| 1 | USB recognition | G1 | Read what Linux already holds (USB ids, cached descriptor fields, bound drivers, ALSA card list). Sends nothing. |
| 2 | Device identification | G2 | One Variax 600 name question over interface 1. An incomplete reply or timeout stops the check. The PODxt Live is not asked in this step. |
| 3 | Settings read | G3 | Variax current settings (session on, two blocks, session off) and the PODxt Live current tone. Partial current settings, not a backup. Step 4 stays off. |
| 4 | Change check | G3 | Stays locked. The person would change `string1_tuning` on the guitar and the app would read again. This build does not offer it. |

The exact action and risk text for each step is `Stage::action` / `Stage::risk`
in `src/device_probe.rs`; the window shows them verbatim before asking.

## Rules (`ProbeSession`)

- The default answer is **No** (`Consent::default()`); in the window the
  focused default button is **No, stop**.
- Step 2's Yes button says **Yes, send one Variax identity request** on a
  real computer, and **Yes, rehearse one Variax identity request
  (simulation)** in Simulation. The rehearsal does not claim a live send.
- Only **Yes** runs the waiting step. No, or closing the window before the
  step starts, stops the sequence and sends nothing. Close in the same
  moment as Yes wins. Once Yes has started, the exchange runs on the window
  thread and the window cannot cancel it.
- Step 2 is offered only after this session's step 1 has passed for one
  PODxt Live with the POD driver on interface 0 and the Variax driver on
  interface 1. `snd-usb-pod` and `snd_usb_pod` are the same POD driver.
  `snd-usb-variax` and `snd_usb_variax` are the same Variax driver. A
  swapped pair, or any other driver, stops step 1 and does not offer step 2.
  A new Device check starts at step 1 again.
- After step 2 has been answered, **Start over** does not discard the check
  until the steps 1–2 report is saved. **Save the step 2 report, then run
  step 1 again** writes that report first and starts a fresh step 1 only
  when the save succeeds.
- A passed step makes the next step *wait*; it never runs it.
- The detailed report (schema 1) is plain text with fixed keys (`stage`,
  `gate`, `decision`, `status`, `detail`). The window labels it for issue
  #125. It carries no serial number, preset name or path.
- The steps 1–2 report (schema 2) is a second text, built only from fixed
  tokens (`src/probe_report.rs`). Identity tokens come from the product and
  version the check decoded. The gate is a typed field, not a free-form
  label. Saving it creates a new file and refuses to overwrite. A save
  failure does not run a stage again.

## Backends

`ProbeBackend` has two window backends.

**Simulation** (the default) uses `SimulatedRig`:

- Stage 1 reads `SyntheticSysfs`, an in-memory copy of the sysfs/procfs
  layout through the `OsMetadataSource` trait. Choosing Simulation never
  constructs `LinuxSysfs`.
- Stage 2 uses an in-memory identity script (`src/identity_stage.rs`). The
  success script is given one Variax 600 reply. That is simulated behavior.
  Stages 3–4 still use `simulation::VirtualControl` and
  `read_session::read_current_on` (revision `0001`, preset name `SIMULATED`).
- The **Pretend setup** list picks a scenario: connected, nothing connected,
  two PODxt Live units, unreadable information, a silent guitar, or tuning
  left unchanged.

**This computer** uses `HostRig` around `LinuxSysfs`. It is offered only
when `live_stage_available(Stage::UsbRecognition)` is true. The mode label,
the banner, the completion sentence, and the Reports note come from
`open_gates()`. On Linux a normal build includes `usb`, so that is G1
and G2. Without `usb` it is G1 only, and the window says step 1 finished
and that gate G2 stays unoffered. The `gui` feature alone does not include
`usb`. The reader is built only when that mode is chosen.
It reads only the [g1.md section 1](hardware-gates/g1.md#1-what-the-app-reads)
paths under a configurable root (the app uses `/`; tests inject a temporary
fake tree) with `read_dir`, `read_to_string` and `read_link`, fills the same
in-memory map and reuses its parser. Both trait methods run their reads inside
`run_if_unlocked(Gate::OsDiscovery, …)`. Step 2 is offered only after a fresh
expected driver binding, and only when G2 is live in that binary. Steps 3 and
4 stay locked. A passed step never starts the next one. The answer stays
**No** until the person chooses Yes. Test builds do not open USB.

## Tests

- Core simulation (`src/device_probe.rs`): recognised, absent, two devices,
  malformed metadata (vendor/product, interface number, card list), missing
  card list, decline, cancel, no automatic advance, full ladder, silent guitar.
  Simulation mode never holds a `LinuxSysfs`.
- Real reader (`src/device_probe.rs`, Unix tests on temporary fake trees, never
  the real `/sys` or `/proc`): PODxt Live with `snd-usb-pod` and
  `snd-usb-variax` bound through relative `driver` symlinks, and the same
  tree with `snd_usb_pod` on interface 0 and `snd_usb_variax` on interface 1.
  A swapped underscore pair does not pass. Also covered: absent (including
  another Line 6 product), an unbound interface, two units; malformed
  `idProduct`, non-UTF-8 `idVendor`, bad `bInterfaceNumber`, bad card list,
  missing card list, empty tree; `serial`/`descriptors` present but never
  read. `real_reader_reads_only_its_injected_root_with_g1_open` checks that
  the default root constant is `/` and then reads only an injected temporary
  tree. No test calls `LinuxSysfs::new()` and reads from it. This-computer
  mode runs step 1 only; decline and cancel run nothing; the report contains
  no serial number. G3–G7 stay locked (`src/hardware_gates.rs`). These
  step-1 tests do not call the identity path.
- GUI (`src/gui.rs`): default-No via Enter, decline button, no automatic
  advance, stop on failure and start over, all in Simulation with zero real
  readers built. The mode test checks that **This computer** is present
  only on Linux and that the step-1 indicator does not say the real reader is
  open on any other platform. With `usb`, the label names G1 and G2. On Linux, this-computer mode uses an injected
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

## Owner validation and scope

- The owner reported a step 1 pass on a v0.09 Linux development build.
  The exact revision and behavior at plug-in were unrecorded. Sound remained
  normal and the owner observed no display or light change after Yes.
  The [Phase 2 test card](owner-tests/phase-2.md) specifies a master build.
  Results belong on #125.
- Stage 2 in Simulation feeds one published Variax 600 3.10 reply to one
  identity request, then keeps listening until the three-second window ends.
  A second reply that arrives after the first does not pass. That proves the
  simulation's decision. The 2026-11-14 captures show the same request and
  reply, then a settings read that this stage does not send. A PODxt Live
  3.01 reply is recorded and does not pass. The capture files stay private.
  See [the G2 ledger](hardware-gates/g2.md). The live identity path does not
  use the settings-read entry. On Linux, a binary with the `usb` feature
  names gates G1 and G2 (`gates-open=G1,G2,G3`) and offers step 2 after the
  expected driver binding. A binary without `usb` stays on `gates-open=G1`
  and does not offer step 2. The user runs the live check. Agents do not.
  The app does not ask for a password. Yes on the step is the risk
  acceptance. If Linux refuses the open, the question is not sent and the
  driver is not detached. The live read asks for 16 bytes at a time, matching
  the read that already returned this guitar's identity. It sends that one
  question and does not ask again.
- The inventory table (`supported_devices.txt` schema 2, `device_inventory.rs`)
  from `508cd00` is not part of this change.
- Issue #114 stays open. The owner granted a one-rig exception for this
  identity attempt only. This checkout tracks both `supported_devices.txt`
  (a name list) and `supported-devices.txt` (USB and Line 6 id rows). Neither
  file was edited. `src/usb.rs` returns before USB enumeration in every test
  build. The normal app can open USB when the user chooses Yes on step 2.
  One versioned inventory is still unmet. Gates after G2 stay locked.
