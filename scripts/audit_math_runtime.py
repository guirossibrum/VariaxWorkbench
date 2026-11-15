"""Measure reference-runtime math sensitivity without changing canonical output.

Linux glibc, MPFR (two working precisions), and a recompiled Apple source
candidate are compared. No backend is promoted to production or treated as an
original macOS system library. Original-derived traces stay ignored locally.
"""
import collections
import hashlib
import json
import math
import platform
import struct
import sys
from pathlib import Path
import unicorn
from unicorn.x86_const import UC_X86_REG_FPCW,UC_X86_REG_FPTAG,UC_X86_REG_MXCSR
from build_math_reference import build
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from math_backends import AppleSourceMath,MPFRMath,TraceMath,FUNCTIONS,bits,same_number,mapped_libraries
from test_coefficient_generator import cases


def controls(oracle):
    return {'x87_control':oracle.cpu.reg_read(UC_X86_REG_FPCW),'x87_tags':oracle.cpu.reg_read(UC_X86_REG_FPTAG),'mxcsr':oracle.cpu.reg_read(UC_X86_REG_MXCSR)}


def finite_ulp(a,b,single):
    if not math.isfinite(a) or not math.isfinite(b):return None
    width=32 if single else 64;mask=(1<<width)-1;sign=1<<(width-1)
    def ordered(value):
        value=int.from_bytes(bytes.fromhex(bits(value,single)),'little')
        return ((~value)&mask) if value&sign else value|sign
    return abs(ordered(a)-ordered(b))


def main():
    apple_path,mpfr_path,build_record=build()
    mpfr=MPFRMath(mpfr_path,256);precise=MPFRMath(mpfr_path,512);apple=AppleSourceMath(apple_path)
    environment=mpfr.environment();assert environment['host_rounding_mode']==0,'non-nearest host rounding is not an audited environment'
    trace=TraceMath();oracle=CoefficientOracle(math_backend=trace,runtime_version='initialized-glibc-trace-audit-v1');models=oracle.load_models()
    reference=json.loads((ROOT/'analysis/reports/coefficient-generator-tests-initialized-host-libm-v2-budget200m.json').read_text())
    canonical={(r['case'],r['input_sha256']):r['output_sha256'] for r in reference['records']}
    corpus=list(cases(models));outputs=[];control_snapshots=[]
    for i,(name,settings) in enumerate(corpus):
        output=oracle.generate(settings);assert hashlib.sha256(output).hexdigest()==canonical[(name,hashlib.sha256(settings).hexdigest())],('tracing changed canonical output',name)
        outputs.append(output);control_snapshots.append(controls(oracle));print('TRACE',i+1,name,flush=True)
    trace_records=list(trace.records.values())
    print('TRACE COMPLETE:',len(trace_records),'unique argument tuples',flush=True)
    comparison=[];summaries=collections.defaultdict(collections.Counter)
    for i,record in enumerate(trace_records):
        name=record['function'];single=FUNCTIONS[name][2];fmt='<f' if single else '<d'
        values=[struct.unpack(fmt,bytes.fromhex(v))[0] for v in record['input_bits']]
        host=struct.unpack(fmt,bytes.fromhex(record['result_bits']))[0]
        high=getattr(mpfr,name)(*values);higher=getattr(precise,name)(*values);historical=getattr(apple,name)(*values)
        if bits(high,single)!=bits(higher,single) and not (math.isnan(high) and math.isnan(higher)):
            raise RuntimeError(('MPFR 256/512 rounding disagrees',name,record['input_bits']))
        summary=summaries[name];summary['unique_inputs']+=1;summary['calls']+=record['calls']
        high_diff=bits(host,single)!=bits(high,single);apple_diff=bits(host,single)!=bits(historical,single)
        summary['glibc_vs_mpfr_bit_differences']+=int(high_diff);summary['glibc_vs_apple_bit_differences']+=int(apple_diff)
        summary['glibc_vs_mpfr_numerical_differences']+=int(not same_number(host,high))
        summary['glibc_vs_apple_numerical_differences']+=int(not same_number(host,historical))
        ulp=finite_ulp(host,high,single)
        if ulp is not None:summary['max_glibc_vs_mpfr_finite_ulp']=max(summary['max_glibc_vs_mpfr_finite_ulp'],ulp)
        if high_diff or apple_diff:comparison.append({**record,'mpfr_bits':bits(high,single),'apple_source_bits':bits(historical,single),'glibc_vs_mpfr_finite_ulp':ulp})
        if (i+1)%10000==0:print('MATH',i+1,'/',len(trace_records),flush=True)
    namespace='math-runtime-audit-v1';directory=ROOT/'analysis/extracted'/namespace;directory.mkdir(parents=True,exist_ok=True)
    preserve(directory/'observed-math-inputs.json',(json.dumps(trace_records,indent=2)+'\n').encode())
    preserve(directory/'math-differences.json',(json.dumps(comparison,indent=2)+'\n').encode())
    full={}
    for name,backend in [('mpfr256',mpfr),('apple92-recompiled',apple)]:
        candidate=CoefficientOracle(math_backend=backend,runtime_version=f'initialized-{name}-audit-v1');candidate.load_models();records=[]
        for i,((case,settings),baseline) in enumerate(zip(corpus,outputs)):
            try:actual=candidate.generate(settings)
            except Exception as error:
                records.append({'case':case,'status':'failed','error':str(error),'last_pc':hex(candidate.last_pc or 0)});print('CANDIDATE FAILED',name,case,str(error),flush=True);break
            words=lambda b:[int.from_bytes(b[i:i+3],'big') for i in range(0,len(b),3)]
            differences=[j for j,(a,b) in enumerate(zip(words(actual),words(baseline))) if a!=b]
            records.append({'case':case,'status':'computed','sha256':hashlib.sha256(actual).hexdigest(),'differs_from_glibc':bool(differences),'changed_words_including_checksum':differences,'controls':controls(candidate)})
            preserve(directory/f'{name}-{i}.packed',actual)
            print('CANDIDATE',name,i+1,case,len(differences),'changed words',flush=True)
        full[name]=records
    after=mpfr.environment();assert after['host_rounding_mode']==0
    report={'source_sha256':oracle.sha256,'canonical_runtime':'initialized-host-libm-v2','canonical_output_unchanged_by_trace':True,
        'oracle_controls_after_startup':controls(CoefficientOracle()),'oracle_controls_after_each_canonical_call':control_snapshots,
        'environment_before':environment,'environment_after':after,'mapped_library_sha256':mapped_libraries(),'python':sys.version,'platform':platform.platform(),'unicorn':unicorn.__version__,
        'comparison_build':build_record,'canonical_cases':len(corpus),'unique_math_inputs':len(trace_records),'math_summaries':dict(summaries),'mpfr_256_512_target_rounding_agrees':True,'full_candidates':full,
        'limitations':['No original macOS runtime binary has been supplied or authenticated','Apple Libm-92 is a separately recompiled source candidate, not established as the Workbench runtime','MPFR precision agreement is an independent mathematical comparison, not macOS or Windows behavior','Original embedded Windows math comparison is separate; complete Windows generator/JVM execution remains pending','NaN payload/zero-sign differences are counted separately from numerical differences','No canonical references, production math backend, audio claims or hardware locks are changed']}
    preserve(ROOT/'analysis/reports/math-runtime-audit-v1.json',(json.dumps(report,indent=2)+'\n').encode())
    print('AUDIT COMPLETE:',len(corpus),'canonical cases;',len(trace_records),'unique math input tuples',flush=True)


if __name__=='__main__':main()
