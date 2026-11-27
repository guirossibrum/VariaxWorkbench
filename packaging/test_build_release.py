"""Exercise the release entry point with local Git and fake release workers."""
import json
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory
import unittest

from release_manifest import PROFILES, TARGET_MARKERS, record, sha256, verify


RUN_ID = "12345"
DMG = "Variax Workbench - Owner USB.dmg"
SCRIPTS = Path(__file__).resolve().parent


class ReleaseCommandTests(unittest.TestCase):
    def setUp(self):
        scratch = TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.git("init", "-q", "-b", "master")
        self.git("config", "user.email", "test@example.invalid")
        self.git("config", "user.name", "Release test")
        self.git("-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "-m", "base")
        self.revision = self.git("rev-parse", "HEAD")
        self.git("branch", "owner-pin")
        self.git("-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "-m", "next")
        self.git("remote", "add", "origin", str(self.repo))

        packaging = self.repo / "packaging"
        packaging.mkdir()
        for script in ("build-release.sh", "release_manifest.py"):
            shutil.copyfile(SCRIPTS / script, packaging / script)
        (packaging / "build-private-owner-usb.sh").write_text(
            r"""set -euo pipefail
printf 'private\n' >> "$TEST_CALLS"
test "$1" = "$TEST_REVISION"
test "$2" = test-mac
test "$4" = 1
if [[ $# == 4 ]]; then
    test "$3" = "$TEST_OUTPUT"
    exit
fi
test "$5" = --stage-only
test "$(git rev-parse owner-pin)" != "$1"
cp -a "$TEST_PAIR" "$3/pair"
if [[ "$TEST_FAILURE" == private ]]; then
    exit 1
fi
if [[ "$TEST_FAILURE" == collision ]]; then
    destination="$TEST_OUTPUT/variax-workbench-owner-build-$4-${1:0:8}-${3##*.}"
    mkdir "$destination"
    touch "$destination/existing"
fi
""",
            encoding="utf-8",
        )

        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        gh = bin_dir / "gh"
        gh.write_text(
            r"""#!/bin/bash
set -euo pipefail
case "$1 $2" in
    'workflow run')
        printf 'dispatch\n' >> "$TEST_CALLS"
        test "$3" = release-package.yml
        test "$4 $5" = '--ref master'
        test "$6 $7" = "--raw-field revision=$TEST_REVISION"
        test "$8 $9" = "--raw-field alpha_number=$TEST_ALPHA"
        test "${10}" = --raw-field
        [[ "${11}" == "request_id=$TEST_REVISION-"* ]]
        git update-ref refs/heads/owner-pin HEAD
        printf 'https://example.invalid/actions/runs/%s\n' "$TEST_RUN_ID"
        ;;
    'run watch')
        printf 'watch\n' >> "$TEST_CALLS"
        test "$*" = "run watch $TEST_RUN_ID --exit-status --compact"
        [[ "$TEST_FAILURE" != hosted ]]
        ;;
    'run download')
        printf 'download\n' >> "$TEST_CALLS"
        test "$3 $4 $5 $6" = "$TEST_RUN_ID --name release-set-$TEST_REVISION --dir"
        cp -a "$TEST_HOSTED" "$7"
        [[ "$TEST_FAILURE" != download ]]
        ;;
    *) exit 1 ;;
esac
""",
            encoding="utf-8",
        )
        gh.chmod(0o755)
        for command in ("ssh", "scp"):
            executable = bin_dir / command
            executable.write_text("#!/bin/bash\nexit 99\n", encoding="utf-8")
            executable.chmod(0o755)

        self.output = self.root / "owner delivery"
        self.calls = self.root / "calls"
        self.hosted = self.root / "hosted"
        self.private_pair = self.root / "private"
        self.make_hosted_set()
        self.make_private_pair()
        self.env = {
            **os.environ,
            "PATH": f"{bin_dir}:{os.environ['PATH']}",
            "TEST_REVISION": self.revision,
            "TEST_RUN_ID": RUN_ID,
            "TEST_CALLS": str(self.calls),
            "TEST_HOSTED": str(self.hosted),
            "TEST_PAIR": str(self.private_pair),
            "TEST_OUTPUT": str(self.output),
            "TEST_ALPHA": "",
            "TEST_FAILURE": "",
        }

    def git(self, *args):
        return subprocess.check_output(
            ["git", *args], cwd=self.repo, text=True,
        ).strip()

    def make_hosted_set(self):
        for platform, name in (("linux", "linux.tar.gz"), ("macos", "mac.dmg"), ("windows", "windows.zip")):
            directory = self.hosted / platform
            directory.mkdir(parents=True)
            package = directory / name
            package.write_bytes(b"hosted native test fixture")
            digest = sha256(package)
            checksum = f"SHA-256: {digest}  {name}" if platform == "windows" else f"Package SHA-256 {digest}"
            (directory / f"{platform}-verification.txt").write_text("\n".join((
                f"Commit {self.revision}", "Build number 1", PROFILES[platform],
                *TARGET_MARKERS[platform], checksum,
            )), encoding="utf-8")
            (directory / "manifest.json").write_text(
                json.dumps(record(directory, platform, self.revision, "1")), encoding="utf-8",
            )
        (self.hosted / "release-manifest.json").write_text(
            json.dumps(verify(self.hosted, self.revision, "1")), encoding="utf-8",
        )

    def make_private_pair(self):
        pair = self.private_pair
        for name in (
            "linux/rig-check", "linux/variax-workbench-gui", "linux/catalogue.md",
            "linux/coverage.md", "linux/release-notes.md", "linux/assets/ui/SOURCES.md",
            "linux/assets/ui/shell/fixture.png", "linux/assets/ui/pod-farm/fixture.png",
            f"macos/{DMG}",
        ):
            path = pair / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"private worker test fixture")
        for platform in ("linux", "macos"):
            lines = [
                f"Commit {self.revision}", "Build number 1", "Profile owner-usb (gui,portable-math,usb)",
                *TARGET_MARKERS[platform],
            ]
            if platform == "macos":
                lines.append(f"Package SHA-256 {sha256(pair / platform / DMG)}")
            (pair / platform / "verification.txt").write_text("\n".join(lines), encoding="utf-8")
        (pair / "REVISION").write_text(self.revision + "\n", encoding="utf-8")
        (pair / "BUILD_NUMBER").write_text("1\n", encoding="utf-8")
        self.write_pair_checksums()

    def write_pair_checksums(self):
        pair = self.private_pair
        (pair / "SHA256SUMS").write_text("".join(
            f"{sha256(path)}  {path.relative_to(pair).as_posix()}\n"
            for path in sorted(pair.rglob("*")) if path.is_file() and path.name != "SHA256SUMS"
        ), encoding="utf-8")

    def run_release(self, *args, mode="--owner-delivery"):
        options = [mode, "--mac-host", "test-mac", "--output", str(self.output)] if mode else []
        return subprocess.run(
            ["/bin/bash", "packaging/build-release.sh", "--revision", "owner-pin", *options, *args],
            cwd=self.repo, env=self.env, text=True, capture_output=True, check=False, timeout=10,
        )

    def assert_calls(self, *calls):
        actual = self.calls.read_text(encoding="utf-8").splitlines() if self.calls.exists() else []
        self.assertEqual(actual, list(calls))

    def assert_failed_without_delivery(self, result, *calls):
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assert_calls(*calls)
        self.assertEqual(list(self.output.glob("*")), [])
        self.assertNotIn("Owner delivery (", result.stdout)

    def test_delivers_one_set_with_the_original_pin(self):
        result = self.run_release()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_calls("dispatch", "watch", "download", "private")
        delivery, = self.output.iterdir()
        manifest = json.loads((delivery / "release-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["hosted_artifact_run_id"], int(RUN_ID))
        self.assertEqual(manifest["revision"], self.revision)
        self.assertEqual(manifest["build_number"], 1)
        self.assertIn("Offline only", manifest["capabilities"]["windows"])
        self.assertIn("approved live USB", manifest["capabilities"]["macos"])
        self.assertEqual(set(manifest["platforms"]), {"linux", "macos", "windows"})
        for platform, entry in manifest["platforms"].items():
            expected_profile = "offline" if platform == "windows" else "owner-usb"
            self.assertEqual((entry["profile"], entry["revision"], entry["build_number"]),
                             (expected_profile, self.revision, 1))
            for name, digest in entry["files"].items():
                self.assertEqual(sha256(delivery / platform / name), digest)
        self.assertFalse((delivery / "mac-download.txt").exists())

    def test_wrong_windows_revision_build_profile_or_target(self):
        report = self.hosted / "windows/windows-verification.txt"
        original = report.read_text(encoding="utf-8")
        for line, replacement in (
            (f"Commit {self.revision}", f"Commit {'f' * 40}"),
            ("Build number 1", "Build number 2"),
            (PROFILES["windows"], "Features: gui,portable-math,usb"),
            (TARGET_MARKERS["windows"][0], "Release target: x86_64-unknown-linux-gnu"),
        ):
            with self.subTest(line=line):
                self.git("update-ref", "refs/heads/owner-pin", self.revision)
                self.calls.unlink(missing_ok=True)
                report.write_text(original.replace(line, replacement), encoding="utf-8")
                self.assert_failed_without_delivery(self.run_release(), "dispatch", "watch", "download")

    def test_wrong_mac_build_after_successful_hosted_run(self):
        report = self.private_pair / "macos/verification.txt"
        report.write_text(report.read_text(encoding="utf-8").replace(
            "Build number 1", "Build number 2",
        ), encoding="utf-8")
        self.write_pair_checksums()
        result = self.run_release()
        self.assert_failed_without_delivery(result, "dispatch", "watch", "download", "private")
        self.assertIn("macos: expected verification entry Build number 1", result.stderr)

    def test_corrupt_windows_zip_stops_before_private_build(self):
        (self.hosted / "windows/windows.zip").write_bytes(b"corrupted")
        self.assert_failed_without_delivery(self.run_release(), "dispatch", "watch", "download")

    def test_corrupt_private_binary_leaves_no_delivery(self):
        (self.private_pair / "linux/rig-check").write_bytes(b"corrupted")
        result = self.run_release()
        self.assert_failed_without_delivery(result, "dispatch", "watch", "download", "private")
        self.assertIn("SHA256SUMS does not match", result.stderr)

    def test_failed_hosted_run_skips_artifact_fetch_and_private_build(self):
        self.env["TEST_FAILURE"] = "hosted"
        self.assert_failed_without_delivery(self.run_release(), "dispatch", "watch")

    def test_failed_artifact_download_cleans_the_partial_download(self):
        self.env["TEST_FAILURE"] = "download"
        self.assert_failed_without_delivery(self.run_release(), "dispatch", "watch", "download")

    def test_failed_private_worker_cleans_its_output(self):
        self.env["TEST_FAILURE"] = "private"
        self.assert_failed_without_delivery(self.run_release(), "dispatch", "watch", "download", "private")

    def test_output_collision_preserves_existing_directory(self):
        self.env["TEST_FAILURE"] = "collision"
        result = self.run_release()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("Could not publish the owner set", result.stderr)
        self.assert_calls("dispatch", "watch", "download", "private")
        delivery, = self.output.iterdir()
        self.assertEqual([path.name for path in delivery.iterdir()], ["existing"])

    def test_combined_mode_refuses_alpha_publication(self):
        self.assert_failed_without_delivery(self.run_release("--alpha-number", "7"))

    def test_standalone_hosted_alpha_uses_existing_publication_flag(self):
        self.env["TEST_ALPHA"] = "7"
        result = self.run_release("--alpha-number", "7", mode="")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_calls("dispatch", "watch")
        self.assertFalse(self.output.exists())

    def test_standalone_owner_usb_still_calls_the_pair_worker_in_delivery_mode(self):
        result = self.run_release(mode="--owner-usb")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_calls("private")


if __name__ == "__main__":
    unittest.main()
