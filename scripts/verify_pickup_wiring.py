"""Vary full electrical loads, rates, tapers and wiring on synthetic circuits."""
import json
import struct
import subprocess
import tempfile
from pathlib import Path
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from verify_pickup_biquad import bits


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();tables=[];calculated=[]
    circuits=[(5000.,100000.,2.5,1e-10),(8000.,250000.,4.25,2e-10)]
    offsets=sorted([start+i*4 for start in (0x18,0x3c,0x60,0x88,0xac,0xd0,0xf4,0x11c,0x144) for i in range(9)]+[0x84,0x140])
    with tempfile.TemporaryDirectory(prefix='pickup-wiring-') as temporary:
        paths=[]
        for i,c in enumerate(circuits):
            p=Path(temporary)/f'{i}.bin';p.write_bytes(struct.pack('>4d',*c)+bytes(53));paths.append(p)
        wp=Path(temporary)/'wiring.bin'
        for w in [(5.25e-10,470000.,250000.,250000.,4.7e-8),(8.75e-10,1e6,25000.,1e6,1e-9)]:
            wp.write_bytes(struct.pack('<5d',*w))
            for rate in (31250.,44100.):
                ratebits=f'{struct.unpack("<I",struct.pack("<f",rate))[0]:08x}'
                for pair in [(0,0),(0,1)]:
                    a,b=[circuits[i] for i in pair]
                    oracle.write(oracle.OUTPUT+0x1000,struct.pack('<4d',*a));oracle.write(oracle.OUTPUT+0x1100,struct.pack('<4d',*b))
                    for series in (False,True):
                        for log in (False,True):
                            oracle.write(oracle.OBJECT+8,struct.pack('<f',rate))
                            oracle.write(oracle.OUTPUT+0x2000,struct.pack('<5dIB3x',*w,int(log),int(series)))
                            oracle.write(oracle.OUTPUT,bytes(512))
                            oracle.call('0x25d40',oracle.OBJECT,oracle.OUTPUT,oracle.OUTPUT+0x1000,oracle.OUTPUT+0x1100,oracle.OUTPUT+0x2000,instruction_limit=200_000_000,timeout_us=10_000_000)
                            expected=' '.join(bits(v) for v in struct.unpack('<64d',oracle.read(oracle.OUTPUT,512)))
                            args=[str(paths[pair[0]]),str(paths[pair[1]]),'series' if series else 'parallel','log' if log else 'linear',ratebits]
                            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-tables',*args,str(wp)],text=True).strip()
                            assert actual==expected,('tables',w,rate,pair,series,log)
                            prefix=[*map(bits,a),*map(bits,b),ratebits,str(int(series)),str(int(log))]
                            tables.append(' '.join([*prefix,*map(bits,w),expected]))
                            for frozen in (False,True):
                                ww=(*w[:4],w[4]*(-1 if frozen else 1))
                                oracle.write(oracle.OBJECT,bytes(0x180));oracle.write(oracle.OBJECT+8,struct.pack('<f',rate))
                                oracle.write(oracle.OUTPUT+0x2000,struct.pack('<5dIB3x',*ww,int(log),int(series)))
                                oracle.call('0x262c0',oracle.OBJECT,oracle.OUTPUT+0x1000,oracle.OUTPUT+0x1100,oracle.OUTPUT+0x2000,instruction_limit=200_000_000,timeout_us=10_000_000)
                                expected=' '.join(f'{o:x}:{struct.unpack("<I",oracle.read(oracle.OBJECT+o,4))[0]:08x}' for o in offsets)
                                actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-calculated',*args,bits(ww[4]),str(wp)],text=True).strip()
                                assert actual==expected,('calculated',w,rate,pair,series,log,frozen)
                                calculated.append(' '.join([*prefix,*map(bits,ww),expected]))
                print('PASS: wiring',w,'rate',rate,flush=True)
    preserve(ROOT/'tests/fixtures/native-pickup-tables-wiring.txt', ('\n'.join(tables)+'\n').encode())
    preserve(ROOT/'tests/fixtures/native-pickup-calculated-wiring.txt', ('\n'.join(calculated)+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-wiring-{oracle.RUNTIME_VERSION}.json',(json.dumps({'source_sha256':oracle.sha256,'tables':len(tables),'calculated_outputs':len(calculated),'scope':'Synthetic varied loads/controls at two rates, two wiring modes and tapers, including frozen tone; all native fields exact; host-libm candidate only'},indent=2)+'\n').encode())
    print('PASS:',len(tables),'varied-wiring tables;',len(calculated),'varied-wiring calculated responses')


if __name__=='__main__':main()
