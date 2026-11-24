"""Read the PODxt Live 3.01 SysEx entry table from the private archive.

Static only. This reads the archived container, classifies the 256-word
dispatch table, and measures region 5. It does not execute firmware and it
does not open a device.
"""

import argparse
import collections
import gzip
import hashlib
import math
import json
import zlib
from pathlib import Path

from inspect_firmware import inspect
from map_firmware import pod_load_records, pod_word_section

ARCHIVE_SHA256 = "738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5"
IMAGE_SHA256 = "58204bd9d8e4337676a23e78fab19baf221d2c48a9a3961ccb27a7a1bc3c0b93"
TABLE_BASE = 0x1A9C1
OPCODE_COUNT = 256
# Word 0x80CE disassembles to RTS. The class name records that address match.
RETURN_WORD = 0x80CE
NAME_SCAN = ("midi", "usb", "sharc", "pod", "line", "variax", "boot", "flash")


def classify_target(word, program_start, program_end, return_word):
    if word == 0:
        return "zero"
    if word == return_word:
        return "return"
    if program_start <= word < program_end:
        return "program"
    return "outside_program"


def shannon_entropy(values):
    if not values:
        return 0.0
    counts = collections.Counter(values)
    total = len(values)
    return -sum((count / total) * math.log2(count / total) for count in counts.values())


def _width1_words(payload):
    records = pod_load_records(payload)
    cursor = 0
    words = None
    destination = None
    for record in records:
        width = record["width_tag_raw"]
        count = record["word_count"]
        start = cursor + 10
        stop = start + record["payload_bytes"]
        if width == 1 and record["destination_word_raw"] <= TABLE_BASE < record["destination_word_raw"] + count:
            destination = record["destination_word_raw"]
            raw = payload[start:stop]
            words = [int.from_bytes(raw[index:index + 2], "big") for index in range(0, len(raw), 2)]
        cursor = stop
    if words is None:
        raise ValueError("dispatch table is outside the 16-bit load record")
    index = TABLE_BASE - destination
    if index + OPCODE_COUNT > len(words):
        raise ValueError("dispatch table exceeds the 16-bit load record")
    return destination, len(words), words[index:index + OPCODE_COUNT]


def _program_span(payload):
    spans = []
    for record in pod_load_records(payload):
        if record["width_tag_raw"] != 3:
            continue
        start = record["destination_word_raw"]
        spans.append((start, start + record["word_count"]))
    spans.sort()
    if len(spans) != 2 or spans[0][1] != spans[1][0]:
        raise ValueError("expected two contiguous 48-bit program records")
    return spans[0][0], spans[1][1]


def _printable_runs(payload):
    runs = []
    index = 0
    while index < len(payload):
        if 32 <= payload[index] < 127:
            end = index + 1
            while end < len(payload) and 32 <= payload[end] < 127:
                end += 1
            if end - index >= 5:
                runs.append(payload[index:end])
            index = end
        else:
            index += 1
    return runs


def _decompress_successes(payload):
    tried = 0
    successes = 0
    for offset in (0, 2, 4, 8):
        for function in (zlib.decompress, gzip.decompress):
            tried += 1
            try:
                function(payload[offset:])
                successes += 1
            except (zlib.error, OSError, EOFError, ValueError):
                pass
    zlib_magic = []
    gzip_magic = []
    for signature, bucket, function in (
        (b"\x78\xda", zlib_magic, zlib.decompress),
        (b"\x1f\x8b", gzip_magic, gzip.decompress),
    ):
        start = 0
        while True:
            found = payload.find(signature, start)
            if found < 0:
                break
            bucket.append(found)
            tried += 1
            try:
                function(payload[found:])
                successes += 1
            except (zlib.error, OSError, EOFError, ValueError):
                pass
            start = found + 1
    return tried, successes, zlib_magic, gzip_magic


def _region5(region):
    section, payload = pod_word_section(region)
    runs = _printable_runs(payload)
    tried, successes, zlib_magic, gzip_magic = _decompress_successes(payload)
    words = [int.from_bytes(payload[index:index + 2], "big") for index in range(0, len(payload) - 1, 2)]
    common = collections.Counter(words).most_common(1)[0]
    blocks = [payload[index:index + 16] for index in range(0, len(payload) - 15, 16)]
    block_counts = collections.Counter(blocks)
    try:
        pod_load_records(payload)
    except ValueError as error:
        load_error = str(error)
    else:
        raise ValueError("region 5 parsed as load records")
    name_hits = sum(
        1 for run in runs if any(name in run.decode("ascii").lower() for name in NAME_SCAN)
    )
    compressed = zlib.compress(payload, 9)
    return {
        "payload_bytes": section["payload_bytes"],
        "padding_ff_bytes": section["padding_ff_bytes"],
        "payload_sha256": section["payload_sha256"],
        "entropy_bits_per_byte": f"{shannon_entropy(payload):.4f}",
        "distinct_byte_values": len(set(payload)),
        "zlib_level9_bytes": len(compressed),
        "word16_count": len(words),
        "word16_distinct": len(set(words)),
        "word16_entropy_bits": f"{shannon_entropy(words):.3f}",
        "word16_most_common": f"0x{common[0]:04X}",
        "word16_most_common_count": common[1],
        "aligned16_blocks": len(blocks),
        "aligned16_distinct": len(block_counts),
        "aligned16_max_repeat": max(block_counts.values()),
        "printable_runs_at_least_5": len(runs),
        "printable_runs_at_least_8": sum(1 for run in runs if len(run) >= 8),
        "longest_printable_run": max((len(run) for run in runs), default=0),
        "name_scan": list(NAME_SCAN),
        "name_scan_hits": name_hits,
        "decompress_attempts": tried,
        "decompress_successes": successes,
        "zlib_magic_78da_offsets": zlib_magic,
        "gzip_magic_offsets": gzip_magic,
        "load_record_error": load_error,
    }


def inventory(archive):
    if hashlib.sha256(archive).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("expected the exact archived PODxt Live 3.01 container")
    header, image = inspect(archive)
    if hashlib.sha256(image).hexdigest() != IMAGE_SHA256 or len(image) != 1048576:
        raise ValueError("expected the 1,048,576-byte PODxt Live 3.01 image")
    if header["product_id"] != "0003000a" or header["version_hex"] != "03010000":
        raise ValueError("unexpected product or version")
    region4 = image[header["regions"][4]["offset"]:header["regions"][4]["offset"] + header["regions"][4]["bytes"]]
    region5 = image[header["regions"][5]["offset"]:header["regions"][5]["offset"] + header["regions"][5]["bytes"]]
    _section, payload = pod_word_section(region4)
    program_start, program_end = _program_span(payload)
    if not program_start <= RETURN_WORD < program_end:
        raise ValueError("return word is outside the loaded program")
    destination, width1_count, table = _width1_words(payload)
    targets = {}
    counts = collections.Counter()
    for opcode, word in enumerate(table):
        if word > 0xFFFF:
            raise ValueError(f"table word {word:#x} does not fit the recorded hex width")
        kind = classify_target(word, program_start, program_end, RETURN_WORD)
        counts[kind] += 1
        targets[f"0x{opcode:02X}"] = f"0x{word:04X}"
    offsets = []
    start = 0
    signature = bytes.fromhex("00010c")
    while True:
        found = image.find(signature, start)
        if found < 0:
            break
        offsets.append(f"0x{found:X}")
        start = found + 1
    return {
        "archive_sha256": ARCHIVE_SHA256,
        "image_sha256": IMAGE_SHA256,
        "product_id": header["product_id"],
        "version_hex": header["version_hex"],
        "image_bytes": len(image),
        "program_start": f"0x{program_start:X}",
        "program_end_exclusive": f"0x{program_end:X}",
        "program_words": program_end - program_start,
        "return_word": f"0x{RETURN_WORD:X}",
        "table_base": f"0x{TABLE_BASE:X}",
        "table_index_width_bits": 8,
        "width1_destination": f"0x{destination:X}",
        "width1_count": width1_count,
        "class_counts": {name: counts[name] for name in ("zero", "return", "program", "outside_program")},
        "targets": targets,
        "sequence_f0_00_01_0c": image.count(bytes.fromhex("f000010c")),
        "sequence_00_01_0c_offsets": offsets,
        "region5": _region5(region5),
        "target_confidence": "confirmed",
        "region5_identity": "unknown",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--check", type=Path, help="compare with a checked-in inventory")
    args = parser.parse_args()
    try:
        if not args.archive.is_file():
            raise ValueError("archive must be a regular file")
        result = inventory(args.archive.read_bytes())
        if args.check is None:
            print(json.dumps(result, indent=2))
        elif json.loads(args.check.read_text()) != result:
            raise ValueError(f"inventory differs from {args.check}")
        else:
            print(f"PASS: all {sum(result['class_counts'].values())} dispatcher targets match")
    except (KeyError, OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
