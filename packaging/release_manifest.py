"""Record and verify a complete release set before delivery."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
from zipfile import BadZipFile

from windows.package import GNU_BUILD_ORIGIN, GNU_TARGET, GNU_VERIFICATION, compiler_commit, verify_zip

PACKAGES = {"linux": "*.tar.gz", "macos": "*.dmg", "windows": "*.zip"}
TARGETS = {
    "linux": ["x86_64-unknown-linux-gnu"],
    "macos": ["aarch64-apple-darwin", "x86_64-apple-darwin"],
    "windows": ["x86_64-pc-windows-msvc"],
}
TARGET_MARKERS = {
    "linux": ("Target x86_64-unknown-linux-gnu",),
    "macos": ("aarch64-apple-darwin MACOSX_DEPLOYMENT_TARGET=11.0",
              "x86_64-apple-darwin MACOSX_DEPLOYMENT_TARGET=10.15"),
    "windows": ("Release target: x86_64-pc-windows-msvc",),
}
PROFILES = {
    "linux": "Profile offline (gui,portable-math)",
    "macos": "Profile offline (gui,portable-math)",
    "windows": "Features: gui,portable-math",
}


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def record(directory, platform, revision, build_number=None, *, local_windows=False):
    report_path = directory / f"{platform}-verification.txt"
    packages = sorted(directory.glob(PACKAGES[platform]))
    if len(packages) != 1:
        raise ValueError(f"{platform}: expected exactly one native package")
    for path in packages + [report_path]:
        if path.is_symlink() or not path.is_file() or not path.stat().st_size:
            raise ValueError(f"{platform}: missing or invalid {path.name}")

    report = report_path.read_text(encoding="utf-8")
    lines = report.splitlines()
    commits = re.findall(r"^Commit(?:: | )([0-9a-f]{40})$", report, flags=re.MULTILINE)
    if commits != [revision]:
        raise ValueError(f"{platform}: report does not identify the pinned commit {revision}")
    if build_number is not None and lines.count(f"Build number {build_number}") != 1:
        raise ValueError(f"{platform}: report does not identify build {build_number}")
    if PROFILES[platform] not in lines:
        raise ValueError(f"{platform}: unexpected build profile")
    targets = [GNU_TARGET] if local_windows else TARGETS[platform]
    markers = (f"Release target: {GNU_TARGET}",) if local_windows else TARGET_MARKERS[platform]
    for target_line in markers:
        if target_line not in lines:
            raise ValueError(f"{platform}: missing target metadata {target_line}")

    package = packages[0]
    package_digest = sha256(package)
    pattern = r"^SHA-256: ([0-9a-f]{64})  .+$" if platform == "windows" else r"^Package SHA-256 ([0-9a-f]{64})$"
    native_digests = re.findall(pattern, report, flags=re.MULTILINE)
    if native_digests != [package_digest]:
        raise ValueError(f"{platform}: package checksum differs from native verification")
    manifest = {
        "platform": platform,
        "revision": revision,
        "profile": "offline",
        "features": ["gui", "portable-math"],
        "targets": targets,
        "files": {package.name: package_digest, report_path.name: sha256(report_path)},
    }
    if build_number is not None:
        manifest["build_number"] = int(build_number)
    if local_windows:
        detached = package.with_suffix(".zip.sha256")
        if detached.is_symlink() or not detached.is_file():
            raise ValueError("windows: missing or invalid detached ZIP checksum")
        metadata = {
            "Commit: ": revision,
            "Build number ": build_number,
            "Features: ": "gui,portable-math",
            "Release target: ": GNU_TARGET,
            "Build origin: ": GNU_BUILD_ORIGIN,
            "Verification: ": GNU_VERIFICATION,
        }
        for prefix, expected in metadata.items():
            if [line for line in lines if line.startswith(prefix)] != [prefix + expected]:
                raise ValueError(f"windows: expected verification entry {prefix}{expected}")
        verified_report = verify_zip(package, detached)
        if report != verified_report:
            raise ValueError("windows: report differs from verified ZIP metadata and checksum")
        versions = [line for line in lines if line.startswith("rustc ")]
        if len(versions) != 1:
            raise ValueError("windows: expected one Rust compiler version")
        manifest.update(
            build_origin=GNU_BUILD_ORIGIN, verification=GNU_VERIFICATION,
            rustc=versions[0], rustc_commit=compiler_commit(report),
        )
        manifest["files"][detached.name] = sha256(detached)
    return manifest


def verify_worker(worker_dir, platform, revision, build_number, *, local_windows=False):
    manifest_path = worker_dir / "manifest.json"
    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise ValueError(f"{platform}: missing or invalid worker manifest")
    recorded = json.loads(manifest_path.read_text(encoding="utf-8"))
    actual = record(worker_dir, platform, revision, build_number, local_windows=local_windows)
    if recorded != actual:
        raise ValueError(f"{platform}: worker manifest does not match the verified files")
    return actual


def verify(directory, revision, build_number=None):
    results = {
        platform: verify_worker(directory / platform, platform, revision, build_number)
        for platform in PACKAGES
    }
    manifest = {"revision": revision, "platforms": results}
    if build_number is not None:
        manifest["build_number"] = int(build_number)
    return manifest


def verify_owner_delivery(directory, revision, build_number, run_id=None):
    features = ["gui", "portable-math", "usb"]
    linux = directory / "linux"
    artwork = linux / "assets/ui"
    dmg_name = "Variax Workbench - Owner USB.dmg"
    artwork_files = [list(artwork.glob(f"{group}/*.png")) for group in ("shell", "pod-farm")]
    if not all(artwork_files):
        raise ValueError("linux: missing bundled artwork")
    files = {
        "linux": [
            linux / name for name in (
                "rig-check", "variax-workbench-gui", "catalogue.md", "coverage.md",
                "release-notes.md", "verification.txt",
            )
        ] + [artwork / "SOURCES.md", *artwork_files[0], *artwork_files[1]],
        "macos": [directory / "macos" / name for name in (dmg_name, "verification.txt")],
    }
    checksums = (directory / "SHA256SUMS").read_text(encoding="utf-8").splitlines()
    actual_checksums = []
    for name, expected in (("REVISION", revision), ("BUILD_NUMBER", build_number)):
        path = directory / name
        if path.read_text(encoding="utf-8") != expected + "\n":
            raise ValueError(f"owner pair: unexpected {name}")
        actual_checksums.append(f"{sha256(path)}  {name}")

    platforms = {}
    for platform, paths in files.items():
        digests = {}
        for path in paths:
            if path.is_symlink() or not path.is_file() or not path.stat().st_size:
                raise ValueError(f"{platform}: missing or invalid {path.name}")
            digest = sha256(path)
            digests[path.relative_to(directory / platform).as_posix()] = digest
            actual_checksums.append(f"{digest}  {path.relative_to(directory).as_posix()}")
        report = (directory / platform / "verification.txt").read_text(encoding="utf-8")
        lines = report.splitlines()
        metadata = {
            "Commit ": revision,
            "Build number ": build_number,
            "Profile ": f"owner-usb ({','.join(features)})",
        }
        if platform == "macos":
            metadata["Package SHA-256 "] = digests[dmg_name]
        for prefix, expected in metadata.items():
            if [line for line in lines if line.startswith(prefix)] != [prefix + expected]:
                raise ValueError(f"{platform}: expected verification entry {prefix}{expected}")
        for marker in TARGET_MARKERS[platform]:
            if lines.count(marker) != 1:
                raise ValueError(f"{platform}: expected verification entry {marker}")
        platforms[platform] = {
            "platform": platform,
            "revision": revision,
            "build_number": int(build_number),
            "profile": "owner-usb",
            "features": features,
            "targets": TARGETS[platform],
            "files": digests,
        }
    if sorted(actual_checksums) != sorted(checksums):
        raise ValueError("owner pair: SHA256SUMS does not match the verified files")

    platforms["windows"] = verify_worker(
        directory / "windows", "windows", revision, build_number, local_windows=run_id is None,
    )
    manifest = {
        "revision": revision,
        "build_number": int(build_number),
        "platforms": platforms,
        "capabilities": {
            "linux": "Offline editing, Rig Check Practice, and approved live USB checks.",
            "macos": "Offline editing, Rig Check Practice, and approved live USB checks.",
            "windows": "Offline only: UI, file operations, and Rig Check Practice. Live USB unavailable.",
        },
    }
    if run_id is not None:
        manifest["hosted_artifact_run_id"] = int(run_id)
    return manifest


def write_manifest(path, manifest):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(manifest, stream, indent=2, sort_keys=True)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("revision")
    parser.add_argument("directory", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--record", choices=PACKAGES)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--owner-delivery", metavar="HOSTED_RUN_ID")
    mode.add_argument("--local-owner-delivery", action="store_true")
    mode.add_argument("--check-local-owner-delivery", action="store_true")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.revision):
        parser.error("provide a full commit SHA")
    build_number = os.environ.get("VARIAX_BUILD_NUMBER")
    if build_number is not None and not re.fullmatch(r"[1-9][0-9]*", build_number):
        parser.error("invalid release build number")
    if args.owner_delivery is not None and (
        build_number is None or not re.fullmatch(r"[0-9]+", args.owner_delivery)
    ):
        parser.error("owner delivery requires a build number and hosted run ID")
    local_owner = args.local_owner_delivery or args.check_local_owner_delivery
    if local_owner and build_number is None:
        parser.error("local owner delivery requires a build number")
    try:
        if args.record:
            manifest = record(args.directory, args.record, args.revision, build_number)
            name = "manifest.json"
        elif args.owner_delivery or local_owner:
            manifest = verify_owner_delivery(
                args.directory, args.revision, build_number, args.owner_delivery,
            )
            name = "release-manifest.json"
        else:
            manifest = verify(args.directory, args.revision, build_number)
            name = "release-manifest.json"
        manifest_path = args.directory / name
        if args.check or args.check_local_owner_delivery:
            reported = json.loads(manifest_path.read_text(encoding="utf-8"))
            if reported != manifest:
                raise ValueError("release set differs from the combined manifest")
            return
        write_manifest(manifest_path, manifest)
    except (BadZipFile, OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
