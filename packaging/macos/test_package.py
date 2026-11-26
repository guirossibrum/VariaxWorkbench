import plistlib
import tempfile
import unittest
from pathlib import Path

import package


LOADS = """binary:
    /usr/lib/libSystem.B.dylib (compatibility version 1.0.0)
    /System/Library/Frameworks/OpenGL.framework/Versions/A/OpenGL (compatibility version 1.0.0)
"""


class PackageTests(unittest.TestCase):
    def test_inspect_valid_deployment_targets(self):
        package.verify_load_commands("arm64", "cmd LC_BUILD_VERSION\n minos 11.0", LOADS)
        package.verify_load_commands("x86_64", "cmd LC_VERSION_MIN_MACOSX\n version 10.15", LOADS)

    def test_inspect_rejects_other_target_or_non_system_library(self):
        with self.assertRaisesRegex(ValueError, "unexpected minimum macOS version"):
            package.verify_load_commands("x86_64", "minos 11.0", LOADS)
        for path in ("/opt/homebrew/lib/libusb.dylib", "@rpath/liba.dylib"):
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "non-system dylib"):
                package.verify_load_commands("arm64", "minos 11.0", LOADS + f"    {path}\n")

    def test_inspect_rejects_framework_missing_on_catalina(self):
        framework = "/System/Library/Frameworks/UniformTypeIdentifiers.framework/UniformTypeIdentifiers"
        with self.assertRaisesRegex(ValueError, "unavailable on Catalina"):
            package.verify_load_commands("x86_64", "minos 10.15", LOADS + f"    {framework}\n")

    def test_plist_declares_architecture_minimums_and_icon(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "Info.plist"
            package.plist(path, "Rig Check", "rig-check", "test.rig-check", "0.9.0")
            manifest = plistlib.loads(path.read_bytes())
        self.assertEqual(manifest["LSMinimumSystemVersion"], "10.15")
        self.assertEqual(manifest["LSMinimumSystemVersionByArchitecture"], {"x86_64": "10.15", "arm64": "11.0"})
        self.assertEqual(manifest["CFBundleExecutable"], "rig-check")
        self.assertEqual(manifest["CFBundleIconFile"], "AppIcon.icns")


if __name__ == "__main__":
    unittest.main()
