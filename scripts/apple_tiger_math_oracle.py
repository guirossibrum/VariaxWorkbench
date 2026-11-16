"""Bounded i386 math calls from a pinned Apple 10.4.11 runtime, research only.

The original Mach-O is mapped into private Unicorn memory. No native library
load, dynamic loader, initializer, installer or OS/device service is supplied.
Unknown imports, interrupts, unmapped accesses and limits stop execution.
"""
import hashlib
from pathlib import Path
import struct

from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UC_HOOK_MEM_INVALID, UC_HOOK_INTR
from unicorn.x86_const import UC_X86_REG_EIP, UC_X86_REG_ESP, UC_X86_REG_FPCW, UC_X86_REG_FPTAG, UC_X86_REG_MXCSR

from math_backends import AdapterFunction, FUNCTIONS


class AppleTigerMath:
    SHA256 = '36f2a3fbaa04d2ddc7a7ad600c083c637ff28fcae5aa86252509742b16325358'
    STACK, AFTER, RETURN, OUTPUT = 0x21000000, 0x20000100, 0x22000000, 0x20001000
    PURE_IMPORTS = {'___sinl': '_sinl', '___cosl': '_cosl', '___tanl': '_tanl', '_atan2l': '_atan2l'}

    def __init__(self, path, control=0x37f):
        if control not in (0x37f, 0x27f):
            raise ValueError('unsupported research x87 control scenario')
        source = Path(path)
        if not source.is_file() or source.stat().st_size != 1_886_600:
            raise ValueError('requires the regular pinned 1886600-byte runtime slice')
        data = source.read_bytes()
        if hashlib.sha256(data).hexdigest() != self.SHA256:
            raise ValueError('requires the pinned original i386 libSystem 88.3.9 slice')
        if data[:4] != bytes.fromhex('cefaedfe') or struct.unpack_from('<I', data, 4)[0] != 7:
            raise ValueError('requires a 32-bit i386 Mach-O')
        self.control = control
        self.mxcsr = 0x1f80
        self.environment_counts = {}
        self.value_cache = {}
        self.cache_hits = 0
        self.cpu = Uc(UC_ARCH_X86, UC_MODE_32)
        self.stubs = {}
        self.import_calls = {}
        self.segments = []
        sections = []
        offset = 28
        for _ in range(struct.unpack_from('<I', data, 16)[0]):
            cmd, size = struct.unpack_from('<II', data, offset)
            if size < 8 or offset + size > len(data):
                raise ValueError('invalid load command')
            if cmd == 1:
                vm, length, fileoff, filesize, _, prot, count = struct.unpack_from('<7I', data, offset + 24)
                if fileoff + filesize > len(data) or filesize > length:
                    raise ValueError('invalid segment')
                if length:
                    self.cpu.mem_map(vm, (length + 4095) & ~4095)
                    self.cpu.mem_write(vm, data[fileoff:fileoff + filesize])
                    self.segments.append((vm, vm + length, bool(prot & 4)))
                for i in range(count):
                    at = offset + 56 + i * 68
                    if at + 68 > offset + size:
                        raise ValueError('section outside segment command')
                    address, span = struct.unpack_from('<II', data, at + 32)
                    flags, reserved1, reserved2 = struct.unpack_from('<III', data, at + 56)
                    sections.append((address, span, flags & 255, reserved1, reserved2))
            elif cmd == 2:
                symoff, nsymbols, stroff, strsize = struct.unpack_from('<4I', data, offset + 8)
            elif cmd == 11:
                indirectoff, nindirect = struct.unpack_from('<II', data, offset + 56)
            offset += size
        if symoff + nsymbols * 12 > len(data) or stroff + strsize > len(data):
            raise ValueError('symbol table outside file')
        self.symbols, names = {}, []
        for i in range(nsymbols):
            string, kind, section, desc, value = struct.unpack_from('<IBBHI', data, symoff + i * 12)
            if string >= strsize:
                raise ValueError('invalid symbol string')
            name = data[stroff + string:stroff + strsize].split(b'\0')[0].decode('ascii')
            names.append(name)
            if kind & 0xe == 0xe:
                self.symbols[name] = value
        if indirectoff + nindirect * 4 > len(data):
            raise ValueError('indirect table outside file')
        for address, span, kind, first, stride in sections:
            if kind != 8:  # S_SYMBOL_STUBS
                continue
            if not stride or span % stride or first + span // stride > nindirect:
                raise ValueError('invalid symbol stub range')
            for i in range(span // stride):
                index = struct.unpack_from('<I', data, indirectoff + (first + i) * 4)[0]
                if index >= len(names):
                    raise ValueError('invalid indirect symbol')
                self.stubs[address + i * stride] = names[index]
            self.cpu.hook_add(UC_HOOK_CODE, self._import, begin=address, end=address + span - 1)
        for name in FUNCTIONS:
            if '_' + name not in self.symbols:
                raise ValueError('missing named math export: ' + name)
        self.cpu.mem_map(self.STACK, 0x10000)
        self.cpu.mem_map(self.AFTER & ~4095, 0x2000)
        self.cpu.mem_map(self.RETURN, 0x1000)
        self.cpu.hook_add(UC_HOOK_MEM_INVALID, self._invalid)
        self.cpu.hook_add(UC_HOOK_INTR, self._interrupt)
        self.methods = {}
        for name in FUNCTIONS:
            self.methods[name] = AdapterFunction(lambda *values, name=name: self.evaluate(name, *values))

    def _import(self, cpu, address, size, user):
        name = self.stubs.get(address)
        if name not in self.PURE_IMPORTS:
            raise RuntimeError(f'Apple runtime import is unsupported: {name or hex(address)}')
        target = self.symbols[self.PURE_IMPORTS[name]]
        if not any(start <= target < end and executable for start, end, executable in self.segments):
            raise RuntimeError('pure math import target is not executable')
        self.import_calls[name] = self.import_calls.get(name, 0) + 1
        cpu.reg_write(UC_X86_REG_EIP, target)

    def _invalid(self, cpu, access, address, size, value, user):
        raise RuntimeError(f'unmapped Apple math access {address:#x} at {cpu.reg_read(UC_X86_REG_EIP):#x}')

    def _interrupt(self, cpu, number, user):
        raise RuntimeError(f'guest interrupt {number} is forbidden; no OS forwarding')

    def __getattr__(self, name):
        if name in self.methods:
            return self.methods[name]
        raise AttributeError(name)

    def set_environment(self, control, mxcsr):
        if control not in (0x37f, 0x27f) or mxcsr & ~0x3f != 0x1f80:
            raise ValueError('unmodeled caller floating-point environment')
        self.control, self.mxcsr = control, mxcsr
        key = f'{control:#x}/{mxcsr:#x}'
        self.environment_counts[key] = self.environment_counts.get(key, 0) + 1

    def evaluate(self, name, *values):
        if name not in FUNCTIONS:
            raise ValueError('unsupported math export')
        _, arity, single = FUNCTIONS[name]
        if len(values) != arity:
            raise ValueError('incorrect math argument count')
        fmt = 'f' if single else 'd'
        args = struct.pack('<' + fmt * arity, *values)
        key = (name, args, self.control, self.mxcsr)
        if key in self.value_cache:
            self.cache_hits += 1
            return self.value_cache[key]
        if len(self.value_cache) >= 500_000:
            raise RuntimeError('private math value-cache bound exceeded')

        stack = self.STACK + 0xeffc  # (entry ESP + return address) is 16-byte aligned
        self.cpu.mem_write(self.STACK, bytes(0x10000))
        self.cpu.mem_write(stack, struct.pack('<I', self.AFTER) + args)
        code = (b'\xd9\x1d' if single else b'\xdd\x1d') + struct.pack('<I', self.OUTPUT)
        code += b'\xe9' + struct.pack('<i', self.RETURN - (self.AFTER + len(code) + 5))
        self.cpu.mem_write(self.AFTER, code)
        self.cpu.ctl_remove_cache(self.AFTER, self.AFTER + len(code))
        self.cpu.reg_write(UC_X86_REG_ESP, stack)
        self.cpu.reg_write(UC_X86_REG_FPCW, self.control)
        self.cpu.reg_write(UC_X86_REG_FPTAG, 0xffff)
        self.cpu.reg_write(UC_X86_REG_MXCSR, self.mxcsr)
        self.cpu.emu_start(self.symbols['_' + name], self.RETURN, count=1_000_000, timeout=2_000_000)
        if self.cpu.reg_read(UC_X86_REG_EIP) != self.RETURN:
            raise RuntimeError('bounded Apple math call did not return')
        result = struct.unpack('<' + fmt, bytes(self.cpu.mem_read(self.OUTPUT, 4 if single else 8)))[0]
        self.value_cache[key] = result
        return result


class AppleTigerHostX87Math(AppleTigerMath):
    """Alternate research model using current-host x87 for traced trig wrappers."""
    def __init__(self, library):
        super().__init__(library)
        from native_x87_reference import NativeX87
        self.native_x87 = NativeX87()
        self.native_x87_calls = 0

    def evaluate(self, name, *values):
        if name in ('sin','sinf','cos','cosf','tan','tanf'):
            if len(values) != 1 or self.control != 0x37f:
                raise ValueError('unmodeled native-x87 argument/control scenario')
            # These pinned Apple wrappers load the input, issue FSIN/FCOS/FPTAN,
            # then store the declared return width. The helper rejects the
            # argument-reduction fallback region rather than inventing it.
            self.native_x87_calls += 1
            return self.native_x87.evaluate(name, values[0])
        return super().evaluate(name, *values)


from coefficient_oracle import CoefficientOracle


class AppleTigerCoefficientOracle(CoefficientOracle):
    """Separate candidate namespace; no reference/default rollout is implied."""
    CANDIDATE_RUNTIME_VERSION = 'initialized-apple-tiger8839-v1'
    MATH_BACKEND = AppleTigerMath
    # Independently checked original pow(10,-3) and pow(10,-2), one ULP above
    # the ordinary reference constants. Never derive/accept arbitrary startup.
    EXPECTED_KNOB_CONSTANTS = (0.0010000000000000002, 0.010000000000000002)

    def __init__(self, library):
        backend = self.MATH_BACKEND(library)
        if (backend.pow(10., -3.), backend.pow(10., -2.)) != self.EXPECTED_KNOB_CONSTANTS:
            raise RuntimeError('pinned historical runtime constant contract changed')
        super().__init__(math_backend=backend, runtime_version=self.CANDIDATE_RUNTIME_VERSION)

    def _dispatch(self, cpu, address, size, context):
        name = self.adapters.get(address, '').lstrip('_')
        if name in FUNCTIONS:
            self.libm.set_environment(cpu.reg_read(UC_X86_REG_FPCW), cpu.reg_read(UC_X86_REG_MXCSR))
        super()._dispatch(cpu, address, size, context)


class AppleTigerHostX87CoefficientOracle(AppleTigerCoefficientOracle):
    CANDIDATE_RUNTIME_VERSION = 'initialized-apple-tiger8839-current-host-x87-v1'
    MATH_BACKEND = AppleTigerHostX87Math


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('library', type=Path)
    args = parser.parse_args()
    oracle = AppleTigerMath(args.library)
    for name in FUNCTIONS:
        values = (2., 3.) if name.startswith('pow') else ((1., 2.) if name == 'atan2f' else (0.5,))
        try:
            print(name, oracle.evaluate(name, *values), flush=True)
        except Exception as error:
            print(name, 'STOP:', error, flush=True)
