"""Build an explicit offline Linux package; never install or contact hardware."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'packaging/linux'))
from install import APP, BINARIES, DESKTOP_ID, entries, relative_name


def command(args, **kwargs):
    return subprocess.check_output(args, cwd=ROOT, text=True, **kwargs).strip()


def archive(directory, output, epoch):
    """Stable ordering, modes, ownership and timestamps for identical payloads."""
    if output.exists():
        raise ValueError(f'refusing to replace archive: {output}')
    with tempfile.NamedTemporaryFile(dir=output.parent, delete=False) as temporary:
        staged = Path(temporary.name)
    try:
        with staged.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=epoch) as zipped:
            with tarfile.open(fileobj=zipped, mode='w', format=tarfile.PAX_FORMAT) as tar:
                for source in [directory, *sorted(directory.rglob('*'))]:
                    if source.is_symlink():
                        raise ValueError('package contains a symbolic link')
                    info = tar.gettarinfo(str(source), str(source.relative_to(directory.parent)))
                    info.uid = info.gid = 0
                    info.uname = info.gname = ''
                    info.mtime = epoch
                    info.pax_headers = {}
                    info.mode = 0o755 if source.is_dir() or source.stat().st_mode & 0o111 else 0o644
                    if source.is_file():
                        with source.open('rb') as data: tar.addfile(info, data)
                    else: tar.addfile(info)
        os.link(staged, output)
    finally:
        staged.unlink(missing_ok=True)


def build(output, math_profile, desktop_backend='x11'):
    if platform.system() != 'Linux':
        raise ValueError('this package profile builds on Linux only')
    if output.exists():
        raise ValueError('output exists; choose a new package archive path')
    if command(['git', 'status', '--porcelain', '--untracked-files=normal']):
        raise ValueError('commit source changes before building a versioned package')
    for name in ['src/usb.rs', 'src/gui.rs']:
        if 'const LIVE_ACCESS_LOCKED: bool = true;' not in (ROOT/name).read_text():
            raise ValueError(f'hardware lock is not enabled in {name}')
    version = tomllib.loads((ROOT/'Cargo.toml').read_text())['package']['version']
    revision = command(['git', 'rev-parse', 'HEAD'])
    epoch = int(os.environ.get('SOURCE_DATE_EPOCH', command(['git', 'show', '-s', '--format=%ct', 'HEAD'])))
    rustc = command(['rustc', '-vV'])
    target = next(line.removeprefix('host: ') for line in rustc.splitlines() if line.startswith('host: '))
    features = 'gui' + (',portable-math' if math_profile == 'software' else '')
    environment = dict(os.environ, CARGO_TARGET_DIR=str(ROOT/'target'), CARGO_NET_OFFLINE='true',
                       SOURCE_DATE_EPOCH=str(epoch))
    environment['RUSTFLAGS'] = environment.get('RUSTFLAGS', '') + f' --remap-path-prefix={ROOT}=/variax-workbench'
    subprocess.run(['cargo', 'build', '--offline', '--locked', '--release', '--features', features, '--bins'],
                   cwd=ROOT, env=environment, check=True)
    # This command only reports the compiled math profile; no device API is used.
    profile = command([str(ROOT/'target/release/variax-workbench'), 'math-profile'])
    metadata = json.loads(command(['cargo', 'metadata', '--offline', '--locked', '--format-version=1',
                                   '--features', features, '--filter-platform', target]))
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent, prefix='package-stage-') as stage:
        directory = Path(stage)/f'{APP}-{version}-{target}-{math_profile}'
        directory.mkdir()
        records = []
        def put(name, data, mode=0o644):
            relative = relative_name(name)
            path = directory/'files'/relative
            path.parent.mkdir(parents=True, exist_ok=True)
            with path.open('xb') as stream: stream.write(data)
            path.chmod(mode)
            records.append({'path':name, 'size':len(data), 'mode':mode,
                            'sha256':hashlib.sha256(data).hexdigest()})
        for name in sorted(BINARIES):
            put('bin/'+name, (ROOT/'target/release'/name).read_bytes(), 0o755)
        for folder, suffix in [('applications','desktop'), ('icons/hicolor/scalable/apps','svg'), ('mime/packages','xml')]:
            name = DESKTOP_ID+'.'+suffix
            data = (ROOT/'packaging/linux'/name).read_bytes()
            if suffix == 'desktop':
                data = data.replace(b'--display x11', ('--display '+desktop_backend).encode())
            put(f'share/{folder}/{name}', data)
        for name in ['README.md', 'Cargo.lock', 'docs/math-profiles.md', 'docs/linux-package.md']:
            put(f'share/doc/{APP}/'+Path(name).name, (ROOT/name).read_bytes())
        for source in sorted((ROOT/'third_party').rglob('*')):
            if source.is_file():
                put(f'share/doc/{APP}/third_party/'+source.relative_to(ROOT/'third_party').as_posix(), source.read_bytes())
        dependencies = []
        for package in sorted(metadata['packages'], key=lambda p:p['id']):
            if package.get('source') is None: continue
            source_root = Path(package['manifest_path']).parent
            identifier = package['name']+'-'+package['version']
            notices = []
            for source in sorted(source_root.rglob('*')):
                if (source.is_file() and not source.is_symlink()
                        and any(word in source.name.upper() for word in ['LICENSE','LICENCE','COPYING','NOTICE','COPYRIGHT'])):
                    relative = source.relative_to(source_root).as_posix()
                    name = f'share/doc/{APP}/dependency-notices/{identifier}/{relative}'
                    put(name, source.read_bytes());notices.append(relative)
            dependencies.append({'name':package['name'], 'version':package['version'],
                                 'license':package.get('license'), 'source':package['source'],
                                 'notice_files':notices})
        put(f'share/doc/{APP}/dependencies.json', (json.dumps(dependencies, indent=2, sort_keys=True)+'\n').encode())
        manifest = {'application':APP, 'format':1, 'version':version, 'revision':revision,
                    'target':target, 'rustc':rustc, 'math_profile':profile, 'desktop_backend':desktop_backend, 'source_date_epoch':epoch,
                    'hardware_access_locked':True, 'vendor_resources_included':False,
                    'files':sorted(records, key=lambda x:x['path'])}
        entries(manifest)
        (directory/'manifest.json').write_text(json.dumps(manifest, indent=2, sort_keys=True)+'\n')
        shutil.copyfile(ROOT/'packaging/linux/install.py', directory/'install.py')
        (directory/'install.py').chmod(0o755)
        # Archive creation is non-overwriting; install is a separate explicit step.
        archive(directory, output, epoch)
    print(output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--math-profile', choices=['host', 'software'], default='host')
    parser.add_argument('--desktop-backend', choices=['x11', 'auto', 'wayland'], default='x11')
    args = parser.parse_args()
    try: build(args.output.absolute(), args.math_profile, args.desktop_backend)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error))
