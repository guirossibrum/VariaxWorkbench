"""Evaluate a portable software-math candidate without changing product math."""
import collections
import hashlib
import json
import struct
from build_rust_math_reference import build
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from math_backends import RustSoftwareMath,MPFRMath,FUNCTIONS,bits,same_number
from test_coefficient_generator import cases
from audit_math_runtime import finite_ulp

VERSION='rust-software-math-audit-v2'


def main():
    libraries,receipt=build();backends={k:RustSoftwareMath(v) for k,v in libraries.items()}
    mpfr=MPFRMath((ROOT/'.tools/math-reference/libmpfr-reference.so').resolve())
    trace=json.loads((ROOT/'analysis/extracted/math-runtime-audit-v1/observed-math-inputs.json').read_text())
    statistics=collections.defaultdict(collections.Counter);differences=[]
    for i,row in enumerate(trace):
        name=row['function'];single=FUNCTIONS[name][2];fmt='<f' if single else '<d'
        args=[struct.unpack(fmt,bytes.fromhex(v))[0] for v in row['input_bits']]
        debug=getattr(backends['debug'],name)(*args);release=getattr(backends['release'],name)(*args)
        if bits(debug,single)!=bits(release,single):raise RuntimeError(('debug/release candidate mismatch',name,row['input_bits']))
        reference=getattr(mpfr,name)(*args);info=statistics[name];info['inputs']+=1
        host_changed=bits(release,single)!=row['result_bits'];mpfr_changed=bits(release,single)!=bits(reference,single)
        info['bit_differences_from_glibc']+=int(host_changed);info['bit_differences_from_mpfr256']+=int(mpfr_changed)
        info['numerical_differences_from_mpfr256']+=int(not same_number(release,reference))
        ulp=finite_ulp(release,reference,single)
        if ulp is not None:info['max_finite_ulp_vs_mpfr256']=max(info['max_finite_ulp_vs_mpfr256'],ulp)
        if host_changed or mpfr_changed:differences.append({**row,'rust_bits':bits(release,single),'mpfr256_bits':bits(reference,single)})
    directory=ROOT/'analysis/extracted'/VERSION;directory.mkdir(parents=True,exist_ok=True)
    preserve(directory/'differences.json',(json.dumps(differences,indent=2)+'\n').encode())
    print('INPUTS COMPLETE',len(trace),'debug/release identical',flush=True)
    full=[]
    try:oracle=CoefficientOracle(math_backend=backends['release'],runtime_version='initialized-'+VERSION)
    except RuntimeError as error:full.append({'status':'initialization-failed','error':str(error)})
    else:
        models=oracle.load_models();baseline_dir=ROOT/'analysis/extracted/coefficient-reference'/oracle.sha256/'initialized-host-libm-v2'
        canonical=json.loads((ROOT/'analysis/reports/coefficient-generator-tests-initialized-host-libm-v2-budget200m.json').read_text())
        expected={(r['case'],r['input_sha256']):r['output_sha256'] for r in canonical['records']}
        for i,(name,settings) in enumerate(cases(models)):
            try:output=oracle.generate(settings)
            except Exception as error:full.append({'case':name,'status':'failed','error':str(error)});break
            stem=hashlib.sha256(name.encode()+b'\0'+bytes(settings)).hexdigest()[:16]
            baseline=(baseline_dir/f'{stem}.packed.bin').read_bytes()
            assert hashlib.sha256(baseline).hexdigest()==expected[(name,hashlib.sha256(settings).hexdigest())]
            word=lambda b,j:int.from_bytes(b[j:j+3],'big')
            changed=[j//3 for j in range(0,len(output),3) if word(output,j)!=word(baseline,j)]
            full.append({'case':name,'status':'computed','sha256':hashlib.sha256(output).hexdigest(),'changed_words_including_checksum':changed})
            preserve(directory/f'{i}.packed',output);print('PATCH',i+1,name,len(changed),'changed words',flush=True)
    report={'scope':'Software-only Rust math comparison; no production backend adoption or cross-target parity claim',
        'build':receipt,'input_count':len(trace),'debug_release_input_results_identical':True,'functions':dict(statistics),'full_mac_generator_substitution':full,
        'limitations':['Linux x86_64 only; Windows/macOS and other architectures/compilers are untested','Candidate is not authenticated historical Mac math','Full generator substitutes math imports into original Mac code; it is not complete Rust compiler arithmetic','Canonical references and hardware locks are unchanged; no production dependency was added']}
    preserve(ROOT/'analysis/reports'/f'{VERSION}.json',(json.dumps(report,indent=2)+'\n').encode());print('RUST SOFTWARE MATH AUDIT COMPLETE',flush=True)


if __name__=='__main__':main()
