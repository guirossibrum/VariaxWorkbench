"""Differential raw-body all-block assembly, excluding Java/model orchestration."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from synthetic_dsp_body import make_body
from verify_native_bodies import native_read


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    oracle=CoefficientOracle();schema=json.loads((ROOT/'analysis/reports/native-body-schema.json').read_text())
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/bodies').glob('*.bin'));assert len(paths)==28
    records=[];synthetic_outputs=[];synthetic_inputs=[]
    with tempfile.TemporaryDirectory(prefix='dsp-body-variants-') as temporary:
        synthetic_paths=[]
        for seed in range(8):
            payload=make_body(seed);canonical,_=native_read(payload);assert canonical==payload
            p=Path(temporary)/f'synthetic-{seed}.bin';p.write_bytes(payload);synthetic_paths.append(p);synthetic_inputs.append(payload)
        for path in [*paths,*synthetic_paths]:
            payload=path.read_bytes()
            for variant in range(6):
                raw=bytearray(3212);start=101+variant*2897;object_start=8+variant*3212
                fields=[f for f in schema if start<=f['file_offset']<start+2897]
                for field in fields:
                    width=field['width'];count=field['count'];source=payload[field['file_offset']:field['file_offset']+width*count]
                    offset=field['object_offset']-object_start
                    assert 0<=offset and offset+width*count<=len(raw)
                    raw[offset:offset+width*count]=b''.join(source[i:i+width][::-1] for i in range(0,len(source),width))
                oracle.write(oracle.OBJECT,bytes(0x1134));oracle.call('0x6770',oracle.OBJECT)
                assert struct.unpack('<I',oracle.read(oracle.OBJECT+8,4))[0]==0x47189680
                oracle.write(oracle.OUTPUT+0x2000,bytes(raw));oracle.call('0x1820',oracle.OBJECT,oracle.OUTPUT+0x2000,instruction_limit=20_000_000,timeout_us=10_000_000)
                oracle.call('0x180a0',oracle.OUTPUT);oracle.call('0x23a0',oracle.OBJECT,oracle.OUTPUT,instruction_limit=20_000_000,timeout_us=10_000_000)
                expected=oracle.read(oracle.OUTPUT,1091*4)
                output=Path(temporary)/f'{len(records)}.bin'
                process=subprocess.run([str(ROOT/'target/debug/variax-workbench'),'dsp-body-variant',str(path),str(variant),str(output)],text=True,capture_output=True)
                assert process.returncode==0,(path.name,variant,process.stderr)
                actual=output.read_bytes()
                if actual!=expected:
                    a=struct.unpack('<1091I',actual);b=struct.unpack('<1091I',expected);diffs=[(i,f'{x:08x}',f'{y:08x}') for i,(x,y) in enumerate(zip(a,b)) if x!=y]
                    raise AssertionError((path.name,variant,diffs[:12]))
                if path in synthetic_paths:synthetic_outputs.append(expected)
                records.append({'body':path.name,'variant':variant,'output_sha256':hashlib.sha256(actual).hexdigest()})
            print('PASS:',path.name,'six variants',flush=True)
    preserve(ROOT/'tests/fixtures/native-dsp-body-inputs.bin',b''.join(synthetic_inputs))
    preserve(ROOT/'tests/fixtures/native-dsp-body-outputs.bin',b''.join(synthetic_outputs))
    preserve(ROOT/f'analysis/reports/dsp-body-variants-{oracle.RUNTIME_VERSION}-synthetic-v2.json',(json.dumps({'source_sha256':oracle.sha256,'cases':len(records),'native_default_rate_bits':'47189680','native_default_rate':39062.5,'synthetic_bodies':len(synthetic_inputs),'synthetic_outputs':len(synthetic_outputs),'scope':'All 17 blocks from raw typed body variants: original UpdateBlocksFromPatch/GenerateHardwarePatch and fresh native engine defaults, not full model interpolation/Java/preset orchestration or audio. Every 1091 logical word compared.', 'records':records},indent=2)+'\n').encode())
    print('PASS:',len(records),'complete raw-body all-block assemblies')


if __name__=='__main__':main()
