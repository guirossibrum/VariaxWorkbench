"""Original initialized table generator versus Rust; vendor evidence stays local."""
import json
import struct
import subprocess
import tempfile
from pathlib import Path
from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve
from verify_pickup_biquad import bits


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();rate=31250.;ratebits=f'{struct.unpack("<I",struct.pack("<f",rate))[0]:08x}'
    oracle.write(oracle.OBJECT+8,struct.pack('<f',rate))
    wiring=(3.5e-10,1e6,500000.,500000.,2.2e-8)
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/pickups').glob('*.bin'))
    assert len(paths)==43
    vectors=[];count=0
    with tempfile.TemporaryDirectory(prefix='pickup-tables-') as temporary:
        zero=Path(temporary)/'zero.bin';zero.write_bytes(bytes(85))
        synthetic=[]
        for i,c in enumerate([(5000.,100000.,2.5,1e-10),(8000.,250000.,4.25,2e-10),(100.,1e6,.05,1e-14),(1e6,1e6,100.,.005)]):
            p=Path(temporary)/f'{i}.bin';p.write_bytes(struct.pack('>4d',*c)+bytes(53));synthetic.append(p)
        pairs=[(p,zero) for p in paths]+[(paths[i],paths[(i+1)%len(paths)]) for i in range(0,len(paths),4)]+[(p,q) for p in synthetic for q in [zero,synthetic[0]]]
        for left,right in pairs:
            a=struct.unpack('>4d',left.read_bytes()[:32]);b=struct.unpack('>4d',right.read_bytes()[:32])
            oracle.write(oracle.OBJECT+0x1000,struct.pack('<4d',*a));oracle.write(oracle.OBJECT+0x1100,struct.pack('<4d',*b))
            for series in (False,True):
                for logarithmic in (False,True):
                    oracle.write(oracle.OBJECT+0x2000,struct.pack('<5dIB3x',*wiring,int(logarithmic),int(series)))
                    oracle.write(oracle.OUTPUT,bytes(512))
                    oracle.call('0x25d40',oracle.OBJECT,oracle.OUTPUT,oracle.OBJECT+0x1000,oracle.OBJECT+0x1100,oracle.OBJECT+0x2000,instruction_limit=200_000_000,timeout_us=10_000_000)
                    expected=' '.join(bits(v) for v in struct.unpack('<64d',oracle.read(oracle.OUTPUT,512)))
                    args=[str(left),str(right),'series' if series else 'parallel','log' if logarithmic else 'linear',ratebits]
                    actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-tables',*args],text=True).strip()
                    if actual!=expected:
                        diffs=[(i,x,y) for i,(x,y) in enumerate(zip(actual.split(),expected.split())) if x!=y]
                        raise AssertionError((left.name,right.name,series,logarithmic,diffs[:8]))
                    if left in synthetic:
                        vectors.append(' '.join([*map(bits,a),*map(bits,b),ratebits,str(int(series)),str(int(logarithmic)),*map(bits,wiring),expected]))
                    count+=1
            print('PASS:',left.name,right.name,flush=True)
    preserve(ROOT/'tests/fixtures/native-pickup-tables.txt', ('\n'.join(vectors)+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-tables-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,'comparisons':count,'synthetic_vectors':len(vectors),'scope':'All 64 table doubles, two tapers, two wiring modes, bounded native fitting/host libm; not final response smoothing, geometry, packed engine or audio'},indent=2)+'\n').encode())
    print('PASS:',count,'complete tables;',len(vectors),'synthetic vectors')


if __name__=='__main__':main()
