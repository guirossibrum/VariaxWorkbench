"""Strict offline inventory of hash-pinned packaged resource definitions.

This audits resource framing, not native acceptance or authorization. Never reads
user license storage, executes vendor code, contacts a service or applies a pack.
"""
import hashlib
import json
from pathlib import Path
import struct

from pod_farm_resource_contract import chunks
from research_setup import write_new

SOURCE_HASHES = {
    'BassPack.astwx': 'cc3c8a893875935772f2fd37fd572d1cf3c32261e2cd29f65c15d375ac9e301c',
    'ClassicAmpsPack.astwx': '60e9a5d3d923171cc01927694037fa2d5471b4ed3150892cf3ee7ab34792dfb9',
    'FXExpansion.astwx': 'd4c53e0350fa1239145b4bf132a68f35e191c7ae10a2f424e6c2ae598fee05e5',
    'GP2.astwx': 'e8f81fec37a1f49ec9ec9fbef0e96c4fa9c7cf317f7d811ec411a3311f94d91c',
    'GP2Ext.astwx': '27b5f508f05e22feb80146a30cd0ce5c76dff04606b4b01a58026532e14d0f06',
    'GP3Std.astwx': '7c70394c3e8e762be28e4006490a878acedd629af82ac47f771e08073b942f33',
    'HiGainPack.astwx': '11c41e6c1a8a716cffe435c961f8cfd5292f9f0bee2e419edad803b281504d9b',
    'Z1Std.astwx': 'bba64eeef14743c4af766ec7cf31d7eee1419e8b4ce28fb4ebaa66cb7832b606',
    'Z5Std.astwx': '6fd22ba24520ba49a5c1279f0095ed19886cf3d27dc4c3dad7ec93256209c9b1',
    'ilxplt.astwx': '3aaf1c0c7644b5c4148214d022ffc88e76659df0e1bd3c04e83f27a2911dfede',
    'ilxstd.astwx': 'eb1de8360f25e618e2856f3f2d53495497f1fe3af8e6ca7a470a39e88724783a',
}


def audit_asset_header(data):
    """Exact 56-byte AINF is an audit restriction, not native acceptance policy."""
    if len(data) < 12 or data[:4] != b'FORM' or data[8:12] != b'L6RB':
        raise ValueError('expected FORM/L6RB')
    if struct.unpack_from('>I', data, 4)[0] + 8 != len(data):
        raise ValueError('FORM length mismatch')
    top = list(chunks(data, 12, len(data)))
    headers = [(o, n) for t, o, n in top if t == b'AINF']
    if len(headers) != 1 or headers[0][1] != 56:
        raise ValueError('audit requires one exact AINF header')
    offset, _ = headers[0]
    words = struct.unpack_from('>4I', data, offset)
    return {'asset_id': words[0], 'asset_version': words[1],
            'word08': words[2], 'eligibility_word': words[3],
            'AINF_offset': offset, 'installed_state': None, 'authorized_state': None}


def main():
    root = Path(__file__).resolve().parents[1]
    resources = root/'analysis/pod-farm-258/payloads/Line_6_TWXY_Framework.pkg/L6TWXY.framework/Versions/A/data/res'
    records = []
    for name, expected in sorted(SOURCE_HASHES.items()):
        source = resources/name
        if source.is_symlink() or not source.is_file():
            raise ValueError('expected private regular package resource')
        data = source.read_bytes()
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f'source identity mismatch: {name}')
        records.append({'resource': name, 'sha256': expected, **audit_asset_header(data)})
    value = {'evidence_kind': 'static packaged headers only', 'records': records,
             'native_execution': False, 'device_access': False}
    raw = (json.dumps(value, indent=2, sort_keys=True)+'\n').encode()
    output = root/'analysis/pod-farm-258/native-edge/reports'/('pack-headers-'+hashlib.sha256(raw).hexdigest()+'.json')
    write_new(output, raw)
    print(output.relative_to(root))
    print(f'{len(records)} packaged definitions; installed/authorized state unknown')


if __name__ == '__main__':
    main()
