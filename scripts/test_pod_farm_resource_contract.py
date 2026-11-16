"""Synthetic resource guards and PINF reconstruction checks, no original code."""
import struct
import unittest

from pod_farm_resource_contract import (
    system_definitions, pinf_read_projection, pinf_write_projection,
    registry_integer_projection, dual_tone_selection,
)


def chunk(tag, payload):
    return tag+struct.pack('>I', len(payload))+payload+b'\0'*(len(payload) & 1)


def resource(model=0x3e700003, identifier=0x3f20002a, short=False):
    header = struct.pack('>I', model)+bytes(0x98)
    obj = bytearray(0x114)
    struct.pack_into('>II', obj, 0x54, identifier, 0)
    struct.pack_into('>iii', obj, 0x7c, 0, 1, 1)
    body = chunk(b'LOUT', header)+chunk(b'UNKN', b'x')+chunk(b'LOBJ', obj[:20] if short else obj)
    return chunk(b'FORM', b'L6RB'+chunk(b'LIST', b'LAYO'+chunk(b'LIST', b'LOUT'+body)))


class ResourceContract(unittest.TestCase):
    def test_explicit_model_and_parameter_pair(self):
        data = resource()
        result = system_definitions(data)
        self.assertEqual(len(result), 1)
        self.assertEqual((result[0]['scalar_tag58'], result[0]['lower'], result[0]['upper'],
                          result[0]['metadata_word84']), (0, 0, 1, 1))

    def test_wrong_model_or_identifier_is_not_substituted(self):
        self.assertEqual(system_definitions(resource(model=0x1234)), [])
        self.assertEqual(system_definitions(resource(identifier=0x3f000003)), [])

    def test_truncated_resource(self):
        with self.assertRaises(ValueError):
            system_definitions(resource()[:-1])
        with self.assertRaisesRegex(ValueError, 'layout object'):
            system_definitions(resource(short=True))

    def test_short_list_type_rejected(self):
        with self.assertRaisesRegex(ValueError, 'LIST type'):
            system_definitions(chunk(b'FORM', b'L6RB'+chunk(b'LIST', b'x')))

    def test_opaque_odd_chunk_does_not_change_selected_definition(self):
        data = resource()
        before = bytes(data)
        self.assertEqual(system_definitions(data)[0]['identifier'], 0x3f20002a)
        self.assertEqual(data, before)

    def test_pinf_extension_has_no_projected_field(self):
        data = struct.pack('>II', 3, 0x130001)+bytes(64)+struct.pack('>I', 1)
        projection = pinf_read_projection(data+b'unknown tail')
        self.assertEqual(pinf_write_projection(projection, 999), data)
        self.assertNotEqual(pinf_write_projection(projection, 999), data+b'unknown tail')

    def test_pinf_last_name_unit_is_terminated(self):
        data = struct.pack('>II32HI', 3, 0x130001, *([65]*32), 1)
        result = pinf_read_projection(data)
        self.assertEqual(result['name_units'], [65]*31)
        self.assertEqual(pinf_write_projection(result, 2)[70:72], b'\0\0')

    def test_pinf_embedded_nul_and_short_prefix(self):
        data = struct.pack('>II32HI', 3, 0x130001, 65, 0, *([66]*30), 1)
        result = pinf_read_projection(data)
        self.assertEqual(result['name_units'], [65])
        self.assertEqual(pinf_write_projection(result, 2)[10:72], bytes(62))
        self.assertEqual(pinf_read_projection(b'')['name_units'], [])

    def test_pinf_zero_version_uses_explicit_synthetic_context(self):
        result = pinf_read_projection(bytes(76))
        self.assertEqual(pinf_write_projection(result, 0x12345678)[72:], bytes.fromhex('12345678'))


class RemainingContracts(unittest.TestCase):
    def test_signed_clamp_extremes(self):
        for word, expected in [(0x80000000, 0), (0xffffffff, 0), (0, 0), (1, 1),
                               (2, 1), (0x7fffffff, 1)]:
            with self.subTest(word=word):
                self.assertEqual(registry_integer_projection(word, 0, 1), expected)

    def test_low_word_is_not_a_64_bit_domain(self):
        self.assertEqual(registry_integer_projection(0x100000001, 0, 1), 1)
        self.assertEqual(registry_integer_projection(0x1ffffffff, 0, 1), 0)

    def test_signed_registry_endpoints(self):
        self.assertEqual(registry_integer_projection(0xfffffffb, -3, 7), 0xfffffffd)
        self.assertEqual(registry_integer_projection(4, -3, 7), 4)

    def test_registry_inverted_endpoints_differ_from_scalar_helper(self):
        self.assertEqual(registry_integer_projection(4, 9, 2), 2)

    @staticmethod
    def tone_file(body, head=b'\0\0\0\1', extra=b''):
        return chunk(b'FORM', b'L6P2'+chunk(b'HEAD', head)+extra+
                     chunk(b'LIST', b'BANK'+body))

    def test_two_forms_selected_in_order_past_opaque_children(self):
        first = chunk(b'FORM', b'L6PA')
        second = chunk(b'FORM', b'L6AS')
        data = self.tone_file(chunk(b'UNKN', b'xx')+first+chunk(b'BINF', b'')+second)
        spans = dual_tone_selection(data)['patch_spans']
        self.assertEqual([data[o:o+n] for o, n in spans], [first, second])

    def test_first_head_first_bank_and_first_two_forms(self):
        body = chunk(b'BINF', bytes(68))+chunk(b'FORM', b'L6PA')*3
        data = self.tone_file(body, extra=chunk(b'HEAD', b'\0\0\0\2')+
                              chunk(b'LIST', b'BANK'+body))
        result = dual_tone_selection(data)
        self.assertEqual(result['head_word'], 1)
        self.assertEqual(len(result['patch_spans']), 2)
        self.assertTrue(all(o < len(data)//2 for o, n in result['patch_spans']))

    def test_required_children_and_strict_truncation(self):
        with self.assertRaises(ValueError):
            dual_tone_selection(self.tone_file(chunk(b'BINF', bytes(68))))
        with self.assertRaises(ValueError):
            dual_tone_selection(self.tone_file(chunk(b'FORM', b'L6PA')*2))
        with self.assertRaises(ValueError):
            dual_tone_selection(self.tone_file(b'', head=b'x'))

    def test_child_type_selection_is_not_patch_acceptance(self):
        data = self.tone_file(chunk(b'BINF', b'')+chunk(b'FORM', b'FAKE')*2)
        self.assertEqual(len(dual_tone_selection(data)['patch_spans']), 2)
        with self.assertRaises(ValueError):
            dual_tone_selection(data[:-1])


if __name__ == '__main__':
    unittest.main()
