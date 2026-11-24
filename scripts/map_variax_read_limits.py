"""Check the Variax 300/600 3.10 preset-read limits against the archive.

Opcode 0x3C reads the current Y-memory preset buffer. The checker compares
derived addresses, the reply frame and the transmit-queue bound with
research/variax-310-read-limits.json. It does not print disassembly.
"""

import argparse
import json
from pathlib import Path

from map_variax_buffer import decode_do_immediate
from map_variax_store import (
    ARCHIVE_SHA256,
    is_short_bcc,
    is_short_bra,
    is_short_bsr,
    program_image,
    short_target,
    tag_words,
)

PRESET = 0xA47
PRESET_WORDS = 0x444
QUEUE_MASK = 0x3F


def reply_bytes(count):
    """F0, five header bytes, opcode, six nibbles per word, and F7."""
    return 8 + 6 * count


def last_word(start, count):
    """Last preset-word index read, for a nonzero count and a plain sum."""
    return start + count - 1


def stays_inside(start, count):
    if count <= 0:
        return False
    return start >= 0 and last_word(start, count) < PRESET_WORDS


def range_contains(program, start, end, needles):
    return [pc for pc in range(start, end) if program.get(pc) in needles]


def writes_m6(word):
    """True when a movec form writes m6. The W bit selects the direction."""
    if (word & 0xFF00FF) == 0x0500A6:
        return True
    if (word & 0xFFC0FF) == 0x04C0A6:
        return True
    if (word & 0xFFFFE0) == 0x0466A0:
        return True
    if (word & 0xFFC080) == 0x05C000 and (word & 0x3F) == 0x26:
        return True
    if (word & 0xFFC080) == 0x058000 and (word & 0x3F) == 0x26:
        return True
    return False


def inventory(archive):
    header, _image, program, records = program_image(archive)
    if (program[0x144AE], program[0x144AF]) != (0x66F400, PRESET):
        raise ValueError("0x3C no longer starts at Y:0xA47")
    if (program[0x144C2], program[0x144C3]) != (0x06C700, 0x144DA):
        raise ValueError("0x3C no longer loops on the received count")
    if decode_do_immediate(program[0x144C5]) != 6 or program[0x144C6] != 0x144D5:
        raise ValueError("0x3C no longer emits six nibbles from one word")
    if program[0x144B8] != 0xF7:
        raise ValueError("0x3C no longer checks the trailer for F7")
    if (program[0x144B9], program[0x144BA]) != (0x0AF0A2, 0x147ED):
        raise ValueError("a bad trailer no longer takes the 0x72 path")
    if (program[0x147ED], program[0x147EE]) != (0x56F400, 0x72):
        raise ValueError("the bad-trailer reply is no longer 0x72")
    if (program[0x144BD], program[0x144BE]) != (0x56F400, 0x3C):
        raise ValueError("the success reply no longer starts with 0x3C")
    if (program[0x1472B], program[0x1472C]) != (0x56F400, 0xF7):
        raise ValueError("the success reply no longer ends with F7")
    if (program[0x144D6], program[0x144D7]) != (0x0A77D6, 0xFFFFFF):
        raise ValueError("0x3C no longer reloads the saved pointer after six nibbles")
    if program[0x144D8] != 0x205E00:
        raise ValueError("0x3C no longer advances one word after the nibble loop")
    if (program[0x144D9], program[0x144DA]) != (0x0A7796, 0xFFFFFF):
        raise ValueError("0x3C no longer saves the advanced pointer inside the count loop")
    if (program[0x144DB], program[0x144DC]) != (0x0AF080, 0x1472B):
        raise ValueError("the reply closer is no longer the instruction after the count loop")
    if (program[0x136A], program[0x136B]) != (0x0BF080, 0x1241):
        raise ValueError("the reply no longer queues F0 through the transmit wait")
    if decode_do_immediate(program[0x136E]) != 5:
        raise ValueError("the reply header is no longer five bytes")
    header_words = tag_words(records, 0xA42, 5)
    if header_words != (0x00, 0x01, 0x0C, 0x07, 0x04):
        raise ValueError("the reply header bytes changed")
    if range_contains(program, 0x144A9, 0x144DC, {0x444, 0x445, PRESET_WORDS}):
        raise ValueError("0x3C gained a 1,092-word compare")
    if program[0x14154] != 0x0505A6 or program[0x144F2] != 0x0466A7:
        raise ValueError("the known m6 writes changed")
    if not writes_m6(program[0x14154]) or not writes_m6(program[0x144F2]):
        raise ValueError("the m6-write test no longer recognizes a known write")
    m6_writes = [pc for pc in range(0x144A9, 0x144DD) if writes_m6(program.get(pc, 0))]
    if m6_writes:
        raise ValueError("0x3C now writes m6")
    if range_contains(program, 0x144A9, 0x144DC, {0x0F17, 0x0F45, 0x0F63, 0x0FA6}):
        raise ValueError("0x3C now calls a flash helper")
    if program[0x1245] != 0x017F8E or program[0x19D3] != 0x017F8E:
        raise ValueError("the transmit queue no longer masks both indices with 0x3F")
    if (program[0x56], program[0x57], program[0x58], program[0x59]) != (
        0x0BF080, 0x20D2, 0x0BF080, 0x19B7,
    ):
        raise ValueError("the transmit drain is no longer the vector after receive")
    drain_words = sorted(pc for pc, word in program.items() if word == 0x19B7)
    if drain_words != [0x59]:
        raise ValueError("the transmit drain address is no longer only the vector extension")
    short_drain = [
        pc
        for pc, word in program.items()
        if (is_short_bsr(word) or is_short_bra(word) or is_short_bcc(word))
        and short_target(pc, word) == 0x19B7
    ]
    if short_drain:
        raise ValueError("a short branch now enters the transmit drain")

    past = None
    for record, _main in records:
        if record["memory_tag_raw"] != 2:
            continue
        dest = record["destination_word_raw"]
        end = dest + record["word_count"] - 1
        if dest <= 0xE8B <= end:
            past = end
    if past is None:
        raise ValueError("Y:0xE8B is no longer inside an initialized record")

    return {
        "archive_sha256": ARCHIVE_SHA256,
        "product_id": header["product_id"],
        "version_hex": header["version_hex"],
        "opcode": "0x3C",
        "handler": "P:0x144A9",
        "buffer": "Y:0xA47",
        "address_formula": "Y:0xA47 + start + i when m6 is linear",
        "handler_writes_m6": False,
        "buffer_words": PRESET_WORDS,
        "last_buffer_address": "Y:0xE8A",
        "first_address_past_buffer": "Y:0xE8B",
        "initialized_record_through": f"Y:0x{past:X}",
        "span_check": False,
        "flash_call": False,
        "reply_header": ["0x00", "0x01", "0x0C", "0x07", "0x04"],
        "reply_bytes": "8 + 6 * count",
        "reply_bytes_for_count_32": reply_bytes(32),
        "largest_count_from_start_0": PRESET_WORDS,
        "reply_bytes_for_full_buffer": reply_bytes(PRESET_WORDS),
        "queue_index_mask": "0x3F",
        "queue_bytes": QUEUE_MASK,
        "transmit_wait": "P:0x1241",
        "transmit_drain_vector": "P:0x0058",
        "host_reads": [
            {"start": 851, "count": 32, "last_word": last_word(851, 32)},
            {"start": 883, "count": 32, "last_word": last_word(883, 32)},
        ],
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
            raise ValueError(f"read-limit trace differs from {args.check}")
        else:
            print("PASS: preset-read limits match the checked-in trace")
    except (KeyError, OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
