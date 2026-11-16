# Parallel work ownership

The user authorized independent agents using disposable worktrees. GitHub issues
and the project remain the progress/dependency source of truth; this file records
ownership and integration boundaries, not a replacement task queue.

- Codex owns the coefficient engine and all issues other than those reserved below.
  Current implementation: #14 factory/calibration extraction. #13 static firmware baseline mapping is integrated. #10 research reproducibility and #11 Linux packaging are integrated. #7 parameter domains, #8 bank/collection editing and #9 offline GUI workflow coverage are integrated. #5 portable-math implementation is integrated; its historical-runtime/default-rollout validation remains open. #3 and #4 are integrated.
- Grok completed #17, #12 and the offline investigation in #21 and architecture in #26. #20 remains
  reserved with its recorded artifact/version unknowns; new evidence must not
  cause a live query. Current implementation: #27 offline POD USB audio routing and synthetic streams,
  in a new disposable worktree. Codex keeps
  #13 firmware mapping and the other unreserved issues.


Each agent creates a separate branch/worktree per issue, outside the primary
checkout's source directories (for example, under its ignored `.worktrees/`).
Keep one implementation issue active per agent and respect dependencies. Never
stash, reset, clean or overwrite another checkout's changes. Do not share build
output directories between agents. Disposable means removable after integration
and verification, not disposable before commits/results have been preserved.

Grok owns implementation/research for its four issues. Codex does not duplicate
those tasks. Prefer module-specific files and focused documentation. Necessary
Cargo/lib wiring may change on a branch; keep README changes scoped to the
assigned feature and avoid rewriting global engine counts or unrelated status.
Codex reconciles the shared README ledger/backlog index during integration.

Finish each issue with tests, evidence and remaining limitations, commit/push the
issue branch and open a focused PR to `master`. Codex reviews and merges completed
issue work one at a time, then updates shared status and cleans only its own
finished worktrees. Preserve Grok worktrees until their owner releases them.

All agents retain the absolute hardware boundary in AGENTS.md: no live USB/MIDI,
audio capture/playback, probing/enumeration, driver actions, presets/reset/update
operations, or lock overrides. Vendor packages, extracted binaries, captures,
credentials and user presets stay ignored/local. Only synthetic test data and
permitted provenance/derived documentation may be committed. Passing offline
checks never authorizes hardware operations.
