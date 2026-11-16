"""Source/profile/CPU-boundary checks for optional historical math research."""
import argparse
import json
import math
from pathlib import Path
import struct
import tempfile

from apple_tiger_math_oracle import AppleTigerMath, AppleTigerCoefficientOracle
from coefficient_oracle import CoefficientOracle, default_settings
from inspect_firmware import preserve


def expect_error(call, kind, phrase):
    try:
        call()
    except kind as error:
        if phrase not in str(error):
            raise
    else:
        raise AssertionError('required rejection did not occur: ' + phrase)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path)
    args = parser.parse_args()
    checks = []
    with tempfile.TemporaryDirectory() as name:
        bad = Path(name) / 'wrong.macho'
        data = bytearray(args.library.read_bytes())
        data[-1] ^= 1
        bad.write_bytes(data)
        expect_error(lambda: AppleTigerMath(bad), ValueError, 'pinned original')
        checks.append('same-size corrupted source is rejected by SHA-256')
    expect_error(lambda: AppleTigerMath(args.library, control=0), ValueError, 'control')
    backend = AppleTigerMath(args.library)
    expect_error(lambda: backend.set_environment(0x37f, 0x9f80), ValueError, 'floating-point')
    expect_error(lambda: backend.evaluate('exp'), ValueError, 'argument')
    checks.append('unreviewed controls/FTZ and incorrect argument count are rejected')
    for name, values, expected in [('sin',(0.,),0.),('sinf',(0.,),0.),('cos',(0.,),1.),('cosf',(0.,),1.),
                                    ('tan',(0.,),0.),('tanf',(0.,),0.),('exp',(0.,),1.),('expf',(0.,),1.),
                                    ('log',(1.,),0.),('log10f',(1.,),0.),('pow',(2.,3.),8.),
                                    ('powf',(2.,3.),8.),('atan2f',(0.,1.),0.)]:
        if backend.evaluate(name,*values) != expected:
            raise AssertionError('exact basic math identity changed: ' + name)
        # Repeat to exercise a completed, environment-keyed cache entry.
        if backend.evaluate(name,*values) != expected:
            raise AssertionError('cached math identity changed: ' + name)
    if backend.cache_hits != 13:
        raise AssertionError('cache did not preserve typed call separation')
    if backend.pow(10.,-3.) != 0.0010000000000000002 or backend.pow(10.,-2.) != 0.010000000000000002:
        raise AssertionError('source-pinned independent pow values changed')
    checks.append('13 exact basic identities and repeated typed results; dated pow constants independently match')
    expect_error(lambda: CoefficientOracle(math_backend=backend,runtime_version=CoefficientOracle.RUNTIME_VERSION), ValueError, 'distinct reference namespace')
    checks.append('historical backend cannot reuse the canonical namespace')
    # Default and source-specific profiles reject each other's constants and
    # zero state. No profile accepts whatever happens to be observed at startup.
    ordinary = CoefficientOracle()
    original = ordinary.read(0x3d798,16)
    for state in [bytes(16),struct.pack('<2d',*AppleTigerCoefficientOracle.EXPECTED_KNOB_CONSTANTS)]:
        ordinary.write(0x3d798,state)
        expect_error(lambda: ordinary.generate(default_settings()),RuntimeError,'initialization')
    ordinary.write(0x3d798,original)
    historical = AppleTigerCoefficientOracle(args.library)
    if len(historical.initializers_completed) != 26 or len(historical.destructor_registrations) != 53:
        raise AssertionError('historical constructor coverage changed')
    for state in [bytes(16),struct.pack('<2d',*CoefficientOracle.EXPECTED_KNOB_CONSTANTS)]:
        historical.write(0x3d798,state)
        expect_error(lambda: historical.generate(default_settings()),RuntimeError,'initialization')
    checks.append('both exact startup contracts reject zero and the other profile constants')
    # Fresh instances for each stopped CPU; nothing resumes an unmodeled path.
    stopped = AppleTigerMath(args.library)
    forbidden = next(at for at,name in stopped.stubs.items() if name == '___error')
    expect_error(lambda: stopped.cpu.emu_start(forbidden,stopped.RETURN,count=1000,timeout=100000),RuntimeError,'import is unsupported')
    checks.append('actual unapproved import entry is intercepted before execution')
    stopped = AppleTigerMath(args.library)
    stopped.cpu.mem_write(stopped.AFTER,b'\xcd\x80')
    expect_error(lambda: stopped.cpu.emu_start(stopped.AFTER,stopped.RETURN,count=1000,timeout=100000),RuntimeError,'interrupt')
    checks.append('synthetic guest OS interrupt is rejected without forwarding')
    stopped = AppleTigerMath(args.library)
    stopped.cpu.mem_write(stopped.symbols['_exp'],b'\xeb\xfe')
    expect_error(lambda: stopped.evaluate('exp',0.123456789),RuntimeError,'did not return')
    checks.append('synthetic non-returning guest stops at the unchanged call bound')
    report = {'status':'pass','scope':'optional original-runtime CPU research, no native vendor loading or device access',
              'source_sha256':backend.SHA256,'checks':checks,
              'limits':['Unicorn x87 transcendental operations use host math; instruction checks remain separate',
                        'No default profile, canonical output, teardown, audio or hardware equivalence claim']}
    encoded=(json.dumps(report,indent=2)+'\n').encode()
    import hashlib
    folder=Path(__file__).resolve().parents[1]/'analysis/reports'
    folder.mkdir(parents=True,exist_ok=True)
    path=folder/('apple-tiger-guards-'+hashlib.sha256(encoded).hexdigest()[:16]+'.json')
    preserve(path,encoded)
    print(path)
    print('PASS:',len(checks),'source/profile/CPU-boundary checks')


if __name__ == '__main__':
    main()
