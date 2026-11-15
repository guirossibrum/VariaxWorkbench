"""Original mixed-precision band coefficients against Rust; no device services."""
import json
import struct
import subprocess
from coefficient_oracle import CoefficientOracle,ROOT
from native_oracle import float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();vectors=[]
    oracle.write(oracle.OBJECT,bytes(0x50));oracle.write(oracle.OBJECT+0xc,struct.pack('<I',1));oracle.write(oracle.OBJECT+0x40,struct.pack('<I',oracle.OUTPUT+0x1000))
    for kind in (0,1,2,3,4,5,6,99):
        for rate in (31250.,44100.,48000.):
            oracle.write(oracle.OBJECT+8,struct.pack('<f',rate))
            for frequency in (20.,110.,1200.,12000.):
                for q in (.3,1.5):
                    for gain in (-12.,0.,12.):
                        for mix in (0.,.5,1.,1.25):
                            args=[kind,*map(float_bits,[frequency,q,gain,mix])]
                            oracle.call('0x27730',oracle.OBJECT,0,0,*args)
                            expected=' '.join(f'{v:08x}' for v in struct.unpack('<5I',oracle.read(oracle.OUTPUT+0x1000+20,20)))
                            fields=[*args,float_bits(rate)]
                            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'dsp-equalizer',*[f'{v:08x}' for v in fields]],text=True).strip()
                            assert actual==expected,(kind,rate,frequency,q,gain,mix,actual,expected)
                            vectors.append(' '.join([*[f'{v:08x}' for v in fields],expected]))
        print('PASS: kind',kind,flush=True)
    preserve(ROOT/'tests/fixtures/native-dsp-equalizer.txt', ('\n'.join(vectors)+'\n').encode())
    preserve(ROOT/f'analysis/reports/dsp-equalizer-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'cases':len(vectors),'scope':'Original mixed-precision SetStringEQBandParams coefficients including gain signs and wet/dry mixing; synthetic inputs and host-libm adapters, not block packing or sound'},indent=2)+'\n').encode())
    print('PASS:',len(vectors),'mixed-precision EQ coefficient cases')


if __name__=='__main__':main()
