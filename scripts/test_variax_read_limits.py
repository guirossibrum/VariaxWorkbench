"""Editor-versus-firmware checks for Variax 0x3C limits. No device and no image."""

import unittest
from pathlib import Path

from map_variax_read_limits import (
    PRESET_WORDS,
    QUEUE_MASK,
    last_word,
    reply_bytes,
    stays_inside,
)

VARIAX_RS = Path(__file__).resolve().parents[1] / "crates/line6-devices/src/family/variax.rs"


class ReadLimitTests(unittest.TestCase):
    def test_reply_length_and_queue_capacity(self):
        self.assertEqual(reply_bytes(32), 200)
        self.assertEqual(reply_bytes(PRESET_WORDS), 6560)
        self.assertEqual(reply_bytes(9), 62)
        self.assertGreater(reply_bytes(10), QUEUE_MASK)
        self.assertEqual(QUEUE_MASK, 63)

    def test_editor_reads_end_inside_the_preset(self):
        self.assertEqual(
            [(last_word(start, 32), stays_inside(start, 32)) for start in (851, 883)],
            [(882, True), (914, True)],
        )
        self.assertTrue(stays_inside(0, PRESET_WORDS))
        self.assertFalse(stays_inside(0, PRESET_WORDS + 1))
        self.assertFalse(stays_inside(PRESET_WORDS, 1))
        self.assertFalse(stays_inside(1091, 2))

    def test_the_request_enum_sends_only_the_two_metadata_spans(self):
        source = VARIAX_RS.read_text()
        self.assertEqual(source.count("0x3c, 0x00, 0x02"), 2)
        self.assertIn("message.len() == 200", source)
        self.assertIn("matches!(message[5], 0 | 4)", source)
        self.assertNotIn("0x3c, 0x00, 0x00", source)


if __name__ == "__main__":
    unittest.main()
