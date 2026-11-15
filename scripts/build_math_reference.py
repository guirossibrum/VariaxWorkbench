"""Build ignored, offline math-audit helpers from pinned Apple source and MPFR.

No original Line 6 or Apple code is copied into the repository. Existing source
checkout and helpers are preserved. These are comparison candidates, not a claim
that Linux recompilation recreates an original macOS system runtime.
"""
import hashlib
import json
import shutil
import subprocess
from pathlib import Path
from native_oracle import ROOT
from inspect_firmware import preserve

APPLE_URL='https://github.com/apple-oss-distributions/Libm.git'
APPLE_COMMIT='0501897231e5e00031dac38fe44309a3881fe296'
ASSEMBLY=['s_sin','s_sinf','s_cos','s_cosf','s_tan','s_tanf','e_exp','e_expf','e_log','e_log10f','e_atan2f']


def build():
    source=ROOT/'analysis/extracted/apple-libm-92';output=ROOT/'.tools/math-reference/apple-libm-92'
    if not source.exists():subprocess.run(['git','clone','--depth','1','--branch','Libm-92',APPLE_URL,str(source)],check=True)
    assert subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()==APPLE_COMMIT
    assert not subprocess.check_output(['git','-C',str(source),'status','--porcelain'],text=True).strip(),'pinned Apple source has local changes'
    output.mkdir(parents=True,exist_ok=True);directory=source/'i386.subproj';objects=[];commands=[]
    def run(args):commands.append([str(x) for x in args]);subprocess.run(commands[-1],check=True)
    for name in ['e_pow.c','e_powf.c','math_private.h']:preserve(output/name,(directory/name).read_bytes())
    for name,symbol in [('e_pow','pow'),('e_powf','powf')]:
        obj=output/(name+'.o')
        run(['cc','-m64','-O2','-std=c99','-D_DEFAULT_SOURCE','-fPIC','-ffp-contract=off','-fno-builtin','-fno-strict-aliasing','-c',output/(name+'.c'),'-o',obj])
        run(['llvm-objcopy','--redefine-sym',f'{symbol}=apple92_{symbol}',obj]);objects.append(obj)
    for name in ASSEMBLY:
        obj=output/(name+'.o');run(['cc','-m64','-fPIC','-x','assembler-with-cpp','-I',directory,'-c',directory/(name+'.s'),'-o',obj]);objects.append(obj)
    apple=output/'libapple92-reference.so';run(['cc','-shared','-Wl,-z,noexecstack','-Wl,-Bsymbolic',*objects,'-lm','-o',apple])
    reference=output.parent/'libmpfr-reference.so';run(['cc','-m64','-O2','-fPIC','-shared',ROOT/'scripts/math_reference.c','-lmpfr','-lm','-o',reference])
    used=[directory/(name+'.s') for name in ASSEMBLY]+[directory/name for name in ['e_pow.c','e_powf.c','math_private.h','math.h','abi.h','machine/asm.h']]+[source/'APPLE_LICENSE']
    manifest={'apple_repository':APPLE_URL,'apple_tag':'Libm-92','apple_commit':APPLE_COMMIT,'compiler':subprocess.check_output(['cc','--version'],text=True).splitlines()[0],
        'commands':commands,'source_sha256':{str(p.relative_to(source)):hashlib.sha256(p.read_bytes()).hexdigest() for p in used},
        'helpers':{str(p.resolve()):hashlib.sha256(p.read_bytes()).hexdigest() for p in [apple,reference]},
        'limitations':['Apple source recompiled for Linux x86_64, not an original macOS runtime binary','C pow/powf compiled with SSE arithmetic and host headers; assembly uses the x86_64 ABI from the original source','Remaining fabs/scalbn/sqrt dependencies use host glibc; no host transcendental fallback is linked','The old assembly has different special-value behavior; candidate results are not canonical','No third-party source or compiled helper is published; preserve the upstream license in the local checkout']}
    # Paths can differ across disposable worktrees; content-addressed receipts
    # preserve the exact build rather than overwriting a previous environment.
    receipt=json.dumps(manifest,indent=2)+'\n';digest=hashlib.sha256(receipt.encode()).hexdigest()[:16]
    preserve(output/f'build-{digest}.json',receipt.encode())
    return apple.resolve(),reference.resolve(),manifest


if __name__=='__main__':
    apple,mpfr,_=build();print('Apple comparison helper:',apple);print('MPFR helper:',mpfr)
