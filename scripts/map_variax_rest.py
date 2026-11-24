"""Check the remaining Variax 3.10 command handlers against the archive.

The checker records reply opcodes, the unsolicited send sites, and that
these handlers do not call the flash helpers directly. It does not print
disassembly.
"""

import argparse
import json
from pathlib import Path

from map_variax_flash import references
from map_variax_store import (
    ARCHIVE_SHA256,
    is_short_bcc,
    is_short_bra,
    program_image,
    short_target,
)

TABLE = 0x14326
FIRST = 0x10
JSR = 0x0BF080
MOVE_IMM_A = 0x56F400
FLASH = {0x0F17, 0x0F45, 0x0F63, 0x0FA6, 0x0F8C, 0x0F9A, 0x1ABA}

# Handler bodies and the routines they call. Each pair is start, end.
WINDOWS = (
    (0x14394, 0x14433),
    (0x1446C, 0x144A9),
    (0x144F6, 0x14561),
    (0x14569, 0x14583),
    (0x145FE, 0x14623),
    (0x1465B, 0x14668),
    (0x1469D, 0x146BD),
    (0x146CF, 0x14731),
    (0x147FE, 0x14805),
    (0x1378, 0x1393),
    (0x13B0, 0x14B0),
    (0x14A34, 0x14A4B),
    (0x14A58, 0x14A64),
    (0x14B0, 0x1584),
    (0x16F6, 0x171A),
    (0x17CE, 0x1866),
    (0x1AC6, 0x1AD2),
    (0x1AEC, 0x1BA3),
    (0x148DA, 0x14A0B),
)


def pc_text(address):
    return f"P:0x{address:04X}"


def require_word(program, address, word, label):
    if program.get(address) != word:
        raise ValueError(label)


def handler(program, opcode):
    return program[TABLE + opcode - FIRST]


def flash_calls(program, start, end):
    found = []
    pc = start
    while pc < end:
        word = program.get(pc)
        if word == JSR and program.get(pc + 1) in FLASH:
            found.append(pc_text(pc))
            pc += 2
            continue
        if word == JSR or (word is not None and (word & 0xFFF000) == 0x0AF000):
            pc += 2
            continue
        pc += 1
    return found


def immediate_send(program, address, opcode):
    require_word(program, address, MOVE_IMM_A, pc_text(address))
    require_word(program, address + 1, opcode, pc_text(address))


def trace(archive):
    header, _image, program, _records = program_image(archive)
    product = header["product_id"]
    version = header["version_hex"]
    if product != "00070004" or version != "03100000":
        raise ValueError("expected Variax 300/600 firmware 3.10")
    expected = {
        0x10: 0x1451D,
        0x16: 0x144FA,
        0x17: 0x14506,
        0x1C: 0x144F6,
        0x1D: 0x145FE,
        0x1E: 0x14618,
        0x23: 0x1465B,
        0x29: 0x1469D,
        0x35: 0x14569,
        0x36: 0x14513,
        0x37: 0x14518,
        0x3E: 0x143D7,
        0x3F: 0x1446C,
        0x40: 0x14545,
        0x41: 0x14554,
        0x47: 0x147FE,
        0x49: 0x1452A,
        0x4E: 0x143B5,
        0x5E: 0x144FE,
        0x5F: 0x14502,
        0x69: 0x14394,
        0x6A: 0x143A4,
        0x6B: 0x143C6,
        0x75: 0x146CF,
    }
    for opcode, address in expected.items():
        if handler(program, opcode) != address:
            raise ValueError(pc_text(address))
    for start, end in WINDOWS:
        found = flash_calls(program, start, end)
        if found:
            raise ValueError("flash call " + ", ".join(found))
    # 0x47 discards a fixed run, then falls into the shared 0x71 send.
    require_word(program, 0x147FE, 0x66F400, "0x47 count")
    require_word(program, 0x147FF, 0x2003, "0x47 count")
    require_word(program, 0x14800, 0x06D600, "0x47 loop")
    require_word(program, 0x14805, MOVE_IMM_A, "shared 0x71")
    require_word(program, 0x14806, 0x71, "shared 0x71")
    require_word(program, 0x14430, 0x0BE680, "0x3E indirect")
    immediate_send(program, 0x14488, 0x3F)
    immediate_send(program, 0x146A6, 0x29)
    immediate_send(program, 0x146DC, 0x75)
    immediate_send(program, 0x148DD, 0x1E)
    immediate_send(program, 0x1493D, 0x69)
    immediate_send(program, 0x1499A, 0x6A)
    immediate_send(program, 0x149D7, 0x6B)
    immediate_send(program, 0x149F1, 0x41)
    # Selection helper adds 55 only for a formed value of 5 through 54,
    # and then always calls the loader.
    if not is_short_bcc(program[0x11AE]) or short_target(0x11AE, program[0x11AE]) != 0x11B3:
        raise ValueError("helper lower skip")
    if not is_short_bcc(program[0x11B0]) or short_target(0x11B0, program[0x11B0]) != 0x11B3:
        raise ValueError("helper upper skip")
    require_word(program, 0x11B2, 0x017780, "add 55")
    # A non-F7 byte on the 0x7E header path branches to itself.
    if not is_short_bra(program[0x142E6]) or short_target(0x142E6, program[0x142E6]) != 0x142E6:
        raise ValueError("header spin")
    if not is_short_bcc(program[0x142BF]) or short_target(0x142BF, program[0x142BF]) != 0x142F5:
        raise ValueError("header bypass")
    require_word(program, 0x142BB, 0x7E, "universal byte")
    immediate_send(program, 0x1363, 0x28)
    immediate_send(program, 0x1AA6, 0x74)
    immediate_send(program, 0x1AB2, 0x4D)
    require_word(program, 0x14A14, 0x0ACC66, "program status bit 6")
    require_word(program, 0x14A15, 0x0ACC67, "program status bit 7")
    require_word(program, 0x14A27, 0x0ACC64, "cc status bit 4")
    require_word(program, 0x14A28, 0x0ACC65, "cc status bit 5")
    require_word(program, 0x14A29, 0x0ACC67, "cc status bit 7")
    if [pc for pc, kind in references(program, 0x0DCE) if kind == "jsr"] != [0x119C]:
        raise ValueError("0x28 caller")
    if [pc for pc, kind in references(program, 0x14A0B) if kind == "jsr"] != [0x11B9]:
        raise ValueError("program-change caller")
    cc_callers = [pc for pc, kind in references(program, 0x14A1D) if kind in {"jsr", "bsr"}]
    if cc_callers != [0x10EE, 0x1106, 0x11C1, 0x149D3]:
        raise ValueError("cc callers")
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "product_id": product,
        "version_hex": version,
        "handlers_without_direct_flash": [f"0x{opcode:02X}" for opcode in expected],
        "discard_then_71": {"opcode": "0x47", "count": 8195},
        "indirect_store_unknown": "0x3E",
        "header_spin_on_non_f7": "P:0x142E6",
        "header_spin_entered_from_byte": "0x7E",
        "unsolicited": [
            {"opcode": "0x28", "send": "P:0x1363", "only_counter_caller": "P:0x119C"},
            {"opcode": "0x74", "send": "P:0x1AA6"},
            {"opcode": "0x4D", "send": "P:0x1AB2"},
        ],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", type=Path)
    args = parser.parse_args()
    result = trace(args.archive.read_bytes())
    if args.check is None:
        print(json.dumps(result, indent=2))
    elif json.loads(args.check.read_text()) != result:
        raise SystemExit("flash-rest trace does not match the checked-in file")
    else:
        print("PASS: remaining handlers match the checked-in trace")


if __name__ == "__main__":
    main()
