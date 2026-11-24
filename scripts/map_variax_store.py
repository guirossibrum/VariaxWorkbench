"""Check the Variax 300/600 3.10 preset-store sequence against the archive.

The checker reads the private image and compares derived addresses, call
targets and factory-slot counts with research/variax-310-store-handlers.json.
It does not print disassembly or preset bytes.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

from inspect_firmware import inspect
from map_firmware import loader_records

ARCHIVE_SHA256 = "e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a"
SLOT_BASE = 0xD00000
SPARE_BASE = 0xD3C000
SECTOR_WORDS = 0x1000
PROGRAM_WORDS = 0x444
STAMP_SOURCE = 0x151BB
STAMP_WORDS = (0x30, 0x33, 0x31, 0x30)
BANKS = (
    "Custom 1", "T-Model", "Spank", "Lester", "Special", "R-Billy",
    "Chime", "Semi", "Jazzbox", "Acoustic", "Reso", "Custom 2",
)

# Assembler encodings for the two arithmetic instructions the address math uses.
# Checked against a typed synthetic program, then compared with the archive.
ADD_60 = 0x017C80
MAC_X1_12 = 0x010CFA
MAC_Y0_12 = 0x010CEA
SUB_55 = 0x017784


def slot_address(index):
    return (SLOT_BASE + index * SECTOR_WORDS) & 0xFFFFFF


def group_of(index):
    return index // 20


def spare_sector(index):
    return 60 + group_of(index)


def spare_address(index):
    return slot_address(spare_sector(index))


def editor_store_index(bank, slot):
    """Workbench store index: bank * 5 + (4 - slot). Not program_number()."""
    return bank * 5 + (4 - slot)


def editor_store_indices():
    return [editor_store_index(bank, slot) for bank in range(12) for slot in range(5)]


def checksum_delta_for_zero_stamp():
    return sum(STAMP_WORDS) & 0xFFFFFF


def short_displacement(opcode):
    field = ((opcode >> 1) & 0x1E0) | (opcode & 0x1F)
    if field & 0x100:
        field -= 0x200
    return field


def short_target(pc, opcode):
    return (pc + short_displacement(opcode)) & 0xFFFFFF


def is_short_bsr(opcode):
    return (opcode & 0xFFFC20) == 0x050800


def is_short_bcc(opcode):
    return (opcode & 0xFF0C20) == 0x050400


def is_short_bra(opcode):
    return (opcode & 0xFFFC20) == 0x050C00


def program_image(archive):
    if hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("expected the exact archived Variax 300/600 3.10 image")
    header, image = inspect(archive)
    region = header["regions"][1]
    main = image[region["offset"]:region["offset"] + region["bytes"]]
    program = {}
    records = []
    parsed = loader_records(main)
    for record in parsed["records"]:
        records.append((record, main))
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
    return header, image, program, records


def absolute_calls(program, start, end):
    calls = []
    pc = start
    while pc < end:
        word = program.get(pc)
        if word is None:
            break
        if word == 0x0BF080:
            calls.append((pc, "jsr", program[pc + 1]))
            pc += 2
            continue
        if is_short_bsr(word):
            calls.append((pc, "bsr", short_target(pc, word)))
            pc += 1
            continue
        pc += 1
    return calls


def require_branch(program, pc, target, kind):
    word = program[pc]
    if kind == "bra":
        if not is_short_bra(word):
            raise ValueError(f"P:{pc:#x} is not a short bra")
    elif not is_short_bcc(word):
        raise ValueError(f"P:{pc:#x} is not a short conditional branch")
    found = short_target(pc, word)
    if found != target:
        raise ValueError(f"P:{pc:#x} branches to {found:#x}, expected {target:#x}")


def tag_words(records, address, count):
    for record, main in records:
        if record["memory_tag_raw"] != 2:
            continue
        dest = record["destination_word_raw"]
        if not dest <= address < dest + record["word_count"]:
            continue
        if address + count > dest + record["word_count"]:
            raise ValueError("stamp extends outside its tag-2 record")
        offset = record["header_byte_offset"] + 9 + 3 * (address - dest)
        return tuple(
            int.from_bytes(main[offset + 3 * index:offset + 3 * index + 3], "little")
            for index in range(count)
        )
    raise ValueError("tag-2 record does not cover the stamp")


def factory_slot_report(header, image):
    region = header["regions"][3]
    blob = image[region["offset"]:region["offset"] + region["bytes"]]
    if len(blob) != 60 * 4096:
        raise ValueError("region 3 is not 60 sectors of 4096 bytes")
    stamp_nonzero = reserved_nonzero = tail_nonzero = checksum_mismatch = 0
    for slot in range(60):
        sector = blob[slot * 4096:(slot + 1) * 4096]
        raw = sector[:PROGRAM_WORDS * 3]
        words = [int.from_bytes(raw[index:index + 3], "little") for index in range(0, len(raw), 3)]
        if words[767:771] != [0, 0, 0, 0]:
            stamp_nonzero += 1
        if words[1089] or words[1090]:
            reserved_nonzero += 1
        host = sum(words[:1089]) & 0xFFFFFF
        firmware = sum(words[:1091]) & 0xFFFFFF
        if host != words[1091] or firmware != words[1091]:
            checksum_mismatch += 1
        if any(byte != 0xFF for byte in sector[PROGRAM_WORDS * 3:]):
            tail_nonzero += 1
    return {
        "slot_count": 60,
        "sector_bytes": 4096,
        "programmed_bytes": PROGRAM_WORDS * 3,
        "stamp_nonzero_slots": stamp_nonzero,
        "words_1089_1090_nonzero_slots": reserved_nonzero,
        "bytes_3276_4095_nonzero_slots": tail_nonzero,
        "checksum_mismatch_slots": checksum_mismatch,
    }


def inventory(archive):
    header, image, program, records = program_image(archive)
    store_calls = [(kind, target) for _, kind, target in absolute_calls(program, 0x0FA6, 0x0FD2)]
    handler_calls = [(kind, target) for _, kind, target in absolute_calls(program, 0x14731, 0x1474A)]
    expected_store = [
        ("jsr", 0x1A), ("bsr", 0x0F17), ("bsr", 0x0F45),
        ("bsr", 0x0F63), ("bsr", 0x0F17), ("bsr", 0x0F63),
    ]
    expected_handler = [("jsr", 0x1257), ("jsr", 0x1257), ("jsr", 0x209D), ("jsr", 0x0FA6)]
    if store_calls != expected_store:
        raise ValueError(f"store call order changed: {store_calls}")
    if handler_calls != expected_handler:
        raise ValueError(f"handler call order changed: {handler_calls}")

    pins = {
        0x0FAE: 0x1A,
        0x0FB0: SPARE_BASE,
        0x0FB3: MAC_X1_12,
        0x0FB6: PROGRAM_WORDS,
        0x0FBC: SLOT_BASE,
        0x0FBD: MAC_Y0_12,
        0x0FC7: PROGRAM_WORDS,
        0x0F59: 0x2710,  # byte-program polls, then restarts
        0x0F82: 0x1E8,  # sector-erase tick, then restarts
        0x0FCC: ADD_60,
        0x14738: 0xF7,
        0x1473B: 0xD46,
        0x1473D: STAMP_SOURCE,
        0x14745: 0xE8A,
        0x14806: 0x71,
        0x1482E: 0x66,
        0x1FB2: 0xD20000,
        0x1FD0: 0x017B85,  # cmp #$3b,a — index 59 is the last unadjusted slot
        0x1FD2: SUB_55,
    }
    for address, word in pins.items():
        if program[address] != word:
            raise ValueError(f"P:{address:#x} is {program[address]:#x}, expected {word:#x}")

    require_branch(program, 0x14739, 0x1482D, "bcc")
    require_branch(program, 0x14749, 0x14805, "bra")
    require_branch(program, 0x0F3D, 0x0F32, "bcc")
    require_branch(program, 0x0F5A, 0x0F46, "bcc")
    require_branch(program, 0x0F61, 0x0F56, "bcc")
    require_branch(program, 0x0F83, 0x0F63, "bcc")
    for address in (0x0F44, 0x0F62, 0x0F8B, 0x0FD1):
        if program[address] != 0x0C:
            raise ValueError(f"expected rts at P:{address:#x}")

    counts = Counter(program.values())
    if counts[SPARE_BASE] != 1 or counts[0xD3D000] or counts[0xD3E000]:
        raise ValueError("spare base literal is not unique to the store")
    if counts[ADD_60] != 1:
        raise ValueError("adding 60 is not unique to the spare erase")
    spare_sites = [address for address, word in program.items() if word == SPARE_BASE]
    if spare_sites != [0x0FB0]:
        raise ValueError("spare literal moved")

    def pair_sites(target):
        return sorted(
            address for address, word in program.items()
            if word == 0x0BF080 and program.get(address + 1) == target
        )

    reload = range(0x1E9C, 0x1FA7)
    loader = range(0x1FA7, 0x2001)
    for span, name in ((reload, "reload"), (loader, "loader")):
        if any(program[address] in (SPARE_BASE, ADD_60) for address in span):
            raise ValueError(f"{name} path references the spare")
    for target in (0x0FA6, 0x20A5):
        if any(address in reload for address in pair_sites(target)):
            raise ValueError("reload path calls the store or the slot-address helper")
    loader_calls = absolute_calls(program, 0x1FA7, 0x2001)
    if any(target == 0x0FA6 for _, _, target in loader_calls):
        raise ValueError("loader calls the store")
    if not any(target == 0x20A5 for _, _, target in loader_calls):
        raise ValueError("loader no longer uses the slot-address helper")
    indices = editor_store_indices()
    if sorted(indices) != list(range(60)) or len(set(indices)) != 60:
        raise ValueError("editor store index formula does not cover 0-59 once")

    stamp = tag_words(records, STAMP_SOURCE, 4)
    if stamp != STAMP_WORDS:
        raise ValueError("identity stamp words changed")
    if checksum_delta_for_zero_stamp() != 0xC4:
        raise ValueError("stamp sum is not 0xC4")

    groups = []
    for number in range(3):
        first = number * 20
        groups.append({
            "indices": f"{first}-{first + 19}",
            "spare_sector": 60 + number,
            "spare_address": f"0x{slot_address(60 + number):X}",
            "banks": list(BANKS[number * 4:(number + 1) * 4]),
        })

    steps = [
        {"step": 1, "operation": "program_words", "words": PROGRAM_WORDS,
         "address": "spare", "helper": "P:0x0F17"},
        {"step": 2, "operation": "program_one_zero_byte", "words": 1,
         "address": "slot", "space": "Y", "helper": "P:0x0F45"},
        {"step": 3, "operation": "erase_sector", "address": "slot", "helper": "P:0x0F63"},
        {"step": 4, "operation": "program_words", "words": PROGRAM_WORDS,
         "address": "slot", "space": "X", "helper": "P:0x0F17"},
        {"step": 5, "operation": "erase_sector", "address": "spare", "helper": "P:0x0F63"},
    ]
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "product_id": header["product_id"],
        "version_hex": header["version_hex"],
        "handler": "P:0x14731",
        "store_routine": "P:0x0FA6",
        "slot_address": "0xD00000 + index * 0x1000",
        "spare_address": "0xD3C000 + (index // 20) * 0x1000",
        "ordered_steps": steps,
        "groups": groups,
        "reply_0x71": "P:0x14805",
        "reply_only_after_store_returns": True,
        "rejection_0x66": "P:0x1482D",
        "second_byte_required": "0xF7",
        "stamp_source": "Y:0x151BB",
        "stamp_destination_words": [767, 768, 769, 770],
        "stamp_words": [f"0x{word:02X}" for word in STAMP_WORDS],
        "checksum_words_summed": 1091,
        "checksum_destination_word": 1091,
        "checksum_delta_when_stamp_and_words_1089_1090_are_zero": "0xC4",
        "factory_slots": factory_slot_report(header, image),
        "spare_literal": "P:0x0FB0",
        "store_callers": [f"P:0x{address:X}" for address in pair_sites(0x0FA6)],
        "loader_does_not_read_spare": True,
        "reload_does_not_read_spare": True,
        "index_aliasing_its_spare": 63,
        "editor_store_index_set": "0-59 exactly once",
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
            raise ValueError(f"store trace differs from {args.check}")
        else:
            print("PASS: preset-store sequence matches the checked-in trace")
    except (KeyError, OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
