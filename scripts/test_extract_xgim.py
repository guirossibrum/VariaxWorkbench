"""Synthetic XGIM containers only. No vendor file and no device access."""

import struct
import tempfile
import unittest
import zlib
from pathlib import Path

from extract_xgim import XgimError, main, parse_xgim, write_images


def chunk(tag, data):
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def png(width, height, rgb):
    raw = b"".join(b"\x00" + bytes(rgb) * width for _ in range(height))
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


def container(images, index_offset=None, reserved=0, indexed=False):
    body = b"".join(images)
    if indexed:
        index_offset = 16 + len(body)
        entries = b""
        cursor = 16
        for number, image in enumerate(images, start=1):
            entries += struct.pack(">IIIII", 0x00010000 + number, cursor, len(image), 1, 1)
            cursor += len(image)
        body += entries
    elif index_offset is None:
        index_offset = 16 + len(body) + 64
    return b"XGIM" + struct.pack(">III", index_offset, len(images), reserved) + body


class ExtractXgimTests(unittest.TestCase):
    def test_split_counts_sizes_and_reports_a_missing_name_index(self):
        images = [png(2, 3, (10, 20, 30)), png(4, 1, (1, 2, 3))]
        parsed = parse_xgim(container(images, reserved=0))
        self.assertEqual(parsed["count"], 2)
        self.assertFalse(parsed["has_name_index"])
        self.assertFalse(parsed["index_inside_file"])
        self.assertEqual([(image["width"], image["height"]) for image in parsed["images"]], [(2, 3), (4, 1)])
        self.assertEqual(parsed["images"][0]["bytes"], images[0])

    def test_count_mismatch_and_bad_magic_fail(self):
        images = [png(1, 1, (0, 0, 0))]
        bad_count = b"XGIM" + struct.pack(">III", 80, 2, 0) + images[0]
        with self.assertRaises(XgimError):
            parse_xgim(bad_count)
        with self.assertRaises(XgimError):
            parse_xgim(b"PNG!" + images[0])

    def test_write_refuses_to_replace_an_existing_image(self):
        parsed = parse_xgim(container([png(1, 1, (9, 9, 9))]))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            write_images(parsed, output)
            self.assertTrue((output / "0000.png").is_file())
            self.assertIn('"has_name_index": false', (output / "manifest.json").read_text())
            with self.assertRaises(XgimError):
                write_images(parsed, output)

    def test_numeric_index_is_checked_and_is_not_a_name(self):
        images = [png(2, 2, (1, 1, 1)), png(3, 1, (2, 2, 2))]
        parsed = parse_xgim(container(images, indexed=True))
        self.assertTrue(parsed["index_inside_file"])
        self.assertFalse(parsed["has_name_index"])
        self.assertEqual([image["id"] for image in parsed["images"]], [0x00010001, 0x00010002])
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            write_images(parsed, output)
            self.assertTrue((output / "0000-00010001.png").is_file())
            self.assertTrue((output / "0001-00010002.png").is_file())

    def test_list_writes_nothing(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "sample.imb"
            source.write_bytes(container([png(1, 2, (4, 5, 6))]))
            before = set(Path(directory).iterdir())
            self.assertEqual(main([str(source), "--list"]), 0)
            self.assertEqual(set(Path(directory).iterdir()), before)


if __name__ == "__main__":
    unittest.main()
