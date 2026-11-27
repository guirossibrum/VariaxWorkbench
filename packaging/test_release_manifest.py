"""Offline release-set checks. Packages in these fixtures are synthetic."""
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from release_manifest import PROFILES, TARGET_MARKERS, record, sha256, verify

REVISION = "a" * 40
OTHER = "b" * 40
NAMES = {"linux": "linux.tar.gz", "macos": "mac.dmg", "windows": "windows.zip"}


class ReleaseManifestTests(unittest.TestCase):
    def setUp(self):
        scratch = TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.directory = Path(scratch.name)
        for platform, filename in NAMES.items():
            worker = self.directory / platform
            worker.mkdir()
            (worker / filename).write_bytes(b"native package fixture")
            prefix = "Commit: " if platform == "windows" else "Commit "
            digest = sha256(worker / filename)
            native = f"SHA-256: {digest}  {filename}" if platform == "windows" else f"Package SHA-256 {digest}"
            lines = (prefix + REVISION, PROFILES[platform], *TARGET_MARKERS[platform], native)
            (worker / f"{platform}-verification.txt").write_text("\n".join(lines), encoding="utf-8")
            manifest = record(worker, platform, REVISION)
            (worker / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")

    def test_all_platforms_must_match(self):
        result = verify(self.directory, REVISION)
        self.assertEqual(result["revision"], REVISION)
        self.assertEqual(set(result["platforms"]), set(NAMES))

    def test_mismatched_revision_is_rejected(self):
        report = self.directory / "macos" / "macos-verification.txt"
        report.write_text(report.read_text(encoding="utf-8").replace(REVISION, OTHER), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "pinned commit"):
            verify(self.directory, REVISION)

    def test_missing_package_is_rejected(self):
        (self.directory / "windows" / NAMES["windows"]).unlink()
        with self.assertRaisesRegex(ValueError, "expected exactly one"):
            verify(self.directory, REVISION)

    def test_mismatched_profile_or_target_is_rejected(self):
        worker = self.directory / "linux"
        report = worker / "linux-verification.txt"
        original = report.read_text(encoding="utf-8")
        report.write_text(original.replace("offline", "owner-usb"), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unexpected build profile"):
            verify(self.directory, REVISION)
        report.write_text(original.replace("linux-gnu", "linux-gnu-other"), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "missing target metadata"):
            verify(self.directory, REVISION)

    def test_changed_package_is_rejected(self):
        (self.directory / "linux" / NAMES["linux"]).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "differs from native verification"):
            verify(self.directory, REVISION)

    def test_replaced_package_before_worker_record_is_rejected(self):
        for platform, filename in NAMES.items():
            with self.subTest(platform=platform):
                worker = self.directory / platform
                (worker / filename).write_bytes(b"wrong package")
                with self.assertRaisesRegex(ValueError, "differs from native verification"):
                    record(worker, platform, REVISION)

    def test_second_reported_revision_is_rejected(self):
        report = self.directory / "linux" / "linux-verification.txt"
        with report.open("a", encoding="utf-8") as stream:
            stream.write(f"\nCommit {OTHER}\n")
        with self.assertRaisesRegex(ValueError, "pinned commit"):
            verify(self.directory, REVISION)


if __name__ == "__main__":
    unittest.main()
