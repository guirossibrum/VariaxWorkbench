#!/usr/bin/env python3
"""Render the public #114 inventory candidate from tracked evidence."""

import argparse
import hashlib
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROJECTION = "research/audio-inventory-schema2.tsv"
UNDERSCORE = "supported_devices.txt"
HYPHEN = "supported-devices.txt"
IDENTIFIERS = "research/device-inventory-identifiers.tsv"
OUTPUT = "research/device-inventory.tsv"
SOURCE_COMMIT = "a55a70a854f56fcfd3c9e3582ce746416f8a3884"
PINS = {
    PROJECTION: "c3ed43ad76a0486ef327cb0cf1c42d389124c765418b0065b29f2d1db37ba790",
    UNDERSCORE: "7ccae7e58b3f645552b927e9dc9e7f85ca1aa6f9f32ad1a56e5b3257e0584b0b",
    HYPHEN: "e972981c43d24610f32fa8723b3c705af153940f354727f421c55ac54aad8d7b",
}
ID_WIDTHS = {"usb_vid": 4, "usb_pid": 4, "l6_product": 8}
SOURCE_LOCATION = re.compile(r"(?:src/[^:]+\.rs|docs/[^:]+\.md):[1-9]\d*(?:-[1-9]\d*)?")


def read_lines(root, path):
    # Hash canonical LF bytes, including the final newline, on every platform.
    data = (root / path).read_bytes().replace(b"\r\n", b"\n")
    if path in PINS and hashlib.sha256(data).hexdigest() != PINS[path]:
        raise ValueError(f"source digest mismatch for {path}")
    return data.decode("utf-8").splitlines()


def read_identifiers(root, projection):
    revision, heading, *lines = read_lines(root, IDENTIFIERS)
    if revision != f"source_revision\t{SOURCE_COMMIT}":
        raise ValueError(f"unexpected citation revision in {IDENTIFIERS}")
    if heading != "ordinal\tname\tfield\tvalue\tsource_location":
        raise ValueError(f"unexpected columns in {IDENTIFIERS}")
    identifiers = defaultdict(dict)
    for line_number, line in enumerate(lines, 3):
        ordinal_text, name, field, value, source = line.split("\t")
        ordinal = int(ordinal_text)
        if not 1 <= ordinal <= len(projection):
            raise ValueError(f"identifier ordinal out of range on line {line_number}")
        frozen_name, role = projection[ordinal - 1]
        if name != frozen_name or role == "alias":
            raise ValueError(f"identifier requires a matching non-alias row on line {line_number}")
        if field not in ID_WIDTHS or field in identifiers[ordinal]:
            raise ValueError(f"invalid or repeated identifier field on line {line_number}")
        if value == "unresolved":
            if source != "-":
                raise ValueError(f"unresolved identifier has a source on line {line_number}")
        elif not (
            re.fullmatch(f"0x[0-9A-F]{{{ID_WIDTHS[field]}}}", value)
            and SOURCE_LOCATION.fullmatch(source)
        ):
            raise ValueError(f"identifier requires a formatted value and public citation on line {line_number}")
        identifiers[ordinal][field] = value
    return identifiers


def draft_lines(root, path, frozen_names):
    by_name = defaultdict(list)
    for number, line in enumerate(read_lines(root, path), 1):
        name = line.split(" | ", 1)[0] if path == HYPHEN else line
        if name not in frozen_names:
            raise ValueError(f"unmapped draft row {path}:{number}")
        by_name[name].append(str(number))
    return {name: ",".join(numbers) for name, numbers in by_name.items()}


def render(root=ROOT):
    heading, *lines = read_lines(root, PROJECTION)
    if heading != "name\trole":
        raise ValueError(f"unexpected columns in {PROJECTION}")
    projection = [line.split("\t") for line in lines]
    frozen_names = {name for name, _ in projection}
    underscore = draft_lines(root, UNDERSCORE, frozen_names)
    hyphen = draft_lines(root, HYPHEN, frozen_names)
    identifiers = read_identifiers(root, projection)
    output = [
        "schema\t3",
        "\t".join(("ordinal", "name", "role", *ID_WIDTHS, "underscore_lines", "hyphen_lines")),
    ]
    for ordinal, (name, role) in enumerate(projection, 1):
        values = identifiers.get(ordinal, {})
        output.append("\t".join((
            str(ordinal), name, role, *(values.get(field, "-") for field in ID_WIDTHS),
            underscore.get(name, "-"), hyphen.get(name, "-"),
        )))
    return ("\n".join(output) + "\n").encode("utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify the committed output byte for byte")
    args = parser.parse_args()
    try:
        output = render()
        if args.check:
            if output != (ROOT / OUTPUT).read_bytes():
                raise ValueError(f"generated output differs from {OUTPUT}")
        else:
            sys.stdout.buffer.write(output)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
