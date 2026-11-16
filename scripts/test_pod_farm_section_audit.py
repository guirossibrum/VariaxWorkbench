"""Synthetic adversarial framing checks; no original/vendor fixtures."""
import struct
import unittest

from pod_farm_section_audit import audit_mapping, audit_settings


def mapping(count=1, version=2):
    return struct.pack('>4I', 0x4c364d45, version, count, 0) + bytes(104)


def settings(order='<', count=1, total=44):
    return struct.pack(order+'11I', 0x36414d31, 20, 2, total, count,
                       7, 8, 9, 10, 11, 12)


class SectionAudit(unittest.TestCase):
    def test_mapping_records_reserved_and_tail(self):
        result = audit_mapping(mapping()+b'unknown')
        self.assertEqual(result, {'reserved': 0, 'records': (bytes(104),), 'tail': b'unknown'})

    def test_mapping_every_truncation_rejected(self):
        for n in range(120):
            with self.subTest(n=n), self.assertRaises(ValueError):
                audit_mapping(mapping()[:n])

    def test_mapping_count_cannot_wrap_or_return_partial_records(self):
        for count in (2, 0xffffffff):
            with self.subTest(count=count), self.assertRaises(ValueError):
                audit_mapping(mapping(count=count))

    def test_mapping_native_version_one_shortcut_not_audit_support(self):
        with self.assertRaises(ValueError):
            audit_mapping(mapping(version=1))

    def test_settings_both_orders_and_tail(self):
        expected = {'records': ((7, 8, 9, 10, 11, 12),), 'tail': b'tail'}
        for order in ('<', '>'):
            self.assertEqual(audit_settings(settings(order)+b'tail'), expected)

    def test_settings_every_truncation_rejected(self):
        for n in range(44):
            with self.subTest(n=n), self.assertRaises(ValueError):
                audit_settings(settings()[:n])

    def test_settings_count_length_and_wrap_rejected(self):
        for count, total in ((2, 44), (0xffffffff, 0xfffffffc), (0, 44)):
            with self.subTest(count=count), self.assertRaises(ValueError):
                audit_settings(settings(count=count, total=total))

    def test_settings_zero_count_and_unknown_version(self):
        self.assertEqual(audit_settings(settings(count=0, total=20)[:20])['records'], ())
        data = bytearray(settings())
        struct.pack_into('<I', data, 8, 3)
        with self.assertRaises(ValueError):
            audit_settings(data)


if __name__ == '__main__':
    unittest.main()
