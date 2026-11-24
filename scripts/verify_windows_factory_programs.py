"""Compare the original PE32 Windows coefficient engine with the 3.10 factory archive."""

import argparse
import json
import struct
import subprocess
import tempfile
from collections import Counter
from pathlib import Path

from inspect_firmware import preserve
from verify_factory_programs import (
    ARCHIVE_SHA256, PROGRAM_BYTES, SLOTS, VOLUME_CURVE_WORD, checked_program, compare,
    digest, firmware_program, original_programs, volume_rounding_words, word_at,
)
from windows_coefficient_oracle import WindowsCoefficientOracle


PRECISION = {24: 0x07f, 53: 0x27f, 64: 0x37f}


def verify(args, directory):
    if digest(args.archive.read_bytes()) != ARCHIVE_SHA256:
        raise ValueError('unexpected Variax 3.10 archive SHA-256')
    baseline = json.loads(args.baseline.read_text())
    if baseline['archive_sha256'] != ARCHIVE_SHA256:
        raise ValueError('comparison uses a different factory archive')
    reference = directory / 'reference'
    subprocess.run([args.extractor, '--variax-reference', reference, args.archive],
                   stdout=subprocess.DEVNULL, check=True)
    presets = directory / 'presets'
    presets.mkdir()
    for slot in range(SLOTS):
        preserve(presets / f'{slot}.vwm', (reference / f'slot-{slot:02d}.vwm').read_bytes())

    oracle = WindowsCoefficientOracle(PRECISION[args.precision], args.dispatch,
                                      artifact_root=args.artifact_root, quantizer_f32=args.quantizer_f32)
    curve = oracle.volume_curve()
    firmware_curve_differences = [
        [546 + i for i, word in enumerate(curve) if word_at(firmware, 546 + i) != word]
        for firmware in (
            firmware_program(reference / f'slot-{slot:02d}.u24le') for slot in range(SLOTS)
        )
    ]
    report = {
        'archive_sha256': ARCHIVE_SHA256,
        'windows_pe32_sha256': oracle.SHA256,
        'baseline_report_sha256': digest(args.baseline.read_bytes()),
        'x87_precision': args.precision,
        'x87_control_word': hex(PRECISION[args.precision]),
        'dispatch': args.dispatch,
        'sse2_selected': bool(oracle.sse2_selected),
        'quantizer_f32': args.quantizer_f32,
        'cpp_initializers': len(oracle.initializers),
        'exit_callbacks_captured': len(oracle.registered_exit_callbacks),
        'volume_curve_sha256': digest(struct.pack('>9I', *curve)),
        'volume_curve_mismatches_by_slot': firmware_curve_differences,
    }
    if args.volume_only:
        report['totals'] = {'firmware_exact_volume_curves': firmware_curve_differences.count([])}
        return report

    records = oracle.load_models()
    initial = oracle.read(oracle.ENGINE, oracle.ENGINE_SIZE)

    def generate(settings, mask):
        oracle.write(oracle.ENGINE, initial)
        return oracle.generate(settings, mask)

    programs, class_hashes = original_programs(args.artifact_root, directory, presets, generate)
    if class_hashes != baseline['java_class_sha256']:
        raise ValueError('comparison uses different original Java classes')
    counts = Counter()
    rows = []
    for slot, (expected, candidate) in enumerate(zip(baseline['slots'], programs, strict=True)):
        firmware = checked_program(firmware_program(reference / f'slot-{slot:02d}.u24le'))
        if (expected['slot'] != slot or digest(firmware) != expected['program_sha256']['firmware']
                or digest((presets / f'{slot}.vwm').read_bytes()) != expected['preset_sha256']):
            raise ValueError(f'comparison uses different firmware or preset bytes for slot {slot}')
        old = expected['differing_words']['std_vs_firmware']
        old_words = set(old.get('unknown', [])) | set(old.get('coefficient_rounding', []))
        differences = compare(candidate, firmware, volume_rounding_words(candidate, firmware))
        new_words = set(differences.get('unknown', [])) | set(differences.get('coefficient_rounding', []))
        counts['exact_slots'] += not differences
        counts['different_bytes'] += sum(a != b for a, b in zip(candidate, firmware, strict=True))
        counts.update({category: len(words) for category, words in differences.items()})
        counts['resolved_positions'] += len(old_words - new_words)
        counts['new_positions'] += len(new_words - old_words)
        counts['unknown_resolved_positions'] += len(set(old.get('unknown', [])) - new_words)
        counts['matches_synthetic_volume_point'] += word_at(candidate, VOLUME_CURVE_WORD) == curve[5]
        counts['firmware_matches_synthetic_volume_point'] += word_at(firmware, VOLUME_CURVE_WORD) == curve[5]
        counts['matches_std'] += digest(candidate) == expected['program_sha256']['host_std']
        counts['matches_wide'] += digest(candidate) == expected['program_sha256']['software_wide']
        rows.append({
            'slot': slot, 'windows_program_sha256': digest(candidate),
            'different_words': differences,
            'resolved_positions': sorted(old_words - new_words),
            'new_positions': sorted(new_words - old_words),
        })
    return {
        **report,
        'windows_model_hash_map_sha256': digest(json.dumps(
            {f'{item["kind"]}:{item["id"] & 0xffffffff:08x}': item['sha256'] for item in records},
            sort_keys=True,
        ).encode()),
        'model_resources': len(records),
        'totals': counts,
        'slots': rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('artifact-root', 'archive', 'extractor', 'baseline', 'report'):
        parser.add_argument(f'--{flag}', type=lambda path: Path(path).resolve(), required=True)
    parser.add_argument('--precision', type=int, choices=PRECISION, required=True)
    parser.add_argument('--dispatch', choices=('initialized-sse2', 'image-default-x87'), required=True)
    parser.add_argument('--volume-only', action='store_true')
    parser.add_argument('--quantizer-f32', action='store_true',
                        help='narrow x87 precision only inside the original Q23 converter')
    args = parser.parse_args()
    if args.report.exists():
        parser.error('report destination must be new')
    with tempfile.TemporaryDirectory(prefix='variax-windows-factory-') as temporary:
        report = verify(args, Path(temporary))
    preserve(args.report, (json.dumps(report, separators=(',', ':')) + '\n').encode())
    print(json.dumps(report['totals'], indent=2))


if __name__ == '__main__':
    main()
