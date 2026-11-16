"""Owned synthetic checks for audited in-memory scalar projection facts."""
import struct
import unittest

from pod_farm_metadata_words import project_words


def record(tag, lower, upper, word134):
    result = bytearray(0x138)
    struct.pack_into('<4I', result, 0x128, tag, lower, upper, word134)
    return bytes(result)


class MetadataWords(unittest.TestCase):
    def test_signed_extension_and_current_separation(self):
        result = project_words(record(0, 0xffffffff, 7, 2), 5)
        self.assertEqual(result, {'scalar_tag': 0, 'flags118': 0x10000000,
                                 'word120': 0xffffffffffffffff, 'word128': 7,
                                 'word130_current': 5, 'word138_from134': 2})

    def test_unsigned_extension(self):
        result = project_words(record(0x10, 0x80000000, 0xffffffff, 0x80000001), 0xffffffff)
        self.assertEqual(result['flags118'], 0x20000000)
        self.assertEqual([result[k] for k in ['word120', 'word128', 'word138_from134']],
                         [0x80000000, 0xffffffff, 0x80000001])

    def test_float_words_are_copied_without_host_math(self):
        result = project_words(record(1, 0x80000000, 0x7fc01234, 0x3f000000), 0xffff00003f800000)
        self.assertEqual(result['flags118'], 0)
        self.assertEqual([result[k] for k in ['word120', 'word128', 'word138_from134', 'word130_current']],
                         [0x80000000, 0x7fc01234, 0x3f000000, 0x3f800000])

    def test_unknown_types_stay_unclassified(self):
        for tag in (2, 0xffffffff):
            with self.assertRaisesRegex(ValueError, 'unclassified'):
                project_words(record(tag, 0, 1, 0), 0)

    def test_truncated_input(self):
        for length in (0, 0x127, 0x137):
            with self.assertRaisesRegex(ValueError, 'truncated'):
                project_words(bytes(length), 0)

    def test_current_word_bound(self):
        for current in (-1, 1 << 64):
            with self.assertRaisesRegex(ValueError, 'u64'):
                project_words(record(0, 0, 1, 0), current)


if __name__ == '__main__':
    unittest.main()
