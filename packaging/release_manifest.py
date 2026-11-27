"""Record and verify a complete native release set before delivery."""
import argparse
import hashlib
import json
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


def record(directory, platform, revision):
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
    return {
        "platform": platform,
        "revision": revision,
        "profile": "offline",
        "features": ["gui", "portable-math"],
        "targets": TARGETS[platform],
        "files": {package.name: package_digest, report_path.name: sha256(report_path)},
    }


def verify(directory, revision):
    results = {}
    for platform in PACKAGES:
        worker_dir = directory / platform
        manifest_path = worker_dir / "manifest.json"
        if not manifest_path.is_file():
            raise ValueError(f"{platform}: missing worker manifest")
        recorded = json.loads(manifest_path.read_text(encoding="utf-8"))
        actual = record(worker_dir, platform, revision)
        if recorded != actual:
            raise ValueError(f"{platform}: worker manifest does not match the verified files")
        results[platform] = actual
    return {"revision": revision, "platforms": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("revision")
    parser.add_argument("directory", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--record", choices=PACKAGES)
    mode.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.revision):
        parser.error("provide a full commit SHA")
    try:
        if args.record:
            manifest = record(args.directory, args.record, args.revision)
            name = "manifest.json"
        else:
            manifest = verify(args.directory, args.revision)
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
