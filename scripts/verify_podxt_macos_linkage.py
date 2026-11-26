#!/usr/bin/env python3
"""Build and inspect the USB profile on a Mac without launching either app."""

import argparse
from importlib.util import module_from_spec, spec_from_file_location
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PACKAGE_SPEC = spec_from_file_location("macos_package", ROOT / "packaging/macos/package.py")
PACKAGE = module_from_spec(PACKAGE_SPEC)
PACKAGE_SPEC.loader.exec_module(PACKAGE)
TARGETS = {"aarch64-apple-darwin": "arm64", "x86_64-apple-darwin": "x86_64"}
BINARIES = ("rig-check", "variax-workbench-gui")
ARCHIVE_SYMBOLS = PACKAGE.USB_SYMBOLS
build_outputs = PACKAGE.build_usb_outputs
require_defined_symbols = PACKAGE.require_defined_symbols
verify_binary = PACKAGE.verify_usb_binary


def inspect(target, arch, evidence, expect_owner_link):
    env = {
        **os.environ,
        "SDKROOT": PACKAGE.xcrun("--sdk", "macosx", "--show-sdk-path"),
        "MACOSX_DEPLOYMENT_TARGET": ".".join(map(str, PACKAGE.ARCHITECTURES[arch])),
    }
    cargo = [
        "cargo", "+1.99.1", "build", "--locked", "--offline", "--release",
        "--no-default-features", "--features", "gui,portable-math,usb",
        "--target", target, "--bin", "rig-check", "--bin", "variax-workbench-gui",
        "--message-format=json-render-diagnostics",
    ]
    log = evidence / f"{arch}-cargo.jsonl"
    with log.open("x") as stream:
        subprocess.run(cargo, cwd=ROOT, env=env, stdout=stream, check=True)
    archive, binaries = build_outputs(log.read_text())

    def report(label, tool, *args):
        output = PACKAGE.xcrun(tool, *args)
        (evidence / f"{arch}-{label}-{tool}.txt").write_text(output + "\n")
        return output

    if report("libusb-archive", "lipo", "-archs", archive).split() != [arch]:
        raise ValueError(f"{archive}: expected one {arch} slice")
    require_defined_symbols(report("libusb-archive", "nm", "-gU", archive), ARCHIVE_SYMBOLS)
    for name, executable in binaries.items():
        if report(name, "lipo", "-archs", executable).split() != [arch]:
            raise ValueError(f"{executable}: expected one {arch} slice")
        verify_binary(
            arch, report(name, "vtool", "-show-build", executable),
            report(name, "otool", "-L", executable),
        )
        symbols = report(name, "nm", "-gU", executable)
        if expect_owner_link:
            require_defined_symbols(symbols, ARCHIVE_SYMBOLS)
    print(f"{target}: static archive, architecture, deployment and dylib checks passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path, help="new directory for native inspection logs")
    parser.add_argument(
        "--expect-owner-link", action="store_true",
        help="require USB implementation symbols in both owner-build executables",
    )
    args = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("native linkage verification needs macOS and the Apple SDK")
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    with (evidence / "revision.txt").open("x") as stream:
        subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, stdout=stream, check=True)
        subprocess.run(["git", "status", "--short"], cwd=ROOT, stdout=stream, check=True)
    for target, arch in TARGETS.items():
        inspect(target, arch, evidence, args.expect_owner_link)


if __name__ == "__main__":
    main()
