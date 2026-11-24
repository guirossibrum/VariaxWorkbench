"""Check the PODxt Live 3.01 command-handler claims against the archive.

Static only. With --check it disassembles the region-4 program using the
private SHARC tool and compares the locked instruction lines. It does not
execute firmware and it does not open a device.
"""

import argparse
import hashlib
import json
import re
import subprocess
import tempfile
from pathlib import Path

from inspect_firmware import inspect
from map_firmware import pod_load_records, pod_word_section

ARCHIVE_SHA256 = "738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5"
PROGRAM_START = 0x8000
PROGRAM_END = 0x96B8
DASM = Path(__file__).resolve().parents[1] / ".tools" / "sharc-research" / "sharc-dasm"

# Address and the exact pinned-disassembler text. --check re-disassembles these.
EVIDENCE = (
    (0x80D9, "IF NOT FLAG1_IN, JUMP (0x000080D9)"),
    (0x80DC, "CALL (0x000080D6)"),
    (0x80DD, "R0 = 0x00000050"),
    (0x80E0, "CALL (0x000080D6)"),
    (0x80E1, "R0 = 0x00000051"),
    (0x80E6, "R0 = R0 XOR R0"),
    (0x80E9, "R0 = LSHIFT R0 BY 4,  R1 = DM(I4, M0)"),
    (0x80FB, "R0 = DM(I4, 0x01)"),
    (0x80FC, "LCNTR = R0, DO (0x00008102)"),
    (0x8102, "CALL (0x0000930F)"),
    (0x80F9, "CALL (0x000080F3)"),
    (0x80FA, "JUMP (0x0000930B)"),
    (0x8111, "JUMP (M15, I15)"),
    (0x8124, "R2 = R2 XOR R2,  R0 = DM(I4, 0x01)"),
    (0x812C, "JUMP (0x000095A0)"),
    (0x812E, "R0 = 0x00000056"),
    (0x8161, "CALL (0x000093F2)"),
    (0x8165, "JUMP (0x000080E0)"),
    (0x8166, "CALL (0x0000816B)"),
    (0x8168, "R0 = 0x00000072"),
    (0x8170, "IF SZ, JUMP (0x0000817D)"),
    (0x8171, "LCNTR = R0, DO (0x0000817B)"),
    (0x8173, "R0 = 0x00000071"),
    (0x817E, "R0 = 0x00000074"),
    (0x818B, "LCNTR = 0x00A0, DO (0x0000818D) UNTIL LCE"),
    (0x8190, "LCNTR = 0x00A0, DO (0x00008192) UNTIL LCE"),
    (0x8194, "JUMP (0x000094BA)"),
    (0x81A6, "CALL (0x00009324)"),
    (0x81A7, "JUMP (0x000080DC)"),
    (0x81AA, "R0 = R0 OR LSHIFT R1 BY 7"),
    (0x81AB, "R1 = 0x00000050"),
    (0x930B, "R1 = 0x01000000"),
    (0x930F, "I0 = DM(0x0000C2D6)"),
    (0x9311, "R1 = 0x000000A0"),
    (0x9312, "DM(0x01000555) = R1"),
    (0x9315, "DM(I0, 0x01) = R0"),
    (0x931A, "IF NE, JUMP (0x00009317)"),
    (0x931D, "R0 = 0x00000080"),
    (0x931E, "DM(0x01000555) = R0"),
    (0x9321, "R0 = 0x00000030"),
    (0x9322, "DM(0x00000000, I0) = R0"),
    (0x9324, "CALL (0x0000931C)"),
    (0x9326, "IF NE, JUMP (0x00009325)"),
    (0x932E, "R1 = 0x000000AA"),
    (0x932F, "DM(0x01000555) = R1"),
    (0x9330, "R1 = 0x00000055"),
    (0x9331, "DM(0x010002AA) = R1"),
    (0x9337, "R1 = 0x00000020"),
    (0x9339, "R0 = 0x000000A0"),
    (0x933A, "DM(0x01000000) = R0"),
    (0x9366, "I5 = 0x0001B9D8"),
    (0x9367, "CALL (0x00009428)"),
    (0x9368, "CALL (0x0000936A)"),
    (0x9369, "JUMP (0x000080DC)"),
    (0x937F, "JUMP (0x00009324)"),
    (0x93B4, "R2 = 0x00000003"),
    (0x93BA, "JUMP (0x0000931C)"),
    (0x93F2, "I3 = 0x02000000"),
    (0x93F9, "LCNTR = 0x00100000"),
    (0x940A, "R0 = 0x01018000"),
    (0x940C, "CALL (0x00009324)"),
    (0x941C, "JUMP (0x000080DC)"),
    (0x941E, "CALL (0x0000936A)"),
    (0x941F, "JUMP (0x000080DC)"),
    (0x9425, "CALL (0x00009428)"),
    (0x9427, "RTS"),
    (0x9429, "R0 = BTGL R0 BY 15"),
    (0x942B, "LCNTR = 0x0050, DO (0x0000942F) UNTIL LCE"),
    (0x942F, "CALL (0x0000930F)"),
    (0x943A, "R2 = R4 * R2"),
    (0x943B, "CALL (0x00009333)"),
    (0x943D, "JUMP (0x000080DC)"),
    (0x95A0, "I0 = 0x0001B978"),
    (0x95A2, "JUMP (M15, I15)"),
)

FLASH = {
    "none": (
        "0x52", "0x53", "0x54", "0x55", "0x56", "0x57", "0x59", "0x5C", "0x5D",
        "0x5E", "0x5F", "0x61", "0x63", "0x65", "0x66", "0x69", "0x6E", "0x6F",
        "0x73", "0x74", "0x75", "0x76",
    ),
    "program": ("0x6D", "0x71"),
    "erase": ("0x6C",),
    "program_and_erase": ("0x60", "0x67", "0x72"),
    "external_fill": ("0x68",),
}

STATE_PREAMBLE = ("0x81FA", "0x81FA", "0x81FA", "0x93E3", "0x81FA", "0x81FA")
MAIN_LOOP_STATE = ("0x81FA", "0x93AE", "0x93B4", "0x93BB", "0x93C6", "0x93CB")
TRANSMIT_OPCODES = (0x50, 0x51, 0x54, 0x56, 0x58, 0x5C, 0x5E, 0x6D, 0x71, 0x72, 0x74)
_OPCODE_LOAD = re.compile(r"^R0 = 0x([0-9A-F]{8})$")


def _program_bytes(payload):
    cursor = 0
    chunks = []
    for record in pod_load_records(payload):
        start = cursor + 10
        stop = start + record["payload_bytes"]
        if record["width_tag_raw"] == 3:
            chunks.append((record["destination_word_raw"], payload[start:stop]))
        cursor = stop
    chunks.sort()
    if [addr for addr, _ in chunks] != [0x8000, 0x8080]:
        raise ValueError("unexpected 48-bit program records")
    return b"".join(blob for _, blob in chunks)


def _width1_words(payload):
    cursor = 0
    for record in pod_load_records(payload):
        start = cursor + 10
        stop = start + record["payload_bytes"]
        if record["destination_word_raw"] == 0x1A900 and record["width_tag_raw"] == 1:
            raw = payload[start:stop]
            return [int.from_bytes(raw[i:i + 2], "big") for i in range(0, len(raw), 2)]
        cursor = stop
    raise ValueError("missing 16-bit load record")


def _disassembly(program):
    if not DASM.is_file():
        raise ValueError("private SHARC disassembler is not built")
    with tempfile.NamedTemporaryFile() as tmp:
        tmp.write(program)
        tmp.flush()
        out = subprocess.check_output([str(DASM), "--base", "0x8000", tmp.name], text=True)
    listing = {}
    for line in out.splitlines():
        addr, text = line.split("\t", 1)
        listing[int(addr, 16)] = text
    return listing


def _class_counts(words):
    counts = {"zero": 0, "program": 0, "outside": 0}
    for word in words:
        if word == 0:
            counts["zero"] += 1
        elif PROGRAM_START <= word < PROGRAM_END:
            counts["program"] += 1
        else:
            counts["outside"] += 1
    return counts


def _transmit_facts(listing):
    """Opcode immediates sent between F0 and the payload, and stray transmit sites."""
    targets = ("0x000080D6", "0x000080D8", "0x000080D9")
    outside = [
        addr for addr, text in listing.items()
        if any(f"CALL ({target})" in text or f"JUMP ({target})" in text for target in targets)
        and not (0x80D6 <= addr <= 0x81A5)
    ]
    opcodes = []
    for addr, text in listing.items():
        match = _OPCODE_LOAD.fullmatch(text)
        if match is None:
            continue
        if listing.get(addr - 1) == "CALL (0x000080D6)" and listing.get(addr + 1) == "CALL (0x000080D9)":
            opcodes.append(int(match.group(1), 16))
    if sorted(set(opcodes)) != sorted(TRANSMIT_OPCODES):
        raise ValueError(f"transmit opcode immediates are {sorted(set(opcodes))}")
    if outside:
        raise ValueError(f"transmit sites outside 0x80D6-0x81A5: {outside}")
    if any("= 0x00000045" in text for text in listing.values()):
        raise ValueError("decoded program contains an immediate 0x45")
    if any("= 0x00000013" in text for text in listing.values()):
        raise ValueError("decoded program contains a register load of 0x13")
    return [f"0x{opcode:02X}" for opcode in TRANSMIT_OPCODES]


def inventory(archive):
    if hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("expected the exact archived PODxt Live 3.01 container")
    header, image = inspect(archive)
    region = image[header["regions"][4]["offset"]:header["regions"][4]["offset"] + header["regions"][4]["bytes"]]
    _section, payload = pod_word_section(region)
    listing = _disassembly(_program_bytes(payload))
    opcodes = _transmit_facts(listing)
    for addr, text in EVIDENCE:
        actual = listing.get(addr)
        if actual != text:
            raise ValueError(f"disassembly at {addr:#x} is {actual!r}, expected {text!r}")
    words = _width1_words(payload)
    base = 0x1A900
    preamble = [f"0x{words[0x1B917 - base + i]:04X}" for i in range(6)]
    main_state = [f"0x{words[0x1B911 - base + i]:04X}" for i in range(6)]
    if preamble != list(STATE_PREAMBLE) or main_state != list(MAIN_LOOP_STATE):
        raise ValueError("state tables differ from the locked words")
    if any(word == 0x940A for word in words):
        raise ValueError("16-bit load record contains 0x940A")
    low = words[0x1BB78 - base:0x1BB78 - base + 128]
    if not all(PROGRAM_START <= word < PROGRAM_END for word in low):
        raise ValueError("low half of the 0x54 jump table leaves the program")
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "evidence_count": len(EVIDENCE),
        "flash": {name: list(opcodes) for name, opcodes in FLASH.items()},
        "state_preamble_0x1B917": preamble,
        "main_loop_state_0x1B911": main_state,
        "table_0x1B978": _class_counts(words[0x1B978 - base:0x1B978 - base + 256]),
        "table_0x1BB78_low128_all_program": True,
        "routine_0x940A_in_width1_words": False,
        "transmit_opcode_immediates": opcodes,
        "transmit_sites_outside_0x80D6_0x81A5": 0,
        "immediate_0x45": False,
        "register_immediate_0x13": False,
        "target_confidence": "confirmed",
        "flash_identity": "likely",
        "host_to_sharc": "contradicted by owner hardware runs; host-to-SHARC translation unknown",
        "sharc_scope": "SHARC internal command set; no host message is shown to reach these routines",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", type=Path, help="compare with the checked-in command record")
    args = parser.parse_args()
    try:
        if not args.archive.is_file():
            raise ValueError("archive must be a regular file")
        result = inventory(args.archive.read_bytes())
        if args.check is None:
            print(json.dumps(result, indent=2))
        elif json.loads(args.check.read_text()) != result:
            raise ValueError(f"command record differs from {args.check}")
        else:
            print(f"PASS: {result['evidence_count']} instruction lines match")
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
