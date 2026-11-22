# GitHub work backlog

Repository: https://github.com/guirossibrum/VariaxWorkbench. Default branch: `master`.

Project: https://github.com/users/guirossibrum/projects/17. All 34 issues are on
the private project with an explicit Order and Phase. Their 44 dependencies are
native GitHub blocked-by relationships, not only issue-body links.

Work sequentially in project Order, respecting dependencies and the authorization phase.
Project Order is a topological ordering; issue numbers are identifiers, not execution order.
Issues #1 and #2 are complete; #3 is In Progress. Consult the GitHub project for current status.

## Current plan (delivery reset, 2026-11-20) — read this first

The owner-approved delivery reset replaced the dependency chain below for
hardware gates. Live work is split into operation gates G1–G8 and delivered in
phases of increasing risk, each ending with an owner hands-on test and an
owner-only exit review:

| Phase | Scope | Gate | Exit review |
| --- | --- | --- | --- |
| 1 | Clean-up and landing | — | [#148](https://github.com/guirossibrum/VariaxWorkbench/issues/148) — signed off 2026-11-22 |
| 2 | USB recognition | G1 [#125](https://github.com/guirossibrum/VariaxWorkbench/issues/125) | [#149](https://github.com/guirossibrum/VariaxWorkbench/issues/149) |
| 3 | Identification | G2 [#126](https://github.com/guirossibrum/VariaxWorkbench/issues/126) | GitHub project |
| 4 | Settings read | G3 [#127](https://github.com/guirossibrum/VariaxWorkbench/issues/127) | GitHub project |
| 5 | Sound in the app | G4 [#128](https://github.com/guirossibrum/VariaxWorkbench/issues/128) | GitHub project |
| 6 | Backup | G5 [#23](https://github.com/guirossibrum/VariaxWorkbench/issues/23) | GitHub project |
| 7 | Upload | G6 [#25](https://github.com/guirossibrum/VariaxWorkbench/issues/25), G7 [#22](https://github.com/guirossibrum/VariaxWorkbench/issues/22) | GitHub project |
| 8 | Firmware | G8 [#30](https://github.com/guirossibrum/VariaxWorkbench/issues/30) | GitHub project |

Phase 1 was signed off on the owner's hands-on offline test; its unfinished
items moved to the *Phase 1 carry-over (non-blocking)* milestone, except
[#122](https://github.com/guirossibrum/VariaxWorkbench/issues/122) and [#130](https://github.com/guirossibrum/VariaxWorkbench/issues/130), which are now Phase 2 safety prerequisites.
The editor work the Device check window builds on is [#166](https://github.com/guirossibrum/VariaxWorkbench/issues/166).

Phase 2 deliverables: [#131](https://github.com/guirossibrum/VariaxWorkbench/issues/131)
G1 procedure and form, [#134](https://github.com/guirossibrum/VariaxWorkbench/issues/134)
probe stage 1, [#137](https://github.com/guirossibrum/VariaxWorkbench/issues/137)
prototype decision, [#157](https://github.com/guirossibrum/VariaxWorkbench/issues/157)
Device check window, [#158](https://github.com/guirossibrum/VariaxWorkbench/issues/158)
G1-only test build, then #125 and #149. Owner rule (2026-11-22): gate
authorization is the owner's direct permission to an agent, recorded by the
agent; nobody signs in GitHub issues. G1 was authorized that way on 2026-11-22.

**#5, #6, #33 and #22 are not prerequisites for G1–G3.** #5's known
coefficient difference is recorded and no longer blocks reads; #6 and #33 are
sound/coefficient validation needed before audio claims and writes; #22 is now
G7 (store/restore). G1–G3 depend on the [#112](https://github.com/guirossibrum/VariaxWorkbench/issues/112)
ledger and [#110](https://github.com/guirossibrum/VariaxWorkbench/issues/110),
not on #5, #6 or #33. No gate is authorized. GitHub issues and the project
remain the source of truth; the list below is the original backlog, kept for
its history.

All issues remain subject to [AGENTS.md](../AGENTS.md). Live USB/MIDI access
is locked; creating an issue does not authorize its hardware steps.

## Offline implementation and research

- [x] [#1: Port complete body schema and interpolation to Rust](https://github.com/guirossibrum/VariaxWorkbench/issues/1)
- [x] [#2: Port pickup response and electronics calculations to Rust](https://github.com/guirossibrum/VariaxWorkbench/issues/2)
- [ ] [#3: Port remaining DSP blocks, tuning and fixed-point conversions](https://github.com/guirossibrum/VariaxWorkbench/issues/3)
  Depends on #1, #2.
- [ ] [#4: Reproduce Java preset conversion, model adjustments and final patch assembly](https://github.com/guirossibrum/VariaxWorkbench/issues/4)
  Depends on #3.
- [ ] [#5: Validate native oracle math and historical runtime rounding](https://github.com/guirossibrum/VariaxWorkbench/issues/5)
- [ ] [#6: Build complete Rust/native differential validation matrix](https://github.com/guirossibrum/VariaxWorkbench/issues/6)
  Depends on #4, #5.
- [ ] [#7: Validate model-specific parameter ranges and combinations](https://github.com/guirossibrum/VariaxWorkbench/issues/7)
- [ ] [#8: Complete bank/collection editing and GUI bank workflow](https://github.com/guirossibrum/VariaxWorkbench/issues/8)
- [ ] [#9: Complete offline GUI interaction and dirty-document tests](https://github.com/guirossibrum/VariaxWorkbench/issues/9)
- [ ] [#10: Make local extraction, tool setup and oracle generation reproducible](https://github.com/guirossibrum/VariaxWorkbench/issues/10)
- [ ] [#11: Package the Rust offline editor for Linux](https://github.com/guirossibrum/VariaxWorkbench/issues/11)
  Depends on #10.
- [ ] [#12: Finish offline updater acquisition and artifact provenance records](https://github.com/guirossibrum/VariaxWorkbench/issues/12)
- [ ] [#13: Map firmware architectures, regions and integrity mechanisms](https://github.com/guirossibrum/VariaxWorkbench/issues/13)
- [ ] [#14: Recover factory preset and calibration data layouts](https://github.com/guirossibrum/VariaxWorkbench/issues/14)
  Depends on #13.
- [ ] [#15: Establish versioned factory reference and compare saved settings](https://github.com/guirossibrum/VariaxWorkbench/issues/15)
  Depends on #14.
- [ ] [#16: Reverse engineer update and recovery protocols offline](https://github.com/guirossibrum/VariaxWorkbench/issues/16)
  Depends on #13.
- [ ] [#17: Implement hardened Rust firmware inspection and rejection tests](https://github.com/guirossibrum/VariaxWorkbench/issues/17)
- [ ] [#18: Build offline receiver, update and recovery simulations](https://github.com/guirossibrum/VariaxWorkbench/issues/18)
  Depends on #16, #17.
- [ ] [#19: Assess firmware improvement feasibility without flashing](https://github.com/guirossibrum/VariaxWorkbench/issues/19)
  Depends on #13, #16.
- [ ] [#20: Determine PODxt Live USB firmware version requirements](https://github.com/guirossibrum/VariaxWorkbench/issues/20)
- [ ] [#21: Investigate Linux Variax identity matching and POD bridge behavior](https://github.com/guirossibrum/VariaxWorkbench/issues/21)
- [ ] [#22: Complete offline readiness ledger and hardware-operation review gate](https://github.com/guirossibrum/VariaxWorkbench/issues/22)
  Depends on #6, #18, #21, #33. Now gate G7 (store/restore, phase 7); it no longer gates reads.
- [ ] [#25: Define a future preset-write safety design without enabling writes](https://github.com/guirossibrum/VariaxWorkbench/issues/25)
  Depends on #6, #18, #23, #24.

## Unified application and future device families

- [#26: Design one Rust app with per-device capabilities and independent backends](https://github.com/guirossibrum/VariaxWorkbench/issues/26)
- [#27: Recover and validate PODxt Live USB audio routing on Linux](https://github.com/guirossibrum/VariaxWorkbench/issues/27)
- [#28: Add integrated guitar recording and DAW interoperability](https://github.com/guirossibrum/VariaxWorkbench/issues/28)
- [#29: Implement PODxt Live preset editing and librarian within the unified app](https://github.com/guirossibrum/VariaxWorkbench/issues/29)
- [#30: Integrate device-specific firmware inspection and maintenance workflows](https://github.com/guirossibrum/VariaxWorkbench/issues/30)
- [#31: Research and add Workbench HD / James Tyler Variax JTV-69 backend](https://github.com/guirossibrum/VariaxWorkbench/issues/31)
- [#32: Research and add POD HD500X backend](https://github.com/guirossibrum/VariaxWorkbench/issues/32)

See [product vision](product-vision.md). These are unimplemented roadmap targets;
the current implementation remains sequential with #3 In Progress.

- [#33: Expand late-stage synthetic testing at least 5x, including bends and alternate tunings](https://github.com/guirossibrum/VariaxWorkbench/issues/33)
  Depends on #6, #18, #28, #29; required before #22. Includes body-dependent sustain/note decay, continuous bends/vibrato (also during decay), per-string custom tunings, their combined audio behavior and independently verified pitch/artifact checks. Unsupported audio behavior stays unverified.

## Future hardware work — explicit authorization required

- [ ] [#23: Plan and obtain comprehensive verified device backups after authorization](https://github.com/guirossibrum/VariaxWorkbench/issues/23)
  Depends on #22, #16. Now gate G5 (phase 6).
- [ ] [#24: Plan staged read-only integration after explicit authorization](https://github.com/guirossibrum/VariaxWorkbench/issues/24)
  Depends on #22, #21 (superseded: G1–G3 now depend on #112 and #110 through #125–#127).

- [ ] [#34: Add a short guided hardware and playing check](https://github.com/guirossibrum/VariaxWorkbench/issues/34)
  Depends on #24, #28. The [offline rehearsal app](manual-check.md) exists; control recognition and audio measurement remain pending. Target roughly five minutes, with selectors/knobs, a small note sample, bends and optional existing alternate tuning.

Issue bodies contain acceptance criteria, code/document references and the
hardware boundary. `docs/backlog.json` preserves the topic keys, dependencies
and GitHub issue mapping. GitHub issue state is the source of truth for progress.

The [emulation plan](emulation-plan.md) describes the proposed virtual POD/guitar
control path, firmware/DSP feasibility work and pre-hardware test gate for #18.
