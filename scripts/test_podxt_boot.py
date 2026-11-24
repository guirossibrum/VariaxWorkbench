"""Checks for the PODxt Live 3.01 boot record. No device and no firmware image."""

import json
import unittest
from pathlib import Path

from map_podxt_boot import (
    CHECKSUM_COUNT, CHECKSUM_START, EVIDENCE, GLOBAL_BASE, PROGRAM_END, PROGRAM_START,
)

RECORD = Path(__file__).resolve().parents[1] / "research" / "podxt-301-boot.json"


class BootClaimTests(unittest.TestCase):
    def test_evidence_addresses_are_inside_the_loaded_program(self):
        self.assertGreaterEqual(len(EVIDENCE), 20)
        for addr, text in EVIDENCE:
            self.assertGreaterEqual(addr, PROGRAM_START)
            self.assertLess(addr, PROGRAM_END)
            self.assertTrue(text)

    def test_checksum_window_and_global_indexes(self):
        self.assertEqual(CHECKSUM_COUNT, 0x80001)
        self.assertEqual(CHECKSUM_START + CHECKSUM_COUNT - 1, 0x01080000)
        self.assertEqual(GLOBAL_BASE + 2, 0x1B9AA)
        self.assertEqual(GLOBAL_BASE + 3, 0x1B9AB)
        self.assertEqual((1 << 19) & 0x80000, 0x80000)
        self.assertEqual((1 << 19) & 0x7FFFF, 0)

    def test_checked_in_record_matches_the_locked_measurements(self):
        record = json.loads(RECORD.read_text())
        self.assertEqual(record["evidence_count"], len(EVIDENCE))
        self.assertEqual(record["checksum_start"], "0x01000000")
        self.assertEqual(record["checksum_count"], "0x00080001")
        self.assertEqual(record["checksum_instructions"], ["0x826E", "0x8270"])
        self.assertEqual(record["r2_text_before_checksum"], 0)
        self.assertEqual(record["loaded_c2e5"], "0x00000000")
        self.assertEqual(record["c2e5_writers"], ["0x910E", "0x95C7"])
        self.assertEqual(record["counter_clear_entry"], "0x8028")
        self.assertEqual(record["target_confidence"], "confirmed")


if __name__ == "__main__":
    unittest.main()
