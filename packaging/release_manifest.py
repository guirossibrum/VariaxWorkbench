"""Record and verify a complete native release set before delivery."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re

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


def record(directory, platform, revision, build_number=None):
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
    for target_line in TARGET_MARKERS[platform]:
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
        "targets": TARGETS[platform],
        "files": {package.name: package_digest, report_path.name: sha256(report_path)},
    }
    if build_number is not None:
        manifest["build_number"] = int(build_number)
    return manifest


def verify_worker(worker_dir, platform, revision, build_number):
    manifest_path = worker_dir / "manifest.json"
    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise ValueError(f"{platform}: missing or invalid worker manifest")
    recorded = json.loads(manifest_path.read_text(encoding="utf-8"))
    actual = record(worker_dir, platform, revision, build_number)
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


def verify_owner_delivery(directory, revision, build_number, run_id):
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

    platforms["windows"] = verify_worker(directory / "windows", "windows", revision, build_number)
    return {
        "revision": revision,
        "build_number": int(build_number),
        "hosted_artifact_run_id": int(run_id),
        "platforms": platforms,
        "capabilities": {
            "linux": "Offline editing, Rig Check Practice, and approved live USB checks.",
            "macos": "Offline editing, Rig Check Practice, and approved live USB checks.",
            "windows": "Offline only: UI, file operations, and Rig Check Practice. Live USB unavailable.",
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("revision")
    parser.add_argument("directory", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--record", choices=PACKAGES)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--owner-delivery", metavar="HOSTED_RUN_ID")
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
    try:
        if args.record:
            manifest = record(args.directory, args.record, args.revision, build_number)
            name = "manifest.json"
        elif args.owner_delivery:
            manifest = verify_owner_delivery(
                args.directory, args.revision, build_number, args.owner_delivery,
            )
            name = "release-manifest.json"
        else:
            manifest = verify(args.directory, args.revision, build_number)
            name = "release-manifest.json"
        manifest_path = args.directory / name
        if args.check:
            reported = json.loads(manifest_path.read_text(encoding="utf-8"))
            if reported != manifest:
                raise ValueError("release set differs from the combined manifest")
            return
        with manifest_path.open("x", encoding="utf-8", newline="\n") as stream:
            json.dump(manifest, stream, indent=2, sort_keys=True)
            stream.write("\n")
    except (OSError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
