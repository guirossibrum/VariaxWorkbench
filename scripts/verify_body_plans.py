"""Record native interpolation routing, deliberately replacing field blending.

Original mode selection, triangle/bilinear control flow and float32 weights run.
InterpolateBetween/CopyFrom are replaced in private emulator memory with trace
adapters. This validates selection and nesting only, NOT coefficient blending.
The committed fixture contains synthetic coordinates/variant indices only.
"""
import json
import struct
import subprocess
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP
from coefficient_oracle import CoefficientOracle, ROOT
from native_oracle import float_bits


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle()
    pointers=[oracle.OBJECT+0x2000+i*0x1000 for i in range(6)]
    descriptor=oracle.OBJECT+0x100
    result=oracle.OUTPUT
    expressions={}
    def blend(cpu,address,size,context):
        stack=cpu.reg_read(UC_X86_REG_ESP)
        out,left,right,weight=struct.unpack('<4I',oracle.read(stack+4,16))
        expressions[out]=f'B({weight:08x},{expressions[left]},{expressions[right]})'
    def copy(cpu,address,size,context):
        stack=cpu.reg_read(UC_X86_REG_ESP)
        out,source=struct.unpack('<2I',oracle.read(stack+4,8))
        expressions[out]=expressions[source]
    for address,handler in [(0x213d0,blend),(0x1d130,copy)]:
        oracle.write(address,b'\xc3')
        oracle.cpu.hook_add(UC_HOOK_CODE,handler,begin=address,end=address)
    # Include exact transition coordinates, adjacent floats and signed zero.
    coordinates=[0.0,-0.0,0.0001,0.25,struct.unpack('<f',struct.pack('<I',0x3effffff))[0],
                 0.5,struct.unpack('<f',struct.pack('<I',0x3f000001))[0],0.75,0.9999,1.0]
    rows=[]
    for mode in (0,1,2,3,4,37):
        oracle.write(descriptor,struct.pack('<8I',result,mode,*pointers))
        for x in coordinates:
            for y in coordinates:
                expressions.clear();expressions.update({p:f'V{i}' for i,p in enumerate(pointers)})
                oracle.call('0x94f0',descriptor,float_bits(x),float_bits(y))
                expected=expressions[result]
                actual=subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),'body-plan',str(mode),f'{float_bits(x):08x}',f'{float_bits(y):08x}'],text=True).strip()
                assert actual==expected,(mode,x,y,actual,expected)
                rows.append(f'{mode} {float_bits(x):08x} {float_bits(y):08x} {expected}')
    fixture=ROOT/'tests/fixtures/native-body-interpolation-plans.txt'
    fixture.write_text('\n'.join(rows)+'\n')
    report={'source_sha256':oracle.sha256,'comparisons':len(rows),
            'scope':'Native routing helpers with field-blending/copy trace adapters; not coefficient results',
            'modes':[0,1,2,3,4,37],'fixture':str(fixture.relative_to(ROOT)),
            'adapters':{'0x213d0':'record nested InterpolateBetween call, then return','0x1d130':'record CopyFrom source, then return'},
            'status':'pass'}
    (ROOT/'analysis/reports/body-interpolation-plan-agreement.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f'PASS: {len(rows)} bit-exact native mode/triangle/bilinear routing plans (field blending intentionally untested)')

if __name__=='__main__': main()
