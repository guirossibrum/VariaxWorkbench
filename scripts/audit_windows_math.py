"""Compare original embedded Windows math against observed Mac import inputs.

Full patch comparisons use the original *Mac* generator with Windows intrinsic
substitution, isolating math effects. They are not full Windows application
execution and do not establish its compiler/JVM/CRT initialization semantics.
"""
import collections
import hashlib
import json
import math
import struct
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from math_backends import WindowsIntrinsicMath,FUNCTIONS,bits,same_number
from test_coefficient_generator import cases
from windows_math_oracle import WindowsMathOracle


def main():
    trace=json.loads((ROOT/'analysis/extracted/math-runtime-audit-v1/observed-math-inputs.json').read_text())
    summaries={};variants={};directory=ROOT/'analysis/extracted/windows-math-audit-v1';directory.mkdir(parents=True,exist_ok=True)
    for label,control,dispatch in [('sse2-53',0x27f,'initialized-sse2'),('sse2-64',0x37f,'initialized-sse2'),('x87-image-53',0x27f,'image-default-x87')]:
        backend=WindowsIntrinsicMath(control,dispatch);stats=collections.defaultdict(collections.Counter);differences=[];failed=None
        for i,row in enumerate(trace):
            name=row['function'];single=FUNCTIONS[name][2];fmt='<f' if single else '<d'
            args=[struct.unpack(fmt,bytes.fromhex(v))[0] for v in row['input_bits']]
            glibc=struct.unpack(fmt,bytes.fromhex(row['result_bits']))[0]
            try:actual=getattr(backend,name)(*args)
            except Exception as error:failed={'input_index':i,'function':name,'input_bits':row['input_bits'],'error':str(error)};break
            info=stats[name];info['inputs']+=1;info['calls_in_mac_trace']+=row['calls']
            changed=bits(actual,single)!=row['result_bits'];info['bit_differences_from_glibc']+=int(changed);info['numerical_differences_from_glibc']+=int(not same_number(actual,glibc))
            if changed:differences.append({**row,'windows_intrinsic_bits':bits(actual,single)})
            if (i+1)%10000==0:print(label,'INPUTS',i+1,'/',len(trace),flush=True)
        summaries[label]={'original_pe_sha256':WindowsMathOracle.SHA256,'control_word':control,'dispatch':dispatch,'original_sse2_selection':backend.oracle.sse2_selected,'adapters':backend.oracle.adapters,'functions':dict(stats),'compared_inputs':sum(v['inputs'] for v in stats.values()),'failed':failed}
        preserve(directory/f'{label}-differences.json',(json.dumps(differences,indent=2)+'\n').encode());variants[label]=backend
        print(label,'INPUT COMPARISON COMPLETE',summaries[label]['compared_inputs'],'failed',failed,flush=True)
    reference=json.loads((ROOT/'analysis/reports/coefficient-generator-tests-initialized-host-libm-v2-budget200m.json').read_text())
    canonical={(r['case'],r['input_sha256']):r['output_sha256'] for r in reference['records']}
    full={}
    for label in ['sse2-53','x87-image-53']:
        backend=variants[label];records=[]
        try:oracle=CoefficientOracle(math_backend=backend,runtime_version=f'initialized-windows-intrinsics-{label}-audit-v1')
        except Exception as error:
            full[label]=[{'status':'initialization-failed','error':str(error)}]
            print(label,'INITIALIZATION STOPPED',str(error),flush=True)
            continue
        models=oracle.load_models()
        baseline_dir=ROOT/'analysis/extracted/coefficient-reference'/oracle.sha256/'initialized-host-libm-v2'
        for i,(name,settings) in enumerate(cases(models)):
            try:output=oracle.generate(settings)
            except Exception as error:records.append({'case':name,'status':'failed','error':str(error)});break
            stem=hashlib.sha256(name.encode()+b'\0'+bytes(settings)).hexdigest()[:16]
            baseline=(baseline_dir/f'{stem}.packed.bin').read_bytes();assert hashlib.sha256(baseline).hexdigest()==canonical[(name,hashlib.sha256(settings).hexdigest())]
            words=lambda b:[int.from_bytes(b[j:j+3],'big') for j in range(0,len(b),3)]
            changed=[j for j,(a,b) in enumerate(zip(words(output),words(baseline))) if a!=b]
            records.append({'case':name,'status':'computed','sha256':hashlib.sha256(output).hexdigest(),'changed_words_including_checksum':changed})
            preserve(directory/f'{label}-{i}.packed',output);print(label,'PATCH',i+1,name,len(changed),'changed words',flush=True)
        full[label]=records
    report={'scope':'Original Windows CRT machine-code routines with private errno, original SSE selection, forbidden OS imports; original Mac generator with those routine results substituted for import calls. Not complete Windows generator/JVM execution or an original Mac system runtime.',
        'pe_sha256':WindowsMathOracle.SHA256,'mac_sha256':'3aa232329d467101637764397b77a7b8ffa6af0ccd72ec380d02f145477ba9fe','observed_math_input_count':len(trace),'variants':summaries,'full_math_substitution':full,
        'limitations':['Original Mac libSystem binary is not available','Float aliases isolate the result width of the Mac import, not complete Windows compiler arithmetic','x87-image mode preserves the file default flag; initialized-sse2 runs original CPU-selection instructions','FPU precision modes are explicit comparison conditions, not an observation from a running Windows JVM','Private errno replaces only the CRT pointer getter; all OS imports remain forbidden','Canonical references, production dependencies and hardware access remain unchanged']}
    preserve(ROOT/'analysis/reports/windows-math-audit-v1.json',(json.dumps(report,indent=2)+'\n').encode());print('WINDOWS MATH AUDIT COMPLETE',flush=True)


if __name__=='__main__':main()
