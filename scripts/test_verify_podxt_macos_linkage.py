"""Offline fixtures for the native PODxt USB linkage verifier."""

import json
from pathlib import Path
import unittest

from verify_podxt_macos_linkage import (
    BINARIES,
    build_outputs,
    require_defined_symbols,
    verify_binary,
)

MINIMUM = "      cmd LC_BUILD_VERSION\n    minos 10.15\n"
SYSTEM_LIBS = "rig-check:\n  /usr/lib/libSystem.B.dylib (compatibility version 1.0.0)\n"


class LinkageTest(unittest.TestCase):
    def test_cargo_reports_the_current_archive_and_both_binaries(self):
        script = {
            "reason": "build-script-executed",
            "package_id": "path+file:///src/libusb1-sys-0.7.0#0.7.0",
            "linked_libs": ["framework=IOKit", "static=usb-vendored"],
            "out_dir": "/cargo/build/sys-id/out",
        }
        artifacts = [
            {
                "reason": "compiler-artifact",
                "executable": f"/cargo/{name}",
                "target": {"name": name, "kind": ["bin"]},
            }
            for name in BINARIES
        ]

        def cargo_log():
            return "\n".join(map(json.dumps, [script, *artifacts]))

        archive, executables = build_outputs(cargo_log())
        self.assertEqual(archive, Path("/cargo/build/sys-id/out/libusb-vendored.a"))
        self.assertEqual(set(executables), set(BINARIES))
        script["linked_libs"] = ["dylib=usb-1.0"]
        with self.assertRaises(ValueError):
            build_outputs(cargo_log())

    def test_symbols_must_be_defined_in_mach_o(self):
        require_defined_symbols("object.o:\n0000000000000120 T _libusb_open", ["libusb_open"])
        with self.assertRaises(ValueError):
            require_defined_symbols("                 U _libusb_open", ["libusb_open"])

    def test_load_commands_apply_package_rules_and_refuse_even_system_libusb(self):
        verify_binary("x86_64", MINIMUM, SYSTEM_LIBS)
        verify_binary("arm64", MINIMUM.replace("10.15", "11.0"), SYSTEM_LIBS)
        for arch, minimum, imports in [
            ("arm64", MINIMUM, SYSTEM_LIBS),
            ("x86_64", MINIMUM, SYSTEM_LIBS + "  @rpath/libfoo.dylib (version 1.0.0)"),
            ("x86_64", MINIMUM, SYSTEM_LIBS + "  /usr/lib/libusb-1.0.dylib (version 1.0.0)"),
            ("x86_64", MINIMUM, SYSTEM_LIBS +
             "  /System/Library/Frameworks/UniformTypeIdentifiers.framework/Versions/A/UniformTypeIdentifiers (version 1.0.0)"),
        ]:
            with self.subTest(arch=arch, imports=imports), self.assertRaises(ValueError):
                verify_binary(arch, minimum, imports)


if __name__ == "__main__":
    unittest.main()
