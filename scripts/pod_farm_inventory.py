"""Offline POD Farm 2.58 data inventory. Never load or execute vendor code.

Reports and extracted bytes belong in ignored analysis storage, never in git.
The pinned source hash identifies the supplied copy, not a trusted signature.
"""
import argparse
from collections import Counter
import gzip
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess
import xml.etree.ElementTree as ET

from research_setup import destination, unpack_odc, write_new

SOURCE_SHA = '483e4129f2d319665ab7273194818acf570181cac8eb8f75dc5f96581715217e'
SOURCE_SIZE = 201113929
LIMIT = 256 * 1024 * 1024


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def macho(data):
    """Read bounded Mach-O headers, dylib imports and symbol names as data."""
    if data[:4] == bytes.fromhex('cafebabe'):
        require(len(data) >= 8, 'short fat header')
        count, = struct.unpack_from('>I', data, 4)
        require(1 <= count <= 16 and 8 + 20 * count <= len(data), 'bad fat table')
        result, spans = [], []
        for i in range(count):
            cpu, _, offset, size, _ = struct.unpack_from('>IIIII', data, 8 + 20*i)
            require(offset >= 8 + 20*count and size >= 28 and offset+size <= len(data), 'bad fat slice')
            require(all(offset+size <= a or offset >= b for a, b in spans), 'overlapping fat slices')
            spans.append((offset, offset+size))
            records = macho(data[offset:offset+size])
            require(len(records) == 1 and records[0]['cpu'] == cpu, 'fat CPU mismatch')
            result.extend(records)
        return result
    magic = data[:4]
    if magic not in [bytes.fromhex(s) for s in ['cefaedfe', 'cffaedfe', 'feedface', 'feedfacf']]:
        raise ValueError('not a supported Mach-O')
    endian = '<' if magic[0] in (0xce, 0xcf) else '>'
    bits = 64 if magic in (bytes.fromhex('cffaedfe'), bytes.fromhex('feedfacf')) else 32
    header = 32 if bits == 64 else 28
    require(len(data) >= header, 'short Mach-O header')
    cpu, _, kind, count, command_bytes = struct.unpack_from(endian+'IIIII', data, 4)
    require(count <= 65536 and header+command_bytes <= len(data), 'bad load command bounds')
    offset, imports, symbols, code_signature = header, [], [], False
    for _ in range(count):
        require(offset+8 <= header+command_bytes, 'short load command')
        command, size = struct.unpack_from(endian+'II', data, offset)
        require(size >= 8 and offset+size <= header+command_bytes, 'bad load command size')
        if command in (0xc, 0x80000018, 0x8000001f, 0x80000023, 0x20):
            require(size >= 24, 'short dylib command')
            start, = struct.unpack_from(endian+'I', data, offset+8)
            require(24 <= start < size, 'bad dylib name offset')
            raw = data[offset+start:offset+size]
            require(b'\0' in raw, 'unterminated dylib name')
            imports.append(raw.split(b'\0', 1)[0].decode('utf-8'))
        if command == 2:
            require(size >= 24, 'short symtab command')
            symoff, nsyms, stroff, strsize = struct.unpack_from(endian+'IIII', data, offset+8)
            width = 16 if bits == 64 else 12
            require(symoff+nsyms*width <= len(data) and stroff+strsize <= len(data), 'bad symtab bounds')
            for i in range(nsyms):
                index, = struct.unpack_from(endian+'I', data, symoff+i*width)
                require(index < strsize, 'bad string index')
                raw = data[stroff+index:stroff+strsize]
                require(b'\0' in raw, 'unterminated symbol')
                symbols.append(raw.split(b'\0', 1)[0].decode('utf-8', 'replace'))
        code_signature |= command == 0x1d
        offset += size
    require(offset == header+command_bytes, 'load command count mismatch')
    return [{'cpu': cpu, 'bits': bits, 'filetype': kind,
             'sha256': digest(data), 'imports': imports,
             'has_code_signature_command': code_signature, 'symbols': symbols}]


def iff_chunks(data):
    """Structural IFF survey only. Does not interpret POD Farm parameter values."""
    require(len(data) >= 12 and data[:4] == b'FORM', 'not FORM')
    size, = struct.unpack_from('>I', data, 4)
    require(size >= 4 and size+8 == len(data), 'FORM size mismatch')
    def walk(start, end, depth):
        require(depth <= 16, 'IFF nesting limit')
        result = []
        while start < end:
            require(start+8 <= end, 'short IFF header')
            tag = data[start:start+4]
            length, = struct.unpack_from('>I', data, start+4)
            finish = start+8+length
            require(finish+(length & 1) <= end, 'short IFF body/padding')
            payload = data[start+8:finish]
            record = {'tag': tag.decode('ascii'), 'size': length}
            if tag in (b'FORM', b'LIST', b'CAT '):
                require(length >= 4, 'short IFF container')
                record['type'] = payload[:4].decode('ascii')
                record['children'] = walk(start+12, finish, depth+1)
            elif tag == b'PINF' and length >= 8:
                record['version_word'], record['target_word'] = struct.unpack_from('>II', payload)
            result.append(record)
            start = finish+(length & 1)
        return result
    return {'type': data[8:12].decode('ascii'), 'chunks': walk(12, len(data), 0)}


def seven(tool, *args):
    return subprocess.check_output([str(tool), *map(str, args)], timeout=120)


def extract(source, tool, output):
    raw = source.read_bytes()
    require(len(raw) == SOURCE_SIZE and digest(raw) == SOURCE_SHA, 'source identity mismatch')
    require(not output.exists(), 'use a fresh extraction output directory')
    output.mkdir(parents=True)
    write_new(output/'dmg-list.txt', seven(tool, 'l', '-slt', source))
    for member, name in [('POD Farm 2/POD Farm 2.pkg', 'installer.pkg'),
                         ('POD Farm 2/Documents/PODFarm_Help.pdf', 'PODFarm_Help.pdf')]:
        write_new(output/name, seven(tool, 'x', '-so', source, member))
    package = output/'installer.pkg'
    listing = seven(tool, 'l', '-slt', package)
    write_new(output/'pkg-list.txt', listing)
    for section in listing.decode().split('\n\n'):
        fields = dict(line.split(' = ', 1) for line in section.splitlines() if ' = ' in line)
        if fields.get('Type') != 'file':
            continue
        member = fields['Path']
        require(int(fields['Size']) <= LIMIT, 'oversize XAR member')
        target = destination(output/'xar', member)
        data = seven(tool, 'x', '-so', package, member)
        require(len(data) == int(fields['Size']), 'XAR extraction length mismatch')
        write_new(target, data)
        if member.endswith('/Payload'):
            with gzip.GzipFile(fileobj=io.BytesIO(data)) as stream:
                plain = stream.read(LIMIT+1)
            require(len(plain) <= LIMIT, 'oversize CPIO')
            name = member.split('/')[0]
            record = unpack_odc(plain, output/'payloads'/name)
            write_new(output/'payloads'/(name+'.links.json'), json.dumps(record, indent=2).encode())
    require(source.stat().st_size == SOURCE_SIZE and digest(source.read_bytes()) == SOURCE_SHA,
            'original identity changed during extraction')
    return inventory(output)


def inventory(output):
    records, packages, natives, tones = [], [], [], []
    for p in sorted((output/'xar').glob('*/PackageInfo')):
        root = ET.fromstring(p.read_bytes())
        packages.append(dict(root.attrib, payload=root.find('payload').attrib))
    for p in sorted((output/'payloads').rglob('*')):
        require(not p.is_symlink(), 'unexpected extracted symlink')
        if not p.is_file():
            continue
        data = p.read_bytes()
        name = str(p.relative_to(output/'payloads'))
        records.append({'path': name, 'size': len(data), 'sha256': digest(data)})
        if data[:4] in [bytes.fromhex(s) for s in ['cafebabe', 'cefaedfe', 'cffaedfe', 'feedface', 'feedfacf']]:
            natives.append({'path': name, 'slices': macho(data)})
        if p.suffix == '.l6t':
            tones.append({'path': name, **iff_chunks(data)})
    report = {'source_sha256': SOURCE_SHA, 'source_size': SOURCE_SIZE,
              'packages': packages, 'files': records, 'natives': natives, 'tones': tones,
              'vendor_code_executed': False, 'device_access': False,
              'signature_authenticated': False}
    write_new(output/'inventory.json', (json.dumps(report, indent=2)+'\n').encode())
    print(json.dumps({'packages': len(packages), 'files': len(records), 'native_files': len(natives),
                      'tone_types': dict(Counter(t['type'] for t in tones))}))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path('.tools/coordinator-inputs/installers/POD Farm 2.58.dmg'))
    parser.add_argument('--seven', type=Path, default=Path('.tools/7zip/7zz'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--inventory-only', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]/'analysis'
    require(args.output.resolve().is_relative_to(root.resolve()), 'output must be in local ignored analysis storage')
    if args.inventory_only:
        require(args.source.stat().st_size == SOURCE_SIZE and digest(args.source.read_bytes()) == SOURCE_SHA,
                'source identity mismatch')
        inventory(args.output)
    else:
        extract(args.source, args.seven, args.output)


if __name__ == '__main__':
    main()
