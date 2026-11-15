"""Run the original i386 Pack routine in Unicorn against synthetic input only.

Maps only its recovered machine-code bytes and private data/stack memory. No
system calls, OS framework stubs or device access are provided. Address and
function extent come from REA's decompile index, bound to the artifact digest.
"""
import hashlib
import json
from pathlib import Path
import struct

import unicorn
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EIP

ROOT = Path(__file__).resolve().parents[1]


def main():
    path = ROOT / "analysis/extracted/macho-slices/L6VariaxCoefEngine.jnilib.i386.macho"
    binary = path.read_bytes()
    index = json.loads((ROOT / "analysis/decompiled/native-coef/decompile-index.json").read_text())
    digest = hashlib.sha256(binary).hexdigest()
    assert digest == index["target_sha256"]
    function = next(f for f in index["functions"] if f["name"] == "L6VariaxHWPatch::Pack")
    ranges = function["body"]["ranges"]
    assert len(ranges) == 1
    begin, end = (int(ranges[0][key], 16) for key in ("start", "end"))
    # Resolve the function address through LC_SEGMENT rather than assuming that
    # file offsets equal virtual addresses.
    ncmds = struct.unpack_from("<I", binary, 16)[0]
    cursor = 28
    code = None
    for _ in range(ncmds):
        command, size = struct.unpack_from("<II", binary, cursor)
        if command == 1:
            vmaddr, vmsize, fileoff, filesize = struct.unpack_from("<IIII", binary, cursor + 24)
            if vmaddr <= begin <= end < vmaddr + filesize:
                offset = fileoff + begin - vmaddr
                code = binary[offset:offset + end - begin + 1]
        cursor += size
    assert code is not None
    words = [0, 1, 0xffffff, 0xffffffff, 0x12345678]
    seed = 0x600175
    while len(words) < 1091:
        seed = (1664525 * seed + 1013904223) & 0xffffffff
        words.append(seed)
    data = struct.pack("<1091I", *words)
    object_address, output_address, stack_address, sentinel = 0x100000, 0x110000, 0x120000, 0x130000
    emulator = Uc(UC_ARCH_X86, UC_MODE_32)
    code_page = begin & ~4095
    emulator.mem_map(code_page, ((end - code_page + 4096) // 4096) * 4096)
    emulator.mem_write(begin, code)
    for address in (object_address, output_address, stack_address, sentinel):
        emulator.mem_map(address, 8192)
    emulator.mem_write(object_address, data + b"\xa5" * 4)
    stack = stack_address + 4096
    emulator.mem_write(stack, struct.pack("<III", sentinel, object_address, output_address))
    emulator.reg_write(UC_X86_REG_ESP, stack)
    emulator.emu_start(begin, sentinel, timeout=1_000_000, count=100_000)
    assert emulator.reg_read(UC_X86_REG_EIP) == sentinel, "routine did not return"
    output = bytes(emulator.mem_read(output_address, 3276))
    assert bytes(emulator.mem_read(object_address, len(data))) == data
    fixtures = ROOT / "tests/fixtures"
    (fixtures / "native-pack-input.bin").write_bytes(data)
    (fixtures / "native-pack-output.bin").write_bytes(output)
    report = {"source": str(path.relative_to(ROOT)), "sha256": digest,
              "function": function["name"], "address": hex(begin),
              "code_sha256": hashlib.sha256(code).hexdigest(),
              "unicorn_version": unicorn.__version__, "input_words": len(words),
              "output_bytes": len(output), "input_sha256": hashlib.sha256(data).hexdigest(),
              "output_sha256": hashlib.sha256(output).hexdigest(),
              "checksum_hex": output[-3:].hex(),
              "scope": "Original machine code emulated on synthetic words; no OS or hardware access"}
    (ROOT / "analysis/reports/native-pack-emulation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
