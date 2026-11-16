"""Owned reconstruction of the statically recovered tone conversion classifier.

The result is an internal dispatch class, not a device identity, acceptance test,
converter, parameter-domain definition or native oracle. Inputs represent u32
words from metadata. This module never loads vendor code or contacts a device.
"""


def conversion_class(product, version):
    """Project the product/version thresholds at AU VA 0x19a242."""
    product &= 0xffffffff
    version &= 0xffffffff
    if product in (0x300, 0x400, 0x500, 0x600, 0x80000):
        return 0
    if product in (0x30002, 0x30005, 0x3000a):
        for threshold, result in ((0x1060000, 1), (0x2100000, 2), (0x2900000, 3)):
            if version < threshold:
                return result
        return 4
    if product == 0x80001 and version < 0x2490000:
        return 1
    if 0x80001 <= product <= 0x8000e or product == 0xfffffff0:
        return 2 if version < 0x2530000 else 4 if version < 0x2540000 else 6
    if 0xc0000 <= product <= 0xc0002:
        return 5
    return {0x130001: 7, 0x130011: 8}.get(product, -1)
