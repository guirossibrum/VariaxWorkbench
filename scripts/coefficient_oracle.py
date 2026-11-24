"""Offline host-library oracle with explicit memory/libm adapters.

Original i386 code executes in Unicorn. Imports are resolved only to shipped
symbols or bounded in-memory allocation/copy and host libm adapters. Unknown
imports fail closed. No file, network, USB, JNI or device services are supplied.
Host libm is an approximation of the original macOS runtime, so complete outputs
remain reference candidates until rounding is cross-validated.
"""
import collections
import ctypes
import ctypes.util
import struct
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_INVALID
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ESP, UC_X86_REG_EIP, UC_X86_REG_FPCW, UC_X86_REG_FPTAG, UC_X86_REG_MXCSR
from native_oracle import NativeOracle, ROOT


class CoefficientOracle(NativeOracle):
    RUNTIME_VERSION = 'initialized-host-libm-v2'
    # A research subclass may specify independently established constants for
    # a pinned original runtime. The existing reference contract stays exact.
    EXPECTED_KNOB_CONSTANTS = (0.001, 0.01)
    HEAP = 0x2000000
    HEAP_SIZE = 0x1000000
    SHIMS = 0x1300000

    def __init__(self, *, math_backend=None, runtime_version=None, artifact_root=ROOT):
        if math_backend is None and runtime_version is not None:
            raise ValueError('a distinct runtime version requires an explicit math backend')
        if math_backend is not None:
            if not runtime_version or runtime_version == self.RUNTIME_VERSION:
                raise ValueError('alternate math requires a distinct reference namespace')
            self.RUNTIME_VERSION = runtime_version
        super().__init__(artifact_root=artifact_root)
        self.cpu.mem_map(self.HEAP, self.HEAP_SIZE)
        self.cpu.mem_map(self.SHIMS, 0x10000)
        self.heap_cursor = self.HEAP
        self.allocations = {}
        self.import_calls = collections.Counter()
        self.diagnostics = []
        self.adapters = {}
        self.last_pc = None
        self.destructor_registrations = []
        self.initializers_completed = []
        self.runtime_initialized = False
        self.cpu.reg_write(UC_X86_REG_FPCW, 0x37f)
        self.cpu.reg_write(UC_X86_REG_FPTAG, 0xffff)
        self.cpu.reg_write(UC_X86_REG_MXCSR, 0x1f80)
        self.libm = math_backend if math_backend is not None else ctypes.CDLL(ctypes.util.find_library('m'))
        self._resolve_imports()
        self.cpu.hook_add(UC_HOOK_CODE, self._dispatch,
                          begin=self.SHIMS, end=self.SHIMS + 0xffff)
        self.cpu.hook_add(UC_HOOK_CODE, self._dispatch, begin=0, end=1)
        self.cpu.hook_add(UC_HOOK_MEM_INVALID, self._bad_memory)
        self._initialize_modules()

    def _initialize_modules(self):
        # This local compatibility wrapper would otherwise register callbacks
        # with the original process runtime. Capture only; never use host atexit.
        assert self.sha256 == '3aa232329d467101637764397b77a7b8ffa6af0ccd72ec380d02f145477ba9fe'
        assert len(self.module_initializers) == 26
        assert self.read(0x1759, 6) == bytes.fromhex('5589e583ec28')
        shim = self.SHIMS + 0xd000
        self.adapters[shim] = 'oracle_capture_cxa_atexit'
        self.write(shim, b'\xc3')
        self.write(0x1759, b'\xe9' + struct.pack('<i', shim - 0x1759 - 5))
        for address in self.module_initializers:
            self.call_initializer(address)
            self.initializers_completed.append(address)
        tone, volume = struct.unpack('<2d', self.read(0x3d798, 16))
        if (tone, volume) != self.EXPECTED_KNOB_CONSTANTS:
            raise RuntimeError(f'unexpected initialized knob constants: {(tone, volume)}')
        if len(self.destructor_registrations) != 53:
            raise RuntimeError('incomplete module destructor registrations')
        self.runtime_initialized = True

    def _bad_memory(self, cpu, access, address, size, value, context):
        raise RuntimeError(f'Unmapped memory access {access} at {address:#x}, size {size}, pc={cpu.reg_read(UC_X86_REG_EIP):#x}')

    def allocate(self, size):
        if not 0 <= size <= self.HEAP_SIZE:
            raise ValueError(f'invalid private allocation: {size}')
        address = self.heap_cursor
        self.heap_cursor += (max(size, 1) + 15) & ~15
        if self.heap_cursor > self.HEAP + self.HEAP_SIZE:
            raise MemoryError('private oracle heap exhausted')
        self.allocations[address] = size
        return address

    def cstring(self, address, limit=65536):
        value = bytearray()
        for offset in range(limit):
            byte = self.read(address + offset, 1)[0]
            if byte == 0:
                return bytes(value)
            value.append(byte)
        raise ValueError('unterminated private string')

    def _resolve_imports(self):
        data = (self.artifact_root / 'analysis/extracted/macho-slices/L6VariaxCoefEngine.jnilib.i386.macho').read_bytes()
        cursor, sections, sym, indirect = 28, [], None, None
        for _ in range(struct.unpack_from('<I', data, 16)[0]):
            command, size = struct.unpack_from('<II', data, cursor)
            if command == 2:
                sym = struct.unpack_from('<4I', data, cursor + 8)
            if command == 11:
                indirect = struct.unpack_from('<2I', data, cursor + 56)
            if command == 1:
                for i in range(struct.unpack_from('<I', data, cursor + 48)[0]):
                    offset = cursor + 56 + i * 68
                    address, length = struct.unpack_from('<II', data, offset + 32)
                    flags, first, stride = struct.unpack_from('<III', data, offset + 56)
                    if flags & 255 in (6, 7, 8):
                        sections.append((address, length, flags & 255, first, stride))
            cursor += size
        assert sym is not None and indirect is not None
        symoff, count, stroff, strlen = sym
        def symbol(index):
            assert index < count
            string_index, typ, section, description, value = struct.unpack_from('<IBBHI', data, symoff + index * 12)
            end = data.index(0, stroff + string_index, stroff + strlen)
            return data[stroff + string_index:end].decode('ascii'), typ, value
        entries = struct.unpack_from('<' + 'I' * indirect[1], data, indirect[0])
        shim_number = 0
        for address, length, kind, first, stride in sections:
            step = stride if kind == 8 else 4
            for i in range(length // step):
                index = entries[first+i]
                if index & 0xc0000000:
                    continue
                name, typ, value = symbol(index)
                location = address + i * step
                if typ & 0xe == 0xe:
                    if kind == 8:
                        self.write(location, b'\xe9' + struct.pack('<i', value-location-5))
                    else:
                        self.write(location, struct.pack('<I', value))
                    continue
                if kind != 8:
                    # Only an empty-string storage symbol is supported as data.
                    if name == '__ZNSs4_Rep20_S_empty_rep_storageE':
                        self.write(location, struct.pack('<I', self.allocate(64)))
                    continue
                shim = self.SHIMS + shim_number * 32
                shim_number += 1
                assert shim_number < 1024
                self.adapters[shim] = name
                if name.lstrip('_') in ('sin', 'sinf', 'cos', 'cosf', 'tan', 'tanf', 'exp', 'expf', 'log', 'log10f', 'pow', 'powf', 'atan2f'):
                    self.write(shim, b'\xdd\x05' + struct.pack('<I', shim + 16) + b'\xc3')
                else:
                    self.write(shim, b'\xc3')
                self.write(location, b'\xe9' + struct.pack('<i', shim-location-5))

    def _dispatch(self, cpu, address, size, context):
        self.last_pc = address
        if address == 0:
            raise RuntimeError('attempt to execute null pointer')
        name = self.adapters.get(address)
        if name is None:
            return
        self.import_calls[name] += 1
        stack = cpu.reg_read(UC_X86_REG_ESP)
        args = struct.unpack('<6I', self.read(stack+4, 24))
        result = 0
        short = name.lstrip('_')
        if short == 'oracle_capture_cxa_atexit':
            if len(self.destructor_registrations) >= 53 or not (0 < args[0] < 0x34000):
                raise RuntimeError('unexpected private destructor registration')
            self.destructor_registrations.append(tuple(args[:3]))
        elif short in ('malloc', 'Znwm', 'Znwj', 'Znam', 'Znaj'):
            result = self.allocate(args[0])
        elif short == 'calloc':
            result = self.allocate(args[0]*args[1])
        elif short in ('free', 'ZdlPv', 'ZdaPv'):
            if args[0] and args[0] not in self.allocations:
                raise ValueError(f'free of unknown private pointer {args[0]:#x}')
        elif short in ('memcpy', 'memmove'):
            if args[2] > self.HEAP_SIZE:
                raise ValueError('oversized memory copy')
            self.write(args[0], self.read(args[1], args[2])); result = args[0]
        elif short == 'memset':
            if args[2] > self.HEAP_SIZE:
                raise ValueError('oversized memory fill')
            self.write(args[0], bytes([args[1] & 255])*args[2]); result = args[0]
        elif short == 'strncpy':
            if args[2] > 65536:
                raise ValueError('oversized private string copy')
            raw = bytearray()
            for i in range(args[2]):
                byte = self.read(args[1]+i, 1)[0]
                if byte == 0:
                    break
                raw.append(byte)
            self.write(args[0], bytes(raw).ljust(args[2], b'\0')); result = args[0]
        elif short == 'puts':
            # Capture diagnostics only; never call host stdout or a device.
            self.diagnostics.append(self.cstring(args[0]).decode('ascii', errors='replace'))
        elif short == 'strlen':
            result = len(self.cstring(args[0]))
        elif short in ('strcmp', 'strncmp'):
            if short == 'strncmp':
                if args[2] > 65536:
                    raise ValueError('oversized private string compare')
                def bounded_string(pointer):
                    raw = bytearray()
                    for i in range(args[2]):
                        byte = self.read(pointer+i, 1)[0]
                        if byte == 0:
                            break
                        raw.append(byte)
                    return bytes(raw)
                left, right = bounded_string(args[0]), bounded_string(args[1])
            else:
                left, right = self.cstring(args[0]), self.cstring(args[1])
            result = ((left > right) - (left < right)) & 0xffffffff
        elif short in ('sin', 'sinf', 'cos', 'cosf', 'tan', 'tanf', 'exp', 'expf', 'log', 'log10f', 'pow', 'powf', 'atan2f'):
            single = short.endswith('f')
            scalar = ctypes.c_float if single else ctypes.c_double
            width, fmt = (4, '<f') if single else (8, '<d')
            arity = 2 if short in ('pow', 'powf', 'atan2f') else 1
            values = [struct.unpack(fmt, self.read(stack+4+i*width, width))[0] for i in range(arity)]
            method = getattr(self.libm, short)
            method.argtypes, method.restype = [scalar] * arity, scalar
            value = method(*values)
            self.write(address+16, struct.pack('<d', value))
        else:
            raise RuntimeError(f'unsupported import {name} at caller {self.read(stack,4).hex()}')
        cpu.reg_write(UC_X86_REG_EAX, result)

    def load_models(self, records=None):
        """Use already native-verified serialization offsets, then native AddModel.

        Bucket storage is preallocated so vector growth needs no C++ runtime.
        Original constructors and insertion routines still execute unchanged.
        """
        import hashlib
        import json
        if records is None:
            records = json.loads((self.artifact_root / 'analysis/reports/model-catalog.json').read_text())
        schema = json.loads((self.artifact_root / 'analysis/reports/native-body-schema.json').read_text())
        self.model_objects = {}
        self.call('0x6770', self.OBJECT)
        for field in (0x1120, 0x1128):
            table = struct.unpack('<I', self.read(self.OBJECT + field, 4))[0]
            for i in range(16):
                begin = self.allocate(8 * 128)
                self.write(table + i * 12, struct.pack('<3I', begin, begin, begin + 8 * 128))
        for record in records:
            payload = (self.artifact_root / record['path']).read_bytes()
            assert hashlib.sha256(payload).hexdigest() == record['sha256']
            if record['kind'] == 'body':
                obj = self.allocate(0x4bb4)
                self.call('0x9750', obj)
                for field in schema:
                    width = field['width']
                    start = field['file_offset']
                    raw = payload[start:start + width * field['count']]
                    self.write(obj + field['object_offset'], b''.join(raw[i:i+width][::-1] for i in range(0, len(raw), width)))
                self.call('0x5e20', self.OBJECT, record['id'], obj)
            else:
                obj = self.allocate(0x5c)
                self.call('0x9ea0', obj)
                self.write(obj+4, b''.join(payload[i:i+8][::-1] for i in range(0, 32, 8)))
                self.write(obj+0x40, payload[32:33])
                self.write(obj+0x24, payload[33:37][::-1])
                self.write(obj+0x28, b''.join(payload[i:i+4][::-1] for i in range(37, 61, 4)))
                self.write(obj+0x44, b''.join(payload[i:i+4][::-1] for i in range(61, 85, 4)))
                self.call('0x7790', self.OBJECT, record['id'] & 0xffffffff, obj)
            self.model_objects[(record['kind'], record['id'])] = obj
        return records

    def generate(self, settings):
        # Whole-library reference output is invalid without this module's
        # constructor. Partial body/circuit oracles do not use these globals.
        tone, volume = struct.unpack('<2d', self.read(0x3d798, 16))
        if not self.runtime_initialized or (tone, volume) != self.EXPECTED_KNOB_CONSTANTS:
            raise RuntimeError('whole-generator reference blocked: native module initialization is incomplete or knob constants changed (issue #5)')
        patch = self.OUTPUT + 0x2000
        self.write(patch, bytes(settings))
        self.call('0x180a0', self.OUTPUT)
        self.call('0x2c40', self.OBJECT, self.OUTPUT, patch,
                  instruction_limit=200_000_000, timeout_us=10_000_000)
        self.call('0x1bd10', self.OUTPUT, self.OUTPUT + 0x4000)
        return self.read(self.OUTPUT + 0x4000, 3276)


def default_settings():
    """Synthetic electric input; layout from JNI 0x4790, Java VWPatch defaults."""
    b = bytearray(0xc0)
    struct.pack_into('<I', b, 4, 0x3100003)
    b[8] = 1
    struct.pack_into('<3I', b, 0xc, 0x3110001, 0x3110000, 0xffffffff)
    for offset, value in {0x18:2.0, 0x1c:6.0, 0x38:500000.0,
                          0x3c:500000.0, 0x40:2.2e-8, 0x44:1e6, 0x48:3.5e-10,
                          0x98:1.0, 0xa8:220.0, 0xac:220.0, 0xb0:220.0,
                          0xb4:220.0, 0xb8:220.0}.items():
        struct.pack_into('<f', b, offset, value)
    struct.pack_into('<I', b, 0xa0, 1)
    return b
