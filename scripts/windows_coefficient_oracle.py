"""Workbench 1.75 PE32 coefficient execution in private i386 memory.

Original module initializers, JNI exports, resource parsing, generator, and packer
execute in Unicorn. All OS imports remain trapped by WindowsMathOracle. The
adapters below are limited to private allocation, registration of unused exit
callbacks, CRT thread-local C locale data, and Java fields, streams, and byte arrays.
"""

import hashlib
import json
import struct

from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_FPCW, UC_X86_REG_FPTAG

from native_oracle import NativeOracle, ROOT
from verify_coefficient_jni import FIELD_LAYOUT
from windows_math_oracle import WindowsMathOracle


class WindowsCoefficientOracle(WindowsMathOracle):
    HEAP = 0x24000000
    HEAP_SIZE = 0x800000
    JNI = 0x25000000
    ENGINE = 0x10039d80
    ENGINE_SIZE = 0x1134
    DATA_ARRAY = 1
    PATCH = 2
    STREAM = 3

    read = NativeOracle.read
    write = NativeOracle.write

    def __init__(self, control=0x27f, dispatch='initialized-sse2', *,
                 artifact_root=ROOT, quantizer_f32=False):
        super().__init__(control, dispatch, artifact_root=artifact_root)
        self.artifact_root = artifact_root
        self.cursor = self.HEAP
        self.allocations = set()
        self.cpu.mem_map(self.HEAP, self.HEAP_SIZE)
        self.cpu.mem_map(self.JNI, 0x10000)
        self.registered_exit_callbacks = []
        self.jni_fields = {}
        self.jni_handles = {}
        self.stream = b''
        self.stream_position = 0
        self.array_released = False

        self.install(0x100155a9, 1, self.allocate)
        self.install(0x100187d9, 1, self.allocate)
        self.install(0x1001889c, 1, self.free)
        self.install(0x10015cba, 1, self.register_exit)
        # The serialized model reader uses the CRT default C locale.
        # __getptd normally obtains this pair through Windows TLS.
        thread_data = self.allocate(0x100)
        self.write(thread_data + 0x68, self.read(0x100392b8, 4) + self.read(0x10039498, 4))
        self.install(0x10018d2f, 0, lambda: thread_data)
        self.setup_jni()
        self.initializers = struct.unpack('<55I', self.read(0x1002a128, 55 * 4))
        if not all(0x10028fb0 <= address <= 0x10029980 for address in self.initializers):
            raise ValueError('unexpected PE C++ initializer table')
        for address in self.initializers:
            self.call(address)
        self.call(0x10001c10, self.JNI, 0, 0x70004)
        if quantizer_f32:
            self.use_f32_quantizer()

    def use_f32_quantizer(self):
        # Set x87 precision only inside the original Q23 converter. Its cdecl
        # entry and two RET exits were traced in the fingerprinted PE.
        def enter(cpu, _address, _size, _context):
            self.quantizer_caller_control = cpu.reg_read(UC_X86_REG_FPCW)
            cpu.reg_write(UC_X86_REG_FPCW, self.quantizer_caller_control & ~0x300)

        def leave(cpu, _address, _size, _context):
            cpu.reg_write(UC_X86_REG_FPCW, self.quantizer_caller_control)

        self.cpu.hook_add(UC_HOOK_CODE, enter, begin=0x10008370, end=0x10008370)
        for address in (0x100083c6, 0x1000841a):
            self.cpu.hook_add(UC_HOOK_CODE, leave, begin=address, end=address)

    def call(self, address, *arguments, this=0):
        self.cpu.reg_write(UC_X86_REG_ECX, this)
        self.cpu.reg_write(UC_X86_REG_FPTAG, 0xffff)
        control = self.cpu.reg_read(UC_X86_REG_FPCW)
        NativeOracle._execute(self, address, arguments, 200_000_000, 10_000_000)
        if self.cpu.reg_read(UC_X86_REG_FPCW) != control:
            raise ValueError(f'PE call changed the x87 control word at {address:#x}')
        return self.cpu.reg_read(UC_X86_REG_EAX)

    def allocate(self, size):
        address = self.cursor
        self.cursor += (max(size, 1) + 15) & ~15
        if self.cursor > self.HEAP + self.HEAP_SIZE:
            raise MemoryError('PE private heap exhausted')
        self.allocations.add(address)
        return address

    def free(self, address):
        if address:
            self.allocations.remove(address)
        return 0

    def register_exit(self, address):
        if not 0x10001000 <= address < 0x10029ce5:
            raise ValueError('exit callback outside the PE text section')
        self.registered_exit_callbacks.append(address)
        return 0

    def install(self, address, nargs, action, *, stdcall=False, float_result=False):
        ret = b'\xc2' + struct.pack('<H', 4 * nargs) if stdcall else b'\xc3'
        code = b'\xd9\x05' + struct.pack('<I', address + 16) + ret if float_result else ret
        self.write(address, code)
        self.cpu.ctl_remove_cache(address, address + len(code))

        def dispatch(cpu, _address, _size, _context):
            stack = cpu.reg_read(UC_X86_REG_ESP)
            args = struct.unpack('<' + 'I' * nargs, self.read(stack + 4, 4 * nargs))
            result = action(*args)
            if float_result:
                self.write(address + 16, struct.pack('<I', result))
            else:
                cpu.reg_write(UC_X86_REG_EAX, result & 0xffffffff)

        self.cpu.hook_add(UC_HOOK_CODE, dispatch, begin=address, end=address)

    def cstring(self, pointer):
        data = self.read(pointer, 64)
        end = data.index(0)
        return data[:end].decode('ascii')

    def setup_jni(self):
        table = self.JNI + 0x100
        self.write(self.JNI, struct.pack('<I', table))
        methods = {
            0x18: (2, lambda name: 10 if self.cstring(name) == 'java/io/OutputStream' else 0),
            0x7c: (2, lambda obj: 11 if obj in (self.PATCH, self.STREAM) else 0),
            0x80: (3, lambda _obj, _class: 0),
            0x84: (4, self.method),
            0xc8: (4, self.read_stream),
            0x178: (4, self.field),
            0x180: (3, lambda obj, handle: self.value(obj, handle, 'Z')),
            0x190: (3, lambda obj, handle: self.value(obj, handle, 'I')),
            0x198: (3, lambda obj, handle: self.value(obj, handle, 'F')),
            0x2ac: (2, lambda obj: 3276 if obj == self.DATA_ARRAY else 0),
            0x2e0: (3, self.elements),
            0x300: (4, self.release),
            0x390: (1, lambda: 0),
        }
        for offset, (nargs, action) in methods.items():
            address = self.JNI + 0x1000 + 8 * offset
            self.write(table + offset, struct.pack('<I', address))

            def checked(env, *args, action=action):
                if env != self.JNI:
                    raise ValueError('unexpected JNI environment')
                return action(*args)

            self.install(address, nargs, checked, stdcall=True, float_result=offset == 0x198)
        self.array = self.JNI + 0x7000

    def method(self, class_handle, name, signature):
        name, signature = self.cstring(name), self.cstring(signature)
        if class_handle != 11 or name != 'read' or signature not in ('()I', '([BII)I'):
            raise ValueError(f'unexpected JNI method {name} {signature}')
        return 20 if signature == '()I' else 21

    def read_stream(self, stream, method, _arguments):
        if stream != self.STREAM or method != 20:
            raise ValueError('unexpected JNI stream call')
        if self.stream_position == len(self.stream):
            return -1
        result = self.stream[self.stream_position]
        self.stream_position += 1
        return result

    def field(self, class_handle, name, kind):
        name, kind = self.cstring(name), self.cstring(kind)
        if class_handle != 11 or self.jni_fields[name][0] != kind:
            raise ValueError(f'unexpected JNI field {name} {kind}')
        handle = len(self.jni_handles) + 1
        self.jni_handles[handle] = name
        return handle

    def value(self, obj, handle, kind):
        expected, bits = self.jni_fields[self.jni_handles[handle]]
        if obj != self.PATCH or kind != expected:
            raise ValueError('unexpected JNI field access')
        return bits

    def elements(self, obj, copy_flag):
        if obj != self.DATA_ARRAY or copy_flag != 0:
            raise ValueError('unexpected JNI byte array request')
        return self.array

    def release(self, obj, data, mode):
        if (obj, data, mode) != (self.DATA_ARRAY, self.array, 0) or self.array_released:
            raise ValueError('unexpected JNI byte array release')
        self.array_released = True
        return 0

    def load_models(self):
        records = json.loads((self.artifact_root / 'analysis/reports/model-catalog.json').read_text())
        for record in records:
            resource = (self.artifact_root / record['path']).read_bytes()
            if hashlib.sha256(resource).hexdigest() != record['sha256']:
                raise ValueError('model catalog resource fingerprint mismatch')
            self.stream = resource
            self.stream_position = 0
            entry = {'body': 0x10005970, 'pickup': 0x10005a20}[record['kind']]
            if self.call(entry, self.JNI, 0, self.STREAM, record['id'] & 0xffffffff) & 255 != 1:
                raise ValueError('original PE rejected model resource')
            if self.stream_position != len(resource):
                raise ValueError(f'original PE consumed {self.stream_position}/{len(resource)} model bytes')
        return records

    def generate(self, settings, mask=0xffffffff):
        self.jni_fields = {
            name: (kind, settings[offset] if kind == 'Z' else struct.unpack_from('<I', settings, offset)[0])
            for name, (offset, kind) in FIELD_LAYOUT.items()
        }
        self.jni_fields['m_block_mask'] = ('I', mask)
        self.write(self.array, bytes(3276))
        self.array_released = False
        self.call(0x10003cb0, self.JNI, 0, self.DATA_ARRAY, self.PATCH)
        if not self.array_released:
            raise ValueError('PE generator did not commit the JNI byte array')
        if set(self.jni_handles.values()) != set(self.jni_fields):
            raise ValueError('PE field map did not cover the input')
        return self.read(self.array, 3276)

    def volume_curve(self):
        samples = self.array
        patch = samples + 0x100
        self.write(samples, struct.pack('<9f', *(index / 8 for index in range(9))))
        self.call(0x10008d00, samples, this=patch)
        return struct.unpack('<9I', self.read(patch + 0x888, 36))
