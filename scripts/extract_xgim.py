#!/usr/bin/env python3
"""Split an XGIM container of concatenated PNGs into numbered files.

POD Farm 2.58 stores UI art in ``.imb`` files. The ones inspected for the
layout work start with a 16-byte header and then PNG bytes, one after another.
They do not carry a name index. This script does not invent names.

Header, big-endian, as observed in POD Farm 2.58 ``App.imb``:

    0   4s   magic ``XGIM``
    4   u32  index offset; past the end of the file means there is no index
    8   u32  image count
    12  u32  reserved, observed as 0
    16       PNG payloads

Write the images under an ignored directory such as ``analysis/``. The script
refuses to replace an existing output file.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from pathlib import Path

MAGIC = b"XGIM"
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
IEND = b"IEND\xaeB`\x82"
HEADER = struct.Struct(">4sIII")


class XgimError(ValueError):
    """The container is not a complete XGIM image list."""


def parse_xgim(data: bytes) -> dict:
    if len(data) < HEADER.size or not data.startswith(MAGIC):
        raise XgimError("not an XGIM container")
    _magic, index_offset, count, reserved = HEADER.unpack_from(data, 0)
    images = []
    pos = HEADER.size
    while True:
        start = data.find(PNG_MAGIC, pos)
        if start < 0:
            break
        end_mark = data.find(IEND, start)
        if end_mark < 0:
            raise XgimError(f"PNG at offset {start} has no IEND")
        end = end_mark + len(IEND)
        blob = data[start:end]
        width, height = _png_size(blob)
        images.append(
            {
                "offset": start,
                "width": width,
                "height": height,
                "bytes": blob,
            }
        )
        pos = end
    if len(images) != count:
        raise XgimError(f"header count {count} but found {len(images)} PNG payloads")
    trailing = data[pos:]
    index_inside = HEADER.size <= index_offset < len(data)
    return {
        "count": count,
        "index_offset": index_offset,
        "reserved": reserved,
        "has_name_index": False,
        "index_inside_file": index_inside,
        "trailing_bytes": len(trailing),
        "images": images,
    }


def _png_size(blob: bytes) -> tuple[int, int]:
    # IHDR width and height follow the 8-byte signature, 4-byte length and 4-byte type.
    if len(blob) < 24 or blob[12:16] != b"IHDR":
        raise XgimError("PNG payload has no IHDR")
    width, height = struct.unpack(">II", blob[16:24])
    if width < 1 or height < 1:
        raise XgimError(f"PNG size {width}x{height} is empty")
    return width, height


def manifest_rows(parsed: dict) -> list[dict]:
    rows = []
    for index, image in enumerate(parsed["images"]):
        blob = image["bytes"]
        rows.append(
            {
                "file": f"{index:04d}.png",
                "index": index,
                "width": image["width"],
                "height": image["height"],
                "bytes": len(blob),
                "sha256": hashlib.sha256(blob).hexdigest(),
            }
        )
    return rows


def write_images(parsed: dict, output: Path) -> Path:
    output.mkdir(parents=True, exist_ok=True)
    rows = manifest_rows(parsed)
    targets = [output / row["file"] for row in rows]
    targets.append(output / "manifest.json")
    existing = [path for path in targets if path.exists()]
    if existing:
        raise XgimError(f"refusing to replace {existing[0]}")
    for image, target in zip(parsed["images"], targets):
        target.write_bytes(image["bytes"])
    document = {
        "count": parsed["count"],
        "index_offset": parsed["index_offset"],
        "reserved": parsed["reserved"],
        "has_name_index": False,
        "index_inside_file": parsed["index_inside_file"],
        "trailing_bytes": parsed["trailing_bytes"],
        "images": rows,
    }
    targets[-1].write_text(json.dumps(document, indent=2) + "\n")
    return targets[-1]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path, nargs="?")
    parser.add_argument("--list", action="store_true", help="print sizes and write nothing")
    args = parser.parse_args(argv)
    data = args.source.read_bytes()
    parsed = parse_xgim(data)
    if args.list:
        print(
            f"{args.source} count={parsed['count']} "
            f"index_offset={parsed['index_offset']} "
            f"index_inside_file={parsed['index_inside_file']} "
            f"trailing_bytes={parsed['trailing_bytes']}"
        )
        for row in manifest_rows(parsed):
            print(f"{row['file']} {row['width']}x{row['height']} {row['bytes']}")
        return 0
    if args.output is None:
        parser.error("output directory is required unless --list is set")
    path = write_images(parsed, args.output)
    print(f"wrote {parsed['count']} images and {path}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, XgimError) as error:
        print(f"extract_xgim: {error}", file=sys.stderr)
        sys.exit(1)
