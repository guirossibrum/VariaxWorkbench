"""Original sitar parameter-to-word path, including zero-Q stock defaults."""
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
    oracle=CoefficientOracle();rng=random.Random(60037);fixtures=[]
    with tempfile.TemporaryDirectory(prefix='dsp-sitar-') as temporary:
        p=Path(temporary)/'input.bin'
        for rate in (31250.,44100.,48000.):
            for case in range(32):
                source=bytearray(0x84);source[4]=case%2;struct.pack_into('<f',source,8,rate)
                for offset,choices in [(0xc,[0.,-0.,.001,.1,1.,10.]),(0x24,[0.,40.,110.,1000.,12000.]),(0x3c,[0.,20.,80.,440.,10000.]),(0x54,[0.,-0.,.1,.5,1.,6.46]),(0x6c,[-.5,0.,.5,1.,2.])]:struct.pack_into('<6f',source,offset,*[rng.choice(choices) for _ in range(6)])
                source=bytes(source);p.write_bytes(source)
                for seed in (0,0x60037,0xffffffff):
                    x=seed;words=[]
                    for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
                    oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.write(oracle.OUTPUT,source);oracle.call('0x27310',oracle.OUTPUT,oracle.OBJECT)
                    expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
                    actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-resonance-block',f'{seed:08x}','sitar',str(p)],text=True).split()
                    if [int(v,16) for v in actual]!=expected:
                        diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
                        raise AssertionError((rate,case,seed,diffs[:5]))
                    changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i]);fixtures.append(f'{seed:08x} {source.hex()} => {changes}')
    preserve(ROOT/'tests/fixtures/native-dsp-sitar.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/f'analysis/reports/dsp-sitar-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'cases':len(fixtures),'seed':60037,'scope':'Original SitarBuzz UpdateHWPatch including double exp after float rate*time, zero-Q/zero-duration defaults and signed zero; all seeded words, no audio/device'},indent=2)+'\n').encode())
    print('PASS:',len(fixtures),'complete sitar block assemblies')


if __name__=='__main__':main()
