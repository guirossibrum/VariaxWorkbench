"""Verify unsigned x64 Windows executables and assemble the offline ZIP."""

import hashlib
import struct
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

APPS = {
    "Variax Workbench.exe": "variax-workbench-gui.exe",
    "Rig Check.exe": "rig-check.exe",
}
CONTENTS = (*APPS, "How to open.txt", "BUILD.txt")
CHECKSUMS = "SHA256SUMS.txt"

# Imports supplied by Windows on the supported x64 systems. A new import needs
# review before it becomes part of the package's runtime requirements.
SYSTEM_DLLS = {
    "advapi32.dll", "bcrypt.dll", "bcryptprimitives.dll", "cfgmgr32.dll",
    "combase.dll", "comctl32.dll", "comdlg32.dll", "d3d11.dll", "d3d12.dll", "dbghelp.dll", "dcomp.dll",
    "dwmapi.dll", "dxgi.dll", "gdi32.dll", "imm32.dll", "kernel32.dll",
    "msimg32.dll", "msvcrt.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll",
    "opengl32.dll", "powrprof.dll", "propsys.dll", "psapi.dll", "rpcrt4.dll",
    "secur32.dll", "setupapi.dll", "shell32.dll", "shlwapi.dll",
    "ucrtbase.dll", "user32.dll", "userenv.dll", "uxtheme.dll", "version.dll",
    "windowscodecs.dll", "winmm.dll", "winspool.drv", "ws2_32.dll", "wtsapi32.dll",
}


def windows_imports(data):
    def read(fmt, offset):
        if not 0 <= offset <= len(data) - struct.calcsize(fmt):
            raise ValueError("truncated PE executable")
        return struct.unpack_from(fmt, data, offset)

    if data[:2] != b"MZ":
        raise ValueError("missing DOS header")
    pe, = read("<I", 0x3C)
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("missing PE header")
    machine, count, _, _, _, header_size, flags = read("<HHIIIHH", pe + 4)
    optional = pe + 24
    magic, = read("<H", optional)
    if machine != 0x8664 or magic != 0x20B or not flags & 0x0002 or flags & 0x2000:
        raise ValueError("expected an x64 Windows executable")
    directories, = read("<I", optional + 108)
    if header_size < 112 or directories > (header_size - 112) // 8:
        raise ValueError("invalid PE data directories")

    def directory(index):
        return read("<II", optional + 112 + index * 8) if index < directories else (0, 0)

    if directory(4) != (0, 0):
        raise ValueError("expected an unsigned Windows executable")

    section_table = optional + header_size
    sections = [
        read("<IIII", section_table + 40 * index + 8)
        for index in range(count)
    ]

    def offset_of(rva, length):
        for _, address, raw_size, raw_offset in sections:
            delta = rva - address
            if 0 <= delta <= raw_size - length and raw_offset + delta + length <= len(data):
                return raw_offset + delta
        raise ValueError("PE import points outside the file")

    def dll_name(rva):
        offset = offset_of(rva, 1)
        end = data.find(b"\0", offset, offset + 256)
        if end < 0:
            raise ValueError("invalid PE import name")
        offset_of(rva, end - offset + 1)
        return data[offset:end].decode("ascii").lower()

    dlls = set()
    # Normal and delay-load import descriptors have distinct layouts.
    for index, width, name_field in ((1, 5, 3), (13, 8, 1)):
        address, size = directory(index)
        if (address, size) == (0, 0):
            continue
        stride = width * 4
        for delta in range(0, size - stride + 1, stride):
            fields = read("<" + "I" * width, offset_of(address + delta, stride))
            if not any(fields):
                break
            if index == 13 and fields[0] != 1:
                raise ValueError("unsupported PE delay-load address format")
            dlls.add(dll_name(fields[name_field]))
        else:
            raise ValueError("unterminated PE import table")

    if not dlls:
        raise ValueError("Windows executable has no imports")
    for dll in sorted(dlls):
        system_api = dll.startswith(("api-ms-win-", "ext-ms-win-")) and dll.endswith(".dll")
        if dll not in SYSTEM_DLLS and not system_api:
            raise ValueError(f"non-system DLL import: {dll}")
    return sorted(dlls)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def checksum(data, name):
    return f"{sha256(data)}  {name}\n"


def build_zip(archive, binaries, instructions, build_info):
    payload = {
        title: (binaries / executable).read_bytes()
        for title, executable in APPS.items()
    }
    verification = [build_info.rstrip(), ""]
    for title, data in payload.items():
        verification.extend([
            f"{title}: {len(data)} bytes",
            "Windows system imports: " + ", ".join(windows_imports(data)),
        ])
    payload["How to open.txt"] = instructions.read_bytes()
    payload["BUILD.txt"] = ("\n".join(verification) + "\n").encode()
    payload[CHECKSUMS] = "".join(
        checksum(data, name) for name, data in payload.items()
    ).encode("ascii")
    with ZipFile(archive, "x", compression=ZIP_DEFLATED, compresslevel=9) as bundle:
        for name, data in payload.items():
            bundle.writestr(name, data)
    return verify_zip(archive, archive.with_suffix(".zip.sha256"), write_checksum=True)


def verify_zip(archive, detached_checksum, *, write_checksum=False):
    with ZipFile(archive) as bundle:
        if bundle.namelist() != [*CONTENTS, CHECKSUMS] or bundle.testzip() is not None:
            raise ValueError("unexpected or damaged ZIP contents")
        expected = "".join(checksum(bundle.read(name), name) for name in CONTENTS)
        if bundle.read(CHECKSUMS) != expected.encode("ascii"):
            raise ValueError("invalid checksums inside ZIP")
        for name in APPS:
            windows_imports(bundle.read(name))
        build_info = bundle.read("BUILD.txt").decode()
    detached = checksum(archive.read_bytes(), archive.name)
    if write_checksum:
        with detached_checksum.open("x", encoding="ascii", newline="\n") as output:
            output.write(detached)
    elif detached_checksum.read_text(encoding="ascii") != detached:
        raise ValueError("invalid ZIP checksum")
    return f"{build_info}\nZip: {archive.stat().st_size} bytes\nSHA-256: {detached}"
