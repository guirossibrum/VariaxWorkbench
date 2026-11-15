# GitHub work backlog

Repository: https://github.com/guirossibrum/VariaxWorkbench. Default branch: `master`.

Project: https://github.com/users/guirossibrum/projects/17. All 25 issues are on
the private project with an explicit Order and Phase. Their 24 dependencies are
native GitHub blocked-by relationships, not only issue-body links.

Work sequentially in Order, respecting dependencies and the authorization phase.
Issue #1 is In Progress; no second implementation issue has been started.

All issues remain subject to [AGENTS.md](../AGENTS.md). Live USB/MIDI access
is locked; creating an issue does not authorize its hardware steps.

## Offline implementation and research

- [ ] [#1: Port complete body schema and interpolation to Rust](https://github.com/guirossibrum/VariaxWorkbench/issues/1)
- [ ] [#2: Port pickup response and electronics calculations to Rust](https://github.com/guirossibrum/VariaxWorkbench/issues/2)
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
  Depends on #6, #18, #21.
- [ ] [#25: Define a future preset-write safety design without enabling writes](https://github.com/guirossibrum/VariaxWorkbench/issues/25)
  Depends on #6, #18, #23, #24.

## Future hardware work — explicit authorization required

- [ ] [#23: Plan and obtain comprehensive verified device backups after authorization](https://github.com/guirossibrum/VariaxWorkbench/issues/23)
  Depends on #22, #16.
- [ ] [#24: Plan staged read-only integration after explicit authorization](https://github.com/guirossibrum/VariaxWorkbench/issues/24)
  Depends on #22, #21.

Issue bodies contain acceptance criteria, code/document references and the
hardware boundary. `docs/backlog.json` preserves the topic keys, dependencies
and GitHub issue mapping. GitHub issue state is the source of truth for progress.
