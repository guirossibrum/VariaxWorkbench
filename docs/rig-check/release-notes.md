# Rig Check release notes

Rig Check walks you through tests of a PODxt Live and Variax rig one
explained, confirmed step at a time and writes a text report. Catalogue:
[catalogue.md](catalogue.md) (version 1.0).

## This build (unreleased, 0.9.0, catalogue 1.0)

**What it tests**

- Practice mode (`rig-check.sh --practice`, or choose Practice in the
  window): every step of the catalogue against simulated devices. Nothing is
  sent to a real rig.
- Real rig (`--real`): this computer (0.1 system, 0.2 USB and drivers),
  the engine check (0.3) and the audio device listing (8.1). **Every device
  step is refused** with "not yet authorized by the owner (#267)" until the
  owner approves each one for other people's rigs.

**What it never does**

- It never runs a step you did not confirm. The default answer is not to run.
- It never repeats a save, never sends a store during a live step, and never
  installs firmware.
- It never opens an audio stream (steps 8.2-8.5 are "not available in this
  version").
- It never uses the network. Your user name, computer name, home folder and
  serial numbers are removed from the report.
- It writes only inside its own folder (`rig-check-output` next to the
  program, or `--out DIR`), never over an existing file.

## Planned releases (#243)

| Version | Platform | Adds |
| --- | --- | --- |
| v1 | Linux | computer, engine check, connection, read-only device steps, guided actions, report |
| v2 | Linux | live (unsaved) steps and saves of a slot's own bytes |
| v3 | Windows, macOS | after the connection research (#262), packaged with #209 |
| v4 | all | audio/DAW steps, after the owner's separate audio approval |

Each version's device steps run on other people's rigs only after the
owner records which steps, models and firmware are covered (#267).

## Building

- **Linux (portable folder):** `packaging/rig-check/build-linux.sh OUT` builds
  the release binary with the git commit, compiler and target recorded, and
  assembles `OUT/rig-check-<version>-linux-<arch>/` with `rig-check.sh`, the
  catalogue, these notes and `SHA256SUMS`. It refuses to replace an existing
  folder and does not publish anything. The folder needs no installation; the
  window uses X11 or Wayland like Variax Workbench (see
  [linux-package.md](../linux-package.md)); `--text` runs in a terminal.
- **Windows and macOS:** not built yet. They follow #209: unsigned builds
  with guided OS approval (Windows SmartScreen "More info, Run anyway";
  macOS right-click Open, or System Settings > Privacy & Security > Open
  Anyway), one portable folder per platform with the same launcher rules
  (reports beside the program). The device connection on these systems goes
  through the Line 6 driver's MIDI ports; see
  [transport-research.md](transport-research.md) (#262) for the adapter that
  must exist first. `cargo check --target x86_64-pc-windows-gnu
  --no-default-features --bin rig-check` passes today.
