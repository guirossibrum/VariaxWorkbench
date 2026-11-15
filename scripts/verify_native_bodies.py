"""Verify body-definition serialization using original i386 reader instructions.

The only input adapter supplies bytes from a local buffer. StringX::Format is
replaced with an immediate return inside emulator memory because binary
SerialPersistance ignores field/group labels. Numeric readers, array loops,
object field order, constructors and model writes execute original machine code.
No device, operating-system service, application UI or real process is opened.
"""
import hashlib
import json
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP
from native_oracle import NativeOracle, ROOT

FIELD_METHODS = {
    0xcbf0: ("int", 4, False), 0xcc40: ("float", 4, False),
    0xcc90: ("double", 8, False), 0xcce0: ("bool", 1, False),
    0xcd30: ("int", 4, True), 0xcde0: ("float", 4, True),
    0xce90: ("double", 8, True), 0xcf40: ("bool", 1, True),
}
BODY_BYTES = 17483
NATIVE_BYTES = 0x4bb4


def native_read(payload):
    oracle = NativeOracle()
    stream, table_address = oracle.OUTPUT + 0x1000, oracle.OUTPUT + 0x1100
    persistence, read_byte = oracle.OUTPUT + 0x2000, oracle.OUTPUT + 0xf000
    cursor, fields = 0, []

    def supply_byte(cpu, address, size, context):
        nonlocal cursor
        if cursor >= len(payload):
            raise ValueError(f"native body reader exceeds input at byte {cursor}")
        cpu.reg_write(UC_X86_REG_EAX, payload[cursor])
        cursor += 1

    def record_field(cpu, address, size, context):
        kind, width, array = FIELD_METHODS[address]
        stack = cpu.reg_read(UC_X86_REG_ESP)
        _, this, label, destination, count = struct.unpack("<5I", oracle.read(stack, 20))
        assert this == persistence
        count = count if array else 1
        offset = destination - oracle.OBJECT
        assert 0 <= offset < NATIVE_BYTES and offset + width*count <= NATIVE_BYTES
        label_bytes = bytearray()
        for i in range(128):
            byte = oracle.read(label+i, 1)[0]
            if byte == 0:
                break
            label_bytes.append(byte)
        label_text = label_bytes.decode("ascii", errors="replace")
        if not label_text.isprintable():
            label_text = "dynamic label (ignored by binary reader)"
        fields.append({"name": label_text, "type": kind, "width": width,
                       "count": count, "file_offset": cursor, "object_offset": offset})

    table = [0] * 19
    table[0], table[3] = read_byte, 0xc1a0
    for slot, address in [(14, 0xc3e0), (15, 0xc430), (16, 0xc540), (17, 0xc570), (18, 0xc5a0)]:
        table[slot] = address
    oracle.write(table_address, struct.pack("<19I", *table))
    oracle.write(stream, struct.pack("<I", table_address))
    oracle.write(read_byte, b"\xc3")
    oracle.cpu.hook_add(UC_HOOK_CODE, supply_byte, begin=read_byte, end=read_byte)
    for address in FIELD_METHODS:
        oracle.cpu.hook_add(UC_HOOK_CODE, record_field, begin=address, end=address)
    # The serial backend never examines the labels; the native string's empty
    # representation remains untouched so its original destructor still runs.
    oracle.write(0xd500, b"\xc3")
    oracle.call("0xcb60", persistence, stream, 0)
    oracle.call("0x9750", oracle.OBJECT)
    oracle.call("L6VWBodyModel::ReadWrite", oracle.OBJECT, persistence)
    assert cursor == len(payload) == BODY_BYTES
    canonical, expected_offset = bytearray(), 0
    for field in fields:
        assert field["file_offset"] == expected_offset, (field, expected_offset)
        width, count = field["width"], field["count"]
        raw = oracle.read(oracle.OBJECT + field["object_offset"], width * count)
        for i in range(count):
            canonical.extend(raw[i*width:(i+1)*width][::-1])
        expected_offset += width * count
    assert expected_offset == BODY_BYTES
    return bytes(canonical), fields


def synthetic_body(fields):
    output = bytearray()
    for i, field in enumerate(fields):
        for j in range(field["count"]):
            if field["type"] == "bool":
                output.append(1 if field["file_offset"] == 0 else (i+j) % 2)
            elif field["type"] == "int":
                output.extend(struct.pack(">I", 4 if field["file_offset"] == 97 else i*17+j))
            elif field["type"] == "float":
                output.extend(struct.pack(">f", (i+1)*0.125 + j*0.03125))
            else:
                output.extend(struct.pack(">d", (i+1)*0.125 + j*0.03125))
    assert len(output) == BODY_BYTES
    return bytes(output)


def main():
    directory = ROOT / "analysis/extracted/windows-app/modules/variax/device/coefeng/data/bodies"
    records, schema = [], None
    for path in sorted(directory.glob("*.bin")):
        payload = path.read_bytes()
        decoded, fields = native_read(payload)
        assert decoded == payload, path
        if schema is None:
            schema = fields
        assert fields == schema, path
        records.append({"file": path.name, "bytes": len(payload),
                        "sha256": hashlib.sha256(payload).hexdigest(),
                        "status": "all original native-reader fields match big-endian serialization"})
    fixture = synthetic_body(schema)
    actual, fields = native_read(fixture)
    assert actual == fixture and fields == schema
    (ROOT / "tests/fixtures/native-body-definition.bin").write_bytes(fixture)
    (ROOT / "analysis/reports/native-body-schema.json").write_text(json.dumps(schema, indent=2) + "\n")
    report = {"source_sha256": NativeOracle().sha256,
              "scope": "Original i386 model/numeric/array serialization code; memory ReadByte adapter and ignored-format-label adapter only; no hardware",
              "adapters": [{"address": "0xd500", "function": "StringX::Format", "replacement": "ret in emulator memory; binary labels are ignored"}],
              "reader": "L6VWBodyModel::ReadWrite at 0x99c0",
              "files_checked": len(records), "field_calls": len(schema),
              "body_bytes": BODY_BYTES, "header_bytes": 101,
              "variant_count": 6, "variant_bytes": 2897,
              "records": records,
              "synthetic_fixture_sha256": hashlib.sha256(fixture).hexdigest()}
    (ROOT / "analysis/reports/native-body-serialization.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"PASS: original native body reader confirms {len(records)} full body files, {len(schema)} field calls, and synthetic fixture")


if __name__ == "__main__":
    main()
