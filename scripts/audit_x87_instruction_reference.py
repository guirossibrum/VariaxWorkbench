"""Compare the Apple guest trace with owned native x87 instructions, file-only."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import struct

from inspect_firmware import preserve
from math_backends import bits
from native_x87_reference import NativeX87


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('trace', type=Path)
    parser.add_argument('apple_report', type=Path)
    args = parser.parse_args()
    trace_bytes, report_bytes = args.trace.read_bytes(), args.apple_report.read_bytes()
    if hashlib.sha256(trace_bytes).hexdigest() != '13dcfbf977114421578f4430f411eb64167a513af6ed774105b3ddb5f0a882ec':
        raise ValueError('requires pinned canonical input trace')
    apple = json.loads(report_bytes)
    if (apple['status'] != 'pass' or apple['requested_distinct_inputs'] != 206228
            or apple['source_sha256'] != '36f2a3fbaa04d2ddc7a7ad600c083c637ff28fcae5aa86252509742b16325358'
            or apple['trace_sha256'] != hashlib.sha256(trace_bytes).hexdigest()
            or apple['pilot_per_function'] != 0 or apple['failures']
            or sum(record['distinct_inputs'] for record in apple['summaries'].values()) != 206228):
        raise ValueError('requires complete Apple guest-math report')
    changes = {(record['function'],tuple(record['input_bits'])):record['apple_tiger_bits'] for record in apple['differences']}
    native = NativeX87()
    summaries, differences = collections.defaultdict(collections.Counter), []
    for record in json.loads(trace_bytes):
        name = record['function']
        if name not in ('sinf','cosf','tanf','tan'):
            continue
        single = name.endswith('f')
        value = struct.unpack('<f' if single else '<d',bytes.fromhex(record['input_bits'][0]))[0]
        result = bits(native.evaluate(name,value),single)
        expected = changes.get((name,tuple(record['input_bits'])),record['result_bits'])
        summaries[name]['distinct_inputs'] += 1
        if result != expected:
            summaries[name]['native_x87_vs_apple_guest_bit_differences'] += 1
            differences.append({**record,'apple_guest_bits':expected,'native_instruction_bits':result})
    report = {'scope':'owned native x87 arithmetic, no vendor code/libm/device; one current CPU, not every historical Intel CPU',
              'trace_sha256':hashlib.sha256(trace_bytes).hexdigest(),'apple_math_report_sha256':hashlib.sha256(report_bytes).hexdigest(),
              'helper':native.record,'control':'0x37f','summaries':dict(summaries),'differences':differences}
    encoded=(json.dumps(report,indent=2)+'\n').encode()
    folder=Path(__file__).resolve().parents[1]/'analysis/reports'
    folder.mkdir(parents=True,exist_ok=True)
    path=folder/('x87-instruction-comparison-'+hashlib.sha256(encoded).hexdigest()[:16]+'.json')
    preserve(path,encoded)
    print(path)


if __name__ == '__main__':
    main()
