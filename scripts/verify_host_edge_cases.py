"""Synthetic regressions for mixed-precision EQ and derived sitar NaNs."""
import random
import struct
import subprocess
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from native_oracle import float_bits


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();vectors=[];rng=random.Random(600004)
    oracle.write(oracle.OBJECT,bytes(0x50));oracle.write(oracle.OBJECT+0xc,struct.pack('<I',1));oracle.write(oracle.OBJECT+0x40,struct.pack('<I',oracle.OUTPUT+0x1000))
    for i in range(300):
        kind=i%6;rate=[31250.,39062.5,44100.,48000.][i%4]
        args=[kind,*map(float_bits,[rng.uniform(20.,12000.),rng.uniform(.2,4.),rng.uniform(-18.,6.),rng.uniform(-.25,1.25)])]
        oracle.write(oracle.OBJECT+8,struct.pack('<f',rate));oracle.call('0x27730',oracle.OBJECT,0,0,*args)
        expected=' '.join(f'{v:08x}' for v in struct.unpack('<5I',oracle.read(oracle.OUTPUT+0x1000+20,20)))
        fields=[*args,float_bits(rate)]
        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-equalizer',*[f'{v:08x}' for v in fields]],text=True).strip()
        assert actual==expected,(i,fields,actual,expected)
        vectors.append(' '.join([*[f'{v:08x}' for v in fields],expected]))
    preserve(ROOT/'tests/fixtures/native-dsp-eq-mixing.txt',('\n'.join(vectors)+'\n').encode())
    vectors=[]
    for rate in [31250.,39062.5,48000.]:
        for enabled in [0,1]:
            for case in range(4):
                source=bytearray(0x84);source[4]=enabled;struct.pack_into('<f',source,8,rate)
                struct.pack_into('<6f',source,0xc,*([.1]*6));struct.pack_into('<6f',source,0x54,*([.5]*6));struct.pack_into('<6f',source,0x6c,*([.5]*6))
                for off,active in [(0x24,case&1),(0x3c,case&2)]:
                    for i in range(6):struct.pack_into('<I',source,off+i*4,(0x7fc00000 if i%2 else 0xffc00000) if active else float_bits(110. if off==0x24 else 80.))
                for seed in [0,0xffffffff]:
                    state=seed;words=[]
                    for _ in range(1091):state=(state*1664525+1013904223)&0xffffffff;words.append(state)
                    oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.write(oracle.OUTPUT,bytes(source));oracle.call('0x27310',oracle.OUTPUT,oracle.OBJECT)
                    expected=struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4))
                    changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i])
                    vectors.append(f'{seed:08x} {source.hex()} => {changes}')
    preserve(ROOT/'tests/fixtures/native-dsp-derived-sitar.txt',('\n'.join(vectors)+'\n').encode())
    subprocess.run(['cargo','test','--lib','dsp::'],cwd=ROOT,check=True)
    print('PASS: 300 fractional EQ mixing cases and 48 derived-frequency sitar cases')


if __name__=='__main__':main()
