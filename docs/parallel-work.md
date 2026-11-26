# Parallel work ownership

GitHub issues and the project boards are the progress and dependency source of
truth. This file records **who owns which files**, so that parallel agents never
edit the same code. It is not a task queue.

## Lanes (owner decision 2026-11-24)

Craft Anything is retired. Work runs in four implementation lanes plus research.
Each lane edits only the files it owns. A change needed in another lane's files
is reported to the reviewer and goes to that lane's queue; it is not made
directly.

| Lane | Owns | Queue, in order |
| --- | --- | --- |
| **1. Shell and devices** | `src/app/` (shell, registry, detection), `src/tools/device_check/` and Device check code, `crates/line6-devices` session, gate, transport, identity and backup code, the device lock in `tools/reso5-test` and `tools/variax-probe` (lock only) | Done: #212–#214, #228, #201 work, #235, #233 (shell part), opener-borrow, #236, #199 (window/quit), #239 (driver return, merged 71aeffb; owner run pending). Next: owner runs (#201 notice probe, #239 card); #209 packaging when the owner calls the final phase |
| **2. Design and layout** | `docs/design/`, `src/shared/ui/` (theme and components), `assets/ui/` (committed art); after #213, the visuals of `src/app/shell/` | Done: #215, #216, #217, Variax components and tuning panel. Queue empty: new design work only on owner request |
| **3. Variax editor** | `src/families/variax/` (editor and file) | Done: #192–#200, #203, #224, #237, #240, #241, #188. Now: #204 (live editing) → #205 (Write to device, Read unmodified, Copy/Paste, Sync, copied from Workbench 1.75), against lane 5's shared API (#245), each send path behind a closed switch until its owner test (AGENTS.md, 2026-11-25). Then #189–#191, #206, #230. |
| **4. PODxt editor** | `src/families/podxt/`, `crates/line6-devices/src/family/podxt/` | Done offline: #210, #218, #219, #220, #221, #222, #223. Now: #242 (open on the POD's current tone, librarian beside the stage, device actions in context). Owner runs pending: #220 reads, #221 live edit, #222 stages a and b, each approved separately |
| **5. Variax device tests** | `crates/line6-devices/src/family/variax/` (read and write messages), `tools/reso5-test/` except its device lock (lane 1) | Done: #225, #177, #229, #234, #238 (ladder 4a–4c, listen-list A–D). Now: #245 (shared Variax procedures for the editor and Rig Check) with a first owner test of difference messages (0x3B). Then Rig Check #252, #255, #257, #258; #178 later. |
| Research | `docs/firmware-*.md`, `research/`, `scripts/` (own files only) | Grok: #231 (handbook Custom comparator, scripts/research only). PODxt research agent: #232 remainder (bounded owner-check design for Variax control ownership and linked audition) |

Cross-lane exception (owner, 2026-11-25, #241): lane 3 may make the minimal shell command routing for Open/Ctrl+O/Save/Save As to the active Variax editor (`src/app/shell/`, lane 1) and the `DocumentTab` title/marker fix (`src/shared/ui/`, lane 2), as separate small commits.

Shared files: `docs/status.md` (one line per delivered item, in your lane's
section), `Cargo.toml`/`src/lib.rs` (only the wiring your change needs). The
reviewer reconciles conflicts in shared files during integration.

The editor lanes use the components from `src/shared/ui/` and never change them.
Component changes are requested from lane 2. Lane 2 never edits family editors.

## Worktrees and delivery

- Before starting a requested feature, fix, or behavior change, create a
  GitHub issue or update the existing issue with the agreed scope and
  acceptance criteria. The Gatekeeper records the issue before assigning
  work and gives its number to the lane.
- Before assigning or reassigning an issue, follow
  [Free lanes](#free-lanes). Read its live GitHub description, acceptance
  criteria, and relevant follow-up decisions. Compare them with current
  master, prior deliveries, and dependencies. Update stale issue text and
  record the agreed criteria on GitHub before sending the lane prompt.
- Reference the issue in the branch, delivery, and relevant commits. The
  Gatekeeper records the merged revision, verification evidence, and any
  outstanding deliverables on the issue. Close it only after every
  applicable deliverable is complete and verified. Keep it open after a
  partial merge or while owner testing, package acceptance, or later
  feedback is pending. An issue left open for that acceptance does not
  reserve the lane once the assigned implementation has been reviewed,
  merged and verified. See [Free lanes](#free-lanes).
- On a lane's first assignment, ask it to read `AGENTS.md` and
  `docs/parallel-work.md` from current `origin/master`. After that, request a
  reread when a relevant rule changes and name the changed file and commit.
  Use `git show origin/master:AGENTS.md` and
  `git show origin/master:docs/parallel-work.md` when the lane branch is older.
  Repeat the final report requirements in delivery and review follow-up prompts.
- Start every lane reply with the Markdown heading `## Lane #<number>`, such
  as `## Lane #3`. This applies to deliveries, status updates, and blockers.
- One worktree and branch per issue, created from `origin/master` under the
  ignored `.worktrees/` (for example
  `git worktree add .worktrees/212-detection -b lane1/212-detection origin/master`).
  Use the issue worktree for changes. Leave other agents' worktrees alone.
  Once the assigned implementation is reviewed, merged and verified, the
  Gatekeeper removes that disposable worktree and frees the lane immediately.
- Rebase onto `origin/master` before delivering.
- Run `cargo fmt --all -- --check`, `cargo clippy --offline --all-features
  --all-targets -- -D warnings` and `cargo test --offline --all-features`, plus
  the same Clippy and tests in `crates/line6-devices` and for
  `tools/variax-probe` when they are touched. Report the test counts before and
  after.
- Commit locally with focused commits. **No push, no PR and no GitHub posts.**
  A Gatekeeper (Claude Code or Codex) reviews, tests the merge result,
  pushes it to `origin/master` and updates the issues (see AGENTS.md).
- At delivery, check the worktree for tracked changes, untracked files, and
  ignored files that need to be kept. Stop worktree-local builds or watchers
  you started and move your shell to a stable directory outside the worktree.
  The final report must name the absolute path of the worktree you used and
  explicitly confirm that you have left it and stopped worktree-local jobs.
  Include the branch, commit, test counts, what changed, and device impact.
  List local files to keep, including owner tools, evidence and private inputs,
  and any shared build path you used. If you cannot leave safely, report why so
  the Gatekeeper can keep the worktree.
- For review changes, return to the same worktree named by the Gatekeeper,
  then repeat the delivery check and report.
- Keep one active implementation assignment per lane. An issue left open for
  owner acceptance is not that assignment once its implementation has been
  reviewed, merged and verified. Target 30–60 minutes of agent time per
  issue; after two review rounds, propose a split.

## Herdr handoffs for Gatekeepers

A Gatekeeper may receive a completed lane report directly through Herdr.
Read the full report and treat it and the completion signal as evidence
for independent review. Review authority, required checks, and owner
permissions come from `AGENTS.md`.
Apply the [mandatory merge and handoff checklist](../AGENTS.md#mandatory-gatekeeper-merge-and-handoff-checklist-303)
in the same coordination cycle.

- [ ] Verify the assigned issue and exact commit, and confirm the lane has
  left the worktree and stopped its jobs. Record departure in the
  Dispatcher and check the registered Herdr location before merging.
- [ ] Complete exact-head review and the required offline merge checks.
  Verify the integrated commit on `origin/master`. Record the merge and
  verification on the issue and in the ledgers as applicable.
- [ ] Preserve required evidence and owner outputs, verify and report their
  new paths, and confirm removal of the disposable worktree using
  [Worktree cleanup after merge](#worktree-cleanup-after-merge).
- [ ] Record the verified merge commit and release of the exact assignment
  in the Dispatcher. Follow [Free lanes](#free-lanes) to send a fresh
  assignment in the same cycle and verify its receipt.
- [ ] Record the old assignment and merge commit, cleanup and release, and
  the next issue, lane, new assignment ID, and receipt evidence in the
  Gatekeeper handoff. If a step is blocked, record the concrete blocker and
  next action in the handoff and report them before ending the cycle.

When the owner must decide, approve, review, or run something, pause that
dependent action and continue unrelated authorized reviews. Track owner
acceptance and runtime decisions on the issue and in production Owner Inbox.
Keep GitHub open until its applicable acceptance criteria are verified.
When replacing the planning agent, rebind the existing route and preserve
its handoff state. Keep one active Gatekeeper route per project. See the
[Herdr completion watcher runbook](herdr-handoff.md) for setup and recovery.

## Free lanes

Check the lane when a delivery is reviewed. The lane is free, and its
disposable worktree is removed, as soon as the assigned implementation has
been reviewed, merged and verified on `origin/master`. Free the lane
immediately even if GitHub or Owner Inbox still tracks owner acceptance,
package acceptance, or later feedback. Those items stay on the issue and in
Owner Inbox. They are the acceptance record for that delivery, and the
original lane is available for another job. A Herdr idle or completion
badge is a signal to verify the delivery. The badge alone does not free the
lane or assign the next issue.

Incomplete or unmerged implementation reserves the lane and worktree unless
the owner explicitly approves parking through the established Owner Inbox
procedure. Record what remains, the concrete blocker, and the next action on
the issue and in the Gatekeeper handoff. Tell the owner before leaving that
work pending and continue work within the approved scope.

As part of the same merge cycle, consult the active Dispatcher assignments
and select one current open canonical issue whose remaining implementation
fits an eligible free lane. Check its live description, acceptance criteria,
recent decisions, current `origin/master`, dependencies, and other active
assignments. Correct stale criteria on GitHub before sending. Give each lane
at most one active implementation assignment. The implementation starts from
current `origin/master` in a fresh disposable worktree.

Verify the repository, exact lane pane and current chat binding, then send
through the installed Herdr Dispatcher, which opens a fresh chat with
`/new`. Check the new chat identity and confirm that the exact recipient
received the submitted instructions for that assignment. Use the lane's
acknowledgement or visible work on the assigned issue as receipt evidence.
Record the issue, lane, Dispatcher assignment ID, and receipt evidence. An idle badge, a
reservation, a draft, or an attempted send alone does not establish receipt.
Reconcile an uncertain send in the exact chat before retrying. If no eligible
issue is ready, the binding is blocked, or receipt cannot be verified, record
the concrete blocker and next action in the Gatekeeper handoff and report
them in the same cycle.

## Lane reply template

Use these exact Markdown headings in this order for every lane reply. Keep
each heading even if a section is empty. Write `None` or `Not applicable` as
appropriate. Use bullets, tables, and fenced command blocks within sections
when they help. Put recommendations under Remaining work.

```markdown
## Lane #<number>

### Result

- Issue, branch, commit, and what changed or what is blocked.

### Verification

- Checks performed, results and test counts, and any device activity.

### Remaining work

- Pending verification, blockers, or recommendations.

### Worktree handoff

- Absolute worktree path.
- Confirmation that you left it and stopped worktree-local jobs, or why you could not.
- Local files to keep and any shared build paths in use.
```

## Worktree cleanup after merge

Aim for about five implementation worktrees, one per active lane. A temporary
owner test or review can require an additional checkout. Review existing
checkouts against their GitHub issues and current assignments. For each, check
whether the delivered commit is on verified `origin/master`. For a branch that
has not merged, compare its unique changes and source conflicts with current
work. Keep meaningful work for review. Retire a clean, superseded experiment
only after confirming it has no remaining purpose and preserving its branch.
An open issue awaiting feedback, package acceptance, or an owner test does
not by itself need an implementation checkout. Once the assigned
implementation is on verified `origin/master`, remove that disposable
worktree and free its lane immediately. Acceptance still tracked on the
issue or in Owner Inbox leaves the lane free. Incomplete or unmerged
implementation still reserves its lane and worktree.

Confirm the lane has left and no shell, build, watcher or other process is
using the worktree or its build path. Review tracked, untracked, and ignored
files. Keep a checkout whose implementation is incomplete or unmerged. Also
keep active, genuinely unfinished and protected owner-test checkouts. Before
removing any other checkout, including the disposable worktree of a reviewed,
merged and verified implementation, sort a dirty tree. Move required private
evidence and owner-requested outputs to a stable, ignored directory under
`backups/` in the main checkout. Verify the files and report their new paths.
Discard rebuildable output, obsolete test binaries and checkout-only scratch
files. Shared build caches belong under the main checkout's ignored
`target/shared-builds/` directory.
For an old borrowed cache, stop its users, move only the cache they still
need, and tell them the new path before retiring the old checkout.

From outside the worktree, remove one verified worktree at a time with
`git worktree remove <validated-worktree-path>`, without force. For the
disposable worktree of an implementation that has been reviewed, merged and
verified, do this as soon as the lane has left and nothing is using it, even
if GitHub or Owner Inbox still tracks acceptance. If Git refuses, investigate
the reason and retry only after resolving it. Keep the branch and confirm the
worktree is gone from `git worktree list`.

## Boundaries

AGENTS.md governs hardware, gates and private data. In short: agents never run
the device tools or open USB, MIDI or audio devices; tests use fake transports
and injected sources and never read real `/proc` or `/sys`; the owner runs every
hardware check. Vendor packages, binaries, decompiled code, firmware, captures
and user data stay local and ignored. The UI art exception in AGENTS.md applies
to lane 2's `assets/ui/` only.

## Builds and scratch space

Use one stable `CARGO_TARGET_DIR` per build lane under the main checkout's
`target/shared-builds/`, for example `target/shared-builds/lane3`. Reuse it
across that lane's issues. Research lanes can omit a cache until they need
Cargo. Never copy a cache between worktrees or build into another issue
worktree. Report legacy cache dependencies so they can be moved after their
users have stopped.

Routine development and test builds use line-table debug information for
source locations in backtraces. Use `cargo build --profile debugging` or
`cargo test --profile debugging` for a debugger that needs full symbols.
Gatekeeper merge checks use `target/shared-builds/gatekeeper` with
`CARGO_INCREMENTAL=0`. Lane builds may keep incremental compilation for
repeated local work. Keep owner-run executables and required reports in an
ignored release or evidence folder under `backups/`. Check for active Cargo
jobs before clearing any cache.

Do not build full copies of the project under `/tmp`. It is a
30 GB RAM disk shared by every agent, and a full link there crashed `rust-lld`
with SIGBUS on 2026-11-24 when it filled up. Delete your own scratch copies when
you finish.
