"""Check Rust passive-pickup combination against original machine code.

Original CombinePickups runs directly in Unicorn, without math or OS adapters.
The raw circuit combination is only one part of pickup response generation.
No device access, transfer, response-table fitting or audio emulation occurs.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_MXCSR
from native_oracle import NativeOracle, ROOT


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=NativeOracle();oracle.cpu.reg_write(UC_X86_REG_MXCSR,0x1f80)
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/pickups').glob('*.bin'))
    assert len(paths)==43
    native_cases=0
    def compare(left,right,series):
        nonlocal native_cases
        a=left.read_bytes()[:32];b=right.read_bytes()[:32]
        oracle.write(oracle.OBJECT+0x1000,b''.join(a[i:i+8][::-1] for i in range(0,32,8)))
        oracle.write(oracle.OBJECT+0x1100,b''.join(b[i:i+8][::-1] for i in range(0,32,8)))
        oracle.call('0x23b30',oracle.OBJECT,oracle.OUTPUT,oracle.OBJECT+0x1000,oracle.OBJECT+0x1100,int(series))
        active=oracle.cpu.reg_read(UC_X86_REG_EAX)&255
        bits=struct.unpack('<4Q',oracle.read(oracle.OUTPUT,32))
        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'combine-pickups',str(left),str(right),'series' if series else 'parallel'],text=True).strip()
        expected=str(active)+' '+ ' '.join(f'{v:016x}' for v in bits)
        assert actual==expected,(left.name,right.name,series,actual,expected)
        native_cases+=1
        return ' '.join(f'{v:016x}' for v in struct.unpack('>4Q',a)+struct.unpack('>4Q',b))+f' {int(series)} '+expected
    for i,left in enumerate(paths):
        for right in paths:
            for series in (False,True): compare(left,right,series)
        print(f'PASS: {i+1}/43 pickup definitions in all pairings',flush=True)
    synthetic=[(5000.,100000.,2.5,1e-10),(8000.,-250000.,4.25,2e-10),(0.,0.,0.,0.),
               (123.,-456.,0.,7.89e-10),(1e6,1e6,100.,0.005),(100.,500.,1e-12,1e-14),
               (0.,0.,1.,0.),(5000.,0.,0.,0.)]
    vectors=[]
    with tempfile.TemporaryDirectory(prefix='pickup-combination-') as temporary:
        examples=[]
        for i,values in enumerate(synthetic):
            path=Path(temporary)/f'{i}.bin';path.write_bytes(struct.pack('>4d',*values)+bytes(53));examples.append(path)
        for left in examples:
            for right in examples:
                for series in (False,True): vectors.append(compare(left,right,series))
    (ROOT/'tests/fixtures/native-pickup-combinations.txt').write_text('\n'.join(vectors)+'\n')
    report={'source_sha256':oracle.sha256,'function':'0x23b30','vendor_definitions':43,
        'vendor_pairing_cases':43*43*2,'synthetic_cases':len(vectors),'total_cases':native_cases,
        'scope':'Bit-exact raw passive-circuit properties and active flag; original code with no external adapters; no response tables, full engine, audio or device validation',
        'status':'pass'}
    (ROOT/'analysis/reports/pickup-combination-agreement.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'PASS: {native_cases} bit-exact combinations ({len(vectors)} synthetic)',flush=True)

if __name__=='__main__': main()
