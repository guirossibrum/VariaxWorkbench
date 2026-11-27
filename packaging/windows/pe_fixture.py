"""Synthetic x64 Windows import table for offline packaging tests."""

import struct


def executable(dll=b"Kernel32.dll", *, delay=None, machine=0x8664, signed=False):
    """One mapped import section in a synthetic PE, with no executable code."""
    data = bytearray(0x500)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    optional = 0x98
    header_size = 240
    struct.pack_into("<HHIIIHH", data, 0x84, machine, 1, 0, 0, 0, header_size, 2)
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<I", data, optional + 108, 16)
    struct.pack_into("<II", data, optional + 112 + 8, 0x1000, 40)
    if signed:
        struct.pack_into("<II", data, optional + 112 + 4 * 8, 0x400, 16)
    struct.pack_into("<IIII", data, optional + header_size + 8, 0x300, 0x1000, 0x300, 0x200)
    struct.pack_into("<IIIII", data, 0x200, 0, 0, 0, 0x1100, 0)
    data[0x300:0x300 + len(dll)] = dll
    if delay:
        struct.pack_into("<II", data, optional + 112 + 13 * 8, 0x1040, 64)
        struct.pack_into("<II", data, 0x240, 1, 0x1140)
        data[0x340:0x340 + len(delay)] = delay
    return bytes(data)
