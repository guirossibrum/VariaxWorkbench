"""Compare smoothed/native float response fields, excluding untouched state."""
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve
from verify_pickup_biquad import bits


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();rate=31250.;ratebits=f'{struct.unpack("<I",struct.pack("<f",rate))[0]:08x}'
    wiring=(3.5e-10,1e6,500000.,500000.,2.2e-8)
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/pickups').glob('*.bin'));assert len(paths)==43
    offsets=sorted([start+i*4 for start in (0x18,0x3c,0x60,0x88,0xac,0xd0,0xf4,0x11c,0x144) for i in range(9)]+[0x84,0x140])
    vectors=[];count=0
    with tempfile.TemporaryDirectory(prefix='pickup-calculated-') as temporary:
        zero=Path(temporary)/'zero.bin';zero.write_bytes(bytes(85))
        synthetic=[]
        for i,c in enumerate([(5000.,100000.,2.5,1e-10),(8000.,250000.,4.25,2e-10)]):
            p=Path(temporary)/f'{i}.bin';p.write_bytes(struct.pack('>4d',*c)+bytes(53));synthetic.append(p)
        pairs=[(p,zero) for p in paths]+[(p,q) for p in synthetic for q in [zero,synthetic[0]]]
        for left,right in pairs:
            a=struct.unpack('>4d',left.read_bytes()[:32]);b=struct.unpack('>4d',right.read_bytes()[:32])
            oracle.write(oracle.OUTPUT+0x1000,struct.pack('<4d',*a));oracle.write(oracle.OUTPUT+0x1100,struct.pack('<4d',*b))
            for frozen in (False,True):
                for logarithmic in (False,True):
                    w=(*wiring[:4],wiring[4]*(-1 if frozen else 1))
                    oracle.write(oracle.OBJECT,bytes(0x180));oracle.write(oracle.OBJECT+8,struct.pack('<f',rate))
                    oracle.write(oracle.OUTPUT+0x2000,struct.pack('<5dIB3x',*w,int(logarithmic),0))
                    oracle.call('0x262c0',oracle.OBJECT,oracle.OUTPUT+0x1000,oracle.OUTPUT+0x1100,oracle.OUTPUT+0x2000,instruction_limit=200_000_000,timeout_us=10_000_000)
                    expected=' '.join(f'{o:x}:{struct.unpack("<I",oracle.read(oracle.OBJECT+o,4))[0]:08x}' for o in offsets)
                    args=[str(left),str(right),'parallel','log' if logarithmic else 'linear',ratebits,bits(w[4])]
                    actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-calculated',*args],text=True).strip()
                    if actual!=expected:
                        diffs=[(x,y) for x,y in zip(actual.split(),expected.split()) if x!=y]
                        raise AssertionError((left.name,right.name,frozen,logarithmic,diffs[:8]))
                    if left in synthetic:
                        vectors.append(' '.join([*map(bits,a),*map(bits,b),ratebits,'0',str(int(logarithmic)),*map(bits,w),expected]))
                    count+=1
            print('PASS:',left.name,right.name,flush=True)
    preserve(ROOT/'tests/fixtures/native-pickup-calculated.txt', ('\n'.join(vectors)+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-calculated-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,'comparisons':count,'fields_per_output':len(offsets),'synthetic_vectors':len(vectors),'scope':'All CalculateResponse-assigned float fields including smoothing, two tapers and frozen-tone convention; host-libm reference, not geometry/packing/audio'},indent=2)+'\n').encode())
    print('PASS:',count,'calculated responses;',len(vectors),'synthetic vectors')


if __name__=='__main__':main()
