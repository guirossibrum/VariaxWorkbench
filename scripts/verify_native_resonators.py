"""Capture native resonator coefficient conversion on synthetic finite inputs."""
import json
import struct
from native_oracle import NativeOracle, ROOT, float_bits


def main():
    oracle = NativeOracle()
    values = [-8.0, -2.0001, -2.0, -1.5, -1.0, -0.5, -1e-7, -0.0,
              0.0, 1e-7, 0.5, 1.0, 1.5, 2.0, 2.0001, 8.0]
    records = []
    for i, value in enumerate(values):
        args = [value, values[(i+5) % len(values)], values[(i+11) % len(values)]]
        index = i % 12
        oracle.write(oracle.OBJECT, bytes(4368))
        address = oracle.call("L6VariaxHWPatch::SetBodyResonatorCoefs", oracle.OBJECT,
                              index, *(float_bits(v) for v in args))
        actual = struct.unpack("<III", oracle.read(oracle.OBJECT + 0x294 + index*12, 12))
        records.append({"index": index, "input_bits": [float_bits(v) for v in args],
                        "input_values": args, "output_words": list(actual)})
    fixture = ROOT / "tests/fixtures/native-resonator-vectors.txt"
    fixture.write_text("\n".join(" ".join(f"{v:08x}" for v in [*r['input_bits'], *r['output_words']]) for r in records) + "\n")
    report = {"source_sha256": oracle.sha256, "function": "L6VariaxHWPatch::SetBodyResonatorCoefs",
              "address": hex(address), "vectors": records, "scope": "CPU emulation on finite synthetic coefficients; no device or OS access"}
    (ROOT / "analysis/reports/native-resonator-emulation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
