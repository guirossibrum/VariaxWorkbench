"""Compare electric level/trim balancing with original host calls, offline."""
import json
import struct
import subprocess
from coefficient_oracle import CoefficientOracle,default_settings,ROOT
from native_oracle import float_bits
from inspect_firmware import preserve


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();models=oracle.load_models()
    for address in (0x262c0,0x23a0):oracle.write(address,b'\xc3')
    body=oracle.model_objects[('body',0x3100003)]
    first=oracle.model_objects[('pickup',0x3110001)];second=oracle.model_objects[('pickup',0x3110000)]
    combined_record=next(r for r in models if r['kind']=='pickup' and r['id']<0)
    combined=oracle.model_objects[('pickup',combined_record['id'])]
    for obj in (first,second):oracle.write(obj+0x40,b'\0')
    schema=json.loads((ROOT/'analysis/reports/native-body-schema.json').read_text());gain_offsets=[r['object_offset'] for r in schema if r['name']=='continuousGain'];assert len(gain_offsets)==6
    vectors=[]
    for a,b in [(-3.5,0.),(5.,-6.),(0.,0.)]:
        oracle.write(first+0x24,struct.pack('<f',a));oracle.write(second+0x24,struct.pack('<f',b));oracle.write(combined+0x24,struct.pack('<f',2.5))
        for bodygain in (-12.,0.,6.):
            for offset in gain_offsets:oracle.write(body+offset,struct.pack('<f',bodygain))
            for trim in (-12.,0.,18.):
                for reverse in (False,True):
                    for both in (False,True):
                        for combo in (False,True):
                            s=default_settings();s[9]=int(both)
                            struct.pack_into('<2f',s,0x18,*( (2.,6.) if reverse else (6.,2.) ))
                            struct.pack_into('<2f',s,0x28,-12.,3.)
                            struct.pack_into('<f',s,0xbc,trim)
                            if combo:struct.pack_into('<I',s,0x14,combined_record['id']&0xffffffff)
                            oracle.write(oracle.OUTPUT+0x2000,bytes(s));oracle.call('0x180a0',oracle.OUTPUT)
                            oracle.call('0x2c40',oracle.OBJECT,oracle.OUTPUT,oracle.OUTPUT+0x2000,instruction_limit=20_000_000,timeout_us=10_000_000)
                            gains=struct.unpack('<2I',oracle.read(oracle.OBJECT+0xf38+0x14,8))
                            trimword=struct.unpack('<I',oracle.read(oracle.OBJECT+0xbbc+0x38,4))[0]
                            fields=[f'{float_bits(a):08x}',f'{float_bits(b):08x}' if both else 'none',f'{float_bits(-12.):08x}',f'{float_bits(3.):08x}',f'{float_bits(2.5):08x}' if combo else 'none',f'{float_bits(bodygain):08x}',f'{float_bits(trim):08x}',str(int(reverse))]
                            actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'pickup-levels',*fields],text=True).strip().split()
                            # Inactive second pickup has no coil setter; its
                            # intermediate balanced level is not observable here.
                            expected=[f'{gains[0]:08x}',f'{gains[1]:08x}',f'{trimword:08x}']
                            assert actual[0]==expected[0] and actual[2]==expected[2],(fields,actual,expected)
                            if both:assert actual[1]==expected[1],(fields,actual,expected)
                            vectors.append(' '.join([*fields,expected[0],expected[1] if both else 'unobserved',expected[2]]))
    preserve(ROOT/'tests/fixtures/native-pickup-levels-observed.txt', ('\n'.join(vectors)+'\n').encode())
    preserve(ROOT/f'analysis/reports/pickup-levels-{oracle.RUNTIME_VERSION}-observed.json',(json.dumps({'source_sha256':oracle.sha256,'cases':len(vectors),'scope':'Electric host levels and trim: synthetic offsets/levels/constant body gains; native electrical fitting/final setters bypassed. Inactive second intermediate level is not independently compared.'},indent=2)+'\n').encode())
    print('PASS:',len(vectors),'electric level/trim cases')


if __name__=='__main__':main()
