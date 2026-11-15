"""Use original native serialization/model code with a byte-only memory stream.

Only ReadByte is supplied by Python. All numeric endian conversion, field order,
array handling and object writes are performed by original library instructions.
"""
import json
from pathlib import Path
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX
from native_oracle import NativeOracle, ROOT


def native_read(payload):
    oracle = NativeOracle()
    stream = oracle.OUTPUT + 0x1000
    vtable = oracle.OUTPUT + 0x1100
    persistence = oracle.OUTPUT + 0x2000
    read_byte = oracle.OUTPUT + 0xf000
    cursor = 0

    def supply_byte(cpu, address, size, context):
        nonlocal cursor
        if cursor >= len(payload):
            raise ValueError("native parser read beyond the supplied byte stream")
        cpu.reg_write(UC_X86_REG_EAX, payload[cursor])
        cursor += 1

    table = [0] * 19
    table[0] = read_byte
    table[3] = 0xc1a0  # Original ReadBytes, repeatedly calls ReadByte.
    for offset, address in [(14, 0xc3e0), (15, 0xc430), (16, 0xc540), (17, 0xc570), (18, 0xc5a0)]:
        table[offset] = address
    oracle.write(vtable, struct.pack("<19I", *table))
    oracle.write(stream, struct.pack("<I", vtable))
    oracle.write(read_byte, b"\xc3")
    oracle.cpu.hook_add(UC_HOOK_CODE, supply_byte, begin=read_byte, end=read_byte)
    oracle.call("0xcb60", persistence, stream, 0)  # SerialPersistance constructor.
    oracle.call("0x9ea0", oracle.OBJECT)  # Pickup-model constructor.
    oracle.call("L6VWPickupModel::ReadWrite", oracle.OBJECT, persistence)
    assert cursor == len(payload)
    electrical = struct.unpack("<4d", oracle.read(oracle.OBJECT + 4, 32))
    volume, = struct.unpack("<f", oracle.read(oracle.OBJECT + 0x24, 4))
    pole = struct.unpack("<6f", oracle.read(oracle.OBJECT + 0x28, 24))
    humbucker = oracle.read(oracle.OBJECT + 0x40, 1)[0] != 0
    coils = struct.unpack("<6f", oracle.read(oracle.OBJECT + 0x44, 24))
    # Canonical no-padding memory result, preserving exact IEEE bit patterns.
    return struct.pack(">4dBf6f6f", *electrical, humbucker, volume, *pole, *coils)


def main():
    paths = sorted((ROOT / "analysis/extracted/windows-app/modules/variax/device/coefeng/data/pickups").glob("*.bin"))
    records = []
    for path in paths:
        payload = path.read_bytes()
        canonical = native_read(payload)
        assert canonical == payload, path
        records.append({"file": path.name, "bytes": len(payload), "status": "native parser matches big-endian field layout"})
    synthetic = struct.pack(">4dBf6f6f", 1234.5, 6789.25, 2.125, 0.000000345,
                            1, -3.5, *[i * 0.125 for i in range(6)], *[-i * 0.25 for i in range(6)])
    canonical = native_read(synthetic)
    assert canonical == synthetic
    (ROOT / "tests/fixtures/native-pickup-definition.bin").write_bytes(synthetic)
    report = {"scope": "Original model/serialization instructions with synthetic ReadByte adapter; no OS/device access",
              "model_function": "0x9f00", "source_sha256": NativeOracle().sha256,
              "files_checked": len(records), "records": records,
              "synthetic_fixture": "tests/fixtures/native-pickup-definition.bin"}
    (ROOT / "analysis/reports/native-pickup-serialization.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"PASS: original native parser confirms {len(records)} pickup files and synthetic fixture")


if __name__ == "__main__":
    main()
