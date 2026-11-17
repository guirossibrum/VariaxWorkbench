"""Read two source-qualified x86 PE table entries; no executable is loaded.

Input is an explicitly supplied private standalone L6SM.sys, never a device.
This static check is not a transport test or native compatibility oracle.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import stat

SOURCE = 'b109c86093a54a33b8eff7f677053f3ca1960c3c304a137ed6ea776f4f17c5d7'


def inspect(data):
    if hashlib.sha256(data).hexdigest() != SOURCE:
        raise ValueError('standalone x86 source hash mismatch')
    pe = struct.unpack_from('<I', data, 0x3c)[0]
    count = struct.unpack_from('<H', data, pe + 6)[0]
    optional = struct.unpack_from('<H', data, pe + 20)[0]

    def word(va):
        for i in range(count):
            pos = pe + 24 + optional + i * 40
            _, rva, size, raw = struct.unpack_from('<4I', data, pos + 8)
            if rva <= va - 0x10000 <= rva + size - 4:
                return struct.unpack_from('<I', data, raw + va - 0x10000 - rva)[0]
        raise ValueError('table address outside file-backed section')

    major = word(0x17a30 + 14 * 4)
    handler = word(0x17400 + 0x48)
    if (major, handler) != (0x12d3d, 0x11c78):
        raise ValueError('static source table disagreement')
    return {'source_sha256': SOURCE, 'major_14_wrapper': hex(major),
            'vtable_48_handler': hex(handler), 'vendor_executed': False,
            'hardware_access': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    args = parser.parse_args()
    info = args.source.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_size != 39296:
        raise ValueError('source must be the regular 39296-byte private x86 file')
    print(json.dumps(inspect(args.source.read_bytes()), indent=2))
