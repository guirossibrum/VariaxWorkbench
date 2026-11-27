"""Exercise the release entry point with local Git and fake release workers."""
import json
import os
from pathlib import Path
import shutil
import subprocess
from tempfile import TemporaryDirectory
import unittest

from release_manifest import OWNER_DMG, PROFILES, TARGET_MARKERS, record, sha256, verify
from windows.package import APPS, GNU_BUILD_ORIGIN, GNU_TARGET, GNU_VERIFICATION, build_zip
from windows.pe_fixture import executable


RUN_ID = "12345"
RUSTC_COMMIT = "a" * 40
DMG = OWNER_DMG
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
        for script in ("build-release.sh", "deliver-owner-dmg.sh", "release_manifest.py"):
            shutil.copyfile(SCRIPTS / script, packaging / script)
        (packaging / "windows").mkdir()
        shutil.copyfile(SCRIPTS / "windows/package.py", packaging / "windows/package.py")
        (packaging / "build-windows-gnu.py").write_text(
            """import os
from pathlib import Path
import shutil
import subprocess
import sys

env = os.environ
with open(env["TEST_CALLS"], "a") as calls:
    calls.write("gnu\\n")
assert sys.argv[1] == env["TEST_REVISION"]
assert env["VARIAX_BUILD_NUMBER"] == "1"
subprocess.run(["git", "update-ref", "refs/heads/owner-pin", "HEAD"], check=True)
shutil.copytree(env["TEST_GNU"], sys.argv[2])
if env["TEST_FAILURE"] == "gnu":
    sys.exit(1)
""",
            encoding="utf-8",
        )
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
        remote_bin = self.root / "remote bin"
        remote_bin.mkdir()
        downloads = self.root / "Mac Downloads"
        downloads.mkdir()
        self.downloads = downloads
        for path, script in (
            (bin_dir / "ssh", r"""#!/bin/bash
set -euo pipefail
printf 'transfer\n' >> "$TEST_CALLS"
test "$#" = 7
test "$1 $2 $3 $4 $5 $6" = '-T -o BatchMode=yes -o ConnectTimeout=10 test-mac'
if [[ "$TEST_FAILURE" == ssh ]]; then
    printf 'Mock SSH connection failed\n' >&2
    exit 255
fi
if [[ "$TEST_FAILURE" != legacy-copy ]]; then
    sets=("$TEST_OUTPUT"/variax-workbench-owner-build-*)
    test "${#sets[@]}" = 1
    test -s "${sets[0]}/release-manifest.json"
    test -s "${sets[0]}/windows/windows.zip"
    test -s "${sets[0]}/linux/rig-check"
    test -s "${sets[0]}/macos/Variax Workbench - Owner USB.dmg"
fi
export PATH="$TEST_REMOTE_BIN:$PATH"
command=${7//'$HOME/Downloads'/'$TEST_DOWNLOADS'}
case "$TEST_FAILURE" in
    corrupt-transfer) printf 'corrupted DMG' | /bin/sh -c "$command" ;;
    lost-ack) /bin/sh -c "$command" >/dev/null; exit 255 ;;
    *) /bin/sh -c "$command" ;;
esac
"""),
            (bin_dir / "scp", "#!/bin/bash\nexit 99\n"),
            (remote_bin / "uname", "#!/bin/bash\nprintf 'Darwin\n'\n"),
            (remote_bin / "shasum", r"""#!/bin/bash
set -euo pipefail
test "$1 $2" = "-a 256"
checksum=$(sha256sum "$3")
digest=${checksum%% *}
if [[ "$TEST_FAILURE" == directory-race && "$3" == */.variax-owner-dmg.* ]]; then
    mkdir "$TEST_DOWNLOADS/Variax Workbench (Build 1 ${TEST_REVISION:0:8} Owner USB ${digest:0:12}).dmg"
fi
printf '%s\n' "$checksum"
"""),
        ):
            path.write_text(script, encoding="utf-8")
            path.chmod(0o755)

        self.output = self.root / "owner delivery"
        self.calls = self.root / "calls"
        self.hosted = self.root / "hosted"
        self.gnu = self.root / "gnu"
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
            "TEST_GNU": str(self.gnu),
            "TEST_PAIR": str(self.private_pair),
            "TEST_OUTPUT": str(self.output),
            "TEST_REMOTE_BIN": str(remote_bin),
            "TEST_DOWNLOADS": str(downloads),
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

    def make_gnu_package(self, *, revision=None, build_number="1", target=GNU_TARGET):
        self.gnu.mkdir()
        payload = self.root / "windows-payload"
        payload.mkdir()
        for name in APPS.values():
            (payload / name).write_bytes(executable())
        instructions = payload / "How to open.txt"
        instructions.write_text("Synthetic fixture", encoding="utf-8")
        report = build_zip(self.gnu / "windows.zip", payload, instructions, "\n".join((
            f"Commit: {revision or self.revision}", f"Build number {build_number}",
            f"Release target: {target}", PROFILES["windows"],
            f"Build origin: {GNU_BUILD_ORIGIN}", f"Verification: {GNU_VERIFICATION}",
            "rustc 1.99.1 (aaaaaaaaa 2026-10-13)", f"commit-hash: {RUSTC_COMMIT}",
        )))
        (self.gnu / "windows-verification.txt").write_text(report, encoding="utf-8")

    def record_gnu_package(self):
        (self.gnu / "manifest.json").write_text(json.dumps(record(
            self.gnu, "windows", self.revision, "1", local_windows=True,
        )), encoding="utf-8")

    def run_release(self, *args, mode="--owner-delivery-hosted"):
        options = [mode, "--mac-host", "test-mac", "--output", str(self.output)] if mode else []
        return subprocess.run(
            ["/bin/bash", "packaging/build-release.sh", "--revision", "owner-pin", *options, *args],
            cwd=self.repo, env=self.env, text=True, capture_output=True, check=False, timeout=10,
        )

    def retry_dmg(self, delivery):
        return subprocess.run(
            ["/bin/bash", str(self.repo / "packaging/build-release.sh"),
             "--retry-owner-dmg", str(delivery), "--mac-host", "test-mac"],
            cwd=self.root, env=self.env, text=True, capture_output=True, check=False, timeout=10,
        )

    def expected_download(self):
        digest = sha256(self.private_pair / "macos" / DMG)
        return self.downloads / f"Variax Workbench (Build 1 {self.revision[:8]} Owner USB {digest[:12]}).dmg"

    def assert_transfer_failure_retains_set(self, result):
        self.assertNotEqual(result.returncode, 0)
        self.assert_calls("gnu", "private", "transfer")
        delivery, = self.output.iterdir()
        self.assertTrue((delivery / "release-manifest.json").is_file())
        self.assertIn(f"Verified local set retained at {delivery}", result.stderr)
        self.assertIn("Retry transfer only:", result.stderr)
        self.assertNotIn("Mac DMG verified at", result.stdout)
        self.assertFalse(list(self.downloads.glob(".variax-owner-dmg.*")))
        return delivery

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

    def test_local_set_keeps_the_original_pin_without_actions(self):
        self.make_gnu_package()
        self.record_gnu_package()
        self.git("remote", "remove", "origin")
        result = self.run_release(mode="--owner-delivery")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_calls("gnu", "private", "transfer")
        delivery, = self.output.iterdir()
        manifest = json.loads((delivery / "release-manifest.json").read_text(encoding="utf-8"))
        self.assertNotIn("hosted_artifact_run_id", manifest)
        self.assertEqual((manifest["revision"], manifest["build_number"]), (self.revision, 1))
        self.assertEqual(set(manifest["platforms"]), {"linux", "macos", "windows"})
        windows = manifest["platforms"]["windows"]
        self.assertEqual(windows["targets"], [GNU_TARGET])
        self.assertEqual(windows["build_origin"], GNU_BUILD_ORIGIN)
        self.assertEqual(windows["verification"], GNU_VERIFICATION)
        self.assertTrue(windows["rustc"].startswith("rustc 1.99.1 "))
        self.assertEqual(windows["rustc_commit"], RUSTC_COMMIT)
        self.assertEqual(windows["features"], ["gui", "portable-math"])
        self.assertIn("Offline only", manifest["capabilities"]["windows"])
        self.assertIn("approved live USB", manifest["capabilities"]["macos"])
        detached = delivery / "windows/windows.zip.sha256"
        self.assertEqual(windows["files"][detached.name], sha256(detached))
        self.assertFalse((delivery / "mac-download.txt").exists())
        downloaded, = self.downloads.iterdir()
        self.assertEqual(downloaded, self.expected_download())
        self.assertEqual(sha256(downloaded), manifest["platforms"]["macos"]["files"][DMG])
        self.assertIn(f"Mac DMG verified at test-mac:{downloaded}", result.stdout)
        recheck = subprocess.run(
            ["python3", str(SCRIPTS / "release_manifest.py"), self.revision, str(delivery),
             "--check-local-owner-delivery"],
            env={**self.env, "VARIAX_BUILD_NUMBER": "1"}, text=True, capture_output=True,
            check=False, timeout=10,
        )
        self.assertEqual(recheck.returncode, 0, recheck.stderr)

    def test_ssh_failure_allows_transfer_only_retry_from_outside_git(self):
        self.make_gnu_package()
        self.record_gnu_package()
        self.env["TEST_FAILURE"] = "ssh"
        result = self.run_release(mode="--owner-delivery")
        delivery = self.assert_transfer_failure_retains_set(result)
        self.assertIn("Mock SSH connection failed", result.stderr)
        self.assertEqual(list(self.downloads.iterdir()), [])
        self.git("remote", "remove", "origin")
        shutil.rmtree(self.private_pair)
        shutil.rmtree(self.gnu)
        self.env["TEST_FAILURE"] = ""
        result = self.retry_dmg(delivery)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_calls("gnu", "private", "transfer", "transfer")
        downloaded, = self.downloads.iterdir()
        self.assertEqual(downloaded.read_bytes(), (delivery / "macos" / DMG).read_bytes())

    def test_changed_saved_set_is_refused_before_retry_contacts_mac(self):
        self.make_gnu_package()
        self.record_gnu_package()
        self.env["TEST_FAILURE"] = "ssh"
        delivery = self.assert_transfer_failure_retains_set(self.run_release(mode="--owner-delivery"))
        self.env["TEST_FAILURE"] = ""
        for name in ("linux/rig-check", f"macos/{DMG}", "windows/windows.zip", "release-manifest.json"):
            with self.subTest(name=name):
                path = delivery / name
                contents = path.read_bytes()
                path.write_bytes(b"changed")
                result = self.retry_dmg(delivery)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assert_calls("gnu", "private", "transfer")
                path.write_bytes(contents)

    def test_corrupt_stream_keeps_downloads_empty(self):
        self.make_gnu_package()
        self.record_gnu_package()
        self.env["TEST_FAILURE"] = "corrupt-transfer"
        result = self.run_release(mode="--owner-delivery")
        self.assert_transfer_failure_retains_set(result)
        self.assertIn("staged DMG checksum mismatch", result.stderr)
        self.assertEqual(list(self.downloads.iterdir()), [])

    def test_destination_collision_preserves_existing_file(self):
        self.make_gnu_package()
        self.record_gnu_package()
        destination = self.expected_download()
        destination.write_bytes(b"existing contents")
        result = self.run_release(mode="--owner-delivery")
        self.assert_transfer_failure_retains_set(result)
        self.assertIn(f"destination already exists: {destination}", result.stderr)
        self.assertEqual(destination.read_bytes(), b"existing contents")

    def test_directory_created_during_transfer_receives_no_file(self):
        self.make_gnu_package()
        self.record_gnu_package()
        self.env["TEST_FAILURE"] = "directory-race"
        result = self.run_release(mode="--owner-delivery")
        self.assert_transfer_failure_retains_set(result)
        self.assertIn("Could not create", result.stderr)
        self.assertEqual(list(self.downloads.iterdir()), [self.expected_download()])
        self.assertEqual(list(self.expected_download().iterdir()), [])

    def test_shared_helper_copies_the_existing_remote_dmg_for_the_usb_pair(self):
        source = self.root / "Mac builder's DMG"
        source.write_bytes(b"remote builder's verified bytes")
        filename = "Variax Workbench (standalone Owner USB).dmg"
        result = subprocess.run(
            ["/bin/bash", "-c", 'source "$1"; deliver_owner_dmg test-mac "$2" "$3" "$4"', "--",
             str(SCRIPTS / "deliver-owner-dmg.sh"), filename, sha256(source), str(source)],
            env={**self.env, "TEST_FAILURE": "legacy-copy"}, text=True, capture_output=True,
            check=False, timeout=10, stdin=subprocess.DEVNULL,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_calls("transfer")
        destination, = self.downloads.iterdir()
        self.assertEqual(destination, self.downloads / filename)
        self.assertEqual(result.stdout.strip(), str(destination))
        self.assertEqual(destination.read_bytes(), source.read_bytes())

    def test_retry_accepts_a_verified_copy_after_lost_ssh_acknowledgment(self):
        self.make_gnu_package()
        self.record_gnu_package()
        self.env["TEST_FAILURE"] = "lost-ack"
        delivery = self.assert_transfer_failure_retains_set(self.run_release(mode="--owner-delivery"))
        destination, = self.downloads.iterdir()
        inode = destination.stat().st_ino
        self.env["TEST_FAILURE"] = ""
        result = self.retry_dmg(delivery)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assert_calls("gnu", "private", "transfer", "transfer")
        self.assertEqual(destination, self.expected_download())
        self.assertEqual(destination.stat().st_ino, inode)
        self.assertEqual(sha256(destination), sha256(delivery / "macos" / DMG))

    def test_local_worker_failure_cleans_staged_output_and_skips_pair(self):
        self.make_gnu_package()
        self.env["TEST_FAILURE"] = "gnu"
        self.assert_failed_without_delivery(self.run_release(mode="--owner-delivery"), "gnu")

    def test_local_pair_failure_cleans_staged_windows_package(self):
        self.make_gnu_package()
        self.record_gnu_package()
        self.env["TEST_FAILURE"] = "private"
        self.assert_failed_without_delivery(self.run_release(mode="--owner-delivery"), "gnu", "private")

    def test_local_rejects_tampered_windows_checksum_or_report(self):
        self.make_gnu_package()
        self.record_gnu_package()
        for name, bad_content, message in (
            ("windows.zip.sha256", "bad digest", "invalid ZIP checksum"),
            ("windows-verification.txt", "Fake runtime claim", "report differs from verified ZIP"),
        ):
            with self.subTest(name=name):
                self.git("update-ref", "refs/heads/owner-pin", self.revision)
                self.calls.unlink(missing_ok=True)
                path = self.gnu / name
                original = path.read_bytes()
                with path.open("a", encoding="utf-8") as stream:
                    stream.write(f"{bad_content}\n")
                result = self.run_release(mode="--owner-delivery")
                self.assert_failed_without_delivery(result, "gnu", "private")
                self.assertIn(message, result.stderr)
                path.write_bytes(original)

    def test_local_requires_gnu_package_from_same_revision_and_build(self):
        for options, message in (
            ({"target": "x86_64-pc-windows-msvc"}, "missing target metadata"),
            ({"revision": "f" * 40}, "pinned commit"),
            ({"build_number": "2"}, "does not identify build 1"),
        ):
            with self.subTest(options=options):
                self.git("update-ref", "refs/heads/owner-pin", self.revision)
                self.calls.unlink(missing_ok=True)
                self.make_gnu_package(**options)
                shutil.copyfile(self.hosted / "windows/manifest.json", self.gnu / "manifest.json")
                result = self.run_release(mode="--owner-delivery")
                self.assert_failed_without_delivery(result, "gnu", "private")
                self.assertIn(message, result.stderr)
                shutil.rmtree(self.gnu)
                shutil.rmtree(self.root / "windows-payload")

    def test_hosted_windows_manifest_rejects_gnu_metadata(self):
        self.make_gnu_package()
        with self.assertRaisesRegex(ValueError, "missing target metadata"):
            record(self.gnu, "windows", self.revision, "1")

    def test_local_mode_refuses_alpha_publication(self):
        self.assert_failed_without_delivery(self.run_release("--alpha-number", "7", mode="--owner-delivery"))

    def test_gnu_worker_rejects_stable_compiler_drift_before_building(self):
        rustup = self.root / "bin/rustup"
        stable = self.root / "bin/test-stable-rustc"
        rustup.write_text(
            """#!/bin/bash
set -eu
case "$*" in
    'run 1.99.1 rustc -vV') printf 'commit-hash: %s\\n' "$TEST_PINNED_COMPILER" ;;
    'which --toolchain stable rustc') printf '%s\\n' "$TEST_STABLE_RUSTC" ;;
    *) touch "$TEST_CALLS"; exit 99 ;;
esac
""",
            encoding="utf-8",
        )
        stable.write_text(
            "#!/bin/bash\ntest \"$*\" = -vV || exit 99\nprintf 'commit-hash: %s\\n' \"$TEST_STABLE_COMPILER\"\n",
            encoding="utf-8",
        )
        for command in (rustup, stable):
            command.chmod(0o755)
        target_dir = self.root / "cargo-target"
        result = subprocess.run(
            ["python3", str(SCRIPTS / "build-windows-gnu.py"), self.revision, str(self.gnu)],
            cwd=self.repo, text=True, capture_output=True, check=False, timeout=10,
            env={**self.env, "VARIAX_BUILD_NUMBER": "1", "CARGO_TARGET_DIR": str(target_dir),
                 "TEST_PINNED_COMPILER": RUSTC_COMMIT, "TEST_STABLE_COMPILER": "b" * 40,
                 "TEST_STABLE_RUSTC": str(stable)},
        )
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("differs from pinned Rust 1.99.1 compiler", result.stderr)
        self.assert_calls()
        self.assertFalse(target_dir.exists())
        self.assertFalse(self.gnu.exists())


if __name__ == "__main__":
    unittest.main()
