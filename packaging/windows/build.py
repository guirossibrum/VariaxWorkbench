#!/usr/bin/env python3
"""Build, verify and package the unsigned Windows x64 offline alpha."""

import argparse
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


def smoke(extracted, scratch, env):
    # Execute the archived payload. Practice uses only the fake rig. In
    # computer mode skip even OS USB and audio inventory on the build host.
    env = dict(env, XDG_DATA_HOME=str(scratch / "xdg-data"),
               VARIAX_WORKBENCH_DATA_DIR=str(scratch / "workbench-data"))
    workbench = str(extracted / "Variax Workbench.exe")
    rig = str(extracted / "Rig Check.exe")
    version = run(workbench, "--version", env=env, cwd=extracted).strip()
    require(version, "Variax Workbench")
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
            "The Windows connection isn't available in this version yet.")
    if re.search(r"Windows version: \d+(?:\.\d+){2,3}", computer) is None:
        raise ValueError("Computer smoke check did not report the Windows version")
    outcomes = re.findall(
        r"^\[(?:[1-7]\.\d+|8\.[2-5])\].* — ([^\n]+)$", computer, re.M
    )
    if not outcomes or set(outcomes) != {"NOT AVAILABLE"}:
        raise ValueError(f"Unexpected device outcomes in Windows computer mode: {outcomes}")
    return (
        f"{version}\nRig Check catalogue loaded.\n"
        "Practice report includes simulated USB recognition.\n"
        "Computer report includes Windows information and the offline engine comparison.\n"
        "Device steps report the Windows notice. OS USB and audio inventories were skipped.\n"
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
