"""Compare generated whole-Rust-engine math variants with original instructions.

Identity rewriting must reproduce the existing canonical corpus. The software
variant is compared independently with original-machine-code software-math
substitution; original-derived resources/results stay ignored locally.
"""
import hashlib
import json
import struct
import subprocess
import tempfile
from pathlib import Path
from build_engine_math_variant import prepare,build
from coefficient_oracle import CoefficientOracle,ROOT
from inspect_firmware import preserve
from math_backends import RustSoftwareMath,MPFRMath
from test_coefficient_generator import cases
from verify_host_synthetic import BODY_IDS


def changed_words(actual,expected):
    if len(actual)!=3276 or len(expected)!=3276:raise RuntimeError('wrong packed output size')
    return [i//3 for i in range(0,3276,3) if actual[i:i+3]!=expected[i:i+3]]


def batch(binary,inputs,records):
    with tempfile.TemporaryDirectory(prefix='variax-engine-math-') as temporary:
        directory=Path(temporary);manifest=directory/'models.tsv';input_path=directory/'inputs.bin';output=directory/'outputs.bin'
        manifest.write_text('\n'.join(f'{r["kind"]}\t{r["id"]&0xffffffff:08x}\t{(ROOT/r["path"]).resolve()}' for r in records)+'\n')
        input_path.write_bytes(b''.join(inputs))
        result=subprocess.run([str(binary),'host-generate-batch',str(input_path),str(manifest),str(output)],capture_output=True,text=True)
        if result.returncode:raise RuntimeError(result.stderr)
        data=output.read_bytes()
        if len(data)!=len(inputs)*3276:raise RuntimeError('batch output size mismatch')
        return [data[i:i+3276] for i in range(0,len(data),3276)]


def verify(binaries,directory,receipt,*,scope='Generated complete Rust engine source variant; not production adoption, historical Mac authentication, cross-target or audio parity'):
    mpfr=MPFRMath((ROOT/'.tools/math-reference/libmpfr-reference.so').resolve())
    env=mpfr.environment()
    if env['host_rounding_mode']!=0 or env['host_mxcsr']&0xffc0!=0x1f80:raise RuntimeError('nearest rounding without subnormal flushing required')
    oracle=CoefficientOracle();models=oracle.load_models();corpus=list(cases(models));names=[n for n,_ in corpus];inputs=[bytes(s) for _,s in corpus]
    canonical_report=json.loads((ROOT/'analysis/reports/coefficient-generator-tests-initialized-host-libm-v2-budget200m.json').read_text())
    canonical={(r['case'],r['input_sha256']):r['output_sha256'] for r in canonical_report['records']}
    original=[];software_original=[]
    software_report=json.loads((ROOT/'analysis/reports/rust-software-widened-float-audit-v1.json').read_text())['full_mac_generator_substitution']
    if len(software_report)!=len(corpus):raise RuntimeError('software reference corpus length changed')
    for i,(name,data) in enumerate(corpus):
        stem=hashlib.sha256(name.encode()+b'\0'+bytes(data)).hexdigest()[:16]
        output=(ROOT/'analysis/extracted/coefficient-reference'/oracle.sha256/'initialized-host-libm-v2'/f'{stem}.packed.bin').read_bytes()
        if hashlib.sha256(output).hexdigest()!=canonical[(name,hashlib.sha256(data).hexdigest())]:raise RuntimeError('canonical output hash changed')
        candidate=(ROOT/'analysis/extracted/rust-software-widened-float-audit-v1'/f'{i}.packed').read_bytes()
        row=software_report[i]
        if row['case']!=name or row['status']!='computed' or hashlib.sha256(candidate).hexdigest()!=row['sha256']:raise RuntimeError('software reference hash or case changed')
        original.append(output);software_original.append(candidate)
    result_directory=directory/'results';result_directory.mkdir(parents=True,exist_ok=True);results={}
    for label,binary in binaries.items():
        outputs=batch(binary,inputs,models);records=[]
        for i,(name,actual) in enumerate(zip(names,outputs)):
            diff=changed_words(actual,original[i]);soft_diff=changed_words(actual,software_original[i])
            if label.startswith('identity-') and diff:raise RuntimeError(('identity rewrite changed canonical behavior',name,diff))
            records.append({'case':name,'sha256':hashlib.sha256(actual).hexdigest(),'changed_words_vs_canonical':diff,'changed_words_vs_original_software_math':soft_diff})
        preserve(result_directory/f'{label}.bin',b''.join(outputs));results[label]=records
        print(label,'CASES',len(records),'canonical matches',sum(not r['changed_words_vs_canonical'] for r in records),'software-oracle matches',sum(not r['changed_words_vs_original_software_math'] for r in records),flush=True)
    if (result_directory/'software-debug.bin').read_bytes()!=(result_directory/'software-release.bin').read_bytes():raise RuntimeError('software engine debug/release disagree')
    # The original synthetic definitions are public fixtures, not vendor data.
    bodies=(ROOT/'tests/fixtures/native-dsp-body-inputs.bin').read_bytes();pickups=(ROOT/'tests/fixtures/native-host-pickups.bin').read_bytes();resources=directory/'synthetic-resources';resources.mkdir(parents=True,exist_ok=True);records=[]
    for kind,ids,payload,size in [('body',BODY_IDS,bodies,17483),('pickup',[0x3110001,0x3110000,-2,-3],pickups,85)]:
        if len(payload)!=len(ids)*size:raise RuntimeError('synthetic resource fixture size changed')
        for i,id in enumerate(ids):
            data=payload[i*size:(i+1)*size];path=resources/f'{kind}-{i}.bin';preserve(path,data)
            records.append({'kind':kind,'id':id,'path':str(path),'sha256':hashlib.sha256(data).hexdigest()})
    synthetic_inputs=(ROOT/'tests/fixtures/native-host-inputs.bin').read_bytes()
    if len(synthetic_inputs)!=48*192:raise RuntimeError('synthetic input corpus changed')
    synthetic_inputs=[synthetic_inputs[i:i+192] for i in range(0,len(synthetic_inputs),192)]
    helper=(ROOT/'.tools/rust-math-reference/target/release/libvariax_math_research.so').resolve()
    reference=CoefficientOracle(math_backend=RustSoftwareMath(helper,widen_float=True),runtime_version='initialized-rust-engine-synthetic-wide-math-v1');reference.load_models(records)
    expected=[]
    for i,settings in enumerate(synthetic_inputs):
        expected.append(reference.generate(settings));print('SYNTHETIC ORACLE',i+1,flush=True)
    preserve(result_directory/'synthetic-original-software.bin',b''.join(expected));synthetic={}
    legacy=(ROOT/'tests/fixtures/native-host-outputs.bin').read_bytes()
    if len(legacy)!=48*3276:raise RuntimeError('legacy synthetic output size changed')
    for label in binaries:
        outputs=batch(binaries[label],synthetic_inputs,records)
        profile_expected=expected if label.startswith('software-') else [legacy[i:i+3276] for i in range(0,len(legacy),3276)]
        synthetic[label]=[{'case':i,'changed_words':changed_words(a,b),'sha256':hashlib.sha256(a).hexdigest()} for i,(a,b) in enumerate(zip(outputs,profile_expected))]
        preserve(result_directory/f'synthetic-{label}.bin',b''.join(outputs));print('SYNTHETIC',label,'matches',sum(not r['changed_words'] for r in synthetic[label]),'/',len(outputs),flush=True)
    host_math={}
    forbidden={'sin','sinf','cos','cosf','tan','tanf','exp','expf','log','logf','log10','log10f','pow','powf','atan2','atan2f','sqrt','sqrtf','fma','fmaf'}
    for label,binary in binaries.items():
        undefined=subprocess.check_output(['llvm-nm','--undefined-only',str(binary)],text=True)
        names={line.split()[-1].split('@')[0] for line in undefined.splitlines() if line.split()}
        host_math[label]=sorted(names&forbidden)
    report={'scope':scope,
        'source':receipt,'audit_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'environment':env,'host_math_imports':host_math,'binaries':{k:{'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for k,p in binaries.items()},
        'full_vendor_resource_cases':results,'synthetic_cases':synthetic,'software_debug_release_vendor_outputs_equal':True,
        'limitations':['44 counted method aliases, with identity behavior checked before candidate results','Software square root is also substituted; original Mac sqrt instructions are not an imported math adapter','Input corpus is unchanged; this is not new distinct-sample coverage for issue #33','No original assets or actual device traffic are published or used live','Product core/manifest/default behavior stay unchanged; wider-float double rounding and cross-platform compiler behavior remain unverified']}
    data=(json.dumps(report,indent=2)+'\n').encode();path=directory/f'engine-math-report-{hashlib.sha256(data).hexdigest()[:16]}.json';preserve(path,data)
    if any(r['changed_words_vs_original_software_math'] for label,rows in results.items() if label.startswith('software-') for r in rows) or any(r['changed_words'] for rows in synthetic.values() for r in rows) or any(host_math[label] for label in ['software-debug','software-release']):
        raise AssertionError(('software engine mismatch or host-math import; inspect report',str(path)))
    print('ENGINE MATH AUDIT COMPLETE',path,flush=True)


def main():
    directory,source,environment,receipt=prepare();binaries={}
    for name,software,release in [('identity-debug',False,False),('software-debug',True,False),('software-release',True,True)]:
        binaries[name]=build(directory,source,environment,software=software,release=release)
    verify(binaries,directory,receipt)


if __name__=='__main__':main()
