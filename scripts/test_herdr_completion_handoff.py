import copy
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

from herdr_completion_handoff import baseline, change_hold, hold_file, observe, tick, report_from_screen

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
CONFIG = {"gatekeeper": {"pane": "gate"}, "lanes": {"4": {"pane": "lane"}}}


class FakeHerdr:
    def __init__(self):
        self.status = {"gate": {"agent_status": "working"},
                       "lane": {"agent_status": "done", "completion_seq": 2}}
        self.screen = {"lane": REPORT, "gate": ""}
        self.sent = []
        self.fail = False

    def agents(self):
        return copy.deepcopy(self.status)

    def match(self, agents, binding, gatekeeper=False):
        return agents.get(binding["pane"])

    def read(self, pane, visible=False):
        return self.screen[pane]

    def prompt(self, pane, text):
        self.sent.append((pane, text))
        if self.fail:
            raise RuntimeError("lost acknowledgement")


class HandoffTests(unittest.TestCase):
    def setUp(self):
        self.herdr = FakeHerdr()
        self.state = baseline(self.herdr, CONFIG)
        self.writes = []

    def poll(self):
        tick(self.herdr, CONFIG, self.state, lambda state: self.writes.append(copy.deepcopy(state)))

    def completion(self):
        self.herdr.status["lane"]["completion_seq"] = 3
        self.herdr.screen["lane"] = REPORT.replace("abcdef1", "abcdef2")

    def test_report_parser_accepts_only_complete_ordered_reports(self):
        self.assertIn("### Worktree handoff", report_from_screen(REPORT, 4))
        self.assertNotIn("Worked for", report_from_screen(REPORT, 4))
        self.assertIsNone(report_from_screen(REPORT, 3))
        self.assertIsNone(report_from_screen(REPORT.replace("### Result", "Result"), 4))

    def test_baseline_reads_only_visible_screen_when_lane_is_working(self):
        self.herdr.status["lane"]["agent_status"] = "working"
        with patch.object(self.herdr, "read", wraps=self.herdr.read) as read:
            baseline(self.herdr, CONFIG)
        read.assert_called_once_with("lane", visible=True)

    def test_baseline_and_stale_screen_are_not_delivered(self):
        self.herdr.status["gate"]["agent_status"] = "idle"
        self.poll()
        self.herdr.status["lane"]["completion_seq"] = 3
        self.poll()
        self.assertEqual(self.herdr.sent, [])

    def test_durable_queue_waits_for_ready_gatekeeper_and_deduplicates(self):
        self.completion()
        self.poll()
        self.assertEqual(self.herdr.sent, [])
        self.state = copy.deepcopy(self.writes[-1])
        self.herdr.status["gate"]["agent_status"] = "blocked"
        self.poll()
        self.assertEqual(self.herdr.sent, [])
        self.herdr.status["gate"]["agent_status"] = "idle"
        self.poll()
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)
        self.assertIn("abcdef2", self.herdr.sent[0][1])
        self.assertEqual(self.state["pending"], [])

    def test_held_repository_keeps_reports_and_resumes_once(self):
        self.completion()
        self.herdr.status["gate"]["agent_status"] = "idle"
        with TemporaryDirectory() as directory:
            config = Path(directory) / "config.json"
            routes = {"/work/held": CONFIG, "/work/other": CONFIG}
            change_hold(config, routes, "/work/held", "Owner chooses the default", False)
            marker = hold_file(config, "/work/held")
            self.assertFalse(hold_file(config, "/work/other").exists())
            tick(self.herdr, CONFIG, self.state, lambda _: None, held=marker.exists)
            self.assertEqual(self.herdr.sent, [])
            self.assertEqual(len(self.state["pending"]), 1)
            change_hold(config, routes, "/work/held", None, True)
            tick(self.herdr, CONFIG, self.state, lambda _: None, held=marker.exists)
            tick(self.herdr, CONFIG, self.state, lambda _: None, held=marker.exists)
            self.assertEqual(len(self.herdr.sent), 1)
            self.assertEqual(self.state["pending"], [])

    def test_missing_session_never_sends(self):
        self.completion()
        self.poll()
        self.herdr.status.pop("gate")
        self.poll()
        self.assertEqual(self.herdr.sent, [])
        self.assertEqual(len(self.state["pending"]), 1)

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
        with patch("herdr_completion_handoff.repository_for", side_effect=Path):
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

    def test_ambiguous_send_is_retained_without_replay(self):
        self.completion()
        self.herdr.status["gate"]["agent_status"] = "idle"
        self.herdr.fail = True
        with self.assertRaises(RuntimeError):
            self.poll()
        self.state = copy.deepcopy(self.writes[-1])
        self.poll()
        self.assertEqual(len(self.herdr.sent), 1)
        token = self.state["pending"][0]["token"]
        self.herdr.screen["gate"] = f"Received {token}"
        self.poll()
        self.assertEqual(self.state["pending"], [])
        self.assertEqual(len(self.herdr.sent), 1)


if __name__ == "__main__":
    unittest.main()
