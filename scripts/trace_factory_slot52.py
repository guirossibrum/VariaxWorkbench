"""Trace the slot 52 secondary pickup Q table using private offline inputs."""

import argparse
import json
import math
from pathlib import Path
import struct
import subprocess
import tempfile

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_ECX, UC_X86_REG_EBP, UC_X86_REG_ESP, UC_X86_REG_FPCW,
    UC_X86_REG_FP0, UC_X86_REG_FPSW, UC_X86_REG_XMM0, UC_X86_REG_XMM3, UC_X86_REG_XMM7,
)

from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve
from verify_factory_programs import (
    ARCHIVE_SHA256, checked_program, digest, firmware_program, load_models,
    original_programs, word_at,
)
from verify_factory_response import vx
from verify_windows_factory_programs import PRECISION
from windows_coefficient_oracle import WindowsCoefficientOracle

SLOT = 52
# Entry/return sites in the fingerprinted PE32 complex arithmetic routines.
PE_MULTIPLY = (0x10010620, 0x10010665)
PE_DIVIDE = (0x10010670, 0x100106bc)
PE_RECIPROCAL = (0x100106c0, 0x100106e9)


def values(oracle, address, kind, count):
    return struct.unpack(f'<{count}{kind}', oracle.read(address, struct.calcsize(kind) * count))


def hook(oracle, address, action):
    oracle.cpu.hook_add(UC_HOOK_CODE, lambda *_: action(), begin=address, end=address)


def native_trace(oracle, windows, replacement=None, biquad_replacement=(), narrow=()):
    trace = {'fits': []}
    reg = oracle.cpu.reg_read

    def read(address, kind, count):
        return values(oracle, address, kind, count)

    def enter_fit():
        stack = reg(UC_X86_REG_ESP)
        if windows:
            circuit, wiring, output, primary = read(stack + 4, 'I', 4)
            controls = read(stack + 20, 'd', 3)
        else:
            _, circuit, wiring, output, primary = read(stack + 4, 'I', 5)
            controls = read(stack + 24, 'd', 3)
        trace['fits'].append({
            'controls': controls, 'primary': read(primary, 'd', 3),
            'low_trials': [], 'low_biquad': [], 'low_stages': [],
        })
        trace['circuit'] = read(circuit, 'd', 4)
        trace['wiring'] = read(wiring, 'd', 5)
        trace['fit_output'] = output

    def leave_fit():
        row = trace['fits'][-1]
        output = trace.pop('fit_output')
        row['result'] = read(output, 'd', 3)
        if replacement is not None and len(trace['fits']) == 1:
            assert abs(row['controls'][1] - replacement['controls'][1]) < 1e-12
            oracle.write(output + 8, struct.pack('<d', replacement['result'][1]))

    def table():
        address = reg(UC_X86_REG_ESP) + 0x18 if windows else reg(UC_X86_REG_EBP) - 0x260
        trace['table_q'] = read(address + 0x168, 'd', 9)
        trace['table_gain'] = read(address + 0x1b0, 'd', 9)

    def final():
        address = read(reg(UC_X86_REG_ESP) + (4 if windows else 8), 'I', 1)[0]
        trace['f32_q'] = read(address, 'f', 9)

    hook(oracle, 0x1000e310 if windows else 0x252b0, enter_fit)
    hook(oracle, 0x1000ea41 if windows else 0x261a4, leave_fit)
    hook(oracle, 0x1000eba7 if windows else 0x2631c, table)
    hook(oracle, 0x10008f70 if windows else 0x1c990, final)

    def x87(index):
        # Unicorn exposes the physical FP registers as (significand, sign/exponent).
        top = (reg(UC_X86_REG_FPSW) >> 11) & 7
        significand, sign_exponent = reg(UC_X86_REG_FP0 + ((top + index) & 7))
        result = math.ldexp(significand, (sign_exponent & 0x7fff) - 16383 - 63)
        return -result if sign_exponent & 0x8000 else result

    def xmm(register):
        return struct.unpack('<d', (reg(register) & ((1 << 64) - 1)).to_bytes(8, 'little'))[0]

    def low_trial():
        if windows:
            midpoint, = read(reg(UC_X86_REG_ESP) + 0x18, 'd', 1)
            residuals = tuple(x87(i) for i in range(3))
        else:
            midpoint, = read(reg(UC_X86_REG_EBP) - 0x80, 'd', 1)
            residuals = (xmm(UC_X86_REG_XMM0), xmm(UC_X86_REG_XMM7), xmm(UC_X86_REG_XMM3))
        if midpoint < 25:
            trace['fits'][-1]['low_trials'].append((midpoint, *residuals))

    def biquad_enter():
        if 'fit_output' not in trace:
            return
        stack = reg(UC_X86_REG_ESP)
        if windows:
            _, output, coefs = read(stack, 'I', 3)
            frequency, = read(stack + 12, 'd', 1)
        else:
            _, output, _, coefs = read(stack, 'I', 4)
            frequency, = read(stack + 16, 'd', 1)
        if abs(frequency) < 35:
            trace['biquad_call'] = (output, frequency)
            trace['fits'][-1]['biquad_coefs'] = read(coefs, 'd', 5)
            trace['fits'][-1]['low_stages'].append({'frequency': frequency})

    def biquad_leave():
        if 'biquad_call' not in trace:
            return
        output, frequency = trace.pop('biquad_call')
        original = read(output, 'd', 2)
        trace['fits'][-1]['low_biquad'].append((frequency, *original))
        for sample, real, imaginary, _ in biquad_replacement:
            if len(trace['fits']) == 1 and abs(sample - frequency) < 1e-11:
                trace.setdefault('biquad_substitutions', []).append(frequency)
                oracle.write(output, struct.pack('<2d', real, imaginary))
                break

    hook(oracle, 0x1000e5e0 if windows else 0x2570a, low_trial)
    hook(oracle, 0x1000d5e0 if windows else 0x23d30, biquad_enter)
    # Caller instructions immediately following before/center/after and upper-sweep calls.
    for address in ((0x1000e40f, 0x1000e49a, 0x1000e52d, 0x1000e727)
                    if windows else (0x253b1, 0x254ca, 0x255de, 0x2584c)):
        hook(oracle, address, biquad_leave)

    def stage(name, pointer):
        if 'biquad_call' in trace:
            trace['fits'][-1]['low_stages'][-1][name] = read(pointer(), 'f', 2)

    if windows:
        for name, address, offset in (
            ('polar', 0x1000d665, 0x1c), ('z', 0x1000d66e, 0x1c),
            ('z2', 0x1000d686, 0x14),
        ):
            hook(oracle, address, lambda name=name, offset=offset:
                 stage(name, lambda: reg(UC_X86_REG_ESP) + offset))
        hook(oracle, 0x1000cdd3, lambda: stage(
            'denominator', lambda: read(reg(UC_X86_REG_ESP), 'I', 1)[0]))
        hook(oracle, 0x1000cdd3, lambda: stage('numerator', lambda: reg(UC_X86_REG_ECX)))
    else:
        for name, address, offset in (
            ('polar', 0x23d9e, -0x20), ('z', 0x23da9, -0x20),
            ('z2', 0x23dca, -0x28), ('denominator', 0x23f16, -0x38),
            ('numerator', 0x23f16, -0x30),
        ):
            hook(oracle, address, lambda name=name, offset=offset:
                 stage(name, lambda: reg(UC_X86_REG_EBP) + offset))

    saved_control = None

    def narrow_enter():
        nonlocal saved_control
        if 'biquad_call' in trace and len(trace['fits']) == 1:
            saved_control = reg(UC_X86_REG_FPCW)
            oracle.cpu.reg_write(UC_X86_REG_FPCW, saved_control & ~0x300)

    def narrow_leave():
        nonlocal saved_control
        if saved_control is not None:
            oracle.cpu.reg_write(UC_X86_REG_FPCW, saved_control)
            saved_control = None

    for entry, leave in narrow:
        hook(oracle, entry, narrow_enter)
        hook(oracle, leave, narrow_leave)
    return trace


def packing(program):
    words = [word_at(program, i) for i in range(138)]
    return [{'exponent_word': i, 'mantissa_word': i + 1,
             'exponent': words[i], 'decoded_q': vx(words, i)}
            for i in range(120, 138, 2)]


def run(args, directory):
    if digest(args.archive.read_bytes()) != ARCHIVE_SHA256:
        raise ValueError('factory archive fingerprint mismatch')
    baseline = json.loads((ROOT / 'docs/factory-program-parity.json').read_text())
    windows = json.loads((ROOT / 'docs/factory-program-windows-precision.json').read_text())
    reference = directory / 'reference'
    subprocess.run([args.extractor, '--variax-reference', reference, args.archive],
                   stdout=subprocess.DEVNULL, check=True)
    presets = directory / 'presets'
    presets.mkdir()
    preset = presets / '0.vwm'
    preserve(preset, (reference / f'slot-{SLOT:02}.vwm').read_bytes())
    expected = baseline['slots'][SLOT]
    assert digest(preset.read_bytes()) == expected['preset_sha256']
    firmware = checked_program(firmware_program(reference / f'slot-{SLOT:02}.u24le'))
    assert digest(firmware) == expected['program_sha256']['firmware']

    mac = CoefficientOracle(artifact_root=args.artifact_root)
    mac_dir = directory / 'mac'
    mac_dir.mkdir()
    initial, manifest, models = load_models(mac, mac_dir)
    mac.write(mac.OBJECT, initial)
    assert (mac.sha256, mac.RUNTIME_VERSION, models) == (
        baseline['native_sha256'], baseline['native_runtime'], baseline['model_hash_map_sha256'])
    mac_trace = native_trace(mac, False)
    mac_programs, classes = original_programs(
        args.artifact_root, mac_dir, presets, lambda settings, _: mac.generate(settings), count=1)
    mac_program, = mac_programs
    assert classes == baseline['java_class_sha256']
    assert digest(mac_program) == expected['program_sha256']['original']

    assert subprocess.check_output([args.host_std, 'math-profile'], text=True).strip() == 'host-std-v1'
    rust_path = directory / 'rust.packed'
    subprocess.run([args.host_std, 'render-preset', preset, manifest, rust_path],
                   stdout=subprocess.DEVNULL, check=True)
    rust = checked_program(rust_path.read_bytes())
    assert rust == mac_program

    mac_fit = mac_trace['fits'][0]  # First call corresponds to tone point 4.

    def windows_run(width, replacement=None, biquad_replacement=(), narrow=()):
        win = WindowsCoefficientOracle(PRECISION[width], 'initialized-sse2', artifact_root=args.artifact_root)
        records = win.load_models()
        models = digest(json.dumps({
            f'{row["kind"]}:{row["id"] & 0xffffffff:08x}': row['sha256'] for row in records
        }, sort_keys=True).encode())
        assert (win.SHA256, models) == (
            windows['windows_pe32_sha256'], windows['windows_model_hash_map_sha256'])
        trace = native_trace(win, True, replacement, biquad_replacement, narrow)
        scratch = Path(tempfile.mkdtemp(prefix=f'windows-{width}-', dir=directory))
        programs, win_classes = original_programs(
            args.artifact_root, scratch, presets, win.generate, count=1)
        program, = programs
        assert win_classes == classes
        if replacement is None and not biquad_replacement and not narrow:
            assert digest(program) == windows['slots'][SLOT][f'windows_{width}']['windows_program_sha256']
        return trace, program

    win53, program53 = windows_run(53)
    win64, program64 = windows_run(64)
    samples = sorted({frequency for trace in (mac_trace, win53)
                      for frequency, *_ in trace['fits'][0]['low_biquad']})
    inputs = (*mac_trace['circuit'], *mac_trace['wiring'],
              *mac_fit['primary'], *mac_fit['controls'], *samples)
    replay = subprocess.check_output([args.rust_probe], text=True,
                                     input=' '.join(map(str, inputs))).splitlines()
    rust_fit = tuple(map(float, replay[0].split()))
    rust_table, rust_f32 = [], []
    for index, line in enumerate(replay[1:10]):
        raw, narrowed, exponent, mantissa = line.split()
        rust_table.append(float(raw))
        rust_f32.append(float(narrowed))
        assert (int(exponent), int(mantissa)) == (
            word_at(rust, 120 + 2 * index), word_at(rust, 121 + 2 * index))
    assert rust_fit == mac_fit['result']
    assert rust_table == list(mac_trace['table_q'])
    assert rust_f32 == list(mac_trace['f32_q'])
    rust_samples = [tuple(map(float, line.split())) for line in replay[10:]]
    rust_biquad = {frequency: (real, imaginary) for frequency, real, imaginary, _ in rust_samples}
    assert all(rust_biquad[frequency] == (real, imaginary)
               for frequency, real, imaginary in mac_fit['low_biquad'])
    substituted, program_sub = windows_run(53, mac_fit)
    assert packing(program_sub) == packing(mac_program)
    # Replace only the biquad output at the left sample of the disputed midpoint.
    left_sample = mac_fit['result'][0] - 10
    replacement = [row for row in rust_samples if row[0] == left_sample]
    assert len(replacement) == 1
    biquad, program_biquad = windows_run(53, biquad_replacement=replacement)
    narrowed, program_narrowed = windows_run(
        53, narrow=(PE_MULTIPLY, PE_DIVIDE, PE_RECIPROCAL))
    reciprocal, program_reciprocal = windows_run(53, narrow=(PE_RECIPROCAL,))
    multiply, program_multiply = windows_run(53, narrow=(PE_MULTIPLY,))
    assert biquad['biquad_substitutions'] == [left_sample]
    assert all(packing(program) == packing(mac_program)
               for program in (program_biquad, program_narrowed, program_multiply))
    assert packing(program_reciprocal) == packing(program53) == packing(program64)
    assert [word for word in range(120, 138)
            if word_at(program53, word) != word_at(firmware, word)] == [129, 133]
    assert all(word_at(firmware, i) - word_at(program53, i) == 1 for i in (129, 133))
    outliers = [i for i in range(121, 138, 2)
                if abs(word_at(mac_program, i) - word_at(firmware, i)) > 100_000]
    assert outliers == list(range(123, 136, 2))
    assert sum(word_at(mac_program, i - 1) != word_at(firmware, i - 1)
               for i in outliers) == 5
    assert all(len(trace['fits']) == 5 for trace in
               (mac_trace, win53, win64, substituted, biquad, narrowed, reciprocal, multiply))
    return {
        'firmware': packing(firmware),
        'rust': {'fit': rust_fit, 'table_q': rust_table, 'f32_q': rust_f32,
                 'low_biquad': rust_samples, 'packed': packing(rust)},
        'mac': {**mac_trace, 'packed': packing(mac_program)},
        'windows_53': {**win53, 'packed': packing(program53)},
        'windows_64': {**win64, 'packed': packing(program64)},
        'windows_53_narrow_reciprocal': {**reciprocal, 'packed': packing(program_reciprocal)},
        'windows_53_narrow_multiply': {**multiply, 'packed': packing(program_multiply)},
        'windows_53_narrow_complex': {**narrowed, 'packed': packing(program_narrowed)},
        'windows_53_rust_low_biquad': {**biquad, 'packed': packing(program_biquad)},
        'windows_53_mac_raw_q': {**substituted, 'packed': packing(program_sub)},
        'mac_vs_firmware_mantissas_over_100000': outliers,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ('artifact-root', 'archive', 'extractor', 'host-std', 'rust-probe', 'report'):
        parser.add_argument(f'--{argument}', type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        parser.error('report path already exists')
    # Java/native intermediates live in a fresh directory alongside the private report.
    with tempfile.TemporaryDirectory(prefix='slot52-', dir=args.report.parent) as scratch:
        report = run(args, Path(scratch))
        preserve(args.report, (json.dumps(report, indent=2, allow_nan=False) + '\n').encode())


if __name__ == '__main__':
    main()
