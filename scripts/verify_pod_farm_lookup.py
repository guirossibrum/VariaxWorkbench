"""Bounded CPU oracle for two reviewed pure-data POD Farm lookup leaves.

Only hash-verified leaf instructions enter emulated RX memory. No loader, entry
point, imports, DSP, authorization or device code is mapped or called. Synthetic
table/stack memory has no host bridge. This is a research helper, not Rust runtime.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_PROT_READ, UC_PROT_WRITE, UC_PROT_EXEC
from unicorn import UC_HOOK_CODE, UC_HOOK_INTR
from unicorn.x86_const import UC_X86_REG_RSP, UC_X86_REG_RBP, UC_X86_REG_RDI
from unicorn.x86_const import UC_X86_REG_RSI, UC_X86_REG_RDX, UC_X86_REG_RAX, UC_X86_REG_RIP

from research_setup import write_new

SOURCE_SHA = 'dced9e6bee1ac57ba9135c1b8751d26141be82155122f7aa151b2cb457bd55a1'
CODE = 0x10000000
TABLE = 0x20000000
STACK = 0x30000000
STOP = CODE + 0x51000
LEAVES = {0x50430: 0x50487, 0x50b00: 0x50bd1}
GROUPS = {0: 0, 1: 1, 0x1000: 2, 0x7000: 3, 0x7001: 4}
INVALID = [-1, 2, 0xfff, 0x1001, 0x6fff, 0x7002, 0x7fffffff, 0x80000000]


def verify(source):
    import unicorn
    if unicorn.__version__ != '2.1.4':
        raise ValueError('requires pinned offline Unicorn 2.1.4')
    data = source.read_bytes()
    if hashlib.sha256(data).hexdigest() != SOURCE_SHA:
        raise ValueError('native slice identity mismatch; no emulation started')
    uc = Uc(UC_ARCH_X86, UC_MODE_64)
    uc.mem_map(CODE+0x50000, 0x1000, UC_PROT_READ | UC_PROT_WRITE)
    for start, end in LEAVES.items():
        uc.mem_write(CODE+start, data[start:end])
    uc.mem_protect(CODE+0x50000, 0x1000, UC_PROT_READ | UC_PROT_EXEC)
    uc.mem_map(TABLE, 0x70000, UC_PROT_READ | UC_PROT_WRITE)
    uc.mem_map(STACK, 0x4000, UC_PROT_READ | UC_PROT_WRITE)
    active = [0, 0]
    observed = [0]

    def instruction(machine, address, size, _):
        if not (active[0] <= address < address+size <= active[1]):
            raise ValueError(f'instruction escaped reviewed leaf: {address:x}')
        observed[0] += 1

    def interrupt(*_):
        raise ValueError('guest interrupt rejected; no host service is available')

    uc.hook_add(UC_HOOK_CODE, instruction)
    uc.hook_add(UC_HOOK_INTR, interrupt)

    def call(start, *args):
        active[:] = [CODE+start, CODE+LEAVES[start]]
        top = STACK+0x3f00
        uc.mem_write(top, struct.pack('<Q', STOP))
        uc.reg_write(UC_X86_REG_RSP, top)
        uc.reg_write(UC_X86_REG_RBP, 0)
        for register, value in zip([UC_X86_REG_RDI, UC_X86_REG_RSI, UC_X86_REG_RDX], args):
            uc.reg_write(register, value & 0xffffffffffffffff)
        uc.emu_start(CODE+start, STOP, count=512)
        if uc.reg_read(UC_X86_REG_RIP) != STOP:
            raise ValueError('leaf did not return within instruction budget')
        return uc.reg_read(UC_X86_REG_RAX)

    selector_cases = lookup_cases = 0
    for selector in [*GROUPS, *INVALID]:
        result = call(0x50430, selector)
        if result != int(selector in GROUPS):
            raise ValueError(f'selector mismatch: {selector:x}')
        selector_cases += 1
    for selector, group in GROUPS.items():
        for index in range(216):
            address = TABLE+8+group*0x15858+0x18+index*0x198
            for flag in [0, 1, 0x80]:
                uc.mem_write(address, bytes([flag]))
                result = call(0x50b00, TABLE, selector, index)
                if result != (address if flag else 0):
                    raise ValueError(f'lookup mismatch: {selector:x}, {index}, {flag}')
                lookup_cases += 1
            uc.mem_write(address, b'\0')
    for selector in INVALID:
        for index in [0, 215, 216, 0xffffffff, 0x3f000003]:
            if call(0x50b00, TABLE, selector, index) != 0:
                raise ValueError('invalid selector accepted')
            lookup_cases += 1
    for selector in GROUPS:
        for index in [216, 217, 0x80000000, 0xffffffff, 0x3f000003]:
            if call(0x50b00, TABLE, selector, index) != 0:
                raise ValueError('invalid ordinal accepted')
            lookup_cases += 1
    return {'source_sha256': SOURCE_SHA, 'unicorn_version': unicorn.__version__,
            'selector_cases': selector_cases, 'lookup_cases': lookup_cases,
            'total_cases': selector_cases+lookup_cases, 'instructions': observed[0],
            'mismatches': 0, 'synthetic_flags': [0, 1, 128],
            'leaves': [{'start_va': hex(start), 'end_va_exclusive': hex(end),
                        'sha256': hashlib.sha256(data[start:end]).hexdigest()}
                       for start, end in LEAVES.items()],
            'emulated_leaf_execution': True, 'initialization_called': False, 'imports_mapped': False,
            'native_vendor_execution': False, 'host_service_bridge': False,
            'device_access': False, 'hardware_applicability_verified': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=Path('analysis/pod-farm-258/native-edge/L6TWXY.x86_64.macho'))
    args = parser.parse_args()
    result = verify(args.source)
    data = (json.dumps(result, indent=2, sort_keys=True)+'\n').encode()
    root = Path(__file__).resolve().parents[1]
    path = root/'analysis/pod-farm-258/native-edge'/('lookup-oracle-'+hashlib.sha256(data).hexdigest()+'.json')
    write_new(path, data)
    print(json.dumps(result, sort_keys=True))
    print(path)


if __name__ == '__main__':
    main()
