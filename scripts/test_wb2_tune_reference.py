"""Synthetic boundary checks for wb2_tune_reference; no vendor payload used."""
import struct
import unittest

from wb2_tune_reference import decode_tune, encode_tune, set_tune_reference


def chunk(tag, payload):
    return tag + struct.pack('>I', len(payload)) + payload + (b'\0' if len(payload) & 1 else b'')


class TuneReferenceTests(unittest.TestCase):
    def test_writer_round_trip_six_values(self):
        values = [-12.0, -7.5, -2.0, 3.0, 7.25, 12.0]
        name, decoded = decode_tune(encode_tune(b'synthetic', values))
        self.assertEqual(name, b'synthetic')
        self.assertEqual([decoded[i] for i in range(6)], values)

    def test_truncated_header_and_chunk_lengths_rejected(self):
        good = encode_tune(b'x', [0.0] * 6)
        for bad in (good[:7], good[:-1],
                    chunk(b'LIST', b'TUNE' + b'THDR' + struct.pack('>I', 20) + b'x')):
            with self.subTest(length=len(bad)), self.assertRaises(ValueError):
                decode_tune(bad)

    def test_missing_or_short_header_rejected(self):
        for payload in (b'TUNE', b'TUNE' + chunk(b'THDR', b'abc')):
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                decode_tune(chunk(b'LIST', payload))

    def test_strg_size_mismatch_rejected(self):
        payload = b'TUNE' + chunk(b'THDR', b'\0\0\0\0n\0') + chunk(b'STRG', b'1234567')
        with self.assertRaisesRegex(ValueError, 'eight bytes'):
            decode_tune(chunk(b'LIST', payload))

    def test_unknown_children_rejected_and_out_of_range_index_skips(self):
        children = chunk(b'THDR', b'\0\0\0\0n\0')
        children += chunk(b'UNKN', b'opaque')
        with self.assertRaisesRegex(ValueError, 'unknown'):
            decode_tune(chunk(b'LIST', b'TUNE' + children))
        children = chunk(b'THDR', b'\0\0\0\0n\0')
        children += chunk(b'STRG', bytes((6, 0, 0, 0)) + struct.pack('>f', 4.0))
        children += chunk(b'STRG', bytes((2, 0, 0, 0)) + struct.pack('>f', 1.5))
        children += chunk(b'STRG', bytes((2, 9, 8, 7)) + struct.pack('>f', 2.5))
        _, values = decode_tune(chunk(b'LIST', b'TUNE' + children))
        self.assertEqual(values, {2: 2.5})

    def test_index_count_and_type_errors(self):
        with self.assertRaisesRegex(ValueError, 'six string records'):
            encode_tune(b'x', [0.0] * 5)
        self.assertEqual(set_tune_reference(5, 1.0), 0)
        self.assertEqual(set_tune_reference(6, 1.0), 0x80000002)
        self.assertEqual(set_tune_reference(5, 1.0, count=6), 0x80000008)
        self.assertEqual(set_tune_reference(0, 1.0, type_tag=0), 0x80000009)


if __name__ == '__main__':
    unittest.main(verbosity=2)
