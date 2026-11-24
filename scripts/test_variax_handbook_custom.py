"""Offline checks for the ten handbook Custom assignments. No device and no archive."""

import contextlib
import io
import tempfile
import unittest
from pathlib import Path

import compare_variax_handbook_custom as cmp

ROOT = Path(__file__).resolve().parents[1]
HANDBOOK = ROOT / "research" / "variax-300-handbook-custom.tsv"

# Workbench program pairs from tests/variax_handbook_reference.rs, then the
# expanded-region slot indexes those programs use.
PROGRAMS = (
    (4, 100),
    (3, 106),
    (2, 101),
    (1, 108),
    (0, 109),
    (59, 74),
    (58, 83),
    (57, 87),
    (56, 66),
    (55, 91),
)
SLOT_INDEXES = (
    (4, 45),
    (3, 51),
    (2, 46),
    (1, 53),
    (0, 54),
    (59, 19),
    (58, 28),
    (57, 32),
    (56, 11),
    (55, 36),
)
RESO2 = "ab" * 32
LESTER = "cd" * 32
SPANK = "ef" * 32
JAZZ = "a1" * 32
CUSTOM_ACOUSTIC = "b2" * 32
OFFSET = "424242"


def sample_manifest() -> cmp.Manifest:
    """Sixty slots. One Custom position shares the handbook source's whole slot."""
    slots = {
        index: cmp.Slot(index, f"{index:02x}" * 32, f"Model-{index}") for index in range(60)
    }

    def put(index: int, name: str, sha: str | None = None) -> None:
        slots[index] = cmp.Slot(index, slots[index].sha256 if sha is None else sha, name)

    put(0, "JazzBox-5", JAZZ)
    put(40, "JazzBox-5", JAZZ)
    put(1, "Reso-2", RESO2)
    put(53, "Reso-2", RESO2)
    put(2, "Acoustic-5", CUSTOM_ACOUSTIC)
    put(45, "Acoustic-5")
    put(3, "Spank-2", SPANK)
    put(13, "Spank-2", SPANK)
    put(4, "Lester-1", LESTER)
    put(19, "Lester-1", LESTER)
    put(46, "Acoustic-4")
    put(51, "Reso-4")
    put(54, "Reso-1")
    put(28, "R-Billy-2")
    put(32, "Chime-3")
    put(11, "Spank-4")
    put(36, "Semi-4")
    put(59, "Drop D")
    put(58, "Open A")
    put(57, "Open D")
    put(56, "Open C")
    put(55, "Open G")
    return cmp.Manifest(slots)


def manifest_text(slots: dict[int, cmp.Slot] | None = None, mark: str = cmp.FIRMWARE_MARK) -> str:
    if slots is None:
        slots = sample_manifest().slots
    lines = [
        f"# {mark}; synthetic test manifest.",
        "kind\tindex\tfile_offset\tbytes\tsha256\tname",
        "template\t0\t" + OFFSET + "\t2619\t" + ("11" * 32) + "\tT-Model-5",
        "settings\t0\tderived\t980\t" + ("22" * 32) + "\tSHOULD-NOT-APPEAR",
    ]
    for index in range(60):
        slot = slots[index]
        lines.append(f"slot\t{index}\t{OFFSET}\t4096\t{slot.sha256}\t{slot.name}")
    return "\n".join(lines) + "\n"


def rows_by_field(rows):
    return {
        (bank, position, field): (value, label)
        for _baseline, bank, position, field, value, label in rows
    }


class HandbookCustomCompareTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.handbook_bytes = HANDBOOK.read_bytes()
        cls.assignments = cmp.load_handbook(HANDBOOK)

    def tearDown(self):
        self.assertEqual(HANDBOOK.read_bytes(), self.handbook_bytes)

    def test_handbook_programs_and_slot_indexes(self):
        self.assertEqual(len(self.assignments), 10)
        for assignment, programs, indexes in zip(self.assignments, PROGRAMS, SLOT_INDEXES):
            with self.subTest(bank=assignment.custom_bank, position=assignment.custom_position):
                self.assertEqual(
                    (assignment.custom_program, assignment.source_program), programs
                )
                self.assertEqual(
                    (assignment.custom_slot_index, assignment.source_slot_index), indexes
                )

    def test_names_agree_on_spelling(self):
        self.assertTrue(cmp.names_agree("JazzBox-5", "Jazzbox", 5))
        self.assertTrue(cmp.names_agree("Reso-2", "Reso", 2))
        self.assertTrue(cmp.names_agree("Reso 2", "Reso", 2))
        self.assertTrue(cmp.names_agree("R-Billy-2", "R-Billy", 2))
        self.assertFalse(cmp.names_agree("Acoustic-5", "Acoustic", 4))
        self.assertFalse(cmp.names_agree("Drop D", "Lester", 1))
        self.assertFalse(cmp.names_agree("Open G", "Semi", 4))

    def test_handbook_baseline_without_manifest_covers_ten_positions(self):
        rows = cmp.compare(self.assignments, None, "handbook", derive=False)
        text = cmp.render(
            rows, baseline="handbook", handbook=HANDBOOK, manifest=None, derive=False
        )
        self.assertTrue(text.startswith("# baseline: handbook shipping assignments\n"))
        self.assertIn("# other baseline: firmware 3.10 expanded defaults\n", text)
        self.assertNotIn("derived", text.casefold())
        fields = rows_by_field(rows)
        self.assertEqual(len(fields), 10 * (len(cmp.LOCATOR_FIELDS) + len(cmp.MANIFEST_FIELDS) + len(cmp.UNKNOWN_FIELDS)))
        for bank, position in cmp.EXPECTED_POSITIONS:
            key = (bank, str(position))
            self.assertEqual(fields[(*key, "tone_knob")], ("unknown", "unknown"))
            self.assertEqual(fields[(*key, "dsp_program")], ("unknown", "unknown"))
            self.assertEqual(fields[(*key, "unit_calibration")], ("unknown", "unknown"))
            self.assertEqual(fields[(*key, "saved_tone_flag")], ("unknown", "unknown"))
            self.assertEqual(fields[(*key, "expanded_slot_name")], ("not-supplied", "absent"))
            self.assertEqual(fields[(*key, "source_bank")][1], "handbook-shipping")
        self.assertNotIn("Lester-1", text)
        self.assertNotIn("Drop D", text)
        self.assertNotIn("Acoustic-5", text)
        self.assertNotIn(OFFSET, text)

    def test_manifest_comparison_labels_the_one_whole_slot_match(self):
        manifest = sample_manifest()
        rows = cmp.compare(self.assignments, manifest, "handbook", derive=True)
        fields = rows_by_field(rows)
        self.assertEqual(fields[("Custom 1", "4", "expanded_slot_name")], ("Reso-2", "firmware-3.10-expanded"))
        self.assertEqual(fields[("Custom 1", "4", "name_matches")], ("yes", "comparison"))
        self.assertEqual(fields[("Custom 1", "4", "slot_bytes_match")], ("yes", "comparison"))
        self.assertEqual(fields[("Custom 1", "4", "assignment_difference")], ("none", "comparison"))
        self.assertEqual(fields[("Custom 1", "4", "same_bytes_as")], ("53 Reso-2", "firmware-3.10-expanded"))
        self.assertEqual(fields[("Custom 1", "4", "same_name_different_bytes")], ("none", "firmware-3.10-expanded"))
        self.assertEqual(fields[("Custom 1", "4", "model_name_matches")], ("yes", "comparison"))
        self.assertEqual(fields[("Custom 1", "1", "expanded_slot_name")], ("Lester-1", "firmware-3.10-expanded"))
        self.assertEqual(fields[("Custom 1", "1", "assignment_difference")], ("name-and-bytes", "comparison"))
        self.assertEqual(fields[("Custom 1", "1", "same_bytes_as")], ("19 Lester-1", "firmware-3.10-expanded"))
        self.assertEqual(fields[("Custom 1", "1", "reconstructed_name")], ("Acoustic-5", "derived"))
        self.assertEqual(fields[("Custom 1", "1", "reconstructed_from_slot")], ("45", "derived"))
        self.assertEqual(fields[("Custom 1", "2", "same_bytes_as")], ("13 Spank-2", "firmware-3.10-expanded"))
        self.assertEqual(
            fields[("Custom 1", "3", "same_name_different_bytes")],
            ("45 Acoustic-5", "firmware-3.10-expanded"),
        )
        self.assertEqual(fields[("Custom 1", "3", "slot_bytes_match")], ("no", "comparison"))
        self.assertEqual(fields[("Custom 1", "5", "expanded_slot_name")][0], "JazzBox-5")
        self.assertEqual(fields[("Custom 2", "1", "expanded_slot_name")][0], "Drop D")
        self.assertEqual(fields[("Custom 2", "5", "expanded_slot_name")][0], "Open G")
        differences = [
            fields[(bank, str(position), "assignment_difference")][0]
            for bank, position in cmp.EXPECTED_POSITIONS
        ]
        self.assertEqual(differences.count("none"), 1)
        for bank, position in cmp.EXPECTED_POSITIONS:
            self.assertEqual(fields[(bank, str(position), "model_name_matches")], ("yes", "comparison"))
            self.assertEqual(fields[(bank, str(position), "tone_knob")], ("unknown", "unknown"))
            self.assertEqual(
                fields[(bank, str(position), "reconstruction")],
                (cmp.RECONSTRUCTION, "derived"),
            )
        text = cmp.render(
            rows,
            baseline="handbook",
            handbook=HANDBOOK,
            manifest=Path("manifest.tsv"),
            derive=True,
        )
        for secret in (RESO2, LESTER, SPANK, JAZZ, CUSTOM_ACOUSTIC, OFFSET, "SHOULD-NOT-APPEAR", "T-Model-5"):
            self.assertNotIn(secret, text)

    def test_name_and_byte_differences_stay_separate(self):
        slots = dict(sample_manifest().slots)
        source_sha = slots[54].sha256
        slots[0] = cmp.Slot(0, "b0" * 32, "Reso-1")
        named = cmp.compare(self.assignments, cmp.Manifest(slots), "firmware-3.10", derive=False)
        fields = rows_by_field(named)
        self.assertEqual(fields[("Custom 1", "5", "assignment_difference")], ("bytes", "comparison"))
        self.assertEqual(fields[("Custom 1", "5", "name_matches")], ("yes", "comparison"))
        self.assertEqual(fields[("Custom 1", "5", "slot_bytes_match")], ("no", "comparison"))

        slots[0] = cmp.Slot(0, source_sha, "JazzBox-5")
        copied = cmp.compare(self.assignments, cmp.Manifest(slots), "handbook", derive=False)
        fields = rows_by_field(copied)
        self.assertEqual(fields[("Custom 1", "5", "assignment_difference")], ("name", "comparison"))
        self.assertEqual(fields[("Custom 1", "5", "slot_bytes_match")], ("yes", "comparison"))

    def test_selected_baseline_names_itself_and_keeps_the_values(self):
        manifest = sample_manifest()
        handbook_rows = cmp.compare(self.assignments, manifest, "handbook", derive=True)
        firmware_rows = cmp.compare(self.assignments, manifest, "firmware-3.10", derive=True)
        self.assertEqual(len(handbook_rows), len(firmware_rows))
        self.assertTrue(all(row[0] == "handbook-shipping" for row in handbook_rows))
        self.assertTrue(all(row[0] == "firmware-3.10-expanded" for row in firmware_rows))
        self.assertEqual([row[1:] for row in handbook_rows], [row[1:] for row in firmware_rows])
        firmware_text = cmp.render(
            firmware_rows,
            baseline="firmware-3.10",
            handbook=HANDBOOK,
            manifest=Path("manifest.tsv"),
            derive=True,
        )
        self.assertTrue(firmware_text.startswith("# baseline: firmware 3.10 expanded defaults\n"))
        self.assertIn("# other baseline: handbook shipping assignments\n", firmware_text)

    def test_firmware_baseline_and_derive_require_a_manifest(self):
        with self.assertRaisesRegex(cmp.CompareError, "factory manifest of the expanded defaults"):
            cmp.compare(self.assignments, None, "firmware-3.10", derive=False)
        with self.assertRaisesRegex(cmp.CompareError, "3.10 model-bank slots"):
            cmp.compare(self.assignments, None, "handbook", derive=True)

    def test_manifest_file_requires_sixty_named_slots(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            good = root / "manifest.tsv"
            good.write_text(manifest_text(), encoding="utf-8")
            loaded = cmp.load_manifest(good)
            self.assertEqual(loaded.slots[1].name, "Reso-2")
            self.assertEqual(len(loaded.slots), 60)

            unmarked = root / "unmarked.tsv"
            unmarked.write_text(
                manifest_text(mark="Firmware defaults"),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(cmp.CompareError, "firmware 3.10 expanded defaults"):
                cmp.load_manifest(unmarked)

            missing = root / "missing.tsv"
            dropped = "\n".join(
                line for line in manifest_text().splitlines() if not line.startswith("slot\t59\t")
            )
            missing.write_text(dropped + "\n", encoding="utf-8")
            with self.assertRaisesRegex(cmp.CompareError, "missing slot 59"):
                cmp.load_manifest(missing)

            duplicate = root / "duplicate.tsv"
            duplicate.write_text(
                manifest_text() + f"slot\t0\t{OFFSET}\t4096\t{'33' * 32}\tAgain\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(cmp.CompareError, "duplicate slot 0"):
                cmp.load_manifest(duplicate)

    def test_export_creates_one_new_file_and_refuses_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / "manifest.tsv"
            manifest.write_text(manifest_text(), encoding="utf-8")
            manifest_bytes = manifest.read_bytes()
            destination = root / "custom-compare.tsv"
            code, stdout, stderr = self.run_main(
                [
                    "--baseline",
                    "handbook",
                    "--handbook",
                    str(HANDBOOK),
                    "--manifest",
                    str(manifest),
                    "--derive",
                    "--export",
                    str(destination),
                ]
            )
            self.assertEqual(code, 0, stderr)
            self.assertEqual(destination.read_text(encoding="utf-8"), stdout)
            self.assertTrue(stdout.startswith("# baseline: handbook shipping assignments\n"))
            self.assertIn("\tderived\n", stdout)
            self.assertEqual(manifest.read_bytes(), manifest_bytes)
            self.assertEqual(list(root.glob(".handbook-compare-*")), [])

            code, _stdout, stderr = self.run_main(
                ["--handbook", str(HANDBOOK), "--export", str(destination)]
            )
            self.assertEqual(code, 1)
            self.assertIn("refusing to replace", stderr)
            self.assertEqual(destination.read_text(encoding="utf-8"), stdout)

            code, _stdout, stderr = self.run_main(
                ["--handbook", str(HANDBOOK), "--export", str(HANDBOOK)]
            )
            self.assertEqual(code, 1)
            self.assertIn("refusing to replace", stderr)
            self.assertEqual(HANDBOOK.read_bytes(), self.handbook_bytes)

            code, _stdout, stderr = self.run_main(
                [
                    "--handbook",
                    str(HANDBOOK),
                    "--manifest",
                    str(manifest),
                    "--export",
                    str(manifest),
                ]
            )
            self.assertEqual(code, 1)
            self.assertIn("refusing to replace", stderr)
            self.assertEqual(manifest.read_bytes(), manifest_bytes)

            missing_dir = root / "missing" / "out.tsv"
            code, _stdout, stderr = self.run_main(
                ["--handbook", str(HANDBOOK), "--export", str(missing_dir)]
            )
            self.assertEqual(code, 1)
            self.assertIn("export directory does not exist", stderr)
            self.assertFalse(missing_dir.exists())

    def run_main(self, args):
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = cmp.main(args)
        return code, stdout.getvalue(), stderr.getvalue()


if __name__ == "__main__":
    unittest.main()
