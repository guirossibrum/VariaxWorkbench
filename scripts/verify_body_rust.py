"""Compare every typed Rust body field and position normalization with native code.

Original native serialization supplies the full canonical body-byte reference.
Original SSE normalization executes without math/runtime adapters. All inputs and
outputs are files/private emulator memory; no device interface is available.
"""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from native_oracle import NativeOracle, ROOT, float_bits
from verify_native_bodies import native_read
from unicorn.x86_const import UC_X86_REG_FPCW, UC_X86_REG_FPTAG


def run(*args):
    return subprocess.check_output([str(ROOT/'target/debug/variax-workbench'),*map(str,args)],text=True).strip()


def as_float(value):
    return struct.unpack('<f',struct.pack('<f',value))[0]


def adjacent(value,direction):
    bits=float_bits(value)
    return struct.unpack('<f',struct.pack('<I',bits+direction))[0]


def native_normalize(oracle,payload,position):
    oracle.cpu.reg_write(UC_X86_REG_FPCW,0x37f)
    oracle.cpu.reg_write(UC_X86_REG_FPTAG,0xffff)
    for file_offset,object_offset in [(1,0x4b50),(33,0x4b70),(65,0x4b90)]:
        oracle.write(oracle.OBJECT+object_offset,payload[file_offset:file_offset+4][::-1])
    oracle.write(oracle.OBJECT+0x4bb0,payload[:1])
    oracle.call('0x9b60',oracle.OBJECT,float_bits(position))
    # Drain the original x87 ABI return to float32 in private memory.
    output=oracle.OUTPUT+0x8000
    capture=oracle.RETURN+0x100
    oracle.write(capture,b'\xd9\x1d'+struct.pack('<I',output))
    oracle.cpu.emu_start(capture,capture+6,count=1,timeout=2_000_000)
    return struct.unpack('<I',oracle.read(output,4))[0]


def main():
    subprocess.run(['cargo','build'],cwd=ROOT,check=True)
    paths=sorted((ROOT/'analysis/extracted/windows-app/modules/variax/device/coefeng/data/bodies').glob('*.bin'))
    assert len(paths)==28
    paths.append(ROOT/'tests/fixtures/native-body-definition.bin')
    rows=[]
    synthetic_vectors=[]
    with tempfile.TemporaryDirectory(prefix='variax-body-rust-') as temporary:
        for index,path in enumerate(paths):
            payload=path.read_bytes()
            expected,fields=native_read(payload)
            output=Path(temporary)/f'{index}.bin'
            run('body-roundtrip',path,output)
            assert output.read_bytes()==expected==payload,path
            bridge,middle,neck=[struct.unpack_from('>f',payload,o)[0] for o in (1,33,65)]
            positions=[bridge-1,neck+1,*[bridge+(neck-bridge)*i/16 for i in range(17)]]
            for anchor in (bridge,middle,neck): positions.extend([adjacent(anchor,-1),anchor,adjacent(anchor,1)])
            inputs=sorted({float_bits(as_float(v)) for v in positions})
            oracle=NativeOracle()
            for bits in inputs:
                position=struct.unpack('<f',struct.pack('<I',bits))[0]
                wanted=native_normalize(oracle,payload,position)
                actual=int(run('normalize-body',path,f'{bits:08x}'),16)
                assert actual==wanted,(path,position,f'{actual:08x}',f'{wanted:08x}')
                if path.name=='native-body-definition.bin':
                    synthetic_vectors.append(f'{bits:08x} {wanted:08x}')
            rows.append({'file':str(path.relative_to(ROOT)),'source_sha256':hashlib.sha256(payload).hexdigest(),
                         'typed_roundtrip':'matches every native serialized field','normalization_bit_exact_cases':len(inputs),
                         'has_middle':payload[0]!=0})
    report={'native_source_sha256':NativeOracle().sha256,'vendor_bodies':28,'synthetic_bodies':1,
            'normalization_cases':sum(r['normalization_bit_exact_cases'] for r in rows),'records':rows,
            'scope':'Full typed Rust decoding/reserialization versus original native serialization, and bit-exact SSE normalization; no coefficient blending or hardware'}
    (ROOT/'analysis/reports/body-rust-native-agreement.json').write_text(json.dumps(report,indent=2)+'\n')
    (ROOT/'tests/fixtures/native-body-normalization.txt').write_text('\n'.join(synthetic_vectors)+'\n')
    print(f'PASS: all typed fields in 28 vendor bodies + synthetic fixture; {report["normalization_cases"]} bit-exact native normalization comparisons')

if __name__=='__main__': main()
