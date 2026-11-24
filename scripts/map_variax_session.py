"""Check the Variax 300/600 3.10 session flag and DSP-bypass flag.

The checker compares derived addresses and control flow with
research/variax-310-session-handlers.json. It does not print disassembly.
"""

import argparse
import json
from pathlib import Path

from map_variax_buffer import decode_do_immediate
from map_variax_store import (
    ARCHIVE_SHA256,
    is_short_bra,
    is_short_bsr,
    program_image,
    require_branch,
)

BYPASS = 0x17A0
SESSION = 0x17F4
WRITE_OPCODES = {0x547000, 0x557000, 0x567000, 0x577000, 0x747000}
READ_OPCODES = {0x56F000, 0x57F000, 0x44F000, 0x45F000, 0x44F042, 0x45F042, 0x46F000}
ORI_MR = 0x0003F8


def absolute_uses(program, address):
    found = []
    for pc, word in program.items():
        if word != address or pc - 1 not in program:
            continue
        found.append((pc - 1, program[pc - 1]))
    return sorted(found)


def require_uses(program, address, expected, label):
    found = absolute_uses(program, address)
    if found != expected:
        raise ValueError(f"{label} references changed: {found}")
    unknown = [pc for pc, opcode in found if opcode not in WRITE_OPCODES | READ_OPCODES]
    if unknown:
        raise ValueError(f"{label} has an unclassified reference at {unknown}")


def calls_from(program, start, end):
    found = []
    pc = start
    while pc < end:
        word = program[pc]
        if word == 0x0BF080:
            found.append(program[pc + 1])
            pc += 2
            continue
        if word == 0x0AF080:
            found.append(program[pc + 1])
            pc += 2
            continue
        if is_short_bsr(word) or is_short_bra(word):
            from map_variax_store import short_target
            found.append(short_target(pc, word))
        pc += 1
    return found


def long_callers(program, target):
    return sorted(
        pc for pc, word in program.items()
        if word == 0x0BF080 and program.get(pc + 1) == target
    )


def range_contains(program, start, end, needles):
    return [
        pc for pc in range(start, end)
        if program.get(pc) in needles or (
            program.get(pc) == 0x0BF080 and program.get(pc + 1) in needles
        )
    ]


def inventory(archive):
    header, _image, program, _records = program_image(archive)
    require_uses(program, BYPASS, [
        (0x59D, 0x44F042),
        (0x633, 0x45F042),
        (0x1F90, 0x57F000),
        (0x2011, 0x57F000),
        (0x14023, 0x557000),
        (0x14563, 0x547000),
    ], "bypass")
    require_uses(program, SESSION, [
        (0x135E, 0x57F000),
        (0x179C, 0x747000),
        (0x14282, 0x557000),
        (0x144E0, 0x547000),
        (0x14A0D, 0x56F000),
        (0x14A20, 0x57F000),
    ], "session")

    if program[0x144DF] != 0x014186:
        raise ValueError("0x2A no longer keeps only bit 0")
    if not is_short_bra(program[0x144E2]):
        raise ValueError("0x2A no longer shares the trailing read")
    require_branch(program, 0x144E2, 0x14565, "bra")
    if [program[pc] for pc in range(0x14561, 0x14569)] != [
        0x0BF080, 0x1257, 0x547000, BYPASS, 0x0BF080, 0x1257, 0x0AF080, 0x1483E,
    ]:
        raise ValueError("0x4C no longer stores the byte and reads one trailer")
    if any(program[pc] == 0xF7 for pc in list(range(0x144DD, 0x144E3)) + list(range(0x14561, 0x14569))):
        raise ValueError("the flag handlers gained an F7 test")
    if program[0x14563] in {0x014186}:
        raise ValueError("0x4C masks the stored byte")

    if (program[0x0000], program[0x0001]) != (0x0AF080, 0x0DBC):
        raise ValueError("reset no longer enters at P:0x0DBC")
    if (program[0x0DCC], program[0x0DCD]) != (0x0AF080, 0x14004):
        raise ValueError("reset entry no longer jumps to P:0x14004")
    if long_callers(program, 0x1A13) != [0x14014]:
        raise ValueError("the startup routine gained or lost a caller")
    if long_callers(program, 0x14018) != [0x1A25]:
        raise ValueError("the bypass clear gained or lost a caller")
    if long_callers(program, 0x14274) != [0x1A21]:
        raise ValueError("the session clear gained or lost a caller")
    if program[0x1401E] != 0x20001B or program[0x14274] != 0x20001B:
        raise ValueError("a reset clear no longer starts from a cleared accumulator")
    if program[0x179B] != 0x3C0000:
        raise ValueError("the queued session clear no longer stores zero")

    if (program[0x5A1], program[0x5A2]) != (0x0AF0A2, 0x062B):
        raise ValueError("the audio gate no longer leaves the model dispatch")
    if (program[0x636], program[0x637]) != (0x0AF0A2, 0x03DD):
        raise ValueError("a set bypass flag no longer selects the short mix")
    if program[0x3E5] != 0x055555:
        raise ValueError("the short mix coefficient changed")
    if decode_do_immediate(program[0x3E6]) != 0x10 or decode_do_immediate(program[0x3E9]) != 6:
        raise ValueError("the short mix loop counts changed")
    if [program[pc] for pc in (0x3ED, 0x3EE, 0x3EF, 0x3F0)] != [0x565B00, 0x565D00, 0x565B00, 0x565D00]:
        raise ValueError("the short mix no longer stores the accumulator")
    if program[0x14A65] != 0x881 or (0x881 & (1 << 20)) != 0:
        raise ValueError("startup no longer writes an event word with bit 20 clear")
    if (program[0x593], program[0x597], program[0x598]) != (0x5EF000, 0x0140C6, 0x100000):
        raise ValueError("the model gate no longer tests bit 20 of the event word")
    if (program[0x599], program[0x59B], program[0x59F], program[0x5A0]) != (
        0x44F000, 0x45F000, 0x200062, 0x200042,
    ):
        raise ValueError("the model gate no longer combines the three X flags")
    if (program[0x62B], program[0x631], program[0x632], program[0x635]) != (
        0x44F000, 0x0140C6, 0x100000, 0x200062,
    ):
        raise ValueError("the short-mix gate no longer combines bit 20 and the bypass flag")
    if program[0x638] != 0x56F000 or program[0x639] != 0x17ED:
        raise ValueError("the third flag moved into the short-mix test")
    if program[0x123E] != 0xFF or (program[0x1258], program[0x1259]) != (0x0BF080, 0x1A97):
        raise ValueError("an empty read no longer calls the notice helper and retries")
    if (program[0xE6D], program[0xE6E]) != (0x0A7074, 0xD0):
        raise ValueError("the audio interrupt no longer sets bit 20")
    if (program[0x14BFC], program[0x14BFD], program[0x14BFE]) != (0x3B0000, 0x7B7000, 0xD0):
        raise ValueError("startup no longer clears the event word")
    if (program[0x1A1F], program[0x1A20], program[0x1A30], program[0x1A31]) != (
        0x0BF080, 0x14BFC, 0x0BF080, 0x14A64,
    ):
        raise ValueError("startup no longer clears the event word before rewriting it")
    if program[0x201C] != 0x3B0000 or (program[0x201D], program[0x201E]) != (0x7B7000, 0xD0):
        raise ValueError("the reload wait no longer clears the event word")
    if program[0x2004] != 0x01418D:
        raise ValueError("the reload wait no longer returns early when its flag is 1")

    for start, end, label in (
        (0x142B3, 0x14326, "SysEx header"),
        (0x14668, 0x14690, "0x43"),
        (0x14731, 0x1474A, "0x61"),
        (0x0FA6, 0x0FD2, "store"),
        (0x111B, 0x1124, "program-change action"),
    ):
        hit = range_contains(program, start, end, {SESSION, BYPASS, 0x17FC, 0x17ED})
        if hit:
            raise ValueError(f"{label} now references a gate flag at {hit}")
    if range_contains(program, 0x0F17, 0x0F8C, {0x11EF, 0x10A9, 0x1257, 0x1A97, ORI_MR}):
        raise ValueError("a flash poll now parses input or masks interrupts")
    if program[0x212C] != 0x000004 or program[0x2108] != 0x131B:
        raise ValueError("the byte interrupt no longer queues at X:0x131B")
    if program[0x0E8E] != 0x000004 or calls_from(program, 0x0DE4, 0x0E58).count(0x03F2) != 2:
        raise ValueError("the audio interrupt no longer reaches the mix gate")
    if (program[0x3F6], program[0x3F7]) != (0x0BF080, 0x0593):
        raise ValueError("the audio interrupt no longer reaches the mix gate")

    if program[0x120D] != 0xC0:
        raise ValueError("the program-change status compare changed")
    require_branch(program, 0x1211, 0x121E, "bcc")
    if calls_from(program, 0x121E, 0x1228) != [0x1257, 0x14285, 0x1232]:
        raise ValueError("program change no longer reaches P:0x14285")
    if (program[0x14289], program[0x1428A]) != (0x0ACC70, 0x0ACC72):
        raise ValueError("program change no longer marks the queued word as class 5")
    if calls_from(program, 0x111B, 0x1123) != [0x1FA7, 0x1E9C]:
        raise ValueError("class 5 no longer loads and reloads")
    if (program[0x1DF1], program[0x1DFB]) != (0x070001, 0x070002):
        raise ValueError("the queued session-clear words changed")
    if long_callers(program, 0x174D) != [0x10D6]:
        raise ValueError("the session-clear table gained or lost a caller")

    return {
        "archive_sha256": ARCHIVE_SHA256,
        "product_id": header["product_id"],
        "version_hex": header["version_hex"],
        "bypass": {
            "opcode": "0x4C",
            "handler": "P:0x14561",
            "flag": "X:0x17A0",
            "stored_bits": "the whole received byte",
            "trailing_byte_checked_for_f7": False,
            "reply": None,
            "writes": ["P:0x14563", "P:0x14023"],
            "reset_writer": "P:0x14023 writes 0, called only from the reset startup at P:0x1A25",
            "nonzero_selects": "P:0x03DD",
            "mix_coefficient": "0x055555",
            "mix_input_groups": 16,
            "mix_samples_per_group": 6,
            "mix_stores_zero": False,
        },
        "session": {
            "opcode": "0x2A",
            "handler": "P:0x144DD",
            "flag": "X:0x17F4",
            "stored_bits": "bit 0",
            "trailing_byte_checked_for_f7": False,
            "reply": None,
            "writes": ["P:0x144E0", "P:0x14282", "P:0x179C"],
            "readers": ["P:0x135E", "P:0x14A0D", "P:0x14A20"],
            "required_by_0x43": False,
            "required_by_0x61": False,
        },
        "program_change": {
            "status": "0xC0",
            "parser": "P:0x121E",
            "action": "P:0x111B calls P:0x1FA7 and P:0x1E9C",
            "writes_bypass_flag": False,
            "writes_session_flag": False,
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
            raise ValueError(f"session trace differs from {args.check}")
        else:
            print("PASS: session and bypass handlers match the checked-in trace")
    except (KeyError, OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
