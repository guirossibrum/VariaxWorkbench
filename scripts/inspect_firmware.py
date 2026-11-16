"""Strict, offline L6FF firmware inspection. Never contacts or writes a device.

Container interpretation is checked against the supplied original files. Header
MD5 checks integrity, not authenticity. Region type meanings and executable ISA
are not inferred from their numeric values. Only local analysis files are saved.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[1]
MAX_IMAGE = 16 * 1024 * 1024


def inspect(data):
    if len(data) < 12 or data[:4] != b'FORM' or data[8:12] != b'L6FF':
        raise ValueError('expected FORM/L6FF')
    if struct.unpack_from('>I', data, 4)[0] != len(data)-8:
        raise ValueError('FORM length mismatch')
    chunks, cursor = [], 12
    while cursor < len(data):
        if cursor+8 > len(data):
            raise ValueError('truncated chunk header')
        tag, size = data[cursor:cursor+4], struct.unpack_from('>I', data, cursor+4)[0]
        end = cursor + 8 + size
        if end + size % 2 > len(data):
            raise ValueError('chunk exceeds file length')
        chunks.append((tag, data[cursor+8:end]))
        cursor = end + size % 2
    heads = [p for t,p in chunks if t == b'HEAD']
    compressed = [p for t,p in chunks if t == b'data']
    if len(heads) != 1 or len(compressed) != 1 or len(heads[0]) != 52:
        raise ValueError('expected one 52-byte HEAD and one data chunk')
    header = heads[0]
    format_version, version, product, compression, length = struct.unpack('>5I', header[:20])
    if format_version != 0x10000 or compression != 1 or not 0 < length <= MAX_IMAGE:
        raise ValueError('unsupported format/compression or image length')
    decoder = zlib.decompressobj()
    image = decoder.decompress(compressed[0], length + 1)
    if len(image) != length or not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:
        raise ValueError('compressed stream length/termination mismatch')
    if hashlib.md5(image).digest() != header[20:36]:
        raise ValueError('uncompressed image MD5 mismatch')
    regions, expected_offset = [], 0
    for tag, payload in chunks:
        if tag != b'dinf':
            if tag not in (b'HEAD', b'data'):
                raise ValueError(f'unsupported chunk {tag!r}')
            continue
        if len(payload) != 16:
            raise ValueError('expected 16-byte region information')
        kind, region_version, offset, size = struct.unpack('>4I', payload)
        if offset != expected_offset or offset+size > len(image):
            raise ValueError('noncontiguous/out-of-range region')
        raw = image[offset:offset+size]
        regions.append({'type_raw':kind, 'version_hex':f'{region_version:08x}',
                        'offset':offset,'bytes':size,'sha256':hashlib.sha256(raw).hexdigest()})
        expected_offset += size
    if not regions or expected_offset != len(image):
        raise ValueError('region coverage differs from complete image')
    return {'container':'FORM/L6FF','product_id':f'{product:08x}',
            'version_hex':f'{version:08x}','compression':'zlib',
            'image_bytes':len(image),'image_sha256':hashlib.sha256(image).hexdigest(),
            'image_md5':hashlib.md5(image).hexdigest(), 'header_md5_matches':True,
            'regions':regions,'integrity_is_not_authentication':True}, image


def preserve(path, data):
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f'refusing to replace different contents: {path}')
        return
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
        temporary = Path(output.name)
        try:
            output.write(data)
            output.flush()
            import os
            os.fsync(output.fileno())
            os.link(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def self_test():
    def chunk(tag,payload):
        return tag+struct.pack('>I',len(payload))+payload+(b'\0' if len(payload)%2 else b'')
    image = bytes(range(256))*4
    head = struct.pack('>5I',0x10000,0x3010000,0x3000a,1,len(image))+hashlib.md5(image).digest()+bytes(16)
    body = b'L6FF'+chunk(b'HEAD',head)+chunk(b'dinf',struct.pack('>4I',0,0x3010000,0,len(image)))+chunk(b'data',zlib.compress(image))
    valid = chunk(b'FORM',body)
    assert inspect(valid)[1] == image
    checks = 0
    for n in range(len(valid)):
        try: inspect(valid[:n])
        except ValueError: checks += 1
        else: raise AssertionError(f'accepted truncated prefix {n}')
    for offset in (4, 20, 32, 36, 40, 96):
        bad=bytearray(valid); bad[offset] ^= 0xff
        try: inspect(bytes(bad))
        except (ValueError,zlib.error): checks += 1
        else: raise AssertionError(f'accepted damaged structural/integrity field at {offset}')
    print(f'PASS: synthetic L6FF roundtrip and {checks} corrupt/truncated inputs rejected')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files', nargs='*', type=Path)
    parser.add_argument('--self-test', action='store_true')
    args=parser.parse_args()
    if args.self_test:
        self_test()
    reports=[]
    for path in args.files:
        data=path.read_bytes()
        report,image=inspect(data)
        report.update({'source':str(path),'source_bytes':len(data),'source_sha256':hashlib.sha256(data).hexdigest()})
        directory=ROOT/'analysis/extracted/firmware'/report['source_sha256']
        directory.mkdir(parents=True,exist_ok=True)
        preserve(directory/'image.bin',image)
        for i,region in enumerate(report['regions']):
            offset,size=region['offset'],region['bytes']
            preserve(directory/f'region-{i:02d}-type-{region["type_raw"]}.bin',image[offset:offset+size])
        report['output_directory']=str(directory.relative_to(ROOT))
        preserve(directory/'manifest.json',(json.dumps(report,indent=2)+'\n').encode())
        reports.append(report)
        print(f'PASS: {path.name}: product {report["product_id"]}, version {report["version_hex"]}, {len(image)} bytes, {len(report["regions"])} regions; embedded MD5 matches')
    if reports:
        directory = ROOT/'analysis/reports'
        directory.mkdir(parents=True,exist_ok=True)
        data = (json.dumps(reports,indent=2)+'\n').encode()
        path = directory/('firmware-containers-'+hashlib.sha256(data).hexdigest()[:16]+'.json')
        preserve(path,data)
        print(path)

if __name__ == '__main__':
    main()
