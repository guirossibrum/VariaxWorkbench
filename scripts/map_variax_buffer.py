"""Check the Variax 300/600 3.10 edit-buffer handlers against the archive.

Opcode 0x43 replaces the current preset buffer. Opcode 0x3B patches runs
inside it. The checker compares derived addresses and control flow with
research/variax-310-buffer-handlers.json. It does not print disassembly.
"""

import argparse
import json
from pathlib import Path

from map_variax_store import (
    ARCHIVE_SHA256,
    is_short_bra,
    program_image,
    require_branch,
    short_target,
)

PRESET = 0xA47


def decode_do_immediate(opcode):
    """Return the count of `do #n,end`, or None when the word is not that form."""
    if opcode & 0xFF00F0 != 0x060080:
        return None
    return ((opcode & 0xF) << 8) | ((opcode >> 8) & 0xFF)


def calls_between(program, start, end):
    found = []
    pc = start
    while pc < end:
        word = program[pc]
        if word == 0x0BF080:
            found.append((pc, program[pc + 1]))
            pc += 2
            continue
        pc += 1
    return found


def require_long_jump(program, pc, opcode, target):
    if program[pc] != opcode or program[pc + 1] != target:
        raise ValueError(f"P:{pc:#x} does not jump to {target:#x}")


def inventory(archive):
    header, _image, program, _records = program_image(archive)
    if decode_do_immediate(program[0x126B]) != 6:
        raise ValueError("the word reader does not take six nibbles")
    if any(program[address] == 0xF7 for address in range(0x1268, 0x127E)):
        raise ValueError("the word reader gained a terminator check")
    if decode_do_immediate(program[0x20A0]) != 0x443:
        raise ValueError("the preset checksum no longer sums 1,091 words")

    full_calls = [target for _, target in calls_between(program, 0x14668, 0x14686)]
    if full_calls != [0x1268, 0x1268, 0x1257, 0x2095]:
        raise ValueError(f"0x43 call order changed: {full_calls}")
    if program[0x14669] != PRESET or program[0x14671] != 0x14678:
        raise ValueError("0x43 no longer fills Y:0xA47 inside its counted loop")
    require_long_jump(program, 0x1467E, 0x0AF0A2, 0x1482D)
    require_long_jump(program, 0x14683, 0x0AF0AA, 0x1483E)
    require_long_jump(program, 0x14685, 0x0AF080, 0x1483A)
    if program[0x1483A] != 0x0BF080 or program[0x1483B] != 0x1E9C:
        raise ValueError("a matched 0x43 no longer reloads through P:0x1E9C")
    reload_calls = [target for _, target in calls_between(program, 0x1483A, 0x1483E)]
    if reload_calls != [0x1E9C]:
        raise ValueError(f"the matched-buffer reload changed: {reload_calls}")

    if not is_short_bra(program[0x14433]) or short_target(0x14433, program[0x14433]) != 0x1443F:
        raise ValueError("0x3B no longer begins at its run header")
    require_branch(program, 0x14444, 0x14466, "bcc")
    diff_calls = [target for _, target in calls_between(program, 0x14433, 0x1446C)]
    if diff_calls != [0x1268, 0x1261, 0x1261, 0x1268, 0x131B]:
        raise ValueError(f"0x3B call order changed: {diff_calls}")
    if 0x2095 in diff_calls:
        raise ValueError("0x3B gained a checksum")
    if program[0x14437] != 0x1443E or program[0x1445F] != PRESET:
        raise ValueError("0x3B no longer writes at Y:0xA47 + the second header word")
    if program[0x14467] != 0x71:
        raise ValueError("0x3B no longer replies 0x71")
    require_long_jump(program, 0x1446A, 0x0AF080, 0x1483A)

    return {
        "archive_sha256": ARCHIVE_SHA256,
        "product_id": header["product_id"],
        "version_hex": header["version_hex"],
        "full_buffer": {
            "opcode": "0x43",
            "handler": "P:0x14668",
            "count_source": "first word",
            "destination": "Y:0xA47",
            "bounds_check": False,
            "word_reader": "P:0x1268",
            "word_reader_nibbles": 6,
            "word_reader_checks_terminator": False,
            "terminator_after_count": "F7",
            "bad_terminator": "P:0x1482D replies 0x66 and does not reload",
            "checksum": "P:0x2095",
            "checksum_words": 1091,
            "checksum_failure": "P:0x1483E returns with no reply and no reload",
            "checksum_success": "P:0x1483A reloads through P:0x1E9C and sends no 0x71",
        },
        "difference": {
            "opcode": "0x3B",
            "handler": "P:0x14433",
            "header": "count word, then start word",
            "destination": "Y:0xA47 + start",
            "bounds_check": False,
            "checksum": False,
            "terminator": "F7",
            "on_terminator": "P:0x14466 replies 0x71, then P:0x1483A reloads",
            "editor_terminal_run": "start word, then count word, when the run includes word 1090",
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", type=Path, help="compare with the checked-in trace")
    args = parser.parse_args()
    try:
        if not args.archive.is_file():
            raise ValueError("archive must be a regular file")
        result = inventory(args.archive.read_bytes())
        if args.check is None:
            print(json.dumps(result, indent=2))
        elif json.loads(args.check.read_text()) != result:
            raise ValueError(f"buffer trace differs from {args.check}")
        else:
            print("PASS: edit-buffer handlers match the checked-in trace")
    except (KeyError, OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
