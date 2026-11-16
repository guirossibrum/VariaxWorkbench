"""Owned arithmetic-only x87 helper for research on one current x86-64 CPU.

No Apple/Line 6 code is loaded natively. This is not an application dependency
or a claim of bit-identical behavior on every historical Intel processor.
"""
import ctypes
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess

from inspect_firmware import preserve


class NativeX87:
    def __init__(self):
        if platform.machine() != 'x86_64':
            raise RuntimeError('native x87 research helper is scoped to this x86-64 build')
        root = Path(__file__).resolve().parents[1]
        source = root / 'scripts/x87_instruction_reference.c'
        compiler = shutil.which('gcc')
        if not compiler:
            raise RuntimeError('research compiler gcc is unavailable')
        flags = ['-shared','-fPIC','-O2','-fno-builtin','-fno-stack-protector',
                 '-ffp-contract=off','-frounding-math','-nostdlib']
        version = subprocess.check_output([compiler,'--version'],text=True).splitlines()[0]
        spec = {'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),
                'compiler':version,'flags':flags,'machine':platform.machine()}
        key = hashlib.sha256(json.dumps(spec,sort_keys=True).encode()).hexdigest()[:16]
        folder = root / '.tools/native-x87-reference' / key
        folder.mkdir(parents=True,exist_ok=True)
        library = folder / 'reference.so'
        if library.exists() and not (folder/'build.json').is_file():
            raise RuntimeError('cached research helper has no verified build record')
        if not library.exists():
            temp = folder / 'new-reference.so'
            if temp.exists():
                raise RuntimeError('retained incomplete helper build; inspect it before retry')
            subprocess.run([compiler,*flags,str(source),'-o',str(temp)],check=True)
            if subprocess.check_output(['nm','-u',str(temp)],text=True).strip():
                raise RuntimeError('owned x87 helper unexpectedly imports external functions')
            preserve(library,temp.read_bytes())
            temp.unlink()
        if subprocess.check_output(['nm','-u',str(library)],text=True).strip():
            raise RuntimeError('cached x87 helper imports external functions')
        self.record = dict(spec, library_sha256=hashlib.sha256(library.read_bytes()).hexdigest(),
                           scope='owned native x87 arithmetic only; no vendor code, libm or device API')
        preserve(folder/'build.json',(json.dumps(self.record,indent=2)+'\n').encode())
        self.library = ctypes.CDLL(str(library))
        self.function = self.library.x87_reference
        self.function.argtypes = [ctypes.c_int,ctypes.c_double,ctypes.c_int]
        self.function.restype = ctypes.c_double

    def evaluate(self, name, value):
        if name not in ('sin','sinf','cos','cosf','tan','tanf') or not -1048576 < value < 1048576:
            raise ValueError('input is outside the bounded native-instruction comparison')
        operation = 0 if name.startswith('sin') else (1 if name.startswith('cos') else 2)
        return self.function(operation,value,int(name.endswith('f')))
