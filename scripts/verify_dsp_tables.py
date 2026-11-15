"""Original fixed-point/VX coefficient writers against seeded Rust word arrays."""
import json
import random
import struct
import subprocess
from native_oracle import NativeOracle,ROOT,float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=NativeOracle();rng=random.Random(60031)
    values=[-4.,-2.,-1.,-.99999994,-.5,-1e-9,-0.,0.,1e-9,.125,.5,.99999994,1.,2.,4.]
    patterns=[[float_bits(values[(i+j)%len(values)]) for i in range(9)] for j in (0,5,10)]
    patterns.extend([[float_bits(rng.uniform(-3.,3.)) for _ in range(9)] for _ in range(5)])
    mapping={'mic-random':0x18500,'volume-curve':0x1bf30,'reverb-density':0x1c060,'pickup-lowpass':0x1c2d0,'pick-gain':0x1c400,'compressor-gain':0x1c570,'mic-notch':0x1c6a0,'mic-position':0x1c7d0,'tone2-gain':0x1c900,'tone2-q':0x1c990,'tone1-gain':0x1ca20,'tone-off-gain':0x1cab0,'tone-off-q':0x1cb40,'tone-off-frequency':0x1be10}
    cases=[]
    for name,address in mapping.items():
        for pattern in patterns:cases.append((name,address,pattern,lambda data:[data]))
    for tank in range(4):
        for pattern in patterns:cases.append(('reverb-feedback',0x1c190,[tank,*pattern],lambda data,tank=tank:[tank,data]))
    for value in values:
        v=float_bits(value)
        cases.append(('secondary-frequency',0x1bd80,[v],None))
        for index in (0,3):cases.append(('coil-gain',0x1b4f0,[index,v],None))
        for index in (0,23):
            for phase in (0,1):cases.append(('pole',0x1b620,[index,v,phase],None))
        for index in (0,5):cases.append(('sum-level',0x1a970,[index,v],None))
        cases.extend([('trim-level',0x1aaa0,[v],None),('drone-level',0x1ad10,[v],None)])
    fixtures=[];total=0
    for seed in (0,0x60031,0xffffffff):
        for name,address,args,pointer_args in cases:
            x=seed;words=[]
            for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
            oracle.write(oracle.OBJECT,struct.pack('<1091I',*words))
            if pointer_args:
                data=args[1:] if name=='reverb-feedback' else args
                oracle.write(oracle.OUTPUT,struct.pack('<9I',*data));native_args=pointer_args(oracle.OUTPUT)
            else:native_args=args
            oracle.call(hex(address),oracle.OBJECT,*native_args)
            expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-coefficient',f'{seed:08x}',name,*[f'{v:08x}' for v in args]],text=True).split()
            if [int(v,16) for v in actual]!=expected:
                diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
                raise AssertionError((name,args,seed,diffs[:5]))
            changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i])
            fixtures.append(' '.join([f'{seed:08x}',name,*[f'{v:08x}' for v in args]])+' => '+changes);total+=1
    preserve(ROOT/'tests/fixtures/native-dsp-tables.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/'analysis/reports/dsp-tables-v1.json',(json.dumps({'source_sha256':oracle.sha256,'setter_functions':len(mapping)+1+6,'cases':total,'seed':60031,'scope':'Coefficient table and selected Q23 scalar setters; all 1091 words including preservation; synthetic finite inputs; not full DSP block calculations or device operations'},indent=2)+'\n').encode())
    print('PASS:',total,'full-word coefficient setter comparisons')


if __name__=='__main__':main()
