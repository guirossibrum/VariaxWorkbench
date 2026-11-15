"""Exercise the complete original native coefficient path offline.

These are host-runtime-adapted reference candidates, not proof of Rust parity or
hardware/audio validity. Synthetic native inputs bypass Java ApplyHacks and final
Java metadata overlay; cases explicitly described as Java-shaped set those flags.
"""
import ctypes.util
import hashlib
import json
import platform
import struct
import sys
from coefficient_oracle import CoefficientOracle, default_settings, ROOT
from inspect_firmware import preserve


def changed(settings, offset, fmt, value):
    result=bytearray(settings)
    struct.pack_into(fmt,result,offset,value)
    return result


def cases(records):
    baseline=default_settings()
    yield 'baseline',baseline
    acoustic={0x3100012,0x3100013,0x3100014,0x3100015,0x3100016,0x3100017,0x310001a,0x310001b}
    twelve={0x310000d,0x3100013,0x3100015}
    for record in records:
        if record['kind']=='body':
            b=changed(baseline,4,'<I',record['id'])
            b[0]=int(record['id'] in acoustic)
            b[1]=int(record['id'] in twelve)
            b[0xa4]=int(record['id']==0x3100018)
            struct.pack_into('<I',b,0xa0,3 if b[0] else (2 if b[0xa4] else 1))
            yield 'body/'+record['name'],b
        else:
            b=changed(baseline,0xc,'<I',record['id'] & 0xffffffff)
            yield 'pickup/'+record['name'],b
    for name,offset,values in [
        ('position',0x18,[0.5,2.0,4.0,6.0,8.0]),
        ('angle_normalized',0x20,[-1.0,0.0,1.0]),
        ('pickup_level_db',0x28,[-20.0,0.0,6.0]),
        ('volume_resistance',0x38,[25000.0,250000.0,1000000.0]),
        ('tone_resistance',0x3c,[25000.0,250000.0,1000000.0]),
        ('tone_capacitance',0x40,[1e-9,2.2e-8,4.7e-8]),
        ('tone_position',0x98,[0.0,0.5,1.0]),
        ('trim_db',0xbc,[-20.0,0.0,6.0])]:
        for value in values:
            yield f'{name}/{value}',changed(baseline,offset,'<f',value)
    for value in (0.0,0.5,1.0):
        b=changed(baseline,0x98,'<f',value); b[0x9c]=1
        yield f'saved-tone-position/{value}',b
    for mode in ('parallel','series','phase-reversed'):
        b=bytearray(baseline); b[9]=1; b[0x32]=int(mode=='series'); b[0x31]=int(mode=='phase-reversed')
        yield 'two-pickups/'+mode,b
    for semitones in (-12.0,-2.0,0.0,7.0,12.0):
        b=changed(baseline,0x50,'<f',semitones); b[0x4c]=1
        for i in range(6): struct.pack_into('<f',b,0x68+i*4,1.0)
        yield f'alt-tuning/{semitones}',b
    yield 'baseline-after-sweeps',baseline


def main():
    oracle=CoefficientOracle()
    models=oracle.load_models()
    directory=ROOT/'analysis/extracted/coefficient-reference'/oracle.sha256/oracle.RUNTIME_VERSION
    directory.mkdir(parents=True,exist_ok=True)
    records=[]
    baseline_output=None
    for i,(name,settings) in enumerate(cases(models)):
        print(f'[{i+1}] {name}',flush=True)
        try:
            output=oracle.generate(settings)
            words=[int.from_bytes(output[n:n+3],'big') for n in range(0,len(output),3)]
            assert len(words)==1092
            assert sum(words[:-1]) & 0xffffff == words[-1], 'packed additive checksum mismatch'
            if baseline_output is None: baseline_output=output
            if name=='baseline-after-sweeps': assert output==baseline_output, 'state-dependent baseline output'
            stem=hashlib.sha256(name.encode()+b'\0'+bytes(settings)).hexdigest()[:16]
            preserve(directory/f'{stem}.input.bin',bytes(settings))
            preserve(directory/f'{stem}.packed.bin',output)
            records.append({'case':name,'status':'pass','input_sha256':hashlib.sha256(settings).hexdigest(),
                            'output_sha256':hashlib.sha256(output).hexdigest(),'checksum':f'{words[-1]:06x}',
                            'differs_from_baseline':output!=baseline_output,'fixture_stem':stem})
        except Exception as error:
            records.append({'case':name,'status':'fail','error':str(error),'last_pc':hex(oracle.last_pc or 0)})
            # Do not continue a potentially invalid CPU state after failure.
            break
    if records[-1]['status']=='pass':
        saved_tone=[r['output_sha256'] for r in records if r['case'].startswith('saved-tone-position/')]
        assert len(set(saved_tone))==3, 'saved tone positions failed to vary complete output'
        fresh=CoefficientOracle();fresh.load_models()
        assert fresh.generate(default_settings())==baseline_output, 'fresh-instance baseline mismatch'
    report={'source_sha256':oracle.sha256,'runtime_version':oracle.RUNTIME_VERSION,'instruction_budget':200_000_000,
            'module_initializers_completed':len(oracle.initializers_completed),'destructor_registrations_captured':len(oracle.destructor_registrations),'scope':'Original complete native generation and packing; not a Rust port, Java metadata overlay or hardware test',
            'runtime_adapters':'Private allocation/memory and local host libm, Mach-O symbol resolution; unknown imports fail closed',
            'host':{'python':sys.version,'platform':platform.platform(),'libm':ctypes.util.find_library('m')},
            'models_loaded':len(models),'fixture_directory':str(directory.relative_to(ROOT)),
            'import_calls':dict(oracle.import_calls),'cold_and_warm_baseline_equal':records[-1]['status']=='pass',
            'records':records,'passed':sum(r['status']=='pass' for r in records),'failed':sum(r['status']=='fail' for r in records),
            'limitations':['Host libm may differ in rounding from historical macOS libm',
                           'Synthetic native inputs bypass Java ApplyHacks and final metadata overlay',
                           'Internal negative-ID pickup combinations are exercised as synthetic primary pickups',
                           'Checksum and successful execution do not prove audio validity or Rust parity']}
    preserve(ROOT/f'analysis/reports/coefficient-generator-tests-{oracle.RUNTIME_VERSION}-budget200m.json', (json.dumps(report,indent=2)+'\n').encode())
    print(f'{report["passed"]} passed; {report["failed"]} failed; runtime: {oracle.RUNTIME_VERSION}',flush=True)
    if report['failed']: raise SystemExit(1)

if __name__=='__main__':
    main()
