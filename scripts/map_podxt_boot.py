"""Check the PODxt Live 3.01 boot-path claims against the archive.

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
from map_firmware import pod_word_section
from map_podxt_commands import PROGRAM_END, PROGRAM_START, _disassembly, _program_bytes
from map_podxt_store import _width2_word

ARCHIVE_SHA256 = "738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5"
CHECKSUM_START = 0x01000000
CHECKSUM_COUNT = 0x00080001
GLOBAL_BASE = 0x0001B9A8

EVIDENCE = (
    (0x8028, "JUMP (0x00009105)"),
    (0x8089, "R0 = DM(0x000000E1)"),
    (0x808A, "BTST R0 BY 30"),
    (0x808B, "IF SZ, JUMP (0x00008089)"),
    (0x80AA, "I0 = 0x02000000"),
    (0x80AB, "R0 = 0x00000000"),
    (0x80AC, "R1 = 0x00100000"),
    (0x80AD, "LCNTR = R1, DO (0x000080AE)"),
    (0x80AE, "DM(I0, 0x01) = R0"),
    (0x8124, "R2 = R2 XOR R2,  R0 = DM(I4, 0x01)"),
    (0x8125, "M1 = R0"),
    (0x812A, "I0 = 0x0001B9A8"),
    (0x812B, "DM(M1, I0) = R0"),
    (0x8133, "I0 = 0x0001B9A8"),
    (0x8134, "R0 = DM(M1, I0)"),
    (0x825E, "CALL (0x00009380)"),
    (0x825F, "CALL (0x00008080)"),
    (0x8260, "CALL (0x00009609)"),
    (0x8266, "I0 = 0x01000000"),
    (0x8267, "R1 = 0x00080001"),
    (0x8268, "R0 = -R2"),
    (0x8269, "LCNTR = R1, DO (0x0000826D)"),
    (0x826A, "R0 = R0 + R2,  R5 = DM(I0, 0x01)"),
    (0x826B, "R2 = FDEP R5 BY 0:8"),
    (0x826C, "R1 = FEXT R5 BY 8:8"),
    (0x826D, "R2 = R2 + R1"),
    (0x826E, "DM(0x0001B9AB) = R0"),
    (0x826F, "R1 = LSHIFT R0 BY -16"),
    (0x8270, "DM(0x0001B9AA) = R1"),
    (0x8271, "CALL (0x000080B2)"),
    (0x8272, "CALL (0x000093AA)"),
    (0x8273, "CALL (0x000095C5)"),
    (0x8274, "JUMP (0x00008271)"),
    (0x910D, "R1 = R1 XOR R1"),
    (0x910E, "DM(0x0000C2E5) = R1"),
    (0x95C5, "R0 = DM(0x0000C2E5)"),
    (0x95C6, "R0 = R0 + 1"),
    (0x95C7, "DM(0x0000C2E5) = R0"),
    (0x95C8, "BTST R0 BY 19"),
    (0x95C9, "IF NOT SZ, JUMP (0x000095C9)"),
)


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
    named = sorted(addr for addr, text in listing.items() if "0x0001B9AA" in text or "0x0001B9AB" in text)
    if named != [0x826E, 0x8270]:
        raise ValueError(f"unexpected checksum-address instructions: {named}")
    if any("R2" in listing[addr] for addr in range(0x823C, 0x8268)):
        raise ValueError("R2 is written before the checksum negation")
    writers = sorted(addr for addr, text in listing.items() if text.startswith("DM(0x0000C2E5) ="))
    if writers != [0x910E, 0x95C7]:
        raise ValueError(f"unexpected C2E5 writers: {writers}")
    entries = sorted(addr for addr, text in listing.items() if "0x00009105" in text)
    if entries != [0x8028]:
        raise ValueError(f"unexpected entries to 0x9105: {entries}")
    fill = " ".join(listing[addr] for addr in range(0x8080, 0x80B2))
    if any(token in fill for token in ("0x0000930F", "0x0000931C", "0x00009324", "0x0000936A")):
        raise ValueError("0x8080 reaches a flash routine")
    if _width2_word(payload, 0xC2E5) != 0:
        raise ValueError("loaded DM(0xC2E5) is not zero")
    loop_targets = sorted(
        addr for addr, text in listing.items()
        if "JUMP (0x00008266)" in text or "CALL (0x00008266)" in text
    )
    if loop_targets:
        raise ValueError(f"checksum is re-entered: {loop_targets}")
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "evidence_count": len(EVIDENCE),
        "checksum_start": f"0x{CHECKSUM_START:08X}",
        "checksum_count": f"0x{CHECKSUM_COUNT:08X}",
        "checksum_instructions": ["0x826E", "0x8270"],
        "r2_text_before_checksum": 0,
        "loaded_c2e5": "0x00000000",
        "c2e5_writers": ["0x910E", "0x95C7"],
        "counter_clear_entry": "0x8028",
        "global_base": f"0x{GLOBAL_BASE:08X}",
        "target_confidence": "confirmed",
        "host_to_sharc": "contradicted by owner hardware runs; host-to-SHARC translation unknown",
        "sharc_scope": "SHARC internal command set; no host message is shown to reach these routines",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", type=Path, help="compare with the checked-in boot record")
    args = parser.parse_args()
    try:
        if not args.archive.is_file():
            raise ValueError("archive must be a regular file")
        result = inventory(args.archive.read_bytes())
        if args.check is None:
            print(json.dumps(result, indent=2))
        elif json.loads(args.check.read_text()) != result:
            raise ValueError(f"boot record differs from {args.check}")
        else:
            print(f"PASS: {result['evidence_count']} boot-path lines match")
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
