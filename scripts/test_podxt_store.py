"""Checks for the PODxt Live 3.01 store record. No device and no firmware image."""

import json
import unittest
from pathlib import Path

from map_podxt_store import (
    EVIDENCE, HOST_SIZES, PROGRAM_END, PROGRAM_START, SENTINEL_OFFSET, SLOT_COUNT, SLOT_WORDS,
    host_address, host_addresses,
)

RECORD = Path(__file__).resolve().parents[1] / "research" / "podxt-301-store.json"


class StoreClaimTests(unittest.TestCase):
    def test_evidence_addresses_are_inside_the_loaded_program(self):
        self.assertGreaterEqual(len(EVIDENCE), 40)
        for addr, text in EVIDENCE:
            self.assertGreaterEqual(addr, PROGRAM_START)
            self.assertLess(addr, PROGRAM_END)
            self.assertTrue(text)

    def test_host_addresses_fit_the_scanned_slots(self):
        addresses = host_addresses()
        self.assertEqual(len(addresses), sum(HOST_SIZES.values()))
        self.assertEqual(max(addresses), 298)
        self.assertEqual(host_address("tones", 0), 0)
        self.assertEqual(host_address("tones", 64), 192)
        self.assertEqual(host_address("amp", 0), 64)
        self.assertEqual(host_address("amp", 64), 256)
        self.assertEqual(host_address("fx", 0), 128)
        self.assertEqual(host_address("fx", 63), 191)
        self.assertTrue(all(index * SLOT_WORDS + SLOT_WORDS <= SLOT_COUNT * SLOT_WORDS for index in addresses))
        self.assertEqual(409 * SLOT_WORDS + 0x2F, SENTINEL_OFFSET)
        self.assertEqual(409 * SLOT_WORDS + SLOT_WORDS, 0x8020)
        self.assertNotEqual(SENTINEL_OFFSET % SLOT_WORDS, 0)

    def test_checked_in_record_matches_the_locked_measurements(self):
        record = json.loads(RECORD.read_text())
        self.assertEqual(record["evidence_count"], len(EVIDENCE))
        self.assertEqual(record["loaded_c2df"], "0x01070000")
        self.assertEqual(record["loaded_c2d6"], "0x00000000")
        self.assertEqual(record["c2df_writers"], ["0x9378", "0x9387", "0x9390"])
        self.assertEqual(record["slot_words"], 80)
        self.assertEqual(record["slot_count"], 384)
        self.assertEqual(record["lower_first_bytes"], "506c")
        self.assertEqual(record["lower_last_bytes"], "1969")
        self.assertTrue(record["upper_all_ff"])
        self.assertEqual(record["lower_blank_slots"], 0)
        self.assertEqual(record["upper_blank_slots"], 384)
        self.assertEqual(record["host_address_max"], 298)
        self.assertEqual(record["sentinel_index"], 409)
        self.assertEqual(record["global_straight_line_flash_targets"], 0)
        self.assertFalse(record["opcode_0x71_erases"])
        self.assertEqual(record["file_mapping"], "likely")
        self.assertEqual(record["target_confidence"], "confirmed")


if __name__ == "__main__":
    unittest.main()
