"""Check the PODxt Live 3.01 store-path claims against the archive.

Static only. With --check it disassembles the region-4 program using the
private SHARC tool and compares the locked instruction lines. It does not
execute firmware and it does not open a device.
"""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

from inspect_firmware import inspect
from map_firmware import pod_load_records, pod_word_section
from map_podxt_commands import PROGRAM_END, PROGRAM_START, _disassembly, _program_bytes

ARCHIVE_SHA256 = "738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5"
SLOT_WORDS = 0x50
SLOT_COUNT = 0x180
SENTINEL_OFFSET = 0x7FFF
LOWER_BASE = 0x01070000
UPPER_BASE = 0x01078000
# Line 6 Edit TranslateAddr as implemented by Location::address.
HOST_BASES = {"tones": (0, 128), "amp": (64, 192), "fx": (128, 256)}
HOST_SIZES = {"tones": 128, "amp": 107, "fx": 64}

EVIDENCE = (
    (0x816B, "CALL (0x000093DF)"),
    (0x816C, "CALL (0x000081A8)"),
    (0x81AA, "R0 = R0 OR LSHIFT R1 BY 7"),
    (0x81AB, "R1 = 0x00000050"),
    (0x81AD, "R1 = DM(0x0000C2DF)"),
    (0x81AE, "R0 = R0 + R1,  R1 = DM(0x00, I7)"),
    (0x931A, "IF NE, JUMP (0x00009317)"),
    (0x9322, "DM(0x00000000, I0) = R0"),
    (0x9324, "CALL (0x0000931C)"),
    (0x9333, "DM(0x0000C2DA) = R0"),
    (0x9334, "DM(0x0000C2D6) = R1"),
    (0x9335, "DM(0x0000C2DB) = R2"),
    (0x9365, "CALL (0x000093ED)"),
    (0x9367, "CALL (0x00009428)"),
    (0x9368, "CALL (0x0000936A)"),
    (0x936A, "R0 = DM(0x0000C2DF)"),
    (0x936B, "DM(I7, 0x01) = R0"),
    (0x936C, "LCNTR = 0x0180, DO (0x00009375) UNTIL LCE"),
    (0x936D, "R0 = DM(0xFFFFFFFF, I7)"),
    (0x936E, "R1 = BTGL R0 BY 15"),
    (0x936F, "R2 = 0x00000050"),
    (0x9370, "R5 = R0 + R2,  I0 = R1"),
    (0x9371, "R3 = DM(0x00000000, I0)"),
    (0x9372, "R3 = LSHIFT R3 BY 16"),
    (0x9373, "R4 = 0xFFFF0000"),
    (0x9374, "R3 = R3 - R4,  DM(-0x01, I7) = R5"),
    (0x9375, "IF EQ, CALL (0x00009333)"),
    (0x9376, "R5 = DM(0x0000C2DF)"),
    (0x9377, "R0 = BTGL R5 BY 15"),
    (0x9378, "DM(0x0000C2DF) = R0"),
    (0x9379, "R1 = 0x00007FFF"),
    (0x937A, "R0 = R0 + R1,  R1 = DM(I7, -0x01)"),
    (0x937B, "DM(0x0000C2D6) = R0"),
    (0x937C, "R0 = 0x00001969"),
    (0x937D, "CALL (0x0000930F)"),
    (0x937E, "DM(0x0000C2D6) = R5"),
    (0x937F, "JUMP (0x00009324)"),
    (0x9380, "R1 = DM(0x01078000)"),
    (0x9381, "R0 = 0xFFFF0000"),
    (0x9382, "R1 = LSHIFT R1 BY 16"),
    (0x9383, "R1 = R1 - R0"),
    (0x9384, "IF EQ, RTS"),
    (0x9385, "R2 = 0x01078000"),
    (0x9386, "R1 = DM(0x01070000)"),
    (0x9387, "DM(0x0000C2DF) = R2"),
    (0x9388, "R1 = LSHIFT R1 BY 16"),
    (0x9389, "R1 = R1 - R0"),
    (0x938A, "IF EQ, RTS"),
    (0x938B, "R0 = 0x19690000"),
    (0x938C, "R1 = DM(0x01077FFF)"),
    (0x938D, "R1 = LSHIFT R1 BY 16"),
    (0x938E, "R1 = R1 - R0"),
    (0x938F, "IF EQ, R2 = BTGL R2 BY 15"),
    (0x9390, "DM(0x0000C2DF) = R2"),
    (0x9391, "R2 = BTGL R2 BY 15"),
    (0x9392, "DM(0x0000C2D6) = R2"),
    (0x9393, "JUMP (0x00009324)"),
    (0x941D, "CALL (0x000093ED)"),
    (0x941E, "CALL (0x0000936A)"),
    (0x9420, "CALL (0x000093DF)"),
    (0x9421, "B5 = B4"),
    (0x9422, "I5 = I4"),
    (0x9423, "L5 = L4"),
    (0x9424, "MODIFY (I5, 0x00000002)"),
    (0x9425, "CALL (0x00009428)"),
    (0x9426, "L5 = 0x00000000"),
    (0x9427, "RTS"),
    (0x9428, "CALL (0x000081A8)"),
    (0x9429, "R0 = BTGL R0 BY 15"),
    (0x942A, "DM(0x0000C2D6) = R0"),
    (0x942B, "LCNTR = 0x0050, DO (0x0000942F) UNTIL LCE"),
    (0x942C, "R1 = DM(I5, 0x01)"),
    (0x942D, "R0 = DM(I5, 0x01)"),
    (0x942E, "R0 = R0 OR LSHIFT R1 BY 8"),
    (0x942F, "CALL (0x0000930F)"),
    (0x9431, "CALL (0x000093ED)"),
    (0x943C, "CALL (0x0000936A)"),
)


def host_address(kind, index):
    low, high = HOST_BASES[kind]
    if not 0 <= index < HOST_SIZES[kind]:
        raise ValueError(f"host index out of range for {kind}")
    return index + (low if index < 64 else high)


def host_addresses():
    return [host_address(kind, index) for kind, size in HOST_SIZES.items() for index in range(size)]


def _width2_word(payload, address):
    cursor = 0
    for record in pod_load_records(payload):
        start = cursor + 10
        if record["destination_word_raw"] == 0xC000 and record["width_tag_raw"] == 2:
            index = address - 0xC000
            if not 0 <= index < record["word_count"]:
                raise ValueError(f"{address:#x} is outside the 32-bit load record")
            raw = payload[start + index * 4:start + index * 4 + 4]
            return int.from_bytes(raw, "big")
        cursor = start + record["payload_bytes"]
    raise ValueError("missing 32-bit load record")


def _blank_slots(chunk):
    return sum(1 for index in range(SLOT_COUNT) if chunk[index * SLOT_WORDS * 2:index * SLOT_WORDS * 2 + 2] == b"\xff\xff")


def _straight_line_flash(listing, word):
    flash = ("0x0000930F", "0x0000931C", "0x00009324", "0x00009333", "0x0000936A")
    pc = word
    for _ in range(40):
        text = listing.get(pc)
        if text is None:
            return False
        if any(token in text for token in flash):
            return True
        if text.startswith(("RTS", "RTI", "JUMP")):
            return False
        pc += 1
    return False


def inventory(archive):
    if hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("expected the exact archived PODxt Live 3.01 container")
    header, image = inspect(archive)
    region = image[header["regions"][4]["offset"]:header["regions"][4]["offset"] + header["regions"][4]["bytes"]]
    _section, payload = pod_word_section(region)
    listing = _disassembly(_program_bytes(payload))
    for addr, text in EVIDENCE:
        actual = listing.get(addr)
        if actual != text:
            raise ValueError(f"disassembly at {addr:#x} is {actual!r}, expected {text!r}")
    store_body = " ".join(listing[addr] for addr in range(0x9420, 0x9431))
    if "0x00009324" in store_body or "0x0000931C" in store_body or "0x0000936A" in store_body:
        raise ValueError("opcode 0x71 body reaches an erase or commit routine")
    if "CALL (0x0000930F)" not in listing[0x942F]:
        raise ValueError("opcode 0x71 does not program through 0x930F")
    read_body = " ".join(listing[addr] for addr in range(0x816B, 0x8187))
    if "BTGL" in read_body:
        raise ValueError("opcode 0x76 toggles bit 15")
    writers = sorted(addr for addr, text in listing.items() if text.startswith("DM(0x0000C2DF) ="))
    if writers != [0x9378, 0x9387, 0x9390]:
        raise ValueError(f"unexpected C2DF writers: {writers}")
    program_callers = sorted(addr for addr, text in listing.items() if "CALL (0x00009428)" in text)
    commit_callers = sorted(
        addr for addr, text in listing.items()
        if "CALL (0x0000936A)" in text or "JUMP (0x0000936A)" in text
    )
    if program_callers != [0x9367, 0x9425] or commit_callers != [0x9368, 0x941E, 0x943C]:
        raise ValueError(f"unexpected store callers: {program_callers} {commit_callers}")
    boot = " ".join(listing[addr] for addr in range(0x9380, 0x9394))
    if "0x0107FFFF" in boot:
        raise ValueError("boot selector reads the upper sentinel")
    named = sorted(addr for addr, text in listing.items() if "0x0001B9A8" in text or "0x0001B9A9" in text)
    if named != [0x812A, 0x8133, 0x8C73]:
        raise ValueError(f"unexpected global-address instructions: {named}")
    if _straight_line_flash(listing, 0x8C73):
        raise ValueError("0x8C73 reaches a flash routine on its straight-line path")
    cursor = 0
    table = None
    for record in pod_load_records(payload):
        start = cursor + 10
        if record["destination_word_raw"] == 0x1A900 and record["width_tag_raw"] == 1:
            raw = payload[start:start + record["payload_bytes"]]
            words = [int.from_bytes(raw[i:i + 2], "big") for i in range(0, len(raw), 2)]
            table = words[0x1B978 - 0x1A900:0x1B978 - 0x1A900 + 256]
        cursor = start + record["payload_bytes"]
    if table is None:
        raise ValueError("missing 16-bit load record")
    flash_targets = [
        word for word in table
        if PROGRAM_START <= word < PROGRAM_END and _straight_line_flash(listing, word)
    ]
    if flash_targets:
        raise ValueError(f"global jump targets reach flash on a straight line: {flash_targets}")
    loaded_base = _width2_word(payload, 0xC2DF)
    if loaded_base != LOWER_BASE or _width2_word(payload, 0xC2D6) != 0:
        raise ValueError("loaded pointer words differ")
    lower = header["regions"][17]
    upper = header["regions"][18]
    if (lower["offset"], lower["bytes"], upper["offset"], upper["bytes"]) != (0xE0000, 0x10000, 0xF0000, 0x10000):
        raise ValueError("region 17/18 placement differs")
    lower_bytes = image[lower["offset"]:lower["offset"] + lower["bytes"]]
    upper_bytes = image[upper["offset"]:upper["offset"] + upper["bytes"]]
    if upper_bytes != b"\xff" * len(upper_bytes):
        raise ValueError("region 18 is not entirely FF")
    if (lower["offset"], upper["offset"]) != ((LOWER_BASE - 0x01000000) * 2, (UPPER_BASE - 0x01000000) * 2):
        raise ValueError("half file offsets do not match the 16-bit mapping")
    addresses = host_addresses()
    if max(addresses) != 298 or any(index * SLOT_WORDS + SLOT_WORDS > SLOT_COUNT * SLOT_WORDS for index in addresses):
        raise ValueError("a host address does not fit in the scanned slots")
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "evidence_count": len(EVIDENCE),
        "loaded_c2df": f"0x{loaded_base:08X}",
        "loaded_c2d6": "0x00000000",
        "c2df_writers": [f"0x{addr:04X}" for addr in writers],
        "slot_words": SLOT_WORDS,
        "slot_count": SLOT_COUNT,
        "sentinel_offset": f"0x{SENTINEL_OFFSET:04X}",
        "lower_file_offset": "0xE0000",
        "upper_file_offset": "0xF0000",
        "lower_first_bytes": lower_bytes[:2].hex(),
        "lower_last_bytes": lower_bytes[-2:].hex(),
        "upper_all_ff": True,
        "lower_blank_slots": _blank_slots(lower_bytes),
        "upper_blank_slots": _blank_slots(upper_bytes),
        "host_address_max": max(addresses),
        "sentinel_index": 409,
        "global_address_instructions": [f"0x{addr:04X}" for addr in named],
        "global_straight_line_flash_targets": 0,
        "opcode_0x71_erases": False,
        "target_confidence": "confirmed",
        "file_mapping": "likely",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", type=Path, help="compare with the checked-in store record")
    args = parser.parse_args()
    try:
        if not args.archive.is_file():
            raise ValueError("archive must be a regular file")
        result = inventory(args.archive.read_bytes())
        if args.check is None:
            print(json.dumps(result, indent=2))
        elif json.loads(args.check.read_text()) != result:
            raise ValueError(f"store record differs from {args.check}")
        else:
            print(f"PASS: {result['evidence_count']} store-path lines match")
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
