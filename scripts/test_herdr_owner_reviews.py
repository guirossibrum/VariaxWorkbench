import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

from herdr_completion_handoff import bind_gatekeeper, hold_file
from herdr_owner_reviews import active_review, deliver_feedback, feedback, open_review, review_root, save_feedback


class FakeHerdr:
    def __init__(self):
        self.ready = False
        self.fail = False
        self.prompts = []

    def agents(self):
        return {"next": {"agent_status": "idle", "agent_session": {"id": "new"}}}

    def match(self, agents, binding, gatekeeper=False):
        return {"agent_status": "idle"} if self.ready else {"agent_status": "working"}

    def read(self, pane):
        return ""

    def prompt(self, pane, text):
        self.prompts.append((pane, text))
        if self.fail:
            raise RuntimeError("uncertain")


class OwnerReviewTests(unittest.TestCase):
    def setUp(self):
        temp = TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.config = Path(temp.name) / "config.json"
        self.repo = "/project"
        self.route = {"gatekeeper": {"pane": "plan", "session": {"id": "old"}}}
        self.routes = {self.repo: self.route}
        self.root = review_root(self.config, self.repo)
        self.herdr = FakeHerdr()
        self.path = open_review(self.config, self.routes, self.repo, "Check the logo.")

    def test_reply_reaches_plan_while_hold_stays_active(self):
        save_feedback(self.path, "Logo is visible.\nCheck passed.")
        self.assertTrue(hold_file(self.config, self.repo).exists())
        self.assertTrue(deliver_feedback(self.herdr, self.route, self.root))
        self.assertEqual(self.herdr.prompts, [])
        self.herdr.ready = True
        self.assertTrue(deliver_feedback(self.herdr, self.route, self.root))
        self.assertFalse(deliver_feedback(self.herdr, self.route, self.root))
        self.assertEqual(len(self.herdr.prompts), 1)
        self.assertIn("Logo is visible.\nCheck passed.", self.herdr.prompts[0][1])
        self.assertTrue(hold_file(self.config, self.repo).exists())

    def test_rebinding_preserves_feedback_and_hold(self):
        save_feedback(self.path, "Logo is visible.")
        self.herdr.ready = True
        self.herdr.fail = True
        with self.assertRaises(RuntimeError):
            deliver_feedback(self.herdr, self.route, self.root)
        self.assertEqual(next(feedback(self.path))[1]["status"], "sending")
        self.herdr.fail = False
        with patch("herdr_completion_handoff.Herdr", return_value=self.herdr):
            bind_gatekeeper(self.config, {"routes": self.routes}, self.repo, "next")
        route = json.loads(self.config.read_text())["routes"][self.repo]
        self.assertEqual(active_review(self.root), self.path)
        self.assertTrue(hold_file(self.config, self.repo).exists())
        self.assertTrue(deliver_feedback(self.herdr, route, self.root))
        self.assertEqual([pane for pane, _ in self.herdr.prompts], ["plan", "next"])
        self.assertEqual(next(feedback(self.path))[1]["status"], "delivered")

    def test_no_replacement_of_an_open_review(self):
        with self.assertRaisesRegex(ValueError, "already active"):
            open_review(self.config, self.routes, self.repo, "A second review")
        self.assertEqual(active_review(self.root), self.path)


if __name__ == "__main__":
    unittest.main()
