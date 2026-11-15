"""Diagnose bounded private-emulator runs without relaxing either limit.

An optional previous native-output directory is a regression reference, not an
independent sound/hardware oracle. Stop at the first failure; retain the report.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import time

from unicorn import UC_QUERY_TIMEOUT
from coefficient_oracle import CoefficientOracle, ROOT
from test_coefficient_generator import cases
from inspect_firmware import preserve


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--expected-dir',type=Path)
    args=parser.parse_args()
    oracle=CoefficientOracle();models=oracle.load_models();records=[]
    for i,(name,settings) in enumerate(cases(models),1):
        record={'index':i,'case':name,'input_sha256':hashlib.sha256(settings).hexdigest(),
                'body_id':f'{struct.unpack_from("<I",settings,4)[0]:08x}',
                'pickup1_id':f'{struct.unpack_from("<I",settings,12)[0]:08x}'}
        started=time.monotonic()
        try:
            output=oracle.generate(settings)
            record['output_sha256']=hashlib.sha256(output).hexdigest()
            if args.expected_dir:
                stem=hashlib.sha256(name.encode()+b'\0'+bytes(settings)).hexdigest()[:16]
                assert output==(args.expected_dir/(stem+'.packed.bin')).read_bytes(),'previous native output differs'
            record['status']='pass'
        except Exception as error:
            record.update(status='fail',error=str(error),wall_timeout_hit=bool(oracle.cpu.query(UC_QUERY_TIMEOUT)))
        record['elapsed_seconds']=time.monotonic()-started;records.append(record)
        print(json.dumps(record),flush=True)
        if record['status']=='fail':break
    report={'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,
            'instruction_limit':200_000_000,'timeout_us':10_000_000,
            'scope':'Private native generator; original bounds retained; previous outputs are regression evidence only',
            'records':records,'passed':sum(r['status']=='pass' for r in records),
            'failed':sum(r['status']=='fail' for r in records)}
    data=(json.dumps(report,indent=2)+'\n').encode()
    path=ROOT/'analysis/reports'/('generator-limits-'+hashlib.sha256(data).hexdigest()[:16]+'.json')
    preserve(path,data);print(path,flush=True)
    if report['failed']:raise SystemExit(1)


if __name__=='__main__':main()
