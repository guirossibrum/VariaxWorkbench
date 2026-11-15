"""Body-resonance and string-tone native parameter-to-word path comparisons."""
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
    oracle=CoefficientOracle();rng=random.Random(60038);fixtures=[];counts={}
    metadata={'body':(0x1ec,0x17a40,0x16f40),'string-tone':(0x6e8,0x28c40,0x28a00)}
    with tempfile.TemporaryDirectory(prefix='dsp-resonance-') as temporary:
        p=Path(temporary)/'input.bin'
        for kind,(length,update,write) in metadata.items():
            for rate in (31250.,44100.,48000.):
                for case in range(24):
                    source=bytearray(length);source[4]=case%2;struct.pack_into('<f',source,8,rate)
                    def floats(offset,values):struct.pack_into('<'+'f'*len(values),source,offset,*values)
                    if kind=='body':
                        struct.pack_into('<I',source,0xc,[0,1,6,12][case%4]);floats(0x10,[rng.uniform(-1.,1.),rng.uniform(-1.,1.),rng.choice([-24.,-6.,0.,6.]),rng.choice([0.,100.,1000.,12000.]),rng.choice([0.,80.,800.,8000.]),rng.uniform(-1.,1.)])
                        for i in range(12):floats(0x2c+i*16,[rng.choice([20.,110.,440.,1200.,6000.]),rng.choice([.3,.707,2.,10.]),rng.choice([-24.,-6.,0.,6.])])
                    else:
                        for string in range(6):
                            floats(0x658+string*4,[rng.choice([-24.,-6.,0.,6.])]);floats(0x688+string*4,[rng.choice([0.,.001,.01,.1,1.])]);floats(0x6a0+string*4,[rng.choice([0.,.002,.02,.2,2.])])
                            for band in range(6):
                                offset=0x20+(string*6+band)*44
                                floats(offset+8,[rng.choice([20.,110.,440.,1200.,6000.]),rng.choice([.3,.707,2.,10.]),rng.choice([-24.,-6.,0.,6.])]);source[offset+0x14]=0 if case==0 else (1 if case==1 else rng.randrange(2))
                    source=bytes(source);p.write_bytes(source)
                    for seed in (0,0x60038,0xffffffff):
                        x=seed;words=[]
                        for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
                        oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.write(oracle.OUTPUT,source);oracle.call(hex(update),oracle.OUTPUT);oracle.call(hex(write),oracle.OUTPUT,oracle.OBJECT)
                        expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
                        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-resonance-block',f'{seed:08x}',kind,str(p)],text=True).split()
                        if [int(v,16) for v in actual]!=expected:
                            diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
                            raise AssertionError((kind,rate,case,seed,diffs[:5]))
                        changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i]);fixtures.append(f'{kind} {seed:08x} {source.hex()} => {changes}');counts[kind]=counts.get(kind,0)+1
            print('PASS:',kind,counts[kind],flush=True)
    preserve(ROOT/'tests/fixtures/native-dsp-resonance-blocks.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/f'analysis/reports/dsp-resonance-blocks-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'counts':counts,'seed':60038,'scope':'Unmodified body-resonance/string-tone native UpdateCoefs and UpdateHWPatch, all seeded logical words; synthetic params and host libm, not engine/audio/hardware proof'},indent=2)+'\n').encode())
    print('PASS:',sum(counts.values()),'complete resonant-block assemblies')


if __name__=='__main__':main()
