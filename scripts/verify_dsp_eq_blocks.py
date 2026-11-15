"""Complete string and twelve-string EQ parameter-to-word comparisons."""
import json
import random
import struct
import subprocess
import tempfile
from pathlib import Path
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();rng=random.Random(60036);fixtures=[];counts={}
    with tempfile.TemporaryDirectory(prefix='dsp-eq-block-') as temporary:
        p=Path(temporary)/'input.bin'
        for kind,n,writer in [('string-eq',6,0x27e10),('twelve-eq',2,0x16b30)]:
            for rate in (31250.,44100.,48000.):
                for case in range(24):
                    source=bytearray(struct.pack('<IfII',case%2,rate,n,1));levels=[];parameters=[]
                    for string in range(6):
                        level=rng.uniform(-1.,2.);levels.append(level);source.extend(struct.pack('<f',level))
                        for band in range(n):
                            mode=0 if case==0 else (1 if case==1 else rng.choice([0,0,1,2,3,4,5,99]))
                            record=struct.pack('<I4f',mode,rng.choice([20.,110.,1000.,6000.,12000.]),rng.choice([.3,.707,1.5,3.]),rng.choice([-12.,0.,12.]),rng.choice([0.,.5,1.,1.25]));parameters.append(record);source.extend(record)
                    source=bytes(source);p.write_bytes(source)
                    for seed in (0,0x60036,0xffffffff):
                        x=seed;words=[]
                        for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
                        oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.write(oracle.OUTPUT,bytes(0x50));oracle.write(oracle.OUTPUT+4,bytes([case%2]));oracle.write(oracle.OUTPUT+8,struct.pack('<fI6f6II',rate,n,*levels,*([n]*6),oracle.OUTPUT+0x2000))
                        for i,record in enumerate(parameters):oracle.write(oracle.OUTPUT+0x2000+i*48,record+bytes(28))
                        oracle.call('0x28310',oracle.OUTPUT);oracle.call(hex(writer),oracle.OUTPUT,oracle.OBJECT)
                        expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
                        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-eq-block',f'{seed:08x}',kind,str(p)],text=True).split()
                        if [int(v,16) for v in actual]!=expected:
                            diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
                            raise AssertionError((kind,rate,case,seed,diffs[:5]))
                        changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i]);fixtures.append(f'{kind} {seed:08x} {source.hex()} => {changes}');counts[kind]=counts.get(kind,0)+1
            print('PASS:',kind,counts[kind],flush=True)
    preserve(ROOT/'tests/fixtures/native-dsp-eq-blocks.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/f'analysis/reports/dsp-eq-blocks-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'cases':counts,'seed':60036,'scope':'Original full base UpdateCoefs and StringEQ/12StringEQ UpdateHWPatch, all 1091 seeded words, synthetic pointer-free interchange with private native band-array adapter; not full engine/audio'},indent=2)+'\n').encode())
    print('PASS:',sum(counts.values()),'complete EQ block assemblies')


if __name__=='__main__':main()
