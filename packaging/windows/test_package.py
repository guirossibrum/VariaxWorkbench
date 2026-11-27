import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from zipfile import ZipFile

from build import smoke
from package import APPS, CONTENTS, CHECKSUMS, build_zip, verify_zip, windows_imports

from pe_fixture import executable

REVISION = "a" * 40
WINDOWS_FIELDS = ["Windows version: 10.0.26100", "Windows build: 26100", "Architecture: x86_64"]


class PackageTests(unittest.TestCase):
    def test_reads_normal_and_delayed_system_imports(self):
        self.assertEqual(
            windows_imports(executable(delay=b"OpenGL32.dll")),
            ["kernel32.dll", "opengl32.dll"],
        )

    def test_refuses_x86_signed_or_truncated_payloads(self):
        for data, reason in (
            (executable(machine=0x14C), "expected an x64"),
            (executable(signed=True), "expected an unsigned"),
            (executable()[:144], "truncated PE"),
        ):
            with self.subTest(reason=reason), self.assertRaisesRegex(ValueError, reason):
                windows_imports(data)

    def test_refuses_non_system_and_separate_runtime_imports(self):
        for kwargs in ({"dll": b"VCRUNTIME140.dll"}, {"delay": b"libusb-1.0.dll"}):
            with self.subTest(kwargs=kwargs), self.assertRaisesRegex(ValueError, "non-system DLL"):
                windows_imports(executable(**kwargs))

    def test_zip_contains_both_apps_with_verified_checksums(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for executable_name in APPS.values():
                (root / executable_name).write_bytes(executable())
            instructions = root / "How to open.txt"
            instructions.write_text("More info, then Run anyway.", encoding="utf-8")
            archive = root / "test.zip"
            detached = root / "test.zip.sha256"
            result = build_zip(archive, root, instructions, "Release target: x86_64-pc-windows-msvc")
            self.assertIn("kernel32.dll", result)
            self.assertIn("SHA-256:", result)
            with ZipFile(archive) as bundle:
                self.assertEqual(bundle.namelist(), [*CONTENTS, CHECKSUMS])
            self.assertEqual(verify_zip(archive, detached), result)
            with self.assertRaises(FileExistsError):
                build_zip(archive, root, instructions, "unexpected overwrite")
            detached.write_text("bad detached digest", encoding="ascii")
            with self.assertRaisesRegex(ValueError, "invalid ZIP checksum"):
                verify_zip(archive, detached)


    def smoke_with_report(self, system_results, *, device_outcome="not_available",
                          reason="The Windows connection isn't available in this version yet.",
                          usb_outcome="skipped", audio_outcome="skipped", messages=0,
                          version="Variax Workbench 0.09 (build 468)", revision=REVISION):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)

            def fake_run(*args, env, cwd, input_text=None):
                self.assertEqual(cwd, scratch / "extracted")
                self.assertEqual(env["XDG_DATA_HOME"], str(scratch / "xdg-data"))
                if args[1] == "--version":
                    return version
                if args[1] == "--catalogue":
                    return "0.1 Computer and operating system\n8.1 Audio device inventory"
                destination = Path(args[-1])
                destination.mkdir()
                if args[1] == "--practice":
                    self.assertEqual(input_text, "accept\nr\nc\nr\nc\nr\nc\nr\nc\nq\n")
                    contents = (
                        "Mode: practice (simulated devices)\nsimulated computer\n"
                        "[1.1] See the PODxt Live on USB — PASSED\n"
                    )
                else:
                    self.assertEqual(args[1], "--real")
                    self.assertEqual(input_text, "accept\nr\nc\ns\nr\nc\ns\n")
                    steps = [
                        {"id": "0.1", "outcome": "passed", "results": system_results, "messages": 0},
                        {"id": "0.2", "outcome": usb_outcome, "messages": 0},
                        {"id": "0.3", "outcome": "passed", "messages": 0},
                        {"id": "1.1", "outcome": device_outcome, "reason": reason, "messages": messages},
                        {"id": "2.7", "outcome": "skipped_needs", "messages": 0},
                        {"id": "8.1", "outcome": audio_outcome, "messages": 0},
                    ]
                    # A version in the report summary cannot satisfy the step check.
                    record = {"finished": True, "git_commit": revision, "steps": steps}
                    contents = (
                        "Operating system: Windows\nWindows version: 10.0.26100\n"
                        "[0.3] Engine check (no device) — PASSED\n"
                        "Synthetic preset (full pipeline): all 1,092 words identical\n"
                        "The Windows connection isn't available in this version yet.\n"
                        "BEGIN RIG CHECK JSON\n"
                        f"{json.dumps(record)}\n"
                        "END RIG CHECK JSON\n"
                    )
                (destination / "run.txt").write_text(contents, encoding="utf-8")
                return ""

            with patch("build.run", side_effect=fake_run):
                env = {"VARIAX_BUILD_NUMBER": "468", "RIG_CHECK_GIT_COMMIT": REVISION}
                return smoke(scratch / "extracted", scratch, env)

    def test_smoke_records_numbered_workbench_and_rig_check_source(self):
        verification = self.smoke_with_report(WINDOWS_FIELDS)
        self.assertIn("Variax Workbench 0.09 (build 468)\n", verification)
        self.assertIn(f"Rig Check report revision: {REVISION}\n", verification)

    def test_smoke_refuses_unstamped_or_mismatched_workbench(self):
        for build in ("dev", "21", "4680"):
            with self.subTest(build=build), self.assertRaisesRegex(ValueError, r"build 468"):
                self.smoke_with_report(WINDOWS_FIELDS, version=f"Variax Workbench 0.09 (build {build})")

    def test_smoke_refuses_unstamped_or_mismatched_rig_check(self):
        for revision in (None, "b" * 40):
            with self.subTest(revision=revision), self.assertRaisesRegex(ValueError, "does not identify source"):
                self.smoke_with_report(WINDOWS_FIELDS, revision=revision)

    def test_smoke_records_step_01_version_build_and_recognized_architecture(self):
        for version, architecture in (
            ("10.0.26100", "x86_64"), ("10.0.26100.1742", "aarch64"),
            ("10.0.26100", "x86"), ("10.0.26100.1742", "arm"),
        ):
            with self.subTest(version=version, architecture=architecture):
                fields = [f"Windows version: {version}", "Windows build: 26100", f"Architecture: {architecture}"]
                verification = self.smoke_with_report(fields)
                self.assertIn("\n".join(fields), verification)
                self.assertIn("audio inventories were skipped", verification)
                self.assertIn("Zero device messages recorded", verification)

    def test_smoke_refuses_missing_unavailable_or_invalid_step_01_fields(self):
        for index, invalid, error in (
            (0, None, "Expected one Windows version"),
            (0, "Windows version: not available on this OS", "Invalid Windows version"),
            (0, "Windows version: 10.0.26100.1742.1", "Invalid Windows version"),
            (1, None, "Expected one Windows build"),
            (1, "Windows build: not available on this OS", "Windows build.*does not match"),
            (1, "Windows build: 1742", "Windows build.*does not match"),
            (2, None, "Expected one Architecture"),
            (2, "Architecture: not available on this OS", "Unrecognized Windows architecture"),
            (2, "Architecture: AMD64", "Unrecognized Windows architecture"),
        ):
            with self.subTest(field=index, invalid=invalid), self.assertRaisesRegex(ValueError, error):
                fields = ["Windows version: 10.0.26100.1742", "Windows build: 26100", "Architecture: x86_64"]
                fields[index:index + 1] = [] if invalid is None else [invalid]
                self.smoke_with_report(fields)

    def test_smoke_refuses_native_inventory_or_device_activity(self):
        for options, error in (
            ({"device_outcome": "passed"}, "Unexpected device outcomes"),
            ({"reason": "other notice"}, "Unexpected device outcomes"),
            ({"messages": 1}, "Expected zero device messages"),
            ({"usb_outcome": "passed"}, "Unexpected computer outcomes"),
            ({"audio_outcome": "passed"}, "Unexpected computer outcomes"),
        ):
            with self.subTest(options=options), self.assertRaisesRegex(ValueError, error):
                self.smoke_with_report(WINDOWS_FIELDS, **options)


if __name__ == "__main__":
    unittest.main()
