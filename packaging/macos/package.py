"""Generate bundle metadata, lay out the disk image, and inspect its contents."""

import plistlib
import re
import subprocess
import sys
from pathlib import Path


ARCHITECTURES = {"x86_64": (10, 15), "arm64": (11, 0)}
APPS = {"Variax Workbench": "variax-workbench-gui", "Rig Check": "rig-check"}


def plist(path, title, binary, identifier, version):
    architecture_minimums = {
        arch: ".".join(map(str, minimum)) for arch, minimum in ARCHITECTURES.items()
    }
    with Path(path).open("wb") as output:
        plistlib.dump(
            {
                "CFBundleName": title,
                "CFBundleDisplayName": title,
                "CFBundleExecutable": binary,
                "CFBundleIdentifier": identifier,
                "CFBundleIconFile": "AppIcon.icns",
                "CFBundleInfoDictionaryVersion": "6.0",
                "CFBundlePackageType": "APPL",
                "CFBundleShortVersionString": version,
                "CFBundleVersion": version,
                "LSMinimumSystemVersion": architecture_minimums["x86_64"],
                "LSMinimumSystemVersionByArchitecture": architecture_minimums,
                "NSHighResolutionCapable": True,
            },
            output,
        )


def image(path, stage, background):
    from dmgbuild import build_dmg

    filenames = ["Variax Workbench.app", "Rig Check.app", "How to open.txt"]
    build_dmg(
        path,
        "Variax Workbench",
        settings={
            "format": "UDZO",
            "filesystem": "HFS+",
            "files": [str(Path(stage) / name) for name in filenames],
            "symlinks": {"Applications": "/Applications"},
            "background": background,
            "window_rect": ((160, 160), (720, 420)),
            "icon_size": 80,
            "text_size": 14,
            "icon_locations": {
                "Variax Workbench.app": (150, 132),
                "Rig Check.app": (150, 276),
                "Applications": (557, 204),
                "How to open.txt": (557, 327),
            },
        },
    )


def xcrun(*args):
    return subprocess.check_output(["xcrun", *args], text=True).rstrip()


def verify_load_commands(arch, vtool, otool):
    minimum = re.search(r"^\s*minos\s+(\d+\.\d+(?:\.\d+)?)\s*$", vtool, re.M)
    if minimum is None and "LC_VERSION_MIN_MACOSX" in vtool:
        minimum = re.search(r"^\s*version\s+(\d+\.\d+(?:\.\d+)?)\s*$", vtool, re.M)
    if minimum is None:
        raise ValueError(f"{arch}: no macOS deployment target in vtool output")
    version = tuple(map(int, minimum[1].split(".")))
    if version not in (ARCHITECTURES[arch], (*ARCHITECTURES[arch], 0)):
        raise ValueError(f"{arch}: unexpected minimum macOS version {minimum[1]}")

    loads = otool.splitlines()[1:]
    if not loads:
        raise ValueError(f"{arch}: no dylib load commands in otool output")
    for line in loads:
        path = line.strip().split(" (", 1)[0]
        if not path.startswith(("/usr/lib/", "/System/Library/")):
            raise ValueError(f"{arch}: non-system dylib {path}")
        if arch == "x86_64" and "/UniformTypeIdentifiers.framework/" in path:
            raise ValueError(f"{arch}: framework is unavailable on Catalina: {path}")


def verify(mount, scratch, dmg):
    for title, binary in APPS.items():
        bundle = Path(mount, f"{title}.app")
        executable = bundle / "Contents" / "MacOS" / binary
        arches = xcrun("lipo", "-archs", executable).split()
        if sorted(arches) != sorted(ARCHITECTURES):
            raise ValueError(f"{title}: expected two slices, got {arches}")
        bundle_bytes = sum(file.stat().st_size for file in bundle.rglob("*") if file.is_file())
        print(f"\n{title}.app: {bundle_bytes} bytes; universal binary: {executable.stat().st_size} bytes")
        for arch in ARCHITECTURES:
            thin = Path(scratch, f"{binary}-{arch}")
            xcrun("lipo", executable, "-thin", arch, "-output", thin)
            vtool = xcrun("vtool", "-show-build", thin)
            otool = xcrun("otool", "-L", thin)
            print(f"\n{title} ({arch}): {thin.stat().st_size} bytes")
            print(f"vtool -show-build\n{vtool}\notool -L\n{otool}", flush=True)
            verify_load_commands(arch, vtool, otool)
    print(f"\nDisk image: {Path(dmg).stat().st_size} bytes")


if __name__ == "__main__":
    commands = {"plist": plist, "image": image, "verify": verify}
    commands[sys.argv[1]](*sys.argv[2:])
