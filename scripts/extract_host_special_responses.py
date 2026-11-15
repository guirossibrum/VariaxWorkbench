"""Extract two host response-table snapshots into ignored local analysis files.

Only original host instructions execute in private CPU memory. This neither
contacts a device nor reads firmware from one. Do not publish the snapshots.
"""
import hashlib
import json
import struct
from coefficient_oracle import CoefficientOracle,ROOT,default_settings
from inspect_firmware import preserve


def main():
    oracle=CoefficientOracle();oracle.load_models();initial=oracle.read(oracle.OBJECT,0x1134)
    directory=ROOT/'analysis/extracted/host-special-responses'/oracle.sha256/oracle.RUNTIME_VERSION
    directory.mkdir(parents=True,exist_ok=True);records=[]
    for id,resistance,cap in [(-3,1e6,2.2e-8),(-7,500000.,4.7e-8)]:
        oracle.write(oracle.OBJECT,initial);settings=default_settings();struct.pack_into('<I',settings,0x14,id&0xffffffff)
        for offset,value in [(0x38,resistance),(0x3c,resistance),(0x40,cap)]:struct.pack_into('<f',settings,offset,value)
        oracle.generate(settings);snapshot=oracle.read(oracle.OBJECT+0xdd0,360)
        assert struct.unpack_from('<f',snapshot,8)[0]==39062.5
        output=directory/f'{id}.bin';preserve(output,snapshot)
        records.append({'alternate_id':id,'file':str(output.relative_to(ROOT)),'sha256':hashlib.sha256(snapshot).hexdigest()})
        print(f'special\t{id&0xffffffff:08x}\t{output.resolve()}')
    preserve(directory/'provenance.json',(json.dumps({'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,'internal_rate':39062.5,'scope':'Local original host table snapshots only; not firmware, audio equivalence or hardware readiness. Keep these vendor-derived tables private.','records':records},indent=2)+'\n').encode())


if __name__=='__main__':main()
