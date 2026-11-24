"""Compare deterministic complete presets with the initialized original host, offline."""

import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor
import json
import struct
from pathlib import Path
import subprocess
import tempfile

from coefficient_oracle import CoefficientOracle, ROOT
from host_wide_corpus import Case, SAVE_TONE, SEED, TONE, TRIM, cases, coverage, defaults, float_bits
from inspect_firmware import preserve
from verify_factory_programs import PROGRAM_BYTES, compare, digest, load_models
from verify_host_complete import FIELD_LAYOUT, make_preset, run_java


def oracle_shard(artifact_root, mapped, *, fresh):
    def initialized_engine():
        engine = CoefficientOracle(artifact_root=artifact_root)
        engine.load_models()
        return engine, engine.read(engine.OBJECT, 0x1134)

    oracle, initial = initialized_engine()
    results = []
    blocked = False
    for offset, (case, settings) in enumerate(mapped):
        if not fresh and offset and offset % 256 == 0:
            print(f'Original ordered: {offset}/{len(mapped)} attempted', flush=True)
        if blocked:
            results.append((None, 'after-native-failure'))
            continue
        if fresh:
            oracle.write(oracle.OBJECT, initial)
        try:
            results.append((oracle.generate(settings), None))
        except (AssertionError, RuntimeError, ValueError) as error:
            print(f'native {"fresh" if fresh else "batch"} case {case}: {error}', flush=True)
            results.append((None, 'native-generation-failed'))
            if fresh and offset + 1 < len(mapped):
                # A stopped emulation may have changed memory outside the engine object.
                oracle, initial = initialized_engine()
            blocked = not fresh
    return results


def render_rust(args, directory, presets, manifest, count):
    fresh = []
    for index in range(count):
        destination = directory / f'{index}.packed'
        result = subprocess.run([args.binary, 'render-preset', presets / f'{index}.vwm',
                                 manifest, destination], capture_output=True, text=True)
        if result.returncode:
            print(f'rust fresh case {index}: {result.stderr.strip()}', flush=True)
        fresh.append(destination.read_bytes() if not result.returncode else None)
    listing = presets / 'paths.txt'
    preserve(listing, ''.join(f'{index}.vwm\n' for index in range(count)).encode())
    output = directory / 'rust-batch.packed'
    errors = subprocess.check_output([args.batch, listing, manifest, output], text=True)
    rejected = set()
    for line in errors.splitlines():
        print(f'rust batch case {line}', flush=True)
        rejected.add(int(line.split('\t', 1)[0]))
    data = output.read_bytes()
    if len(data) != count * PROGRAM_BYTES:
        raise ValueError('Rust batch length mismatch')
    batch = [None if index in rejected else data[at:at + PROGRAM_BYTES]
             for index, at in enumerate(range(0, len(data), PROGRAM_BYTES))]
    return fresh, batch



def verify_rust_guards(args, directory, manifest):
    """Exercise rejected inputs between two accepted stateful renders."""
    directory.mkdir()
    anchor_first = make_preset(Case('guard', 'Prime', defaults()).line())
    anchor_last = make_preset(Case('guard', 'Read state', defaults(0x03100012)).line())
    good = make_preset(Case('guard', 'Invalid template', defaults() | {
        TRIM: float_bits(4.125), TONE: float_bits(0.375), SAVE_TONE: 1,
    }).line())

    def changed_field(local, original, replacement):
        needle = b'PARM' + struct.pack('>4I', 12, 0x3f000000 | local, 1, float_bits(original))
        if good.count(needle) != 1:
            raise ValueError('ambiguous input probe field')
        return good.replace(needle, b'PARM' + struct.pack('>4I', 12, *replacement))

    probes = {
        'nonfinite': changed_field(0x200000, 4.125, (0x3f200000, 1, 0x7fc00000)),
        'missing': changed_field(0x200000, 4.125, (0x3ffffffe, 1, float_bits(4.125))),
        'out-of-range': changed_field(0x200004, 0.375, (0x3f200004, 1, float_bits(1.25))),
    }
    for name, payload in {'prime': anchor_first, 'check': anchor_last, **probes}.items():
        preserve(directory / f'{name}.vwm', payload)
    for name, payload in probes.items():
        output = directory / f'{name}.packed'
        input_file = directory / f'{name}.vwm'
        result = subprocess.run([args.binary, 'render-preset', input_file, manifest, output],
                                capture_output=True, text=True)
        if not result.returncode or output.exists() or input_file.read_bytes() != payload:
            raise ValueError(f'Rust render guard failed for {name}')
    golden = directory / 'golden.txt'
    invalid = directory / 'invalid.txt'
    preserve(golden, b'prime.vwm\ncheck.vwm\n')
    preserve(invalid, ('prime.vwm\n' + ''.join(f'{name}.vwm\n' for name in probes)
                       + 'check.vwm\n').encode())
    plain = directory / 'golden.packed'
    mixed = directory / 'mixed.packed'
    if subprocess.check_output([args.batch, golden, manifest, plain], text=True):
        raise ValueError('Rust control batch rejected valid inputs')
    errors = subprocess.check_output([args.batch, invalid, manifest, mixed], text=True)
    rejected = [int(row.split('\t', 1)[0]) for row in errors.splitlines()]
    data = plain.read_bytes()
    expected = data[:PROGRAM_BYTES] + bytes(len(probes) * PROGRAM_BYTES) + data[PROGRAM_BYTES:]
    if rejected != list(range(1, len(probes) + 1)) or mixed.read_bytes() != expected:
        raise ValueError('Rust invalid cases changed the succeeding stateful render')
    return list(probes)


def overlay(artifact_root, directory, casefile, layout, presets, mapped, results, label):
    packed = directory / f'{label}.packed'
    preserve(packed, b''.join(output if output is not None else bytes(PROGRAM_BYTES)
                              for output, _error in results))
    final = run_java(directory, casefile, layout, packed, presets, artifact_root=artifact_root)
    if [row[:2] for row in final] != [row[:2] for row in mapped]:
        raise ValueError('original Java changed its mapping between passes')
    return [bytes.fromhex(row[2]) if result[0] is not None else None
            for row, result in zip(final, results, strict=True)]


def slot_result(expected, actual, native_error):
    if actual is None or expected is None:
        return {'status': 'incomplete', 'rust_rejected': actual is None, 'oracle_error': native_error}
    if len(expected) != PROGRAM_BYTES or len(actual) != PROGRAM_BYTES:
        raise ValueError('final program length mismatch')
    differences = compare(expected, actual, frozenset())
    return {
        'status': 'different' if differences else 'exact',
        'rust_sha256': digest(actual), 'oracle_sha256': digest(expected),
        **({'differing_words': differences} if differences else {}),
    }


def verify(args, directory):
    full_corpus = list(cases())
    inventory = coverage(full_corpus)
    end = None if args.limit is None else args.start + args.limit
    corpus = full_corpus[args.start:end]
    if not corpus:
        raise ValueError('case selection is empty')
    count = len(corpus)
    if subprocess.check_output([args.binary, 'math-profile'], text=True).strip() != 'host-std-v1':
        raise ValueError('canonical reference comparison requires host-std-v1')
    print(f'Comparing {count}/{len(full_corpus)} complete presets', flush=True)
    casefile = directory / 'cases.txt'
    preserve(casefile, ''.join(case.line() + '\n' for case in corpus).encode())
    presets = directory / 'presets'
    presets.mkdir()
    for index, case in enumerate(corpus):
        preserve(presets / f'{index}.vwm', make_preset(case.line()))
    layout = directory / 'layout.txt'
    preserve(layout, ''.join(f'{name} {offset} {kind}\n'
                             for name, (offset, kind) in FIELD_LAYOUT.items()).encode())
    app = args.artifact_root / 'analysis/extracted/windows-app'
    classes = ROOT / 'scripts'
    jdk = args.artifact_root / '.tools/jdk/bin'
    subprocess.run([jdk / 'javac', '-cp', app, '-d', directory,
                    classes / 'java-host-stubs/cmn/l6java/utils/L6StaticUtils.java',
                    classes / 'VerifyHostPipeline.java'], check=True)
    mapped = run_java(directory, casefile, layout, '-', presets, artifact_root=args.artifact_root)
    if len(mapped) != count or any(mask != 'ffffffff' for _, mask, _ in mapped):
        raise ValueError('unexpected original mapping count or block mask')
    settings = [(index, bytes.fromhex(row[0])) for index, row in enumerate(mapped)]
    oracle = CoefficientOracle(artifact_root=args.artifact_root)
    _, manifest, model_hash = load_models(oracle, directory)
    guards = verify_rust_guards(args, directory / 'guards', manifest)

    with ProcessPoolExecutor(max_workers=args.jobs + 1) as native:
        ordered = native.submit(oracle_shard, args.artifact_root, settings, fresh=False)
        shard_size = (count + args.jobs - 1) // args.jobs
        isolated = [
            native.submit(oracle_shard, args.artifact_root, settings[at:at + shard_size], fresh=True)
            for at in range(0, count, shard_size)
        ]
        with ThreadPoolExecutor(max_workers=1) as rust:
            rendered = rust.submit(render_rust, args, directory, presets, manifest, count)
            fresh = []
            for shard in isolated:
                fresh.extend(shard.result())
                print(f'Original fresh: {len(fresh)}/{count}', flush=True)
            batch = ordered.result()
            print(f'Original ordered: {sum(data is not None for data, _ in batch)}/{count}', flush=True)
            rust_fresh, rust_batch = rendered.result()

    results = {}
    for label, native_results, rust_results in (('fresh', fresh, rust_fresh), ('batch', batch, rust_batch)):
        original = overlay(args.artifact_root, directory, casefile, layout, presets, mapped,
                           native_results, label)
        results[label] = [slot_result(expected, actual, native_result[1])
                          for expected, actual, native_result in zip(original, rust_results, native_results, strict=True)]
    return {
        'rust_guard_cases': guards,
        'seed': SEED, 'start': args.start, 'full_corpus_size': len(full_corpus), 'full_corpus_coverage': inventory,
        'corpus_sha256': digest(casefile.read_bytes()), 'oracle_sha256': oracle.sha256,
        'oracle_runtime': oracle.RUNTIME_VERSION, 'resources_sha256': model_hash,
        'rust_binary_sha256': digest(args.binary.read_bytes()),
        'rust_batch_sha256': digest(args.batch.read_bytes()),
        'totals': {label: dict(Counter(row['status'] for row in rows)) for label, rows in results.items()},
        'cases': [{'case': args.start + index, 'group': case.group,
                   **{label: rows[index] for label, rows in results.items()}}
                  for index, case in enumerate(corpus)],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('artifact-root', 'binary', 'batch', 'report'):
        parser.add_argument(f'--{flag}', type=lambda path: Path(path).resolve(), required=True)
    parser.add_argument('--jobs', type=int, default=4, help='independent fresh-engine workers')
    parser.add_argument('--start', type=int, default=0, help='zero-based first case')
    parser.add_argument('--limit', type=int, help='maximum cases for a focused replay')
    args = parser.parse_args()
    if args.jobs < 1 or args.start < 0 or (args.limit is not None and args.limit < 1) or args.report.exists():
        parser.error('need valid case selection, positive worker count and a new report path')
    with tempfile.TemporaryDirectory(prefix='variax-host-wide-') as temporary:
        report = verify(args, Path(temporary))
    preserve(args.report, (json.dumps(report, separators=(',', ':')) + '\n').encode())
    print(json.dumps(report['totals'], indent=2))
    if any(counts.get('exact', 0) != len(report['cases']) for counts in report['totals'].values()):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
