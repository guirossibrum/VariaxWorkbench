import plistlib
import contextlib
import io
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

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

    def test_runtime_artwork_in_relocated_bundle(self):
        with tempfile.TemporaryDirectory() as folder:
            stage = Path(folder) / "stage"
            staged_app = stage / "Variax Workbench.app"
            package.stage_artwork(staged_app)
            installed_app = Path(folder) / "Mac Applications" / staged_app.name
            installed_app.parent.mkdir()
            shutil.move(str(staged_app), str(installed_app))
            root = installed_app / package.BUNDLE_ARTWORK
            self.assertEqual(package.verify_artwork(installed_app), 10)
            self.assertEqual(len((root / package.ARTWORK_MANIFEST).read_text().splitlines()), 10)
            for name in ("workbench-mark.png", "variax.png", "podxt-live.png"):
                self.assertTrue((root / "shell" / name).is_file(), name)

    def test_runtime_artwork_rejects_missing_extra_or_corrupt_files(self):
        for name, contents, error in (
            ("shell/variax.png", None, "missing"),
            ("shell/unapproved.png", b"extra", "unexpected"),
            ("shell/podxt-live.png", b"changed", "Artwork checksum mismatch"),
            (package.ARTWORK_MANIFEST, b"changed", "manifest differs"),
        ):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as folder:
                bundle = Path(folder) / "Variax Workbench.app"
                package.stage_artwork(bundle)
                path = bundle / package.BUNDLE_ARTWORK / name
                if contents is None:
                    path.unlink()
                else:
                    path.write_bytes(contents)
                with self.assertRaisesRegex(ValueError, error):
                    package.verify_artwork(bundle)

    def test_owner_dmg_requires_static_libusb_in_each_mounted_binary_slice(self):
        symbols = "\n".join(f"000001 T _{name}" for name in package.USB_SYMBOLS)
        with tempfile.TemporaryDirectory() as folder:
            mount = Path(folder) / "mount"
            image = Path(folder) / "owner.dmg"
            image.touch()
            for title, binary in package.APPS.items():
                app = mount / f"{title}.app"
                executable = app / "Contents" / "MacOS" / binary
                executable.parent.mkdir(parents=True)
                executable.write_bytes(b"universal")
                if title == "Variax Workbench":
                    package.stage_artwork(app)

            def tools(tool, *args):
                if tool == "lipo" and args[0] == "-archs":
                    return "arm64 x86_64"
                if tool == "lipo":
                    Path(args[-1]).write_bytes(b"slice")
                    return ""
                if tool == "vtool":
                    return "minos 11.0" if args[-1].name.endswith("arm64") else "minos 10.15"
                if tool == "otool":
                    return LOADS
                if tool == "nm":
                    if args[-1].name == unavailable_slice:
                        return ""
                    return symbols
                self.fail(f"unexpected tool: {tool}")

            with patch("package.xcrun", side_effect=tools), contextlib.redirect_stdout(io.StringIO()):
                unavailable_slice = None
                package.verify(mount, folder, image, "owner-usb")
                for name in package.APPS.values():
                    for arch in package.ARCHITECTURES:
                        unavailable_slice = f"{name}-{arch}"
                        with self.subTest(slice=unavailable_slice), self.assertRaisesRegex(
                            ValueError, "missing defined Mach-O symbol"
                        ):
                            package.verify(mount, folder, image, "owner-usb")


if __name__ == "__main__":
    unittest.main()
