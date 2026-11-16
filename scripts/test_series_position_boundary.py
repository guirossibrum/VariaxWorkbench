"""Vendor-free packed-output guard regression; no oracle imports or execution."""
import unittest

from audit_series_position_boundary import words


class PackedWordsTests(unittest.TestCase):
    def test_full_length_with_matching_checksum(self):
        packed = bytearray(3276)
        packed[:3] = packed[-3:] = bytes.fromhex('123456')
        decoded = words(packed)
        self.assertEqual(len(decoded), 1092)
        self.assertEqual(decoded[0], 0x123456)
        self.assertEqual(decoded[-1], 0x123456)

    def test_checksum_matching_truncations_are_rejected(self):
        # All-zero shortened buffers passed the old word-count/checksum guard:
        # range(0, length, 3) still decodes 1092 entries for both lengths.
        for length in (3274, 3275):
            with self.subTest(length=length):
                with self.assertRaisesRegex(ValueError, 'length'):
                    words(bytes(length))

    def test_full_length_bad_checksum_is_rejected(self):
        packed = bytearray(3276)
        packed[-1] = 1
        with self.assertRaisesRegex(ValueError, 'checksum'):
            words(packed)


if __name__ == '__main__':
    unittest.main()
