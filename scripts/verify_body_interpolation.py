"""Compare complete Rust interpolated body fields with original native execution.

This executes original InterpolateBetween and routing helpers, with no field or
routing replacements. Only explicit memory/libm runtime adapters are supplied.
Vendor-derived outputs remain ignored locally; committed fixtures are synthetic.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import argparse
from coefficient_oracle import CoefficientOracle, ROOT
from native_oracle import float_bits


def canonical(oracle,address,schema):
    output=bytearray()
    for f in schema:
        width,count=f['width'],f['count']
        raw=oracle.read(address+f['object_offset']-8,width*count)
        for i in range(count): output.extend(raw[i*width:(i+1)*width][::-1])
    assert len(output)==2897
    return bytes(output)


def synthetic_body(schema, mode):
    header = bytes([1])+b''.join(struct.pack('>8f',position,0.0,*[0.0]*6) for position in (1.0,3.0,7.0))+struct.pack('>I',mode)
    variants=[]
    for variant in range(6):
        raw=bytearray()
        for index,field in enumerate(schema):
            for n in range(field['count']):
                if field['type']=='bool':
                    raw.append((variant+index+n)%2)
                elif field['type']=='int':
                    name=field['name']
                    if name=='NumCoils': value=variant%5
                    elif name=='type': value=(variant//2+n)%3
                    elif name=='NumDroneStrings': value=variant+1
                    elif name=='analogGain': value=[0,3,0x7fffffff,0x80000000,0xffffffff,1][variant]
                    else: value=variant*3+index%11+n
                    raw.extend(struct.pack('>I',value))
                else:
                    value=100.0*(variant+1)+(index+1)*0.125+n*0.03125
                    if field['name'] in ('gain','resoGain','ResoGain'): value=-20.0+variant*3.0+n*0.125
                    if field['file_offset']==2067 and n==0 and variant in (0,2): value=0.0
                    raw.extend(struct.pack('>f',value))
        assert len(raw)==2897
        variants.append(bytes(raw))
    return header+b''.join(variants)


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--release',action='store_true');args=parser.parse_args()
    if args.release: subprocess.run(['cargo','build','--release'],cwd=ROOT,check=True)
    binary=ROOT/('target/release/variax-workbench' if args.release else 'target/debug/variax-workbench')
    oracle=CoefficientOracle();records=oracle.load_models()
    schema=[f for f in json.loads((ROOT/'analysis/reports/native-body-schema.json').read_text()) if 101<=f['file_offset']<2998]
    cases=[(0.,0.),(1.,1.),(0.,1.),(.5,.5),(.25,.75),(.75,.25),(.125,.375),(.625,.875),(.49999997,.50000006)]
    report=[]
    with tempfile.TemporaryDirectory(prefix='body-interpolate-') as temporary:
        for record in records:
            if record['kind']!='body': continue
            native_body=oracle.model_objects[('body',record['id'])]
            for i,(x,y) in enumerate(cases):
                oracle.write(oracle.OUTPUT,bytes(0xc90))
                oracle.call('0x1d050',oracle.OUTPUT)
                oracle.call('0x9d60',native_body,oracle.OUTPUT,float_bits(x),float_bits(y))
                expected=canonical(oracle,oracle.OUTPUT,schema)
                output=Path(temporary)/f'{record["id"]}-{i}.bin'
                result=subprocess.run([str(binary),'interpolate-body',str(ROOT/record['path']),str(output),f'{float_bits(x):08x}',f'{float_bits(y):08x}'],text=True,capture_output=True)
                if result.returncode: raise RuntimeError((record['name'],result.stderr))
                actual=output.read_bytes()
                if actual!=expected:
                    differences=[]
                    for f in schema:
                        start=f['file_offset']-101;width=f['width']
                        for n in range(f['count']):
                            offset=start+n*width
                            if actual[offset:offset+width]!=expected[offset:offset+width]:
                                differences.append({'name':f['name'],'offset':offset,'index':n,'type':f['type'],
                                    'rust':actual[offset:offset+width].hex(),'native':expected[offset:offset+width].hex()})
                    diagnostic={'body':record['name'],'x':x,'y':y,'difference_count':len(differences),'differences':differences}
                    (ROOT/'analysis/reports/body-interpolation-mismatch.json').write_text(json.dumps(diagnostic,indent=2)+'\n')
                    print(json.dumps({**diagnostic,'differences':differences[:12]},indent=2),flush=True)
                    raise SystemExit(1)
                report.append({'body':record['name'],'x_bits':f'{float_bits(x):08x}','y_bits':f'{float_bits(y):08x}',
                    'sha256':hashlib.sha256(actual).hexdigest(),'status':'pass'})
            print(f'PASS: {record["name"]}',flush=True)
    fixture = synthetic_body(schema,4)
    inputs_path=ROOT/'tests/fixtures/native-body-interpolation-input.bin'
    inputs_path.write_bytes(fixture)
    native_body=oracle.allocate(0x4bb4)
    oracle.call('0x9750',native_body)
    oracle.write(native_body+0x4b4c,struct.pack('<I',4))
    for variant in range(6):
        offset=101+variant*2897
        for f in schema:
            width,count=f['width'],f['count']
            raw=fixture[offset+f['file_offset']-101:offset+f['file_offset']-101+width*count]
            oracle.write(native_body+variant*0xc8c+f['object_offset'],b''.join(raw[i:i+width][::-1] for i in range(0,len(raw),width)))
    synthetic_outputs=bytearray();synthetic_rows=[]
    with tempfile.TemporaryDirectory(prefix='body-interpolate-synthetic-') as temporary:
        def check(command, label, call):
            oracle.write(oracle.OUTPUT,bytes(0xc90));oracle.call('0x1d050',oracle.OUTPUT)
            call()
            expected=canonical(oracle,oracle.OUTPUT,schema)
            output=Path(temporary)/f'{len(synthetic_rows)}.bin'
            result=subprocess.run([str(binary),*command[:1],str(inputs_path),str(output),*command[1:]],text=True,capture_output=True)
            if result.returncode: raise RuntimeError((label,result.stderr))
            actual=output.read_bytes()
            if actual!=expected:
                changes=[]
                for f in schema:
                    start=f['file_offset']-101;width=f['width']
                    for n in range(f['count']):
                        offset=start+n*width
                        if actual[offset:offset+width]!=expected[offset:offset+width]:
                            changes.append((f['name'],offset,actual[offset:offset+width].hex(),expected[offset:offset+width].hex()))
                print(label,changes[:20],flush=True)
                raise SystemExit(1)
            synthetic_rows.append(label);synthetic_outputs.extend(expected)
        for left,right in [(0,1),(1,0),(0,2),(2,0),(3,4),(4,3)]:
            for t in (0.0,0.125,0.5,0.875,1.0):
                label=f'blend {left} {right} {float_bits(t):08x}'
                check(['blend-body',str(left),str(right),f'{float_bits(t):08x}'],label,
                    lambda:oracle.call('0x213d0',oracle.OUTPUT,native_body+8+left*0xc8c,native_body+8+right*0xc8c,float_bits(t)))
        for mode in (0,1,2,3,4,37):
            inputs_path.write_bytes(synthetic_body(schema,mode))
            oracle.write(native_body+0x4b4c,struct.pack('<I',mode))
            for x,y in cases:
                label=f'route {mode} {float_bits(x):08x} {float_bits(y):08x}'
                check(['interpolate-body',f'{float_bits(x):08x}',f'{float_bits(y):08x}'],label,
                    lambda:oracle.call('0x9d60',native_body,oracle.OUTPUT,float_bits(x),float_bits(y)))
    inputs_path.write_bytes(fixture)
    (ROOT/'tests/fixtures/native-body-interpolated-output.bin').write_bytes(synthetic_outputs)
    (ROOT/'tests/fixtures/native-body-interpolated-cases.txt').write_text('\n'.join(synthetic_rows)+'\n')
    print(f'PASS: {len(synthetic_rows)} synthetic complete outputs, including disabled transitions and integer/coil handling',flush=True)
    (ROOT/'analysis/reports/body-interpolation-agreement.json').write_text(json.dumps({'native_source_sha256':oracle.sha256,
        'scope':'Full original field interpolation and routing with host libm/memory adapters; not DSP execution or physical audio validation',
        'floating_point':{'x87_control':'037f','x87_tag_initial':'ffff','mxcsr':'1f80'},
        'body_count':28,'case_count':len(report),'synthetic_case_count':len(synthetic_rows),'native_diagnostics':sorted(set(oracle.diagnostics)),'build':'release' if args.release else 'debug','records':report},indent=2)+'\n')
    print(f'PASS: {len(report)} complete body interpolation results, every serialized field bit-exact',flush=True)

if __name__=='__main__': main()
