"""Checks for the PODxt Live 3.01 coverage record. No device and no firmware image."""

import json
import unittest
from pathlib import Path

from map_podxt_coverage import (
    EVIDENCE, GLOBAL_IDS, PROGRAM_END, PROGRAM_START, editor_contract,
)

ROOT = Path(__file__).resolve().parents[1]
RECORD = ROOT / "research" / "podxt-301-coverage.json"
ENTRY = ROOT / "research" / "podxt-301-entry.json"
FAMILY = ROOT / "crates" / "line6-devices" / "src" / "family" / "podxt" / "mod.rs"
IDENTITY = ROOT / "crates" / "line6-devices" / "src" / "identity.rs"


class CoverageClaimTests(unittest.TestCase):
    def test_evidence_addresses_are_inside_the_loaded_program(self):
        self.assertGreaterEqual(len(EVIDENCE), 8)
        for addr, text in EVIDENCE:
            self.assertGreaterEqual(addr, PROGRAM_START)
            self.assertLess(addr, PROGRAM_END)
            self.assertTrue(text)

    def test_editor_contract_matches_the_family_source(self):
        self.assertEqual(editor_contract(FAMILY.read_text()), GLOBAL_IDS)
        self.assertLessEqual(max(GLOBAL_IDS), 0x2F)
        self.assertTrue(all(index < 0x30 for index in GLOBAL_IDS))

    def test_checked_in_record_matches_the_locked_measurements(self):
        record = json.loads(RECORD.read_text())
        self.assertEqual(record["image_bytes"], 1048576)
        self.assertEqual(record["region_count"], 19)
        self.assertEqual(record["ff_bytes"], 517966)
        self.assertEqual(record["blank_region_bytes"], 409600)
        self.assertEqual(record["blank_region_bytes"] / record["image_bytes"], 0.390625)
        self.assertEqual(record["program_words"], 5816)
        self.assertEqual(record["marker_lines"], 28)
        self.assertEqual(record["direct_closure_words"], 1376)
        self.assertEqual(round(record["direct_closure_words"] / record["program_words"] * 100, 2), 23.66)
        self.assertEqual(record["indirect_transfers"], 26)
        self.assertEqual(record["indirect_transfers_in_closure"], 12)
        self.assertEqual(len(record["dispositions"]), 256)
        self.assertEqual(
            record["disposition_counts"],
            {
                "zero": 100,
                "return": 26,
                "traced_entry": 29,
                "outside": 86,
                "immediate_rts": 10,
                "receive_vector": 4,
                "mid_copy": 1,
            },
        )
        self.assertEqual(record["dispositions"]["0x0E"], "zero")
        self.assertEqual(record["dispositions"]["0x10"], "zero")
        self.assertEqual(record["dispositions"]["0x23"], "zero")
        self.assertEqual(record["dispositions"]["0x71"], "traced_entry")
        self.assertEqual(record["dispositions"]["0x75"], "traced_entry")
        self.assertEqual(record["dispositions"]["0xEC"], "mid_copy")
        self.assertEqual(record["dispositions"]["0xBF"], "receive_vector")
        self.assertEqual(record["global_id_max"], 34)
        self.assertEqual(record["regions"][17]["ff_bytes"], 4094)
        self.assertEqual(record["regions"][18]["ff_bytes"], 65536)
        self.assertEqual(record["target_confidence"], "confirmed")

    def test_identity_inquiry_is_outside_the_loaded_program(self):
        identity = IDENTITY.read_text()
        self.assertIn(
            "pub const REQUEST: [u8; 6] = [0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7]",
            identity,
        )
        source = FAMILY.read_text()
        body = source[source.index("fn bytes(self)"):source.index("fn path(self)")]
        self.assertIn("identity::REQUEST", body)
        record = json.loads(RECORD.read_text())
        entry = json.loads(ENTRY.read_text())
        self.assertEqual(record["dispositions"]["0x7E"], "outside")
        self.assertEqual(entry["targets"]["0x7E"], "0xAADF")


if __name__ == "__main__":
    unittest.main()
