#!/usr/bin/env python3
"""Compare the PODxt Live table with Line 6 Edit through a recording sender."""
import argparse
from collections import Counter, defaultdict
from dataclasses import dataclass
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
TABLE = ROOT / "research/podxt-live-parameters.json"
EDIT_PREFIX = "sysex:110:740a"


def float_bits(value):
    return struct.unpack(">I", struct.pack(">f", value))[0]


def from_bits(bits):
    return struct.unpack(">f", struct.pack(">I", bits))[0]


def f32(value):
    return from_bits(float_bits(value))


def split14(value):
    return [value >> 7, value & 127]


def clamp(value, maximum):
    return min(maximum, max(0, value))


def encoded(kind, bits, ordinal=None):
    """Evaluate the data's named encodings with explicit binary32 rounding."""
    if kind is None:
        return []
    if kind == "model-index":
        return [ordinal]
    if kind in {"integer", "variax-wrapped"}:
        return [bits]
    if kind in {"switch", "inverted-switch"}:
        return [127 * (bool(bits) != (kind == "inverted-switch"))]
    if kind == "pedal-assignment":
        return [[0, 64, 127][bits]]
    if kind == "constant-127":
        return [127]

    value = from_bits(bits)
    if kind == "float7-nearest":
        return [clamp(int(f32(f32(value * 127) + 0.5)), 127)]
    if kind == "float14-nearest":
        return split14(clamp(int(f32(f32(value * 16383) + 0.5)), 16383))
    if kind == "float14-truncate":
        return split14(clamp(int(f32(value * 16383)), 16383))
    if kind == "tempo-tenths":
        return split14(int(f32(value * 10)))
    if kind == "gate-db":
        return [int(-value)]
    if kind == "gate-db-ranged":
        return [int(f32(f32(f32(f32(value + 96) / 96) * -96) + 96))]
    raise ValueError(f"Unknown encoding {kind}")


def expected_live(row, bits, ordinal, defaults):
    controllers = row.get("live_cc_defaults_on", []) if defaults else row["live_cc"]
    if "shared_encoder_argument" in row:
        controllers = [row["shared_encoder_argument"]]
    values = encoded(row["live_encoding"], bits, ordinal)
    if row["live_encoding"] == "variax-wrapped":
        return "sysex:111:" + bytes((16, 0, controllers[0], values[0])).hex()
    return ",".join(f"cc:{cc}:{v}" for cc, v in zip(controllers, values, strict=True))


def stored_prefix(index):
    address = index if index < 64 else index + 128
    return "sysex:110:" + bytes((0x71, 0x0a, *split14(address))).hex()


def parse_tone(record, prefix):
    actual_prefix, payload = record.split(",")
    if actual_prefix != prefix or not payload.startswith("sysex:101:"):
        raise AssertionError(f"Unexpected tone framing: {record}")
    decoded = bytes.fromhex(payload.removeprefix("sysex:101:"))
    if len(decoded) != 160:
        raise AssertionError(f"Expected 160 tone bytes, got {len(decoded)}")
    return decoded


def name_bytes(name):
    units = name.encode("utf-16-be")[1::2]
    return bytes(32 if b == 0 else b if 32 <= b <= 122 and b != 96 else 95 for b in units)


@dataclass(frozen=True)
class Sample:
    bits: int
    domain: str
    ordinal: int | None = None
    wire_grid: int | None = None
    nominal: int | None = None


def samples(table, row, definition):
    """Cover wire grids, off-grid rounding probes, and exact formatter choice keys."""
    kind = row["live_encoding"]
    if kind == "model-index":
        for model in table["catalog"]:
            if model["block"] == row["catalog"]:
                yield Sample(model["id"], "model", ordinal=model["index"])
    elif kind in {"inverted-switch", "switch", "pedal-assignment"}:
        for bits in range(3 if kind == "pedal-assignment" else 2):
            yield Sample(bits, "choice")
    elif kind == "tempo-tenths":
        for raw in range(300, 2401):
            yield Sample(float_bits(raw / 10), "wire-grid", wire_grid=raw)
    elif kind == "gate-db":
        for raw_quarters in range(385):
            domain = "wire-grid" if raw_quarters % 4 == 0 else "fractional-db"
            yield Sample(float_bits(-raw_quarters / 4), domain)
    elif kind in {"float7-nearest", "float14-nearest"}:
        fmt = table["formats"].get(table["definitions"].get(definition, {}).get("format"), {})
        if fmt.get("choices") and len(fmt["choices"][0]) == 3:
            for raw, label, bits in fmt["choices"]:
                yield Sample(bits, label, nominal=raw)
        else:
            max_value = 16383 if kind == "float14-nearest" else 127
            for raw in range(max_value + 1):
                yield Sample(float_bits(raw / max_value), "wire-grid", wire_grid=raw)
            if max_value == 16383:
                for raw in range(max_value):
                    yield Sample(float_bits((raw + 0.5) / max_value), "half-step")
    elif kind in {"integer", "variax-wrapped"}:
        low, high = row["wire_range"]
        for bits in range(low, high + 1):
            yield Sample(bits, "raw-integer")
    elif kind is None:
        yield Sample(0, "no-outbound-path")
    else:
        raise ValueError(f"Unknown sample domain {kind}")


@dataclass(frozen=True)
class Case:
    request: tuple
    key: str
    block: str
    row: dict | None = None
    sample: Sample | None = None
    context: tuple = (-1, 0)
    expected: object = None
    defaults: bool = False


def cases(table):
    parameters = {row["key"]: row for row in table["parameters"]}
    slots = defaultdict(list)
    for row in parameters.values():
        if row.get("definition") == "model-specific":
            slots[row["block"]].append(row)
    context_models = defaultdict(list)
    for layout in table["layouts"]:
        block = layout["block"]
        parent = parameters[block.upper() + "_MODEL"]["selector"]
        context = (parent, layout["model_id"])
        for row in slots[block]:
            key = layout["knobs"].get(row["key"])
            label = table["definitions"][key]["label"] if key else "-"
            yield Case(("D", *context, row["selector"]), row["key"], block, expected=label)
            if key:
                context_models[row["key"]].append((context, key))
    mic = parameters["MIC_MODEL"]
    cab = parameters["CAB_MODEL"]["selector"]
    for model in (m["id"] for m in table["catalog"] if m["block"] == "cab"):
        definition = mic["bass_cab_definition"] if model in mic["bass_cab_ids"] else mic["definition"]
        context_models[mic["key"]].append(((cab, model), definition))

    selectors = []
    for row in parameters.values():
        key, block, selector = row["key"], row["block"], row["selector"]
        if selector is None:
            if "named_cc" in row:
                yield Case(("C", key), key, block, expected=str(row["named_cc"]))
            continue
        if key not in {"DELAY_TWEAK", "MOD_TWEAK"}:
            selectors.append(selector)
        for context, definition in context_models[key] or [((-1, 0), row.get("definition"))]:
            yield Case(("B", *context), key, block, context=context)
            for sample in samples(table, row, definition):
                for defaults in ((False, True) if key == "AMP_MODEL" else (False,)):
                    yield Case(("P", selector, sample.bits, *context, int(defaults)),
                               key, block, row, sample, context, defaults=defaults)
        if "incoming_cc" in row:
            controller = row["incoming_cc"]
            for raw in range(128):
                expected = f"{selector}:{float_bits(raw / 127)}"
                yield Case(("I", controller, raw), key, block, expected=expected)
    yield Case(("L",), "ALL_PARAMS", "metadata", expected=selectors)
    for row in table["model_names"]:
        yield Case(("A", row["id"]), f"asset:{row['id']:08x}", "catalog", expected=row["name"])
    for name in ("", "0123456789ABCDEF", *(f"T{chr(n)}Z" for n in range(256))):
        yield Case(("N", name.encode("utf-16-be").hex()), "TONE_NAME", "metadata", expected=name)
    for index in range(128):
        yield Case(("S", index), "TONE_ADDRESS", "metadata", expected=index)


class Comparison:
    def __init__(self, table):
        self.baselines = {}
        self.reserved = next(row["tone_offsets"] for row in table["parameters"]
                             if row["key"] == "UNASSIGNED_TONE_BYTES")
        self.counts = Counter()
        self.block_cases = Counter()
        self.findings = {}
        self.covered_rows = set()
        self.path_differences = defaultdict(Counter)
        self.grid_differences = defaultdict(Counter)

    def check(self, case, check, expected, observed, domain=None):
        self.counts[check] += 1
        if expected == observed:
            return
        group = (case.key, check)
        finding = self.findings.setdefault(group, {"count": 0, "domains": Counter()})
        finding["count"] += 1
        finding["domains"][domain or "default"] += 1
        if "example" not in finding:
            finding["example"] = {"request": case.request, "context": case.context,
                                  "domain": domain, "expected": expected, "observed": observed}

    def tone_pair(self, case, edit, stored):
        tones = (parse_tone(edit, EDIT_PREFIX), parse_tone(stored, stored_prefix(80)))
        for destination, tone in zip(("edit", "stored"), tones, strict=True):
            self.check(case, destination + "-unassigned", [],
                       [(i, tone[i]) for i in self.reserved if tone[i] != 0])
        self.check(case, "edit/stored-payload-parity", True, tones[0] == tones[1])
        return tones

    def compare_parameter(self, case, line):
        row, sample = case.row, case.sample
        set_param, live, edit, stored = line.split("\t")
        tones = self.tone_pair(case, edit, stored)
        self.block_cases[case.block] += 1
        self.covered_rows.add(case.key)
        self.check(case, "live", expected_live(row, sample.bits, sample.ordinal, case.defaults),
                   live, sample.domain)
        if sample.nominal is not None:
            self.check(case, "association-key-live", f"cc:{row['live_cc'][0]}:{sample.nominal}",
                       live, sample.domain)
        offsets = row["tone_offsets"]
        expected_bytes = encoded(row["tone_encoding"], sample.bits, sample.ordinal)
        for destination, actual_tone, baseline in zip(
                ("edit", "stored"), tones, self.baselines[case.context], strict=True):
            self.check(case, destination + "-field", expected_bytes,
                       [actual_tone[i] for i in offsets], sample.domain)
            companions = [(i, before, after) for i, (before, after) in
                          enumerate(zip(baseline, actual_tone, strict=True))
                          if i not in offsets and before != after]
            self.check(case, destination + "-outside-field", [], companions, sample.domain)
            if sample.nominal is not None:
                self.check(case, "association-key-" + destination, [sample.nominal],
                           [actual_tone[i] for i in offsets], sample.domain)
        if offsets and row["live_encoding"] != "variax-wrapped":
            live_values = [int(event.rsplit(":", 1)[1]) for event in live.split(",") if event]
            tone_values = [tones[0][i] for i in offsets]
            if live_values != tone_values:
                self.path_differences[case.key][sample.domain] += 1
            if sample.wire_grid is not None and len(offsets) == 2:
                ideal = split14(sample.wire_grid)
                if live_values != ideal:
                    self.grid_differences[case.key]["live"] += 1
                if tone_values != ideal:
                    self.grid_differences[case.key]["tone"] += 1
        if row.get("file_field") == "PARM" and offsets:
            self.check(case, "map-set", "true", set_param, sample.domain)

    def consume(self, case, line):
        operation = case.request[0]
        if operation == "P":
            self.compare_parameter(case, line)
        elif operation == "B":
            self.baselines[case.context] = self.tone_pair(case, *line.split("\t"))
        elif operation == "N":
            expected = name_bytes(case.expected).ljust(32, b" ")
            for destination, tone in zip(("edit", "stored"),
                                         self.tone_pair(case, *line.split("\t")), strict=True):
                self.check(case, destination + "-name", expected.hex(), tone[:32].hex())
        elif operation == "S":
            parse_tone(line, stored_prefix(case.expected))
            self.counts["stored-address"] += 1
        elif operation == "L":
            self.check(case, "selector-list", case.expected, json.loads(line))
        else:
            check = {"A": "asset-name", "C": "command-constant", "D": "layout-definition", "I": "incoming"}[operation]
            self.check(case, check, case.expected, line)

    def report(self):
        return {"parameter_rows": len(self.covered_rows),
                "block_parameter_cases": dict(sorted(self.block_cases.items())),
                "checks": dict(sorted(self.counts.items())),
                "live_vs_tone": dict(sorted(self.path_differences.items())),
                "14_bit_grid_round_trip": dict(sorted(self.grid_differences.items())),
                "findings": {f"{row}/{check}": value
                             for (row, check), value in sorted(self.findings.items())}}


def verify(classes, jdk):
    table = json.loads(TABLE.read_text())
    plan = list(cases(table))
    comparison = Comparison(table)
    environment = os.environ.copy()
    # Ignore launcher overrides that could inject a Java agent or classpath.
    for key in ("JAVA_TOOL_OPTIONS", "_JAVA_OPTIONS", "JDK_JAVA_OPTIONS", "CLASSPATH"):
        environment.pop(key, None)
    with tempfile.TemporaryDirectory(prefix="podxt-parameters-") as directory:
        work = Path(directory)
        fixture = work / "input.tsv"
        with fixture.open("w", encoding="utf-8") as output:
            for case in plan:
                print(*case.request, sep="\t", file=output)
        subprocess.run([jdk / "javac", "-encoding", "UTF-8", "-cp", classes,
                        "-d", work, ROOT / "scripts/VerifyPodxtParameters.java"],
                       check=True, env=environment)
        command = [jdk / "java", "-XX:-UsePerfData", "-Djava.awt.headless=true", "-cp",
                   os.pathsep.join((str(work), str(classes))), "VerifyPodxtParameters"]
        with fixture.open("rb") as source, subprocess.Popen(
                command, stdin=source, stdout=subprocess.PIPE, encoding="utf-8",
                env=environment) as binary:
            try:
                for case, line in zip(plan, binary.stdout, strict=True):
                    comparison.consume(case, line.rstrip("\n"))
            except BaseException:
                binary.kill()
                raise
        if binary.returncode:
            raise subprocess.CalledProcessError(binary.returncode, binary.args)
    return comparison.report()


class HarnessTests(unittest.TestCase):
    def test_numeric(self):
        self.assertEqual(encoded("pedal-assignment", 1), [64])
        self.assertEqual(encoded("inverted-switch", 1), [0])
        self.assertEqual(encoded("constant-127", float_bits(0)), [127])
        self.assertEqual(encoded("tempo-tenths", float_bits(120.5)), split14(1205))
        self.assertEqual(encoded("gate-db", float_bits(-96)), [96])
        self.assertEqual(encoded("gate-db-ranged", float_bits(-96)), [96])

    def test_14bit_rounding(self):
        bits = float_bits(123.5 / 16383)
        self.assertEqual(encoded("float14-nearest", bits), split14(124))
        self.assertEqual(encoded("float14-truncate", bits), split14(123))

    def test_name_and_address(self):
        self.assertEqual(name_bytes("a\x00`{Āé"), b"a __ _")
        self.assertEqual(stored_prefix(63), "sysex:110:710a003f")
        self.assertEqual(stored_prefix(64), "sysex:110:710a0140")

    def test_inventory_coverage(self):
        table = json.loads(TABLE.read_text())
        rows = table["parameters"]
        self.assertEqual(Counter(b for row in rows for b in row["tone_offsets"]),
                         Counter(range(160)))
        self.assertEqual(len({row["key"] for row in rows}), len(rows))
        for row in rows:
            self.assertTrue(row["sources"])
            self.assertIn(row["evidence"], {"static fact", "inference", "unknown"})
        self.assertEqual({case.key for case in cases(table) if case.request[0] == "P"},
                         {row["key"] for row in rows if row["selector"] is not None})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--classes", type=Path, help="Local Line 6 Edit 3.06 extracted class root")
    parser.add_argument("--jdk-bin", type=Path, help="Directory containing java and javac (JDK 17+)")
    parser.add_argument("--self-test", action="store_true", help="Test predictions without vendor classes")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(HarnessTests)
        return 0 if unittest.TextTestRunner(verbosity=2).run(suite).wasSuccessful() else 1
    if args.classes is None or args.jdk_bin is None:
        parser.error("--classes and --jdk-bin are required for oracle verification")
    classes = args.classes.resolve()
    jdk = args.jdk_bin.resolve()
    if not (classes / "modules/sharctone/device/L6SharcToneMIDITranscoder.class").is_file():
        parser.error("--classes must point to the extracted class root")
    report = verify(classes, jdk)
    print(json.dumps(report, indent=2))
    return 1 if report["findings"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
