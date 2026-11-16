"""Prepare an offline Workbench reference workspace without running installers.

Python standard library only. Original application/driver entry points are never
executed. Explicit reference commands run bounded private CPU emulation or file
readers; no device tool is in the allowlist. See docs/reproducing-references.md.
"""
import argparse
import gzip
import io
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import platform
import stat
import subprocess
import sys
import tempfile
import venv
import xml.etree.ElementTree as ET
import zipfile
from dependency_tools import verify_llvm_nm

ROOT = Path(__file__).resolve().parents[1]
LIMIT = 256 * 1024 * 1024
REFERENCES = [
    'verify_native_pack', 'verify_native_resonators', 'verify_native_pickups',
    'verify_native_bodies', 'verify_body_rust', 'verify_body_plans',
    'verify_body_interpolation', 'verify_pickup_combinations',
    'verify_pickup_response', 'verify_pickup_biquad', 'verify_pickup_secondary',
    'verify_pickup_tables', 'verify_pickup_calculated', 'verify_pickup_geometry',
    'verify_pickup_placement', 'verify_pickup_wiring', 'verify_pickup_levels',
    'verify_native_startup', 'verify_dsp_coefficients', 'verify_dsp_scalar',
    'verify_dsp_tables', 'verify_dsp_filters', 'verify_dsp_resonance',
    'verify_dsp_simple_blocks', 'verify_dsp_equalizer', 'verify_dsp_eq_blocks',
    'verify_dsp_pickup_blocks', 'verify_dsp_body_variants', 'verify_dsp_sitar',
    'test_coefficient_generator', 'verify_dsp_word_map', 'verify_host_pipeline', 'verify_host_synthetic',
    'verify_host_engine', 'verify_host_edge_cases', 'verify_host_complete',
    'verify_preset_compatibility', 'verify_bundle_matrix', 'verify_edit_domains',
]


def sha(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def write_new(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        if path.is_symlink() or path.read_bytes() != data:
            raise ValueError(f'refusing to replace different data: {path}')
        return
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent,delete=False) as output:
            temporary=Path(output.name)
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        try:
            os.link(temporary,path)
        except FileExistsError:
            if path.is_symlink() or path.read_bytes()!=data:
                raise ValueError(f'refusing a raced replacement: {path}')
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def receipt(name, value):
    value = dict(value, recipe_sha256=sha(Path(__file__)))
    data = (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()
    path = ROOT / 'analysis/reports' / (name + '-' + hashlib.sha256(data).hexdigest()[:16] + '.json')
    write_new(path, data)
    print(path)


def destination(root, name):
    relative = PurePosixPath(name)
    if '\\' in name or '\0' in name or ':' in name or relative.is_absolute() or '..' in relative.parts:
        raise ValueError(f'unsafe archive path: {name!r}')
    path = root.joinpath(*relative.parts)
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f'archive path escapes output: {name!r}')
    return path


def unzip(source, output):
    with zipfile.ZipFile(source) as archive:
        if len(archive.infolist()) > 100_000 or sum(i.file_size for i in archive.infolist()) > LIMIT:
            raise ValueError('ZIP exceeds research extraction limits')
        for member in archive.infolist():
            # ZipInfo.filename may normalize backslashes on Windows and remove
            # NUL suffixes. Validate the original archive name on every host.
            path = destination(output, member.orig_filename)
            if stat.S_ISLNK(member.external_attr >> 16):
                raise ValueError('ZIP symbolic links are not accepted')
            if member.is_dir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                write_new(path, archive.read(member))


def unpack_odc(data, output):
    """Read the old ASCII CPIO payload as data; never restore executable modes."""
    if len(data) > LIMIT:
        raise ValueError('CPIO exceeds research extraction limit')
    offset, records, links = 0, 0, []
    while offset + 76 <= len(data):
        header = data[offset:offset+76]
        if header[:6] != b'070707':
            raise ValueError('expected old ASCII CPIO header')
        mode = int(header[18:24], 8)
        namesize, size = int(header[59:65], 8), int(header[65:76], 8)
        offset += 76
        if not 1 <= namesize <= 4096 or offset + namesize + size > len(data):
            raise ValueError('truncated CPIO name/data')
        raw_name = data[offset:offset+namesize]
        if raw_name[-1:] != b'\0':
            raise ValueError('unterminated CPIO name')
        name = raw_name[:-1].decode('utf-8')
        offset += namesize
        payload = data[offset:offset+size]
        offset += size
        if name == 'TRAILER!!!':
            return {'regular_files': records, 'unmaterialized_links': links}
        path = destination(output, name)
        if stat.S_ISDIR(mode):
            path.mkdir(parents=True, exist_ok=True)
        elif stat.S_ISREG(mode):
            write_new(path, payload)
            records += 1
        elif stat.S_ISLNK(mode):
            # Record links; do not create filesystem redirections in an extract.
            links.append({'path': name, 'target': payload.decode('utf-8')})
        else:
            raise ValueError(f'unsupported CPIO node type for {name!r}')
    raise ValueError('CPIO trailer is missing')


def checked_artifact(directory, name):
    records = json.loads((ROOT / 'research/artifacts.json').read_text())['artifacts']
    record = next(item for item in records if item['name'] == name)
    path = directory / name
    if not path.is_file():
        raise ValueError(f'missing original artifact: {path}; supply the locally archived file, never run it')
    if path.stat().st_size != record['size'] or sha(path) != record['sha256']:
        raise ValueError(f'artifact identity mismatch: {path}; extraction refused')
    return path


def extract(installers, tools):
    windows = checked_artifact(installers, 'Variax Workbench v1.75 Installer.exe')
    mac = checked_artifact(installers, 'Variax Workbench 1.75.dmg')
    seven = tools / '7zip/7zz'
    if not seven.is_file():
        raise ValueError(f'missing 7-Zip tool: {seven}; see the research setup guide')
    output = ROOT / 'analysis/extracted'
    for name in ['windows-app', 'mac-app', 'mac-jar', 'macho-slices']:
        if (output / name).exists():
            raise ValueError(f'extraction destination already exists: {output/name}; use a fresh workspace')
    output.mkdir(parents=True, exist_ok=True)
    def seven_extract(source, target, *members):
        subprocess.run([str(seven), 'x', '-y', '-o'+str(target), str(source), *members],
                       check=True, stdout=subprocess.DEVNULL, timeout=120)
    with tempfile.TemporaryDirectory(prefix='extract-', dir=output) as temporary:
        stage = Path(temporary)
        seven_extract(windows, stage/'windows')
        unzip(stage/'windows/Variax Workbench/Variax Workbench.exe', output/'windows-app')
        seven_extract(mac, stage/'dmg', 'Variax Workbench/Variax Workbench.pkg')
        seven_extract(stage/'dmg/Variax Workbench/Variax Workbench.pkg', stage/'pkg')
        payload = (stage/'pkg/Variax_Workbench.pkg/Payload').read_bytes()
        with gzip.GzipFile(fileobj=io.BytesIO(payload)) as stream:
            plain = stream.read(LIMIT+1)
        cpio = unpack_odc(plain, output/'mac-app')
    jar = output/'mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar'
    if sha(jar) != '053b1a9c469472d4733711a102a3bddb9f4a105d1cdd70c3936a5885139f16ba':
        raise ValueError('extracted Mac JAR digest mismatch')
    unzip(jar, output/'mac-jar')
    from split_macho import slices
    records = []
    for source in sorted((output/'mac-jar').rglob('*.mac')):
        data = source.read_bytes()
        for cpu, offset, size in slices(data):
            arch = {7: 'i386', 18: 'ppc'}[cpu]
            path = output/'macho-slices'/f'{source.stem}.{arch}.macho'
            write_new(path, data[offset:offset+size])
            records.append({'source': str(source.relative_to(ROOT)), 'architecture': arch,
                            'offset': offset, 'size': size, 'output': str(path.relative_to(ROOT)), 'sha256': sha(path)})
    target = output/'macho-slices/L6VariaxCoefEngine.jnilib.i386.macho'
    if sha(target) != json.loads((ROOT/'research/native-functions.json').read_text())['target_sha256']:
        raise ValueError('native coefficient slice identity mismatch')
    write_new(ROOT/'analysis/reports/macho-slices.json', (json.dumps(records,indent=2)+'\n').encode())
    images = []
    for source in sorted((output/'windows-app').rglob('*')):
        if source.is_file() and source.suffix.lower() in ['.png','.gif','.jpg','.jpeg','.bmp','.ico']:
            path = ROOT/'assets/original-ui'/source.relative_to(output/'windows-app')
            write_new(path, source.read_bytes())
            images.append({'path': str(path.relative_to(ROOT)), 'sha256': sha(path)})
    receipt('research-extraction', {'mac_installer_sha256': sha(mac), 'windows_installer_sha256': sha(windows),
                                  'jar_sha256': sha(jar), 'cpio': cpio, 'slices': records, 'images': images,
                                  'installers_executed': False, 'device_access': False})


def catalog(tools):
    jar = ROOT/'analysis/extracted/mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar'
    javap = tools/'jdk/bin/javap'
    if not jar.is_file() or not javap.is_file():
        raise ValueError('catalog requires extracted Mac JAR and JDK javap; run extraction/tool setup first')
    text = subprocess.check_output([str(javap), '-constants', '-classpath', str(jar),
                                    'cmn.l6java.L6SP.L6AssetDefs_const'], text=True, timeout=60)
    constants = {name:int(value) for name,value in re.findall(r'public static final int (\w+) = (-?\d+);', text)}
    directory = ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng'
    records = []
    for item in ET.parse(directory/'data/definitions.xml').getroot():
        if item.tag not in ['body','pickup']:
            continue
        symbol = item.attrib['ID']
        try:
            identifier = int(symbol, 0)
        except ValueError:
            identifier = constants[symbol]
        path = destination(directory, item.attrib['file'])
        records.append({'kind':item.tag, 'name':item.attrib['name'], 'id_symbol':symbol,
                        'id':identifier, 'path':str(path.relative_to(ROOT)), 'size':path.stat().st_size, 'sha256':sha(path)})
    if sum(x['kind']=='body' for x in records) != 28 or sum(x['kind']=='pickup' for x in records) != 43:
        raise ValueError('unexpected model catalog counts')
    write_new(ROOT/'analysis/reports/model-catalog.json', (json.dumps(records,indent=2)+'\n').encode())
    print('PASS: catalog has 28 body and 43 pickup definitions; no class was executed')


def index():
    metadata = ROOT/'research/native-functions.json'
    record = json.loads(metadata.read_text())
    path = ROOT/'analysis/decompiled/native-coef/decompile-index.json'
    if path.exists():
        existing = json.loads(path.read_text())
        fields = lambda d: [{k:f[k] for k in ['address','name','status','body'] if k in f} for f in d['functions']]
        if existing['target_sha256'] != record['target_sha256'] or fields(existing) != fields(record):
            raise ValueError('existing REA metadata differs; original index retained')
    else:
        write_new(path, metadata.read_bytes())
    print('PASS: digest-bound derived procedure index prepared; no pseudocode installed')


def doctor(tools, installers):
    failures, checks = [], {'host': {'system': platform.system(), 'machine': platform.machine(), 'python': sys.version, 'libc': platform.libc_ver()}}
    for name, args in [('jdk',['-version']), ('python',['--version']), ('7zip',[])]:
        relative = {'jdk':'jdk/bin/java','python':'python/bin/python','7zip':'7zip/7zz'}[name]
        path = tools/relative
        if not path.is_file():
            failures.append(f'missing {name}: {path}')
        else:
            result = subprocess.run([str(path),*args],capture_output=True,text=True,timeout=30)
            checks[name] = (result.stdout+result.stderr).strip().splitlines()[:3]
            if result.returncode != 0:
                failures.append(f'{name} version command failed')
            text = result.stdout+result.stderr
            if name == 'jdk' and '17.0.21' not in text:
                failures.append('reference JDK differs from tested 17.0.21; verify it before reproducing')
            if name == '7zip' and '26.06' not in text:
                failures.append('extractor differs from tested 7-Zip 26.06; verify it before reproducing')
    for command in [['rustc','--version'],['cargo','--version'],['cc','--version']]:
        try:
            result=subprocess.run(command,capture_output=True,text=True,timeout=30,check=True)
            checks[command[0]]=result.stdout.strip().splitlines()[:1]
            if command[0]=='rustc' and not result.stdout.startswith('rustc 1.99.1 '):
                failures.append('compiler differs from the tested Rust 1.99.1 profile')
        except (OSError,subprocess.SubprocessError) as error:
            failures.append(f'missing/unusable build tool {command[0]}: {error}')
    try:
        checks['llvm-nm'] = verify_llvm_nm().strip().splitlines()[:2]
    except RuntimeError as error:
        failures.append(str(error))
    for name in ['Variax Workbench 1.75.dmg','Variax Workbench v1.75 Installer.exe']:
        try:
            checks[name] = sha(checked_artifact(installers,name))
        except (ValueError,OSError) as error:
            failures.append(str(error))
    print(json.dumps({'checks':checks,'failures':failures,'device_access':False},indent=2))
    if failures:
        raise ValueError('research prerequisites incomplete; see docs/reproducing-references.md')


def references(steps):
    if any(name not in REFERENCES for name in steps):
        raise ValueError('not an approved offline reference step; no command was run')
    python = ROOT/'.tools/python/bin/python'
    if not python.is_file():
        raise ValueError('missing .tools/python; bootstrap or attach the pinned research environment first')
    version = subprocess.check_output([str(python), '-c', 'import unicorn; print(unicorn.__version__)'],text=True).strip()
    if version != '2.1.4':
        raise ValueError(f'expected Unicorn 2.1.4, found {version}')
    # Existing reference helpers regenerate fixtures. Capture before/after hashes
    # in this disposable workspace; never promote changed expectations silently.
    for required in ['analysis/reports/model-catalog.json','analysis/decompiled/native-coef/decompile-index.json',
                     'analysis/extracted/macho-slices/L6VariaxCoefEngine.jnilib.i386.macho']:
        if not (ROOT/required).is_file():
            raise ValueError(f'missing {required}; run extraction, catalog and index first')
    before = {str(p.relative_to(ROOT)):sha(p) for p in (ROOT/'tests/fixtures').rglob('*') if p.is_file()}
    directory = ROOT/'analysis/reports/reference-replay'
    directory.mkdir(parents=True,exist_ok=True)
    completed, failed = [], None
    environment = dict(os.environ, CARGO_NET_OFFLINE='true', PYTHONDONTWRITEBYTECODE='1')
    for name in steps:
        if name not in REFERENCES:
            raise ValueError(f'not an approved offline reference step: {name}')
        log = directory/(name+'.log')
        if log.exists():
            raise ValueError(f'replay log exists; use a fresh workspace: {log}')
        print(f'Running {name}; log: {log}',flush=True)
        with log.open('x') as output:
            try:
                subprocess.run([str(python),str(ROOT/'scripts'/f'{name}.py')],cwd=ROOT,
                               stdout=output,stderr=subprocess.STDOUT,check=True,timeout=1800,env=environment)
            except (subprocess.CalledProcessError,subprocess.TimeoutExpired) as error:
                failed={'step':name,'error':type(error).__name__,'exit_code':getattr(error,'returncode',None)}
        if failed:
            failed['log_sha256']=sha(log)
            break
        completed.append({'step':name,'log_sha256':sha(log)})
    after = {str(p.relative_to(ROOT)):sha(p) for p in (ROOT/'tests/fixtures').rglob('*') if p.is_file()}
    changes = [name for name in sorted(before.keys()|after.keys()) if before.get(name)!=after.get(name)]
    receipt('reference-replay',{'completed':completed,'failed':failed,'fixture_changes':changes,
                               'recipe_sha256':sha(Path(__file__)),'device_access':False})
    if failed:
        raise ValueError(f"reference {failed['step']} failed; retained log: {directory/(failed['step']+'.log')}")
    if changes:
        raise ValueError('fixture outputs changed in the disposable workspace; review receipt, do not promote automatically')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tools',type=Path,default=ROOT/'.tools')
    parser.add_argument('--installers',type=Path,default=ROOT/'vendor/installers')
    sub = parser.add_subparsers(dest='command',required=True)
    for name in ['doctor','extract','catalog','index']:
        sub.add_parser(name)
    link = sub.add_parser('attach-tools');link.add_argument('source',type=Path)
    bootstrap = sub.add_parser('bootstrap-python');bootstrap.add_argument('--wheels',type=Path,required=True)
    run = sub.add_parser('references');run.add_argument('steps',nargs='*',choices=REFERENCES)
    args = parser.parse_args()
    if args.command == 'doctor': doctor(args.tools,args.installers)
    elif args.command == 'extract': extract(args.installers,args.tools)
    elif args.command == 'catalog': catalog(args.tools)
    elif args.command == 'index': index()
    elif args.command == 'references': references(args.steps or REFERENCES)
    elif args.command == 'attach-tools':
        for name in ['jdk','7zip','jdk25','node','ghidra']:
            source = (args.source/name).resolve();target = ROOT/'.tools'/name
            if not source.is_dir():
                if name in ['jdk','7zip']: raise ValueError(f'missing tool directory: {source}')
                continue
            target.parent.mkdir(parents=True,exist_ok=True)
            if target.exists():
                if target.resolve()!=source: raise ValueError(f'refusing to replace tool: {target}')
            else: target.symlink_to(source,target_is_directory=True)
    elif args.command == 'bootstrap-python':
        target = ROOT/'.tools/python'
        if target.exists(): raise ValueError('Python environment exists; refusing to replace it')
        venv.EnvBuilder(with_pip=True).create(target)
        subprocess.run([str(target/'bin/python'),'-m','pip','install','--no-index','--find-links',str(args.wheels),
                        '--require-hashes','-r',str(ROOT/'research/requirements-linux-x86_64.txt')],check=True)


if __name__ == '__main__':
    try:
        main()
    except (ValueError,OSError,KeyError,subprocess.SubprocessError,zipfile.BadZipFile) as error:
        print('Research setup stopped: '+str(error),file=sys.stderr)
        sys.exit(1)
