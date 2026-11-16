"""Compare pinned Apple 88.3.9 guest math to the private canonical input trace.

Pure CPU research; does not change application math or canonical references.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import struct
import time

from apple_tiger_math_oracle import AppleTigerMath
from math_backends import FUNCTIONS, bits
from inspect_firmware import preserve


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path)
    parser.add_argument('trace', type=Path)
    parser.add_argument('--pilot-per-function', type=int, default=0)
    args = parser.parse_args()
    if not 0 <= args.pilot_per_function <= 1000:
        raise ValueError('pilot bound is 0 (all) or 1..1000')
    trace_bytes = args.trace.read_bytes()
    if hashlib.sha256(trace_bytes).hexdigest() != '13dcfbf977114421578f4430f411eb64167a513af6ed774105b3ddb5f0a882ec':
        raise ValueError('requires the pinned original-input trace fingerprint')
    records = json.loads(trace_bytes)
    if len(records) != 206_228:
        raise ValueError('requires the established 206228-tuple canonical trace')
    if args.pilot_per_function:
        grouped = collections.defaultdict(list)
        for record in records:
            grouped[record['function']].append(record)
        records = []
        for group in grouped.values():
            n = min(len(group), args.pilot_per_function)
            records.extend(group[(i * len(group)) // n] for i in range(n))
    oracle = AppleTigerMath(args.library)
    started = time.monotonic()
    differences, summaries, failures = [], collections.defaultdict(collections.Counter), []
    for i, record in enumerate(records):
        name = record['function']
        single = FUNCTIONS[name][2]
        fmt = '<f' if single else '<d'
        values = [struct.unpack(fmt, bytes.fromhex(value))[0] for value in record['input_bits']]
        try:
            result = bits(oracle.evaluate(name, *values), single)
        except Exception as error:
            failures.append({'index':i, 'function':name, 'input_bits':record['input_bits'], 'error':str(error)})
            break  # Never continue a CPU whose unmodeled path has stopped.
        summary = summaries[name]
        summary['distinct_inputs'] += 1
        summary['source_trace_calls'] += record['calls']
        if result != record['result_bits']:
            summary['bit_differences_from_recorded_glibc'] += 1
            differences.append({**record, 'apple_tiger_bits':result})
        if (i + 1) % 1000 == 0:
            print('MATH', i + 1, '/', len(records), flush=True)
        if time.monotonic() - started > 900:
            failures.append({'index':i, 'error':'research wall-clock bound exceeded'})
            break
    report = {'status':'stopped' if failures else 'pass', 'scope':'isolated original i386 math, no native loading/OS forwarding/hardware; not a full generator comparison',
              'source_sha256':oracle.SHA256, 'source_identity':'libSystem 88.3.9 from Apple 10.4.11 Intel update',
              'trace_sha256':hashlib.sha256(trace_bytes).hexdigest(), 'requested_distinct_inputs':len(records),
              'pilot_per_function':args.pilot_per_function, 'x87_control':hex(oracle.control), 'mxcsr_initial':'0x1f80',
              'summaries':dict(summaries), 'pure_math_import_calls':oracle.import_calls,
              'failures':failures, 'differences':differences,
              'limits':['No exact Workbench-user installed OS/runtime is inferred',
                        'Caller FP-environment propagation, full generator/JVM and audio parity remain unverified',
                        'No canonical output/default math profile/hardware lock changes']}
    encoded = (json.dumps(report, indent=2) + '\n').encode()
    folder = Path(__file__).resolve().parents[1] / 'analysis/reports'
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / ('apple-tiger-math-' + hashlib.sha256(encoded).hexdigest()[:16] + '.json')
    preserve(path, encoded)
    print(path, flush=True)
    if failures:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
