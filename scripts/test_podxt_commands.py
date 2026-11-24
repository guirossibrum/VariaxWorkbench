"""Checks for the PODxt Live 3.01 command record. No device and no firmware image."""

import json
import unittest
from pathlib import Path

from map_podxt_commands import (
    EVIDENCE, FLASH, MAIN_LOOP_STATE, PROGRAM_END, PROGRAM_START, STATE_PREAMBLE, TRANSMIT_OPCODES,
)

ENTRY = Path(__file__).resolve().parents[1] / "research" / "podxt-301-entry.json"
COMMANDS = Path(__file__).resolve().parents[1] / "research" / "podxt-301-commands.json"


class CommandClaimTests(unittest.TestCase):
    def test_evidence_addresses_are_inside_the_loaded_program(self):
        self.assertGreaterEqual(len(EVIDENCE), 60)
        for addr, text in EVIDENCE:
            self.assertGreaterEqual(addr, PROGRAM_START)
            self.assertLess(addr, PROGRAM_END)
            self.assertTrue(text)

    def test_flash_labels_cover_each_loaded_opcode_once(self):
        entry = json.loads(ENTRY.read_text())
        labelled = [opcode for opcodes in FLASH.values() for opcode in opcodes]
        self.assertEqual(len(labelled), len(set(labelled)))
        for opcode in labelled:
            self.assertEqual(entry["targets"][opcode] != "0x0000", True)
            word = int(entry["targets"][opcode], 16)
            self.assertTrue(word == 0x80CE or PROGRAM_START <= word < PROGRAM_END)
        self.assertEqual(FLASH["program"], ("0x6D", "0x71"))
        self.assertEqual(FLASH["erase"], ("0x6C",))
        self.assertEqual(FLASH["program_and_erase"], ("0x60", "0x67", "0x72"))
        self.assertEqual(FLASH["external_fill"], ("0x68",))
        self.assertIn("0x75", FLASH["none"])
        self.assertIn("0x74", FLASH["none"])
        self.assertIn("0x57", FLASH["none"])

    def test_checked_in_record_matches_the_locked_tables(self):
        record = json.loads(COMMANDS.read_text())
        self.assertEqual(record["state_preamble_0x1B917"], list(STATE_PREAMBLE))
        self.assertEqual(record["main_loop_state_0x1B911"], list(MAIN_LOOP_STATE))
        self.assertEqual(record["table_0x1B978"], {"zero": 158, "program": 49, "outside": 49})
        self.assertTrue(record["table_0x1BB78_low128_all_program"])
        self.assertFalse(record["routine_0x940A_in_width1_words"])
        self.assertEqual(record["flash_identity"], "likely")
        self.assertEqual(record["evidence_count"], len(EVIDENCE))
        self.assertEqual(record["flash"]["program"], ["0x6D", "0x71"])
        self.assertEqual(record["transmit_opcode_immediates"], [f"0x{opcode:02X}" for opcode in TRANSMIT_OPCODES])
        self.assertEqual(record["transmit_sites_outside_0x80D6_0x81A5"], 0)
        self.assertFalse(record["immediate_0x45"])
        self.assertFalse(record["register_immediate_0x13"])
        self.assertNotIn("0x0F", record["transmit_opcode_immediates"])
        self.assertNotIn("0x24", record["transmit_opcode_immediates"])
        self.assertNotIn("0x40", record["transmit_opcode_immediates"])
        self.assertNotIn("0x45", record["transmit_opcode_immediates"])


if __name__ == "__main__":
    unittest.main()
