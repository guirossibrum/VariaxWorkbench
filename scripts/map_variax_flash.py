"""Check Variax 300/600 3.10 flash handlers against the archive.

The checker compares sector ranges, call targets and reply shapes with
research/variax-310-flash-handlers.json. It does not print disassembly,
and it does not record how a code-load transfer is requested.
"""

import argparse
import json
from pathlib import Path

from map_variax_buffer import decode_do_immediate
from map_variax_store import (
    ARCHIVE_SHA256,
    MAC_Y0_12,
    is_short_bcc,
    is_short_bra,
    is_short_bsr,
    program_image,
    require_branch,
    short_target,
)

TABLE = 0x14326
FIRST_OPCODE = 0x10
SLOT_BASE = 0xD00000
SECTOR = 0x1000
PRESET_WORDS = 0x444
FLASH_BYTES = PRESET_WORDS * 3
LOW_WINDOW = 0x21000
ERASE = 0x0F63
PROGRAM = 0x0F17
BYTE_PROGRAM = 0x0F45
STORE = 0x0FA6
RESET = 0x1ABA
JSR = 0x0BF080
MOVE_IMM_A = 0x56F400
MOVE_M7_M0 = 0x0460A7
BSET_PDRE4 = 0x011D64
BCLR_PDRE4 = 0x011D44
OVERLAP = (
    "unresolved; contradicted by owner backup "
    "(program 63 loads a distinct valid program)"
)


def handler(program, opcode):
    return program[TABLE + opcode - FIRST_OPCODE]


def pc_text(address):
    return f"P:0x{address:04X}"


def require_word(program, address, word, label):
    found = program.get(address)
    if found != word:
        raise ValueError(label)


def immediate_pair(program, address, label):
    """Return the immediate of `move #>imm,a` at `address`."""
    if program.get(address) != MOVE_IMM_A:
        raise ValueError(label)
    return program[address + 1]


def references(program, target):
    hits = []
    extension = set()
    for pc, word in program.items():
        if word == JSR or (word & 0xFFF000) == 0x0AF000:
            extension.add(pc + 1)
    for pc in sorted(program):
        word = program[pc]
        nxt = program.get(pc + 1)
        if word == JSR and nxt == target:
            hits.append((pc, "jsr"))
        elif (word & 0xFFF000) == 0x0AF000 and nxt == target:
            hits.append((pc, "abs"))
        elif is_short_bsr(word) and short_target(pc, word) == target:
            hits.append((pc, "bsr"))
        elif is_short_bra(word) and short_target(pc, word) == target:
            hits.append((pc, "bra"))
        elif is_short_bcc(word) and short_target(pc, word) == target:
            hits.append((pc, "bcc"))
        elif word == target and pc not in extension:
            prev = program.get(pc - 1, 0)
            kind = "extension" if (prev & 0xFF0000) == 0x0A0000 else "literal"
            hits.append((pc, kind))
    return hits


def call_sites(program, target, kinds):
    return [pc for pc, kind in references(program, target) if kind in kinds]


def require_sites(program, target, kinds, expected, label):
    found = call_sites(program, target, kinds)
    if found != expected:
        rendered = ", ".join(pc_text(pc) for pc in found)
        raise ValueError(f"{label}: {rendered}")


def require_abs(program, pc, target, label):
    word = program.get(pc)
    if word is None or (word & 0xFFF000) != 0x0AF000 or program.get(pc + 1) != target:
        raise ValueError(label)


def loop_advances(program, do_pc):
    """Count `(r6)+` words from the instruction after a two-word DO through its end."""
    end = program[do_pc + 1]
    pc = do_pc + 2
    count = 0
    while pc <= end:
        if program.get(pc) == 0x205E00:
            count += 1
        pc += 1
    return count


def walk_end(program, move_pc, do_pc, repeats=1):
    """End address of a byte walk. The start is the middle byte of a one-word move."""
    word = program[move_pc]
    if (word >> 16) != 0x26:
        raise ValueError("a low flash walk no longer starts with a one-word move")
    start = (word >> 8) & 0xFF
    count_word = program[do_pc]
    count = decode_do_immediate(count_word)
    if count is None:
        count = program[do_pc - 1]
    return start + loop_advances(program, do_pc) * count * repeats


def opcode_sites(program, opcode):
    return sorted(pc for pc, word in program.items() if word == opcode)


def require_opcode_sites(program, opcode, expected, label):
    found = opcode_sites(program, opcode)
    if found != expected:
        rendered = ", ".join(pc_text(pc) for pc in found)
        raise ValueError(f"{label}: {rendered}")


def require_absent(program, start, end, opcode, label):
    if any(program.get(pc) == opcode for pc in range(start, end)):
        raise ValueError(label)


def bus_map(program):
    """Address attributes and the Port E bit in force for the flash paths."""
    require_word(program, 0xDBC, 0x08F4B9, "AAR0 lost its movep")
    require_word(program, 0xDBD, 0x000539, "AAR0 lost its value")
    require_word(program, 0xDBE, 0x08F4B8, "AAR1 lost its movep")
    require_word(program, 0xDBF, 0xD00431, "AAR1 lost its value")
    require_word(program, 0xDC0, 0x08F4B7, "AAR2 lost its movep")
    require_word(program, 0xDC1, 0x000001, "AAR2 lost its value")
    require_word(program, 0xDC2, 0x08F4B6, "AAR3 lost its movep")
    require_word(program, 0xDC3, 0x000001, "AAR3 lost its value")
    require_opcode_sites(
        program,
        0x08F4B9,
        [0xDBC],
        "AAR0 is written more than once",
    )
    require_opcode_sites(program, 0x08F4B8, [0xDBE], "AAR1 is written more than once")
    require_opcode_sites(program, 0x08F4B7, [0xDC0], "AAR2 is written more than once")
    require_opcode_sites(program, 0x08F4B6, [0xDC2], "AAR3 is written more than once")
    require_word(program, 0x14004, 0x08F4BB, "BCR lost its movep")
    require_word(program, 0x14005, 0x1F27C2, "BCR lost its value")
    require_opcode_sites(program, 0x08F4BB, [0x14004], "BCR is written more than once")
    require_word(program, 0x20A6, 0x51F400, "the loader address helper lost its base move")
    require_word(program, 0x20A7, SLOT_BASE, "the loader address helper lost its base")
    require_word(program, 0x20A8, 0x010CEA, "the loader address helper lost its 4 KB step")
    require_word(program, 0x20A9, 0x213000, "the loader address helper lost its pointer move")
    require_opcode_sites(
        program,
        BSET_PDRE4,
        [0xF8D, 0xFA7, 0xFD4, 0xFF7, 0x1FE1, 0x141B7, 0x14750, 0x148BC],
        "PDRE bit 4 gained or lost a set",
    )
    require_opcode_sites(
        program,
        BCLR_PDRE4,
        [
            0xF97,
            0xFCF,
            0xFEC,
            0x100F,
            0x101B,
            0x105B,
            0x1FF9,
            0x140E8,
            0x141BB,
            0x141D6,
            0x14796,
            0x148D2,
        ],
        "PDRE bit 4 gained or lost a clear",
    )
    require_absent(program, 0x14623, 0x14647, BSET_PDRE4, "0x6C sets PDRE bit 4")
    require_absent(program, 0x1416B, 0x1417E, BSET_PDRE4, "the name commit sets PDRE bit 4")
    require_absent(program, 0x14583, 0x1458C, BSET_PDRE4, "0x56 sets PDRE bit 4")
    require_absent(program, 0x14142, 0x14150, BSET_PDRE4, "the string commit sets PDRE bit 4")
    require_absent(program, 0x0F9A, 0x0FA6, BSET_PDRE4, "the 32-start erase sets PDRE bit 4")
    require_absent(program, 0x101B, 0x1059, BSET_PDRE4, "the high code erase sets PDRE bit 4")
    require_absent(program, 0x105B, 0x1066, BSET_PDRE4, "the low code erase sets PDRE bit 4")
    require_word(program, 0x141D2, 0x07B49E, "Port E direction lost its movep")
    require_word(program, 0x141D3, 0x000010, "Port E direction lost bit 4")
    require_word(program, 0x141D4, 0x07B49F, "Port E control lost its movep")
    require_word(program, 0x141D5, 0x000000, "Port E control is no longer GPIO")
    require_sites(program, 0x141CD, ("jsr",), [0x1A13], "Port E init gained or lost a caller")
    require_branch(program, 0x1489B, 0x148B9, "bcc")
    require_branch(program, 0x148B8, 0x148D3, "bra")
    reply_pcs = list(range(0x1241, 0x1257))
    reply_pcs += list(range(0x1367, 0x1378))
    reply_pcs += list(range(0x1488C, 0x148D9))
    for pc in reply_pcs:
        word = program.get(pc, 0)
        if (word & 0xFFFF00) in {0x08F400, 0x07B400}:
            raise ValueError("the slot-read reply writes a peripheral register")
    return {
        "aar0": {
            "value": "0x000539",
            "written_at": pc_text(0xDBC),
            "spaces": ["X", "Y", "P"],
            "window": "0x000000-0x07FFFF",
            "pin": "AA0",
            "active_low": True,
            "access": "SRAM",
        },
        "aar1": {
            "value": "0xD00431",
            "written_at": pc_text(0xDBE),
            "spaces": ["X", "Y"],
            "window": "0xD00000-0xDFFFFF",
            "pin": "AA1",
            "active_low": True,
            "access": "SRAM",
            "packing": False,
        },
        "aar2": {"value": "0x000001", "spaces_enabled": False},
        "aar3": {"value": "0x000001", "spaces_enabled": False},
        "bcr": {
            "value": "0x1F27C2",
            "written_at": pc_text(0x14004),
            "selects_between_these_paths": False,
        },
        "address_helpers_add_offset": False,
        "pdre_bit4": {
            "register": "Y:0xFFFF9D",
            "pin": "PE4",
            "manual_name": "ESAI_1 FST",
            "gpio_output_programmed_at": pc_text(0x141CD),
            "called_from": pc_text(0x1A13),
            "board_net": "unknown",
        },
        "external_address_pins": "A0-A17",
        "index_plus_64_same_a0_a17": True,
        "physical_overlap_opposite_bit4": OVERLAP,
    }


def inventory(archive):
    header, _image, program, _records = program_image(archive)
    if header["product_id"] != "00070004" or header["version_hex"] != "03100000":
        raise ValueError("archive header is not Variax 300/600 3.10")

    require_word(program, 0x0F72, 0x51F400, "the erase helper lost its base move")
    require_word(program, 0x0F73, SLOT_BASE, "the erase helper lost its base address")
    require_word(program, 0x0F74, 0x010CCA, "the erase helper lost its 4 KB step")
    if program[0x0F81] != 0x0140CD or program[0x0F82] != 0x0001E8:
        raise ValueError("the erase restart compare changed")
    require_branch(program, 0x0F83, ERASE, "bcc")
    if program[0x0F58] != 0x0140CD or program[0x0F59] != 0x002710:
        raise ValueError("the byte-program restart compare changed")
    require_branch(program, 0x0F5A, 0x0F46, "bcc")

    user_start = immediate_pair(program, 0x0F8E, "the 64-start erase lost its immediate")
    user_count = decode_do_immediate(program[0x0F90])
    spare_start = immediate_pair(program, 0x0F9B, "the 32-start erase lost its immediate")
    spare_count = decode_do_immediate(program[0x0F9D])
    code_start = immediate_pair(program, 0x101C, "the code erase lost its immediate")
    code_count = decode_do_immediate(program[0x101E])
    if program[0x105C] != 0x200013 or decode_do_immediate(program[0x105D]) != 4:
        raise ValueError("the low code erase is no longer four sectors from zero")
    if user_count is None or spare_count is None or code_count is None:
        raise ValueError("an erase loop is no longer an immediate DO")

    direct = ("jsr", "bsr")
    require_sites(program, 0x0F8C, direct, [0x146BD], "the 64-start erase gained or lost a caller")
    require_sites(program, 0x0F9A, direct, [0x146C0], "the 32-start erase gained or lost a caller")
    require_sites(
        program,
        ERASE,
        ("jsr", "bsr", "bcc"),
        [0x0F83, 0x0F94, 0x0FA1, 0x0FC4, 0x0FCE, 0x1022, 0x1061, 0x14144, 0x1416D],
        "the erase helper gained or lost a caller",
    )
    require_sites(program, 0x1016, direct, [0x12AA], "the high code commit gained or lost a caller")
    require_sites(program, 0x1059, direct, [0x12FC], "the low code commit gained or lost a caller")
    if handler(program, 0x63) != 0x146BD or handler(program, 0x68) != 0x146C0:
        raise ValueError("0x63 or 0x68 no longer points at the wide erases")
    if program[0x146C2] != JSR or program[0x146C3] != 0x1257:
        raise ValueError("the wide erases no longer run before the trailer read")

    level_sector = immediate_pair(program, 0x14142, "string-level erase lost its sector")
    level_count = immediate_pair(program, 0x1414B, "string-level program lost its count")
    require_word(program, 0x14149, 0x64F400, "string levels lost their destination move")
    require_word(program, 0x1414A, 0xD04000, "string levels lost their destination")
    name_sector = immediate_pair(program, 0x1416B, "bank-name erase lost its sector")
    name_count = immediate_pair(program, 0x14173, "bank-name program lost its count")
    require_word(program, 0x14171, 0x64F400, "bank names lost their destination move")
    require_word(program, 0x14172, 0xD08000, "bank names lost their destination")
    require_word(program, 0x14177, 0x60F400, "the bank-name marker lost its move")
    require_word(program, 0x14178, 0xD081FE, "the bank-name marker lost its address")
    if immediate_pair(program, 0x14179, "the bank-name marker lost its byte") != 0x45:
        raise ValueError("the bank-name marker byte changed")
    require_sites(program, 0x14142, direct, [0x1410B, 0x1458A, 0x14BC9], "string-level commit gained or lost a caller")
    require_sites(program, 0x1416B, ("jsr", "bsr"), [0x1411D, 0x14645], "bank-name commit gained or lost a caller")
    if program[0x14645] != JSR or program[0x14647] != JSR:
        raise ValueError("0x6C no longer commits before its trailer read")
    require_abs(program, 0x14588, 0x1482D, "0x56 no longer rejects a bad trailer before the erase")
    if program[0x1458A] != JSR or program[0x1458B] != 0x14142:
        raise ValueError("0x56 no longer commits only after its trailer matches")
    if handler(program, 0x54) != 0x144E3 or handler(program, 0x56) != 0x14583:
        raise ValueError("0x54 or 0x56 moved")
    if handler(program, 0x57) != 0x1458E or handler(program, 0x6C) != 0x14623:
        raise ValueError("0x57 or 0x6C moved")
    if handler(program, 0x6D) != 0x14650:
        raise ValueError("0x6D moved")
    if decode_do_immediate(program[0x144E6]) != 6:
        raise ValueError("0x54 no longer receives six words")
    if program[0x144F4] != 0x0AF080 or program[0x144F5] != 0x14647:
        raise ValueError("0x54 no longer joins the trailer instead of committing")
    if immediate_pair(program, 0x14597, "0x57 lost its reply opcode") != 0x57:
        raise ValueError("0x57 no longer replies 0x57")
    if decode_do_immediate(program[0x145A1]) != 6:
        raise ValueError("0x57 no longer sends six words")
    if any(target in {ERASE, PROGRAM, BYTE_PROGRAM, STORE} for _pc, target in _calls_in(program, 0x144E3, 0x144F6)):
        raise ValueError("0x54 now calls a flash helper")
    if any(target in {ERASE, PROGRAM, BYTE_PROGRAM, STORE} for _pc, target in _calls_in(program, 0x1458E, 0x145BD)):
        raise ValueError("0x57 now calls a flash helper")
    if any(target in {ERASE, PROGRAM, BYTE_PROGRAM, STORE} for _pc, target in _calls_in(program, 0x14650, 0x1465B)):
        raise ValueError("0x6D now calls a flash helper")

    if handler(program, 0x44) != 0x147F2 or handler(program, 0x45) != 0x1480A:
        raise ValueError("0x44 or 0x45 moved")
    require_branch(program, 0x147FA, 0x1483E, "bcc")
    if not is_short_bsr(program[0x147FC]) or short_target(0x147FC, program[0x147FC]) != 0x1488C:
        raise ValueError("0x44 no longer replies through P:0x1488C")
    require_sites(program, 0x1488C, ("jsr", "bsr"), [0x147FC], "the slot read gained or lost a caller")
    if immediate_pair(program, 0x14890, "the slot read lost its reply opcode") != 0x45:
        raise ValueError("0x44 no longer replies 0x45")
    if program[0x14899] != 0x0140C5 or program[0x1489A] != 0x7F:
        raise ValueError("the RAM-index test is no longer 0x7F")
    if decode_do_immediate(program[0x148A0]) != PRESET_WORDS:
        raise ValueError("the RAM read is no longer 1,092 words")
    require_word(program, 0x1489C, 0x60F400, "the RAM read lost its buffer move")
    require_word(program, 0x1489D, 0xA47, "the RAM read no longer starts at Y:0xA47")
    if program[0x148B9] != JSR or program[0x148BA] != 0x20A5:
        raise ValueError("the flash read no longer uses the sector address")
    if decode_do_immediate(program[0x148BD]) != FLASH_BYTES:
        raise ValueError("the flash read is no longer 3,276 bytes")
    if program[0x148BC] != 0x011D64 or program[0x148D2] != 0x011D44:
        raise ValueError("the flash read no longer brackets bit 4")
    read_calls = {target for _pc, target in _calls_in(program, 0x147F2, 0x147FE)}
    read_calls.update(target for _pc, target in _calls_in(program, 0x1488C, 0x148DA))
    if read_calls & {ERASE, PROGRAM, BYTE_PROGRAM, STORE, 0x1016, 0x1059}:
        raise ValueError("0x44 now programs or erases")
    if decode_do_immediate(program[0x1480F]) != PRESET_WORDS:
        raise ValueError("0x45 no longer receives 1,092 words")
    if program[0x14819] != JSR or program[0x1481A] != 0x2095:
        raise ValueError("0x45 no longer checksums before storing")
    require_branch(program, 0x1481C, 0x14823, "bcc")
    if program[0x1481E] != JSR or program[0x1481F] != STORE:
        raise ValueError("a matching 0x45 no longer stores")
    if immediate_pair(program, 0x14820, "0x45 lost its success reply") != 0x71:
        raise ValueError("a matching 0x45 no longer replies 0x71")
    if immediate_pair(program, 0x14823, "0x45 lost its mismatch reply") != 0x72:
        raise ValueError("a mismatching 0x45 no longer replies 0x72")
    if program[0x14825] != JSR or program[0x14827] != JSR:
        raise ValueError("0x45 no longer reads the trailer after the store decision")

    if handler(program, 0x58) != 0x145BD:
        raise ValueError("0x58 moved")
    if program[0x145D3] != 0x0140CD or program[0x145D4] != 0x7F:
        raise ValueError("0x58 no longer tests for 0x7F")
    if immediate_pair(program, 0x145D6, "0x58 lost its start") != 5:
        raise ValueError("0x58 no longer starts at sector 5")
    if decode_do_immediate(program[0x145D8]) != 0x32:
        raise ValueError("0x58 no longer walks 50 sectors")
    if program[0x145E7] != 0x01728D or program[0x145E9] != 0x014588:
        raise ValueError("0x58 no longer adds 5 below 50")
    require_sites(program, STORE, direct, [0x1186, 0x141A4, 0x145E2, 0x145F3, 0x14747, 0x1481E], "store gained or lost a caller")
    if references(program, 0x14197):
        raise ValueError("the unreferenced bulk store gained a caller")
    if references(program, 0x14BC9):
        raise ValueError("the extra string-level commit gained a caller")

    if handler(program, 0x62) != 0x1474A or handler(program, 0x67) != 0x14798:
        raise ValueError("0x62 or 0x67 moved")
    if decode_do_immediate(program[0x14752]) != 0x3C:
        raise ValueError("0x62 no longer walks 60 sectors")
    require_word(program, 0x14755, 0x51F400, "0x62 lost its base move")
    require_word(program, 0x14756, SLOT_BASE, "0x62 lost its base")
    require_word(program, 0x14757, MAC_Y0_12, "0x62 lost its 4 KB step")
    if ERASE in {target for _pc, target in _calls_in(program, 0x1474A, 0x14798)}:
        raise ValueError("0x62 now erases")
    if walk_end(program, 0x1479E, 0x147A1) >= LOW_WINDOW:
        raise ValueError("0x67 no longer stays below the preset window")
    if ERASE in {target for _pc, target in _calls_in(program, 0x14798, 0x147CE)}:
        raise ValueError("0x67 now erases")
    if handler(program, 0x59) != 0x147D8:
        raise ValueError("0x59 moved")
    require_branch(program, 0x147DD, 0x147ED, "bcc")
    if program[0x147E4] != JSR or program[0x147E5] != 0x141AA:
        raise ValueError("0x59 no longer uses its copy helper")
    if immediate_pair(program, 0x141AD, "0x59 lost its first sector") != 5:
        raise ValueError("0x59 no longer starts at sector 5")
    if decode_do_immediate(program[0x141AF]) != 0x32:
        raise ValueError("0x59 no longer covers 50 sectors")
    repeats = decode_do_immediate(program[0x141AF])
    if repeats is None or walk_end(program, 0x141AB, 0x141B5, repeats) >= LOW_WINDOW:
        raise ValueError("0x59 no longer programs below the preset window")
    if ERASE in {target for _pc, target in _calls_in(program, 0x141AA, 0x141CC)}:
        raise ValueError("0x59 now erases")

    if handler(program, 0x60) != 0x14687 or program[0x14687] != JSR or program[0x14688] != 0x127E:
        raise ValueError("0x60 no longer calls its commit helper")
    require_branch(program, 0x12A2, 0x12AF, "bcc")
    if program[0x12AA] != JSR or program[0x12C6] != JSR or program[0x12C7] != RESET:
        raise ValueError("0x60 no longer commits and then resets")
    if immediate_pair(program, 0x12AF, "0x60 lost its reject reply") != 0x73:
        raise ValueError("a rejected 0x60 no longer replies 0x73")
    if handler(program, 0x6F) != 0x1468B:
        raise ValueError("0x6F moved")
    branch_calls = [target for _pc, target in _calls_in(program, 0x1468B, 0x1469B)]
    if branch_calls != [0x1257, 0x1326, 0x12C9]:
        raise ValueError("0x6F no longer has one read branch and one commit branch")
    if program[0x14695] != 0x0AF0A2 or program[0x14696] != 0x1483E:
        raise ValueError("0x6F no longer returns on its remaining values")
    if program[0x12FC] != JSR or program[0x1318] != JSR or program[0x1319] != RESET:
        raise ValueError("the 0x6F commit no longer erases and then resets")
    if handler(program, 0x7D) != 0x1469B or program[0x1469B] != JSR or program[0x1469C] != RESET:
        raise ValueError("0x7D no longer resets before its trailer")

    stamp_sites = [pc for pc, word in program.items() if word in {0xD46, 0xD47, 0xD48, 0xD49}]
    if stamp_sites != [0x1473B]:
        raise ValueError("the stored version words gained or lost an address literal")
    m0_sites = sorted(pc for pc, word in program.items() if word == MOVE_M7_M0)
    if m0_sites != [0x0E00, 0x1EF0, 0x14148]:
        rendered = ", ".join(pc_text(pc) for pc in m0_sites)
        raise ValueError(f"the known m0 writes moved: {rendered}")
    if program[0x1FFB] != 0x64F400 or program[0x1FFC] != 0x100000 or program[0x1FFD] != 0x6C7000:
        raise ValueError("a failed checksum no longer stores bit 20")
    require_word(program, 0x1FFE, 0x0A48, "a failed checksum no longer stores Y:0xA48")
    if program[0x2090] != 0x6DF000 or program[0x2092] != 0x6D7000 or program[0x2093] != 0xD0:
        raise ValueError("the reload no longer copies Y:0xA48 to Y:0xD0")
    if program[0x11B2] != 0x017780:
        raise ValueError("the selection helper no longer adds 55")
    require_sites(program, 0x140E8, direct, [0x1A1B], "startup flash setup gained or lost a caller")
    if 0x1FA7 in {target for _pc, target in _calls_in(program, 0x1A13, 0x1A34)}:
        raise ValueError("startup now loads a preset slot")

    def span(start, count):
        return {"first": start, "last": start + count - 1}

    bus = bus_map(program)
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "product_id": header["product_id"],
        "version_hex": header["version_hex"],
        "bus": bus,
        "erase": {
            "helper": pc_text(ERASE),
            "base": "0xD00000",
            "stride_bytes": SECTOR,
            "one_call_one_sector": True,
            "restart_tick_compare": 488,
        },
        "byte_program_restart_compare": 10000,
        "wide_erase": {
            "0x63": {
                "handler": pc_text(handler(program, 0x63)),
                "sectors": span(user_start, user_count),
                "includes_60_through_62": False,
                "before_trailer": True,
                "pdre_bit4": "set",
                "same_a0_a17_as_index_minus_64": True,
            },
            "0x68": {
                "handler": pc_text(handler(program, 0x68)),
                "sectors": span(spare_start, spare_count),
                "includes_60_through_62": True,
                "before_trailer": True,
                "pdre_bit4": "clear",
                "physical_overlap_with_store": OVERLAP,
            },
        },
        "code_load": {
            "0x60": {
                "handler": pc_text(0x14687),
                "sectors": span(code_start, code_count),
                "includes_60_through_62": False,
                "pdre_bit4": "clear",
                "physical_overlap_with_presets": OVERLAP,
                "commits_only_after_trailer_and_sum": True,
                "reject_reply": "0x73",
                "then_resets": True,
            },
            "0x6F": {
                "handler": pc_text(0x1468B),
                "read_branch_erases": False,
                "commit_sectors": span(0, 4),
                "includes_60_through_62": False,
                "pdre_bit4": "clear",
                "physical_overlap_with_presets": OVERLAP,
                "other_values_return": True,
                "commit_then_resets": True,
            },
        },
        "slot_read": {
            "opcode": "0x44",
            "handler": pc_text(0x147F2),
            "reply_opcode": "0x45",
            "requires_trailer_before_read": True,
            "ram_index": "0x7F",
            "ram_words": PRESET_WORDS,
            "ram_nibble_bytes": PRESET_WORDS * 6,
            "flash_bytes": FLASH_BYTES,
            "flash_nibble_bytes": FLASH_BYTES * 2,
            "sector_bytes_not_sent": SECTOR - FLASH_BYTES,
            "programs_or_erases": False,
            "pdre_bit4_during_flash_walk": "set",
            "index_7f_sets_bit4": False,
            "index_50_address": "0xD32000",
            "index_62_address": "0xD3E000",
            "same_external_pins_as_store": True,
            "host_image_byte_compare": False,
            "peripheral_register_writes_in_walk": ["Y:0xFFFF9D bit 4"],
        },
        "conditional_store": {
            "opcode": "0x45",
            "handler": pc_text(0x1480A),
            "words": PRESET_WORDS,
            "matching_checksum_stores_before_trailer": True,
            "mismatch_reply": "0x72",
            "match_reply": "0x71",
        },
        "bulk_store": {
            "opcode": "0x58",
            "handler": pc_text(0x145BD),
            "all_argument": "0x7F",
            "all_sectors": span(5, 0x32),
            "below_50_sector": "argument + 5",
            "at_or_above_50_stores": False,
        },
        "received_program": {
            "0x62": {
                "handler": pc_text(0x1474A),
                "sectors": span(0, 0x3C),
                "erases": False,
                "consumes_input_as_data": True,
                "pdre_bit4": "set",
                "same_external_pins_as_store": True,
            },
            "0x67": {
                "handler": pc_text(0x14798),
                "erases": False,
                "addresses_stay_below": f"0x{LOW_WINDOW:X}",
            },
            "0x59": {
                "handler": pc_text(0x147D8),
                "requires_trailer_first": True,
                "source_sectors": span(5, 0x32),
                "erases": False,
                "program_addresses_stay_below": f"0x{LOW_WINDOW:X}",
            },
        },
        "string_levels": {
            "receive_opcode": "0x54",
            "commit_opcode": "0x56",
            "reply_opcode": "0x57",
            "ram": "Y:0x2D8",
            "words": 6,
            "sector": level_sector,
            "destination": "0xD04000",
            "memory_space": "X",
            "pdre_bit4": "clear",
            "address_offset_added": False,
            "physical_overlap_with_preset_index": OVERLAP,
            "commit_after_trailer": True,
            "uses_spare": False,
        },
        "bank_names": {
            "write_opcode": "0x6C",
            "read_opcode": "0x6D",
            "sector": name_sector,
            "destination": "0xD08000",
            "memory_space": "X",
            "pdre_bit4": "clear",
            "address_offset_added": False,
            "physical_overlap_with_preset_index": OVERLAP,
            "words": name_count,
            "marker": "0xD081FE",
            "commit_before_trailer": True,
            "uses_spare": False,
            "ram_offset_scale_measured": False,
        },
        "reset_opcode": {
            "opcode": "0x7D",
            "handler": pc_text(0x1469B),
            "resets_before_trailer": True,
            "erases_60_through_62_in_this_image": False,
        },
        "version_stamp_literals": [pc_text(0x1473B)],
        "checksum_failure": {
            "flag": "Y:0xA48",
            "value": "0x100000",
            "copies_slot": False,
            "reload_copies_flag_to": "Y:0xD0",
        },
        "startup_calls_loader": False,
        "startup_erases_60_through_62": False,
    }


def _calls_in(program, start, end):
    found = []
    pc = start
    while pc < end:
        word = program.get(pc)
        if word is None:
            break
        if word == JSR:
            found.append((pc, program[pc + 1]))
            pc += 2
            continue
        if is_short_bsr(word):
            found.append((pc, short_target(pc, word)))
        pc += 1
    return found


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
            raise ValueError(f"flash trace differs from {args.check}")
        else:
            print("PASS: flash handlers match the checked-in trace")
    except (KeyError, OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
