"""Formula checks for a full in-bounds Variax 0x3C read. No device and no image."""

import unittest
from pathlib import Path

from map_variax_read_limits import QUEUE_MASK, reply_bytes

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/firmware-variax-dispatch.md"
VARIAX_RS = ROOT / "crates/line6-devices/src/family/variax.rs"
HEADER = [0xF0, 0x00, 0x01, 0x0C, 0x07, 0x00]
COUNTS = (64, 128, 256, 512, 1092)
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


def low_nibbles(value):
    """Six low-nibble-first bytes of a 24-bit word."""
    return [((value >> (4 * index)) & 0x0F) for index in range(6)]


def read_request(start, count):
    """Family frame for opcode 0x3C: count, then start, each six nibbles."""
    return HEADER + [0x3C] + low_nibbles(count) + low_nibbles(start) + [0xF7]


def fits_one_fill(count):
    """True when the whole reply fits in the 63-byte transmit queue."""
    return reply_bytes(count) <= QUEUE_MASK


def full_buffer_section(text):
    start = text.index("## Full-buffer 0x3C read")
    end = text.index("## Preset store", start)
    return text[start:end]


class FullReadFormulaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.doc = DOC.read_text()
        cls.section = full_buffer_section(cls.doc)
        cls.source = VARIAX_RS.read_text()

    def test_request_nibbles_match_the_host_metadata_reads(self):
        self.assertEqual(
            low_nibbles(32),
            [0x00, 0x02, 0x00, 0x00, 0x00, 0x00],
        )
        self.assertEqual(
            low_nibbles(851),
            [0x03, 0x05, 0x03, 0x00, 0x00, 0x00],
        )
        self.assertEqual(
            low_nibbles(883),
            [0x03, 0x07, 0x03, 0x00, 0x00, 0x00],
        )
        collapsed = " ".join(self.source.split())
        metadata1 = read_request(851, 32)
        metadata2 = read_request(883, 32)
        self.assertIn(", ".join(f"0x{byte:02x}" for byte in metadata1), collapsed)
        self.assertIn(", ".join(f"0x{byte:02x}" for byte in metadata2), collapsed)

    def test_full_buffer_requests_and_reply_lengths(self):
        expected = {
            64: 392,
            128: 776,
            256: 1544,
            512: 3080,
            1092: 6560,
        }
        for count in COUNTS:
            request = read_request(0, count)
            self.assertEqual(request[:7], HEADER + [0x3C])
            self.assertEqual(request[7:13], low_nibbles(count))
            self.assertEqual(request[13:19], [0, 0, 0, 0, 0, 0])
            self.assertEqual(request[-1], 0xF7)
            self.assertEqual(len(request), 20)
            self.assertEqual(reply_bytes(count), 8 + 6 * count)
            self.assertEqual(reply_bytes(count), expected[count])
            hex_bytes = " ".join(f"{byte:02X}" for byte in request)
            self.assertIn(f"`{hex_bytes}`", self.section)
            self.assertIn(f"{expected[count]:,}", self.section)

    def test_queue_pacing(self):
        self.assertEqual(QUEUE_MASK, 63)
        self.assertEqual(reply_bytes(9), 62)
        self.assertTrue(fits_one_fill(9))
        self.assertEqual(reply_bytes(10), 68)
        self.assertFalse(fits_one_fill(10))
        for count in COUNTS:
            self.assertGreater(reply_bytes(count), QUEUE_MASK)
            self.assertFalse(fits_one_fill(count))

    def test_metadata_tone_bytes_land_on_words_871_and_872(self):
        def word_for_byte(index):
            return 851 + (index // 3)

        self.assertEqual(word_for_byte(61), 871)
        self.assertEqual(word_for_byte(62), 871)
        self.assertEqual(word_for_byte(63), 872)
        self.assertEqual(0xA47 + 871, 0xDAE)
        self.assertEqual(0xA47 + 872, 0xDAF)
        self.assertIn("word 871", self.section)
        self.assertIn("word 872", self.section)
        self.assertIn("`Y:0xDAE`", self.section)
        self.assertIn("`Y:0xDAF`", self.section)

    def test_section_names_the_questions(self):
        self.assertIn("## Full-buffer 0x3C read", self.doc)
        for label in ("**Confirmed.**", "**Unknown.**"):
            self.assertIn(label, self.section)
        self.assertIn("`P:0x1FE1`", self.section)
        self.assertIn("`P:0x1241`", self.section)
        self.assertIn("Program 5", self.section)
        self.assertIn("PE4", self.section)

    def test_new_section_omits_code_load_recipes(self):
        text = self.section.lower()
        for token in FORBIDDEN:
            self.assertNotIn(token, text)


if __name__ == "__main__":
    unittest.main()
