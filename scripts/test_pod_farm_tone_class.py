"""Owned threshold reconstruction checks, not original-library comparisons."""
import unittest

from pod_farm_tone_class import conversion_class


class ToneClass(unittest.TestCase):
    def test_legacy_constants_ignore_version(self):
        for product in (0x300, 0x400, 0x500, 0x600, 0x80000):
            for version in (0, 0xffffffff):
                self.assertEqual(conversion_class(product, version), 0)

    def test_three_threshold_products_boundary_neighbors(self):
        for product in (0x30002, 0x30005, 0x3000a):
            for boundary, before, after in ((0x1060000, 1, 2), (0x2100000, 2, 3),
                                            (0x2900000, 3, 4)):
                self.assertEqual(conversion_class(product, boundary-1), before)
                self.assertEqual(conversion_class(product, boundary), after)

    def test_special_80001_early_threshold(self):
        self.assertEqual(conversion_class(0x80001, 0x248ffff), 1)
        self.assertEqual(conversion_class(0x80001, 0x2490000), 2)
        self.assertEqual(conversion_class(0x80002, 0x248ffff), 2)

    def test_host_range_and_sentinel_thresholds(self):
        for product in (*range(0x80001, 0x8000f), 0xfffffff0):
            for version, expected in ((0x252ffff, 2), (0x2530000, 4),
                                      (0x253ffff, 4), (0x2540000, 6)):
                self.assertEqual(conversion_class(product, version), expected)

    def test_fixed_classes_and_unknown_neighbors(self):
        for product, expected in ((0xc0000, 5), (0xc0001, 5), (0xc0002, 5),
                                  (0x130001, 7), (0x130011, 8), (0x30003, -1),
                                  (0x8000f, -1), (0xc0003, -1), (0x130002, -1), (0, -1)):
            self.assertEqual(conversion_class(product, 0xffffffff), expected)

    def test_u32_words_are_not_python_signed_order(self):
        self.assertEqual(conversion_class(-16, 0), 2)
        self.assertEqual(conversion_class(0x80001, -1), 6)
        self.assertEqual(conversion_class(0x100080001, 0x102530000), 4)


if __name__ == '__main__':
    unittest.main()
