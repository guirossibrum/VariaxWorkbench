"""Consistency checks for the Variax 3.10 opcode attribution. No device and no image."""

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HISTORY = ROOT / "research/variax-opcode-history.json"
DISPATCH = ROOT / "research/variax-310-dispatch.json"
DOC = ROOT / "docs/firmware-variax-opcode-history.md"

HEADER = (0x00, 0x01, 0x0C, 0x07)
LABELS = {"confirmed", "likely", "unknown"}
FORBIDDEN = (
    "d05555",
    "d02aaa",
    "010000",
    "d10000",
    "aa82",
    "0x4000",
    "004000",
    "d1fff",
    "aa55",
)
ISSUE_UNUSED = {
    "0x3E",
    "0x40",
    "0x44",
    "0x45",
    "0x47",
    "0x49",
    "0x54",
    "0x56",
    "0x58",
    "0x59",
    "0x60",
    "0x62",
    "0x63",
    "0x67",
    "0x68",
    "0x6F",
    "0x7D",
}


def line6_opcode(message):
    """Opcode byte of a Line 6 family message.

    A framed message is F0, then 00 01 0C 07, then the member, then the opcode.
    A payload is the bytes the MIDI layer sends after that header, so its
    first byte is the opcode.
    """
    if not message:
        raise ValueError("empty message")
    if message[0] == 0xF0:
        if tuple(message[1:5]) != HEADER or len(message) < 7:
            raise ValueError("not a Line 6 family header")
        return message[6]
    return message[0]


def member_accepted(member):
    """3.10 enters the opcode table for member 0 or member 4."""
    return member in (0, 4)


class OpcodeHistoryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.history = json.loads(HISTORY.read_text())
        cls.dispatch = json.loads(DISPATCH.read_text())
        cls.doc = DOC.read_text()

    def test_opcode_byte_formula(self):
        framed = [0xF0, 0x00, 0x01, 0x0C, 0x07, 0x00, 0x54, 0xF7]
        self.assertEqual(line6_opcode(framed), 0x54)
        self.assertEqual(line6_opcode([0x56]), 0x56)
        self.assertEqual(line6_opcode([0xF0, 0x00, 0x01, 0x0C, 0x07, 0x04, 0x44]), 0x44)
        self.assertTrue(member_accepted(0))
        self.assertTrue(member_accepted(4))
        self.assertFalse(member_accepted(1))
        self.assertFalse(member_accepted(5))
        with self.assertRaises(ValueError):
            line6_opcode([0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7])

    def test_table_covers_the_nonreturn_handlers(self):
        self.assertEqual(self.history["version"], 1)
        self.assertEqual(self.history["product_id"], "00070004")
        self.assertEqual(self.history["firmware_version"], "3.10")
        expected = {
            opcode
            for opcode, target in self.dispatch["targets"].items()
            if target != "P:0x1483E"
        }
        self.assertEqual(len(expected), 46)
        rows = {
            row["opcode"]: row
            for row in self.history["commands"]
            if not str(row["opcode"]).startswith("universal")
        }
        self.assertEqual(set(rows), expected)
        for opcode, row in rows.items():
            self.assertEqual(row["handler"], self.dispatch["targets"][opcode])
            self.assertIn(row["sender_evidence"], LABELS)
            self.assertIn(row["purpose_evidence"], LABELS)
            self.assertIn(opcode, self.doc)
        universal = {
            row["opcode"]
            for row in self.history["commands"]
            if str(row["opcode"]).startswith("universal")
        }
        self.assertEqual(universal, {"universal-identity", "universal-6e-reply"})

    def test_senders_match_the_opcode_byte(self):
        for row in self.history["commands"]:
            if row["senders"]:
                self.assertEqual(row["no_sender_found_in"], [])
            else:
                self.assertGreaterEqual(len(row["no_sender_found_in"]), 8)
                self.assertIn("Variax Workbench 1.75 Windows decompile", row["no_sender_found_in"])
                self.assertIn("Line 6 Monkey 1.77 Windows", row["no_sender_found_in"])
            for sender in row["senders"]:
                self.assertIn(sender["evidence"], LABELS)
                if sender["opcode_byte"] is None:
                    self.assertTrue(str(row["opcode"]).startswith("universal"))
                    continue
                self.assertEqual(sender["opcode_byte"], int(row["opcode"], 16))
                self.assertTrue(member_accepted(sender["member"]))
                self.assertEqual(
                    line6_opcode([0xF0, 0x00, 0x01, 0x0C, 0x07, sender["member"], sender["opcode_byte"]]),
                    sender["opcode_byte"],
                )

    def test_issue_list_and_known_builders(self):
        rows = {row["opcode"]: row for row in self.history["commands"]}
        for opcode in ISSUE_UNUSED:
            self.assertTrue(rows[opcode]["issue_226_unused_list"])
        for opcode in ("0x54", "0x56", "0x60"):
            self.assertTrue(rows[opcode]["workbench_175_sends"])
        self.assertFalse(rows["0x44"]["workbench_175_sends"])
        self.assertEqual(rows["0x44"]["senders"][0]["name"], "#225 slot read")
        self.assertEqual(rows["0x3E"]["senders"], [])
        self.assertIn("member 1", rows["0x3E"]["acoustic_note"])
        self.assertEqual(rows["0x3F"]["senders"], [])
        self.assertIn("Monkey", rows["0x6F"]["senders"][0]["name"])
        self.assertIn("Mac 1.33", rows["0x6F"]["senders"][0]["versions"])
        self.assertIn("1.77", rows["0x6F"]["senders"][0]["versions"])
        direct = {row["opcode"] for row in self.history["direct_returns_sent_by_workbench_175"]}
        self.assertEqual(direct, {"0x1B", "0x3D", "0x48"})

    def test_the_note_names_the_search_and_leaves_podxt_empty(self):
        self.assertIn("No sender found in", self.doc)
        self.assertIn("## PODxt Live", self.doc)
        self.assertIn("#211 will add this section", self.doc)
        self.assertNotIn("PODxt Live 3.01 opcode", self.doc)
        for opcode in ISSUE_UNUSED:
            self.assertIn(f"`{opcode}`", self.doc)

    def test_committed_note_omits_code_load_recipes(self):
        text = (HISTORY.read_text() + DOC.read_text()).lower()
        for token in FORBIDDEN:
            self.assertNotIn(token, text)


if __name__ == "__main__":
    unittest.main()
