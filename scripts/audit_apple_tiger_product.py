"""Fresh product CLI comparison with both saved historical Apple candidates.

Optional private-resource research only. Builds owned Rust code offline; never
loads vendor libraries natively, accesses a device or changes the default math.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tomllib

from audit_engine_math import batch, changed_words
from build_rust_math_reference import ARCHIVE_SHA256, verify_cached_source
from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve
from test_coefficient_generator import cases


def digest(data):
    return hashlib.sha256(data).hexdigest()


def reference(path, runtime, corpus):
    if not path.is_file() or path.stat().st_size > 4 * 1024 * 1024:
        raise ValueError('candidate report must be a bounded regular file')
    data = path.read_bytes()
    report = json.loads(data)
    if (report['status'] != 'pass' or report['failure'] is not None
            or report['runtime_version'] != runtime
            or report['math_source_sha256'] != '36f2a3fbaa04d2ddc7a7ad600c083c637ff28fcae5aa86252509742b16325358'
            or report['coefficient_source_sha256'] != '3aa232329d467101637764397b77a7b8ffa6af0ccd72ec380d02f145477ba9fe'
            or report['initializers'] != 26 or report['captured_destructor_callbacks'] != 53
            or len(report['records']) != len(corpus)):
        raise ValueError('candidate source/profile/completion contract mismatch')
    outputs = []
    for i, ((name, settings), record) in enumerate(zip(corpus, report['records'])):
        input_sha = digest(settings)
        if record['case'] != name or record['input_sha256'] != input_sha:
            raise ValueError('candidate input order or corpus changed')
        output = (ROOT / 'analysis/extracted' / runtime / f'{i:03}-{input_sha}.packed').read_bytes()
        if len(output) != 3276 or digest(output) != record['output_sha256']:
            raise ValueError('saved candidate bytes changed')
        outputs.append(output)
    return outputs, digest(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('apple_guest_report', type=Path)
    parser.add_argument('apple_current_x87_report', type=Path)
    args = parser.parse_args()
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        raise ValueError('this product audit is scoped to Linux x86-64')
    oracle = CoefficientOracle()
    models = oracle.load_models()
    corpus = [(name, bytes(settings)) for name, settings in cases(models)]
    if len(corpus) != 110:
        raise ValueError('requires the established complete-patch corpus')
    guest, guest_sha = reference(args.apple_guest_report, 'initialized-apple-tiger8839-v1', corpus)
    native, native_sha = reference(args.apple_current_x87_report, 'initialized-apple-tiger8839-current-host-x87-v1', corpus)
    if guest != native:
        raise ValueError('historical candidates no longer agree; inspect separately')
    canonical_bytes = (ROOT / 'analysis/reports/coefficient-generator-tests-initialized-host-libm-v2-budget200m.json').read_bytes()
    if digest(canonical_bytes) != '8289f2af4fb61d8dbc700d02d8d8326668d1105850686a17883be07ca280e219':
        raise ValueError('recorded glibc reference fingerprint changed')
    canonical = {(r['case'], r['input_sha256']): r['output_sha256'] for r in json.loads(canonical_bytes)['records']}
    dependency, files = verify_cached_source()
    package = next(p for p in tomllib.loads((ROOT / 'Cargo.lock').read_text())['package'] if p['name'] == 'libm')
    if package['version'] != '0.2.16' or package['checksum'] != ARCHIVE_SHA256:
        raise ValueError('unexpected portable math dependency')
    names = ['Cargo.toml', 'Cargo.lock'] + [str(p.relative_to(ROOT)) for p in sorted((ROOT / 'src').rglob('*.rs'))]
    hashes = {name: digest((ROOT / name).read_bytes()) for name in names}
    compiler = subprocess.check_output(['rustc', '-vV'], text=True)
    source_id = digest(json.dumps({'files': hashes, 'compiler': compiler}, sort_keys=True).encode())
    directory = ROOT / '.tools/apple-product-audit' / source_id[:16]
    directory.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.pop('RUSTFLAGS', None)
    env.pop('CARGO_ENCODED_RUSTFLAGS', None)
    env['CARGO_TARGET_DIR'] = str(directory / 'target')
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--offline', '--locked', '--features', 'portable-math', '--filter-platform', 'x86_64-unknown-linux-gnu', '--format-version', '1'], cwd=ROOT, env=env, text=True))
    resolved = next(p for p in metadata['packages'] if p['name'] == 'libm')
    if Path(resolved['manifest_path']).resolve() != (dependency / 'Cargo.toml').resolve():
        raise ValueError('Cargo resolved unchecked math source')
    records, commands, binaries = {}, [], {}
    for software in (False, True):
        for release in (False, True):
            label = ('portable' if software else 'host') + ('-release' if release else '-debug')
            command = ['cargo', 'build', '--offline', '--locked', '--bin', 'variax-workbench']
            if software:
                command += ['--features', 'portable-math']
            if release:
                command += ['--release']
            subprocess.run(command, cwd=ROOT, env=env, check=True)
            commands.append(command)
            built = directory / 'target' / ('release' if release else 'debug') / 'variax-workbench'
            binary = directory / label
            preserve(binary, built.read_bytes())
            binary.chmod(0o755)
            expected = 'software-wide-v1/libm-0.2.16' if software else 'host-std-v1'
            if subprocess.check_output([str(binary), 'math-profile'], text=True).strip() != expected:
                raise ValueError('wrong product math profile')
            binaries[label] = digest(binary.read_bytes())
            outputs = batch(binary, [settings for _, settings in corpus], models)
            rows = []
            for (name, settings), actual, apple in zip(corpus, outputs, guest):
                output_sha = digest(actual)
                rows.append({'case': name, 'input_sha256': digest(settings), 'output_sha256': output_sha,
                             'changed_words_vs_both_apple_candidates': changed_words(actual, apple),
                             'matches_recorded_glibc': output_sha == canonical[(name, digest(settings))]})
            records[label] = rows
            print(label, 'Apple matches', sum(not r['changed_words_vs_both_apple_candidates'] for r in rows), '/ 110', flush=True)
    if hashes != {name: digest((ROOT / name).read_bytes()) for name in names}:
        raise ValueError('product source changed during audit')
    report = {'scope': 'fresh Linux x86-64 Rust product binaries against two saved source-pinned Apple generator candidates; no audio/physical/full-JVM equivalence',
              'source_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'source_fingerprint': source_id, 'product_sources': hashes, 'compiler': compiler, 'commands': commands,
              'audit_sha256': digest(Path(__file__).read_bytes()), 'math_archive_sha256': ARCHIVE_SHA256,
              'math_source_files_verified': files, 'apple_guest_report_sha256': guest_sha,
              'apple_current_x87_report_sha256': native_sha, 'canonical_glibc_report_sha256': digest(canonical_bytes),
              'binaries': binaries, 'records': records,
              'limits': ['Saved Apple candidates retain documented x87/ABI/fenv limitations',
                         'Existing 110 cases, not new sample coverage or late-stage issue33 completion',
                         'Default math, canonical references, production dependencies and hardware locks unchanged']}
    encoded = (json.dumps(report, indent=2) + '\n').encode()
    path = directory / ('report-' + digest(encoded)[:16] + '.json')
    preserve(path, encoded)
    print(path, flush=True)
    for label, rows in records.items():
        if label.startswith('portable') and any(r['changed_words_vs_both_apple_candidates'] for r in rows):
            raise RuntimeError('portable product differs from saved Apple candidates; inspect report')
        if label.startswith('host') and not all(r['matches_recorded_glibc'] for r in rows):
            raise RuntimeError('host-profile control changed; inspect report')
    for profile in ('host', 'portable'):
        if [r['output_sha256'] for r in records[profile + '-debug']] != [r['output_sha256'] for r in records[profile + '-release']]:
            raise RuntimeError('debug/release product outputs differ')


if __name__ == '__main__':
    main()
