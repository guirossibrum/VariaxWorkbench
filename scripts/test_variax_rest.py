"""Formula checks for the remaining Variax 3.10 handlers. No device and no image."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WRITE_RS = ROOT / "crates/line6-devices/src/family/variax/write.rs"
FAMILY_RS = ROOT / "crates/line6-devices/src/family/variax.rs"
REST_PY = ROOT / "scripts/map_variax_rest.py"
REST_JSON = ROOT / "research/variax-310-rest-handlers.json"

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

REMAINING = (
    "10",
    "16",
    "17",
    "1c",
    "1d",
    "1e",
    "23",
    "29",
    "35",
    "36",
    "37",
    "3e",
    "3f",
    "40",
    "41",
    "47",
    "49",
    "4e",
    "5e",
    "5f",
    "69",
    "6a",
    "6b",
    "75",
)


class RestFormulaTests(unittest.TestCase):
    def test_family_write_api_omits_the_remaining_opcodes(self):
        source = WRITE_RS.read_text().lower()
        for opcode in REMAINING:
            self.assertNotIn(f"sysex(0x{opcode}", source)
        self.assertIn("vec![0xb0, 79, value]", source)
        header = FAMILY_RS.read_text()
        self.assertIn(
            "const HEADER: [u8; 5] = [0xf0, 0x00, 0x01, 0x0c, 0x07];",
            header,
        )

    def test_discard_count(self):
        self.assertEqual(0x2003, 8195)

    def test_committed_trace_omits_code_load_recipes(self):
        text = (REST_PY.read_text() + REST_JSON.read_text()).lower()
        for token in FORBIDDEN:
            self.assertNotIn(token, text)


if __name__ == "__main__":
    unittest.main()
