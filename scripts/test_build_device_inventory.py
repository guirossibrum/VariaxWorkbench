"""Offline tests for the #114 public inventory and its source migration."""

import csv
import io
import re
import subprocess
import tempfile
import unittest
from collections import Counter, defaultdict
from pathlib import Path

import build_device_inventory as inventory


class PublicDeviceInventoryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rendered = inventory.render()
        schema, table = cls.rendered.decode("utf-8").split("\n", 1)
        cls.schema = schema
        cls.rows = list(csv.DictReader(io.StringIO(table), delimiter="\t"))

    def test_reproducible_output(self):
        self.assertEqual(self.schema, "schema\t3")
        self.assertEqual(self.rendered, inventory.render())
        self.assertEqual(self.rendered, (inventory.ROOT / inventory.OUTPUT).read_bytes())

    def test_frozen_ordinals_and_roles(self):
        with (inventory.ROOT / inventory.PROJECTION).open(encoding="utf-8") as source:
            expected = [(row["name"], row["role"]) for row in csv.DictReader(source, delimiter="\t")]
        actual = [(row["name"], row["role"]) for row in self.rows]
        self.assertEqual(actual, expected)
        self.assertEqual([int(row["ordinal"]) for row in self.rows], list(range(1, 225)))
        self.assertEqual(Counter(row["role"] for row in self.rows), {
            "alias": 186, "product": 29, "guitar": 5, "bridge": 2, "family": 2,
        })

    def test_every_draft_line_maps_to_all_exact_ordinals(self):
        ordinals_by_name = defaultdict(list)
        for row in self.rows:
            ordinals_by_name[row["name"]].append(int(row["ordinal"]))
        for path, column, count in (
            (inventory.UNDERSCORE, "underscore_lines", 216),
            (inventory.HYPHEN, "hyphen_lines", 176),
        ):
            with self.subTest(path=path):
                actual = defaultdict(list)
                for row in self.rows:
                    if row[column] != "-":
                        for line in row[column].split(","):
                            actual[int(line)].append(int(row["ordinal"]))
                draft = (inventory.ROOT / path).read_text(encoding="utf-8").splitlines()
                expected = {
                    line: ordinals_by_name[item.partition(" | ")[0]]
                    for line, item in enumerate(draft, 1)
                }
                self.assertEqual(len(expected), count)
                self.assertEqual(dict(actual), expected)
        self.assertEqual(ordinals_by_name["TonePort"], [164, 165])

    def test_aliases_carry_no_identifiers(self):
        aliases = [row for row in self.rows if row["role"] == "alias"]
        self.assertEqual(len(aliases), 186)
        for row in aliases:
            self.assertTrue(all(row[field] == "-" for field in inventory.ID_WIDTHS), row)

    def test_exact_unresolved_fields(self):
        unresolved = {
            (int(row["ordinal"]), field)
            for row in self.rows for field in inventory.ID_WIDTHS
            if row[field] == "unresolved"
        }
        self.assertEqual(unresolved, {(18, "usb_pid")} | {
            (ordinal, "l6_product")
            for ordinal in (17, 19, 50, 51, 56, 57, 59, 84, 85, 86, 112, 114, 164, 165, 209, 210, 211)
        })

    def test_all_populated_identifiers_have_pinned_public_citations(self):
        with (inventory.ROOT / inventory.IDENTIFIERS).open(encoding="utf-8") as source:
            revision_label, revision = next(source).strip().split("\t")
            self.assertEqual(revision_label, "source_revision")
            self.assertEqual(revision, inventory.SOURCE_COMMIT)
            evidence = {
                (record["ordinal"], record["field"]): (record["value"], record["source_location"])
                for record in csv.DictReader(source, delimiter="\t")
                if record["value"] != "unresolved"
            }
        actual = {
            (row["ordinal"], field): row[field]
            for row in self.rows for field in inventory.ID_WIDTHS
            if row[field] not in {"-", "unresolved"}
        }
        self.assertEqual(actual, {key: value for key, (value, _) in evidence.items()})
        self.assertEqual(len(evidence), 27)
        for (ordinal, field), (value, location) in evidence.items():
            with self.subTest(ordinal=ordinal, field=field):
                path, line_range = location.rsplit(":", 1)
                first, _, last = line_range.partition("-")
                lines = subprocess.check_output(
                    ["git", "show", f"{revision}:{path}"], cwd=inventory.ROOT, text=True
                ).splitlines()
                cited = "\n".join(lines[int(first)-1:int(last or first)]).lower()
                tokens = re.findall(r"\b(?:0x[0-9a-f]+|[0-9]+)\b", cited)
                observed = {int(token, 16 if token.startswith("0x") else 10) for token in tokens}
                self.assertIn(int(value, 16), observed)

    def test_crlf_source_checkout_and_drift_detection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path in (*inventory.PINS, inventory.IDENTIFIERS):
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((inventory.ROOT / path).read_bytes().replace(b"\n", b"\r\n"))
            self.assertEqual(inventory.render(root), self.rendered)
            for path in inventory.PINS:
                with self.subTest(path=path):
                    target = root / path
                    original = target.read_bytes()
                    target.write_bytes(original + b"extra row\r\n")
                    with self.assertRaisesRegex(ValueError, "source digest mismatch"):
                        inventory.render(root)
                    target.write_bytes(original)

    def test_identifier_cannot_be_attached_to_an_alias(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path in (*inventory.PINS, inventory.IDENTIFIERS):
                target = root / path
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((inventory.ROOT / path).read_bytes())
            with (root / inventory.IDENTIFIERS).open("a", encoding="utf-8") as file:
                file.write("1\tAmplifi\tusb_vid\t0x0E41\tsrc/pod_hd500x.rs:18\n")
            with self.assertRaisesRegex(ValueError, "matching non-alias row"):
                inventory.render(root)


if __name__ == "__main__":
    unittest.main()
