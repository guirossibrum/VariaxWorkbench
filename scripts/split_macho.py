"""Extract validated PowerPC/i386 slices from the recovered universal libraries."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]


def slices(data):
    if data[:4] != bytes.fromhex("cafebabe") or len(data) < 8:
        return []
    count = struct.unpack_from(">I", data, 4)[0]
    # Java classes share the FAT_MAGIC bytes; validate the full table and slices.
    if not 1 <= count <= 16 or 8 + count * 20 > len(data):
        return []
    records = []
    for i in range(count):
        cpu, subtype, offset, size, align = struct.unpack_from(">IIIII", data, 8 + 20 * i)
        if cpu not in (7, 18) or offset < 8 + count * 20 or size < 28:
            return []
        if offset + size > len(data) or data[offset:offset + 4] not in (
                bytes.fromhex("cefaedfe"), bytes.fromhex("feedface")):
            return []
        records.append((cpu, offset, size))
    return records


def main():
    output = ROOT / "analysis/extracted/macho-slices"
    output.mkdir(exist_ok=True)
    records = []
    for source in (ROOT / "analysis/extracted/mac-jar").rglob("*.mac"):
        data = source.read_bytes()
        for cpu, offset, size in slices(data):
            arch = {7: "i386", 18: "ppc"}[cpu]
            target = output / f"{source.stem}.{arch}.macho"
            payload = data[offset:offset + size]
            target.write_bytes(payload)
            records.append({"source": str(source.relative_to(ROOT)),
                            "architecture": arch, "offset": offset, "size": size,
                            "output": str(target.relative_to(ROOT)),
                            "sha256": hashlib.sha256(payload).hexdigest()})
    (ROOT / "analysis/reports/macho-slices.json").write_text(json.dumps(records, indent=2) + "\n")
    print(f"Extracted {len(records)} validated native slices")


if __name__ == "__main__":
    main()
