"""Static, source-pinned POD Farm 1.12 research extraction for issue #29.

Reads archive data only. No installer, vendor code, mounting or device access.
The pin identifies the supplied bytes; it does not authenticate the publisher.
"""
import argparse
import gzip
import io
import json
from pathlib import Path
import plistlib

from pod_farm_inventory import digest, macho, require, seven
from research_setup import destination, unpack_odc, write_new

SOURCE_SHA = '086416c6c6b7e4471871bee4d44f4d8949362e57155677fd06d8288a344e399a'
SOURCE_SIZE = 137347009
PREFIX = 'POD Farm/POD Farm.mpkg/Contents/Packages/'
COMPONENTS = ('Line 6 TWXY Framework', 'Line 6 TWX Framework', 'POD Farm',
              'POD Farm Common', 'POD Farm AU Plugin', 'POD Farm VST Plugin',
              'POD Farm RTAS Plugin')
LIMIT = 256 * 1024 * 1024


def checked_source(source):
    require(source.is_file() and source.stat().st_size == SOURCE_SIZE,
            'source size/type mismatch')
    require(digest(source.read_bytes()) == SOURCE_SHA, 'source digest mismatch')


def fresh_output(output, root):
    require(output.resolve().is_relative_to(root.resolve()) and
            output.resolve() != root.resolve(), 'output must be below ignored analysis/')
    require(not output.exists() and not output.is_symlink(), 'use a fresh output directory')
    output.mkdir(parents=True)


def inflate(data):
    with gzip.GzipFile(fileobj=io.BytesIO(data)) as stream:
        plain = stream.read(LIMIT + 1)
    require(len(plain) <= LIMIT, 'oversize CPIO')
    return plain


def extract(source, tool, output):
    checked_source(source)
    fresh_output(output, Path(__file__).resolve().parents[1] / 'analysis')
    write_new(output / 'dmg-list.txt', seven(tool, 'l', '-slt', source))
    packages = []
    for component in COMPONENTS:
        member = PREFIX + component + '.pkg/Contents/'
        metadata = seven(tool, 'x', '-so', source, member + 'Info.plist')
        require(len(metadata) <= 1024 * 1024, 'oversize package metadata')
        info = plistlib.loads(metadata)
        target = output / 'packages' / component
        write_new(target / 'Info.plist', metadata)
        archive = seven(tool, 'x', '-so', source, member + 'Archive.pax.gz')
        require(len(archive) <= LIMIT, 'oversize compressed archive')
        write_new(target / 'Archive.pax.gz', archive)
        links = unpack_odc(inflate(archive), output / 'payloads' / component)
        write_new(target / 'links.json', (json.dumps(links, indent=2) + '\n').encode())
        packages.append({'component': component, 'archive_sha256': digest(archive),
                         'metadata_sha256': digest(metadata),
                         'identifier': info.get('CFBundleIdentifier'),
                         'version': info.get('CFBundleShortVersionString'), **links})
    files, natives, bundles = [], [], []
    for path in sorted((output / 'payloads').rglob('*')):
        require(not path.is_symlink(), 'unexpected extracted symlink')
        if not path.is_file():
            continue
        data = path.read_bytes()
        name = str(path.relative_to(output / 'payloads'))
        destination(output / 'payloads', name)
        files.append({'path': name, 'size': len(data), 'sha256': digest(data)})
        if data[:4] in [bytes.fromhex(s) for s in
                       ('cafebabe', 'cefaedfe', 'cffaedfe', 'feedface', 'feedfacf')]:
            natives.append({'path': name, 'slices': macho(data)})
        if path.name == 'Info.plist':
            info = plistlib.loads(data)
            bundles.append({'path': name, 'metadata': {key: info.get(key) for key in
                            ('CFBundleIdentifier', 'CFBundleVersion', 'CFBundleShortVersionString')}})
    checked_source(source)
    report = {'source_sha256': SOURCE_SHA, 'source_size': SOURCE_SIZE,
              'packages': packages, 'files': files, 'natives': natives, 'bundles': bundles,
              'vendor_code_executed': False, 'device_access': False,
              'signature_authenticated': False}
    write_new(output / 'inventory.json', (json.dumps(report, indent=2) + '\n').encode())
    print(json.dumps({'packages': len(packages), 'files': len(files),
                      'natives': len(natives), 'bundles': bundles}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, type=Path)
    parser.add_argument('--seven', type=Path, default=Path('.tools/7zip/7zz'))
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    extract(args.source, args.seven, args.output)
