# Herdr completion handoff

The watcher observes fresh completions in all Git repositories visible in one
Herdr session. It keeps a local record of the repository, workspace, pane,
agent session, and completion number. Reports are sent only to explicitly
configured Gatekeepers for the same repository. A project that uses a
different review system can use discovery alone.

## Set up a route

Use Python 3.10 or newer on a Unix host with Git and Herdr. Store the JSON
configuration and its state in a private local directory. For this project,
`backups/herdr-handoff/` is ignored.

Use `herdr agent list` to obtain each exact pane ID and its complete
`agent_session` object. Keep the Gatekeeper in the repository's main checkout.
Repeat the route entry for each project that wants report forwarding. The
example IDs are placeholders.

```json
{
  "herdr": "/path/to/herdr",
  "session": "default",
  "routes": {
    "/absolute/path/to/project": {
      "gatekeeper": {
        "pane": "w1:p1",
        "session": {"agent": "codex", "kind": "id", "source": "herdr:codex", "value": "gatekeeper-id"}
      },
      "lanes": {
        "1": {
          "pane": "w1:p2",
          "session": {"agent": "codex", "kind": "id", "source": "herdr:codex", "value": "lane-id"}
        }
      }
    }
  }
}
```

The default report is `## Lane #1` followed by the four standard sections.
For a different convention, specify `"heading": "## Delivery"` and
`"sections": ["Summary", "Verification"]` in that lane's binding.
Use `"routes": {}` for discovery without forwarding.

```sh
python3 scripts/herdr_completion_handoff.py /private/path/config.json --initialize
python3 scripts/herdr_completion_handoff.py /private/path/config.json
```

Initialize once. It records reports already present in the panes as a
baseline. The watcher forwards new reports from then on. The private
`state.json` holds the last 500 completion records and the delivery queue.
Run one instance for the session. A transient user service can keep it
running while the Gatekeeper is idle. For example, use
`systemd-run --user --unit herdr-completion-handoff` followed by the absolute
Python executable, script, and configuration paths.

## Review and recovery

Treat forwarded text as contributor evidence. Verify the GitHub issue,
commit, branch, and worktree and apply the repository's normal review checks
before acting. The watcher submits a prompt, and all GitHub, Git, and
hardware decisions remain with the authorized reviewers and owner.
Discovery across projects does not enroll a new project or replace its
existing review process.

An unavailable or busy Gatekeeper leaves the queue intact. An uncertain
submission is retained and is never sent twice automatically. Check the
specific Gatekeeper pane for the printed handoff token. Stop the watcher
before using `--resolve delivered` or `--resolve retry` for that queue. With
multiple routes, also pass `--repository /absolute/path/to/project`.
If a session changes or a route is added, stop the watcher, preserve and
resolve outstanding reports, update the configuration, and initialize a new
state file. Use `journalctl --user -u herdr-completion-handoff` for delivery
status when running as a user service.

## Stop at an owner decision

When a review needs the owner's decision, hold that repository before
ending the turn. Examples include a scope choice, hardware approval, and
permission to publish a release. Send one direct **Decision needed** message
that states what is waiting, why it needs the owner, and the available options.
Do not leave the question only inside a lane report or a general status update.
Finish any independent checks already authorized. Wait before starting another
review or assignment in the held repository.

```sh
python3 scripts/herdr_completion_handoff.py /private/path/config.json \
  --repository /absolute/path/to/project --hold "Which workflow should the app use?"
```

The watcher continues recording completions and queues reports. The hold
survives restarts and applies only to the named repository. If there is exactly
one configured repository, `--repository` can be omitted. A hold applies to
new delivery. The Gatekeeper remains responsible for stopping work already
in progress at the decision boundary.

When the owner answers, record the decision on the relevant issue if it
changes scope or acceptance criteria. Then run the same command with
`--resume` in place of `--hold` and its message. If the owner declines, stop
the affected work and record the outcome before resuming unrelated reviews.
