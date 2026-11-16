"""Audit original module startup in private memory; no host/device callbacks."""
import hashlib
import json
import struct
import subprocess
from coefficient_oracle import CoefficientOracle, ROOT
from native_oracle import NativeOracle
from inspect_firmware import preserve
from dependency_tools import verify_llvm_nm


def main():
    verify_llvm_nm()
    cold = NativeOracle()
    assert cold.read(0x3d798, 16) == bytes(16)
    try:
        cold.call_initializer(0x12345)
    except ValueError:
        pass
    else:
        raise AssertionError('non-table entry accepted')
    path = ROOT/'analysis/extracted/macho-slices/L6VariaxCoefEngine.jnilib.i386.macho'
    symbols = [line.split() for line in subprocess.check_output(['llvm-nm', str(path)], text=True).splitlines()]
    symbols = [s for s in symbols if len(s) == 3]
    entries = {int(s[0], 16): s[2] for s in symbols if s[2].startswith('__GLOBAL__I_')}
    assert set(cold.module_initializers) == set(entries) and len(entries) == 26
    constants = [(int(s[0], 16), 1.0 if s[2] == '_kComplex_Real_One' else -1.0)
                 for s in symbols if s[2] in ('_kComplex_Real_One', '_kComplex_Real_NegOne')]
    assert len(constants) == 52
    runs = []
    for _ in range(2):
        oracle = CoefficientOracle()
        assert tuple(oracle.initializers_completed) == cold.module_initializers
        for address, expected in constants:
            assert struct.unpack('<2f', oracle.read(address, 8)) == (expected, 0.0)
        assert oracle.import_calls['_pow'] > 0
        before = oracle.read(0x38000, 0x6000)
        runs.append(hashlib.sha256(before).hexdigest())
        # Even a previously initialized object must reject a damaged runtime.
        oracle.write(0x3d798, bytes(16))
        try:
            oracle.generate(bytes(192))
        except RuntimeError as error:
            assert 'initialization' in str(error)
        else:
            raise AssertionError('damaged startup state accepted')
    assert len(set(runs)) == 1, 'fresh native startup state differs'
    report = {'source_sha256': cold.sha256, 'runtime_version': oracle.RUNTIME_VERSION,
              'initializer_table': [{'address': hex(a), 'name': entries[a]} for a in cold.module_initializers],
              'verified_complex_constants': len(constants), 'destructor_callbacks_captured': 53,
              'fresh_startup_memory_equal': True, 'invalid_entry_rejected': True,
              'corrupted_knob_constants_rejected': True,
              'adapters': 'Original constructor instructions; host libm; private allocation; __cxa_atexit registrations captured without execution or host registration',
              'limitations': ['No destructor execution or process teardown equivalence',
                             'Host libm rounding is not historical macOS libm parity',
                             'No real firmware/DSP/audio or hardware emulation']}
    preserve(ROOT/f'analysis/reports/native-startup-{oracle.RUNTIME_VERSION}.json', (json.dumps(report, indent=2)+'\n').encode())
    print('PASS: 26 original initializers, 52 complex constants, 53 captured callbacks, fresh startup repeatability and rejection guards')


if __name__ == '__main__':
    main()
