"""Separate Apple 88.3.9 host-generator candidate; existing references unchanged."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from apple_tiger_math_oracle import AppleTigerCoefficientOracle, AppleTigerHostX87CoefficientOracle
from coefficient_oracle import ROOT
from inspect_firmware import preserve
from test_coefficient_generator import cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path)
    parser.add_argument('--current-host-x87', action='store_true', help='separate owned-native-instruction candidate, no vendor native load')
    args = parser.parse_args()
    oracle = (AppleTigerHostX87CoefficientOracle if args.current_host_x87 else AppleTigerCoefficientOracle)(args.library)
    models = oracle.load_models()
    reference_path = ROOT / 'analysis/reports/coefficient-generator-tests-initialized-host-libm-v2-budget200m.json'
    reference_bytes = reference_path.read_bytes()
    canonical = {(record['case'], record['input_sha256']): record['output_sha256']
                 for record in json.loads(reference_bytes)['records']}
    folder = ROOT / 'analysis/extracted' / oracle.RUNTIME_VERSION
    folder.mkdir(parents=True, exist_ok=True)
    results, failure = [], None
    corpus = list(cases(models))
    if len(corpus) != 110:
        raise ValueError('whole-generator corpus changed')
    for i, (name, settings) in enumerate(corpus):
        try:
            packed = oracle.generate(settings)
        except Exception as error:
            failure = {'case':name, 'error':str(error), 'last_pc':hex(oracle.last_pc or 0)}
            break
        digest = hashlib.sha256(packed).hexdigest()
        input_sha = hashlib.sha256(settings).hexdigest()
        preserve(folder / f'{i:03}-{input_sha}.packed', packed)
        results.append({'case':name, 'input_sha256':input_sha, 'output_sha256':digest,
                        'matches_recorded_glibc':digest == canonical[(name,input_sha)]})
        print('CANDIDATE', i + 1, '/', len(corpus), name, 'same' if results[-1]['matches_recorded_glibc'] else 'different', flush=True)
    report = {'status':'stopped' if failure else 'pass', 'runtime_version':oracle.RUNTIME_VERSION,
              'coefficient_source_sha256':oracle.sha256, 'math_source_sha256':oracle.libm.SHA256,
              'canonical_report_sha256':hashlib.sha256(reference_bytes).hexdigest(),
              'scope':'original host generator with pinned Apple guest math through declared-width adapters; not a native Mac process, device firmware or audio',
              'startup_knob_constants_bits':[struct.pack('<d',v).hex() for v in oracle.EXPECTED_KNOB_CONSTANTS],
              'initializers':len(oracle.initializers_completed), 'captured_destructor_callbacks':len(oracle.destructor_registrations),
              'caller_fp_environments':oracle.libm.environment_counts, 'pure_math_imports':oracle.libm.import_calls,
              'math_value_cache_entries':len(oracle.libm.value_cache), 'math_value_cache_hits':oracle.libm.cache_hits,
              'native_x87_helper':getattr(getattr(oracle.libm,'native_x87',None),'record',None),
              'native_x87_calls':getattr(oracle.libm,'native_x87_calls',0),
              'records':results, 'failure':failure,
              'limits':['Source is a dated Apple update, not this user\'s exact original installed OS',
                        'Existing canonical reports/default application math are unchanged',
                        'Math return values cross the explicit float/double host adapter; complete ABI/fenv/teardown/JVM equivalence is not established',
                        'Unicorn x87 transcendental helpers call host math; original Apple control flow alone does not authenticate physical x87 results',
                        'No hardware, OS-service forwarding, native DLL/library load or installer execution']}
    encoded = (json.dumps(report, indent=2) + '\n').encode()
    path = ROOT / 'analysis/reports' / ('apple-tiger-generator-' + hashlib.sha256(encoded).hexdigest()[:16] + '.json')
    preserve(path, encoded)
    print(path, flush=True)
    if failure:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
