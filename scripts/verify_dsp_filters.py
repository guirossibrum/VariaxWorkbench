"""Original biquad helper and filter parameter-to-word paths, offline only."""
import json
from pathlib import Path
import random
import struct
import subprocess
import tempfile
from unicorn.x86_const import UC_X86_REG_EAX,UC_X86_REG_EDX,UC_X86_REG_XMM0
from coefficient_oracle import CoefficientOracle,ROOT
from native_oracle import float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();rng=random.Random(60035);biquads=[];blocks=[];counts={}
    for kind in (0,1,2,3,4,5,6,7,8,99):
        for rate in (31250.,44100.,48000.):
            for freq in (0.,40.,440.,1200.,12000.):
                for q in (.1,.707,2.,10.):
                    oracle.write(oracle.OBJECT,struct.pack('<I3f',kind,freq,q,0.))
                    oracle.cpu.reg_write(UC_X86_REG_EAX,oracle.OBJECT);oracle.cpu.reg_write(UC_X86_REG_EDX,oracle.OUTPUT);oracle.cpu.reg_write(UC_X86_REG_XMM0,float_bits(rate))
                    oracle.call('0x29290')
                    expected=' '.join(f'{v:08x}' for v in struct.unpack('<5I',oracle.read(oracle.OUTPUT,20)))
                    args=[f'{v:08x}' for v in [kind,float_bits(freq),float_bits(q),float_bits(rate)]]
                    actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-biquad',*args],text=True).strip()
                    assert actual==expected,(kind,rate,freq,q,actual,expected)
                    biquads.append(' '.join(args)+' '+expected)
    metadata={'low-cut':(0x3c,0x1cf70,0x1cca0),'pick-filter':(0x54,0x29780,0x295c0),'compressor':(0x40,None,0x298e0),'reverb':(0x114,0x270f0,0x26e30)}
    with tempfile.TemporaryDirectory(prefix='dsp-filters-') as temporary:
        p=Path(temporary)/'input.bin'
        for kind,(length,update,write) in metadata.items():
            for rate in (31250.,44100.,48000.):
                for case in range(24):
                    source=bytearray(length);source[4]=case%2;struct.pack_into('<f',source,8,rate)
                    def floats(offset,values):struct.pack_into('<'+'f'*len(values),source,offset,*values)
                    if kind=='low-cut':floats(0xc,[[0.,10.,80.,1000.,10000.][case%5]]);source[0x10]=(case//2)%2
                    elif kind=='pick-filter':
                        floats(0xc,[[0.,110.,440.,1000.,6000.][case%5],[.1,.707,2.,10.][case%4],[0.,.001,.02,1.][case%4]]);floats(0x18,[rng.uniform(-3.,3.) for _ in range(9)])
                    elif kind=='compressor':floats(0xc,[[0.,.001,.1,1.][case%4],[0.,.02,.2,2.][case%4]]);floats(0x14,[rng.uniform(-1.,2.) for _ in range(9)])
                    else:
                        floats(0xc,[rng.choice([-10.,0.,1.,10.,100.]) for _ in range(4)]);floats(0x1c,[rng.uniform(-1.,1.) for _ in range(9)]);floats(0x40,[rng.choice([-10.,0.,10.,50.])]);struct.pack_into('<2I',source,0x44,case%4,(case+1)%4);floats(0x4c,[rng.choice([0.,10.,100.,1000.,3000.]) for _ in range(9)])
                    source=bytes(source);p.write_bytes(source)
                    for seed in (0,0x60035,0xffffffff):
                        x=seed;words=[]
                        for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
                        oracle.write(oracle.OBJECT,struct.pack('<1091I',*words));oracle.write(oracle.OUTPUT,source)
                        if kind=='compressor':
                            attack,release=struct.unpack_from('<2I',source,0xc);oracle.call('0x29990',oracle.OUTPUT,attack);oracle.call('0x29800',oracle.OUTPUT,release)
                        if update:oracle.call(hex(update),oracle.OUTPUT)
                        oracle.call(hex(write),oracle.OUTPUT,oracle.OBJECT)
                        expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
                        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-filter-block',f'{seed:08x}',kind,str(p)],text=True).split()
                        if [int(v,16) for v in actual]!=expected:
                            diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
                            raise AssertionError((kind,rate,case,seed,diffs[:5]))
                        changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i])
                        blocks.append(f'{kind} {seed:08x} {source.hex()} => {changes}');counts[kind]=counts.get(kind,0)+1
            print('PASS:',kind,counts[kind],flush=True)
    preserve(ROOT/'tests/fixtures/native-dsp-biquad.txt', ('\n'.join(biquads)+'\n').encode())
    preserve(ROOT/'tests/fixtures/native-dsp-filter-blocks.txt', ('\n'.join(blocks)+'\n').encode())
    preserve(ROOT/f'analysis/reports/dsp-filter-blocks-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'biquad_cases':len(biquads),'blocks':counts,'seed':60035,'scope':'Original generic biquad helper (native regparm/SSE ABI), full filter-block preparation and 1091-word assembly, synthetic inputs with host libm; not full engine/audio'},indent=2)+'\n').encode())
    print('PASS:',len(biquads),'biquads;',sum(counts.values()),'filter blocks')


if __name__=='__main__':main()
