"""Pure-data reference for the recovered Workbench HD LIST/TUNE record path.

This does not execute or call Line 6 code. The narrow format follows static
analysis of WB2AltTuning::FromStream/ToStream and SetTune in the macOS binary.
The Windows comparison is documented separately; this synthetic reference
models the macOS reader's out-of-range skip behavior.
"""
from __future__ import annotations

import struct


def _chunk(tag: bytes, payload: bytes) -> bytes:
    if len(tag) != 4:
        raise ValueError('IFF tag must be four bytes')
    return tag + struct.pack('>I', len(payload)) + payload + (b'\0' if len(payload) & 1 else b'')


def _read_chunks(data: bytes):
    pos = 0
    while pos < len(data):
        if len(data) - pos < 8:
            raise ValueError('truncated IFF chunk header')
        tag, size = data[pos:pos + 4], struct.unpack_from('>I', data, pos + 4)[0]
        start, end = pos + 8, pos + 8 + size
        if end > len(data):
            raise ValueError('IFF chunk exceeds parent')
        yield tag, data[start:end]
        pos = end + (size & 1)
    if pos != len(data):
        raise ValueError('invalid IFF padding')


def encode_tune(name: bytes, values: list[float]) -> bytes:
    """Encode only the source-supported six-value LIST/TUNE form."""
    if len(values) != 6:
        raise ValueError('source writer emits six string records')
    if b'\0' in name:
        raise ValueError('name contains NUL')
    header = b'\0\0\0\0' + name + b'\0'
    header += b'\0' * ((-len(header)) % 4)
    children = [_chunk(b'THDR', header)]
    for index, value in enumerate(values):
        payload = bytes((index, 0, 0, 0)) + struct.pack('>f', float(value))
        children.append(_chunk(b'STRG', payload))
    return _chunk(b'LIST', b'TUNE' + b''.join(children))


def decode_tune(data: bytes) -> tuple[bytes, dict[int, float]]:
    """Decode the recovered shape; unknown children fail and invalid slots skip."""
    roots = list(_read_chunks(data))
    if len(roots) != 1 or roots[0][0] != b'LIST' or len(roots[0][1]) < 4:
        raise ValueError('expected one LIST/TUNE')
    payload = roots[0][1]
    if payload[:4] != b'TUNE':
        raise ValueError('wrong list type')
    children = list(_read_chunks(payload[4:]))
    if not children or children[0][0] != b'THDR' or len(children[0][1]) < 4:
        raise ValueError('missing or short THDR')
    raw_name = children[0][1][4:]
    name = raw_name.split(b'\0', 1)[0]
    values: dict[int, float] = {}
    for tag, record in children[1:]:
        if tag != b'STRG':
            raise ValueError('unknown tuning child field')
        if len(record) != 8:
            raise ValueError('STRG payload must be eight bytes')
        index = record[0]
        if index > 5:
            continue
        # The stream has no type field; the recovered reader assigns type 1.
        values[index] = struct.unpack('>f', record[4:8])[0]
    return name, values


def set_tune_reference(index: int, value: float, *, count: int = 7, type_tag: int = 1) -> int:
    """Mirror the recovered SetTune -> SetParam checks (status codes retained)."""
    if not 0 <= index <= 5:
        return 0x80000002
    param_index = index + 1
    if param_index >= count:
        return 0x80000008
    if type_tag != 1:
        return 0x80000009
    struct.pack('>f', float(value))  # Ensure a representable serialized float.
    return 0
