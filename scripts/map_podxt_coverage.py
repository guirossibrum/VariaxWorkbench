"""Coverage and editor-trap checks for the PODxt Live 3.01 archive.

Static only. With --check it disassembles the region-4 program using the
private SHARC tool. It does not execute firmware and it does not open a device.
"""

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

from inspect_firmware import inspect
from map_firmware import pod_word_section
from map_podxt_commands import PROGRAM_END, PROGRAM_START, _disassembly, _program_bytes, _width1_words
from map_podxt_entry import RETURN_WORD, TABLE_BASE

HOST_TO_SHARC = (
    "contradicted by owner hardware runs; host-to-SHARC translation unknown"
)
SHARC_SCOPE = (
    "SHARC internal command set; no host message is shown to reach these routines"
)
ARCHIVE_SHA256 = "738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5"
IMAGE_BYTES = 0x100000
FAMILY_SOURCE = Path("crates/line6-devices/src/family/podxt/mod.rs")
DIRECT = re.compile(r"(CALL|JUMP) \((0x[0-9A-F]{8})\)")

# Opcodes whose entry behavior is recorded in the command and store sections.
TRACED = {
    0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x59, 0x5C, 0x5D, 0x5E, 0x5F,
    0x60, 0x61, 0x63, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6C, 0x6D, 0x6E,
    0x6F, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76,
}
RECEIVE = {0xBF, 0xCE, 0xDD, 0xFB}
GLOBAL_IDS = [0, 1, 4, 5, 16, 17, 18, 19, 20, 21, 22, 23, 26, 27, 28, 29, 30, 31, 32, 33, 34]

# index, offset, size, type_raw, role. Roles for 1–3 and 6–11 are the observed
# roles already recorded in the firmware map. FF and the load/payload/half
# roles are checked against the bytes below.
REGIONS = (
    (0, 0x00000, 0x4000, 4, "entirely FF"),
    (1, 0x04000, 0x2000, 1, "settings or name-bearing data"),
    (2, 0x06000, 0x2000, 1, "settings or name-bearing data"),
    (3, 0x08000, 0x8000, 0, "repeated patterns consistent with display data"),
    (4, 0x10000, 0x10000, 0, "SHARC load image"),
    (5, 0x20000, 0x10000, 0, "unidentified length-and-checksum payload"),
    (6, 0x30000, 0x10000, 3, "resource directory and named records"),
    (7, 0x40000, 0x10000, 3, "resource directory and named records"),
    (8, 0x50000, 0x10000, 3, "resource directory and named records"),
    (9, 0x60000, 0x10000, 3, "resource directory and named records"),
    (10, 0x70000, 0x10000, 3, "resource directory and named records"),
    (11, 0x80000, 0x10000, 3, "resource directory and named records"),
    (12, 0x90000, 0x10000, 4, "entirely FF"),
    (13, 0xA0000, 0x10000, 4, "entirely FF"),
    (14, 0xB0000, 0x10000, 4, "entirely FF"),
    (15, 0xC0000, 0x10000, 4, "entirely FF"),
    (16, 0xD0000, 0x10000, 4, "entirely FF"),
    (17, 0xE0000, 0x10000, 2, "factory records and lower stored half"),
    (18, 0xF0000, 0x10000, 2, "entirely FF upper stored half"),
)

EVIDENCE = (
    (0x80CB, "JUMP (M15, I15)"),
    (0x81CC, "DM(0x00000009, I0) = R0"),
    (0x81CD, "R0 = DM(I1, 0x01)"),
    (0x81D3, "RTS"),
    (0x8004, "IDLE"),
    (0x8005, "JUMP (0x0000823C)"),
    (0x8010, "JUMP (0x00009348)"),
    (0x8020, "JUMP (0x000080CF)"),
    (0x8028, "JUMP (0x00009105)"),
)


def editor_contract(source):
    start = source.index("fn bytes(self)")
    end = source.index("fn path(self)", start)
    body = source[start:end].lower()
    for token in ("0x75", "0x0e", "0x23", "0x10", "0x57", "0x73", "0xb0", "0xc0"):
        if token not in body:
            raise ValueError(f"PodxtRequest::bytes does not contain {token}")
    for token in ("0x71", "0x60", "0x67", "0x68", "0x6c", "0x6d", "0x6e", "0x72", "0x74", "0x56", "0x54"):
        if token in body:
            raise ValueError(f"PodxtRequest::bytes contains {token}")
    if "identity::request" not in body:
        raise ValueError("PodxtRequest::bytes no longer sends the universal identity inquiry")
    stored = body.split("0x73", 1)[1]
    if stored.count("0x00") < 2:
        raise ValueError("stored read does not send two zero count bytes")
    id_start = source.index("pub fn id(self)")
    id_end = source.index("pub fn name(self)", id_start)
    ids = [int(number) for number in re.findall(r"=>\s*(\d+)", source[id_start:id_end])]
    if ids != GLOBAL_IDS:
        raise ValueError(f"global ids differ: {ids}")
    return ids


def _closure(listing, seeds):
    seen = set()
    pending = list(seeds)
    while pending:
        pc = pending.pop()
        while PROGRAM_START <= pc < PROGRAM_END and pc not in seen:
            seen.add(pc)
            text = listing.get(pc)
            if text is None:
                break
            match = DIRECT.search(text)
            if text.startswith(("RTS", "RTI")):
                break
            if match and (
                text.startswith(("JUMP", "CALL", "IF")) or ", JUMP" in text or ", CALL" in text
            ):
                target = int(match.group(2), 16)
                if text.startswith("JUMP") and not text.startswith("IF"):
                    pc = target
                    continue
                if PROGRAM_START <= target < PROGRAM_END:
                    pending.append(target)
                if text.startswith(("CALL", "IF")):
                    pc += 1
                    continue
            if ("JUMP" in text and match is None and text.startswith("JUMP") and not text.startswith("IF")):
                break
            pc += 1
    return seen


def _indirect(text):
    return ("JUMP" in text or "CALL" in text) and DIRECT.search(text) is None


def _region5_xor(payload):
    """Count 6-byte ASCII windows after each single-byte XOR.

    A constant XOR permutes byte values and preserves entropy, so the
    entropy is not a signal. The window count is.
    """
    readable = 0
    for key in range(256):
        decoded = bytes(byte ^ key for byte in payload)
        start = 0
        while True:
            found = min(
                (index for token in (b"POD", b"USB", b"Line") if (index := decoded.find(token, start)) >= 0),
                default=-1,
            )
            if found < 0:
                break
            window = decoded[found:found + 6]
            if len(window) == 6 and all(32 <= byte < 127 for byte in window):
                readable += 1
            start = found + 1
    return readable


def inventory(archive, family_source):
    if hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("expected the exact archived PODxt Live 3.01 container")
    if len(archive) < 32:
        raise ValueError("archive is too small to be the container")
    header, image = inspect(archive)
    if len(image) != IMAGE_BYTES or len(header["regions"]) != len(REGIONS):
        raise ValueError("image layout differs")
    regions = []
    ff_bytes = 0
    blank_bytes = 0
    for index, offset, size, type_raw, role in REGIONS:
        region = header["regions"][index]
        if (region["offset"], region["bytes"], region["type_raw"]) != (offset, size, type_raw):
            raise ValueError(f"region {index} placement differs")
        blob = image[offset:offset + size]
        ff = blob.count(0xFF)
        ff_bytes += ff
        entirely_ff = ff == size
        if role.startswith("entirely FF") and not entirely_ff:
            raise ValueError(f"region {index} is not entirely FF")
        if entirely_ff:
            blank_bytes += size
        regions.append({
            "index": index,
            "offset": f"0x{offset:05X}",
            "bytes": size,
            "type_raw": type_raw,
            "ff_bytes": ff,
            "role": role,
        })
    if sum(item["bytes"] for item in regions) != IMAGE_BYTES:
        raise ValueError("regions do not cover the image")
    region4 = image[0x10000:0x20000]
    _section, payload = pod_word_section(region4)
    listing = _disassembly(_program_bytes(payload))
    if len(listing) != PROGRAM_END - PROGRAM_START:
        raise ValueError("disassembly does not cover the loaded program")
    for addr, text in EVIDENCE:
        if listing.get(addr) != text:
            raise ValueError(f"disassembly at {addr:#x} is {listing.get(addr)!r}")
    vector_exceptions = [
        addr for addr in range(0x8000, 0x8080)
        if listing[addr] not in ("NOP", "RTI")
    ]
    if vector_exceptions != [0x8004, 0x8005, 0x8010, 0x8020, 0x8028]:
        raise ValueError(f"unexpected vector exceptions: {vector_exceptions}")
    words = _width1_words(payload)
    table = words[TABLE_BASE - 0x1A900:TABLE_BASE - 0x1A900 + 256]
    if len(table) != 256:
        raise ValueError("dispatch table is not 256 words")
    dispositions = {}
    counts = {}
    for opcode, word in enumerate(table):
        if word == 0:
            label = "zero"
        elif word == RETURN_WORD:
            label = "return"
        elif not PROGRAM_START <= word < PROGRAM_END:
            label = "outside"
        elif listing[word].startswith("RTS"):
            label = "immediate_rts"
        elif opcode in TRACED:
            label = "traced_entry"
        elif opcode in RECEIVE:
            if word != 0x8020 or listing[word] != "JUMP (0x000080CF)":
                raise ValueError(f"receive opcode {opcode:#x} is not 0x8020")
            label = "receive_vector"
        elif opcode == 0xEC:
            label = "mid_copy"
        else:
            raise ValueError(f"opcode {opcode:#x} has no disposition")
        dispositions[f"0x{opcode:02X}"] = label
        counts[label] = counts.get(label, 0) + 1
    if sum(counts.values()) != 256:
        raise ValueError("dispositions do not cover 256 opcodes")
    copy = " ".join(listing[addr] for addr in range(0x81CC, 0x81D4))
    if any(token in copy for token in ("0x0000930F", "0x00009324", "0x0000936A", "CALL")):
        raise ValueError("opcode 0xEC reaches a call before RTS")
    seeds = {int(match.group(2), 16) for addr in range(0x8000, 0x8080) if (match := DIRECT.search(listing[addr]))}
    seeds.update(word for word in table if PROGRAM_START <= word < PROGRAM_END)
    seen = _closure(listing, seeds)
    for addr in (0x80B2, 0x808B, 0x930F, 0x9324, 0x936A, 0x9380, 0x9420, 0x95C9):
        if addr not in seen:
            raise ValueError(f"{addr:#x} is outside the direct closure")
    markers = sorted(addr for addr, text in listing.items() if "?" in text or "TODO" in text)
    if any(addr < 0x8275 or addr > 0x82AC or addr in seen for addr in markers):
        raise ValueError("marker lines are not confined to the block after the main loop")
    indirect = sorted(addr for addr, text in listing.items() if _indirect(text))
    ids = editor_contract(family_source)
    editor_opcodes = ["0x0E", "0x10", "0x23", "0x57", "0x73", "0x75"]
    if any(dispositions[opcode] != "zero" for opcode in ("0x0E", "0x10", "0x23")):
        raise ValueError("an editor opcode is no longer a zero target")
    if any(dispositions[opcode] != "traced_entry" for opcode in ("0x57", "0x73", "0x75")):
        raise ValueError("an editor read opcode is no longer a traced entry")
    if listing[0x818B] != "LCNTR = 0x00A0, DO (0x0000818D) UNTIL LCE":
        raise ValueError("opcode 0x75 transmit loop changed")
    if "pub const EDIT_BUFFER_LEN: usize = 168" not in family_source:
        raise ValueError("host edit-buffer length changed")
    if image.find("No Amp".encode("utf-16-be")) != 0x31000:
        raise ValueError("resource name No Amp moved")
    if image.find("Bypass".encode("utf-16-be")) != 0x31020:
        raise ValueError("resource name Bypass moved")
    anchor_counts = (
        image.count(bytes.fromhex("f07e7f0602")),
        image.count(bytes.fromhex("f000010c")),
        image.count(bytes.fromhex("00010c03")),
        image.count(b"3.01"),
        image.count("3.01".encode("utf-16-be")),
        sum(1 for text in listing.values() if "0x0000007E" in text),
    )
    if any(anchor_counts):
        raise ValueError("a host identity or version anchor is present in the image")
    xor_readable = _region5_xor(image[0x20008:0x30000])
    if xor_readable:
        raise ValueError("region 5 XOR screen found a readable window")
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "image_bytes": IMAGE_BYTES,
        "region_count": len(regions),
        "ff_bytes": ff_bytes,
        "blank_region_bytes": blank_bytes,
        "regions": regions,
        "program_words": PROGRAM_END - PROGRAM_START,
        "marker_lines": len(markers),
        "direct_closure_words": len(seen),
        "indirect_transfers": len(indirect),
        "indirect_transfers_in_closure": sum(1 for addr in indirect if addr in seen),
        "vector_exceptions": [f"0x{addr:04X}" for addr in vector_exceptions],
        "disposition_counts": counts,
        "dispositions": dispositions,
        "editor_sysex_opcodes": editor_opcodes,
        "editor_zero_targets": ["0x0E", "0x10", "0x23"],
        "global_ids": ids,
        "global_id_max": max(ids),
        "evidence_count": len(EVIDENCE),
        "target_confidence": "confirmed",
        "host_to_sharc": HOST_TO_SHARC,
        "sharc_scope": SHARC_SCOPE,
        "identity_reply_prefix_count": image.count(bytes.fromhex("f07e7f0602")),
        "family_header_count": image.count(bytes.fromhex("f000010c")),
        "manufacturer_family_count": image.count(bytes.fromhex("00010c03")),
        "ascii_version_3_01_count": image.count(b"3.01"),
        "utf16_version_3_01_count": image.count("3.01".encode("utf-16-be")),
        "immediate_0x7e_count": sum(1 for text in listing.values() if "0x0000007E" in text),
        "sysex_75_loop": listing[0x818B],
        "sharc_75_payload_bytes": 0xA0,
        "sharc_75_frame_bytes": 163,
        "host_edit_buffer_bytes": 168,
        "resource_name_no_amp": "0x31000",
        "resource_name_bypass": "0x31020",
        "region5_xor_readable_windows": xor_readable,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--family-source", type=Path, default=FAMILY_SOURCE)
    parser.add_argument("--check", type=Path, help="compare with the checked-in coverage record")
    args = parser.parse_args()
    try:
        if not args.archive.is_file() or not args.family_source.is_file():
            raise ValueError("archive and family source must be regular files")
        result = inventory(args.archive.read_bytes(), args.family_source.read_text())
        if args.check is None:
            print(json.dumps(result, indent=2))
        elif json.loads(args.check.read_text()) != result:
            raise ValueError(f"coverage record differs from {args.check}")
        else:
            print(
                f"PASS: {result['direct_closure_words']} of {result['program_words']} "
                "program words are on the direct closure"
            )
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
