"""Small i386 oracle for bounded calls into the recovered coefficient library.

Original code runs only inside Unicorn private memory. No OS/device services are
implemented. Unresolved external calls fail; callers must supply valid synthetic
objects. Artifact identity and procedure lookup come from REA's saved index.
"""
import hashlib
import json
from pathlib import Path
import struct

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EIP

ROOT = Path(__file__).resolve().parents[1]


class NativeOracle:
    OBJECT = 0x1000000
    OUTPUT = 0x1010000
    STACK = 0x1100000
    RETURN = 0x1200000

    def __init__(self):
        binary = (ROOT / "analysis/extracted/macho-slices/L6VariaxCoefEngine.jnilib.i386.macho").read_bytes()
        self.index = json.loads((ROOT / "analysis/decompiled/native-coef/decompile-index.json").read_text())
        self.sha256 = hashlib.sha256(binary).hexdigest()
        assert self.sha256 == self.index["target_sha256"]
        assert binary[:4] == bytes.fromhex("cefaedfe")
        commands = struct.unpack_from("<I", binary, 16)[0]
        cursor, segments = 28, []
        for _ in range(commands):
            command, size = struct.unpack_from("<II", binary, cursor)
            assert size >= 8 and cursor + size <= len(binary)
            if command == 1:
                vmaddr, vmsize, fileoff, filesize = struct.unpack_from("<IIII", binary, cursor + 24)
                assert fileoff + filesize <= len(binary) and filesize <= vmsize
                segments.append((vmaddr, vmsize, fileoff, filesize))
            cursor += size
        self.cpu = Uc(UC_ARCH_X86, UC_MODE_32)
        image_end = max(address + size for address, size, _, _ in segments)
        self.cpu.mem_map(0, (image_end + 4095) & ~4095)
        for address, _, offset, size in segments:
            if size:
                self.cpu.mem_write(address, binary[offset:offset+size])
        for address in (self.OBJECT, self.OUTPUT, self.STACK, self.RETURN):
            self.cpu.mem_map(address, 0x10000)
        self.cpu.mem_write(self.STACK, bytes(0x10000))

    def call(self, name, *arguments, instruction_limit=1_000_000, timeout_us=2_000_000):
        assert 0 < instruction_limit <= 20_000_000
        assert 0 < timeout_us <= 10_000_000
        matches = [f for f in self.index["functions"] if
                   (f["name"] == name or f["address"] == name) and f["status"] == "written"]
        assert len(matches) == 1, (name, len(matches))
        address = int(matches[0]["address"], 16)
        stack = self.STACK + 0xf000
        self.cpu.mem_write(stack, struct.pack("<" + "I"*(len(arguments)+1), self.RETURN, *arguments))
        self.cpu.reg_write(UC_X86_REG_ESP, stack)
        self.cpu.emu_start(address, self.RETURN, timeout=timeout_us, count=instruction_limit)
        assert self.cpu.reg_read(UC_X86_REG_EIP) == self.RETURN, f"{name} did not return"
        return address

    def write(self, address, data):
        self.cpu.mem_write(address, data)

    def read(self, address, size):
        return bytes(self.cpu.mem_read(address, size))


def float_bits(value):
    return struct.unpack("<I", struct.pack("<f", value))[0]
