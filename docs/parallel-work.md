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
| **1. Shell and devices** | `src/app/` (shell, registry, detection), `src/tools/device_check/` and Device check code, `crates/line6-devices` session, gate, transport, identity and backup code, the device lock in `tools/reso5-test` and `tools/variax-probe` (lock only) | Done: #212–#214, #228, #201 work, #235, #233 (shell part). Done also: opener-borrow, #236, #199 (window/quit). Next: owner runs (#236 backup, #201 notice probe); then #209 packaging when the owner calls the final phase → #239 (driver rebind after release; Refresh guitar reattaches) |
| **2. Design and layout** | `docs/design/`, `src/shared/ui/` (theme and components), `assets/ui/` (committed art); after #213, the visuals of `src/app/shell/` | #215 (specification and mockups, **owner approval before code**) → #216 (theme and component library) → #217 (shell restyle) |
| **3. Variax editor** | `src/families/variax/` (editor and file) | Done: #192–#197. Next: #202 GUI wiring (device side merged) → #203 → #237 (path-dependent test) → #200 (replace the locked button with live status) → #198, #199, #200 → after #216: #188, #189, #190, #191, #224 (restyle) → #204, #206 → #205 (Sync, after the store tests #177/#178); #230 (from #227) |
| **4. PODxt editor** | `src/families/podxt/`, `crates/line6-devices/src/family/podxt/` | #210 (parameter table) → #218 → #219 (after #216) → #220 (after #213) → #221 → #222 (after #211 items 3–4) → #223 |
| **5. Variax device tests** | `crates/line6-devices/src/family/variax/` (read and write messages), `tools/reso5-test/` except its device lock (lane 1) | Done: #225, #177 ladder (1a, 1b, 2), #229, #234, read retry, #202 device side, #203 device API. Next: #238 (store from any selection, ladder 3a/3b) → #178 |
| Research | `docs/firmware-*.md`, `research/`, `scripts/` (own files only) | Grok: #231 (handbook Custom comparator, scripts/research only). PODxt research agent: #232 remainder (bounded owner-check design for Variax control ownership and linked audition) |

Shared files: `docs/status.md` (one line per delivered item, in your lane's
section), `Cargo.toml`/`src/lib.rs` (only the wiring your change needs). The
reviewer reconciles conflicts in shared files during integration.

The editor lanes use the components from `src/shared/ui/` and never change them.
Component changes are requested from lane 2. Lane 2 never edits family editors.

## Worktrees and delivery

- One worktree and branch per issue, created from `origin/master` under the
  ignored `.worktrees/` (for example
  `git worktree add .worktrees/212-detection -b lane1/212-detection origin/master`).
  Never work in the main checkout. Never remove, reset, stash or clean another
  agent's worktree.
- Rebase onto `origin/master` before delivering.
- Run `cargo fmt --all -- --check`, `cargo clippy --offline --all-features
  --all-targets -- -D warnings` and `cargo test --offline --all-features`, plus
  the same Clippy and tests in `crates/line6-devices` and for
  `tools/variax-probe` when they are touched. Report the test counts before and
  after.
- Commit locally with focused commits. **No push, no PR and no GitHub posts.**
  The reviewer (Claude Code in the main session) reviews, tests the merge
  result, merges to `master` and updates the issues.
- Stop after each issue and report: commits, what changed, test counts, and
  anything that affects devices.
- Keep one issue in progress per agent. Target 30–60 minutes of agent time per
  issue; after two review rounds, propose a split.

## Boundaries

AGENTS.md governs hardware, gates and private data. In short: agents never run
the device tools or open USB, MIDI or audio devices; tests use fake transports
and injected sources and never read real `/proc` or `/sys`; the owner runs every
hardware check. Vendor packages, binaries, decompiled code, firmware, captures
and user data stay local and ignored. The UI art exception in AGENTS.md applies
to lane 2's `assets/ui/` only.

## Builds and scratch space

Build in your worktree under `.worktrees/` or set `CARGO_TARGET_DIR` to a folder on
`/home`. Do not build full copies of the project under `/tmp`: it is a 30 GB RAM disk
shared by every agent, and a full link there crashed `rust-lld` with SIGBUS on
2026-11-24 when it filled up. Delete your own scratch copies when you finish.
