"""Read the Variax 300/600 3.10 dispatch table from the private archive."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from inspect_firmware import inspect
from map_firmware import loader_records

ARCHIVE_SHA256 = "e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a"
TABLE_BASE = 0x14326
FIRST_OPCODE = 0x10
LAST_OPCODE = 0x7D
RETURN = 0x1483E


def inventory(archive):
    if hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("expected the exact archived Variax 300/600 3.10 image")

    header, image = inspect(archive)
    region = header["regions"][1]
    main = image[region["offset"]:region["offset"] + region["bytes"]]
    program = {}
    for record in loader_records(main)["records"]:
        if record["memory_tag_raw"] != 0:
            continue
        start = record["destination_word_raw"]
        offset = record["header_byte_offset"] + 9
        for index in range(record["word_count"]):
            address = start + index
            if address in program:
                raise ValueError(f"overlapping P word at {address:#x}")
            word = offset + 3 * index
            program[address] = int.from_bytes(main[word:word + 3], "little")

    targets = {
        f"0x{opcode:02X}": program[TABLE_BASE + opcode - FIRST_OPCODE]
        for opcode in range(FIRST_OPCODE, LAST_OPCODE + 1)
    }
    if any(target not in program or TABLE_BASE <= target <= TABLE_BASE + LAST_OPCODE - FIRST_OPCODE
           for target in targets.values()):
        raise ValueError("dispatcher entry points outside loaded code or back into table")

    counts = Counter(targets.values())
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "product_id": header["product_id"],
        "version_hex": header["version_hex"],
        "dispatch_entry": "P:0x14316",
        "table_base": f"P:0x{TABLE_BASE:X}",
        "table_end_inclusive": f"P:0x{TABLE_BASE + LAST_OPCODE - FIRST_OPCODE:X}",
        "entry_count": len(targets),
        "direct_return_count": counts[RETURN],
        "distinct_nonreturn_targets": len(counts.keys() - {RETURN}),
        "target_confidence": "confirmed",
        "targets": {opcode: f"P:0x{target:X}" for opcode, target in targets.items()},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", type=Path, help="compare with a checked-in inventory")
    args = parser.parse_args()
    try:
        if not args.archive.is_file():
            raise ValueError("archive must be a regular file")
        result = inventory(args.archive.read_bytes())
        if args.check is None:
            print(json.dumps(result, indent=2))
        else:
            if json.loads(args.check.read_text()) != result:
                raise ValueError(f"inventory differs from {args.check}")
            print(f"PASS: all {result['entry_count']} dispatcher targets match")
    except (KeyError, OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
