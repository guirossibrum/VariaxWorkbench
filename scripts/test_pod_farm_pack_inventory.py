"""Synthetic strict framing checks; never decide native eligibility/licensing."""
import struct
import unittest

from pod_farm_pack_inventory import audit_asset_header


def chunk(tag, payload):
    return tag+struct.pack('>I', len(payload))+payload+b'\0'*(len(payload) & 1)


def form(body):
    return chunk(b'FORM', b'L6RB'+body)


def header(size=56):
    return chunk(b'AINF', (struct.pack('>4I', 17, 2, 3, 19)+bytes(40))[:size].ljust(size, b'!'))


class PackInventory(unittest.TestCase):
    def test_header_words_do_not_establish_installed_or_authorized(self):
        result = audit_asset_header(form(header()))
        self.assertEqual((result['asset_id'], result['asset_version'], result['eligibility_word']), (17, 2, 19))
        self.assertIsNone(result['installed_state'])
        self.assertIsNone(result['authorized_state'])

    def test_every_truncated_frame_rejected(self):
        data = form(header())
        for length in range(len(data)):
            with self.subTest(length=length), self.assertRaises(ValueError):
                audit_asset_header(data[:length])

    def test_signature_type_and_outer_length(self):
        for data in (b'RIFF'+form(header())[4:], form(header())[:8]+b'L6PA'+form(header())[12:], form(header())+b'tail'):
            with self.assertRaises(ValueError):
                audit_asset_header(data)

    def test_duplicate_and_missing_header_are_audit_restrictions(self):
        for data in (form(b''), form(header()+header())):
            with self.assertRaises(ValueError):
                audit_asset_header(data)

    def test_short_or_extended_header_not_native_rejection_claim(self):
        for size in (0, 16, 55, 57, 80):
            with self.subTest(size=size), self.assertRaises(ValueError):
                audit_asset_header(form(header(size)))

    def test_unknown_odd_chunk_changes_only_header_position(self):
        data = form(chunk(b'OWND', b'odd')+header())
        result = audit_asset_header(data)
        self.assertEqual(result['AINF_offset'], 32)
        self.assertEqual(result['asset_id'], 17)


if __name__ == '__main__':
    unittest.main()
