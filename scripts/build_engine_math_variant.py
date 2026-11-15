"""Generate an immutable, isolated Rust engine math experiment from Git.

The product sources/manifest/lockfile remain untouched. A counted rewrite only
renames math method calls; a trait delegates either to the original methods or
to software float64 math. Original arithmetic grouping and field types remain.
"""
import collections
import hashlib
import io
import json
import os
import re
import subprocess
import tarfile
import tomllib
from pathlib import Path
from native_oracle import ROOT
from inspect_firmware import preserve
from build_rust_math_reference import verify_cached_source,ARCHIVE_SHA256

EXPECTED={'sqrt':2,'ln':2,'exp':16,'tan':10,'cos':1,'sin':1,'powf':11,'log10':1}
METHOD=re.compile(r'\.(powf|exp|ln|log10|sin|cos|tan|sqrt)\(')
BASELINE_REVISION='04045febeda33e7205d83c589b75968ebfcf3c60'


def prepare(revision=BASELINE_REVISION):
    dependency,files=verify_cached_source()
    revision=subprocess.check_output(['git','rev-parse',revision],cwd=ROOT,text=True).strip()
    archive=subprocess.check_output(['git','archive',revision],cwd=ROOT)
    recipe=Path(__file__).read_bytes()+(ROOT/'scripts/engine_math_profile.rs').read_bytes()
    digest=hashlib.sha256(recipe).hexdigest()
    directory=(ROOT/'.tools/engine-math-audit-v1'/revision/digest[:16]).resolve()
    source=directory/'source';counts=collections.Counter();patches={};payloads={}
    with tarfile.open(fileobj=io.BytesIO(archive),mode='r:') as tar:
        for member in tar:
            if member.isdir():continue
            name=Path(member.name)
            if not member.isfile() or name.is_absolute() or '..' in name.parts:raise RuntimeError('unsupported Git archive entry')
            data=tar.extractfile(member).read()
            if name.parts[0]=='src' and name.suffix=='.rs':
                original=data.decode();production,separator,tests=original.partition('#[cfg(test)]')
                found=collections.Counter(METHOD.findall(production))
                if found:
                    production=METHOD.sub(lambda m:'.audit_'+m[1]+'(',production)
                    data=(production+separator+tests+'\nuse crate::math_audit::AuditMath as _;\n').encode()
                    counts.update(found);patches[str(name)]={'methods':dict(found),'original_sha256':hashlib.sha256(original.encode()).hexdigest(),'variant_sha256':hashlib.sha256(data).hexdigest()}
            payloads[str(name)]=data
    if dict(counts)!=EXPECTED:raise RuntimeError(('math call-site inventory changed',dict(counts),EXPECTED))
    manifest=payloads['Cargo.toml'].decode()
    if '[features]\n' not in manifest or '[dependencies]\n' not in manifest:raise RuntimeError('unexpected manifest layout')
    manifest=manifest.replace('[features]\n','[features]\nresearch-wide-math = ["dep:libm"]\n',1)
    manifest=manifest.replace('[dependencies]\n','[dependencies]\nlibm = { version = "=0.2.16", optional = true, default-features = false, features = ["force-soft-floats"] }\n',1)
    payloads['Cargo.toml']=manifest.encode()
    lock=payloads['Cargo.lock'].decode();before=tomllib.loads(lock)
    chunks=lock.split('[[package]]');indices=[i for i,c in enumerate(chunks) if re.match(r'\s*name = "variax-workbench"',c)]
    if len(indices)!=1 or ' "image",\n' not in chunks[indices[0]]:raise RuntimeError('unexpected root lockfile entry')
    chunks[indices[0]]=chunks[indices[0]].replace(' "image",\n',' "image",\n "libm",\n',1)
    lock='[[package]]'.join(chunks);after=tomllib.loads(lock)
    for package in before['package']:
        if package['name']=='variax-workbench':package['dependencies'].append('libm');package['dependencies'].sort()
    if before!=after:raise RuntimeError('lockfile changed outside the root dependency list')
    payloads['Cargo.lock']=lock.encode();payloads['src/lib.rs']+=b'\nmod math_audit;\n'
    payloads['src/math_audit.rs']=(ROOT/'scripts/engine_math_profile.rs').read_bytes()
    for name,data in payloads.items():
        path=source/name;path.parent.mkdir(parents=True,exist_ok=True);preserve(path,data)
    environment=os.environ.copy();environment.pop('RUSTFLAGS',None);environment.pop('CARGO_ENCODED_RUSTFLAGS',None)
    environment['CARGO_TARGET_DIR']=str(directory/'target')
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--offline','--locked','--all-features','--filter-platform','x86_64-unknown-linux-gnu','--format-version','1','--manifest-path',str(source/'Cargo.toml')],env=environment,text=True))
    package=next(p for p in metadata['packages'] if p['name']=='libm')
    if Path(package['manifest_path']).resolve()!=(dependency/'Cargo.toml').resolve():raise RuntimeError('candidate resolved unchecked math source')
    node=next(n for n in metadata['resolve']['nodes'] if n['id']==package['id'])
    if 'force-soft-floats' not in node['features'] or node['deps']:raise RuntimeError('candidate math source/configuration changed')
    receipt={'source_revision':revision,'git_archive_sha256':hashlib.sha256(archive).hexdigest(),'recipe_sha256':digest,
        'rewritten_calls':dict(counts),'rewritten_files':patches,'math_archive_sha256':ARCHIVE_SHA256,'math_source_files_verified':files,
        'resolved_features':node['features'],'rustc':subprocess.check_output(['rustc','-vV'],text=True),
        'generated_lock_sha256':hashlib.sha256(payloads['Cargo.lock']).hexdigest(),
        'scope':'Generated source variant only; production source, manifest and default math remain unchanged'}
    preserve(directory/'source-receipt.json',(json.dumps(receipt,indent=2)+'\n').encode())
    return directory,source,environment,receipt


def build(directory,source,environment,*,software=False,release=False):
    command=['cargo','build','--offline','--locked','--manifest-path',str(source/'Cargo.toml'),'--bin','variax-workbench']
    if software:command.extend(['--features','research-wide-math'])
    if release:command.append('--release')
    subprocess.run(command,env=environment,check=True)
    profile='release' if release else 'debug';name=('software' if software else 'identity')+'-'+profile
    binary=directory/'target'/profile/'variax-workbench';copied=directory/'binaries'/name
    copied.parent.mkdir(parents=True,exist_ok=True);preserve(copied,binary.read_bytes());copied.chmod(0o755)
    return copied.resolve()


if __name__=='__main__':
    directory,source,environment,_=prepare()
    for software in [False,True]:print(build(directory,source,environment,software=software))
