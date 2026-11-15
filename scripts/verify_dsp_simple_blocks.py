"""Native parameter-to-word block paths against complete seeded Rust patches."""
import json
from pathlib import Path
import random
import struct
import subprocess
import tempfile
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();rng=random.Random(60034);fixtures=[];counts={}
    metadata={'volume':(0x3c,None,0x29060),'sum':(0x24,None,0x28e50),'decay':(0x84,0x18020,0x17eb0),'drone':(0xa4,0x17d80,0x17bf0),'pitch':(0x6c,None,0x26ac0),'mic':(0xa0,None,0x29b70),'mic-parameters':(0xa0,0x29c80,0x29b70)}
    with tempfile.TemporaryDirectory(prefix='dsp-simple-block-') as temporary:
        p=Path(temporary)/'input.bin'
        for kind,(length,update,write) in metadata.items():
            for rate in (31250.,44100.,48000.):
                for case in range(16):
                    source=bytearray(length);source[4]=case%2;struct.pack_into('<f',source,8,rate)
                    def floats(offset,values):struct.pack_into('<'+'f'*len(values),source,offset,*values)
                    if kind=='volume':
                        floats(0xc,[rng.uniform(-.25,1.25) for _ in range(9)]);struct.pack_into('<I',source,0x30,case%4);source[0x34]=(case//2)%2;floats(0x38,[(-24.,-6.,0.,6.)[case%4]])
                    elif kind=='sum':floats(0xc,[rng.uniform(-2.,2.) for _ in range(6)])
                    elif kind=='decay':
                        for i in range(6):floats(0xc+i*20,[(0.,.001,.1,1.,10.)[(case+i)%5],(0.,.002,.2,2.)[(case+i)%4],rng.choice([0.,1e-6,.125,.5])])
                    elif kind=='drone':
                        struct.pack_into('<I',source,0xc,[0,1,6,12,99][case%5]);floats(0x10,[rng.uniform(0.,1.)]);floats(0x14,[rng.choice([0.,1.,55.,110.,220.,5000.,-100.]) for _ in range(12)]);floats(0x44,[rng.uniform(-1.,1.) for _ in range(12)])
                    elif kind=='pitch':
                        floats(0xc,[rng.choice([-24.,-12.,-2.5,0.,.25,7.,12.,24.]) for _ in range(6)]);floats(0x24,[rng.choice([-10.,0.,.001,.5,5.,1000.,1e20]) for _ in range(6)]);floats(0x3c,[rng.choice([-.5,0.,.5,1.,1.5]) for _ in range(6)]);floats(0x54,[rng.choice([-.5,0.,.5,1.,2.]) for _ in range(6)])
                    elif kind=='mic-parameters':
                        for offset in (0xc,0x54):floats(offset,[rng.uniform(-1.,1.) for _ in range(9)])
                        floats(0x30,[rng.uniform(-5.,30.) for _ in range(9)]);floats(0x78,[rng.choice([24.75,25.,25.5])])
                    else:
                        for offset in (0xc,0x54,0x7c):floats(offset,[rng.uniform(-1.,1.) for _ in range(9)])
                    source=bytes(source);p.write_bytes(source)
                    for seed in (0,0x60034,0xffffffff):
                        x=seed;words=[]
                        for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
                        oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.write(oracle.OUTPUT,source)
                        if update:oracle.call(hex(update),oracle.OUTPUT)
                        oracle.call(hex(write),oracle.OUTPUT,oracle.OBJECT)
                        expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
                        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-block',f'{seed:08x}',kind,str(p)],text=True).split()
                        if [int(v,16) for v in actual]!=expected:
                            diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
                            raise AssertionError((kind,rate,case,seed,diffs[:5]))
                        changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i])
                        fixtures.append(f'{kind} {seed:08x} {source.hex()} => {changes}');counts[kind]=counts.get(kind,0)+1
            print('PASS:',kind,counts[kind],flush=True)
    preserve(ROOT/'tests/fixtures/native-dsp-simple-blocks-v2.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/f'analysis/reports/dsp-simple-blocks-{oracle.RUNTIME_VERSION}-v2.json',(json.dumps({'source_sha256':oracle.sha256,'counts':counts,'seed':60034,'scope':'Complete native parameter conversions and original word writes for six blocks including mic-position preparation, seeded 1091-word equality, synthetic input and host libm; not full engine or audio'},indent=2)+'\n').encode())
    print('PASS:',sum(counts.values()),'simple block assemblies')


if __name__=='__main__':main()
