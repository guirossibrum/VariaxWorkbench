import json
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from zipfile import ZipFile

from build import smoke
from package import APPS, CONTENTS, CHECKSUMS, build_zip, verify_zip, windows_imports


def executable(dll=b"Kernel32.dll", *, delay=None, machine=0x8664, signed=False):
    """One mapped import section in a synthetic PE, with no executable code."""
    data = bytearray(0x500)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    optional = 0x98
    header_size = 240
    struct.pack_into("<HHIIIHH", data, 0x84, machine, 1, 0, 0, 0, header_size, 2)
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<I", data, optional + 108, 16)
    struct.pack_into("<II", data, optional + 112 + 8, 0x1000, 40)
    if signed:
        struct.pack_into("<II", data, optional + 112 + 4 * 8, 0x400, 16)
    struct.pack_into("<IIII", data, optional + header_size + 8, 0x300, 0x1000, 0x300, 0x200)
    struct.pack_into("<IIIII", data, 0x200, 0, 0, 0, 0x1100, 0)
    data[0x300:0x300 + len(dll)] = dll
    if delay:
        struct.pack_into("<II", data, optional + 112 + 13 * 8, 0x1040, 64)
        struct.pack_into("<II", data, 0x240, 1, 0x1140)
        data[0x340:0x340 + len(delay)] = delay
    return bytes(data)


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


    def test_smoke_uses_simulated_usb_and_skips_native_usb_and_audio_inventory(self):
        with tempfile.TemporaryDirectory() as tmp:
            scratch = Path(tmp)
            for device_outcome, inventory_outcome, error in (
                ("not_available", "skipped", None),
                ("passed", "skipped", "Unexpected device outcomes"),
                ("not_available", "passed", "Unexpected computer outcomes"),
            ):
                def fake_run(*args, env, cwd, input_text=None):
                    self.assertEqual(cwd, scratch / "extracted")
                    self.assertEqual(env["XDG_DATA_HOME"], str(scratch / "xdg-data"))
                    if args[1] == "--version":
                        return "Variax Workbench 0.09"
                    if args[1] == "--catalogue":
                        return "0.1 Computer and operating system\n8.1 Audio device inventory"
                    destination = Path(args[-1])
                    destination.mkdir(exist_ok=True)
                    if args[1] == "--practice":
                        self.assertEqual(input_text, "accept\nr\nc\nr\nc\nr\nc\nr\nc\nq\n")
                        contents = (
                            "Mode: practice (simulated devices)\nsimulated computer\n"
                            "[1.1] See the PODxt Live on USB — PASSED\n"
                        )
                    else:
                        self.assertEqual(args[1], "--real")
                        self.assertEqual(input_text, "accept\nr\nc\ns\nr\nc\ns\n")
                        notice = "The Windows connection isn't available in this version yet."
                        steps = [
                            {"id": "0.1", "outcome": "passed"},
                            {"id": "0.2", "outcome": inventory_outcome},
                            {"id": "0.3", "outcome": "passed"},
                            {"id": "1.1", "outcome": device_outcome, "reason": notice, "messages": 0},
                            {"id": "2.7", "outcome": "skipped_needs"},
                            {"id": "8.1", "outcome": "skipped"},
                        ]
                        # Only steps that ran have detailed headings. Skips
                        # and refusals are still present in the JSON report.
                        contents = (
                            "Operating system: Windows\nWindows version: 10.0.26100\n"
                            "[0.3] Engine check (no device) — PASSED\n"
                            "Synthetic preset (full pipeline): all 1,092 words identical\n"
                            f"{notice}\n"
                            "BEGIN RIG CHECK JSON\n"
                            f"{json.dumps({'finished': True, 'steps': steps})}\n"
                            "END RIG CHECK JSON\n"
                        )
                    (destination / "run.txt").write_text(contents, encoding="utf-8")
                    return ""

                with self.subTest(device_outcome=device_outcome, inventory_outcome=inventory_outcome), \
                        patch("build.run", side_effect=fake_run):
                    if error is None:
                        self.assertIn("audio inventories were skipped", smoke(scratch / "extracted", scratch, {}))
                    else:
                        with self.assertRaisesRegex(ValueError, error):
                            smoke(scratch / "extracted", scratch, {})


if __name__ == "__main__":
    unittest.main()
