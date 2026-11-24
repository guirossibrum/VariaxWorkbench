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
| **1. Shell and devices** | `src/app/` (shell, registry, detection), `src/tools/device_check/` and Device check code, `crates/line6-devices` session, gate, transport, identity and backup code, the device lock in `tools/reso5-test` and `tools/variax-probe` (lock only) | #212 (3b detection) → #213 (3c identity, Refresh guitar, device lock; covers #201) → #214 (device-library tidy) → #228 (Device check: drop Simulation mode, scrolling) |
| **2. Design and layout** | `docs/design/`, `src/shared/ui/` (theme and components), `assets/ui/` (committed art); after #213, the visuals of `src/app/shell/` | #215 (specification and mockups, **owner approval before code**) → #216 (theme and component library) → #217 (shell restyle) |
| **3. Variax editor** | `src/families/variax/` (editor and file) | #192, #193, #194, #195, #196, #197, #198, #199, #200 → after #216: #188, #189, #190, #191, #224 (restyle) → after #213: #203, #204, #206 → #205 (Sync, after the store tests #177/#178); #230, #231 (from #227) after #200 |
| **4. PODxt editor** | `src/families/podxt/`, `crates/line6-devices/src/family/podxt/` | #210 (parameter table) → #218 → #219 (after #216) → #220 (after #213) → #221 → #222 (after #211 items 3–4) → #223 |
| **5. Variax device tests** | `crates/line6-devices/src/family/variax/` (read and write messages), `tools/reso5-test/` except its device lock (lane 1) | #225 (read-only `0x44` slot read) → #177 store-stage additions (issue/177-store-checks) → #229 (saved-tone flag in restore matching, before #178). Run by the former #177 agent since 2026-11-24. |
| Research | `docs/firmware-*.md`, `research/`, `scripts/` (own files only) | #226 (Variax opcode history, Grok), #211 (PODxt Live 3.01 firmware), #232 (PODxt Live Variax control ownership, after #211) |

Shared files: `README.md` (one ledger line per delivered item, in your lane's
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
