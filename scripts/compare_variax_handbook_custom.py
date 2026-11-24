"""Compare the ten Variax Custom assignments against one named baseline.

The handbook shipping assignments are research/variax-300-handbook-custom.tsv.
Firmware 3.10 expanded defaults come from a local factory manifest. The two
stay separate: a manifest name or whole-slot match is the archived 3.10
default, and the handbook table is the shipping assignment.

The handbook gives each Custom position a source model. It gives no tone knob,
DSP program, unit calibration, or saved-tone flag, so those stay unknown.

--derive adds rows that name the 3.10 model-bank slot the handbook assigns.
Those rows are labeled derived. They carry the slot index and name, not the
slot bytes. Shipping-specific parameters stay unknown.

--export writes a new file. An existing file is left unchanged. The tool reads
the handbook and, when asked, the manifest. It does not open a device, a
firmware image, or a sysfs path.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

BANKS = (
    "Custom 1",
    "T-Model",
    "Spank",
    "Lester",
    "Special",
    "R-Billy",
    "Chime",
    "Semi",
    "Jazzbox",
    "Acoustic",
    "Reso",
    "Custom 2",
)
PRESETS_PER_BANK = 5
SLOT_COUNT = 60
HANDBOOK_HEADER = "custom_bank\tcustom_position\tsource_bank\tsource_position"
MANIFEST_HEADER = "kind\tindex\tfile_offset\tbytes\tsha256\tname"
MANIFEST_KINDS = {"template", "slot", "settings"}
FIRMWARE_MARK = "Firmware 3.10 defaults"
SHA256_TEXT = re.compile(r"^[0-9a-f]{64}$")
COLUMNS = ("baseline", "custom_bank", "custom_position", "field", "value", "label")

BASELINE_TITLE = {
    "handbook": "handbook shipping assignments",
    "firmware-3.10": "firmware 3.10 expanded defaults",
}
BASELINE_ID = {
    "handbook": "handbook-shipping",
    "firmware-3.10": "firmware-3.10-expanded",
}

LOCATOR_FIELDS = (
    "source_bank",
    "source_position",
    "source_program",
    "source_slot_index",
    "custom_program",
    "custom_slot_index",
)
MANIFEST_FIELDS = (
    "model_slot_name",
    "model_name_matches",
    "expanded_slot_name",
    "name_matches",
    "slot_bytes_match",
    "same_bytes_as",
    "same_name_different_bytes",
    "assignment_difference",
)
UNKNOWN_FIELDS = (
    "tone_knob",
    "dsp_program",
    "unit_calibration",
    "saved_tone_flag",
)
DERIVED_FIELDS = (
    "reconstructed_from_slot",
    "reconstructed_name",
    "reconstruction",
)
FIELD_LABELS = {
    "source_bank": "handbook-shipping",
    "source_position": "handbook-shipping",
    "source_program": "workbench-numbering",
    "source_slot_index": "workbench-numbering",
    "custom_program": "workbench-numbering",
    "custom_slot_index": "workbench-numbering",
    "model_slot_name": "firmware-3.10-model",
    "model_name_matches": "comparison",
    "expanded_slot_name": "firmware-3.10-expanded",
    "name_matches": "comparison",
    "slot_bytes_match": "comparison",
    "same_bytes_as": "firmware-3.10-expanded",
    "same_name_different_bytes": "firmware-3.10-expanded",
    "assignment_difference": "comparison",
    "tone_knob": "unknown",
    "dsp_program": "unknown",
    "unit_calibration": "unknown",
    "saved_tone_flag": "unknown",
    "reconstructed_from_slot": "derived",
    "reconstructed_name": "derived",
    "reconstruction": "derived",
}
EXPECTED_POSITIONS = tuple(
    (bank, position)
    for bank in ("Custom 1", "Custom 2")
    for position in range(1, PRESETS_PER_BANK + 1)
)
RECONSTRUCTION = "3.10 model-bank slot named by the handbook"


class CompareError(Exception):
    """The handbook, manifest, or export path cannot be used as given."""


@dataclass(frozen=True)
class Assignment:
    custom_bank: str
    custom_position: int
    source_bank: str
    source_position: int

    @property
    def custom_slot_index(self) -> int:
        return slot_index(self.custom_bank, self.custom_position)

    @property
    def source_slot_index(self) -> int:
        return slot_index(self.source_bank, self.source_position)

    @property
    def custom_program(self) -> int:
        return program_number(self.custom_bank, self.custom_position)

    @property
    def source_program(self) -> int:
        return program_number(self.source_bank, self.source_position)


@dataclass(frozen=True)
class Slot:
    index: int
    sha256: str
    name: str


@dataclass(frozen=True)
class Manifest:
    slots: dict[int, Slot]

    def slot(self, index: int) -> Slot:
        try:
            return self.slots[index]
        except KeyError as exc:
            raise CompareError(f"manifest is missing slot {index}") from exc

    def others(self, subject: Slot, predicate) -> list[Slot]:
        found = [
            slot
            for slot in self.slots.values()
            if slot.index != subject.index and predicate(slot)
        ]
        return sorted(found, key=lambda slot: slot.index)


def bank_index(name: str) -> int:
    try:
        return BANKS.index(name)
    except ValueError as exc:
        raise CompareError(f"unknown bank: {name}") from exc


def slot_index(bank: str, position: int) -> int:
    """Expanded-region index: bank * 5 + (5 - position).

    Position is one-based. This is the same base as Workbench
    LocationToProgramNumber and is not a file offset.
    """
    if position < 1 or position > PRESETS_PER_BANK:
        raise CompareError(f"position is outside 1..5: {position}")
    zero_based = position - 1
    return bank_index(bank) * PRESETS_PER_BANK + (PRESETS_PER_BANK - 1 - zero_based)


def program_number(bank: str, position: int) -> int:
    """Workbench program: the slot index, plus 55 for banks 1 through 10."""
    base = slot_index(bank, position)
    bank_id = bank_index(bank)
    if bank_id == 0 or bank_id == len(BANKS) - 1:
        return base
    return base + 55


def names_agree(slot_name: str, bank: str, position: int) -> bool:
    """True when a manifest name is the same bank and position.

    Comparison ignores case, spaces, and hyphens, so JazzBox-5 agrees with
    Jazzbox position 5 and Reso-2 agrees with Reso position 2.
    """
    return _normalize(slot_name) == _normalize(f"{bank}-{position}")


def load_handbook(path: Path) -> list[Assignment]:
    text = _read_text(path)
    rows: list[Assignment] = []
    header_seen = False
    for line_number, line in enumerate(text.splitlines(), start=1):
        if not line or line.startswith("#"):
            continue
        if not header_seen:
            if line != HANDBOOK_HEADER:
                raise CompareError(f"{path}:{line_number}: unexpected handbook header")
            header_seen = True
            continue
        parts = line.split("\t")
        if len(parts) != 4:
            raise CompareError(f"{path}:{line_number}: expected four handbook columns")
        custom_bank, custom_position, source_bank, source_position = parts
        rows.append(
            Assignment(
                custom_bank,
                _integer_text(custom_position, path, line_number, "position"),
                source_bank,
                _integer_text(source_position, path, line_number, "position"),
            )
        )
    if not header_seen:
        raise CompareError(f"{path}: missing handbook header")
    found = [(row.custom_bank, row.custom_position) for row in rows]
    if found != list(EXPECTED_POSITIONS):
        raise CompareError(
            f"{path}: handbook must list Custom 1 positions 1 through 5, "
            "then Custom 2 positions 1 through 5"
        )
    for row in rows:
        bank_index(row.source_bank)
        if row.source_position < 1 or row.source_position > PRESETS_PER_BANK:
            raise CompareError(f"{path}: source position is outside 1..5")
    return rows


def load_manifest(path: Path) -> Manifest:
    text = _read_text(path)
    comments = [
        line
        for line in text.splitlines()
        if line.startswith("#") and FIRMWARE_MARK in line
    ]
    if not comments:
        raise CompareError(f"{path}: manifest does not name firmware 3.10 expanded defaults")
    slots: dict[int, Slot] = {}
    header_seen = False
    for line_number, line in enumerate(text.splitlines(), start=1):
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if not header_seen:
            if line != MANIFEST_HEADER:
                raise CompareError(f"{path}:{line_number}: unexpected manifest header")
            header_seen = True
            continue
        if len(parts) != 6:
            raise CompareError(f"{path}:{line_number}: expected six manifest columns")
        kind, index_text, _file_offset, size_text, sha256, name = parts
        if kind not in MANIFEST_KINDS:
            raise CompareError(f"{path}:{line_number}: unknown manifest kind: {kind}")
        if kind != "slot":
            continue
        index = _integer_text(index_text, path, line_number, "slot index")
        if index < 0 or index >= SLOT_COUNT:
            raise CompareError(f"{path}:{line_number}: slot index is outside 0..59")
        if index in slots:
            raise CompareError(f"{path}:{line_number}: duplicate slot {index}")
        if size_text != "4096":
            raise CompareError(f"{path}:{line_number}: slot {index} is not a 4096-byte slot")
        if SHA256_TEXT.fullmatch(sha256) is None:
            raise CompareError(f"{path}:{line_number}: slot {index} has no 64-digit sha256")
        if name == "":
            raise CompareError(f"{path}:{line_number}: slot {index} has an empty name")
        slots[index] = Slot(index, sha256, name)
    if not header_seen:
        raise CompareError(f"{path}: missing manifest header")
    missing = [index for index in range(SLOT_COUNT) if index not in slots]
    if missing:
        raise CompareError(f"{path}: manifest is missing slot {missing[0]}")
    return Manifest(slots)


def compare(
    assignments: list[Assignment],
    manifest: Manifest | None,
    baseline: str,
    derive: bool,
) -> list[tuple[str, str, str, str, str, str]]:
    if baseline not in BASELINE_ID:
        raise CompareError(f"unknown baseline: {baseline}")
    if baseline == "firmware-3.10" and manifest is None:
        raise CompareError(
            "firmware-3.10 baseline requires a factory manifest of the expanded defaults"
        )
    if derive and manifest is None:
        raise CompareError(
            "derived rows require a factory manifest of the 3.10 model-bank slots"
        )
    if len(assignments) != len(EXPECTED_POSITIONS):
        raise CompareError("handbook must list all ten Custom positions")
    baseline_id = BASELINE_ID[baseline]
    rows: list[tuple[str, str, str, str, str, str]] = []
    for assignment in assignments:
        values = _values(assignment, manifest, derive)
        fields = list(LOCATOR_FIELDS)
        fields.extend(MANIFEST_FIELDS)
        fields.extend(UNKNOWN_FIELDS)
        if derive:
            fields.extend(DERIVED_FIELDS)
        for field in fields:
            value, label = values[field]
            rows.append(
                (
                    baseline_id,
                    assignment.custom_bank,
                    str(assignment.custom_position),
                    field,
                    value,
                    label,
                )
            )
    return rows


def render(
    rows: list[tuple[str, str, str, str, str, str]],
    *,
    baseline: str,
    handbook: Path,
    manifest: Path | None,
    derive: bool,
) -> str:
    if baseline == "handbook":
        other = BASELINE_TITLE["firmware-3.10"]
    else:
        other = BASELINE_TITLE["handbook"]
    manifest_name = "not supplied" if manifest is None else str(manifest)
    lines = [
        f"# baseline: {BASELINE_TITLE[baseline]}",
        f"# other baseline: {other}",
        f"# handbook: {handbook}",
        f"# manifest: {manifest_name}",
        "# positions: 10",
        "# Positions are one-based, from bridge to neck.",
        "# custom_slot_index is the expanded-region index from bank*5+(5-position).",
        "# slot_bytes_match is whole-slot equality inside the supplied manifest.",
        "# Shipping tone knob, DSP program, unit calibration, and saved-tone flag stay unknown.",
        "# Which parameter differs is unknown when the slot hashes differ.",
        "# File offsets stay in the manifest.",
    ]
    if derive:
        lines.append("# Reconstruction rows are labeled derived.")
        lines.append("# A derived row names the 3.10 model-bank slot the handbook assigns.")
        lines.append("# Derived rows carry the slot index and name. Shipping-specific parameters stay unknown.")
    lines.append("\t".join(COLUMNS))
    for row in rows:
        if len(row) != len(COLUMNS):
            raise CompareError("comparison row must have six columns")
        if any("\t" in part or "\n" in part or "\r" in part for part in row):
            raise CompareError("comparison values must stay inside one column")
        lines.append("\t".join(row))
    return "\n".join(lines) + "\n"


def write_new(path: Path, text: str, protected: list[Path]) -> None:
    """Create path. Leave every existing file, including protected paths, unchanged."""
    parent = path.parent if path.parent.as_posix() not in ("", ".") else Path(".")
    if not parent.is_dir():
        raise CompareError(f"export directory does not exist: {parent}")
    target = path.resolve()
    for item in protected:
        if item.resolve() == target:
            raise CompareError(f"refusing to replace {item}")
    if path.exists() or path.is_symlink():
        raise CompareError(f"refusing to replace existing file: {path}")
    fd, temporary_name = tempfile.mkstemp(prefix=".handbook-compare-", dir=parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError as exc:
            raise CompareError(f"refusing to replace existing file: {path}") from exc
    finally:
        temporary.unlink(missing_ok=True)


def main(argv: list[str] | None = None) -> int:
    try:
        args = _parse_args(argv)
        assignments = load_handbook(args.handbook)
        manifest = load_manifest(args.manifest) if args.manifest is not None else None
        rows = compare(assignments, manifest, args.baseline, args.derive)
        text = render(
            rows,
            baseline=args.baseline,
            handbook=args.handbook,
            manifest=args.manifest,
            derive=args.derive,
        )
        if args.export is not None:
            protected = [args.handbook]
            if args.manifest is not None:
                protected.append(args.manifest)
            write_new(args.export, text, protected)
        sys.stdout.write(text)
    except CompareError as exc:
        print(exc, file=sys.stderr)
        return 1
    return 0


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare the ten Variax Custom positions against one named baseline."
    )
    parser.add_argument(
        "--baseline",
        choices=tuple(BASELINE_TITLE),
        default="handbook",
        help="handbook shipping assignments, or firmware 3.10 expanded defaults",
    )
    parser.add_argument(
        "--handbook",
        type=Path,
        default=Path("research/variax-300-handbook-custom.tsv"),
        help="handbook assignment table (default: research/variax-300-handbook-custom.tsv)",
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        help="factory manifest of firmware 3.10 expanded defaults",
    )
    parser.add_argument(
        "--derive",
        action="store_true",
        help="add derived rows naming the 3.10 model-bank slot for each handbook assignment",
    )
    parser.add_argument(
        "--export",
        type=Path,
        help="write the table to a new file; an existing path is left unchanged",
    )
    return parser.parse_args(argv)


def _values(
    assignment: Assignment, manifest: Manifest | None, derive: bool
) -> dict[str, tuple[str, str]]:
    values = {
        "source_bank": (assignment.source_bank, FIELD_LABELS["source_bank"]),
        "source_position": (str(assignment.source_position), FIELD_LABELS["source_position"]),
        "source_program": (str(assignment.source_program), FIELD_LABELS["source_program"]),
        "source_slot_index": (
            str(assignment.source_slot_index),
            FIELD_LABELS["source_slot_index"],
        ),
        "custom_program": (str(assignment.custom_program), FIELD_LABELS["custom_program"]),
        "custom_slot_index": (
            str(assignment.custom_slot_index),
            FIELD_LABELS["custom_slot_index"],
        ),
    }
    if manifest is None:
        for field in MANIFEST_FIELDS:
            values[field] = ("not-supplied", "absent")
    else:
        expanded = manifest.slot(assignment.custom_slot_index)
        source = manifest.slot(assignment.source_slot_index)
        name_match = names_agree(
            expanded.name, assignment.source_bank, assignment.source_position
        )
        bytes_match = expanded.sha256 == source.sha256
        values.update(
            {
                "model_slot_name": (source.name, FIELD_LABELS["model_slot_name"]),
                "model_name_matches": (
                    _yes_no(
                        names_agree(source.name, assignment.source_bank, assignment.source_position)
                    ),
                    FIELD_LABELS["model_name_matches"],
                ),
                "expanded_slot_name": (expanded.name, FIELD_LABELS["expanded_slot_name"]),
                "name_matches": (_yes_no(name_match), FIELD_LABELS["name_matches"]),
                "slot_bytes_match": (_yes_no(bytes_match), FIELD_LABELS["slot_bytes_match"]),
                "same_bytes_as": (
                    _format_slots(
                        manifest.others(expanded, lambda slot: slot.sha256 == expanded.sha256)
                    ),
                    FIELD_LABELS["same_bytes_as"],
                ),
                "same_name_different_bytes": (
                    _format_slots(
                        manifest.others(
                            expanded,
                            lambda slot: _normalize(slot.name) == _normalize(expanded.name)
                            and slot.sha256 != expanded.sha256,
                        )
                    ),
                    FIELD_LABELS["same_name_different_bytes"],
                ),
                "assignment_difference": (
                    _difference(name_match, bytes_match),
                    FIELD_LABELS["assignment_difference"],
                ),
            }
        )
    for field in UNKNOWN_FIELDS:
        values[field] = ("unknown", FIELD_LABELS[field])
    if derive:
        if manifest is None:
            raise CompareError(
                "derived rows require a factory manifest of the 3.10 model-bank slots"
            )
        source = manifest.slot(assignment.source_slot_index)
        values["reconstructed_from_slot"] = (
            str(source.index),
            FIELD_LABELS["reconstructed_from_slot"],
        )
        values["reconstructed_name"] = (source.name, FIELD_LABELS["reconstructed_name"])
        values["reconstruction"] = (RECONSTRUCTION, FIELD_LABELS["reconstruction"])
    return values


def _difference(name_match: bool, bytes_match: bool) -> str:
    if name_match and bytes_match:
        return "none"
    if name_match:
        return "bytes"
    if bytes_match:
        return "name"
    return "name-and-bytes"


def _format_slots(slots: list[Slot]) -> str:
    if not slots:
        return "none"
    return ", ".join(f"{slot.index} {slot.name}" for slot in slots)


def _yes_no(value: bool) -> str:
    if value:
        return "yes"
    return "no"


def _normalize(text: str) -> str:
    return "".join(character for character in text.casefold() if character.isalnum())


def _integer_text(text: str, path: Path, line_number: int, label: str) -> int:
    if not text.isdigit() or str(int(text)) != text:
        raise CompareError(f"{path}:{line_number}: {label} must be an integer")
    return int(text)


def _read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as exc:
        raise CompareError(f"cannot read {path}: {exc.strerror}") from exc


if __name__ == "__main__":
    sys.exit(main())
