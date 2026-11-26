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
  The Gatekeeper handles cleanup after integration.
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
- Keep one issue in progress per agent. Target 30–60 minutes of agent time per
  issue; after two review rounds, propose a split.

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
An open issue awaiting feedback or an owner test does not by itself need an
implementation checkout.

Confirm the lane has left and no shell, build, watcher or other process is
using the worktree or its build path. Review tracked, untracked, and ignored
files. Keep active, dirty, genuinely unfinished and protected owner-test
checkouts. Otherwise move required private evidence and owner-requested
outputs to a stable, ignored directory under `backups/` in the main checkout.
Verify the files and report their new paths. Discard rebuildable output,
obsolete test binaries and checkout-only scratch files. Shared build caches
belong under the main checkout's ignored `target/shared-builds/` directory.
For an old borrowed cache, stop its users, move only the cache they still
need, and tell them the new path before retiring the old checkout.

From outside the worktree, remove one verified worktree at a time with
`git worktree remove <validated-worktree-path>`, without force. If Git refuses,
investigate the reason and retry only after resolving it. Keep the branch and
confirm the worktree is gone from `git worktree list`.

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
