"""Synthetic containment and integrity tests; no private input or device needed."""
import gzip
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import pod_farm_112_inventory as reader
from research_setup import unpack_odc


def cpio_entry(name, payload=b'', mode=0o100755):
    name = name.encode() + b'\0'
    fields = (0, 0, mode, 0, 0, 1, 0)
    return (b'070707' + b''.join(f'{x:06o}'.encode() for x in fields) +
            b'00000000000' + f'{len(name):06o}{len(payload):011o}'.encode() + name + payload)


class PrivateReaderGuards(unittest.TestCase):
    def test_wrong_source_stops_before_archive_tool_or_output(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'wrong.dmg'
            source.write_bytes(b'not the supplied original')
            output = Path(directory) / 'output'
            with patch.object(reader, 'seven') as tool:
                with self.assertRaisesRegex(ValueError, 'source size/type mismatch'):
                    reader.extract(source, Path('unused'), output)
                tool.assert_not_called()
            self.assertFalse(output.exists())

    def test_same_size_wrong_digest_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'wrong.dmg'
            source.write_bytes(b'fake')
            with patch.object(reader, 'SOURCE_SIZE', 4):
                with self.assertRaisesRegex(ValueError, 'source digest mismatch'):
                    reader.checked_source(source)

    def test_fresh_output_refuses_existing_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / 'exists'
            output.mkdir()
            (output / 'original').write_bytes(b'preserve')
            with self.assertRaisesRegex(ValueError, 'fresh output'):
                reader.fresh_output(output, root)
            self.assertEqual((output / 'original').read_bytes(), b'preserve')

    def test_output_escape_and_symlink_escape_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'analysis'
            root.mkdir()
            (root / 'outside').symlink_to(Path(directory), target_is_directory=True)
            for output in (root, root / '..' / 'escape', root / 'outside' / 'escape'):
                with self.subTest(output=output), self.assertRaisesRegex(ValueError, 'below ignored'):
                    reader.fresh_output(output, root)

    def test_bounded_inflate_and_truncated_gzip(self):
        with patch.object(reader, 'LIMIT', 8):
            self.assertEqual(reader.inflate(gzip.compress(b'12345678')), b'12345678')
            with self.assertRaisesRegex(ValueError, 'oversize CPIO'):
                reader.inflate(gzip.compress(b'123456789'))
        with self.assertRaises((EOFError, OSError)):
            reader.inflate(gzip.compress(b'data')[:-5])

    def test_cpio_never_restores_execute_or_materializes_link(self):
        data = (cpio_entry('regular', b'owned dummy data') +
                cpio_entry('link', b'/outside', 0o120777) + cpio_entry('TRAILER!!!'))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = unpack_odc(data, root)
            self.assertEqual(report['regular_files'], 1)
            self.assertEqual(report['unmaterialized_links'], [{'path': 'link', 'target': '/outside'}])
            self.assertFalse((root / 'regular').stat().st_mode & 0o111)
            self.assertFalse((root / 'link').exists())
            self.assertFalse((root / 'link').is_symlink())

    def test_cpio_escape_and_device_nodes_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            for data in (cpio_entry('../escape', b'x'), cpio_entry('node', b'', 0o020600)):
                with self.subTest(data=data), self.assertRaises(ValueError):
                    unpack_odc(data + cpio_entry('TRAILER!!!'), Path(directory))


if __name__ == '__main__':
    unittest.main()
