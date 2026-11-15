"""Offline native complex circuit transfer versus Rust, without runtime imports."""
import json
import struct
import subprocess
from pathlib import Path
from native_oracle import NativeOracle, ROOT
from unicorn.x86_const import UC_X86_REG_MXCSR,UC_X86_REG_FPCW,UC_X86_REG_FPTAG


def words(value): return struct.unpack('<2I',struct.pack('<d',value))
def bits(value): return struct.pack('>d',value).hex()

def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=NativeOracle()
    oracle.cpu.reg_write(UC_X86_REG_MXCSR,0x1f80);oracle.cpu.reg_write(UC_X86_REG_FPCW,0x37f);oracle.cpu.reg_write(UC_X86_REG_FPTAG,0xffff)
    wiring=(3.5e-10,1e6,500000.,500000.,2.2e-8)
    oracle.write(oracle.OBJECT+0x2000,struct.pack('<5d',*wiring))
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/pickups').glob('*.bin'))
    count=0;filter_count=0
    for path in paths:
        raw=path.read_bytes()[:32]
        # Native full-generator clamps values before calculating the circuit.
        values=list(struct.unpack('>4d',raw))
        if values[2]==0: continue
        oracle.write(oracle.OBJECT+0x1000,b''.join(raw[i:i+8][::-1] for i in range(0,32,8)))
        for frequency in (40.,110.,440.,1000.,5000.,10000.):
            for volume in (0.125,0.5,1.):
                for tone in (0.,0.5,1.):
                    oracle.call('0x24be0',oracle.OUTPUT,oracle.OBJECT,oracle.OBJECT+0x1000,oracle.OBJECT+0x2000,*words(frequency),*words(volume),*words(tone))
                    expected=struct.unpack('<2Q',oracle.read(oracle.OUTPUT,16))
                    actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-response',str(path),bits(frequency),bits(volume),bits(tone)],text=True).strip()
                    wanted=' '.join(f'{v:016x}' for v in expected)
                    if actual!=wanted:
                        print(path.name,frequency,volume,tone,'rust',actual,'native',wanted,flush=True)
                        raise SystemExit(1)
                    count+=1
        for volume in (0.125,0.5,1.):
            for tone in (0.,0.5,1.):
                oracle.call('0x259a0',oracle.OBJECT,oracle.OBJECT+0x1000,oracle.OBJECT+0x2000,oracle.OUTPUT,*words(volume),*words(tone),instruction_limit=20_000_000,timeout_us=10_000_000)
                expected=' '.join(f'{v:016x}' for v in struct.unpack('<3Q',oracle.read(oracle.OUTPUT,24)))
                actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-filter1',str(path),bits(volume),bits(tone)],text=True).strip()
                assert actual==expected,(path.name,volume,tone,actual,expected)
                filter_count+=1
        print('PASS:',path.name,flush=True)
    circuits=[(5000.,100000.,2.5,1e-10),(8000.,250000.,4.25,2e-10),
              (100.,1e6,0.05,1e-14),(1e6,1e6,100.,0.005)]
    wirings=[wiring,(5.25e-10,470000.,250000.,250000.,4.7e-8),(8.75e-10,1e6,25000.,1e6,1e-9)]
    vectors=[]
    for circuit in circuits:
        oracle.write(oracle.OBJECT+0x1000,struct.pack('<4d',*circuit))
        for w in wirings:
            oracle.write(oracle.OBJECT+0x2000,struct.pack('<5d',*w))
            for volume in (0.125,0.5,1.):
                for tone in (0.,0.5,1.):
                    for f in (40.,440.,5000.,19000.):
                        oracle.call('0x24be0',oracle.OUTPUT,oracle.OBJECT,oracle.OBJECT+0x1000,oracle.OBJECT+0x2000,*words(f),*words(volume),*words(tone))
                        output=struct.unpack('<2d',oracle.read(oracle.OUTPUT,16))
                        vectors.append('response '+' '.join(bits(v) for v in (*circuit,*w,f,volume,tone,*output)))
                    oracle.call('0x259a0',oracle.OBJECT,oracle.OBJECT+0x1000,oracle.OBJECT+0x2000,oracle.OUTPUT,*words(volume),*words(tone),instruction_limit=20_000_000,timeout_us=10_000_000)
                    output=struct.unpack('<3d',oracle.read(oracle.OUTPUT,24))
                    vectors.append('filter '+' '.join(bits(v) for v in (*circuit,*w,volume,tone,*output)))
    fixture = ROOT/'tests/fixtures/native-pickup-response.txt'
    contents = '\n'.join(vectors)+'\n'
    if fixture.exists():
        if fixture.read_text() != contents:
            raise RuntimeError('reference fixture differs; preserve it and investigate before creating a versioned replacement')
    else:
        with fixture.open('x') as stream:
            stream.write(contents)
    (ROOT/'analysis/reports/pickup-response-agreement.json').write_text(json.dumps({'source_sha256':oracle.sha256,'function':'0x24be0','cases':count,'primary_filter_cases':filter_count,'synthetic_vectors':len(vectors),'status':'pass','scope':'Raw circuit transfer and primary peak/Q/gain fit; not secondary filters, full tables, geometry or audio/device execution'},indent=2)+'\n')
    print('PASS:',count,'bit-exact responses;',filter_count,'primary fits;',len(vectors),'synthetic vectors')

if __name__=='__main__':main()
