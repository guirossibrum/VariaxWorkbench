"""Offline synthetic checks for the bounded #311 Windows amp trace."""

import struct
import unittest
import zlib


def symbol_records(data):
    """Validate the extent, names and checksums and preserve record order."""
    if len(data) < 8:
        raise ValueError("short symbol header")
    count, length = struct.unpack_from("<II", data)
    pool_start = 8 + count * 12
    if length != len(data) or pool_start > length:
        raise ValueError("invalid symbol table extent")

    records = []
    for offset, crc, size in struct.iter_unpack("<III", data[8:pool_start]):
        start = pool_start + offset
        name = data[start:start + size]
        if start + size >= length or data[start + size] != 0 or b"\0" in name:
            raise ValueError("invalid symbol name extent")
        if zlib.crc32(name) != crc:
            raise ValueError("invalid symbol checksum")
        records.append((name, crc))
    return records


def symbol_index(records, name):
    crc = zlib.crc32(name)
    for index, (candidate, checksum) in enumerate(records):
        if checksum == crc and candidate == name:
            return index
    raise ValueError(f"missing symbol {name!r}")


def permits_product(record, product):
    """Model the observed catalogue filter for bare numeric entries only."""
    included = record.get("devices", ())
    excluded = record.get("exclude_devices", ())
    return (not included or product in included) and product not in excluded


def amp_records(records, group, model, control, value):
    """Emit the traced TRTG group records for one already-normalized scalar."""
    return b"".join((
        struct.pack("<IHI", 0x80000003, 7, symbol_index(records, group)),
        struct.pack("<IHI", 0x80000005, 7, symbol_index(records, model)),
        struct.pack("<IHI", 0x80000006, 2, 1),
        struct.pack("<IHf", symbol_index(records, control + b"State"), 3, value),
        struct.pack("<IH", 0x80000004, 0),
    ))


def fixture_table(*names):
    headers = bytearray()
    pool = bytearray()
    for name in names:
        headers.extend(struct.pack("<III", len(pool), zlib.crc32(name), len(name)))
        pool.extend(name + b"\0")
    return struct.pack("<II", len(names), 8 + len(headers) + len(pool)) + headers + pool


class SyntheticAmpTrace(unittest.TestCase):
    def setUp(self):
        self.data = fixture_table(b"TestGroup", b"TestDial", b"TestDialState", b"TestModel")
        self.records = symbol_records(self.data)

    def test_exact_name_survives_crc_collision(self):
        self.assertEqual(zlib.crc32(b"plumless"), zlib.crc32(b"buckeroo"))
        records = symbol_records(fixture_table(b"plumless", b"buckeroo"))
        self.assertEqual(symbol_index(records, b"buckeroo"), 1)

    def test_wrong_case_is_missing(self):
        with self.assertRaisesRegex(ValueError, "missing symbol"):
            symbol_index(self.records, b"testmodel")

    def test_bad_crc_is_rejected(self):
        data = bytearray(self.data)
        data[12] ^= 1
        with self.assertRaisesRegex(ValueError, "checksum"):
            symbol_records(data)

    def test_unterminated_name_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "name extent"):
            symbol_records(self.data[:-1] + b"!")

    def test_product_filter(self):
        self.assertTrue(permits_product({}, 17))
        self.assertTrue(permits_product({"devices": []}, 17))
        self.assertTrue(permits_product({"devices": [17]}, 17))
        self.assertFalse(permits_product({"devices": [18]}, 17))
        self.assertFalse(permits_product({"devices": [17], "exclude_devices": [17]}, 17))

    def test_display_symbol_alone_cannot_supply_state_key(self):
        records = symbol_records(fixture_table(b"TestGroup", b"TestDial", b"TestModel"))
        with self.assertRaisesRegex(ValueError, "TestDialState"):
            amp_records(records, b"TestGroup", b"TestModel", b"TestDial", 0.625)

    def test_one_scalar_group_bytes(self):
        self.assertEqual(
            amp_records(self.records, b"TestGroup", b"TestModel", b"TestDial", 0.625),
            bytes.fromhex(
                "03 00 00 80 07 00 00 00 00 00 "
                "05 00 00 80 07 00 03 00 00 00 "
                "06 00 00 80 02 00 01 00 00 00 "
                "02 00 00 00 03 00 00 00 20 3f "
                "04 00 00 80 00 00"
            ),
        )


if __name__ == "__main__":
    unittest.main()
