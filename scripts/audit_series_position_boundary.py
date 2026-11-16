"""Bounded neighboring-float diagnostic around the known series discrepancy.

Original instructions execute only in private Unicorn memory. This compares
two declared research adapters; it does not select the application's profile.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import struct

def digest(data):
    return hashlib.sha256(data).hexdigest()


def words(data):
    if len(data) != 3276:
        raise ValueError('invalid packed length: expected exactly 3276 bytes')
    result = [int.from_bytes(data[i:i + 3], 'big') for i in range(0, len(data), 3)]
    if len(result) != 1092 or sum(result[:-1]) & 0xffffff != result[-1]:
        raise ValueError('invalid packed length/checksum')
    return result


def main():
    from apple_tiger_math_oracle import AppleTigerCoefficientOracle
    from coefficient_oracle import CoefficientOracle, ROOT
    from inspect_firmware import preserve
    from test_coefficient_generator import cases

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path)
    args = parser.parse_args()
    host = CoefficientOracle()
    models = host.load_models()
    apple = AppleTigerCoefficientOracle(args.library)
    apple.load_models()
    name, center = list(cases(models))[102]
    if name != 'two-pickups/series' or digest(center) != '5af4da3506162ec33592d665355722345944ea65f96eba8547966b33ebc582da':
        raise ValueError('known control input changed')
    center_bits = struct.unpack_from('<I', center, 0x18)[0]
    if center_bits != 0x40000000:
        raise ValueError('expected positive 2.0 position')
    rows = []
    # Center first authenticates both controls before any new setting is used.
    for delta in (0, -2, -1, 1, 2, 0):
        settings = bytearray(center)
        struct.pack_into('<I', settings, 0x18, center_bits + delta)
        outputs = [oracle.generate(settings) for oracle in (host, apple)]
        for label, output in zip(('host', 'apple'), outputs):
            folder = ROOT / 'analysis/extracted/series-position-boundary'
            folder.mkdir(parents=True, exist_ok=True)
            preserve(folder / (digest(settings) + '.input'), bytes(settings))
            preserve(folder / (digest(settings) + '-' + label + '.packed'), output)
        if delta == 0 and [digest(o) for o in outputs] != [
                '2ae6c0bc84a01c442380c99335beb469f4890ee0391f1d891c636dd280917aeb',
                'fc62774dc83405c8b303c27ad3616fb6ea20e4d0f39794eeb5b10926db19c03e']:
            raise ValueError('saved center control not reproduced')
        a, b = map(words, outputs)
        differences = [{'word': i, 'host': x, 'apple': y}
                       for i, (x, y) in enumerate(zip(a, b)) if x != y]
        rows.append({'position_bits': f'{center_bits + delta:08x}',
                     'bit_offset': delta, 'input_sha256': digest(settings),
                     'output_sha256': [digest(o) for o in outputs],
                     'differences': differences})
        print('offset', delta, 'different words', len(differences), flush=True)
    report = {
        'scope': 'five adjacent representable positions, existing series topology; four new synthetic settings',
        'coefficient_source_sha256': host.sha256,
        'apple_source_sha256': apple.libm.SHA256,
        'auditor_sha256': digest(Path(__file__).read_bytes()),
        'adapter_sha256': {name: digest((ROOT / 'scripts' / name).read_bytes())
                           for name in ('coefficient_oracle.py', 'native_oracle.py',
                                        'apple_tiger_math_oracle.py', 'test_coefficient_generator.py')},
        'host': {'platform': platform.platform(), 'libc': platform.libc_ver(),
                 'python': platform.python_version()},
        'runtime_versions': [host.RUNTIME_VERSION, apple.RUNTIME_VERSION],
        'startup_contracts': [list(o.EXPECTED_KNOB_CONSTANTS) for o in (host, apple)],
        'initializers': [len(o.initializers_completed) for o in (host, apple)],
        'captured_destructors': [len(o.destructor_registrations) for o in (host, apple)],
        'records': rows,
        'limits': ['Position-bit neighbors are asymmetric in distance across 2.0.',
                   'No independent math-accuracy verdict or full historical runtime authentication.',
                   'Unicorn x87 transcendentals use host helpers; no exception-state/ABI/JVM/teardown validation.',
                   'Packed differences do not establish audio behavior, hardware safety or rollout authority.',
                   'Local boundary diagnostic only, not the broader issue33 sample gate.'],
    }
    encoded = (json.dumps(report, indent=2) + '\n').encode()
    path = ROOT / 'analysis/reports' / ('series-position-boundary-' + digest(encoded)[:16] + '.json')
    preserve(path, encoded)
    print(path)


if __name__ == '__main__':
    main()
