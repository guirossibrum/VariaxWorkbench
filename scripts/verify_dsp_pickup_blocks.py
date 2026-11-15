"""End-to-end native pickup block word assembly from synthetic runtime snapshots."""
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from coefficient_oracle import CoefficientOracle,ROOT
from native_oracle import float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();fixtures=[];counts={'geometry':0,'response':0}
    def compare(kind,source,seed,args):
        x=seed;words=[]
        for _ in range(1091):x=(x*1664525+1013904223)&0xffffffff;words.append(x)
        oracle.write(oracle.OBJECT,struct.pack('<1091I',*words))
        if kind=='geometry':
            scale,coils,*_=struct.unpack_from('<fI',source)
            gains=struct.unpack_from('<4f',source,32);positions=struct.unpack_from('<24f',source,48);phases=source[144:168];apertures=struct.unpack_from('<6I',source,8)
            oracle.write(oracle.OUTPUT,bytes(0xb4));oracle.write(oracle.OUTPUT+4,source[168:169]);oracle.write(oracle.OUTPUT+0xc,struct.pack('<fI4f24f24B6I',scale,coils,*gains,*positions,*phases,*apertures))
            oracle.call('0x23670',oracle.OUTPUT,oracle.OBJECT)
        else:
            oracle.write(oracle.OUTPUT,source);oracle.call('0x23a00',oracle.OUTPUT,oracle.OBJECT)
        expected=list(struct.unpack('<1091I',oracle.read(oracle.OBJECT,1091*4)))
        actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-'+kind,f'{seed:08x}',*args],text=True).split()
        if [int(v,16) for v in actual]!=expected:
            diffs=[(i,a,f'{b:08x}') for i,(a,b) in enumerate(zip(actual,expected)) if int(a,16)!=b]
            raise AssertionError((kind,seed,diffs[:5]))
        changes=' '.join(f'{i:x}:{v:08x}' for i,v in enumerate(expected) if v!=words[i])
        fixtures.append(f'{kind} {seed:08x} {source.hex()} => {changes}');counts[kind]+=1
    with tempfile.TemporaryDirectory(prefix='dsp-pickup-block-') as temporary:
        p=Path(temporary)/'input.bin'
        for line in (ROOT/'tests/fixtures/native-pickup-projection.txt').read_text().splitlines():
            source=bytes.fromhex(line.split()[0]);p.write_bytes(source)
            for seed in (0,0x60033,0xffffffff):compare('geometry',source,seed,[str(p)])
        lines=[]
        for name in ['native-pickup-calculated.txt','native-pickup-calculated-wiring.txt']:lines.extend((ROOT/'tests/fixtures'/name).read_text().splitlines())
        for line in lines:
            fields=line.split();assert len(fields)==99
            for enabled,saved,tone,scalar,function in [(False,False,0.,.5,0),(True,False,1.,1.,1),(False,True,.5,2.,2),(True,True,1.,.125,3)]:
                source=bytearray(0x168)
                for value in fields[16:]:offset,bits=value.split(':');struct.pack_into('<I',source,int(offset,16),int(bits,16))
                source[4]=int(enabled);source[0x10]=int(saved);struct.pack_into('<I',source,0xc,float_bits(tone));struct.pack_into('<I',source,0x14,function);struct.pack_into('<I',source,0x118,float_bits(scalar))
                source=bytes(source);p.write_bytes(source)
                for seed in (0,0x60033,0xffffffff):compare('response',source,seed,[str(p)])
    preserve(ROOT/'tests/fixtures/native-dsp-pickup-blocks.txt', ('\n'.join(fixtures)+'\n').encode())
    preserve(ROOT/f'analysis/reports/dsp-pickup-blocks-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'counts':counts,'scope':'Unmodified original geometry/response UpdateHWPatch and all called setters, compared against all 1091 seeded Rust words; synthetic snapshots only, no device/audio/full-engine parity'},indent=2)+'\n').encode())
    print('PASS:',counts,'complete pickup block assemblies')


if __name__=='__main__':main()
