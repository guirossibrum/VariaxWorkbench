"""Synthetic complete presets for the legacy editor domains in src/edit.rs."""

from collections import Counter
from dataclasses import dataclass
from itertools import chain
import random
import struct


SEED = 180_600_175
RANDOM_CASES = 1000
BODIES = tuple(range(0x03100000, 0x0310001c))
PICKUPS = (*range(0x03110000, 0x03110016), *range(0x03118000, 0x0311800a))
ACOUSTICS = frozenset((0x03100012, 0x03100013, 0x03100014, 0x03100015,
                       0x03100016, 0x03100017, 0x0310001a, 0x0310001b))
TWELVE = (0x0310000d, 0x03100013, 0x03100015)
SITAR = 0x03100018
BODY, PICKUP1, PICKUP2 = 0x1010000, 0x1020000, 0x1030000
ENABLE1, ENABLE2, TUNING = 0x2020001, 0x2030001, 0x2050000
TRIM = 0xff0000
PHASE, SERIES = 196613, 196614
TAPER, TONE, SAVE_TONE = 262145, 262148, 262149
RESISTANCES = (25, 50, 100, 250, 470, 500, 1000)
CAPACITANCES = (0.0011, 0.0022, 0.0047, 0.011, 0.022, 0.047, 0.1, -0.047)
ELECTRIC_FLOATS = {
    131074: (0.1, 9.5), 196610: (0.1, 9.5),
    131075: (-45, 45), 196611: (-45, 45),
    131076: (-18, 6), 196612: (-18, 6),
    TONE: (0, 1),
}
TWELVE_FLOATS = {
    **dict.fromkeys(range(327687, 327693), (-100, 100)),
    **dict.fromkeys(range(327693, 327699), (0, 1)),
    327699: (0, 25),
}
CHOICES = {262144: RESISTANCES, 262146: RESISTANCES, 262147: CAPACITANCES}


def float_bits(value):
    return struct.unpack('>I', struct.pack('>f', value))[0]


def endpoints(low, high):
    a, b = float_bits(low), float_bits(high)
    # The series includes the immediate representable neighbour inside each bound.
    return (a, a - 1 if a >> 31 else a + 1, b - 1, b)


def defaults(body=0x03100000):
    return {
        BODY: body, PICKUP1: PICKUPS[0], PICKUP2: PICKUPS[1],
        ENABLE1: int(body not in ACOUSTICS), ENABLE2: int(body not in ACOUSTICS),
        TUNING: 0, TRIM: float_bits(0),
        131074: float_bits(2), 196610: float_bits(6),
        131075: 0, 196611: 0, 131076: 0, 196612: 0,
        PHASE: 0, SERIES: 0, TAPER: 0, SAVE_TONE: 0, TONE: float_bits(0.5),
        262144: float_bits(250), 262146: float_bits(250), 262147: float_bits(0.022),
        **dict.fromkeys(range(327681, 327693), 0),
        **dict.fromkeys(range(327693, 327699), float_bits(0.5)),
        327699: float_bits(0),
    }


@dataclass(frozen=True)
class Case:
    group: str
    name: str
    parameters: dict

    def line(self):
        entries = ' '.join(f'{key:08x}:{value & 0xffffffff:08x}'
                           for key, value in sorted(self.parameters.items()))
        return f'{self.name.encode().hex()} 0 ffffffff 00000000 {entries}'


def pairings(bodies, *, dormant=False):
    for body in bodies:
        if body in ACOUSTICS and not dormant:
            yield 'acoustic-body', defaults(body)
            continue
        # The editor retains an electric body's settings on a switch to acoustic.
        # These cases cover the same stored selections without editing dormant controls.
        base = (defaults() if dormant else defaults(body)) | {BODY: body}
        for index, first in enumerate(PICKUPS):
            yield ('dormant-pairing' if dormant else 'pairing'), base | {
                PICKUP1: first, PICKUP2: PICKUPS[(index + 11) % len(PICKUPS)],
                SERIES: index & 1, PHASE: (index >> 1) & 1,
            }
        for first, second, phase in ((1, 0, 0), (1, 0, 1), (0, 1, 0), (0, 1, 1)):
            yield ('dormant-single' if dormant else 'single-pickup'), base | {
                ENABLE1: first, ENABLE2: second, PHASE: phase,
            }


def focused():
    for parameter, bounds in ({TRIM: (-20, 6)} | ELECTRIC_FLOATS).items():
        for value in endpoints(*bounds):
            yield 'continuous-boundary', defaults() | {parameter: value, SAVE_TONE: 1}
    for body in TWELVE:
        enabled = defaults(body) | {TUNING: 1}
        for parameter, bounds in TWELVE_FLOATS.items():
            for value in endpoints(*bounds):
                yield 'twelve-boundary', enabled | {parameter: value}
        yield 'twelve-off', defaults(body)
    for parameter, values in CHOICES.items():
        for value in values:
            yield 'electrical-choice', defaults() | {parameter: float_bits(value)}
    for taper in (0, 1):
        yield 'taper-choice', defaults() | {TAPER: taper}
    for body in (BODIES[0], TWELVE[0], TWELVE[1], 0x03100012):
        for index in range(6):
            for semitones in (-12, -11, 11, 12):
                yield 'tuning-boundary', defaults(body) | {TUNING: 1, 327681 + index: semitones}


def seeded():
    rng = random.Random(SEED)
    for index in range(RANDOM_CASES):
        body = BODIES[index % len(BODIES)]
        params = defaults(body)
        params[TRIM] = float_bits(rng.uniform(-20, 6))
        if body not in ACOUSTICS:
            params.update({key: float_bits(rng.uniform(*bounds)) for key, bounds in ELECTRIC_FLOATS.items()})
            params.update({key: float_bits(rng.choice(values)) for key, values in CHOICES.items()})
            params.update({PICKUP1: rng.choice(PICKUPS), PICKUP2: rng.choice(PICKUPS)})
            params.update(dict(zip((ENABLE1, ENABLE2), rng.choice(((1, 1), (1, 0), (0, 1))), strict=True)))
            params.update({key: rng.randrange(2) for key in (PHASE, SERIES, TAPER, SAVE_TONE)})
        if body != SITAR:
            params[TUNING] = rng.randrange(2)
            if params[TUNING]:
                params.update({key: rng.randint(-12, 12) for key in range(327681, 327687)})
                if body in TWELVE:
                    params.update({key: float_bits(rng.uniform(*bounds)) for key, bounds in TWELVE_FLOATS.items()})
        yield 'random', params


def cases():
    anchor = Case('repeat', 'Wide anchor', defaults(0x03100012))
    yield anchor
    rows = chain(pairings(BODIES), focused(), seeded(), pairings(sorted(ACOUSTICS), dormant=True))
    for index, (group, parameters) in enumerate(rows):
        yield Case(group, f'Wide {index:04d}', parameters)
    yield anchor


def coverage(corpus):
    groups = Counter(case.group for case in corpus)
    for body in BODIES:
        pairs = [case.parameters for case in corpus
                 if case.group in ('pairing', 'dormant-pairing') and case.parameters[BODY] == body]
        for slot in (PICKUP1, PICKUP2):
            if {p[slot] for p in pairs if p[ENABLE1] and p[ENABLE2]} != set(PICKUPS):
                raise ValueError(f'pickup coverage incomplete for {body:08x}')
        if {(p[SERIES], p[PHASE]) for p in pairs} != {(0, 0), (0, 1), (1, 0), (1, 1)}:
            raise ValueError(f'wiring coverage incomplete for {body:08x}')
        singles = [case.parameters for case in corpus
                   if case.group in ('single-pickup', 'dormant-single') and case.parameters[BODY] == body]
        if {(p[ENABLE1], p[ENABLE2], p[PHASE]) for p in singles} != {
            (1, 0, 0), (1, 0, 1), (0, 1, 0), (0, 1, 1),
        }:
            raise ValueError(f'single-pickup coverage incomplete for {body:08x}')
    if {case.parameters[BODY] for case in corpus} != set(BODIES) or groups['random'] != RANDOM_CASES:
        raise ValueError('body or random coverage incomplete')
    return dict(sorted(groups.items()))
