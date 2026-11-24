"""Compare the 60 archived 3.10 programs with offline Rust and Java/native renders."""

import argparse
import hashlib
import json
import struct
import subprocess
import tempfile
from collections import Counter, defaultdict
from pathlib import Path

from coefficient_oracle import CoefficientOracle, ROOT, default_settings
from inspect_firmware import preserve
from verify_host_complete import run_java
from verify_host_pipeline import FIELD_LAYOUT


SLOTS = 60
PROGRAM_BYTES = 3276
CHECKSUM_WORD = 1091
# HostParameters::finish writes the name and settings across this byte span.
METADATA_START = 2553
METADATA_END = 2668
# Sixth point of the nine-word default linear volume curve at words 546–554.
VOLUME_CURVE_WORD = 551
ARCHIVE_SHA256 = 'e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a'
PROFILES = ('host-std-v1', 'software-wide-v1/libm-0.2.16')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def firmware_program(path):
    raw = path.read_bytes()
    if len(raw) != 4096 or raw[PROGRAM_BYTES:] != b'\xff' * 820:
        raise ValueError(f'invalid raw slot: {path.name}')
    return b''.join(raw[i:i + 3][::-1] for i in range(0, PROGRAM_BYTES, 3))


def checked_program(data):
    if len(data) != PROGRAM_BYTES:
        raise ValueError(f'expected {PROGRAM_BYTES} program bytes, found {len(data)}')
    words = [int.from_bytes(data[i:i + 3], 'big') for i in range(0, PROGRAM_BYTES, 3)]
    if words[CHECKSUM_WORD] != (sum(words[:1089]) & 0xffffff):
        raise ValueError('invalid final Java-format checksum')
    return data


def compare(left, right, rounding_words):
    changed_words = defaultdict(list)
    for offset in range(0, PROGRAM_BYTES, 3):
        changed = [i for i in range(offset, offset + 3) if left[i] != right[i]]
        if not changed:
            continue
        if offset // 3 == CHECKSUM_WORD:
            category = 'checksum'
        elif all(METADATA_START <= i < METADATA_END for i in changed):
            category = 'settings_metadata'
        else:
            category = 'coefficient_rounding' if offset // 3 in rounding_words else 'unknown'
        changed_words[category].append(offset // 3)
    return dict(changed_words)


def word_at(program, word):
    return int.from_bytes(program[3 * word:3 * word + 3], 'big')


def volume_rounding_words(left, right):
    # Exact binary32 product/add rounding versus carrying the double intermediate.
    def f32(value):
        return struct.unpack('>f', struct.pack('>f', value))[0]

    product = 0.625 * 8388607
    candidates = {int(f32(f32(product) + 0.5)), int(product + 0.5)}
    if {word_at(left, VOLUME_CURVE_WORD), word_at(right, VOLUME_CURVE_WORD)} == candidates:
        return {VOLUME_CURVE_WORD}
    return set()


def load_models(oracle, directory):
    models = oracle.load_models()
    initial = oracle.read(oracle.OBJECT, 0x1134)
    entries = [
        f'{row["kind"]}\t{row["id"] & 0xffffffff:08x}\t{oracle.artifact_root / row["path"]}'
        for row in models
    ]
    model_hashes = {f'{row["kind"]}:{row["id"] & 0xffffffff:08x}': row['sha256'] for row in models}
    for model, resistance, capacitance in [(-3, 1e6, 2.2e-8), (-7, 500000., 4.7e-8)]:
        oracle.write(oracle.OBJECT, initial)
        settings = default_settings()
        struct.pack_into('<I', settings, 0x14, model & 0xffffffff)
        struct.pack_into('<3f', settings, 0x38, resistance, resistance, capacitance)
        oracle.generate(settings)
        snapshot = oracle.read(oracle.OBJECT + 0xdd0, 360)
        path = directory / f'special-{model}.bin'
        preserve(path, snapshot)
        entries.append(f'special\t{model & 0xffffffff:08x}\t{path}')
        model_hashes[f'special:{model}'] = digest(snapshot)
    manifest = directory / 'models.tsv'
    preserve(manifest, ('\n'.join(entries) + '\n').encode())
    return initial, manifest, digest(json.dumps(model_hashes, sort_keys=True).encode())


def original_programs(artifact_root, directory, presets, generate, *, muted=False, count=SLOTS):
    app = artifact_root / 'analysis/extracted/windows-app'
    scripts = ROOT / 'scripts'
    subprocess.run([
        str(artifact_root / '.tools/jdk/bin/javac'), '-cp', str(app), '-d', str(directory),
        str(scripts / 'java-host-stubs/cmn/l6java/utils/L6StaticUtils.java'),
        str(scripts / 'VerifyHostPipeline.java'),
    ], check=True)
    layout = directory / 'layout.txt'
    preserve(layout, ''.join(f'{name} {offset} {kind}\n'
                             for name, (offset, kind) in FIELD_LAYOUT.items()).encode())
    cases = directory / 'cases.txt'
    # File mode reads the actual .vwm, so only flags, mask and case order matter.
    flags = 2 if muted else 0
    preserve(cases, f'00 {flags} ffffffff 00000000\n'.encode() * count)
    mapped = run_java(directory, cases, layout, '-', presets, artifact_root=artifact_root)
    if len(mapped) != count:
        raise ValueError('original Java mapping output count mismatch')
    native = bytearray()
    for slot, (settings, mask, _placeholder) in enumerate(mapped):
        try:
            program = generate(bytes.fromhex(settings), int(mask, 16))
        except (AssertionError, RuntimeError, ValueError) as error:
            raise RuntimeError(f'native generation failed at slot {slot}: {error}') from error
        if len(program) != PROGRAM_BYTES:
            raise ValueError(f'native program length mismatch at slot {slot}')
        native.extend(program)
    packed = directory / 'native.bin'
    preserve(packed, native)
    final = run_java(directory, cases, layout, packed, presets, artifact_root=artifact_root)
    if [row[:2] for row in final] != [row[:2] for row in mapped]:
        raise ValueError('original Java mapping changed between passes')
    classes = [
        'modules/variax/device/L6VariaxMIDITranscoder.class',
        'modules/variax/device/L6VariaxParamMap.class',
        'modules/variax/device/coefeng/L6VariaxCoefEngine.class',
        'cmn/l6java/patch/L6PatchUtils.class',
    ]
    return ([checked_program(bytes.fromhex(row[2])) for row in final],
            {Path(name).stem: digest((app / name).read_bytes()) for name in classes})


def make_oracle(args):
    if args.oracle_apple_runtime:
        from apple_tiger_math_oracle import AppleTigerCoefficientOracle

        return AppleTigerCoefficientOracle(args.oracle_apple_runtime, artifact_root=args.artifact_root)
    if args.oracle_software_helper:
        from math_backends import RustSoftwareMath

        return CoefficientOracle(
            math_backend=RustSoftwareMath(args.oracle_software_helper, widen_float=True),
            runtime_version='initialized-software-wide-factory-v1',
            artifact_root=args.artifact_root,
        )
    return CoefficientOracle(artifact_root=args.artifact_root)


def verify(args, directory):
    bins = dict(zip(PROFILES, (args.host_std, args.software_wide)))
    for profile, binary in bins.items():
        reported = subprocess.check_output([binary, 'math-profile'], text=True).strip()
        if reported != profile:
            raise ValueError(f'expected {profile}, found {reported}')

    source_hash = digest(args.archive.read_bytes())
    if source_hash != ARCHIVE_SHA256:
        raise ValueError('unexpected Variax 3.10 archive SHA-256')
    reference = directory / 'reference'
    subprocess.run([args.extractor, '--variax-reference', reference, args.archive],
                   stdout=subprocess.DEVNULL, check=True)

    presets = directory / 'presets'
    presets.mkdir()
    for slot in range(SLOTS):
        preserve(presets / f'{slot}.vwm', (reference / f'slot-{slot:02d}.vwm').read_bytes())

    oracle = make_oracle(args)
    initial, manifest, models_hash = load_models(oracle, directory)
    def generate(settings, _mask):
        oracle.write(oracle.OBJECT, initial)
        return oracle.generate(settings)

    original, class_hashes = original_programs(args.artifact_root, directory, presets, generate)
    pairs = {
        'std_vs_firmware': ('host_std', 'firmware'),
        'wide_vs_firmware': ('software_wide', 'firmware'),
        'original_vs_firmware': ('original', 'firmware'),
        'std_vs_original': ('host_std', 'original'),
        'wide_vs_original': ('software_wide', 'original'),
        'std_vs_wide': ('host_std', 'software_wide'),
    }
    totals = {pair: Counter() for pair in pairs}
    rows = []
    firmware_word_delta = Counter()
    for slot, original_program in enumerate(original):
        preset = presets / f'{slot}.vwm'
        programs = {
            'firmware': checked_program(firmware_program(reference / f'slot-{slot:02d}.u24le')),
            'original': original_program,
        }
        for key, binary in zip(('host_std', 'software_wide'), bins.values(), strict=True):
            output = directory / f'{key}-{slot}.packed'
            subprocess.run([binary, 'render-preset', preset, manifest, output], check=True)
            programs[key] = checked_program(output.read_bytes())
        profile_pairs = {}
        for word in range(CHECKSUM_WORD):
            std = word_at(programs['host_std'], word)
            wide = word_at(programs['software_wide'], word)
            if std != wide:
                profile_pairs[word] = {std, wide}
        differences = {}
        for pair, (left, right) in pairs.items():
            first, second = programs[left], programs[right]
            rounding_words = volume_rounding_words(first, second)
            # Attribute the discrepancy to the profile choice only if the pair
            # takes the two distinct values produced by the controlled Rust builds.
            rounding_words.update(
                word for word, profile_values in profile_pairs.items()
                if {word_at(first, word), word_at(second, word)} == profile_values
            )
            changes = compare(first, second, rounding_words)
            differences[pair] = changes
            totals[pair]['exact_slots'] += not changes
            totals[pair]['different_bytes'] += sum(
                a != b for a, b in zip(first, second, strict=True)
            )
            totals[pair].update({category: len(words) for category, words in changes.items()})
        for word in differences['original_vs_firmware'].get('unknown', []):
            native = word_at(programs['original'], word)
            firmware = word_at(programs['firmware'], word)
            delta = (native - firmware) & 0xffffff
            magnitude = min(delta, 0x1000000 - delta)
            firmware_word_delta['one'] += magnitude == 1
            firmware_word_delta['two_or_more'] += magnitude >= 2
        rows.append({
            'slot': slot,
            'preset_sha256': digest(preset.read_bytes()),
            'program_sha256': {key: digest(program) for key, program in programs.items()},
            'differing_words': differences,
        })

    math_artifact = args.oracle_apple_runtime or args.oracle_software_helper
    return {
        'archive_sha256': source_hash,
        'model_hash_map_sha256': models_hash,
        'rust_binary_sha256': {profile: digest(binary.read_bytes()) for profile, binary in bins.items()},
        'native_sha256': oracle.sha256,
        'native_runtime': oracle.RUNTIME_VERSION,
        'native_math_artifact_sha256': digest(math_artifact.read_bytes()) if math_artifact else None,
        'java_class_sha256': class_hashes,
        'word_index_base': 0,
        'totals': totals,
        'original_vs_firmware_unknown_word_distance': firmware_word_delta,
        'slots': rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('archive', 'extractor', 'host-std', 'software-wide', 'artifact-root', 'report'):
        parser.add_argument(f'--{flag}', type=lambda path: Path(path).resolve(), required=True)
    oracle = parser.add_mutually_exclusive_group()
    oracle.add_argument('--oracle-software-helper', type=Path, help='local pinned software math helper')
    oracle.add_argument('--oracle-apple-runtime', type=Path, help='pinned Apple 88.3.9 i386 slice')
    args = parser.parse_args()
    if args.report.exists():
        parser.error('report destination must be new')
    with tempfile.TemporaryDirectory(prefix='variax-factory-programs-') as temporary:
        report = verify(args, Path(temporary))
    preserve(args.report, (json.dumps(report, separators=(',', ':')) + '\n').encode())
    print(json.dumps(report['totals'], indent=2))


if __name__ == '__main__':
    main()
