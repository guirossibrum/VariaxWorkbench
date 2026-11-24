"""Measure offline differences between the archived and original Workbench programs."""

import argparse
import cmath
import math
import json
import subprocess
import tempfile
from collections import Counter
from pathlib import Path

from coefficient_oracle import CoefficientOracle, ROOT
from inspect_firmware import preserve
from verify_factory_programs import (
    ARCHIVE_SHA256, SLOTS, checked_program, digest, firmware_program,
    load_models, original_programs,
)
from verify_windows_factory_programs import PRECISION
from windows_coefficient_oracle import WindowsCoefficientOracle


def programs(args, directory):
    """Reproduce the prior reports' four program hashes on every slot."""
    docs = ROOT / 'docs'
    baseline_bytes = (docs / 'factory-program-parity.json').read_bytes()
    baseline = json.loads(baseline_bytes)
    windows = json.loads((docs / 'factory-program-windows-precision.json').read_bytes())
    if (baseline['archive_sha256'] != ARCHIVE_SHA256
            or windows['archive_sha256'] != ARCHIVE_SHA256
            or windows['baseline_report_sha256'] != digest(baseline_bytes)
            or digest(args.archive.read_bytes()) != ARCHIVE_SHA256):
        raise ValueError('factory archive or prior report fingerprint mismatch')
    if subprocess.check_output([args.host_std, 'math-profile'], text=True).strip() != 'host-std-v1':
        raise ValueError('render binary must use host-std-v1')

    reference = directory / 'reference'
    subprocess.run([args.extractor, '--variax-reference', reference, args.archive],
                   stdout=subprocess.DEVNULL, check=True)
    presets = directory / 'presets'
    presets.mkdir()
    for slot in range(SLOTS):
        preserve(presets / f'{slot}.vwm', (reference / f'slot-{slot:02d}.vwm').read_bytes())

    mac_dir = directory / 'mac'
    mac_dir.mkdir()
    mac = CoefficientOracle(artifact_root=args.artifact_root)
    mac_initial, manifest, model_hash = load_models(mac, mac_dir)
    if (mac.sha256 != baseline['native_sha256'] or mac.RUNTIME_VERSION != baseline['native_runtime']
            or model_hash != baseline['model_hash_map_sha256']):
        raise ValueError('Mac oracle fingerprint mismatch')

    def generate_mac(settings, _mask):
        mac.write(mac.OBJECT, mac_initial)
        return mac.generate(settings)

    mac_programs, mac_classes = original_programs(args.artifact_root, mac_dir, presets, generate_mac)

    win_dir = directory / 'windows'
    win_dir.mkdir()
    win = WindowsCoefficientOracle(PRECISION[53], 'initialized-sse2', artifact_root=args.artifact_root)
    records = win.load_models()
    model_hash = digest(json.dumps(
        {f'{row["kind"]}:{row["id"] & 0xffffffff:08x}': row['sha256'] for row in records},
        sort_keys=True,
    ).encode())
    if (win.SHA256 != windows['windows_pe32_sha256']
            or model_hash != windows['windows_model_hash_map_sha256']):
        raise ValueError('Windows oracle fingerprint mismatch')
    win_initial = win.read(win.ENGINE, win.ENGINE_SIZE)

    def generate_windows(settings, mask):
        win.write(win.ENGINE, win_initial)
        return win.generate(settings, mask)

    win_programs, win_classes = original_programs(args.artifact_root, win_dir, presets, generate_windows)
    if mac_classes != baseline['java_class_sha256'] or win_classes != mac_classes:
        raise ValueError('original Java class fingerprint mismatch')

    for slot, (mac_program, win_program, expected, expected_win) in enumerate(zip(
        mac_programs, win_programs, baseline['slots'], windows['slots'], strict=True,
    )):
        preset = presets / f'{slot}.vwm'
        output = directory / f'{slot}.packed'
        subprocess.run([args.host_std, 'render-preset', preset, manifest, output],
                       stdout=subprocess.DEVNULL, check=True)
        result = {
            'firmware': checked_program(firmware_program(reference / f'slot-{slot:02d}.u24le')),
            'host_std': checked_program(output.read_bytes()),
            'original': mac_program,
            'windows_53': win_program,
        }
        hashes = {key: digest(value) for key, value in result.items()}
        if (expected['slot'] != slot or expected_win['slot'] != slot
                or digest(preset.read_bytes()) != expected['preset_sha256']
                or hashes['windows_53'] != expected_win['windows_53']['windows_program_sha256']
                or any(hashes[key] != expected['program_sha256'][key]
                       for key in ('firmware', 'host_std', 'original'))
                or result['host_std'] != result['original']):
            raise ValueError(f'prior program fingerprint or Mac parity mismatch at slot {slot}')
        yield result


def word_counts(left, right, ranges):
    """Raw 24-bit count distance, independent of coefficient encoding."""
    counts = {}
    for first, last, label in ranges:
        histogram = Counter()
        for index in range(first, last + 1):
            distance = abs(left[index] - right[index])
            if distance:
                histogram[min(distance, (1 << 24) - distance)] += 1
        if histogram:
            counts[label] = dict(sorted(histogram.items()))
    return counts


# Partitions of the mapped native program. Overlay ownership is retained in the
# detailed observed-range statistics, with original writer names verbatim.
BLOCKS = (
    (0, 1, 'status'), (2, 35, 'pickup_geometry_or_mic'),
    (36, 155, 'pickup_response_or_effects'), (156, 200, 'body'),
    (201, 203, 'low_cut'), (204, 329, 'string_tone'), (330, 347, 'global_decay'),
    (348, 539, 'string_eq'), (540, 545, 'sum_to_mono'), (546, 554, 'volume'),
    (555, 586, 'sitar'), (587, 627, 'drone_and_unobserved'),
    (628, 679, 'reverb'), (680, 691, 'pitch_shift'), (692, 763, 'twelve_string_eq'),
    (764, 850, 'coil_gain_and_unobserved'), (851, 1090, 'host_overlay_and_tail'),
    (1091, 1091, 'checksum'),
)

PAIRS = {
    'rust_vs_firmware': ('host_std', 'firmware'),
    'mac_vs_windows_53': ('original', 'windows_53'),
    'windows_53_vs_firmware': ('windows_53', 'firmware'),
}
Q23 = 1 << 23
RATE = 39062.5
DB_FLOOR = -120
RESPONSE_MASKS = {
    'string_eq': 4, 'string_tone': 8, 'twelve_string_eq': 0x80000,
    'pickup_response': 0x100, 'low_cut': 0x1000, 'body': 0x200,
}


def signed(word):
    return word - 2 * Q23 if word >= Q23 else word


def q23(word, scale=1):
    return signed(word) * scale / Q23


def vx(words, address):
    exponent, mantissa = words[address:address + 2]
    if not exponent and not mantissa:
        return 0.0
    if not 1 <= exponent <= 255:
        raise ValueError(f'invalid finite VX exponent at word {address}: {exponent}')
    return math.ldexp(q23(mantissa), exponent - 127)


def section(b0=1., b1=0., b2=0., a1=0., a2=0.):
    return b0, b1, b2, a1, a2


def eq_section(words, offset):
    b2, b1, b0, a2, a1 = (q23(word, 4) for word in words[offset:offset + 5])
    return section(b0, b1, b2, a1, a2)


def resonator(words, offset):
    b2, a2, a1 = (q23(word, 2) for word in words[offset:offset + 3])
    return section(-b2, 0., b2, a1, a2)


def pickup_bandpass(inverse, q, gain):
    if not gain:
        return section(0.)
    if q <= 0.:
        raise ValueError('nonpositive Q for a nonzero pickup bandpass')
    square = inverse * inverse
    damping = inverse / q
    denominator = 1. + square + damping
    b0 = gain * damping / denominator
    return section(b0, 0., -b0, 2. * (1. - square) / denominator,
                   (1. + square - damping) / denominator)


def response_paths(words):
    """Named, coefficient-only paths. Each path is a sum of cascades."""
    paths = {name: {} for name in RESPONSE_MASKS}
    status = words[1]
    offset = 360
    for string, count in enumerate(words[354:360]):
        if count > 6:
            raise ValueError('invalid packed string EQ band count')
        stages = tuple(eq_section(words, offset + 5 * i) for i in range(count))
        paths['string_eq'][f'string_{string + 1}'] = (
            (section(q23(words[348 + string])), *stages),
        )
        offset += 5 * count
    for string, count in enumerate(words[204:210]):
        if count > 6:
            raise ValueError('invalid packed string resonator count')
        stages = tuple(
            (resonator(words, 210 + 18 * string + 3 * i),) for i in range(count)
        )
        paths['string_tone'][f'string_{string + 1}_resonators'] = stages
    for string in range(6):
        stages = tuple(eq_section(words, 692 + 10 * string + 5 * i) for i in range(2))
        paths['twelve_string_eq'][f'string_{string + 1}_eq'] = (stages,)
        # The companion gain words belong to pitch mixing, so evaluate
        # independent branch gains without adding coherent shifted signals.
        paths['twelve_string_eq'][f'string_{string + 1}_wet_eq'] = (
            (section(q23(words[758 + string])), *stages),
        )
        paths['twelve_string_eq'][f'string_{string + 1}_dry_gain'] = (
            (section(q23(words[752 + string])),),
        )
    # The original writes the pick filter and compressor after pickup response.
    # Their status flags identify active overlays in this corpus.
    if status & 0x100 and not status & (0x200000 | 0x20000):
        secondary_inverse = vx(words, 37)
        for point in range(9):
            offset = 2 * point
            pole = q23(words[39 + point])
            paths['pickup_response'][f'volume_{point}_lowpass'] = (
                (section(1. - pole, a1=-pole),),
            )
            paths['pickup_response'][f'volume_{point}_primary'] = ((
                pickup_bandpass(vx(words, 66 + offset), vx(words, 48 + offset),
                                vx(words, 84 + offset)),
            ),)
            paths['pickup_response'][f'tone_{point}_primary_gain'] = (
                (section(vx(words, 102 + offset)),),
            )
            paths['pickup_response'][f'tone_{point}_secondary'] = ((
                pickup_bandpass(secondary_inverse, vx(words, 120 + offset),
                                vx(words, 138 + offset)),
            ),)
    if words[201] not in (0, 1):
        raise ValueError('invalid packed low-cut order')
    a2, a1 = (q23(word, 2) for word in words[202:204])
    paths['low_cut']['highpass'] = ((section(1., -2., 1., a1, a2),) * (1 + words[201]),)
    if words[164] > 12:
        raise ValueError('invalid packed body resonator count')
    mix, mid = (q23(word) for word in words[156:158])
    treble_mix, treble_a1 = (q23(word, 2) for word in words[159:161])
    roll_b0, roll_a1 = (q23(word, 2) for word in words[162:164])
    paths['body']['resonator_bank'] = tuple(
        (resonator(words, 165 + 3 * i),) for i in range(words[164])
    )
    paths['body']['body_mix_gain'] = ((section(mix),),)
    paths['body']['mid_mix_gain'] = ((section(mid),),)
    paths['body']['treble_shape'] = ((section(treble_mix, -treble_mix, a1=treble_a1),),)
    paths['body']['rolloff'] = ((section(roll_b0, a1=roll_a1),),)
    return paths


def frequencies():
    low, high = 20., 20000.
    linear = (low + (high - low) * i / 16384 for i in range(16385))
    logarithmic = (low * (high / low) ** (i / 4096) for i in range(4096))
    values = sorted({*linear, *logarithmic, RATE / 2})
    return tuple((f, z, z * z) for f in values
                 for z in (cmath.exp(-2j * math.pi * f / RATE),))


def magnitude(path, z, z2):
    total = 0j
    for cascade in path:
        value = 1 + 0j
        for b0, b1, b2, a1, a2 in cascade:
            value *= (b0 + b1 * z + b2 * z2) / (1. + a1 * z + a2 * z2)
        total += value
    return abs(total)


def response_differences(left, right, status, grid):
    floor = 10 ** (DB_FLOOR / 20)
    results = {}
    for block in RESPONSE_MASKS:
        a, b = left[block], right[block]
        peaks = {
            'enabled': bool(status & RESPONSE_MASKS[block]),
            'paths': len(a.keys() & b.keys()),
            'max_db': 0.,
            'max_linear': 0.,
        }
        if a.keys() != b.keys():
            peaks['unpaired_paths'] = sorted(a.keys() ^ b.keys())
        for path in sorted(a.keys() & b.keys()):
            if a[path] == b[path]:
                continue
            for frequency, z, z2 in grid:
                ma, mb = magnitude(a[path], z, z2), magnitude(b[path], z, z2)
                db = abs(20 * math.log10(max(ma, floor) / max(mb, floor)))
                if db > peaks['max_db']:
                    peaks.update(max_db=db, db_peak_hz=frequency, db_peak_path=path,
                                 db_peak_levels=[20 * math.log10(max(m, floor)) for m in (ma, mb)])
                peaks['max_linear'] = max(peaks['max_linear'], abs(ma - mb))
        results[block] = peaks
    return results


def reverb(programs):
    firmware = programs['firmware']
    if not firmware[1] & 0x400:
        return None
    result = {}
    for name in ('host_std', 'windows_53'):
        candidate = programs[name]
        tables = {}
        for table, first, last in (('allpass_density', 632, 640), ('tank_feedback', 643, 678)):
            pairs = [(index, abs(signed(candidate[index])), abs(signed(firmware[index])))
                     for index in range(first, last + 1)]
            tables[table] = {
                'closest_candidate_margin_counts': Q23 - max(a for _, a, _ in pairs),
                'closest_firmware_margin_counts': Q23 - max(b for _, _, b in pairs),
                'toward_unity_words': [i for i, a, b in pairs if a > b],
                'toward_unity_max_counts': max(0, *(a - b for _, a, b in pairs)),
                'at_or_past_unity_words': [i for i, a, _ in pairs if a >= Q23],
                'firmware_at_or_past_unity_words': [i for i, _, b in pairs if b >= Q23],
                'changed_words': sum(candidate[i] != firmware[i] for i, _, _ in pairs),
            }
        tables['allpass_delay_changed_words'] = [
            i for i in range(628, 632) if candidate[i] != firmware[i]
        ]
        tables['reverb_control_changed_words'] = [
            i for i in (641, 642, 679) if candidate[i] != firmware[i]
        ]
        result[name] = tables
    return result


def summarize(rows, ranges, key, pair):
    result = {}
    for _, _, name in ranges:
        histogram = Counter()
        changed_per_slot = Counter()
        for row in rows:
            distances = row['comparisons'][pair][key].get(name, {})
            histogram.update(distances)
            changed_per_slot[sum(distances.values())] += 1
        result[name] = {
            'max_counts': max(histogram, default=0),
            'distance_distribution': dict(sorted(histogram.items())),
            'changed_words': histogram.total(),
            'max_changed_words_per_slot': max(changed_per_slot),
            'changed_words_per_slot_distribution': dict(sorted(changed_per_slot.items())),
        }
    return result


def summarize_response(rows, pair):
    summary = {}
    for block in RESPONSE_MASKS:
        records = [
            {'slot': row['slot'], **row['comparisons'][pair]['response'][block]}
            for row in rows
        ]
        enabled = [row for row in records if row['enabled'] and row['paths']]
        summary[block] = {
            'peak': max(records, key=lambda r: r['max_db']),
            'enabled_peak': max(enabled, key=lambda r: r['max_db'], default=None),
            'max_linear': max(row['max_linear'] for row in records),
            'slots_with_paths': sum(row['paths'] > 0 for row in records),
            'enabled_slots_with_paths': len(enabled),
            'unpaired_slots': [row['slot'] for row in records if 'unpaired_paths' in row],
        }
    return summary


def measure(program_rows):
    raw_map = (ROOT / 'docs/dsp-word-map.json').read_bytes()
    observed = json.loads(raw_map)['ranges']
    ranges = [(r['first_word'], r['last_word'], f"{r['first_word']}-{r['last_word']}")
              for r in observed] + [(1091, 1091, '1091-1091')]
    if [i for first, last, _ in ranges for i in range(first, last + 1)] != list(range(1092)):
        raise ValueError('observed word map must partition the program in order')
    grid = frequencies()
    rows = []
    for slot, programs_for_slot in enumerate(program_rows):
        words = {
            name: tuple(int.from_bytes(data[i:i + 3], 'big') for i in range(0, len(data), 3))
            for name, data in programs_for_slot.items()
        }
        status = words['firmware'][1]
        if any(values[1] != status for values in words.values()):
            raise ValueError(f'mismatched block status in slot {slot}')
        paths = {name: response_paths(values) for name, values in words.items()}
        comparisons = {}
        for pair, (a, b) in PAIRS.items():
            comparisons[pair] = {
                'blocks': word_counts(words[a], words[b], BLOCKS),
                'observed_ranges': word_counts(words[a], words[b], ranges),
                'response': response_differences(paths[a], paths[b], status, grid),
            }
        rows.append({'slot': slot, 'comparisons': comparisons, 'reverb': reverb(words)})
    if len(rows) != SLOTS or sum(row['reverb'] is not None for row in rows) != 10:
        raise ValueError('unexpected factory slot or active reverb count')

    summary = {
        pair: {
            'blocks': summarize(rows, BLOCKS, 'blocks', pair),
            'observed_ranges': summarize(rows, ranges, 'observed_ranges', pair),
            'response': summarize_response(rows, pair),
        } for pair in PAIRS
    }
    return {
        'archive_sha256': ARCHIVE_SHA256, 'dsp_word_map_sha256': digest(raw_map),
        'word_index_base': 0, 'slot_index_base': 0, 'sample_rate_hz': RATE,
        'response_db_floor': DB_FLOOR, 'grid_points': len(grid), 'pairs': PAIRS,
        'summary': summary, 'slots': rows,
    }


# Proposed factory-corpus screening limits, calibrated above the Windows
# comparator's ordinary word and enabled response peaks. The shared coil-gain
# offset gets its own ceiling. The issue's strict reverb rule is independent.
PROPOSED_BOUND = {
    'ordinary_packed_word_counts': 32,
    'coil_gain_counts': Q23 // 100,
    'enabled_response_db': 0.1,
}


def assess_bound(report):
    comparisons = {}
    for pair, candidate in (('rust_vs_firmware', 'host_std'),
                            ('windows_53_vs_firmware', 'windows_53')):
        stats = report['summary'][pair]
        observed = stats['observed_ranges']
        ordinary_peak = max(
            row['max_counts'] for name, row in observed.items()
            if name not in ('771-774', '1091-1091')
        )
        coil_peak = observed['771-774']['max_counts']
        response_peak = max(
            item['enabled_peak']['max_db'] for item in stats['response'].values()
            if item['enabled_peak'] is not None
        )
        observed_max = {
            'ordinary_packed_word_counts': ordinary_peak,
            'coil_gain_counts': coil_peak,
            'enabled_response_db': response_peak,
        }
        toward_or_at_unity = []
        for row in report['slots']:
            if row['reverb'] is None:
                continue
            tank = row['reverb'][candidate]['tank_feedback']
            if (tank['toward_unity_words'] or tank['at_or_past_unity_words']
                    or tank['firmware_at_or_past_unity_words']):
                toward_or_at_unity.append(row['slot'])
        comparisons[pair] = {
            'observed_max': observed_max,
            'limits_exceeded': [name for name, limit in PROPOSED_BOUND.items()
                                if observed_max[name] > limit],
            'unpaired_response': any(row['unpaired_slots'] for row in stats['response'].values()),
            'tank_feedback_stop_slots': toward_or_at_unity,
        }
    return {'limits': PROPOSED_BOUND, 'comparisons': comparisons}


def finalize_report(report):
    docs = ROOT / 'docs'
    report['prior_report_sha256'] = {
        name: digest((docs / name).read_bytes())
        for name in ('factory-program-parity.json', 'factory-program-windows-precision.json')
    }
    report['proposed_bound'] = assess_bound(report)
    for row in report['slots']:
        for comparison in row['comparisons'].values():
            response = comparison['response']
            for label, enabled_only in (('peak_response', False), ('peak_enabled_response', True)):
                measured = {
                    block: record for block, record in response.items()
                    if record['paths'] and (record['enabled'] or not enabled_only)
                }
                if measured:
                    block = max(measured, key=lambda item: measured[item]['max_db'])
                    comparison[label] = {'block': block, **measured[block]}
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('artifact-root', 'archive', 'extractor', 'host-std', 'report'):
        parser.add_argument(f'--{flag}', type=lambda path: Path(path).resolve(), required=True)
    args = parser.parse_args()
    if args.report.exists():
        parser.error('report destination must be new')
    with tempfile.TemporaryDirectory(prefix='variax-factory-response-') as temporary:
        report = measure(programs(args, Path(temporary)))
    report = finalize_report(report)
    preserve(args.report, (json.dumps(report, separators=(',', ':'), allow_nan=False) + '\n').encode())
    print(json.dumps(report['proposed_bound'], indent=2))


if __name__ == '__main__':
    main()
