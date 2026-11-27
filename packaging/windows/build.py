#!/usr/bin/env python3
"""Build, verify and package the unsigned Windows x64 offline alpha."""

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from tempfile import TemporaryDirectory

from package import APPS, build_zip

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TARGET = "x86_64-pc-windows-msvc"
TOOLCHAIN = "1.99.1"
FEATURES = "gui,portable-math"
WINDOWS_CONNECTION = "The Windows connection isn't available in this version yet."


def run(*args, env, cwd=ROOT, input_text=None):
    result = subprocess.run(
        args, env=env, cwd=cwd, encoding="utf-8", errors="replace",
        input=input_text, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        timeout=90,
    )
    if result.returncode:
        raise RuntimeError(f"{args[0]} exited with {result.returncode}\n{result.stdout}")
    return result.stdout


def require(text, *markers):
    for marker in markers:
        if marker not in text:
            raise ValueError(f"Missing smoke evidence {marker!r}\n{text}")


def windows_system_evidence(step):
    """Check the fields emitted by the Windows collector in step 0.1."""
    results = step.get("results", [])

    def value(field):
        prefix = f"{field}: "
        matches = [line.removeprefix(prefix) for line in results if line.startswith(prefix)]
        if len(matches) != 1:
            raise ValueError(f"Expected one {field} field in Windows step 0.1")
        return matches[0]

    version = value("Windows version")
    if re.fullmatch(r"[0-9]+(?:\.[0-9]+){2,3}", version) is None:
        raise ValueError(f"Invalid Windows version in step 0.1: {version!r}")
    build = value("Windows build")
    if build != version.split(".")[2]:
        raise ValueError(f"Windows build in step 0.1 does not match version {version}: {build!r}")
    architecture = value("Architecture")
    if architecture not in {"x86_64", "aarch64", "x86", "arm"}:
        raise ValueError(f"Unrecognized Windows architecture in step 0.1: {architecture!r}")
    return f"Windows version: {version}\nWindows build: {build}\nArchitecture: {architecture}\n"


def smoke(extracted, scratch, env):
    # Execute the archived payload. Practice uses only the fake rig. In
    # computer mode skip even OS USB and audio inventory on the build host.
    env = dict(env, XDG_DATA_HOME=str(scratch / "xdg-data"),
               VARIAX_WORKBENCH_DATA_DIR=str(scratch / "workbench-data"))
    workbench = str(extracted / "Variax Workbench.exe")
    rig = str(extracted / "Rig Check.exe")
    version = run(workbench, "--version", env=env, cwd=extracted).strip()
    require(version, "Variax Workbench")
    if number := env.get("VARIAX_BUILD_NUMBER"):
        require(version, f"(build {number})")
    require(run(rig, "--catalogue", env=env, cwd=extracted), "0.1 Computer", "8.1 Audio")

    practice = scratch / "practice"
    run(rig, "--practice", "--text", "--out", str(practice),
        env=env, cwd=extracted, input_text="accept\nr\nc\nr\nc\nr\nc\nr\nc\nq\n")
    report, = practice.glob("*.txt")
    scenario = report.read_text(encoding="utf-8")
    require(scenario, "Mode: practice (simulated devices)", "simulated computer")
    if re.search(r"^\[1\.1\].*— PASSED$", scenario, re.M) is None:
        raise ValueError("Practice did not pass the simulated USB recognition step")

    local = scratch / "computer"
    run(rig, "--real", "--text", "--out", str(local),
        env=env, cwd=extracted, input_text="accept\nr\nc\ns\nr\nc\ns\n")
    report, = local.glob("*.txt")
    computer = report.read_text(encoding="utf-8")
    require(computer, "Operating system: Windows", "Synthetic preset (full pipeline)",
            WINDOWS_CONNECTION)
    # The detail section omits steps that never started, including refusals.
    _, begin, data = computer.partition("\nBEGIN RIG CHECK JSON\n")
    data, end, _ = data.partition("\nEND RIG CHECK JSON")
    if not begin or not end:
        raise ValueError("Missing Rig Check JSON report")
    record = json.loads(data)
    revision = env["RIG_CHECK_GIT_COMMIT"]
    if record.get("git_commit") != revision:
        raise ValueError(f"Rig Check report does not identify source {revision}")
    if not record["finished"]:
        raise ValueError("Windows computer smoke check did not finish")
    steps = {step["id"]: step for step in record["steps"]}
    expected_computer = {
        "0.1": "passed",
        "0.2": "skipped",
        "0.3": "passed",
        "2.7": "skipped_needs",  # Offline comparison requires an identity from 2.1.
        "8.1": "skipped",
    }
    outcomes = {step: steps.get(step, {}).get("outcome") for step in expected_computer}
    if outcomes != expected_computer:
        raise ValueError(f"Unexpected computer outcomes in Windows computer mode: {outcomes}")
    system_evidence = windows_system_evidence(steps["0.1"])
    unexpected_messages = [step["id"] for step in steps.values() if step.get("messages") != 0]
    if unexpected_messages:
        raise ValueError(f"Expected zero device messages in Windows computer mode: {unexpected_messages}")
    device_steps = [step for key, step in steps.items() if key not in expected_computer]
    unexpected = [
        step["id"] for step in device_steps
        if (step["outcome"], step.get("reason")) != ("not_available", WINDOWS_CONNECTION)
    ]
    if not device_steps or unexpected:
        raise ValueError(f"Unexpected device outcomes in Windows computer mode: {unexpected}")
    return (
        f"{version}\nRig Check catalogue loaded.\n"
        "Practice report includes simulated USB recognition.\n"
        "Computer report includes step 0.1 and the offline engine comparison.\n"
        f"Rig Check report revision: {revision}\n"
        f"{system_evidence}"
        "Device steps report the Windows notice. Zero device messages recorded.\n"
        "OS USB and audio inventories were skipped.\n"
    )


def main():
    from zipfile import ZipFile

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, nargs="?", default=ROOT / "dist" / "windows")
    args = parser.parse_args()
    if sys.platform != "win32":
        parser.error("Run this script on x64 Windows with Rust and Visual Studio C++ build tools.")

    env = dict(os.environ, RUSTUP_TOOLCHAIN=TOOLCHAIN)
    version = re.search(r'^version = "(\d+\.\d+\.\d+)"$', (ROOT / "Cargo.toml").read_text(encoding="utf-8"), re.M)[1]
    commit = run("git", "rev-parse", "HEAD", env=env).strip()
    rustc = run("rustc", "-vV", env=env).strip()
    require(rustc, f"host: {TARGET}")
    target_dir = (ROOT / env.get("CARGO_TARGET_DIR", "target")).resolve()
    env.update(CARGO_TARGET_DIR=str(target_dir), RUSTFLAGS="-C target-feature=+crt-static",
               RIG_CHECK_GIT_COMMIT=commit, RIG_CHECK_RUSTC_VERSION=rustc.splitlines()[0],
               RIG_CHECK_TARGET=TARGET)
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)
    subprocess.run(
        ["cargo", "build", "--locked", "--release", "--target", TARGET,
         "--no-default-features", "--features", FEATURES,
         *(arg for exe in APPS.values() for arg in ("--bin", Path(exe).stem))],
        env=env, cwd=ROOT, check=True,
    )

    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix=".build-", dir=output) as work:
        scratch = Path(work)
        stage = scratch / "package"
        stage.mkdir()
        archive = stage / f"Variax-Workbench-{version}-windows-x64.zip"
        build_info = (
            f"Commit: {commit}\nRelease target: {TARGET}\nFeatures: {FEATURES}\n"
            f"Static C runtime\n{rustc}"
        )
        if number := env.get("VARIAX_BUILD_NUMBER"):
            build_info += f"\nBuild number {number}"
        verification = build_zip(
            archive, target_dir / TARGET / "release", HERE / "How to open.txt", build_info
        )
        extracted = scratch / "extracted"
        with ZipFile(archive) as bundle:
            bundle.extractall(extracted)
        verification += "\nPackaged executable smoke checks\n" + smoke(extracted, scratch, env)
        (stage / "verification.txt").write_text(verification, encoding="utf-8")
        destination = output / f"{version}-{commit[:8]}-{scratch.name.removeprefix('.build-')}"
        if destination.exists():
            raise FileExistsError(destination)
        stage.rename(destination)
        print(verification)
        print(f"Package and verification report: {destination}")


if __name__ == "__main__":
    main()
