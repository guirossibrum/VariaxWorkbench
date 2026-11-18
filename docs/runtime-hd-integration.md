# Offline runtime and HD research integration

The existing coefficient-runtime and independent POD HD research milestones are
integrated: [PR81](https://github.com/guirossibrum/VariaxWorkbench/pull/81) at
`cfd69cd7bcf8eae8349652a4c9cfb71f5d0e0cf5`,
[PR83](https://github.com/guirossibrum/VariaxWorkbench/pull/83) at
`254416e5a17c6d9b7aa08de888b466c42a097ef1`, and
[PR82](https://github.com/guirossibrum/VariaxWorkbench/pull/82) at
`cd17700ab238f160c5e85a3b82d7f9d5f8d67a22`. This report distinguishes completed host software/reference checks
from device support that remains unavailable. GitHub issues remain the work plan.

## Coefficient-runtime result

The optional Apple runtime harness statically selects pinned i386 libSystem
88.3.9 from an official Intel 10.4.11 update. It runs selected instructions only
inside Unicorn; no Apple/Line 6 library or installer is loaded natively. The
ordinary oracle retains its exact startup contract, while the Apple candidate
has its independently pinned one-ULP-different power constants and a separate
reference namespace. Unknown imports, OS interrupts, unsupported controls and
exhausted budgets stop the research process.

The existing trace covers 206,228 distinct math tuples. Both historical Apple
candidates complete the same 110 generator settings, matching 109 saved glibc
outputs and all 110 saved MPFR candidates. Fresh actual Rust CLI builds on Linux
x86-64, including the combined integration tree, match all 110 Apple candidates
in debug/release under the experimental portable profile. Default host-profile
builds preserve all 110 glibc references. A single series setting differs by four
coefficient words plus checksum; eight fresh-state diagnostics identify `sinf`
as sufficient to reproduce that complete change in this case.

This does not make the experimental profile the default. Its dependency remains
pinned, versioned Rust math compiled into the program; optional research tools
are not application dependencies. It also does not prove the difference is
inaudible, safe for a device, or representative of every possible setting.

Unicorn implements x87 transcendental helpers through host double math. The
separate owned instruction helper agrees on 194,460 float trig results but differs
on one of 848 double tangents on the current CPU. Both still produce the same 110
packed candidates. Historical CPU differences, complete floating-point exception
state, native ABI/JVM behavior, destructors and physical audio remain unverified.
See [the detailed runtime report](apple-tiger-math.md) and
[product math profiles](math-profiles.md).

## Independent HD file evidence

PRs #81 and #83 record limited failed public acquisition routes, hashed manuals,
and independent H5EP/H5ES measurements. Three `.5xe` files under a directory named
HD500X carry the same six-byte sequence the pinned independent source writes in
its `ProX` branch. A setlist has 64 H5EP spans beginning at byte 56 with stride
4136 and 22 trailing bytes; its embedded sequence differs from that source's
`500X` write. Neither directory names, suffixes nor source comments establish a
registered device identity. No third-party converter code was copied or run.

Owner-supplied Editor/PHF/model-default and firmware-image materials have since
been examined statically and remain private. This does not register a
product-specific preset schema, model/effect/default catalog, target-applicable
firmware interpretation, or semantic edit behavior. The HD500X session
continues to reject preset opening as `Unverified`. Linux negotiation and live
USB-audio routing remain unverified. See [HD500X evidence](pod-hd500x.md).

## Verification

- Independent PR81 checks passed 217 Rust tests per GUI math profile; PR83 and
  the final combined tree passed 218 per profile, including 23 headless GUI tests.
  Clippy with warnings denied and formatting passed.
- [Combined-tree CI run 43810057601](https://github.com/guirossibrum/VariaxWorkbench/actions/runs/43810057601) passed Linux x86-64, Windows x86-64 and macOS
  ARM: 194 portable core/file tests in debug and again in release on each target,
  plus Clippy and artifact-free setup/syntax guards. This CI does not rerun
  proprietary-resource comparisons or validate native GUI dialogs/USB integration.
- Optional local research reproduced archive/slice hashes, eight source/profile/
  CPU guards, four product-evidence rejection checks and the direct product/factor
  comparisons. Original inputs and detailed generated outputs remain private.
- Each actual merge tree is checked against the reviewed and tested tree. Source
  branches and private evidence are preserved; merging a milestone does not close
  unresolved issue criteria.

## Remaining risks and boundaries

No USB/MIDI/audio device was enumerated, opened or contacted for these checks.
No driver was installed, loaded or bound. No preset, calibration, firmware, reset
or bank-switch operation was attempted. `LIVE_ACCESS_LOCKED` remains true and
`LIVE_BACKEND_INITIALIZER` remains `None`; no override was added.

The current settings capture is not a complete backup. Physical recovery,
firmware/DSP/peripheral emulation, actual sound, sustain, bends, tuning and device
compatibility are not established by these host-library results. The larger
late-stage synthetic test gate and offline readiness review remain pending.
Equipment stays disconnected; any future live operation requires its own precise
review and explicit authorization. Future implementation belongs to contributors
through agent-coordinator after its repository/CI/dispatcher repair is confirmed;
Codex and Grok retain Gatekeeper responsibilities after these existing tasks.

Independent Gatekeeper review found no blocking issue for PR82's bounded
research scope. Issue #5's third acceptance box remains unchecked: the
differences are quantified, but full historical-runtime accuracy is unresolved.
Issue #32 and its acceptance boxes also remain open.
