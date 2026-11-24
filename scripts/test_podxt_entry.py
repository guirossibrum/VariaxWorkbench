"""Checks for the PODxt Live 3.01 entry inventory. No device and no firmware image."""

import json
import unittest
from pathlib import Path

from map_podxt_entry import classify_target, shannon_entropy

INVENTORY = Path(__file__).resolve().parents[1] / "research" / "podxt-301-entry.json"
PROGRAM_START = 0x8000
PROGRAM_END = 0x96B8
RETURN_WORD = 0x80CE


class EntryHelperTests(unittest.TestCase):
    def test_entropy_of_a_constant_buffer_is_zero(self):
        self.assertEqual(shannon_entropy(b"\xff" * 32), 0.0)
        self.assertEqual(shannon_entropy([0, 1] * 8), 1.0)

    def test_target_classes_follow_the_program_span_and_the_return_word(self):
        self.assertEqual(classify_target(0, PROGRAM_START, PROGRAM_END, RETURN_WORD), "zero")
        self.assertEqual(classify_target(RETURN_WORD, PROGRAM_START, PROGRAM_END, RETURN_WORD), "return")
        self.assertEqual(classify_target(0x8187, PROGRAM_START, PROGRAM_END, RETURN_WORD), "program")
        self.assertEqual(classify_target(0xAA76, PROGRAM_START, PROGRAM_END, RETURN_WORD), "outside_program")
        self.assertEqual(classify_target(PROGRAM_END, PROGRAM_START, PROGRAM_END, RETURN_WORD), "outside_program")


class CheckedInventoryTests(unittest.TestCase):
    def setUp(self):
        self.inventory = json.loads(INVENTORY.read_text())

    def test_class_counts_match_the_recorded_words(self):
        counts = {"zero": 0, "return": 0, "program": 0, "outside_program": 0}
        targets = self.inventory["targets"]
        self.assertEqual(len(targets), 256)
        for opcode in range(256):
            word = int(targets[f"0x{opcode:02X}"], 16)
            kind = classify_target(word, PROGRAM_START, PROGRAM_END, RETURN_WORD)
            counts[kind] += 1
        self.assertEqual(counts, self.inventory["class_counts"])
        self.assertEqual(sum(counts.values()), 256)

    def test_editor_opcodes_keep_their_recorded_entries(self):
        targets = self.inventory["targets"]
        self.assertEqual(targets["0x0E"], "0x0000")
        self.assertEqual(targets["0x0F"], "0x0000")
        self.assertEqual(targets["0x10"], "0x0000")
        self.assertEqual(targets["0x11"], "0x0000")
        self.assertEqual(targets["0x13"], "0x0000")
        self.assertEqual(targets["0x23"], "0x0000")
        self.assertEqual(targets["0x24"], "0x0000")
        self.assertEqual(targets["0x40"], "0x80CE")
        self.assertEqual(targets["0x45"], "0x80CE")
        self.assertEqual(targets["0x57"], "0x812D")
        self.assertEqual(targets["0x71"], "0x9420")
        self.assertEqual(targets["0x73"], "0x8166")
        self.assertEqual(targets["0x74"], "0x818F")
        self.assertEqual(targets["0x75"], "0x8187")
        self.assertEqual(targets["0x76"], "0x816B")
        self.assertTrue(all(targets[f"0x{opcode:02X}"] == "0x0000" for opcode in range(0x40)))

    def test_region5_measurements_stay_unidentified(self):
        region = self.inventory["region5"]
        self.assertEqual(self.inventory["region5_identity"], "unknown")
        self.assertEqual(self.inventory["sequence_f0_00_01_0c"], 0)
        self.assertEqual(region["payload_bytes"], 65528)
        self.assertEqual(region["entropy_bits_per_byte"], "7.9156")
        self.assertEqual(region["distinct_byte_values"], 256)
        self.assertEqual(region["zlib_level9_bytes"], 64335)
        self.assertEqual(region["decompress_successes"], 0)
        self.assertEqual(region["load_record_error"], "unsupported POD word width or empty record")
        self.assertEqual(region["printable_runs_at_least_5"], 222)
        self.assertEqual(region["longest_printable_run"], 12)
        self.assertEqual(region["name_scan_hits"], 0)


if __name__ == "__main__":
    unittest.main()
