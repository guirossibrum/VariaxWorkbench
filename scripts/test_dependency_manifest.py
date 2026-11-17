"""Focused dependency-manifest consistency tests; standard library only."""
import json
import io
from pathlib import Path, PureWindowsPath
import shutil
import subprocess
import tempfile
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

import verify_dependency_manifest as check
import research_setup
from dependency_tools import verify_llvm_nm


class DependencyManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        for name in ["requirements.txt", "requirements-tools.json", "Cargo.toml", "Cargo.lock"]:
            shutil.copy2(check.ROOT / name, self.root / name)
        (self.root / "scripts").mkdir()
        shutil.copy2(check.ROOT / "scripts/research_setup.py", self.root / "scripts/research_setup.py")
        shutil.copy2(check.ROOT / "scripts/dependency_tools.py", self.root / "scripts/dependency_tools.py")
        for name in ["build_rust_math_reference.py", "verify_native_startup.py", "verify_coefficient_jni.py", "audit_engine_math.py"]:
            shutil.copy2(check.ROOT / "scripts" / name, self.root / "scripts" / name)
        (self.root / "docs").mkdir()
        for name in ["reproducing-references.md", "native-analysis.md"]:
            shutil.copy2(check.ROOT / "docs" / name, self.root / "docs" / name)
        (self.root / "research").mkdir()
        shutil.copy2(check.ROOT / "research/requirements-linux-x86_64.txt", self.root / "research/requirements-linux-x86_64.txt")
        (self.root / ".github/workflows").mkdir(parents=True)
        shutil.copy2(check.ROOT / ".github/workflows/offline-core.yml", self.root / ".github/workflows/offline-core.yml")

    def tearDown(self):
        self.temp.cleanup()

    def test_authoritative_pins_and_categories_are_consistent(self):
        self.assertEqual(check.validate(self.root), [])

    def test_missing_or_changed_hash_lock_fails_closed(self):
        path = self.root / "research/requirements-linux-x86_64.txt"
        path.write_text("unicorn==2.1.4 --hash=sha256:" + "0" * 64 + "\n", encoding="utf-8")
        self.assertTrue(any("Unicorn" in error for error in check.validate(self.root)))

    def test_changed_cargo_archive_checksum_is_not_hidden(self):
        path = self.root / "Cargo.lock"
        path.write_text(path.read_text(encoding="utf-8").replace(check.LIBM_SHA256, "0" * 64), encoding="utf-8")
        self.assertTrue(any("Cargo.lock" in error for error in check.validate(self.root)))

    def test_changed_llvm_package_pin_cannot_drift_from_signed_metadata(self):
        path = self.root / "requirements-tools.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        data["profiles"]["linux_x86_64_research"]["required"]["llvm-nm"]["package_sha256"] = "0" * 64
        path.write_text(json.dumps(data), encoding="utf-8")
        self.assertTrue(any("llvm-nm" in error for error in check.validate(self.root)))

    def test_arch_llvm_provisioning_pin_is_required(self):
        path = self.root / "requirements-tools.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        del data["profiles"]["linux_x86_64_research"]["required"]["llvm-nm"]["arch_linux_x86_64"]
        path.write_text(json.dumps(data), encoding="utf-8")
        self.assertTrue(any("Arch Linux x86_64" in error for error in check.validate(self.root)))

    def test_rustfmt_is_required_in_manifest_ci_and_setup_docs(self):
        path = self.root / "requirements-tools.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        data["profiles"]["rust_ci"]["components"].remove("rustfmt")
        path.write_text(json.dumps(data), encoding="utf-8")
        self.assertTrue(any("rustfmt" in error for error in check.validate(self.root)))

    def test_changed_optional_tool_pin_cannot_drift_from_evidence(self):
        path = self.root / "requirements-tools.json"
        data = json.loads(path.read_text(encoding="utf-8"))
        data["profiles"]["linux_x86_64_research"]["optional"]["ghidra_rea"]["jdk"] = "25.0.6"
        path.write_text(json.dumps(data), encoding="utf-8")
        self.assertTrue(any("JDK 25" in error for error in check.validate(self.root)))

    def test_llvm_version_probe_rejects_missing_or_wrong_binary(self):
        def good(_command, **_kwargs):
            return subprocess.CompletedProcess([], 0, "llvm-nm, compatible with GNU nm\nLLVM version 23.1.6\n  Optimized build.\n", "")
        def arch(_command, **_kwargs):
            return subprocess.CompletedProcess([], 0, "llvm-nm, compatible with GNU nm\nLLVM version 23.1.5\n  Optimized build.\n", "")
        def wrong(_command, **_kwargs):
            return subprocess.CompletedProcess([], 0, "LLVM version 23.1.4\n", "")
        def longer(_command, **_kwargs):
            return subprocess.CompletedProcess([], 0, "LLVM version 23.1.60\n", "")
        def incidental(_command, **_kwargs):
            return subprocess.CompletedProcess([], 0, "llvm-nm report\nUnrecognized tool; expected LLVM version 23.1.6\n", "")
        def missing(_command, **_kwargs):
            raise FileNotFoundError("llvm-nm")
        self.assertIn("23.1.6", verify_llvm_nm(good))
        self.assertIn("23.1.5", verify_llvm_nm(arch))
        for probe in (wrong, longer, incidental):
            with self.subTest(probe=probe.__name__), self.assertRaisesRegex(RuntimeError, "expected llvm-nm version line"):
                verify_llvm_nm(probe)
        with self.assertRaisesRegex(RuntimeError, "missing or unusable"):
            verify_llvm_nm(missing)

    def test_llvm_version_probe_rejects_nonzero_and_timeout(self):
        def nonzero(_command, **_kwargs):
            raise subprocess.CalledProcessError(1, ["llvm-nm", "--version"], stderr="failed")
        def timeout(_command, **_kwargs):
            raise subprocess.TimeoutExpired(["llvm-nm", "--version"], 30)
        for probe in (nonzero, timeout):
            with self.subTest(probe=probe.__name__), self.assertRaisesRegex(RuntimeError, "missing or unusable"):
                verify_llvm_nm(probe)

    def test_llvm_version_accepts_horizontal_indentation_and_real_output(self):
        outputs = [
            "llvm-nm, compatible with GNU nm\nLLVM (http://llvm.org/):\n  LLVM version 23.1.5\n  Optimized build.\n",
            "llvm-nm, compatible with GNU nm\n  Debian LLVM version 23.1.6\n  Optimized build.\n",
            "LLVM version 23.1.6\n", " LLVM version 23.1.5\n",
            "\tLLVM version 23.1.6\n", " \t  LLVM version 23.1.5\n",
            "\tApple LLVM version 23.1.6\n",
        ]
        for output in outputs:
            with self.subTest(output=output):
                def probe(command, **kwargs):
                    self.assertEqual(command, ["llvm-nm", "--version"])
                    self.assertEqual(kwargs, dict(capture_output=True, text=True, timeout=30, check=True))
                    return subprocess.CompletedProcess(command, 0, output, "")
                self.assertEqual(verify_llvm_nm(probe), output)

    def test_llvm_version_rejects_indented_wrong_tokens_and_incidental_prose(self):
        for output in (
            "  LLVM version 23.1.3\n", "\tLLVM version 23.1.60\n",
            "  LLVM version 23.1.5.1\n", " LLVM version 23.1.5rc1\n",
            "  expected LLVM version 23.1.5\n", "  report: LLVM version 23.1.6\n",
            "GNU nm (GNU Binutils) 23.1.5\n", "\u00a0LLVM version 23.1.5\n",
        ):
            with self.subTest(output=output), self.assertRaises(RuntimeError):
                verify_llvm_nm(lambda *_a, **_k: subprocess.CompletedProcess([], 0, output, ""))

    def test_llvm_version_rejects_duplicate_and_conflicting_lines(self):
        for first, second in (("23.1.5", "23.1.5"), ("23.1.5", "23.1.6"), ("23.1.5", "23.1.3")):
            with self.subTest(first=first, second=second), self.assertRaises(RuntimeError):
                verify_llvm_nm(lambda *_a, **_k: subprocess.CompletedProcess(
                    [], 0, f"  LLVM version {first}\n", f"\tLLVM version {second}\n"))

    def _doctor_inputs(self, omit=()):
        tools = self.root / "tools"
        paths = ["jdk/bin/java", "python/bin/python", "7zip/7zz"]
        for relative in paths:
            if relative not in omit:
                path = tools / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"test-only placeholder; never executed by this test")
        installers = self.root / "installers"
        installers.mkdir()
        for name in ["Variax Workbench 1.75.dmg", "Variax Workbench v1.75 Installer.exe"]:
            (installers / name).write_bytes(b"synthetic archive identity fixture")
        return tools, installers

    @staticmethod
    def _version_process(command, **_kwargs):
        if (PureWindowsPath(command[0]).parts[-3:] == ("jdk", "bin", "java")
                and command[1:] == ["-version"]):
            return subprocess.CompletedProcess(command, 0, "", 'openjdk version "17.0.21"\n')
        if (PureWindowsPath(command[0]).parts[-3:] == ("python", "bin", "python")
                and command[1:] == ["--version"]):
            return subprocess.CompletedProcess(command, 0, "Python 3.14.8\n", "")
        if (PureWindowsPath(command[0]).parts[-2:] == ("7zip", "7zz")
                and command[1:] == []):
            return subprocess.CompletedProcess(command, 0, "7-Zip 26.06\n", "")
        if command == ["rustc", "--version"]:
            return subprocess.CompletedProcess(command, 0, "rustc 1.99.1 (test)\n", "")
        if command == ["cargo", "--version"]:
            return subprocess.CompletedProcess(command, 0, "cargo 1.99.1 (test)\n", "")
        if command == ["cc", "--version"]:
            return subprocess.CompletedProcess(command, 0, "cc (test compiler) 1\n", "")
        if command == ["llvm-nm", "--version"]:
            return subprocess.CompletedProcess(command, 0, "LLVM version 23.1.6\n", "")
        raise AssertionError(f"doctor attempted an unlisted command: {command!r}")

    def test_version_mock_accepts_only_listed_commands_on_both_path_styles(self):
        for prefix, separator in (("/synthetic/tools", "/"), ("C:\\synthetic\\tools", "\\")):
            for relative, arguments, expected in (
                    (("jdk", "bin", "java"), ["-version"], "17.0.21"),
                    (("python", "bin", "python"), ["--version"], "3.14.8"),
                    (("7zip", "7zz"), [], "26.06")):
                executable = prefix + separator + separator.join(relative)
                with self.subTest(executable=executable):
                    result = self._version_process([executable, *arguments])
                    self.assertEqual(result.returncode, 0)
                    self.assertIn(expected, result.stdout + result.stderr)
                    for command in ([executable, "--unexpected"],
                                    [executable, *arguments, "extra"],
                                    [executable + ".unlisted", *arguments]):
                        with self.assertRaises(AssertionError):
                            self._version_process(command)

    def test_doctor_fails_closed_for_missing_tool_without_running_other_commands(self):
        tools, installers = self._doctor_inputs(omit=("7zip/7zz",))
        output = io.StringIO()
        with patch.object(research_setup.subprocess, "run", side_effect=self._version_process) as run, \
                patch.object(research_setup, "checked_artifact", side_effect=lambda _d, n: installers / n), \
                redirect_stdout(output):
            with self.assertRaisesRegex(ValueError, "prerequisites incomplete"):
                research_setup.doctor(tools, installers)
        self.assertIn("missing 7zip", output.getvalue())
        self.assertEqual(run.call_count, 6)

    def test_doctor_rejects_wrong_tool_version(self):
        tools, installers = self._doctor_inputs()
        for separator in ("/", "\\"):
            output = io.StringIO()
            def wrong_jdk(command, **kwargs):
                command = [command[0].replace("/", separator), *command[1:]]
                result = self._version_process(command, **kwargs)
                if PureWindowsPath(command[0]).parts[-3:] == ("jdk", "bin", "java"):
                    return subprocess.CompletedProcess(command, 0, "", 'openjdk version "17.0.20"\n')
                return result
            with self.subTest(separator=separator), \
                    patch.object(research_setup.subprocess, "run", side_effect=wrong_jdk), \
                    patch.object(research_setup, "checked_artifact", side_effect=lambda _d, n: installers / n), \
                    redirect_stdout(output):
                with self.assertRaisesRegex(ValueError, "prerequisites incomplete"):
                    research_setup.doctor(tools, installers)
            self.assertIn("differs from tested 17.0.21", output.getvalue())


if __name__ == "__main__":
    unittest.main()
