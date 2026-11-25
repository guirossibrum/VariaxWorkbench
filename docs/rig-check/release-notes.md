# Rig Check release notes

Rig Check walks you through tests of a PODxt Live and Variax rig one
explained, confirmed step at a time and writes a text report. Catalogue:
[catalogue](catalogue.md) (version 1.2) and [device-message coverage](coverage.md).

## Owner release (0.9.0, catalogue 1.2)

- Practice exercises the steps on simulated devices.
- Real rig mode is released for the owner's first PODxt Live and Variax
  run. Every device and guided step has its own Run confirmation. The
  start notice, save notice, and unverified Variax firmware notice remain.
- Reads and guided checks precede live edits and stores. All applicable
  earlier steps must pass before a save is offered, and every save requires
  its exact typed confirmation. A silent save waits two minutes following
  the protocol cleanup and ends with USB release.
- Audio streams (8.2–8.5) remain closed. PODxt step 2.7 compares the
  identity captured in 2.1 offline. The device crate's separate addressed
  evidence sweep remains closed pending owner approval. A physical action
  for Variax CC 84 is pending.
- Real runs capture evidence automatically. In practice, use `--evidence`
  to save the verified backups used by the write checks.
- Start with the preset and channel you are playing. Rig Check reads the
  PODxt's current channel and compares the playing tone with the saved tone.
  It identifies the Variax preset by matching the captured working program
  to the saved programs. If there is no match or more than one, it asks you
  to move the guitar selector away and back. Unsaved edits appear in the
  report and do not block later steps on their own. Step 6.2 restores and
  verifies the captured starting buffers.
- The output trim check raises the level near its minimum. The
  alternate-tuning check changes the low string even when alternate tuning
  is already on, and skips acoustic and sitar presets.
- Rig Check uses the built-in Variax model library for the engine, audition,
  trim, and tuning checks. Use `--engine-folder DIR` to select your own
  manifest and reference models. If the selected model data is missing,
  0.3, 4.3–4.5 and 5.3 skip as not applicable. Other checks and saves
  remain available when their own requirements are met.
- The report is local and redacted. A passing run is the owner's approval
  to test the corresponding writes in the app. Shared releases need a
  separate owner decision.

## Building

- **Linux (portable folder):** `packaging/rig-check/build-linux.sh OUT` builds
  the release binary with the git commit, compiler and target recorded, and
  assembles `OUT/rig-check-<version>-linux-<arch>/` with `rig-check.sh`, the
  catalogue, coverage, these notes and `SHA256SUMS`. It refuses to replace an existing
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
