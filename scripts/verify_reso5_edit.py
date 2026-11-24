"""Reproduce both Reso-5 edit programs with the original Workbench host oracle."""

import argparse
import json
import subprocess
import tempfile
from pathlib import Path

from verify_factory_programs import (
    ARCHIVE_SHA256, ROOT, CoefficientOracle, checked_program, digest,
    firmware_program, load_models, original_programs, preserve, word_at,
)
from verify_factory_response import signed


def verify(args, directory):
    baseline = json.loads((ROOT / 'docs/factory-program-parity.json').read_bytes())
    if digest(args.archive.read_bytes()) != ARCHIVE_SHA256:
        raise ValueError('unexpected Variax 3.10 archive fingerprint')
    if subprocess.check_output([args.host, 'math-profile'], text=True).strip() != 'host-std-v1':
        raise ValueError('render binary must use host-std-v1')

    reference = directory / 'reference'
    subprocess.run([args.extractor, '--variax-reference', reference, args.archive],
                   stdout=subprocess.DEVNULL, check=True)
    presets = directory / 'presets'
    presets.mkdir()
    stored = reference / 'slot-50.vwm'
    slot = baseline['slots'][50]
    if digest(stored.read_bytes()) != slot['preset_sha256']:
        raise ValueError('unexpected Reso-5 settings fingerprint')
    preserve(presets / '0.vwm', stored.read_bytes())
    subprocess.run([args.host, 'set', stored, presets / '1.vwm',
                    '02010000', '200000', 'float', '-6.0'], check=True)

    oracle = CoefficientOracle(artifact_root=args.artifact_root)
    models_dir = args.models_dir or directory
    if args.models_dir:
        models_dir.mkdir()
    initial, manifest, models = load_models(oracle, models_dir)
    if (oracle.sha256 != baseline['native_sha256']
            or oracle.RUNTIME_VERSION != baseline['native_runtime']
            or models != baseline['model_hash_map_sha256']):
        raise ValueError('unexpected Mac oracle fingerprint')

    def generate(settings, _mask):
        oracle.write(oracle.OBJECT, initial)
        return oracle.generate(settings)

    originals, classes = original_programs(args.artifact_root, directory, presets, generate, count=2)
    if classes != baseline['java_class_sha256']:
        raise ValueError('unexpected original Java class fingerprints')
    firmware = checked_program(firmware_program(reference / 'slot-50.u24le'))
    if digest(firmware) != slot['program_sha256']['firmware']:
        raise ValueError('unexpected firmware program fingerprint')

    for index, original in enumerate(originals):
        output = directory / f'{index}.packed'
        subprocess.run([args.host, 'render-preset', presets / f'{index}.vwm', manifest, output],
                       stdout=subprocess.DEVNULL, check=True)
        rust = checked_program(output.read_bytes())
        if rust != original:
            raise ValueError(f'Rust program differs from Mac oracle for stage {index + 1}')
        if index == 0 and digest(original) != slot['program_sha256']['original']:
            raise ValueError('stage 1 differs from the prior original reference')
        if index == 0 and rust[2553:2745] != firmware[2553:2745]:
            raise ValueError('stage 1 did not retain the original 192 metadata bytes')
        if index == 1:
            baseline_metadata = bytearray(originals[0][2553:2745])
            baseline_metadata[65:68] = bytes.fromhex('c0c000')
            if rust[2553:2745] != baseline_metadata:
                raise ValueError('stage 2 did not encode the trim edit')
        words = [*range(632, 641), *range(643, 679)]
        candidate = [signed(word_at(rust, i)) for i in words]
        archived = [signed(word_at(firmware, i)) for i in words]
        distance = max(abs(a - b) for a, b in zip(candidate, archived, strict=True))
        margin = (1 << 23) - max(map(abs, candidate))
        if distance > 2 or margin < 838_861:
            raise ValueError(f'reverb check failed for stage {index + 1}')
        print(f'Stage {index + 1}: {digest(original)}. '
              f'Reverb max difference {distance}, closest margin {margin} counts.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('artifact-root', 'archive', 'extractor', 'host'):
        parser.add_argument(f'--{name}', type=Path, required=True)
    parser.add_argument('--models-dir', type=Path,
                        help='write the private model manifest and special snapshots to a new directory')
    args = parser.parse_args()
    for name, path in vars(args).items():
        if path is not None:
            setattr(args, name, path.resolve())
    with tempfile.TemporaryDirectory(prefix='variax-reso5-reference-') as work:
        verify(args, Path(work))


if __name__ == '__main__':
    main()
