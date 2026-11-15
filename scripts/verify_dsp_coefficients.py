"""Original per-block coefficient word writers, with unrelated words preserved."""
import json
import random
import struct
import subprocess
from native_oracle import NativeOracle,ROOT,float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=NativeOracle();rng=random.Random(60032);cases=[]
    patterns=[[-4.,-2.,-1.,-0.,0.],[-1e-9,.125,.5,1.,4.]]+[[rng.uniform(-4.,4.) for _ in range(5)] for _ in range(12)]
    for pattern in patterns:
        a,b,c,d,e=map(float_bits,pattern)
        cases.extend([('body-treble',0x189c0,[a,b,c,d]),('body-mix',0x18dc0,[a,b]),('low-cut',0x18ff0,[a,b]),('pick-filter',0x1b770,[a,b,c]),('compressor',0x1bae0,[a,b])])
        for i in (0,11):cases.append(('body-resonator',0x18670,[i,a,b,c]))
        for i in (0,35):cases.append(('string-eq',0x19230,[i,a,b,c,d,e]))
        for i in (0,11):cases.append(('twelve-eq',0x198e0,[i,a,b,c,d,e]))
        for i in (0,5):
            cases.extend([('string-eq-gain',0x197b0,[i,a]),('twelve-wet-dry',0x19e60,[i,a,b]),('string-tone',0x1a090,[i,6,a,b]),('global-decay',0x1a620,[i,a,b,c]),('sitar-string',0x1ae40,[i,a,b,c,d,e]),('pitch-string',0x1b3b0,[i,a,rng.getrandbits(32)])])
            for r in (0,5):cases.append(('string-tone-resonator',0x1a2d0,[i,r,a,b,c]))
        for i in (0,11):cases.append(('drone-string',0x1abd0,[i,rng.getrandbits(32),a]))
    fixtures=[]
    for seed in (0,0x60032,0xffffffff):
        for name,address,args in cases:
            x=seed;words=[]
            for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
            oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.call(hex(address),oracle.OBJECT,*args)
            expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-coefficient',f'{seed:08x}',name,*[f'{v:08x}' for v in args]],text=True).split()
            if [int(v,16) for v in actual]!=expected:
                diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
                raise AssertionError((name,args,seed,diffs[:5]))
            changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i])
            fixtures.append(' '.join([f'{seed:08x}',name,*[f'{v:08x}' for v in args]])+' => '+changes)
    preserve(ROOT/'tests/fixtures/native-dsp-coefficients.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/'analysis/reports/dsp-coefficients-v1.json',(json.dumps({'source_sha256':oracle.sha256,'setter_functions':len(set(c[0] for c in cases)),'cases':len(fixtures),'seed':60032,'scope':'Per-block coefficient writers, entire seeded 1091-word array; original instructions, no external imports. Not block DSP parameter calculations or audio.'},indent=2)+'\n').encode())
    print('PASS:',len(fixtures),'block coefficient setter comparisons;',len(set(c[0] for c in cases)),'functions')


if __name__=='__main__':main()
