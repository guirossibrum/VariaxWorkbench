"""Synthetic source-rule checks; not native/original-state oracle tests."""
import unittest

from pod_farm_legacy_scalar import (clamp_integer_byte, decode_boolean,
    decode_permutation, decode_three_level, encode_boolean, encode_permutation,
    encode_three_level, legacy_category)


class LegacyScalar(unittest.TestCase):
    def test_signed_clamp_edges(self):
        for word, expected in ((0, 0), (1, 1), (127, 127), (128, 127),
                               (0x7fffffff, 127), (0x80000000, 0), (-1, 0),
                               (0x100000080, 127)):
            self.assertEqual(clamp_integer_byte(word), expected)

    def test_permutation_all_bytes_and_canonical_roundtrip(self):
        expected = [2, 3, 1] + [0]*253
        self.assertEqual([decode_permutation(x) for x in range(256)], expected)
        for word in range(4):
            self.assertEqual(decode_permutation(encode_permutation(word)), word)

    def test_permutation_outside_canonical_words_is_lossy(self):
        for word, byte in ((-1, 3), (0x80000000, 3), (4, 1), (0x7fffffff, 1)):
            self.assertEqual(encode_permutation(word), byte)
        self.assertEqual(decode_permutation(255), 0)

    def test_three_level_all_bytes_and_canonical_roundtrip(self):
        self.assertEqual([decode_three_level(x) for x in range(256)], [0]*42+[1]*44+[2]*170)
        self.assertEqual([encode_three_level(x) for x in range(3)], [0, 64, 127])
        for word in range(3):
            self.assertEqual(decode_three_level(encode_three_level(word)), word)

    def test_three_level_shift_wraps_before_signed_clamp(self):
        self.assertEqual(encode_three_level(-1), 0)
        self.assertEqual(encode_three_level(0x02000000), 0)
        self.assertEqual(encode_three_level(0x04000000), 0)
        self.assertEqual(encode_three_level(0x04000001), 64)

    def test_boolean_all_bytes_and_nonzero_words(self):
        self.assertEqual([decode_boolean(x) for x in range(256)], [0]*64+[1]*192)
        for word, expected in ((0, 0), (-1, 127), (1, 127), (2, 127), (0x100000000, 0)):
            self.assertEqual(encode_boolean(word), expected)

    def test_legacy_bank_category_boundary_neighbors(self):
        for bank, category in ((0, 5), (63, 5), (64, 3), (127, 3), (128, 2),
                               (191, 2), (192, 5), (255, 5), (256, 3), (319, 3),
                               (320, 2), (383, 2), (384, 5)):
            self.assertEqual(legacy_category(170, bank//128, bank%128), 0x400000+category)
            self.assertEqual(legacy_category(168, bank//128, bank%128), 0x400005)

    def test_legacy_raw_bytes_and_helper_precondition(self):
        self.assertEqual(legacy_category(170, 0, 192), 0x400005)
        self.assertEqual(legacy_category(170, 255, 255), 0x400005)
        with self.assertRaises(ValueError):
            legacy_category(169, 0, 0)


if __name__ == '__main__':
    unittest.main()
