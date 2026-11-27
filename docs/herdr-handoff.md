# Herdr completion handoff

**Historical reference.** The owner stopped and disabled the Herdr
Dispatcher on November 27, 2026. Use
[direct Herdr handoffs](parallel-work.md#herdr-handoffs-for-gatekeepers).
Keep the service disabled and use its saved records only to reconcile old
deliveries. The deployment procedures below describe the former setup.

The watcher observes fresh completions in all Git repositories visible in one
Herdr session. It keeps a local record of the repository, workspace, pane,
agent session, and completion number. Reports are sent only to explicitly
configured Gatekeepers for the same repository. A project that uses a
different review system can use discovery alone.

For a setup and operating overview to share with other repositories, see
[the watcher guide](herdr-watcher-share.md).

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

Use the report format only for completed deliveries ready for review. The
default report is `## Lane #1` followed by the four standard sections.
Questions, progress updates, and blockers are ordinary conversation.
The watcher also accepts their rendered plain-text form, including a Grok
title such as `Lane #2` with a right-aligned time. It preserves closed
box-drawn tables inside the report, stops at terminal prompts and footers,
and normalizes the headings before checking for a duplicate.
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

## Batch delivery

On each poll the watcher collects all fresh reports for a repository before
submitting one ordinary `herdr agent prompt` to its bound Gatekeeper. A
working Gatekeeper can receive this prompt using the agent's normal input
behavior. The watcher sends no interrupt key and uses no lifecycle wait.
A successful acknowledgement confirms that Herdr submitted the prompt,
with review and action still pending. A blocked or unavailable Gatekeeper
leaves the saved reports queued. If a batch exceeds 64 KiB of UTF-8 text,
the watcher logs the split and forwards the remainder on subsequent polls.

The configured lane and repository bindings are checked before and after
reading a report. The watcher uses the session identity and increasing
completion sequence for both `idle` and `done`, regardless of who viewed
the pane. The watcher considers the latest matching title in the returned
text and requires every section to contain content. Herdr supplies a pane
snapshot and a separate completion sequence, with no per-turn identifier
on the text. If the first read is incomplete, unavailable, or fails its
binding recheck, the watcher records that sequence as unread and holds it
for manual review. A later view can show an older complete report under
the same sequence. The watcher leaves that sequence held across polls and
restarts. A newer completion can still be forwarded. Keep the saved
completion markers when reviewing the unread report manually.

Treat forwarded text as contributor evidence. Verify the GitHub issue,
commit, branch, and worktree and apply the repository's normal offline
review checks. All GitHub, Git, and hardware decisions remain with the
authorized reviewers and owner.

For VariaxWorkbench, complete the
[Gatekeeper handoff checklist](parallel-work.md#herdr-handoffs-for-gatekeepers)
in the same coordination cycle as each verified merge. The checklist covers
worktree cleanup, Dispatcher release, and verified receipt of the next
assignment or a reported blocker and next action.

## Owner decisions and operational pauses

Use the production **Owner Inbox** for requests and replies. Stop the
dependent action at the decision boundary and continue independent
authorized work, including unrelated lane reviews. The completion watcher
does not read or deliver owner feedback. Old review pointers and the
legacy hold automatically written by the retired review pane have no
effect on forwarding. Archived review records are left in place.

For a deliberate pause of all delivery for a repository, use the separate
operational control. The hold survives restart and pending reports remain
on disk. It applies to the named repository only.

```sh
python3 scripts/herdr_completion_handoff.py /private/path/config.json \
  --repository /absolute/path/to/project --hold "Gatekeeper maintenance"
python3 scripts/herdr_completion_handoff.py /private/path/config.json \
  --repository /absolute/path/to/project --resume
```

## Recover an uncertain submission

The private state records all captured reports and marks a batch as sending
with its token and exact recipient before calling Herdr. A timeout, lost
acknowledgement, or restart can leave the batch in that state. The watcher
checks for the Gatekeeper's `Received Herdr batch TOKEN` response in the
original bound pane and session before automatically clearing it. An echoed
prompt or draft leaves the batch uncertain. Uncertain batches are never
retried automatically and fresh reports can still be sent. A sending
record from the older watcher has no
recorded recipient and needs manual reconciliation.

Herdr 0.9.5's prompt API takes a pane target without a session precondition.
The watcher rechecks the exact session and main checkout before submitting
and verifies the identity returned in the acknowledgement. A pane
replacement between these operations remains possible. A missing or
mismatched acknowledgement leaves the batch uncertain.

Use `journalctl --user -u herdr-completion-handoff` and private `state.json`
to find the batch token and its recipient. Check that recipient pane for the
complete submission. If the pane history is unavailable or the outcome is
unclear, keep the record for recovery. Stop the watcher before resolving a
confirmed submission or an independently verified failure to submit.

```sh
python3 scripts/herdr_completion_handoff.py /private/path/config.json \
  --repository /absolute/path/to/project --resolve delivered --delivery BATCH_TOKEN
```

Use `--resolve retry` instead to requeue the complete batch after establishing
it was not submitted. When only one delivery is uncertain in the selected
repository, `--delivery` can be omitted. A legacy single-report record uses
its report token.

If the named Herdr session, a lane binding, or the set of repository routes
changes, stop the watcher, preserve and resolve outstanding reports, update
the configuration and initialize a new state file. A restart with the same
bindings keeps captured reports. The previous state format picks up the
current session and lane bindings on first use. A completion that the
older watcher recorded without capturing its report needs manual recovery.

### Replace the Gatekeeper

Open the replacement agent in the project's main checkout and find its
exact pane with `herdr agent list`. Run the following against the existing
private configuration:

```sh
python3 scripts/herdr_completion_handoff.py /private/path/config.json \
  --bind-gatekeeper NEW_PANE_ID
```

The watcher validates the replacement and picks up the new binding. Queued
reports can go to the new Gatekeeper. Uncertain submissions stay associated
with their original recipient. Preserve state instead of reinitializing.
