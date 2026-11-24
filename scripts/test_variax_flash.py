"""Formula checks for the Variax 3.10 flash trace. No device and no image."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WRITE_RS = ROOT / "crates/line6-devices/src/family/variax/write.rs"
FLASH_PY = ROOT / "scripts/map_variax_flash.py"
FLASH_JSON = ROOT / "research/variax-310-flash-handlers.json"

SLOT_BASE = 0xD00000
SECTOR = 0x1000
PRESET_WORDS = 1092
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


def sector_address(index):
    return SLOT_BASE + index * SECTOR


def editor_index(bank, slot):
    return bank * 5 + (4 - slot)


def program_number(bank, slot):
    base = editor_index(bank, slot)
    if bank == 0 or bank == 11:
        return base
    return base + 55


class FlashFormulaTests(unittest.TestCase):
    def test_shared_slot_addresses(self):
        self.assertEqual(
            [sector_address(index) for index in (4, 8, 50, 62)],
            [0xD04000, 0xD08000, 0xD32000, 0xD3E000],
        )
        self.assertEqual(
            [editor_index(bank, slot) for bank, slot in ((0, 0), (1, 1), (10, 4))],
            [4, 8, 50],
        )

    def test_program_number_for_flash_index_50(self):
        self.assertEqual(program_number(10, 4), 105)
        self.assertEqual(program_number(0, 0), 4)
        self.assertEqual(program_number(1, 0), 64)
        self.assertEqual(program_number(11, 0), 59)

    def test_wide_erase_and_code_load_ranges(self):
        def covers(first, last, sector):
            return first <= sector <= last

        self.assertTrue(all(covers(32, 63, sector) for sector in (32, 59, 60, 62, 63)))
        self.assertFalse(any(covers(64, 127, sector) for sector in (60, 61, 62)))
        self.assertTrue(covers(64, 127, 64) and covers(64, 127, 127))
        self.assertFalse(any(covers(16, 31, sector) for sector in range(60, 63)))
        self.assertFalse(any(covers(0, 3, sector) for sector in range(60, 63)))

    def test_slot_read_lengths(self):
        flash_bytes = 3276
        self.assertEqual(flash_bytes, PRESET_WORDS * 3)
        self.assertEqual(flash_bytes * 2, PRESET_WORDS * 6)
        self.assertEqual(4096 - flash_bytes, 820)

    def test_family_write_api_sends_store_and_bank_name(self):
        source = WRITE_RS.read_text()
        self.assertIn("sysex(0x61", source)
        self.assertIn("sysex(0x6c", source)
        for opcode in (
            "44",
            "45",
            "54",
            "56",
            "58",
            "59",
            "60",
            "62",
            "63",
            "67",
            "68",
            "6f",
            "7d",
        ):
            self.assertNotIn(f"sysex(0x{opcode}", source)

    def test_committed_trace_omits_code_load_recipes(self):
        text = (FLASH_PY.read_text() + FLASH_JSON.read_text()).lower()
        for token in FORBIDDEN:
            self.assertNotIn(token, text)


if __name__ == "__main__":
    unittest.main()
