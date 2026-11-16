"""Select pinned math-library data from Apple's update without running installers.

Requires a separately obtained 7-Zip executable. Streams the old ASCII CPIO
payload, materializes only two regular library files, and carves their i386
slices. Everything is private research output, never a native library load.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import stat
import struct
import subprocess

from inspect_firmware import preserve

ARCHIVE_SHA = 'ba29bcf6f3a0a1ac657eeb712bd7e5d53bcd4dbcadb00f8bac68299db1fd5ebb'
PAYLOAD_PATH = 'Mac OS X 10.4.11 Combined Update (Intel)/MacOSXUpdCombo10.4.11Intel.pkg/Contents/Archive.pax.gz'
TARGETS = {
    './usr/lib/libSystem.B.dylib': ('d8fa0d166c4492dda68a7fd92a07a20c03233b46b8def96781f55711628729f1', '36f2a3fbaa04d2ddc7a7ad600c083c637ff28fcae5aa86252509742b16325358'),
    './usr/lib/system/libmathCommon.A.dylib': ('16565737f8c75f1b088be5470cd421730baee5cd1c4f0663fff0c2dd4985ac35', '7445e66b1cb7b49407cc9641d71bd090b5706a0999936834d9bd36a56bfaa6a0'),
}


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def read_exact(source, size):
    data = source.read(size)
    if len(data) != size:
        raise ValueError('truncated CPIO payload')
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('output', type=Path, help='new private output directory')
    parser.add_argument('--sevenzip', required=True, type=Path)
    args = parser.parse_args()
    if not args.archive.is_file() or args.archive.stat().st_size != 337_080_169 or sha(args.archive) != ARCHIVE_SHA:
        raise ValueError('requires the pinned original Apple Intel update DMG')
    args.output.mkdir()  # Refuse existing directories, including partial prior runs.
    payload = args.output / 'Archive.pax.gz'
    # Named member only; no shell, package entry point, mount or install script.
    with payload.open('xb') as output:
        process = subprocess.Popen([str(args.sevenzip.resolve()), 'x', '-so', '-bd', str(args.archive.resolve()), PAYLOAD_PATH],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        size = 0
        try:
            while chunk := process.stdout.read(65536):
                size += len(chunk)
                if size > 400 * 1024 * 1024:
                    process.kill()
                    raise ValueError('selected compressed payload exceeds its research bound')
                output.write(chunk)
            error = process.stderr.read()
            if process.wait() != 0:
                raise RuntimeError('7-Zip data extraction failed: ' + error.decode(errors='replace'))
        finally:
            if process.poll() is None:
                process.kill()
            process.wait()
    found, consumed = {}, 0
    with gzip.open(payload, 'rb') as source:
        while True:
            header = read_exact(source, 76)
            consumed += 76
            if header[:6] != b'070707':
                raise ValueError('expected old ASCII CPIO')
            mode, namesize, size = int(header[18:24], 8), int(header[59:65], 8), int(header[65:76], 8)
            if not 1 <= namesize <= 4096 or consumed + namesize + size > 1024 ** 3:
                raise ValueError('CPIO member exceeds bounded stream')
            raw_name = read_exact(source, namesize)
            consumed += namesize
            if raw_name[-1:] != b'\0':
                raise ValueError('unterminated CPIO name')
            name = raw_name[:-1].decode('utf-8')
            selected = name in TARGETS
            if selected and (name in found or not stat.S_ISREG(mode) or size > 16 * 1024 * 1024):
                raise ValueError('unexpected selected library member')
            data = bytearray()
            remaining = size
            while remaining:
                chunk = read_exact(source, min(remaining, 65536))
                if selected:
                    data.extend(chunk)
                consumed += len(chunk)
                remaining -= len(chunk)
            if selected:
                found[name] = bytes(data)
            if name == 'TRAILER!!!':
                while chunk := source.read(65536):  # Also verify the gzip trailer/CRC.
                    consumed += len(chunk)
                    if consumed > 1024 ** 3:
                        raise ValueError('CPIO tail exceeds bounded stream')
                break
    if set(found) != set(TARGETS):
        raise ValueError('selected libraries are incomplete')
    records = []
    for name, data in found.items():
        fat_sha, slice_sha = TARGETS[name]
        if hashlib.sha256(data).hexdigest() != fat_sha or data[:4] != bytes.fromhex('cafebabe'):
            raise ValueError('universal library fingerprint mismatch')
        count = struct.unpack_from('>I', data, 4)[0]
        if not 1 <= count <= 16 or 8 + 20 * count > len(data):
            raise ValueError('invalid FAT table')
        slices = []
        for i in range(count):
            cpu, subtype, offset, size, alignment = struct.unpack_from('>5I', data, 8 + 20 * i)
            if offset < 8 + 20 * count or offset + size > len(data):
                raise ValueError('FAT slice outside file')
            if cpu == 7:
                slices.append((offset, data[offset:offset + size]))
        if len(slices) != 1 or hashlib.sha256(slices[0][1]).hexdigest() != slice_sha:
            raise ValueError('i386 slice fingerprint mismatch')
        offset, part = slices[0]
        if part[:4] != bytes.fromhex('cefaedfe'):
            raise ValueError('expected little-endian i386 Mach-O')
        basename = Path(name).name
        preserve(args.output / basename, data)
        preserve(args.output / (Path(basename).stem + '.i386.macho'), part)
        records.append({'member':name, 'universal_sha256':fat_sha, 'i386_sha256':slice_sha,
                        'i386_offset':offset, 'i386_bytes':len(part)})
    report = {'archive_sha256':ARCHIVE_SHA, 'compressed_payload_sha256':sha(payload),
              'sevenzip_sha256':sha(args.sevenzip), 'uncompressed_stream_bytes':consumed,
              'libraries':records, 'scope':'static files only; no scripts, libraries or installer entry points executed'}
    preserve(args.output / 'manifest.json', (json.dumps(report, indent=2) + '\n').encode())
    print(args.output / 'manifest.json')


if __name__ == '__main__':
    main()
