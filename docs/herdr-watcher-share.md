# Share this Herdr watcher with another repository

The running service is `herdr-completion-handoff`. It watches Herdr's
`default` session on the host. It discovers completed turns across Git
repositories, including linked worktrees. VariaxWorkbench has a configured
forwarding route. Other projects in that session appear in the local
completion log and can enroll without changing the watcher code.

## Determine which agent finished

1. Use `herdr agent list` or `herdr agent get PANE_ID`. Bind the full
   `agent_session` object to the exact pane ID before assigning work.
2. Record the last numeric `completion_seq`. A later completion number for
   that session identifies a newly finished turn. Herdr can show the settled
   agent as `done` or `idle` depending on which client has marked it seen.
   Opening the pane does not invalidate the completion. `working` does not
   provide a finished-turn number.
3. Read that pane with
   `herdr agent read PANE_ID --source recent-unwrapped --lines 200`. A
   finished turn can be a report, question, or blocker. Read the result
   before deciding whether the issue is complete. A pane read does not mark
   the user's TUI badge as seen.

The watcher polls about every eight seconds. It keeps the last 500 completion
records, each with its repository, workspace, pane, agent session, and
completion number. It forwards a report only if the exact session and
repository still match, the report fits the configured template, and the
bound Gatekeeper is ready. The on-disk queue survives an agent replacement
or watcher restart.

## Reuse it in your project

- Use [the watcher](../scripts/herdr_completion_handoff.py) and
  [the owner review console](../scripts/herdr_owner_reviews.py) from the
  same revision and keep them in the same directory. It needs Python 3.10+,
  Git, Herdr, and access to the relevant local Herdr session.
- In an ignored, private configuration, use the repository's main checkout
  as the route key. Bind its Gatekeeper and contributor panes using the
  exact pane and session objects returned by Herdr. The
  [runbook](herdr-handoff.md#set-up-a-route) contains an example configuration
  and commands. Projects can choose their own report heading and section names.
- Reuse the running service for additional repos in the same Herdr session.
  Coordinate changes to the set of routes with its operator. Preserve the
  current state and outstanding reports first. For a separate Herdr session,
  use a separate private configuration, state directory, and service name.
- Discovery alone needs no project route. A forwarding route sends report
  text only to the configured Gatekeeper in that repository. It does not
  replace the project's review, CI, permission, or issue workflow.

## Holds, replacement, and manual recovery

- While a repository is held, completed lane reports queue. The separate
  **Owner reviews** pane saves human feedback and passes it to the plan pane
  even during the hold. The Gatekeeper records the outcome before resuming.
- To replace the planner, use `--bind-gatekeeper NEW_PANE_ID` against the
  existing private configuration. The command prints the pending review and
  saved feedback. Do not initialize a new state file for a planner swap.
- Inspect the service with
  `journalctl --user -u herdr-completion-handoff`. The private `state.json`
  shows discovery events and queued lane reports. Keep pane text, local
  configuration, and owner feedback private.
- Some agent terminals render Markdown and strip the literal heading
  markers. Herdr can detect the completion even when the report parser
  cannot forward it. Lane 2's rendered Grok report showed this behavior.
  In that case, read the completed pane directly. Use the runbook's recovery
  procedure for an uncertain send to avoid a duplicate.

A Gatekeeper still checks the current issue, exact commit, branch,
worktree, and verification evidence. The watcher does not merge code,
write to GitHub, publish builds, or grant approvals.
