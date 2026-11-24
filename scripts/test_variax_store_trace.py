"""Formula checks for the Variax 3.10 store trace. No device and no firmware image."""

import unittest

from map_variax_store import (
    checksum_delta_for_zero_stamp,
    editor_store_indices,
    group_of,
    slot_address,
    spare_address,
    spare_sector,
)

# Quotients measured on the DSP56300 emulator for a typed divide-by-20 program
# whose assembled words at P:0x001A-0x0034 matched the archive. The test locks
# the formula to those measurements. It does not launch the emulator.
MEASURED_GROUPS = {
    0: 0, 1: 0, 19: 0, 20: 1, 21: 1, 39: 1, 40: 2, 59: 2,
    60: 3, 63: 3, 127: 6, 254: 12,
}


class StoreAddressTests(unittest.TestCase):
    def test_measured_groups_and_sector_addresses(self):
        for index, group in MEASURED_GROUPS.items():
            self.assertEqual(group_of(index), group)
            self.assertEqual(slot_address(index), 0xD00000 + index * 0x1000)
            self.assertEqual(spare_sector(index), 60 + group)
            self.assertEqual(spare_address(index), 0xD3C000 + group * 0x1000)

    def test_in_range_slots_use_three_distinct_spares(self):
        spares = {spare_address(index) for index in range(60)}
        self.assertEqual(spares, {0xD3C000, 0xD3D000, 0xD3E000})
        for index in range(60):
            self.assertNotEqual(slot_address(index), spare_address(index))
            self.assertLessEqual(slot_address(index), 0xD3B000)

    def test_only_index_63_aliases_its_spare_below_256(self):
        aliased = [index for index in range(256) if slot_address(index) == spare_address(index)]
        self.assertEqual(aliased, [63])

    def test_editor_formula_covers_every_user_slot_once(self):
        indices = editor_store_indices()
        self.assertEqual(sorted(indices), list(range(60)))
        self.assertEqual(len(indices), 60)
        # Within a bank the flash index runs opposite the Workbench slot number.
        self.assertEqual(indices[0], 4)
        self.assertEqual(indices[4], 0)

    def test_program_number_is_not_the_store_index(self):
        def program_number(bank, slot):
            base = bank * 5 + (4 - slot)
            return base if bank in (0, 11) else base + 55

        middle = [program_number(bank, slot) for bank in range(1, 11) for slot in range(5)]
        self.assertTrue(all(index >= 60 for index in middle))
        self.assertTrue(set(middle).isdisjoint(editor_store_indices()))

    def test_zero_stamp_raises_the_firmware_checksum_by_0xc4(self):
        words = [0] * 1092
        words[1091] = 0
        self.assertEqual(checksum_delta_for_zero_stamp(), 0xC4)
        stamped = words.copy()
        stamped[767:771] = [0x30, 0x33, 0x31, 0x30]
        host = sum(words[:1089]) & 0xFFFFFF
        firmware = sum(stamped[:1091]) & 0xFFFFFF
        self.assertEqual(firmware, (host + 0xC4) & 0xFFFFFF)


if __name__ == "__main__":
    unittest.main()
