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
backup reads stored tones and setups by memory location while the current tone
remains selected. Both backups leave device storage untouched.
See [the G1 procedure](hardware-gates/g1.md).
The window opens on **This computer**. The banner and report name the gates
available in the build. The window scrolls if its contents exceed the
available height. Steps 2 and 3 require Linux and the `usb` feature. Step 2
also needs a fresh step 1 pass. A Linux build without `usb` offers step 1.
Each step asks first, with **No** as the default. The two backup actions
also ask before running. If the guitar does not answer the first Variax
backup question, that backup asks again on the same connection. If it is
still quiet, it gives the driver back and tries the connection once more.

## Variax library backup

The sweep follows Workbench 1.75 Save Backup Bundle
([#207](https://github.com/guirossibrum/VariaxWorkbench/issues/207)).
After identifying the guitar, the app enables PODxt Live Variax messages
and starts the guitar MIDI session. It reads the current preset, then visits
positions 1–5 in each of the 12 banks. Each program change is followed
immediately by the two `0x3C` metadata reads (32 words at 851 and 883,
up to 2,500 ms per reply). After position 5, it requests the selected
bank name with `0x6D` (up to 1,000 ms), without another program change.

The first failed read ends the sweep. If exactly one captured preset matches
the original, the app reselects it and checks the settings. The app closes
the MIDI session and attempts the PODxt Live release even after an error.
Release failures are reported separately from the sweep. DSP processing
stays on throughout.

`index.txt` lists the names read from the guitar. `bank-names/00.syx`
through `bank-names/11.syx` hold their raw replies and are covered by
`MANIFEST.sha256`. The 120 preset replies retain their existing filenames.
Older backups load with no bank names. `variax-probe compare` compares the
preset replies across both formats.

The offline Java recorder exercised Workbench's `RequestBundle` with 120
synthetic metadata replies and 12 synthetic name replies. Its MIDI transcript
matched the Rust fake transport for the 192 sweep messages, session-on
message and two bridge messages. The recorder checks the timeouts and
session pause. Run it with the local original Workbench classes, then pass
the resulting transcript to the ignored Rust comparison test:

```sh
oracle_dir=$(mktemp -d)
"$jdk/bin/javac" -cp "$original" -d "$oracle_dir" \
  scripts/java-host-stubs/cmn/l6java/utils/L6StaticUtils.java \
  scripts/VerifyVariaxWrite.java scripts/VerifyVariaxBackup.java
"$jdk/bin/java" -cp "$oracle_dir:$original" VerifyVariaxBackup > "$oracle_dir/transcript.txt"
VARIAX_BACKUP_REFERENCE="$oracle_dir/transcript.txt" \
  cargo test --offline --manifest-path crates/line6-devices/Cargo.toml \
  --all-features original_backup_transcript -- --ignored
```

Here `jdk` is the local JDK directory and `original` is the directory of
extracted original classes. The [owner test card](variax-backup-test-card.md)
covers the physical rig.

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
- Step 2's Yes button says **Yes, send one Variax identity request**.
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

`HostRig` uses `LinuxSysfs` for step 1. The app reads only the
[G1 paths](hardware-gates/g1.md#1-what-the-app-reads). Tests inject a
temporary root for those paths. Both metadata reads use
`run_if_unlocked(Gate::OsDiscovery, …)`. The label, banner and Reports
note describe which steps the build can run.

Core and headless tests can inject `SimulatedRig`. It uses
`SyntheticSysfs` for step 1, a scripted transport for identity, and
`simulation::VirtualControl` for settings and change reads.

## Tests

- Core simulation (`src/device_probe.rs`): recognised, absent, two devices,
  malformed metadata (vendor/product, interface number, card list), missing
  card list, decline, cancel, no automatic advance, full ladder, silent guitar.
  The simulated test backend holds a `SyntheticSysfs` fixture.
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
  tree. These step-1 tests leave the identity path untouched. Decline and
  cancel run nothing, and the report omits the serial number.
- GUI (`src/gui.rs`): opens directly on **This computer**. Headless tests
  cover No as the default for each live step, decline, start over, access to
  the report on short viewports, and close taking precedence over Yes. The
  host-backend tests read only injected temporary trees. Tests that need
  identity or settings replies use a scripted rig.

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
- The scripted stage 2 test feeds one published Variax 600 3.10 reply to one
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
