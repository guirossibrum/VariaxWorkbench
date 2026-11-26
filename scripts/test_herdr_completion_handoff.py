import copy
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

from herdr_completion_handoff import (
    Herdr, baseline, change_hold, digest, hold_file, main, observe, operational_hold,
    report_from_screen, resolve_delivery, tick, uncertain_batches, write_state,
)

REPO = "/fixture/project"
WORKTREE = "/fixture/linked-worktree"
REPORT = """• ## Lane #4

  ### Result
  Issue #275, commit abcdef1.
  ### Verification
  Offline checks passed.
  ### Remaining work
  None.
  ### Worktree handoff
  I left the worktree and stopped local jobs.

  Worked for 2m
› Ask Codex to do anything
"""
GROK = """Lane #2                                                                    2:07 AM

Result
Issue #280, commit 1234567.
Verification
Offline checks passed.
Remaining work
None.
Worktree handoff
I left the worktree and stopped local jobs.
"""


def binding(pane, agent, identity):
    return {"pane": pane, "session": {
        "agent": agent, "kind": "id", "source": f"herdr:{agent}", "value": identity,
    }}


CONFIG = {
    "gatekeeper": binding("w1:p1", "codex", "gatekeeper"),
    "lanes": {
        "4": binding("w1:p2", "codex", "first"),
        "2": binding("w1:p3", "grok", "second"),
    },
}


def agent(binding, cwd, status, sequence=None):
    return {"pane_id": binding["pane"], "agent_session": binding["session"],
            "foreground_cwd": cwd, "agent_status": status, "completion_seq": sequence}


class FakeHerdr(Herdr):
    """Exercise the real binding and prompt adapter without a Herdr process."""

    def __init__(self):
        super().__init__({"herdr": "/unused", "session": "fixture"}, REPO)
        self.status = {
            "w1:p1": agent(CONFIG["gatekeeper"], REPO, "working"),
            "w1:p2": agent(CONFIG["lanes"]["4"], WORKTREE, "done", 2),
            "w1:p3": agent(CONFIG["lanes"]["2"], WORKTREE, "idle", 20),
        }
        self.screen = {"w1:p2": REPORT, "w1:p3": GROK, "w1:p1": ""}
        self.calls = []
        self.sent = []
        self.fail = False
        self.bad_ack = False
        self.no_ack_cwd = False

    def run(self, *args):
        self.calls.append(args)
        if args == ("list",):
            result = {"agents": list(self.status.values())}
            return json.dumps({"result": result})
        if args[0] == "read":
            return self.screen[args[1]]
        if len(args) != 3 or args[0] != "prompt":
            raise AssertionError(f"Unexpected Herdr call: {args}")
        _, pane, text = args
        self.sent.append((pane, text))
        if self.fail:
            raise subprocess.TimeoutExpired(args, 15)
        recipient = copy.deepcopy(self.status[pane])
        if self.bad_ack:
            recipient["agent_session"]["value"] = "replacement"
        if self.no_ack_cwd:
            recipient.pop("foreground_cwd")
        return json.dumps({"result": {"type": "agent_prompted", "agent": recipient}})


class HandoffTests(unittest.TestCase):
    def setUp(self):
        repo = patch("herdr_completion_handoff.repository_for", side_effect=self.repo_at)
        repo.start()
        self.addCleanup(repo.stop)
        self.herdr = FakeHerdr()
        self.config = copy.deepcopy(CONFIG)
        self.state = baseline(self.herdr, self.config)
        temp = TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.config_path = Path(temp.name) / "config.json"
        self.state_path = self.config_path.with_name("state.json")
        self.save(self.state)

    @staticmethod
    def repo_at(location):
        return Path(REPO) if str(location) == WORKTREE else Path(location)

    def save(self, state):
        write_state(self.state_path, state)

    def restart(self):
        self.state = json.loads(self.state_path.read_text())

    def poll(self, held=lambda: False):
        tick(self.herdr, self.config, self.state, self.save, held=held)

    def complete(self, lane="4"):
        pane = self.config["lanes"][lane]["pane"]
        self.herdr.status[pane]["completion_seq"] += 1
        self.herdr.screen[pane] = self.herdr.screen[pane].replace(
            "abcdef1" if lane == "4" else "1234567",
            "abcdef2" if lane == "4" else "1234568",
        )

    def test_parser_accepts_rendered_grok_title_with_time_and_ordered_sections(self):
        grok = report_from_screen(GROK, 2)
        self.assertTrue(grok.startswith("## Lane #2\n\n### Result\n"))
        self.assertIn("### Worktree handoff", grok)
        self.assertNotIn("2:07 AM", grok)
        self.assertIsNone(report_from_screen(GROK, 4))
        self.assertIsNone(report_from_screen(GROK.replace("Verification\n", ""), 2))
        self.assertIsNone(report_from_screen(GROK.replace("2:07 AM", "extra text"), 2))
        self.assertIn("### Worktree handoff", report_from_screen(REPORT, 4))
        self.assertNotIn("Worked for", report_from_screen(REPORT, 4))
        self.assertIsNone(report_from_screen(REPORT.replace("### Result", "Result"), 4))

    def test_baseline_reads_only_visible_screen_when_lane_is_working(self):
        self.herdr.status["w1:p2"]["agent_status"] = "working"
        self.herdr.calls.clear()
        baseline(self.herdr, self.config)
        self.assertIn(("read", "w1:p2", "--source", "visible"), self.herdr.calls)

    def test_baseline_and_rendered_stale_screen_are_not_delivered(self):
        self.herdr.status["w1:p3"]["completion_seq"] += 1
        self.herdr.screen["w1:p3"] = report_from_screen(GROK, 2)
        self.poll()
        self.assertEqual(self.herdr.sent, [])
        self.assertEqual(self.state["seen"]["2"]["sequence"], 21)

    def test_simultaneous_lanes_submit_one_ordinary_prompt_to_working_gatekeeper(self):
        self.complete("4")
        self.complete("2")
        self.poll()
        self.restart()
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)
        pane, prompt = self.herdr.sent[0]
        self.assertEqual(pane, "w1:p1")
        self.assertIn("abcdef2", prompt)
        self.assertIn("1234568", prompt)
        self.assertIn("Owner Inbox", prompt)
        self.assertIn(("prompt", pane, prompt), self.herdr.calls)
        self.assertEqual(self.herdr.status[pane]["agent_status"], "working")
        self.assertEqual(self.state["pending"], [])

    def test_blocked_and_unavailable_retain_queue_on_disk_until_restart(self):
        self.complete()
        gate = self.herdr.status.pop("w1:p1")
        self.poll()
        self.restart()
        self.assertEqual(len(self.state["pending"]), 1)
        self.herdr.status["w1:p1"] = dict(gate, agent_status="blocked")
        self.poll()
        self.assertEqual(self.herdr.sent, [])
        self.herdr.status["w1:p1"] = gate
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)
        self.assertEqual(self.state["pending"], [])

    def test_gatekeeper_requires_exact_session_and_main_checkout(self):
        self.complete()
        gate = self.herdr.status["w1:p1"]
        other_session = dict(gate["agent_session"], value="other")
        self.herdr.status["w1:p1"] = dict(gate, agent_session=other_session)
        self.poll()
        self.herdr.status["w1:p1"] = dict(gate, foreground_cwd=WORKTREE)
        self.poll()
        self.assertEqual(self.herdr.sent, [])
        self.assertEqual(len(self.state["pending"]), 1)

    def test_gatekeeper_becoming_blocked_after_save_leaves_batch_queued(self):
        self.complete()

        def block_after_save(state):
            self.save(state)
            if any(item["status"] == "sending" for item in state["pending"]):
                self.herdr.status["w1:p1"]["agent_status"] = "blocked"

        tick(self.herdr, self.config, self.state, block_after_save)
        self.restart()
        self.assertEqual(self.herdr.sent, [])
        self.assertEqual([item["status"] for item in self.state["pending"]], ["queued"])

    def test_lane_requires_same_repository_and_session_and_rechecks_after_read(self):
        self.complete()
        lane = self.herdr.status["w1:p2"]
        self.herdr.status["w1:p2"] = dict(lane, foreground_cwd="/other")
        self.poll()
        self.herdr.status["w1:p2"] = lane
        read = self.herdr.read

        def change_identity(pane):
            value = read(pane)
            self.herdr.status[pane] = dict(lane, agent_session={"value": "new"})
            return value

        with patch.object(self.herdr, "read", side_effect=change_identity):
            self.poll()
        self.assertEqual(self.state["seen"]["4"]["sequence"], 2)
        self.assertEqual(self.herdr.sent, [])

    def test_missing_report_remains_eligible_at_same_sequence(self):
        self.complete()
        complete = self.herdr.screen["w1:p2"]
        self.herdr.screen["w1:p2"] = ""
        self.poll()
        self.restart()
        self.assertEqual(self.state["seen"]["4"]["unread_sequence"], 3)
        self.herdr.screen["w1:p2"] = complete
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)
        self.assertNotIn("unread_sequence", self.state["seen"]["4"])

    def test_ambiguous_batch_survives_restart_and_does_not_block_fresh_reports(self):
        self.complete("4")
        self.complete("2")
        self.herdr.fail = True
        with self.assertRaises(subprocess.TimeoutExpired):
            self.poll()
        self.restart()
        batches = uncertain_batches(self.state)
        self.assertEqual(len(batches), 1)
        token, batch = next(iter(batches.items()))
        self.assertEqual(len(batch), 2)
        self.assertEqual(batch[0]["recipient"], self.config["gatekeeper"])
        self.herdr.fail = False
        self.herdr.status["w1:p2"]["completion_seq"] += 1
        self.herdr.screen["w1:p2"] = REPORT.replace("abcdef1", "abcdef3")
        self.poll()
        self.assertEqual(len(self.herdr.sent), 2)
        self.assertIn("abcdef3", self.herdr.sent[-1][1])
        self.assertNotIn("1234568", self.herdr.sent[-1][1])
        self.assertEqual(uncertain_batches(self.state), batches)
        self.herdr.screen["w1:p1"] = f"› Herdr report batch {token}\n"
        self.poll()
        self.assertEqual(uncertain_batches(self.state), batches)
        self.herdr.screen["w1:p1"] = f"• Received Herdr batch {token}\n"
        self.poll()
        self.assertEqual(self.state["pending"], [])
        self.assertEqual(len(self.herdr.sent), 2)

    def test_mismatched_ack_and_rebinding_cannot_confirm_ambiguous_batch(self):
        self.complete()
        self.herdr.bad_ack = True
        with self.assertRaisesRegex(RuntimeError, "acknowledgement"):
            self.poll()
        self.restart()
        token, = uncertain_batches(self.state)
        replacement = binding("w1:p1", "codex", "replacement")
        self.config["gatekeeper"] = replacement
        self.herdr.status["w1:p1"] = agent(replacement, REPO, "working")
        self.herdr.screen["w1:p1"] = f"Received Herdr batch {token}\n"
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)
        self.assertIn(token, uncertain_batches(self.state))
        resolve_delivery(self.state, "delivered", token)
        self.assertEqual(self.state["pending"], [])

    def test_ack_without_cwd_uses_fresh_exact_repository_check(self):
        self.complete()
        self.herdr.no_ack_cwd = True
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)
        self.assertEqual(self.herdr.calls[-1], ("list",))
        self.assertEqual(self.state["pending"], [])

    def test_legacy_sending_record_retained_for_manual_resolution(self):
        prior = {"token": "herdr-handoff-4-9-old", "report": REPORT, "status": "sending"}
        self.state.pop("lane_bindings")
        self.state["pending"].append(prior)
        self.poll()
        self.restart()
        self.assertEqual(self.state["pending"], [prior])
        self.assertEqual(self.state["lane_bindings"], self.config["lanes"])
        resolve_delivery(self.state, "retry", None)
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)

    def test_operational_hold_remains_but_stale_legacy_review_does_not_hold(self):
        routes = {REPO: self.config}
        self.complete()
        legacy = "Owner review in the dedicated review pane"
        change_hold(self.config_path, routes, REPO, legacy, False, operational=True)
        self.restart()
        self.poll(held=lambda: operational_hold(self.config_path, REPO))
        self.assertEqual(self.herdr.sent, [])
        change_hold(self.config_path, routes, REPO, None, True)
        change_hold(self.config_path, routes, REPO, legacy, False)
        self.assertTrue(hold_file(self.config_path, REPO).exists())
        self.poll(held=lambda: operational_hold(self.config_path, REPO))
        self.assertEqual(len(self.herdr.sent), 1)

    def test_main_does_not_read_or_send_legacy_review_while_forwarding(self):
        routes = {REPO: self.config}
        self.complete()
        self.config_path.write_text(json.dumps({"routes": routes, "herdr": "/unused", "session": "fixture"}))
        write_state(self.state_path, {"observed": {}, "completions": [], "routes": {REPO: self.state}})
        legacy = self.config_path.parent / "reviews" / digest(REPO)
        legacy.mkdir(parents=True)
        (legacy / "active.json").write_text("invalid and intentionally unread")
        with patch("herdr_completion_handoff.Herdr", return_value=self.herdr), patch(
            "sys.argv", ["watcher", str(self.config_path), "--once"],
        ):
            main()
        self.assertEqual(len(self.herdr.sent), 1)
        self.assertEqual((legacy / "active.json").read_text(), "invalid and intentionally unread")

    def test_manual_recovery_requires_exact_batch_when_several_are_uncertain(self):
        self.state["pending"] = [
            {"status": "sending", "batch": "first", "token": "report-1"},
            {"status": "sending", "batch": "first", "token": "report-2"},
            {"status": "sending", "batch": "second", "token": "report-3"},
        ]
        with self.assertRaisesRegex(ValueError, "specify --delivery"):
            resolve_delivery(self.state, "retry", None)
        resolve_delivery(self.state, "delivered", "first")
        self.assertEqual(list(uncertain_batches(self.state)), ["second"])

    def test_main_rejects_different_herdr_session_for_existing_state(self):
        routes = {REPO: self.config}
        self.config_path.write_text(json.dumps({"routes": routes, "herdr": "/unused", "session": "other"}))
        write_state(self.state_path, {
            "observed": {}, "completions": [], "routes": {REPO: self.state},
            "herdr_session": "fixture",
        })
        with patch("herdr_completion_handoff.Herdr", return_value=self.herdr), patch(
            "sys.argv", ["watcher", str(self.config_path), "--once"],
        ), patch("sys.stderr"), self.assertRaises(SystemExit):
            main()
        self.assertEqual(self.herdr.sent, [])
        self.assertEqual(json.loads(self.state_path.read_text())["herdr_session"], "fixture")

    def test_batch_limit_splits_by_bytes_without_waiting_for_review(self):
        self.complete("4")
        self.complete("2")
        for pane in ("w1:p2", "w1:p3"):
            self.herdr.screen[pane] = self.herdr.screen[pane].replace("None.", "🎸" * 250)
        with patch("herdr_completion_handoff.MAX_BATCH_BYTES", 2000):
            self.poll()
            self.assertEqual(len(self.state["pending"]), 1)
            self.poll()
        self.assertEqual(len(self.herdr.sent), 2)
        self.assertEqual(self.state["pending"], [])

    def test_unconfigured_repositories_receive_metadata_and_are_deduplicated(self):
        state = {"observed": {}, "completions": []}
        agents = {
            "alpha": {"pane_id": "alpha", "workspace_id": "w1",
                      "agent_session": {"value": "first"}, "completion_seq": 140,
                      "agent_status": "done", "foreground_cwd": "/work/alpha"},
            "beta": {"pane_id": "beta", "workspace_id": "w2",
                     "agent_session": {"value": "second"}, "completion_seq": 141,
                     "agent_status": "idle", "foreground_cwd": "/work/beta"},
        }
        results = observe(agents, state)
        self.assertEqual(observe(agents, state), [])
        self.assertEqual([row["repository"] for row in results], ["/work/alpha", "/work/beta"])
        self.assertEqual([row["pane"] for row in state["completions"]], ["alpha", "beta"])
        self.assertNotIn("report", results[0])

    def test_a_repo_can_configure_its_own_report_format(self):
        custom = {"heading": "## Delivery", "sections": ["Summary"]}
        self.assertEqual(
            report_from_screen("## Delivery\n### Summary\nReviewed.", "release", custom),
            "## Delivery\n\n### Summary\nReviewed.",
        )


if __name__ == "__main__":
    unittest.main()
