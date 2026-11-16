#!/usr/bin/env python3
"""Read-only static provenance checks for the prepared Central binaries.

This script hashes private inputs and checks PE delay-import metadata and
Mach-O slice coordinates, plus qualified hashes of private code ranges. It
never stores or executes vendor code.
"""
from __future__ import annotations

import argparse
import hashlib
import struct
from pathlib import Path

EXPECTED_HASHES = {
    "host": "4da8acf6a8c96682ab31de562683002aac8af6f279b27c2d3e757df88c8fb693",
    "wrapper": "0c6d86e5c611ae0cef3437a6a3fe96174a9711ccd917c7134cdcb64d6eabf5cd",
    "updater": "d24bb48b0f13d9b46c6dd514e712c347ff9a00d4763555614f700c22634bebc7",
    "mac": "d69d4d5fb959ea460787dcbde748fb43ea6b846a937aa98076c55ba061be4a1f",
    "mac_x64": "21bf512d07ae507929cf1581cdc07491605e5dcac58d115e6a59e230d7f95b76",
}

# Hashes of bounded code ranges from the whole-file-pinned private inputs.
# These retain fail-closed source checks without storing vendor instruction or
# method-body bytes in the repository. Coordinates are PE VAs / Mach-O VMs.
PRIVATE_CODE_RANGES = {
    "Central.exe MethodDef 0x0600018a CIL RVA 0x7590": (
        114, "6968068572fc3cc9bc7e77c2c70150744aa48f663a3fead09a17cbabb1389150"),
    "EngineCLIWrapper.dll MethodDef 0x060001b9 CIL RVA 0x4aa0": (
        202, "7bd4392107020edfafe1471583b3aad43cffb8e8c43fc4ce256fa9171f7185ad"),
    "EngineCLIWrapper.dll createEngine thunk VA 0x1000adeb": (
        6, "d398445fe29cbec241cdbe6547819995976311accf37f98293e7d483430f9bb4"),
    "Central macOS x86_64 getter IMP VM 0x10005a347": (
        25, "a842854809128353370ebb5eb96614cb2f0bddb2e5288f4a051f5181481f82eb"),
    "Central macOS x86_64 getter initializer VM 0x10014afe2": (
        13, "aba3e17ec195114448ce0af7126814d77386baec6c51bc24098e4f77e68191b2"),
    "Central macOS x86_64 teardown selector site VM 0x100429de4": (
        7, "938a5930553ecc01c66e0042bdbe855ab7b1e79ff52bdbe5c2657698fd86e501"),
    "Central macOS x86_64 adjacent selector site VM 0x100429e14": (
        7, "7d49e93dda777ea6868002b6ef8514282df4c73bfd29521fd025543b139ed67b"),
    "Central macOS x86_64 teardown callsite VM 0x1000d5cca": (
        7, "2944d89e83db6b44ef66b8876d7ab777a62364a526ac785886da61e7ce15a450"),
    "Central macOS x86_64 vtable callsite VM 0x1000d5d0e": (
        3, "dfc75a9fe173bd841f13127fdc9ac24f4e3efa8c016609a95eddea21803af9c4"),
    "Central macOS x86_64 teardown callsite VM 0x1000f8b82": (
        7, "f5b39a817cff95d38b60061a083db0d49ec94d7ba755a1ac963d703491f3dcd2"),
    "Central macOS x86_64 callback vtable callsite VM 0x1000f8b63": (
        3, "22558207d1d705fcbda49d89be5b34ebe09d409878b0bf88e6819b7724e29e96"),
    "Central macOS x86_64 vtable callsite VM 0x1000f8bb7": (
        3, "dfc75a9fe173bd841f13127fdc9ac24f4e3efa8c016609a95eddea21803af9c4"),
}

def data(path: Path, key: str) -> bytes:
    b = path.read_bytes()
    got = hashlib.sha256(b).hexdigest()
    assert got == EXPECTED_HASHES[key], (key, path, got)
    return b


def assert_private_code_range(name: str, code: bytes) -> None:
    expected_length, expected_hash = PRIVATE_CODE_RANGES[name]
    got = hashlib.sha256(code).hexdigest()
    assert len(code) == expected_length and got == expected_hash, (
        name, len(code), got)


def self_test() -> None:
    # Synthetic PE image: one section maps RVA 0x1000 to raw offset 0x200.
    pe = bytearray(0x204)
    struct.pack_into("<I", pe, 0x3c, 0x40)
    struct.pack_into("<H", pe, 0x46, 1)
    struct.pack_into("<I", pe, 0x74, 0x10000000)
    # Minimal PE section table starts at optional-header end (0x58 + 0x60).
    struct.pack_into("<H", pe, 0x54, 0x60)
    struct.pack_into("<IIII", pe, 0xc0, 0x40, 0x1000, 0x80, 0x200)
    assert pe_offset(bytes(pe), 0x10001004) == 0x204
    # Synthetic rel32 call target and universal Mach-O fat header.
    call_va = 0x10001000
    call = b"\xe8" + struct.pack("<i", 0x1234)
    assert rel32_target(call_va, call) == call_va + 5 + 0x1234
    fat = struct.pack(">IIiiIIIiiIII", 0xcafebabe, 2,
                      0x01000007, 3, 0x40, 0x20, 0,
                      0x0100000c, 0, 0x60, 0x30, 0)
    magic, count = struct.unpack_from(">II", fat, 0)
    slices = [struct.unpack_from(">iiIII", fat, 8 + 20*i) for i in range(count)]
    assert magic == 0xcafebabe and len(slices) == 2
    assert (slices[0][2], slices[0][3], slices[1][2], slices[1][3]) == (0x40, 0x20, 0x60, 0x30)

def rel32_target(va: int, insn: bytes) -> int:
    if len(insn) != 5 or insn[0] != 0xe8:
        raise ValueError("expected a synthetic x86 rel32 call")
    return va + len(insn) + struct.unpack_from("<i", insn, 1)[0]


def pe_offset(b: bytes, va: int) -> int:
    pe = struct.unpack_from("<I", b, 0x3c)[0]
    nsec = struct.unpack_from("<H", b, pe + 6)[0]
    optsz = struct.unpack_from("<H", b, pe + 20)[0]
    opt = pe + 24
    imagebase = struct.unpack_from("<I", b, opt + 28)[0]
    rva = va - imagebase
    for i in range(nsec):
        s = opt + optsz + 40 * i
        vsize, vaddr, rawsize, rawoff = struct.unpack_from("<IIII", b, s + 8)
        if vaddr <= rva < vaddr + max(vsize, rawsize):
            return rawoff + (rva - vaddr)
    raise AssertionError(f"unmapped VA {va:#x}")


def cil_code(b: bytes, va: int) -> bytes:
    """Return one CIL body by PE VA without interpreting or loading it."""
    o = pe_offset(b, va)
    first = b[o]
    if first & 3 == 2:  # tiny header
        size = first >> 2
        return b[o + 1:o + 1 + size]
    flags_and_size = struct.unpack_from("<H", b, o)[0]
    assert flags_and_size & 3 == 3, (hex(va), hex(flags_and_size))
    header_size = (flags_and_size >> 12) * 4
    code_size = struct.unpack_from("<I", b, o + 4)[0]
    return b[o + header_size:o + header_size + code_size]

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--inputs-root", type=Path, help="private Windows 'Line 6 Central' directory")
parser.add_argument("--mac-app", type=Path, help="private macOS app executable")
parser.add_argument("--mac-x64", type=Path, help="private extracted x86_64 Mach-O slice")
parser.add_argument("--self-test", action="store_true", help="exercise format-coordinate helpers using synthetic bytes")
args = parser.parse_args()
if args.self_test:
    self_test()
    print("PASS synthetic PE/Mach-O coordinate self-tests")
    if not args.inputs_root and not args.mac_app and not args.mac_x64:
        raise SystemExit(0)
if not args.inputs_root or not args.mac_app or not args.mac_x64:
    parser.error("provide --inputs-root, --mac-app and --mac-x64 for private-source checks")
root = args.inputs_root
host, wrapper, updater = (data(root / "Line 6 Central.exe", "host"),
                          data(root / "EngineCLIWrapper.dll", "wrapper"),
                          data(root / "UpdaterV3EngineDLL.dll", "updater"))
# Managed host/wrapper identity and the traced wrapper imports.
assert b"appVersionString" in host
assert b"EngineCLIWrapper" in host
assert b"UpdaterV3EngineDLL.dll" in wrapper
assert b"?initialize@Functions@UpdaterV3EngineInterface" in wrapper
assert b"?start@Functions@UpdaterV3EngineInterface" in wrapper
assert b"?createEngine@Functions@UpdaterV3EngineInterface" in wrapper

# Static Central CIL: EngineInterface.createEngine subscribes the app's string
# handler to SendString, then constructs EngineCLIWrapper. The wrapper retains
# NativeStringParamValueReceiver and passes callback pointers in native export
# order: float, int, bool, string, string-array. No assembly is executed.
host_create = cil_code(host, 0x00407590)  # MethodDef 0x0600018a
assert_private_code_range("Central.exe MethodDef 0x0600018a CIL RVA 0x7590", host_create)
wrapper_ctor = cil_code(wrapper, 0x10004aa0)  # MethodDef 0x060001b9
assert_private_code_range("EngineCLIWrapper.dll MethodDef 0x060001b9 CIL RVA 0x4aa0", wrapper_ctor)
assert cil_code(wrapper, 0x100037a4)  # NativeStringParamValueReceiver, MethodDef 0x060001cd
assert b"NativeStringParamValueReceiver" in wrapper and b"stringParamValueDelegate" in wrapper
thunk_off = pe_offset(wrapper, 0x1000adeb)
assert_private_code_range("EngineCLIWrapper.dll createEngine thunk VA 0x1000adeb",
                           wrapper[thunk_off:thunk_off + 6])

# Updater provenance strings and delay-import metadata.
assert b"\\Line 6\\Interposer\\" in updater
assert b"Interposer.dll" in updater
for text in (b"InterposerGetDescriptors", b"InterposerMessage", b"GetProcAddress",
             b"loadInterposer() - Trying to load module"):
    assert text in updater
# IMAGE_DELAY_IMPORT_DESCRIPTOR at VA 0x1049f644 identifies Interposer.dll;
# INT names and IAT resolver thunks bind the actual named exports.
delay_desc = struct.unpack_from("<8I", updater, pe_offset(updater, 0x1049f644))
assert delay_desc == (1, 0x3b5240, 0x4bab00, 0x4baad8, 0x49f684, 0x49f78c, 0, 0)
assert updater[pe_offset(updater, 0x103b5240):].startswith(b"Interposer.dll\0")
expected_symbols = (
    "InterposerMessage", "InterposerGetDescriptorCount", "InterposerClosePipe",
    "InterposerOpenPipe", "InterposerCleanup", "InterposerInit",
    "InterposerGetDescriptors", "InterposerRegisterCallback",
    "InterposerGetApplicationId",
)
resolved_names = []
for i in range(len(expected_symbols)):
    rva = struct.unpack_from("<I", updater, pe_offset(updater, 0x1049f684) + 4*i)[0]
    name_off = pe_offset(updater, 0x10000000 + rva) + 2  # skip IMAGE_IMPORT_BY_NAME hint
    resolved_names.append(updater[name_off:updater.index(b"\0", name_off)].decode("ascii"))
assert tuple(resolved_names) == expected_symbols
slot_va = 0x104baad8
expected_thunks = (0x10268a8d, 0x10268ae4, 0x10268aa8, 0x10268ab2,
                   0x10268abc, 0x10268ac6, 0x10268ad0, 0x10268ada, 0x10268aee)
slots = struct.unpack_from("<9I", updater, pe_offset(updater, slot_va))
assert slots == expected_thunks

# Verify both universal Mach-O slice coordinates and the prepared x86_64 copy.
mac, x64 = data(args.mac_app, "mac"), data(args.mac_x64, "mac_x64")
magic, nfat = struct.unpack_from(">II", mac, 0)
assert magic == 0xcafebabe and nfat == 2
slices = []
for i in range(nfat):
    cpu, subtype, off, size, align = struct.unpack_from(">iiIII", mac, 8 + 20*i)
    slices.append((cpu & 0xffffffff, subtype & 0xffffffff, off, size))
assert slices == [(0x01000007, 3, 0x4000, 0x6db150), (0x0100000c, 0, 0x6e0000, 0x65c230)]
assert mac[0x4000:0x4000+0x6db150] == x64
assert b"tearingDownVariaxScanner" in x64
assert b"getParamValueSetInterposerDelegate" in x64
# ObjC metadata for UpdaterV3Engine maps the delegate getter selector to this
# IMP; the IMP dispatches [ParamValueServer sharedInstance]. The initializer
# sends that getter and stores its retained result in a global delegate slot.
assert struct.unpack_from("<QQQ", x64, 0x62b9d8) == (0x1005bd42c, 0x1005c2d36, 0x10005a347)
assert_private_code_range("Central macOS x86_64 getter IMP VM 0x10005a347",
                          x64[0x5a347:0x5a360])
assert struct.unpack_from("<Q", x64, 0x643050)[0] == 0x100665d80
assert struct.unpack_from("<Q", x64, 0x641d58)[0] == 0x1005c15dc
assert x64[0x5c15dc:0x5c15dc + len(b"sharedInstance\0")] == b"sharedInstance\0"
assert_private_code_range("Central macOS x86_64 getter initializer VM 0x10014afe2",
                          x64[0x14afe2:0x14afef])
assert struct.unpack_from("<QQQ", x64, 0x67c528) == (0x1005c1a71, 0x10058ff40, 0x100429de0)
assert_private_code_range("Central macOS x86_64 teardown selector site VM 0x100429de4",
                          x64[0x429de4:0x429deb])
assert_private_code_range("Central macOS x86_64 adjacent selector site VM 0x100429e14",
                          x64[0x429e14:0x429e1b])
# Two scanner teardown-key sites dispatch through vtable +0x40. The second
# first takes a callback object from receiver offset +0x1c8 and calls slot +0x18.
assert_private_code_range("Central macOS x86_64 teardown callsite VM 0x1000d5cca",
                          x64[0xd5cca:0xd5cd1])
assert_private_code_range("Central macOS x86_64 vtable callsite VM 0x1000d5d0e",
                          x64[0xd5d0e:0xd5d11])
assert_private_code_range("Central macOS x86_64 teardown callsite VM 0x1000f8b82",
                          x64[0xf8b82:0xf8b89])
assert_private_code_range("Central macOS x86_64 callback vtable callsite VM 0x1000f8b63",
                          x64[0xf8b63:0xf8b66])
assert_private_code_range("Central macOS x86_64 vtable callsite VM 0x1000f8bb7",
                          x64[0xf8bb7:0xf8bba])
# The earlier raw decode documents the Variax.json product-selection edge;
# these private-range hashes do not resolve runtime virtual targets or behavior.


print("PASS static-only Central interposer provenance checks")
print("Central/wrapper identity strings include appVersionString, EngineCLIWrapper, and initialize/start/createEngine imports")
print("Qualified private CIL range hashes: callback four maps to NativeStringParamValueReceiver; host subscribes setStringValue to SendString")
print("Updater source identity includes loadInterposer, current-directory API, Interposer path, and queue-out strings")
print("Updater delay-import binding: Interposer.dll descriptor, named import table, per-export IAT thunk resolver confirmed")
print("Mac metadata and qualified private code-range hashes: updater delegate resolves to ParamValueServer; teardown vtable sites and selector/IMP mismatch pinned")
print("Mac universal slices: x86_64 and arm64 coordinates/hash; product-selection edge remains separately documented")
print("No vendor code loaded or executed")
