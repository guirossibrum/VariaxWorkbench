#!/usr/bin/env python3
"""Build and inspect the USB profile on a Mac without launching either app."""

import argparse
from importlib.util import module_from_spec, spec_from_file_location
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PACKAGE_SPEC = spec_from_file_location("macos_package", ROOT / "packaging/macos/package.py")
PACKAGE = module_from_spec(PACKAGE_SPEC)
PACKAGE_SPEC.loader.exec_module(PACKAGE)
TARGETS = {"aarch64-apple-darwin": "arm64", "x86_64-apple-darwin": "x86_64"}
BINARIES = ("rig-check", "variax-workbench-gui")
ARCHIVE_SYMBOLS = ("libusb_open", "libusb_claim_interface", "libusb_interrupt_transfer")


def build_outputs(cargo_log):
    """Use Cargo's output paths so a stale archive cannot satisfy this build."""
    archives = set()
    binaries = {}
    for line in cargo_log.splitlines():
        row = json.loads(line)
        if row["reason"] == "build-script-executed":
            if "static=usb-vendored" in row["linked_libs"] and "libusb1-sys" in row["package_id"]:
                archives.add(Path(row["out_dir"], "libusb-vendored.a"))
        if row["reason"] == "compiler-artifact" and row["executable"]:
            target = row["target"]
            if "bin" in target["kind"] and target["name"] in BINARIES:
                binaries[target["name"]] = Path(row["executable"])
    if len(archives) != 1 or set(binaries) != set(BINARIES):
        raise ValueError("Cargo must report one static libusb archive and both binaries")
    return archives.pop(), binaries


def require_defined_symbols(output, symbols):
    for symbol in symbols:
        if not re.search(rf"^\s*[0-9a-fA-F]+\s+[Tt]\s+_{symbol}\s*$", output, re.M):
            raise ValueError(f"missing defined Mach-O symbol _{symbol}")


def verify_binary(arch, vtool, otool):
    PACKAGE.verify_load_commands(arch, vtool, otool)
    for line in otool.splitlines()[1:]:
        path = line.strip().split(" (", 1)[0]
        if "libusb" in Path(path).name.lower():
            raise ValueError(f"{arch}: dynamically linked libusb {path}")


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
        if expect_owner_link and name == "rig-check":
            require_defined_symbols(symbols, ARCHIVE_SYMBOLS)
    print(f"{target}: static archive, architecture, deployment and dylib checks passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path, help="new directory for native inspection logs")
    parser.add_argument(
        "--expect-owner-link", action="store_true",
        help="require USB implementation symbols in an approved owner Rig Check build",
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
