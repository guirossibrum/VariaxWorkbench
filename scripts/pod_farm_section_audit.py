"""Owned strict offline framing audit; not a native reader or state importer.

Rejects incomplete frames before returning records. Native readers have different
version acceptance, partial mutation and error behavior; see the section ledger.
No plugin, original state fixture, transport or application code is loaded.
"""
import struct


def audit_mapping(data):
    """Audit only writer-version L6ME framing; retain uninterpreted records/tail."""
    if len(data) < 16:
        raise ValueError('short mapping header')
    magic, version, count, reserved = struct.unpack_from('>4I', data)
    if magic != 0x4c364d45 or version != 2:
        raise ValueError('unsupported mapping signature/version')
    end = 16 + count * 104
    if end > len(data):
        raise ValueError('short mapping records')
    return {'reserved': reserved,
            'records': tuple(data[i:i+104] for i in range(16, end, 104)),
            'tail': data[end:]}


def audit_settings(data):
    """Audit writer-version settings framing with unbounded Python arithmetic.

    Header extensions are deliberately unsupported. Record meanings/application
    are not modeled, and this stricter policy does not describe native rejection.
    """
    if len(data) < 20:
        raise ValueError('short settings header')
    if data[:4] == b'1MA6':
        order = '<'
    elif data[:4] == b'6AM1':
        order = '>'
    else:
        raise ValueError('unsupported settings signature')
    _, header, version, total, count = struct.unpack_from(order+'5I', data)
    if header != 20 or version != 2:
        raise ValueError('unsupported settings header/version')
    if total != header + count * 24:
        raise ValueError('inconsistent settings count/length')
    if total > len(data):
        raise ValueError('short settings records')
    return {'records': tuple(struct.unpack_from(order+'6I', data, i)
                             for i in range(header, total, 24)),
            'tail': data[total:]}
