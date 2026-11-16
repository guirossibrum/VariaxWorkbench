"""Synthetic file-reader guards; no vendor or device inputs required."""
import struct
import unittest

from pod_farm_inventory import iff_chunks, macho


def chunk(tag, payload):
    return tag + struct.pack('>I', len(payload)) + payload + b'\0'*(len(payload) & 1)


def form(kind, body):
    return chunk(b'FORM', kind+body)


def thin(cpu=7, commands=b'', count=0):
    bits = cpu == 0x1000007
    return struct.pack('<7I', 0xfeedfacf if bits else 0xfeedface,
                       cpu, 3, 8, count, len(commands), 0) + (b'\0'*4 if bits else b'') + commands


class InventoryGuards(unittest.TestCase):
    def test_thin_architectures(self):
        for cpu, bits in [(7, 32), (0x1000007, 64)]:
            self.assertEqual((macho(thin(cpu))[0]['cpu'], macho(thin(cpu))[0]['bits']), (cpu, bits))

    def test_fat_architectures(self):
        a, b = thin(), thin(0x1000007)
        data = bytes.fromhex('cafebabe')+struct.pack('>I', 2)
        data += struct.pack('>5I', 7, 3, 48, len(a), 0)
        data += struct.pack('>5I', 0x1000007, 3, 48+len(a), len(b), 0)
        self.assertEqual([s['bits'] for s in macho(data+a+b)], [32, 64])

    def test_fat_bounds_and_cpu(self):
        for cpu, offset, size in [(7, 1000, 28), (18, 28, 28), (7, 8, 28)]:
            data = bytes.fromhex('cafebabe')+struct.pack('>6I', 1, cpu, 3, offset, size, 0)+thin()
            with self.assertRaises(ValueError):
                macho(data)

    def test_short_load_commands(self):
        for cmd in [struct.pack('<II', 12, 0), struct.pack('<II', 12, 100)]:
            with self.assertRaises(ValueError):
                macho(thin(commands=cmd, count=1))

    def test_import_and_signature_presence(self):
        cmd = struct.pack('<6I', 12, 32, 24, 0, 0, 0)+b'libX\0\0\0\0'
        signature = struct.pack('<4I', 0x1d, 16, 0, 0)
        record = macho(thin(commands=cmd+signature, count=2))[0]
        self.assertEqual(record['imports'], ['libX'])
        self.assertTrue(record['has_code_signature_command'])

    def test_symbol_bounds(self):
        cmd = struct.pack('<6I', 2, 24, 1000, 1, 2000, 10)
        with self.assertRaises(ValueError):
            macho(thin(commands=cmd, count=1))

    def test_pinf_words_are_opaque(self):
        body = chunk(b'PINF', struct.pack('>II', 3, 0x130001)+b'opaque')
        data = form(b'L6PA', body)
        result = iff_chunks(data)
        self.assertEqual(result['chunks'][0]['target_word'], 0x130001)
        self.assertEqual(set(result['chunks'][0]), {'tag', 'size', 'version_word', 'target_word'})

    def test_unknown_chunk_and_nested_container(self):
        data = form(b'L6P2', chunk(b'LIST', b'BANK'+form(b'L6PA', chunk(b'UNKN', b'abc'))))
        node = iff_chunks(data)['chunks'][0]['children'][0]['children'][0]
        self.assertEqual(node, {'tag': 'UNKN', 'size': 3})

    def test_truncation_and_trailing_bytes(self):
        data = form(b'L6PA', chunk(b'UNKN', b'abc'))
        for broken in [data[:-1], data+b'\0', form(b'L6PA', b'x'), form(b'L6PA', b'UNKN'+struct.pack('>I', 99))]:
            with self.assertRaises(ValueError):
                iff_chunks(broken)

    def test_nesting_limit(self):
        data = chunk(b'UNKN', b'')
        for _ in range(18):
            data = form(b'L6P2', data)
        with self.assertRaises(ValueError):
            iff_chunks(data)


if __name__ == '__main__':
    unittest.main()
