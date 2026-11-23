# Project instructions

## Hardware boundary — user requirement

PODxt Live and Variax 600 are irreplaceable. Live device operations are divided
into gates G1–G8 (see docs/hardware-gates.md). **G1 is authorized by the owner's
direct permission (2026-11-22)**, for the procedure in
[docs/hardware-gates/g1.md](docs/hardware-gates/g1.md) only. It takes effect
only in the build from issue #158, which opens only the G1 lock. Until that
build exists, `hardware_gates::G1_OS_DISCOVERY_LOCKED` stays true and the
Device check runs in simulation only. **G2–G8 are not authorized.** Each gate
needs the owner's own direct permission. Permission for one gate never extends
to another, and general permission to work on this project is not gate
permission. The owner does not authorize a gate by signing a GitHub issue.

Until the gate that covers an operation is both authorized and opened by its
build: no `amidi` send/receive, direct USB scripts, updaters, resets, driver
unbinds/detaches, audio streams, or any operation that contacts either device.
Per-gate compile-time locks stay locked, with no environment, command-line,
GUI, or test override, except the #158 build opening only the G1 lock. Use
captured data, fake transports and CPU emulation. Agents do not run the real
G1 step; the owner runs that test.

Write gates (G6, G7) additionally require a verified backup (G5), the
coefficient-parity decision recorded in #6/#22, and a separately approved
restore procedure. Firmware (G8) is out of scope.

The owner directly authorized the item 2 G6 procedure on 2026-11-23 for the
owner-run `tools/device-controls` worktree probe. It extends `podxt live-test`
to the seven pack-checked model selectors, gate threshold, mic, Mod/Delay
NOTE, tweak parameter, pedal assignment, Mod speed, Delay time and tempo.
The probe snapshots the current tone, reads back the edits, attempts undo
including changed companion values in the affected blocks, and verifies the
full tone dump. Amp model selection uses CC 12 with amp defaults disabled.
This authorization applies only to the probe on this branch; the owner
runs it. It does not authorize preset storage, an automatic program change,
GUI writes, or Variax writes. The prior PODxt G6 test's verified backup and
owner-approved edit-buffer undo are recorded in the README ledger.

The owner directly authorized the owner-run linked recall test on 2026-11-23.
`tools/variax-probe/run-linked-recall` selects existing saved PODxt channels
5B → 1B → 5B. The PODxt recalls the linked Variax 600 models. The procedure
uses the existing PODxt channel-switch and Variax metadata-read permissions,
with no direct guitar program changes or preset storage. It attempts a
return to 5B if the target stage fails. This permission is for this worktree
probe and this test only.

No preset upload, flash write, firmware install, reset, or bank switching has been
authorized. Firmware downloads, version comparison, static extraction and offline
reverse engineering are authorized. Latest firmware availability must not cause
an automatic update. GitHub issues and the linked project contain the work plan.

## Implementation

The product is written in Rust. Python and Java are offline analysis/reference
helpers. Keep original vendor artifacts, captures, and user settings local and
ignored. Do not publish them or contact third parties without explicit user
instructions.

Preserve originals. New copies must use atomic non-overwriting saves. Validate
against original Java/native behavior where practical; label inference, emulated
behavior, real captures, and unverified behavior distinctly. Report partial
captures as partial; current-preset metadata is not a full backup.

Run `cargo test --all-features`, `cargo clippy --all-features --all-targets -- -D
warnings`, and appropriate offline compatibility/reference tests. Tests must not
open USB/MIDI devices or require the guitar to be connected.

## README status ledger

Keep README.md's checked offline passes, pending implementation, and currently
unavailable/not-working checklist current whenever work changes those states.
Attach precise test scope and counts where supported. Distinguish disabled
hardware features, unimplemented features, observed failures, original-library
oracle results and completed Rust behavior. Do not mark simulation success as
physical safety, audio equivalence, recovery capability or permission to unlock
hardware. Keep issue links and status consistent with actual completion.

## Work tracking

GitHub issues and the VariaxWorkbench project are the work-tracking source of
truth. Do not recreate or use TODO.md. README.md contains the requested concise
validation/implementation/failure summary; it is not a second task queue. Respect native dependencies and keep at most one implementation issue In Progress
per agent. The user explicitly authorized parallel agents on separate issues:
use a disposable implementation worktree/branch per issue, preserve the main
checkout, and merge each completed issue only after review and offline checks.
Read `docs/parallel-work.md` for current issue ownership; do not duplicate another
agent's reserved issues or remove another agent's worktree.

Target 30–60 minutes of agent time per issue; after more than two review rounds,
stop and propose splitting the issue.

## Product scope

The final objective is one Rust app for guitar editing, POD editing, USB audio
recording/DAW integration, libraries and device-specific maintenance/update
workflows. Initial rig: Variax 600 + PODxt Live on Linux. Future targets include
Workbench HD/JTV-69, POD HD500X and additional independently verified models.
See docs/product-vision.md. Do not assume cross-family format/firmware or audio
compatibility. Roadmap scope does not authorize hardware access: opening audio
capture/playback streams also requires separate explicit user authorization.

## Long-term portability — user requirement

Prefer a self-contained Rust core for longevity across Linux, Windows and macOS.
Keep coefficient math, preset formats and protocol behavior controlled/versioned;
do not treat the host OS's transcendental math as a cross-platform compatibility
contract. Research-only Linux/MPFR/historical-runtime helpers must remain separate
from application runtime dependencies. Evaluate dependency maintenance, portability,
pinning and replacement paths before adopting new production dependencies.
Keep USB, audio and desktop OS interfaces behind thin replaceable adapters.
Normal application operation must not require Line 6 services or external apps.
A self-contained core does not imply that OS integration can have no OS dependency,
or relax any hardware-access boundary.

## Agent-coordinator workflow

When this checkout is managed by agent-coordinator, read
[AGENT_COORDINATOR.md](AGENT_COORDINATOR.md) and the installed
`agent-coordination` skill before receiving work. Use the exact registered scoped
client and the repository context in the receipt. Cloning or reading this guide
does not activate a session, assign work, authorize CI or change any safety rule
above. Keep machine-specific settings and credentials outside tracked files.

## Required coordinated role reading

Before further coordinated action, contributors must read [the contributor role](docs/roles/contributor.md), and Gatekeepers must read [the Gatekeeper role](docs/roles/gatekeeper.md). Both roles must explicitly load the versioned generic review skill named by their delivery. These role documents preserve all hardware and private-input boundaries; they do not activate a missing coordinator policy or grant work, CI or merge authority.
