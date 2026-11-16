"""Synthetic resource guards and PINF reconstruction checks, no original code."""
import struct
import unittest

from pod_farm_resource_contract import system_definitions, pinf_read_projection, pinf_write_projection


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


if __name__ == '__main__':
    unittest.main()
