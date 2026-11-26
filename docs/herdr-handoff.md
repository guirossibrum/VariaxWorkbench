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
permission to publish a release. Post a **Decision needed** request in
**Owner reviews** stating what is waiting, why it needs the owner, and the
available options. Finish any independent checks already authorized. Wait
before starting another review or assignment in the held repository.

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

## Dedicated owner reviews pane

Keep planning and owner feedback in separate Herdr panes. Create an
**Owner reviews** tab in the project workspace and run the review console in
its shell. The console uses the same private configuration as the watcher.

```sh
python3 scripts/herdr_owner_reviews.py /private/path/config.json --pane
```

To request a milestone review, write a short Markdown file naming the build,
issue, what to check, and what reply you need. Open that request:

```sh
python3 scripts/herdr_owner_reviews.py /private/path/config.json --open /private/path/review.md
```

The request is copied into the private review inbox and ordinary lane
handoffs are held. The owner types feedback in **Owner reviews**, then types
`:send` on its own line and presses Enter. The feedback stays on disk and is
sent to the plan pane when its Gatekeeper is ready. The feedback path works
during a lane hold and does not release the hold. After applying and recording
the owner's feedback, close the review with `--close` and resume from the
completion watcher with `--resume`. Use `--repository` when more than one
project has a route.

### Replace the planning agent

Open the replacement agent in the project's main checkout. Find its exact pane
with `herdr agent list`. Run the following once against the existing private
configuration:

```sh
python3 scripts/herdr_completion_handoff.py /private/path/config.json \
  --bind-gatekeeper NEW_PANE_ID
```

This validates the replacement session and prints any active request and
saved feedback. The running watcher picks up the new binding. The review
pane, holds, saved feedback, and pending lane queue survive. Do not run
`--initialize` when replacing the planner. Check the open review before
starting another task and use `herdr_owner_reviews.py ... --show` to read the
saved request and feedback again.
