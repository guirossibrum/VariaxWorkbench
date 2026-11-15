"""Build pinned software-only Rust math into ignored research helpers.

The exact crate archive and every unpacked source file are checked against the
pinned archive checksum already recorded in the application lockfile. No
dependency is added to the application manifest.
Cargo runs offline and locked after generating this helper's independent lock.
"""
import hashlib
import json
import os
import subprocess
import tarfile
from pathlib import Path
from native_oracle import ROOT
from inspect_firmware import preserve

VERSION='0.2.16'
ARCHIVE_SHA256='b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981'


def verify_cached_source():
    cargo_home=Path(os.environ.get('CARGO_HOME',Path.home()/'.cargo'))
    candidates=list((cargo_home/'registry/src').glob(f'*/libm-{VERSION}'))
    if len(candidates)!=1:raise RuntimeError('one cached libm-0.2.16 source tree is required; no automatic download')
    source=candidates[0];registry=source.parent.name
    archive=cargo_home/'registry/cache'/registry/f'libm-{VERSION}.crate'
    if hashlib.sha256(archive.read_bytes()).hexdigest()!=ARCHIVE_SHA256:raise RuntimeError('cached crate archive checksum mismatch')
    files=0
    with tarfile.open(archive,'r:gz') as tar:
        for member in tar:
            if member.isdir():continue
            path=Path(member.name)
            if not member.isfile() or path.is_absolute() or '..' in path.parts or path.parts[0]!=f'libm-{VERSION}':raise RuntimeError('unexpected crate archive entry')
            expected=tar.extractfile(member).read()
            if (source/Path(*path.parts[1:])).read_bytes()!=expected:raise RuntimeError('cached crate source checksum mismatch: '+member.name)
            files+=1
    return source,files


def build():
    source,files=verify_cached_source()
    directory=ROOT/'.tools/rust-math-reference';directory.mkdir(parents=True,exist_ok=True)
    manifest='''[package]
name = "variax-math-research"
version = "0.0.0"
edition = "2024"
publish = false
[workspace]
[lib]
crate-type = ["cdylib"]
path = "reference.rs"
[dependencies]
libm = { version = "=0.2.16", default-features = false, features = ["force-soft-floats"] }
[profile.dev]
panic = "abort"
[profile.release]
panic = "abort"
'''
    preserve(directory/'Cargo.toml',manifest.encode())
    helper=(ROOT/'scripts/rust_math_reference.rs').read_bytes()
    # Content-addressed sources keep earlier helper builds reproducible.
    if (directory/'reference.rs').exists() and (directory/'reference.rs').read_bytes()!=helper:
        raise RuntimeError('helper source changed; use a new versioned build directory')
    preserve(directory/'reference.rs',helper)
    command=['cargo','generate-lockfile','--offline','--manifest-path',str(directory/'Cargo.toml')]
    if not (directory/'Cargo.lock').exists():subprocess.run(command,check=True)
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--offline','--locked','--format-version','1','--manifest-path',str(directory/'Cargo.toml')],text=True))
    packages=[p for p in metadata['packages'] if p['name']=='libm']
    if len(packages)!=1 or Path(packages[0]['manifest_path']).resolve()!=(source/'Cargo.toml').resolve():raise RuntimeError('Cargo resolved a different math source tree')
    node=next(n for n in metadata['resolve']['nodes'] if n['id']==packages[0]['id'])
    if node['features']!=['force-soft-floats'] or node['deps']:raise RuntimeError('unexpected math features or transitive dependencies')
    commands=[];libraries={};imports={}
    # Isolate compiler flags and output from the application and other agents.
    environment=os.environ.copy();environment.pop('RUSTFLAGS',None);environment.pop('CARGO_ENCODED_RUSTFLAGS',None)
    environment['CARGO_TARGET_DIR']=str(directory/'target')
    for profile in ['debug','release']:
        args=['cargo','build','--offline','--locked','--manifest-path',str(directory/'Cargo.toml')]
        if profile=='release':args.append('--release')
        subprocess.run(args,check=True,env=environment);commands.append(args)
        library=directory/'target'/profile/'libvariax_math_research.so'
        symbols=subprocess.check_output(['llvm-nm','--undefined-only',str(library)],text=True)
        names={line.split()[-1].split('@')[0] for line in symbols.splitlines() if line.split()}
        math_names={'sin','sinf','cos','cosf','tan','tanf','exp','expf','log','logf','log10','log10f','pow','powf','atan2','atan2f','sqrt','sqrtf','fma','fmaf'}
        if names&math_names:raise RuntimeError('software candidate imported host math: '+repr(names&math_names))
        libraries[profile]=library.resolve();imports[profile]=symbols
    receipt={'crate':'libm','version':VERSION,'archive_sha256':ARCHIVE_SHA256,'source_files_verified':files,
        'license_sha256':hashlib.sha256((source/'LICENSE.txt').read_bytes()).hexdigest(),
        'features':['force-soft-floats'],'default_features':False,'rustc':subprocess.check_output(['rustc','-vV'],text=True),
        'resolved_manifest':packages[0]['manifest_path'],'resolved_features':node['features'],
        'helper_sha256':hashlib.sha256(helper).hexdigest(),'lockfile_sha256':hashlib.sha256((directory/'Cargo.lock').read_bytes()).hexdigest(),
        'commands':commands,'undefined_symbols':imports,'libraries':{k:{'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for k,p in libraries.items()},
        'limitations':['Linux x86_64 helper builds only; other target/compiler combinations untested','Pure Rust does not imply original-platform rounding parity','Product manifest and runtime dependencies unchanged']}
    data=(json.dumps(receipt,indent=2)+'\n').encode();digest=hashlib.sha256(data).hexdigest()[:16]
    preserve(directory/f'build-{digest}.json',data)
    return libraries,receipt


if __name__=='__main__':
    libraries,_=build()
    for k,v in libraries.items():print(k,v)
