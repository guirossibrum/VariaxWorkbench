"""Recheck legacy-fixture differences against the matching original math profile.

Only synthetic existing input vectors are used. New expected values come from
original Mac instructions with the disclosed software-math import adapter.
Legacy product fixtures are never changed or weakened.
"""
import hashlib
import json
import struct
import subprocess
import tempfile
from pathlib import Path
from unicorn.x86_const import UC_X86_REG_EAX,UC_X86_REG_EDX,UC_X86_REG_XMM0
from build_engine_math_variant import prepare,build
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from math_backends import RustSoftwareMath
from verify_body_interpolation import canonical


def original(label):
    helper=(ROOT/'.tools/rust-math-reference/target/release/libvariax_math_research.so').resolve()
    return CoefficientOracle(math_backend=RustSoftwareMath(helper,widen_float=True),runtime_version='initialized-software-component-'+label+'-v1')


def run(binary,*arguments):
    return subprocess.check_output([str(binary),*map(str,arguments)],text=True).strip()


def main():
    directory,source,environment,source_receipt=prepare()
    binaries={p:build(directory,source,environment,software=True,release=p=='release') for p in ['debug','release']}
    digest=hashlib.sha256(Path(__file__).read_bytes()).hexdigest();output=directory/'components'/digest[:16];output.mkdir(parents=True,exist_ok=True)
    results={};reference_fixtures={};fixture_hashes={}
    def fixture(name):
        data=(ROOT/'tests/fixtures'/name).read_bytes();fixture_hashes[name]=hashlib.sha256(data).hexdigest();return data
    oracle=original('biquad');rows=[];changed=0
    for line in fixture('native-dsp-biquad.txt').decode().splitlines():
        fields=line.split();kind,freq,q,rate=[int(v,16) for v in fields[:4]]
        oracle.write(oracle.OBJECT,struct.pack('<4I',kind,freq,q,0));oracle.cpu.reg_write(UC_X86_REG_EAX,oracle.OBJECT);oracle.cpu.reg_write(UC_X86_REG_EDX,oracle.OUTPUT);oracle.cpu.reg_write(UC_X86_REG_XMM0,rate);oracle.call('0x29290')
        expected=' '.join(f'{v:08x}' for v in struct.unpack('<5I',oracle.read(oracle.OUTPUT,20)))
        for profile,binary in binaries.items():
            actual=run(binary,'dsp-biquad',*fields[:4])
            if actual!=expected:raise AssertionError(('biquad',profile,fields[:4],actual,expected))
        rows.append(' '.join(fields[:4])+' '+expected);changed+=int(expected!=' '.join(fields[4:]))
    reference_fixtures['native-dsp-biquad.txt']=('\n'.join(rows)+'\n').encode();results['biquad']={'cases':len(rows),'changed_from_legacy':changed};print('BIQUAD',results['biquad'],flush=True)
    with tempfile.TemporaryDirectory(prefix='variax-software-components-') as temp:
        temp=Path(temp)
        for kind in ['tables','calculated']:
            oracle=original(kind)
            for suffix in ['', '-wiring']:
                name=f'native-pickup-{kind}{suffix}.txt';rows=[];changed=0
                for index,line in enumerate(fixture(name).decode().splitlines()):
                    fields=line.split();values=[struct.unpack('<d',int(v,16).to_bytes(8,'little'))[0] for v in fields[:8]];rate=int(fields[8],16);series=int(fields[9]);logarithmic=int(fields[10]);wiring=[struct.unpack('<d',int(v,16).to_bytes(8,'little'))[0] for v in fields[11:16]]
                    left=temp/f'{kind}{suffix}-{index}-left.bin';right=temp/f'{kind}{suffix}-{index}-right.bin';wp=temp/f'{kind}{suffix}-{index}-wiring.bin'
                    left.write_bytes(struct.pack('>4d',*values[:4])+bytes(53));right.write_bytes(struct.pack('>4d',*values[4:])+bytes(53));wp.write_bytes(struct.pack('<5d',*wiring))
                    if kind=='tables':
                        oracle.write(oracle.OBJECT+8,struct.pack('<I',rate));a=oracle.OBJECT+0x1000;b=oracle.OBJECT+0x1100;w=oracle.OBJECT+0x2000
                        oracle.write(a,struct.pack('<4d',*values[:4]));oracle.write(b,struct.pack('<4d',*values[4:]));oracle.write(w,struct.pack('<5dIB3x',*wiring,logarithmic,series));oracle.write(oracle.OUTPUT,bytes(512))
                        oracle.call('0x25d40',oracle.OBJECT,oracle.OUTPUT,a,b,w,instruction_limit=200_000_000,timeout_us=10_000_000)
                        expected=' '.join(f'{v:016x}' for v in struct.unpack('<64Q',oracle.read(oracle.OUTPUT,512)))
                    else:
                        oracle.write(oracle.OBJECT,bytes(0x180));oracle.write(oracle.OBJECT+8,struct.pack('<I',rate));a=oracle.OUTPUT+0x1000;b=oracle.OUTPUT+0x1100;w=oracle.OUTPUT+0x2000
                        oracle.write(a,struct.pack('<4d',*values[:4]));oracle.write(b,struct.pack('<4d',*values[4:]));oracle.write(w,struct.pack('<5dIB3x',*wiring,logarithmic,series))
                        oracle.call('0x262c0',oracle.OBJECT,a,b,w,instruction_limit=200_000_000,timeout_us=10_000_000)
                        offsets=[int(f.split(':')[0],16) for f in fields[16:]]
                        expected=' '.join(f'{o:x}:{struct.unpack("<I",oracle.read(oracle.OBJECT+o,4))[0]:08x}' for o in offsets)
                    args=['pickup-'+kind,left,right,'series' if series else 'parallel','log' if logarithmic else 'linear',fields[8]]
                    if kind=='calculated':args.append(fields[15])
                    args.append(wp)
                    for profile,binary in binaries.items():
                        actual=run(binary,*args)
                        if actual!=expected:raise AssertionError((name,index,profile,[(i,a,b) for i,(a,b) in enumerate(zip(actual.split(),expected.split())) if a!=b][:8]))
                    rows.append(' '.join(fields[:16])+' '+expected);changed+=int(expected!=' '.join(fields[16:]))
                reference_fixtures[name]=('\n'.join(rows)+'\n').encode();results[name]={'cases':len(rows),'changed_from_legacy':changed};print(name,results[name],flush=True)
        oracle=original('body');schema_bytes=(ROOT/'analysis/reports/native-body-schema.json').read_bytes();schema=[f for f in json.loads(schema_bytes) if 101<=f['file_offset']<2998]
        body=fixture('native-body-interpolation-input.bin');case_rows=fixture('native-body-interpolated-cases.txt').decode().splitlines();old=fixture('native-body-interpolated-output.bin');native_body=oracle.allocate(0x4bb4);oracle.call('0x9750',native_body)
        for variant in range(6):
            for field in schema:
                width,count=field['width'],field['count'];start=101+variant*2897+field['file_offset']-101;data=body[start:start+width*count]
                oracle.write(native_body+variant*0xc8c+field['object_offset'],b''.join(data[i:i+width][::-1] for i in range(0,len(data),width)))
        outputs=[];changed=0
        for index,line in enumerate(case_rows):
            f=line.split();input_body=bytearray(body);oracle.write(oracle.OUTPUT,bytes(0xc90));oracle.call('0x1d050',oracle.OUTPUT)
            if f[0]=='blend':
                oracle.call('0x213d0',oracle.OUTPUT,native_body+8+int(f[1])*0xc8c,native_body+8+int(f[2])*0xc8c,int(f[3],16));command=['blend-body',*f[1:]]
            elif f[0]=='route':
                mode=int(f[1]);input_body[97:101]=struct.pack('>I',mode);oracle.write(native_body+0x4b4c,struct.pack('<I',mode));oracle.call('0x9d60',native_body,oracle.OUTPUT,int(f[2],16),int(f[3],16));command=['interpolate-body',*f[2:]]
            else:raise RuntimeError('unknown body fixture case')
            expected=canonical(oracle,oracle.OUTPUT,schema);outputs.append(expected);changed+=int(expected!=old[index*2897:(index+1)*2897]);ip=temp/f'body-{index}.bin';ip.write_bytes(input_body)
            for profile,binary in binaries.items():
                op=temp/f'body-{index}-{profile}.bin';run(binary,command[0],ip,op,*command[1:]);actual=op.read_bytes()
                if actual!=expected:raise AssertionError(('body',index,profile,line,[(i,a,b) for i,(a,b) in enumerate(zip(actual,expected)) if a!=b][:8]))
        reference_fixtures['native-body-interpolated-output.bin']=b''.join(outputs);results['body']={'cases':len(outputs),'changed_from_legacy':changed,'schema_sha256':hashlib.sha256(schema_bytes).hexdigest()};print('BODY',results['body'],flush=True)
    for name,data in reference_fixtures.items():preserve(output/name,data)
    report={'scope':'Matching-profile component comparisons, synthetic inputs only; default product fixtures unchanged','source':source_receipt,'audit_sha256':digest,'input_fixtures_sha256':fixture_hashes,'results':results,'expected_output_sha256':{k:hashlib.sha256(v).hexdigest() for k,v in reference_fixtures.items()},'both_debug_and_release_match':True,'limitations':['Not new distinct-input coverage for issue #33','The four old-profile unit-test failures remain visible in the experimental variant; these checks use matching-profile original instructions','Not a production backend, cross-target or audio parity claim']}
    preserve(output/'report.json',(json.dumps(report,indent=2)+'\n').encode());print('COMPONENT AUDIT COMPLETE',output/'report.json',flush=True)


if __name__=='__main__':main()
