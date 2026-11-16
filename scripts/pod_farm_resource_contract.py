"""Owned offline resource reader and isolated PINF data-flow reconstruction.

No native execution, device APIs, or application codec. Private source reports
are content-addressed; source bytes are never modified or published as fixtures.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from research_setup import write_new

SOURCES = {
    'GP2.astwx': 'e8f81fec37a1f49ec9ec9fbef0e96c4fa9c7cf317f7d811ec411a3311f94d91c',
    'ilxstd.astwx': 'eb1de8360f25e618e2856f3f2d53495497f1fe3af8e6ca7a470a39e88724783a',
    'Z5Std.astwx': '6fd22ba24520ba49a5c1279f0095ed19886cf3d27dc4c3dad7ec93256209c9b1',
}


def chunks(data, start, end):
    while start < end:
        if start+8 > end:
            raise ValueError('short chunk header')
        tag = data[start:start+4]
        size, = struct.unpack_from('>I', data, start+4)
        stop = start+8+size
        if stop+(size & 1) > end:
            raise ValueError('short chunk data/padding')
        yield tag, start+8, size
        start = stop+(size & 1)


def system_definitions(data):
    """Strict resource inspection, not the native reader's short-copy policy."""
    if len(data) < 12 or data[:4] != b'FORM' or data[8:12] != b'L6RB':
        raise ValueError('expected FORM/L6RB')
    if struct.unpack_from('>I', data, 4)[0]+8 != len(data):
        raise ValueError('FORM size mismatch')
    result = []
    for tag, offset, size in chunks(data, 12, len(data)):
        if tag == b'LIST' and size < 4:
            raise ValueError('short LIST type')
        if tag != b'LIST' or data[offset:offset+4] != b'LAYO':
            continue
        for tag2, offset2, size2 in chunks(data, offset+4, offset+size):
            if tag2 == b'LIST' and size2 < 4:
                raise ValueError('short LIST type')
            if tag2 != b'LIST' or data[offset2:offset2+4] != b'LOUT':
                continue
            children = list(chunks(data, offset2+4, offset2+size2))
            headers = [(o, n) for t, o, n in children if t == b'LOUT']
            if len(headers) != 1 or headers[0][1] < 0x9c:
                raise ValueError('ambiguous/truncated layout header')
            model, = struct.unpack_from('>I', data, headers[0][0])
            if model != 0x3e700003:
                continue
            for child, position, length in children:
                if child != b'LOBJ':
                    continue
                if length < 0x114:
                    raise ValueError('truncated layout object')
                identifier, tag58 = struct.unpack_from('>II', data, position+0x54)
                if identifier == 0x3f20002a:
                    lower, upper, word84 = struct.unpack_from('>iii', data, position+0x7c)
                    result.append({'model': model, 'identifier': identifier,
                                   'payload_offset': position, 'payload_length': length,
                                   'scalar_tag58': tag58, 'lower': lower, 'upper': upper,
                                   'metadata_word84': word84})
    return result


def pinf_read_projection(payload):
    """PINF-local reconstruction only; assumes a successful complete stream read.

    Native 0x185a70 reads min(chunk size, 76); 0x178be0 zeroes its PINF buffer,
    then terminates the last name unit. Unknown extensions have no field here.
    """
    prefix = payload[:76].ljust(76, b'\0')
    version, target = struct.unpack_from('>II', prefix)
    units = list(struct.unpack_from('>32H', prefix, 8))[:31]
    if 0 in units:
        units = units[:units.index(0)]
    device_version, = struct.unpack_from('>I', prefix, 72)
    return {'version': version, 'target': target, 'name_units': units,
            'device_version': device_version}


def pinf_write_projection(record, default_device_version):
    """Owned model of the fixed PINF field writes, not a full tone serializer."""
    units = list(record['name_units'])[:31]
    if 0 in units:
        units = units[:units.index(0)]
    units += [0]*(32-len(units))
    return (struct.pack('>II', record['version'], record['target'])+
            struct.pack('>32H', *units)+
            struct.pack('>I', record['device_version'] or default_device_version))


def registry_integer_projection(payload_word, lower, upper):
    """Owned low-word model of 0x2c7c5..0x2c7dc, not a native setter.

    The caller supplies signed registry endpoints. This models the non-float
    branch only, including its max-then-min behavior for inverted endpoints.
    It does not validate a type tag, invoke observers, or establish active state.
    """
    word = payload_word & 0xffffffff
    signed = word - 0x100000000 if word & 0x80000000 else word
    return min(max(signed, lower), upper) & 0xffffffff


def dual_tone_selection(data):
    """Strict structural inspection of the first bank and two FORM children.

    This is a selection reconstruction, not a native loader/serializer. It
    deliberately rejects truncated HEAD prefixes rather than modeling native
    short-copy/uninitialized bytes. Embedded patch contents remain uninterpreted.
    """
    if (len(data) < 12 or data[:4] != b'FORM' or data[8:12] != b'L6P2' or
            struct.unpack_from('>I', data, 4)[0]+8 != len(data)):
        raise ValueError('expected complete FORM/L6P2')
    top = list(chunks(data, 12, len(data)))
    if any(t == b'LIST' and n < 4 for t, o, n in top):
        raise ValueError('short LIST type')
    heads = [(o, n) for t, o, n in top if t == b'HEAD']
    banks = [(o, n) for t, o, n in top if t == b'LIST' and data[o:o+4] == b'BANK']
    if not heads or heads[0][1] < 4 or not banks:
        raise ValueError('required HEAD/BANK missing or truncated')
    offset, size = banks[0]
    body = list(chunks(data, offset+4, offset+size))
    binfs = [(o, n) for t, o, n in body if t == b'BINF']
    forms = [(o-8, n+8) for t, o, n in body if t == b'FORM']
    if not binfs or len(forms) < 2:
        raise ValueError('required BINF/two FORM children missing')
    return {'head_word': struct.unpack_from('>I', data, heads[0][0])[0],
            'patch_spans': forms[:2]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--resources', type=Path, default=Path(
        'analysis/pod-farm-258/payloads/Line_6_TWXY_Framework.pkg/L6TWXY.framework/Versions/A/data/res'))
    args = parser.parse_args()
    records = []
    for name, expected in SOURCES.items():
        source = args.resources/name
        if not source.is_file():
            raise ValueError('required regular resource file missing')
        data = source.read_bytes()
        sha = hashlib.sha256(data).hexdigest()
        if sha != expected:
            raise ValueError(f'resource identity mismatch: {name}')
        definitions = system_definitions(data)
        if len(definitions) != 1:
            raise ValueError(f'expected one concrete system control: {name}')
        record = definitions[0]
        if (record['scalar_tag58'], record['lower'], record['upper'], record['metadata_word84']) != (0, 0, 1, 1):
            raise ValueError('source definition differs from traced contract')
        records.append({'source': name, 'sha256': sha, **record})
    value = {'evidence_kind': 'static source read', 'records': records,
             'native_execution': False, 'hardware_applicability_verified': False}
    raw = (json.dumps(value, indent=2, sort_keys=True)+'\n').encode()
    root = Path(__file__).resolve().parents[1]
    output = root/'analysis/pod-farm-258/native-edge/reports'/('resource-contract-'+hashlib.sha256(raw).hexdigest()+'.json')
    write_new(output, raw)
    print(json.dumps(value))
    print(output)


if __name__ == '__main__':
    main()
