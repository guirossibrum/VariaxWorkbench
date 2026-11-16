"""Bounded single-case math-factor diagnostic, not a compatibility profile.

Vendor instructions stay in Unicorn; only host system libm is loaded natively
for the declared research control. No device or OS-service forwarding.
"""
import argparse
import ctypes
import ctypes.util
import hashlib
import json
from pathlib import Path

from apple_tiger_math_oracle import AppleTigerMath, AppleTigerCoefficientOracle
from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve
from math_backends import FUNCTIONS
from test_coefficient_generator import cases


class ChoiceMath(AppleTigerMath):
    def __init__(self, path, selected):
        super().__init__(path)
        self.selected = selected
        self.host = ctypes.CDLL(ctypes.util.find_library('m'))
        self.host_methods = {}
        for name, (_, arity, single) in FUNCTIONS.items():
            method = getattr(self.host, name)
            scalar = ctypes.c_float if single else ctypes.c_double
            method.argtypes = [scalar] * arity
            method.restype = scalar
            self.host_methods[name] = method

    def evaluate(self, name, *values):
        if name in self.selected:
            return super().evaluate(name, *values)
        return self.host_methods[name](*values)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path)
    args = parser.parse_args()
    models = json.loads((ROOT / 'analysis/reports/model-catalog.json').read_bytes())
    name, settings = list(cases(models))[102]
    settings = bytes(settings)
    input_sha = hashlib.sha256(settings).hexdigest()
    if name != 'two-pickups/series' or input_sha != '5af4da3506162ec33592d665355722345944ea65f96eba8547966b33ebc582da':
        raise ValueError('case changed')
    expected_host = '2ae6c0bc84a01c442380c99335beb469f4890ee0391f1d891c636dd280917aeb'
    expected_apple = 'fc62774dc83405c8b303c27ad3616fb6ea20e4d0f39794eeb5b10926db19c03e'
    rows = []
    groups = [(), tuple(FUNCTIONS), ('pow',), ('powf',), ('sinf',), ('cosf',), ('sinf', 'cosf'), ('pow', 'powf')]
    for group in groups:
        selected = frozenset(group)

        class Hybrid(AppleTigerCoefficientOracle):
            MATH_BACKEND = staticmethod(lambda path: ChoiceMath(path, selected))
            CANDIDATE_RUNTIME_VERSION = 'private-factor-' + ('-'.join(sorted(selected)) or 'host')
            # Independently fixed source contracts, never observed-state acceptance.
            EXPECTED_KNOB_CONSTANTS = (AppleTigerCoefficientOracle.EXPECTED_KNOB_CONSTANTS
                                       if 'pow' in selected else CoefficientOracle.EXPECTED_KNOB_CONSTANTS)

        oracle = Hybrid(args.library)
        oracle.load_models()
        output_sha = hashlib.sha256(oracle.generate(settings)).hexdigest()
        if not selected and output_sha != expected_host:
            raise ValueError('fresh-state host control differs from saved stateful sweep')
        if selected == frozenset(FUNCTIONS) and output_sha != expected_apple:
            raise ValueError('fresh-state Apple control differs from saved stateful sweep')
        rows.append({'apple_functions': sorted(selected), 'output_sha256': output_sha,
                     'matches_glibc': output_sha == expected_host, 'matches_apple': output_sha == expected_apple})
        print(rows[-1], flush=True)
    report = {
        'scope': 'single-settings math-factor isolation; fresh-state controls match saved stateful case102; diagnostic only',
        'input_sha256': input_sha, 'source_sha256': AppleTigerMath.SHA256,
        'audit_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'records': rows,
        'limits': ['One existing settings case, not wider coverage or audio equivalence',
                   'Hybrid functions can interact; a single-factor result is not universal',
                   'Exact startup contract chosen independently for source-pinned pow versus host pow; defaults unchanged'],
    }
    encoded = (json.dumps(report, indent=2) + '\n').encode()
    folder = ROOT / 'analysis/reports'
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / ('apple-math-factors-' + hashlib.sha256(encoded).hexdigest()[:16] + '.json')
    preserve(path, encoded)
    print(path)


if __name__ == '__main__':
    main()
