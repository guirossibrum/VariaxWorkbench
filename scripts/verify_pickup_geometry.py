"""Original pole interpolation/projection with captured setter-call arguments."""
import json
import random
import struct
import subprocess
import tempfile
from pathlib import Path
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP
from coefficient_oracle import CoefficientOracle, ROOT
from native_oracle import NativeOracle,float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    bodyoracle=NativeOracle();count=0;body_vectors=[]
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/bodies').glob('*.bin'));assert len(paths)==28
    for path in [*paths,ROOT/'tests/fixtures/native-body-definition.bin']:
        payload=path.read_bytes()
        for i in range(3):
            source=payload[1+i*32:1+(i+1)*32]
            bodyoracle.write(bodyoracle.OBJECT+0x4b50+i*32,b''.join(source[n:n+4][::-1] for n in range(0,32,4)))
        bodyoracle.write(bodyoracle.OBJECT+0x4bb0,payload[:1])
        for position in (0.,.125,.25,.49999997,.5,.50000006,.75,.875,1.):
            bodyoracle.call('0x9c40',bodyoracle.OBJECT,bodyoracle.OUTPUT,float_bits(position))
            expected=' '.join(f'{v:08x}' for v in struct.unpack('<6I',bodyoracle.read(bodyoracle.OUTPUT,24)))
            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'body-pole-offsets',str(path),f'{float_bits(position):08x}'],text=True).strip()
            assert actual==expected,(path.name,position,actual,expected)
            if path.name=='native-body-definition.bin':body_vectors.append(f'{float_bits(position):08x} {expected}')
            count+=1
    oracle=CoefficientOracle();records={};calls=[]
    def setter(cpu,address,size,context):
        stack=cpu.reg_read(UC_X86_REG_ESP);args=struct.unpack('<4I',oracle.read(stack+4,16));calls.append((address,args))
    for address in (0x182d0,0x1bea0,0x184d0,0x184e0,0x1b4f0,0x1b620):
        oracle.write(address,b'\xc3');oracle.cpu.hook_add(UC_HOOK_CODE,setter,begin=address,end=address)
    rng=random.Random(600);vectors=[]
    with tempfile.TemporaryDirectory(prefix='pickup-geometry-') as temporary:
        p=Path(temporary)/'input.bin'
        for coils in range(5):
            for case in range(24):
                scale=rng.choice([24.75,25.,25.5]);enabled=case%2
                aperture=[rng.randrange(8) for _ in range(6)]
                gains=[rng.choice([-30.,-12.,0.,6.]) for _ in range(4)]
                positions=[rng.choice([-3.,-0.,0.,.125,1.,2.,2.,4.5,8.,12.]) for _ in range(24)]
                phases=[rng.randrange(2) for _ in range(24)]
                raw=struct.pack('<fI6I4f24f24BB',scale,coils,*aperture,*gains,*positions,*phases,enabled)
                assert len(raw)==169;p.write_bytes(raw)
                oracle.write(oracle.OBJECT,bytes(0xb4));oracle.write(oracle.OBJECT+4,bytes([enabled]));oracle.write(oracle.OBJECT+0xc,struct.pack('<fI4f24f24B6I',scale,coils,*gains,*positions,*phases,*aperture))
                calls.clear();oracle.call('0x23670',oracle.OBJECT,oracle.OUTPUT)
                words=[0]*61
                for address,args in calls:
                    assert args[0]==oracle.OUTPUT
                    if address==0x182d0:assert args[1]==0x20;words[0]=args[2]
                    elif address==0x1bea0:words[1]=args[1]
                    elif address==0x184d0:words[2]=args[1]
                    elif address==0x184e0:words[3+args[1]]=args[2]
                    elif address==0x1b4f0:words[9+args[1]]=args[2]
                    elif address==0x1b620:words[13+args[1]*2:15+args[1]*2]=args[2:4]
                expected=' '.join(f'{v:08x}' for v in words)
                actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-project',str(p)],text=True).strip()
                assert actual==expected,(coils,case,actual,expected)
                vectors.append(raw.hex()+' '+expected)
    preserve(ROOT/'tests/fixtures/native-body-pole-offsets.txt', ('\n'.join(body_vectors)+'\n').encode())
    preserve(ROOT/'tests/fixtures/native-pickup-projection.txt', ('\n'.join(vectors)+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-geometry-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'body_pole_comparisons':count,'projection_cases':len(vectors),'projection_values_per_case':61,'seed':600,'scope':'Original body pole interpolation and native projection/phase/sort/gains; final setters replaced with argument recorders, not fixed-point packing or complete placement host pipeline'},indent=2)+'\n').encode())
    print('PASS:',count,'body pole-offset outputs;',len(vectors),'projection traces')


if __name__=='__main__':main()
