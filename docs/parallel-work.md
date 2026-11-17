# Parallel work ownership

The user authorized independent agents using disposable worktrees. GitHub issues
and the project remain the progress/dependency source of truth; this file records
ownership and integration boundaries, not a replacement task queue.

## Current coordination policy

The existing Codex and Grok implementation milestones in PRs #81, #82 and #83
are integrated. Both agents now act as Gatekeepers; new implementation belongs
to the four registered PR1–PR4 VariaxWorkbench contributors through
agent-coordinator. No current PR is adopted into a contributor assignment.
Assignments start only after the coordinator owner confirms the repository/CI/
dispatcher repair and the exact scoped registration/contract is verified.

The agreed tracks are coefficient validation, POD Farm research followed by POD
editing, connector/Workbench HD research followed by the HD backend, and visual
research followed by remaining HD500X work. Native prerequisites and unresolved
source/input gaps still apply. Issue #5 remains open, so #6 must not be presented
as unblocked merely because PR82 merged. Issues #33/#22 stay late-stage gates;
no live operations are authorized. GitHub issues/project and AC receipts supply
the actual execution state; this description does not itself assign work.

## Prior implementation ownership

The following records the earlier manual ownership and integrated milestones.
It does not authorize Codex or Grok to start another direct implementation.

- Codex owns the coefficient engine and all issues other than those reserved below.
  Last direct implementation: #5 historical math-runtime milestone PR82, integrated without changing the default profile; unresolved accuracy criteria stay open. #69 exact rational source-clock AIFF export is integrated. #72 unsaved offline recording-take quit protection is integrated. #19 firmware-improvement feasibility assessment is integrated; no deployable firmware change is justified. #18 bounded receiver/update state simulation is integrated; physical peripherals and recovery remain unsupported. #16 source-mapping research is integrated, with physical recovery explicitly unverified. #15 saved-reference comparison is integrated. #14 archived extraction and the known global-level structure are integrated. #13 static firmware baseline mapping is integrated. #10 research reproducibility and #11 Linux packaging are integrated. #7 parameter domains, #8 bank/collection editing and #9 offline GUI workflow coverage are integrated. #5 portable-math implementation is integrated; its historical-runtime/default-rollout validation remains open. #3 and #4 are integrated.
- Grok completed #17, #12 and the offline investigation in #21 and architecture in #26, plus offline routing in #27 and recording/file workflow in #28. #20 remains
  reserved with its recorded artifact/version unknowns; new evidence must not
  cause a live query. The #29 librarian and bounded missing-domain search are integrated; validated parameter editing remains pending independent definitions. Last direct implementation: #32 independent POD HD500X research milestones PR81/83, integrated; the backend remains incomplete. Codex keeps
  #16 updater/recovery research and the other unreserved issues.


## Worktree and delivery rules

Each contributor creates a separate branch/worktree per issue, outside the primary
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

Finish each assignment with tests, evidence and remaining limitations, commit/push
the issue branch and open a focused PR to the configured base. Return the exact
head through the AC receipt. Gatekeepers review and return corrections or approve
coordinator CI; the coordinator owns the configured merge/cleanup sequence. Do not
manually bypass that flow or erase retained research evidence. Preserve prior
Grok/Codex worktrees until their owner explicitly releases them.

All agents retain the absolute hardware boundary in AGENTS.md: no live USB/MIDI,
audio capture/playback, probing/enumeration, driver actions, presets/reset/update
operations, or lock overrides. Vendor packages, extracted binaries, captures,
credentials and user presets stay ignored/local. Only synthetic test data and
permitted provenance/derived documentation may be committed. Passing offline
checks never authorizes hardware operations.

## Required coordinated role reading

Before further coordinated action, contributors must read [the contributor role](roles/contributor.md), and Gatekeepers must read [the Gatekeeper role](roles/gatekeeper.md). Both roles must explicitly load the versioned generic review skill named by their delivery. These role documents preserve all hardware and private-input boundaries; they do not activate a missing coordinator policy or grant work, CI or merge authority.
