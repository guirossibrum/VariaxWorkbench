# Project instructions

## Hardware boundary — user requirement

PODxt Live and Variax 600 are irreplaceable. Live device operations are divided
into gates G1–G8 (see docs/hardware-gates.md). **G1 is authorized by the owner's
direct permission (2026-11-22)**, for the procedure in
[docs/hardware-gates/g1.md](docs/hardware-gates/g1.md) only. **G2 is authorized
by the owner's direct permission (2026-11-22)** for the bounded Variax 600
identification through the PODxt Live, for any user of the same supported rig,
following [docs/hardware-gates/g2.md](docs/hardware-gates/g2.md) only. #114 stays
open. The build opens `G2_IDENTITY_REQUEST_LOCKED`. A normal build of
`variax-workbench-gui` includes `usb`, so the app offers steps 2 and 3. `gui`
alone still does not. Device check names the steps available in the running build. The
app does not ask for a password. Accepting the risk is the Device check Yes
for that step, and No stays the default. If step 1 passes, step 2 is offered.
If step 2 passes, step 3 is offered. If Linux refuses the open, the question
is not sent and the guitar driver is not detached. G1 stays open as it is.
**On 2026-11-22 the owner released device-check step 3 of G3** after an
external read named the Variax 600, the PODxt Live, and the current settings,
and a later read saw the changed settings. Step 3 reads the Variax current
settings and the PODxt Live current tone. Each family stays in its own module
under `crates/line6-devices`. The owner then ran build 12. Steps 1–3 passed
on USB 8-2.1: Variax 600 firmware 3.10, PODxt Live firmware 3.01, the current
guitar preset, and the current PODxt Live tone. Nothing was written. That
pass does not open step 4. Step 4 stays locked. G4–G8 are not authorized.
Device check step 3 is governed by `G3_DEVICE_CHECK_READ_LOCKED`.
`G3_CURRENT_SETTINGS_READ_LOCKED` stays true so the editor fetch stays off. Each later gate needs the owner's own direct permission. Permission for
one gate never extends to another, and general permission to work on this
project is not gate permission. The owner does not authorize a gate by signing
a GitHub issue.

Until the gate that covers an operation is both authorized and opened by its
build: no `amidi` send/receive, direct USB scripts, updaters, resets, driver
unbinds/detaches, audio streams, or any operation that contacts either device.
Per-gate compile-time locks stay locked, with no environment, command-line,
GUI, or test override, except the #158 build opening the G1 lock and the #159
build opening the G2 lock, and the device-check step 3 read of both families.
An open gate does not let an agent run the device. `usb::read_identity` and
the family reads return before enumeration in every test build. Use captured
data, fake transports and CPU emulation. Agents do not run the real G1 step,
the real G2 step, or the real step 3 read; the user runs those checks. Do not
read real `/proc` or `/sys`.

Write gates (G6, G7) additionally require a verified backup (G5), the
coefficient-parity decision recorded in #6/#22, and a separately approved
restore procedure. Firmware (G8) is out of scope.

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
