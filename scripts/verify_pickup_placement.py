"""Check original host coil placement, bypassing separately-tested electrical DSP."""
import json
import struct
import subprocess
from coefficient_oracle import CoefficientOracle,default_settings,ROOT
from inspect_firmware import preserve
from native_oracle import float_bits


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();models=oracle.load_models()
    # Exclude electrical fitting and final HW setter assembly from this modular
    # geometry reference. Original body interpolation/host geometry still run.
    for address in (0x262c0,0x23a0):oracle.write(address,b'\xc3')
    bodies=[r for r in models if r['kind']=='body'];pickups=[r for r in models if r['kind']=='pickup']
    body=next(r for r in bodies if r['id']==0x3100003)
    count=0
    for pickup in pickups:
        for position,angle,phase in [(2.,-1.,False),(4.,0.,True),(6.,1.,False)]:
            b=default_settings();struct.pack_into('<I',b,0xc,pickup['id']&0xffffffff)
            struct.pack_into('<f',b,0x18,position);struct.pack_into('<f',b,0x20,angle);b[0x30]=int(phase)
            oracle.write(oracle.OUTPUT+0x2000,bytes(b));oracle.call('0x180a0',oracle.OUTPUT)
            oracle.call('0x2c40',oracle.OBJECT,oracle.OUTPUT,oracle.OUTPUT+0x2000,instruction_limit=20_000_000,timeout_us=10_000_000)
            geom=oracle.OBJECT+0xf38;coils=struct.unpack('<I',oracle.read(geom+0x10,4))[0]
            assert coils in (1,2)
            words=[coils]
            for c in range(coils):
                words.extend(struct.unpack('<6I',oracle.read(geom+0x24+c*24,24)))
                words.extend(oracle.read(geom+0x84+c*6,6))
            expected=' '.join(f'{v:08x}' for v in words)
            args=[str(ROOT/pickup['path']),str(ROOT/body['path']),f'{float_bits(position):08x}',f'{float_bits(angle):08x}',str(int(phase)),'0']
            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-place',*args],text=True).strip()
            assert actual==expected,(pickup['name'],position,angle,phase,actual,expected)
            count+=1
        print('PASS:',pickup['name'],flush=True)
    synthetic_body=(ROOT/'tests/fixtures/native-body-definition.bin').read_bytes()
    synthetic_pickup=(ROOT/'tests/fixtures/native-pickup-definition.bin').read_bytes()
    bodyobj=oracle.model_objects[('body',body['id'])]
    for i in range(3):
        source=synthetic_body[1+i*32:1+(i+1)*32]
        oracle.write(bodyobj+0x4b50+i*32,b''.join(source[n:n+4][::-1] for n in range(0,32,4)))
    oracle.write(bodyobj+0x4bb0,synthetic_body[:1])
    pickupid=0x3110001;obj=oracle.model_objects[('pickup',pickupid)]
    oracle.write(obj+4,b''.join(synthetic_pickup[n:n+8][::-1] for n in range(0,32,8)))
    oracle.write(obj+0x24,synthetic_pickup[33:37][::-1]);oracle.write(obj+0x40,synthetic_pickup[32:33])
    for start,offset in [(37,0x28),(61,0x44)]:
        oracle.write(obj+offset,b''.join(synthetic_pickup[n:n+4][::-1] for n in range(start,start+24,4)))
    vectors=[];synthetic_count=0
    for position in (.25,.625,1.):
        for angle in (-1.,0.,1.):
            for phase in (False,True):
                for second in (False,True):
                    for alternate in (False,True):
                        b=default_settings();struct.pack_into('<2I',b,0xc,pickupid,pickupid)
                        struct.pack_into('<4f',b,0x18,position,1.25-position,angle,-angle)
                        b[0x30]=int(phase);b[0x31]=int(not phase);b[9]=int(second);b[0x4c]=int(alternate)
                        oracle.write(oracle.OUTPUT+0x2000,bytes(b));oracle.call('0x180a0',oracle.OUTPUT)
                        oracle.call('0x2c40',oracle.OBJECT,oracle.OUTPUT,oracle.OUTPUT+0x2000,instruction_limit=20_000_000,timeout_us=10_000_000)
                        geom=oracle.OBJECT+0xf38;coils=struct.unpack('<I',oracle.read(geom+0x10,4))[0]
                        single=second and alternate
                        expected_count=(1 if single else 2)*(2 if second else 1)
                        assert coils==expected_count,(coils,expected_count,second,alternate)
                        offset=0
                        for pos,ang,ph in [(position,angle,phase)]+([(1.25-position,-angle,not phase)] if second else []):
                            args=[str(ROOT/'tests/fixtures/native-pickup-definition.bin'),str(ROOT/'tests/fixtures/native-body-definition.bin'),f'{float_bits(pos):08x}',f'{float_bits(ang):08x}',str(int(ph)),str(int(single))]
                            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-place',*args],text=True).strip()
                            n=1 if single else 2;words=[n]
                            for c in range(offset,offset+n):
                                words.extend(struct.unpack('<6I',oracle.read(geom+0x24+c*24,24)));words.extend(oracle.read(geom+0x84+c*6,6))
                            expected=' '.join(f'{v:08x}' for v in words)
                            assert actual==expected,(pos,ang,ph,single,actual,expected)
                            vectors.append(' '.join([args[2],args[3],args[4],args[5],expected]))
                            offset+=n
                        synthetic_count+=1
    preserve(ROOT/'tests/fixtures/native-pickup-placement.txt',('\n'.join(dict.fromkeys(vectors))+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-placement-{oracle.RUNTIME_VERSION}-with-synthetic.json',(json.dumps({'source_sha256':oracle.sha256,'cases':count,'synthetic_host_cases':synthetic_count,'synthetic_unique_placement_vectors':len(set(vectors)),'adapters':{'0x262c0':'return without electrical fitting','0x23a0':'return without hardware setters'},'scope':'43 bundled models on body 0x3100003 plus synthetic two-pickup/alternate-tuning coil-collapse cases; placement, body offsets, angle, phase and coil counts, not level orchestration'},indent=2)+'\n').encode())
    print('PASS:',count,'vendor host placements;',synthetic_count,'synthetic host cases;',len(set(vectors)),'unique synthetic placement vectors')


if __name__=='__main__':main()
