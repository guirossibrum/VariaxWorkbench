"""Build/test both real product math profiles without replacing normal outputs."""
import hashlib
import json
import os
import subprocess
import tomllib
from pathlib import Path
from native_oracle import ROOT
from inspect_firmware import preserve
from build_rust_math_reference import verify_cached_source,ARCHIVE_SHA256
from audit_engine_math import verify


def main():
    dependency,files=verify_cached_source()
    names=['Cargo.toml','Cargo.lock']+[str(p.relative_to(ROOT)) for p in sorted((ROOT/'src').rglob('*.rs'))]
    hashes={name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in names}
    source_id=hashlib.sha256(json.dumps(hashes,sort_keys=True).encode()).hexdigest()
    directory=(ROOT/'.tools/product-engine-math-v1'/source_id[:16]).resolve();directory.mkdir(parents=True,exist_ok=True)
    env=os.environ.copy();env.pop('RUSTFLAGS',None);env.pop('CARGO_ENCODED_RUSTFLAGS',None);env['CARGO_TARGET_DIR']=str(directory/'target')
    package=next(p for p in tomllib.loads((ROOT/'Cargo.lock').read_text())['package'] if p['name']=='libm')
    if package['version']!='0.2.16' or package['checksum']!=ARCHIVE_SHA256:raise RuntimeError('unexpected pinned math dependency')
    metadata=json.loads(subprocess.check_output(['cargo','metadata','--offline','--locked','--features','portable-math','--filter-platform','x86_64-unknown-linux-gnu','--format-version','1'],cwd=ROOT,env=env,text=True))
    resolved=next(p for p in metadata['packages'] if p['name']=='libm')
    if Path(resolved['manifest_path']).resolve()!=(dependency/'Cargo.toml').resolve():raise RuntimeError('resolved unchecked math source')
    binaries={};commands=[]
    for software in [False,True]:
        for release in [False,True]:
            label=('software' if software else 'identity')+('-release' if release else '-debug')
            command=['cargo','build','--offline','--locked','--bin','variax-workbench']
            if software:command.extend(['--features','portable-math'])
            if release:command.append('--release')
            subprocess.run(command,cwd=ROOT,env=env,check=True);commands.append(command)
            built=directory/'target'/('release' if release else 'debug')/'variax-workbench';copy=directory/label
            preserve(copy,built.read_bytes());copy.chmod(0o755)
            expected='software-wide-v1/libm-0.2.16' if software else 'host-std-v1'
            if subprocess.check_output([str(copy),'math-profile'],text=True).strip()!=expected:raise RuntimeError('built binary has the wrong math profile')
            binaries[label]=copy
    current={name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in names}
    if current!=hashes:raise RuntimeError('product source changed during audit builds')
    receipt={'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'source_sha256':hashes,'source_fingerprint':source_id,
        'math_archive_sha256':ARCHIVE_SHA256,'math_source_files_verified':files,'compiler':subprocess.check_output(['rustc','-vV'],text=True),'commands':commands,
        'scope':'Actual product CLI binaries with explicit immutable build profiles; default math remains host-std-v1'}
    preserve(directory/'source-receipt.json',(json.dumps(receipt,indent=2)+'\n').encode())
    verify(binaries,directory,receipt,scope='Actual experimental product math profile, compared against original instructions with matching math adapters. Not cross-target, historical Mac authentication or audio parity.')


if __name__=='__main__':main()
