"""Static, bounded XML/SFNT measurements; never load a font or vendor library.

Pass an already statically extracted res directory. Reports are private and
must be saved under the task's ignored analysis directory, never published.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import struct
import xml.etree.ElementTree as ET


def font_metrics(data):
    if len(data) < 12 or data[:4] != b'\x00\x01\x00\x00':
        raise ValueError('expected bounded TrueType SFNT')
    count = struct.unpack_from('>H', data, 4)[0]
    if not 1 <= count <= 128 or 12 + count * 16 > len(data):
        raise ValueError('invalid table directory')
    tables = {}
    for i in range(count):
        tag, _, offset, length = struct.unpack_from('>4sIII', data, 12 + 16 * i)
        if tag in tables or offset + length > len(data):
            raise ValueError('duplicate or out-of-bounds table')
        tables[tag] = (offset, length)
    for tag, minimum in ((b'head', 20), (b'hhea', 10), (b'OS/2', 78)):
        if tag not in tables or tables[tag][1] < minimum:
            raise ValueError('missing or short metric table')
    head, hhea, os2 = (tables[t][0] for t in (b'head', b'hhea', b'OS/2'))
    return {'units_per_em': struct.unpack_from('>H', data, head + 18)[0],
            'hhea_ascent_descent_gap': struct.unpack_from('>hhh', data, hhea + 4),
            'typo_ascent_descent_gap': struct.unpack_from('>hhh', data, os2 + 68),
            'win_ascent_descent': struct.unpack_from('>HH', data, os2 + 74)}


def main():
    from inspect_firmware import preserve
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('resources', type=Path)
    parser.add_argument('report', type=Path)
    args = parser.parse_args()
    task = Path(__file__).resolve().parents[1]
    if not args.report.resolve().is_relative_to(task / 'analysis'):
        raise ValueError('report must remain in this task analysis directory')
    rows = []
    for path in sorted((args.resources / 'xg_scripts').glob('*.xml')):
        raw = path.read_bytes()
        if len(raw) > 2_000_000 or b'<!DOCTYPE' in raw.upper() or b'<!ENTITY' in raw.upper():
            raise ValueError('unsupported XML size/declarations')
        tree = ET.fromstring(raw)
        rows.append({'file': path.name, 'sha256': hashlib.sha256(raw).hexdigest(),
                     'tag_counts': dict(collections.Counter(x.tag for x in tree.iter()))})
    if len(rows) != 51:
        raise ValueError('expected the source-pinned 51 XML files')
    font = (args.resources / 'fonts/InterDisplay-Regular.ttf').read_bytes()
    if hashlib.sha256(font).hexdigest() != '99614bda7ff423aaf470990692dd93613a5971ab4446e4a6d5a83b3d74865074':
        raise ValueError('font identity differs from inspected source')
    report = {'xml': rows, 'font_sha256': hashlib.sha256(font).hexdigest(),
              'font_bytes': len(font), 'font_metrics': font_metrics(font),
              'auditor_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'scope': 'static declared table metrics/tag counts; no shaping/rasterization/runtime behavior'}
    preserve(args.report, (json.dumps(report, indent=2) + '\n').encode())
    print('Measured', len(rows), 'XML files and one pinned font; private report:', args.report)


if __name__ == '__main__':
    main()
