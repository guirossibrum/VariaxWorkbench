"""Compare secondary peak fitting against bounded original native instructions."""
import json
import math
from pathlib import Path
import struct
import subprocess
import tempfile
from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve
from verify_pickup_biquad import bits, words


def main():
    subprocess.run(['cargo', 'build'], cwd=ROOT, check=True)
    oracle=CoefficientOracle()
    rate=31250.
    ratebits=f'{struct.unpack("<I",struct.pack("<f",rate))[0]:08x}'
    wiring=(3.5e-10,1e6,500000.,500000.,2.2e-8)
    oracle.write(oracle.OBJECT+8,struct.pack('<f',rate))
    oracle.write(oracle.OBJECT+0x2000,struct.pack('<5d',*wiring))
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/pickups').glob('*.bin'))
    assert len(paths)==43
    vectors=[];count=0
    with tempfile.TemporaryDirectory(prefix='secondary-fixture-') as temporary:
        circuits=[(5000.,100000.,2.5,1e-10),(8000.,250000.,4.25,2e-10),(100.,1e6,.05,1e-14),(1e6,1e6,100.,.005)]
        synthetic=[]
        for i,circuit in enumerate(circuits):
            path=Path(temporary)/f'{i}.bin';path.write_bytes(struct.pack('>4d',*circuit)+bytes(53));synthetic.append(path)
        for path in [*paths,*synthetic]:
            circuit=struct.unpack('>4d',path.read_bytes()[:32])
            if circuit[2]==0:continue
            oracle.write(oracle.OBJECT+0x1000,struct.pack('<4d',*circuit))
            limit=1./(math.sqrt((circuit[3]+wiring[4])*circuit[2])*math.tau)
            for volume in (.5,1.):
                for tone in (0.,.001,1.):
                    oracle.call('0x259a0',oracle.OBJECT,oracle.OBJECT+0x1000,oracle.OBJECT+0x2000,oracle.OUTPUT+0x100,*words(volume),*words(tone),instruction_limit=200_000_000,timeout_us=10_000_000)
                    primary=struct.unpack('<3d',oracle.read(oracle.OUTPUT+0x100,24))
                    oracle.write(oracle.OUTPUT,bytes(24))
                    oracle.call('0x252b0',oracle.OBJECT,oracle.OBJECT+0x1000,oracle.OBJECT+0x2000,oracle.OUTPUT,oracle.OUTPUT+0x100,*words(volume),*words(tone),*words(limit),instruction_limit=200_000_000,timeout_us=10_000_000)
                    expected=' '.join(bits(v) for v in struct.unpack('<3d',oracle.read(oracle.OUTPUT,24)))
                    fields=[str(path),ratebits,bits(volume),bits(tone),bits(limit),*map(bits,primary)]
                    actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-filter2',*fields],text=True).strip()
                    assert actual==expected,(path.name,volume,tone,actual,expected)
                    if path in synthetic:
                        vectors.append(' '.join([*map(bits,circuit),ratebits,*map(bits,wiring),*fields[2:],expected]))
                    count+=1
            print('PASS:',path.name,flush=True)
    preserve(ROOT/'tests/fixtures/native-pickup-secondary.txt', ('\n'.join(vectors)+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-secondary-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,'comparisons':count,'synthetic_vectors':len(vectors),'scope':'Secondary frequency/Q/gain fit with native primary parameters and host-libm adapters; no audio/device DSP'},indent=2)+'\n').encode())
    print('PASS:',count,'secondary fits;',len(vectors),'synthetic vectors')


if __name__=='__main__':main()
