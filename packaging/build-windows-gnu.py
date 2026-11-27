"""Cross-build and statically verify the Windows offline owner ZIP on Linux."""

import argparse
import fcntl
import os
from pathlib import Path
import re
import subprocess
import sys
from tempfile import TemporaryDirectory

from release_manifest import record, write_manifest
from windows.package import APPS, GNU_BUILD_ORIGIN, GNU_TARGET, GNU_VERIFICATION, build_zip, compiler_commit

FEATURES = "gui,portable-math"
PINNED_TOOLCHAIN = "1.99.1"


def build(revision, build_number, output):
    actual = subprocess.check_output(
        ["git", "rev-parse", "--verify", f"{revision}^{{commit}}"], text=True,
    ).strip()
    if actual != revision:
        raise ValueError("Windows source does not match the pinned commit")
    common_git_dir = Path(subprocess.check_output(
        ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], text=True,
    ).strip())
    target_dir = Path(os.environ.get(
        "CARGO_TARGET_DIR", common_git_dir.parent / "target/shared-builds/owner-windows-gnu",
    )).resolve()
    pinned = subprocess.check_output(
        ["rustup", "run", PINNED_TOOLCHAIN, "rustc", "-vV"], text=True,
    ).strip()
    rustc_path = subprocess.check_output(
        ["rustup", "which", "--toolchain", "stable", "rustc"], text=True,
    ).strip()
    rustc = subprocess.check_output([rustc_path, "-vV"], text=True).strip()
    stable_commit = compiler_commit(rustc)
    pinned_commit = compiler_commit(pinned)
    if stable_commit != pinned_commit:
        raise ValueError(
            f"Installed stable Rust compiler {stable_commit} differs from "
            f"pinned Rust {PINNED_TOOLCHAIN} compiler {pinned_commit}"
        )
    target_dir.mkdir(parents=True, exist_ok=True)
    env = dict(
        os.environ, CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(target_dir),
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc",
        RUSTFLAGS="-C target-feature=+crt-static", VARIAX_BUILD_NUMBER=build_number,
        RUSTC=rustc_path, RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="",
        RIG_CHECK_GIT_COMMIT=revision, RIG_CHECK_RUSTC_VERSION=rustc.splitlines()[0],
        RIG_CHECK_TARGET=GNU_TARGET,
    )
    env.pop("CARGO_ENCODED_RUSTFLAGS", None)

    with TemporaryDirectory(prefix=".windows-gnu-", dir=output.parent) as work:
        scratch = Path(work)
        source = scratch / "source"
        source.mkdir()
        source_tar = scratch / "source.tar"
        subprocess.run(["git", "archive", f"--output={source_tar}", revision], check=True)
        subprocess.run(["tar", "-xf", str(source_tar), "-C", str(source)], check=True)
        version = re.search(
            r'^version = "(\d+\.\d+\.\d+)"$',
            (source / "Cargo.toml").read_text(encoding="utf-8"), re.M,
        )
        if version is None:
            raise ValueError("Cannot read the pinned application version")
        stage = scratch / "package"
        stage.mkdir()
        archive = stage / f"Variax-Workbench-{version[1]}-windows-x64.zip"
        build_info = (
            f"Commit: {revision}\nBuild number {build_number}\n"
            f"Release target: {GNU_TARGET}\nFeatures: {FEATURES}\n"
            f"Build origin: {GNU_BUILD_ORIGIN}\nVerification: {GNU_VERIFICATION}\n"
            f"Rust flags: {env['RUSTFLAGS']}\n{rustc}"
        )
        # Keep the linked executables stable until the ZIP has read both of them.
        with (target_dir / ".owner-windows-gnu.lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            subprocess.run(
                ["rustup", "run", "stable", "cargo", "build", "--offline", "--locked",
                 "--release", "--target", GNU_TARGET,
                 "--no-default-features", "--features", FEATURES,
                 *(arg for exe in APPS.values() for arg in ("--bin", Path(exe).stem))],
                env=env, cwd=source, check=True,
            )
            verification = build_zip(
                archive, target_dir / GNU_TARGET / "release",
                source / "packaging/windows/How to open.txt", build_info,
            )
        (stage / "windows-verification.txt").write_text(verification, encoding="utf-8")
        write_manifest(stage / "manifest.json", record(
            stage, "windows", revision, build_number, local_windows=True,
        ))
        # The release driver supplies a fresh destination in its private staging directory.
        stage.rename(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("revision")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    number = os.environ.get("VARIAX_BUILD_NUMBER", "")
    if sys.platform != "linux":
        parser.error("Start the Windows GNU cross-build from Linux.")
    if not re.fullmatch(r"[0-9a-f]{40}", args.revision) or not re.fullmatch(r"[1-9][0-9]*", number):
        parser.error("The Windows GNU build needs a full pinned commit and build number.")
    output = args.output.absolute()
    if output.exists() or output.is_symlink():
        parser.error(f"Output already exists: {output}")
    try:
        build(args.revision, number, output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
