"""Formula checks for the PODxt messages on a Variax update route.

No device and no image. The bytes are rebuilt here and compared with the note.
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/firmware-variax-opcode-history.md"
PLACEHOLDER = "## PODxt Live"
SECTION = "## Variax update through a PODxt Live"
NEXT = "## Out of scope"
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


def podxt_frame(value):
    return bytes((0xF0, 0x00, 0x01, 0x0C, 0x03, 0x23, value, 0xF7))


def framed(value):
    return " ".join(f"{byte:02X}" for byte in podxt_frame(value))


class PodxtUpdateRouteTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.doc = DOC.read_text()
        start = cls.doc.index(SECTION)
        cls.section = " ".join(cls.doc[start : cls.doc.index(NEXT, start)].split())
        holder = cls.doc.index(PLACEHOLDER)
        cls.placeholder = " ".join(cls.doc[holder:start].split())

    def test_frames_are_family_3_opcode_23(self):
        self.assertEqual(podxt_frame(1).hex(), "f000010c032301f7")
        self.assertEqual(podxt_frame(0).hex(), "f000010c032300f7")
        self.assertEqual(len(podxt_frame(1)), 8)
        self.assertIn(f"`{framed(1)}`", self.section)
        self.assertIn(f"`{framed(0)}`", self.section)

    def test_podxt_live_usb_id_is_18000(self):
        self.assertEqual(0x4650, 18000)
        self.assertIn("0x4650", self.section)
        self.assertIn("18000", self.section)
        self.assertIn("0x00070004", self.section)
        self.assertIn("1,000 ms", self.section)

    def test_placeholder_stays_empty(self):
        self.assertIn("Nothing is attributed here.", self.placeholder)
        self.assertNotIn("03 23", self.placeholder)
        self.assertNotIn("PODxt Live 3.01 opcode", self.doc)

    def test_section_names_the_senders_and_the_open_points(self):
        for needle in (
            "**Confirmed.**",
            "**Unknown.**",
            "Monkey 1.77 Windows",
            "Monkey 1.33 Windows",
            "Monkey 1.78 Mac",
            "UpdaterV3EngineDLL.dll",
            "d24bb48b0f13d9b46c6dd514e712c347ff9a00d4763555614f700c22634bebc7",
            "XTLIVE_ENABLE_VARIAXMESSAGES_MSG",
            "version 2.11",
            "No file named as a separate Variax updater",
        ):
            self.assertIn(needle, self.section)

    def test_new_section_omits_code_load_recipes(self):
        lowered = self.section.lower()
        for token in FORBIDDEN:
            self.assertNotIn(token, lowered)


if __name__ == "__main__":
    unittest.main()
