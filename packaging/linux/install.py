#!/usr/bin/env python3
"""Install/remove this offline package in an explicit prefix without overwrites.

Python is an installation tool only; the installed Rust applications do not use
it. No device, driver, service, desktop cache, or shell configuration is touched.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import tempfile

APP = 'variax-workbench'
RECEIPT = Path('share/variax-workbench/install-manifest.json')
BINARIES = {'variax-workbench', 'variax-workbench-gui', 'variax-check', 'variax-firmware-inspect'}
DESKTOP_ID = 'io.github.guirossibrum.VariaxWorkbench'


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def relative_name(name):
    p = PurePosixPath(name)
    if not name or p.is_absolute() or '..' in p.parts or '\\' in name or ':' in name or '\0' in name:
        raise ValueError('invalid package path')
    if p.as_posix() != name:
        raise ValueError('non-canonical package path')
    allowed = (len(p.parts) == 2 and p.parts[0] == 'bin' and p.name in BINARIES)
    allowed |= name in {
        f'share/applications/{DESKTOP_ID}.desktop',
        f'share/icons/hicolor/scalable/apps/{DESKTOP_ID}.svg',
        f'share/mime/packages/{DESKTOP_ID}.xml',
    }
    allowed |= p.parts[:3] == ('share', 'doc', APP) and len(p.parts) > 3
    if not allowed:
        raise ValueError(f'path outside package namespace: {name}')
    return Path(*p.parts)


def checked_path(root, relative):
    target = root / relative
    # Refuse symlink redirection, including a symlink used as the prefix itself.
    for part in (target, *target.parents):
        if part.is_symlink():
            raise ValueError(f'symlink in installation path: {part}')
    return target


def entries(manifest):
    if manifest.get('application') != APP or manifest.get('format') != 1:
        raise ValueError('unrecognized package manifest')
    if manifest.get('hardware_access_locked') is not True or manifest.get('vendor_resources_included') is not False:
        raise ValueError('package does not declare the offline-only distribution policy')
    seen = set()
    for entry in manifest['files']:
        path = relative_name(entry['path'])
        if path in seen or entry['mode'] not in (0o644, 0o755) or len(entry['sha256']) != 64:
            raise ValueError('invalid or duplicate manifest entry')
        seen.add(path)
    if not seen:
        raise ValueError('empty package manifest')
    return manifest['files']


def matches(path, entry):
    return (path.is_file() and not path.is_symlink()
            and path.stat().st_size == entry['size'] and digest(path) == entry['sha256']
            and (os.name != 'posix' or path.stat().st_mode & 0o777 == entry['mode']))


def check_prefix(prefix):
    if not prefix.is_absolute() or any(ord(c) < 32 or c in '=%' for c in str(prefix)):
        raise ValueError('prefix must be absolute, without control characters, = or %')


def desktop_content(data, prefix):
    lines = data.decode().splitlines()
    value = next(line.removeprefix('Exec=') for line in lines if line.startswith('Exec='))
    tail = value.removeprefix('variax-workbench-gui')
    if tail not in [' %f', ' --display auto %f', ' --display x11 %f', ' --display wayland %f']:
        raise ValueError('unrecognized desktop command')
    executable = str(prefix/'bin/variax-workbench-gui')
    # The Desktop Entry specification applies string escaping before Exec
    # argument quoting. This is data for a launcher, never a shell command.
    quoted = ''.join('\\'+c if c in '\\"`$' else c for c in executable)
    quoted = '"'+quoted.replace('\\', '\\\\')+'"'
    string_value = lambda s: str(s).replace('\\', '\\\\')
    replacements = {
        'Exec':quoted+tail, 'TryExec':string_value(executable),
        'Icon':string_value(prefix/f'share/icons/hicolor/scalable/apps/{DESKTOP_ID}.svg'),
    }
    return ('\n'.join(k+'='+replacements[k] if (k:=line.partition('=')[0]) in replacements else line
                      for line in lines)+'\n').encode()


def create_new(path, data, mode):
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        temporary.chmod(mode)
        info = temporary.stat()
        os.link(temporary, path)
        return info
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def install(package, prefix):
    check_prefix(prefix)
    manifest = json.loads((package / 'manifest.json').read_text())
    records = entries(manifest)
    receipt = checked_path(prefix, RECEIPT)
    if receipt.exists():
        raise ValueError('installation receipt exists; remove the previous package first')
    targets = []
    for entry in records:
        relative = relative_name(entry['path'])
        source = checked_path(package / 'files', relative)
        target = checked_path(prefix, relative)
        if not matches(source, entry):
            raise ValueError(f'package digest mismatch: {relative}')
        if target.exists():
            raise ValueError(f'refusing to replace existing file: {target}')
        targets.append((source, target, entry))
    made_dirs, created, installed_entries = set(), [], []
    try:
        for target in [t for _, t, _ in targets] + [receipt]:
            for parent in reversed(target.parents):
                if not parent.exists():
                    parent.mkdir()
                    made_dirs.add(parent)
        for source, target, entry in targets:
            checked_path(prefix, relative_name(entry['path']))
            data = source.read_bytes()
            if len(data) != entry['size'] or hashlib.sha256(data).hexdigest() != entry['sha256']:
                raise ValueError(f'package changed during installation: {source}')
            if entry['path'] == f'share/applications/{DESKTOP_ID}.desktop':
                data = desktop_content(data, prefix)
            actual = dict(entry, size=len(data), sha256=hashlib.sha256(data).hexdigest())
            info = create_new(target, data, entry['mode'])
            created.append((target, info, actual['sha256']))
            installed_entries.append(actual)
        manifest = dict(manifest, files=installed_entries)
        data = (json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode()
        info = create_new(receipt, data, 0o644)
        created.append((receipt, info, hashlib.sha256(data).hexdigest()))
    except Exception:
        for path, info, expected in reversed(created):
            if (not path.is_symlink() and path.is_file()
                    and (path.stat().st_dev, path.stat().st_ino) == (info.st_dev, info.st_ino)
                    and (path.stat().st_mode & 0o777) == (info.st_mode & 0o777)
                    and digest(path) == expected):
                path.unlink()
        for directory in sorted(made_dirs, key=lambda p: len(p.parts), reverse=True):
            try: directory.rmdir()
            except OSError: pass
        raise


def uninstall(prefix):
    check_prefix(prefix)
    receipt = checked_path(prefix, RECEIPT)
    manifest = json.loads(receipt.read_text())
    records = entries(manifest)
    targets = []
    for entry in records:
        path = checked_path(prefix, relative_name(entry['path']))
        if not matches(path, entry):
            raise ValueError(f'installed file changed or missing; nothing removed: {path}')
        targets.append(path)
    # Preserve user data, artwork and every file outside the recorded package.
    for path in targets:
        path.unlink()
    receipt.unlink()
    parents = {p for target in targets + [receipt] for p in target.parents if p.is_relative_to(prefix)}
    for parent in sorted(parents, key=lambda p: len(p.parts), reverse=True):
        try: parent.rmdir()
        except OSError: pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'uninstall'])
    parser.add_argument('--prefix', type=Path, required=True)
    args = parser.parse_args()
    if not args.prefix.is_absolute():
        parser.error('--prefix must be an absolute path')
    if args.action == 'install':
        install(Path(__file__).resolve().parent, args.prefix)
    else:
        uninstall(args.prefix)
    print(f'{args.action} complete: {args.prefix}')


if __name__ == '__main__':
    try: main()
    except (ValueError, KeyError, OSError) as error:
        raise SystemExit(str(error))
