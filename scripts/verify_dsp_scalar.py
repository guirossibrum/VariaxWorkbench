"""Native bit encoding and logical-word setters; no packing/upload/device calls."""
import json
import random
import struct
import subprocess
from native_oracle import NativeOracle, ROOT, float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=NativeOracle();rng=random.Random(6003)
    inputs=[0,0x80000000,0x7f800000,0xff800000,0x7fc00000,0xffc00000,1,0x80000001,0x7fffff,0x807fffff,0x3f800000,0xbf800000]
    inputs.extend(rng.getrandbits(32) for _ in range(1000));float_vectors=[]
    for bits in inputs:
        oracle.call('0x18100',oracle.OUTPUT,bits)
        expected=' '.join(f'{v:08x}' for v in struct.unpack('<2I',oracle.read(oracle.OUTPUT,8)))
        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'vx-float',f'{bits:08x}'],text=True).strip()
        assert actual==expected,(hex(bits),actual,expected)
        float_vectors.append(f'{bits:08x} {expected}')
    cases=[]
    for v in (0,1,0x883,0xffffff,0xffffffff):cases.append(('status',0x182a0,[v]))
    for mask in (1,0x20,0x4000,0xffffff,0x80000000):
        for enabled in (0,1):cases.append(('block',0x182d0,[mask,enabled]))
    for bits in inputs[:12]+[float_bits(v) for v in (24.75,25.,25.5)]:cases.append(('scale',0x1bea0,[bits]))
    for v in range(5):cases.append(('coils',0x184d0,[v]))
    for i in range(6):cases.append(('aperture',0x184e0,[i,rng.getrandbits(32)]))
    for v in (0,1,12):cases.append(('resonators',0x18300,[v]))
    for v in (0,1):cases.append(('fourth-order',0x18320,[v]))
    for i in range(6):
        for v in (0,1,5):cases.append(('eq-bands',0x18340,[i,v]))
    for v in (0,1,2,3,0xffffffff):cases.append(('analog',0x18360,[v]))
    for i in range(4):cases.append(('allpass-delay',0x18380,[i,rng.getrandbits(32)]))
    for v in (0,1,999,0xffffffff):cases.append(('pre-delay',0x183a0,[v]))
    for v in (0,1,0xffffffff):cases.append(('reverb-sources',0x183c0,[v,v^0x80000000]))
    for v in (0,1,6,12):cases.append(('drone-count',0x183e0,[v]))
    for v in (0,1,2,3,0xffff):cases.append(('tone-function',0x184b0,[v]))
    for v in (0.,-0.,.125,.5,1.):
        for saved in (0,1):cases.append(('tone-position',0x18400,[float_bits(v),saved]))
    fixtures=[];total=0
    for seed in (0,0x6003,0xffffffff):
        for name,address,args in cases:
            x=seed;words=[]
            for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
            oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.call(hex(address),oracle.OBJECT,*args)
            expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-scalar',f'{seed:08x}',name,*[f'{v:08x}' for v in args]],text=True).split()
            assert [int(v,16) for v in actual]==expected,(name,args,seed)
            changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i])
            fixtures.append(' '.join([f'{seed:08x}',name,*[f'{v:08x}' for v in args]])+' => '+changes);total+=1
    preserve(ROOT/'tests/fixtures/native-vx-float.txt', ('\n'.join(float_vectors)+'\n').encode())
    preserve(ROOT/'tests/fixtures/native-dsp-scalar.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/'analysis/reports/dsp-scalar-v1.json',(json.dumps({'source_sha256':oracle.sha256,'float_encoding_cases':len(inputs),'scalar_setter_cases':total,'scalar_functions':len(set(c[0] for c in cases)),'scope':'Original bit encoding and scalar setters with seeded surrounding words; full 1091-word equality; no DSP block calculations or device operations'},indent=2)+'\n').encode())
    print('PASS:',len(inputs),'VX encodings;',total,'full-word setter comparisons')


if __name__=='__main__':main()
