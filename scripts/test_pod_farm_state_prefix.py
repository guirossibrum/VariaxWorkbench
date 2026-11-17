"""Owned synthetic state-prefix/wrapper checks; never load a plugin."""
import struct
import unittest

from pod_farm_state_prefix import read_state_prefix, vst_restore_status


def synthetic_prefix(order='<', header_length=244, total=900, first=(244, 100)):
    words = [0x36504658, header_length, total, 123, 17, 19,
             *first, 344, 80, 424, 12, 436, 20]+list(range(13))
    name = b'owned synthetic name\0'.ljust(128, b'\0')
    return struct.pack(order+'27I', *words)+name+struct.pack(order+'2I', 3, 7)


class StatePrefix(unittest.TestCase):
    def test_little_endian_fields_and_sections(self):
        result = read_state_prefix(synthetic_prefix())
        self.assertEqual(result['header_length'], 244)
        self.assertEqual(result['advertised_total'], 900)
        self.assertEqual(result['section_words'], ((244, 100), (344, 80), (424, 12), (436, 20)))
        self.assertEqual(result['final_words'], (3, 7))

    def test_swapped_scalar_words_leave_name_bytes_unchanged(self):
        self.assertEqual(read_state_prefix(synthetic_prefix('<')),
                         read_state_prefix(synthetic_prefix('>')))

    def test_short_read_rejected(self):
        for length in (0, 4, 243):
            with self.subTest(length=length), self.assertRaises(ValueError):
                read_state_prefix(synthetic_prefix()[:length])

    def test_signature_is_not_a_form_container(self):
        with self.assertRaisesRegex(ValueError, 'signature'):
            read_state_prefix(b'FORM'+synthetic_prefix()[4:])

    def test_minimum_declared_header(self):
        with self.assertRaisesRegex(ValueError, 'header'):
            read_state_prefix(synthetic_prefix(header_length=243))
        self.assertEqual(read_state_prefix(synthetic_prefix(header_length=512))['header_length'], 512)

    def test_prefix_success_does_not_validate_advertised_total(self):
        result = read_state_prefix(synthetic_prefix(total=0xffffffff))
        self.assertEqual(result['advertised_total'], 0xffffffff)

    def test_offsets_are_raw_and_trailing_bytes_not_preservation(self):
        data = synthetic_prefix(first=(0xffffffff, 0x7fffffff))
        self.assertEqual(read_state_prefix(data), read_state_prefix(data+b'opaque trailing data'))
        self.assertEqual(read_state_prefix(data)['section_words'][0], (0xffffffff, 0x7fffffff))

    def test_vst_wrapper_sign_bit_status(self):
        for result, expected in [(0, 1), (0x1003, 1), (0x7fffffff, 1),
                                 (0x80000000, 0), (0xffffffff, 0)]:
            with self.subTest(result=result):
                self.assertEqual(vst_restore_status(result), expected)


if __name__ == '__main__':
    unittest.main()
