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
pass does not open step 4. Step 4 stays locked. The owner then instructed
the app to include the Variax and PODxt Live library backups from
`tools/device-controls`. Build 16 offers both in Device check on This
computer. The Variax backup sends program changes so it can read each stored
preset, then tries to switch back. If the guitar does not answer the first
identity question, the backup asks once more on that connection, and if it is
still quiet it gives the driver back and tries the connection once more. It
does not store anything. Backup files stay ordinary writable files. The PODxt Live
backup reads stored tones and setups and does not change the current tone.
Nothing is written back by the backups.

**On 2026-11-23 the owner released every gate (G1–G8) by direct permission:**
all previous owner-run tests passed, and the next test writes to the stored
banks of the Variax 600 and the PODxt Live. Every compile-time lock in
`src/hardware_gates.rs` is open, and the app may use all operations of
`crates/line6-devices`, each behind the gate for its kind of action. The owner
approved the tests and implementations at that head. Firmware installation
stays outside the product scope (below) even with G8 released, until the owner
says otherwise. The owner decides which of these rules change. Do not drop a
requested device option because an older gate note did not list it; ask the
owner. General permission to work on this project is not gate permission, and
the owner does not authorize anything by signing a GitHub issue.

The gate structure stays even though every gate is released: each device path
names its gate and goes through `run_if_unlocked`, and a gate is closed again
by setting its constant back to `true`. There is no environment, command-line,
GUI, or test override. An open gate does not let an agent run the device. `usb::read_identity` and
the family reads return before enumeration in every test build. Use captured
data, fake transports and CPU emulation. Agents do not run the real G1 step,
the real G2 step, or the real step 3 read; the user runs those checks. Do not
read real `/proc` or `/sys`.

Write gates (G6, G7) additionally require a verified backup (G5), the
coefficient-parity decision recorded in #6/#22, and a separately approved
restore procedure. Verified library backups of both devices exist; the
restore procedure is part of the bank-write test. Firmware (G8) is out of scope.

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

The owner separately authorized one owner-run PODxt Live Variax-control
release display check. The worktree-only probe may verify the PODxt Live
identity and send the original Workbench 1.75 XT Live release once
(`L6VariaxDevice.SetXTLiveVariaxMessagesEnable(false)`). Check the last EDIT
page manually. This standalone check sends no guitar command, channel change,
or preset store. This permission does not add the release to linked recall.

The owner separately authorized the owner-run linked model edit test on
2026-11-23. In the active PODxt tone, verify F-T-Model 1 and Telecaster on
the Variax 600, temporarily select F-Spank 1 via Line 6 Edit's wrapped
model command, wait 10 seconds and check both devices. Attempt to restore
F-T-Model 1, wait 10 seconds and verify the original guitar metadata and
PODxt tone. This approval is limited to that worktree probe procedure and
those two models. It includes no PODxt channel change or save.

The owner directly authorized an owner-run read-only Variax slot read on
2026-11-24 (#225). The tool sends `0x44` (stored-slot read, reply `0x45`) for
the current buffer (`0x7F`), Reso-5's slot (flash index 50) and its spare
sector (62), and compares the replies offline. Workbench 1.75 never sends
`0x44`. The #181 trace finds no program or erase call in its handler. It
sends no `0x45`, `0x61`, `0x6C` or other write, no program change, and no
PODxt write. The read results are a prerequisite for the #177 store test.
This approval covers that read only.

**Editor sends and stores (owner decision 2026-11-25, after the #238 ladder and
listen-list passed).** The Variax editor may send programs to the guitar's
working buffer for live editing (#204) and store presets through Write to
device, Read unmodified from device, Copy/Paste and Sync, copied from Workbench
1.75 (#205). It must use the shared verified procedures (#245): full upload or
difference messages for live edits; for stores 0x43, 100 ms, 61, one 0x71 wait,
a full read-back, no automatic resend, and the silent-timeout stop. Each path
stays behind its own compile-time switch, closed until its first owner-run test
on the rig (first live edit, first Write to device, first Sync) passes with the
owner's approval.

**Rig Check real runs (owner decision 2026-11-25, #267).** Every Rig Check
device and guided step may be enabled for real runs, including the live and
save steps, once it has been reviewed and merged. The audio steps stay closed
because audio is not implemented. Rig Check must cover every kind of device
message the app sends. Its save steps run only when every earlier step in the
same run passed; after any failure, the rest of the run is read-only. A passing
run on the owner's rig is the owner's approval to test the app's matching
writes. The owner runs each released build first, and it is shared with
others only when the owner says so.

**Distribution (owner decision 2026-11-25, #273).** Builds are not signed or
notarized; there is no Apple developer account and none is planned. The
macOS app carries only an ad-hoc signature, which Apple silicon requires to
run. The disk image tells users how to authorize the app. Untested builds may
be published as GitHub alpha pre-releases, labelled untested. On macOS the
app must not depend on Line 6 or other third-party drivers (the Line 6 driver
is Intel-only, and none exists for Apple silicon). A direct USB connection,
built into the app, is allowed, with owner-run tests as for any device path.

Stored-bank writes (G7) are released for the owner-run bank-write tests. No
firmware install or reset is planned.
Firmware downloads, version comparison, static extraction and offline
reverse engineering are authorized. Latest firmware availability must not cause
an automatic update. GitHub issues and the linked project contain the work plan.

## Implementation

The product is written in Rust. Python and Java are offline analysis/reference
helpers. Do not contact third parties without explicit user instructions.

**Reverse-engineered and vendor material (owner decision 2026-11-25; replaces
the earlier wording).** Raw reverse-engineered material is never published.
That covers decompiled code, disassembly, vendor files as extracted,
installers, firmware images, captures and user data: they stay local and
ignored, and are never committed, attached to issues or uploaded as-is. What
the app needs in order to run may be committed and shipped, once re-packaged
for the app, with each item's source product, version and archive path
recorded. The owner considers this fair use: the app supports legacy gear and
does not compete with Line 6. The repository is public. Commit only what the
app actually uses:
- **UI art** (owner decision 2026-11-24, layout work #215–#217, #219, #224):
  Line 6 UI art from POD Farm 2, Variax Workbench and Line 6 Edit, under
  `assets/ui/`, with `assets/ui/SOURCES.md`.
- **Variax model data** (#278): the body, pickup and special-model
  definitions the host engine needs to render programs, converted into the
  app's own format and built into the app, with a sources file.

Preserve originals. New copies must use atomic non-overwriting saves. **Editor Save
exception (owner decision 2026-11-25, #241):** the editors' Save may replace the
preset file the user opened or created in the editor, by writing a temporary file
in the same folder and renaming it over the original. Save never writes into
library backups, test logs, captures, vendor archives or firmware; Save As and
every other copy stay non-overwriting. Validate
against original Java/native behavior where practical; label inference, emulated
behavior, real captures, and unverified behavior distinctly. Report partial
captures as partial; current-preset metadata is not a full backup.

Run `cargo test --all-features`, `cargo clippy --all-features --all-targets -- -D
warnings`, and appropriate offline compatibility/reference tests. Tests must not
open USB/MIDI devices or require the guitar to be connected.

## README status ledger

Keep the detailed ledger in [docs/status.md](docs/status.md) for verified work
and in [docs/roadmap.md](docs/roadmap.md) for pending or unavailable work.
Update the ledger whenever those states change. Attach precise test scope and
counts where supported. Distinguish disabled hardware features, unimplemented
features, observed failures, original-library oracle results and completed Rust
behavior. Do not mark simulation success as physical safety, audio equivalence,
recovery capability or permission to unlock hardware. Keep issue links and
status consistent with actual completion. README.md keeps short capability lists.
Update it only when user-visible capability changes.

## Work tracking

GitHub issues and the VariaxWorkbench project are the work-tracking source of
truth. Do not recreate or use TODO.md. The README summarizes user-visible
capabilities. The linked documentation records verification and pending work.
Respect native dependencies and keep at most one implementation issue In Progress
per lane. Aim for one disposable checkout per active lane, about five for this
project. Add a checkout only while there is a specific task or an owner-run
procedure that needs it. Once reviewed and tested, integrate durable source
changes into `origin/master`. An open issue does not reserve an old checkout
whose implementation has already merged.
Read `docs/parallel-work.md` for current issue ownership and the delivery and
cleanup procedure. Do not duplicate another agent's reserved issues.

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

## Review and merge (owner decision 2026-11-26)

Claude Code and Codex are the Gatekeepers. The agent-coordinator dispatcher is
retired: do not use its receipts, CI approval or auto-merge, and do not try to
restore it. `AGENT_COORDINATOR.md` and `docs/roles/` are kept for history only;
where they disagree with this section, this section applies.

Lane agents work as described in `docs/parallel-work.md`: one worktree per issue
from `origin/master`, local commits, no push, no PR, and a report when done.
Every Gatekeeper prompt to a lane, including a review follow-up, explicitly
instructs the agent to reread `AGENTS.md` and `docs/parallel-work.md` from current
`origin/master` before starting. Each prompt also repeats the worktree
requirements for the final report.

A Gatekeeper reviews the exact head against each acceptance criterion and this
file. It then merges the branch onto current `origin/master` in a throwaway
worktree, runs the checks listed under Implementation (plus the crate and probe
checks when touched), and pushes that tested result to `origin/master`. It also
updates the issues and the ledgers.

After committing and checking the worktree, the lane agent leaves it. Its final
report must give the absolute worktree path and explicitly confirm the agent has
left it and stopped its worktree-local jobs. Report the branch, commit, any local
files to keep, and any shared build path in use. If the agent cannot leave safely,
it reports the reason and the worktree stays in place.

If review needs changes, the Gatekeeper sends the agent back to the same
worktree. After verifying the delivery is on `origin/master`, the Gatekeeper
removes that individual worktree with `git worktree remove` without force,
once the lane has left and no process is using it. Move evidence and other
durable local files into the main checkout's ignored `backups/` area and report
their new paths. Discard verified disposable build output and checkout-only
scratch work. Shared build caches belong under the main checkout's ignored
`target/shared-builds/` directory. Move an older borrowed cache only after its
users have stopped and can switch to the new path.

Keep active, dirty, genuinely unfinished, and protected owner-test worktrees.
For a historical checkout, check the associated GitHub issue, merge ancestry,
unique changes, and conflicts with current work. A clean, obsolete experiment
may be retired after confirming it has no remaining purpose and retaining its
branch. Follow the checks in `docs/parallel-work.md`.

Do not merge from, reset or switch the main checkout. Gatekeepers make no device
runs; the owner runs every hardware check.
