"""Editor-versus-firmware checks for Variax 0x4C and 0x2A. No device and no image."""

import unittest
from pathlib import Path

WRITE_RS = Path("crates/line6-devices/src/family/variax/write.rs")


def dsp_argument(enabled):
    """Byte after opcode 0x4C. The firmware stores this byte unmasked."""
    return 0 if enabled else 1


def session_bit(byte):
    """Bit stored by the session handler. Bypass does not use this mask."""
    return byte & 1


def bypass_selects_short_mix(flag):
    """The audio gate branches on any nonzero bypass flag."""
    return flag != 0


class SessionFlagTests(unittest.TestCase):
    def test_dsp_off_sends_one_and_dsp_on_sends_zero(self):
        source = WRITE_RS.read_text()
        self.assertIn("Message::DspEnabled(on) => sysex(0x4c, [u8::from(!on)])", source)
        self.assertEqual(dsp_argument(False), 1)
        self.assertEqual(dsp_argument(True), 0)

    def test_store_opens_bypass_and_live_edit_does_not(self):
        source = WRITE_RS.read_text()
        self.assertIn("transaction(session, G7, true,", source)
        self.assertIn("transaction(session, G6, false,", source)

    def test_cleanup_sends_dsp_on_before_session_off(self):
        source = WRITE_RS.read_text()
        dsp_on = source.index("Message::DspEnabled(true)")
        session_off = source.index("Message::Session(false)", dsp_on)
        bridge_off = source.index("Message::Bridge(false)", session_off)
        self.assertLess(dsp_on, session_off)
        self.assertLess(session_off, bridge_off)

    def test_session_keeps_bit_0_and_any_nonzero_bypass_selects_the_mix(self):
        self.assertEqual([session_bit(byte) for byte in (0x00, 0x01, 0x02, 0x03)], [0, 1, 0, 1])
        self.assertEqual(
            [bypass_selects_short_mix(flag) for flag in (0, 1, 0x80)],
            [False, True, True],
        )


if __name__ == "__main__":
    unittest.main()
