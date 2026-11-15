"""Compare Rust preset reads/edits with the original Workbench Java parser.

Requires the already-extracted application and local JDK. Runs offline, creates
new temporary files only, and never opens the UI or a device.
"""
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
JAVA = [str(ROOT / ".tools/jdk/bin/java"), "-Djava.awt.headless=true", "-cp",
        str(ROOT / "analysis/extracted/mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar"),
        str(ROOT / "scripts/InspectPreset.java")]
RUST = [str(ROOT / "target/debug/variax-workbench")]


def run(command):
    return subprocess.run([str(p) for p in command], check=True, capture_output=True, text=True).stdout


def compare(path):
    java = run([*JAVA, path])
    rust = run([*RUST, "inspect", path])
    if java != rust:
        raise AssertionError(f"Original Java and Rust disagree for {path}\nJAVA:\n{java}\nRUST:\n{rust}")
    return rust


def main():
    subprocess.run(["cargo", "build"], cwd=ROOT, check=True)
    report = []
    with tempfile.TemporaryDirectory(prefix="variax-preset-") as directory:
        temporary = Path(directory)
        originals = [ROOT / f"analysis/extracted/windows-app/modules/{module}/device/Default Patch.vwm"
                     for module in ("variax", "variaxacoustic")]
        for i, original in enumerate(originals):
            summary = compare(original)
            output = temporary / f"roundtrip-{i}.vwm"
            run([*RUST, "roundtrip", original, output])
            assert output.read_bytes() == original.read_bytes()
            compare(output)
            report.append({"check": "original-reader agreement and byte-identical roundtrip",
                           "source": str(original.relative_to(ROOT)), "status": "pass"})

        renamed = temporary / "renamed.vwm"
        run([*RUST, "rename", originals[0], renamed, "Rust 🎸 Test"])
        summary = compare(renamed)
        assert summary.startswith("name\tRust 🎸 Test\n")
        report.append({"check": "UTF-16 rename accepted by original Java reader", "status": "pass"})

        edited = temporary / "edited.vwm"
        run([*RUST, "set", renamed, edited, "02010001", "200000", "float", "2.5"])
        summary = compare(edited)
        assert "param\t02010001\t200000\t00000001\t40200000\n" in summary
        report.append({"check": "pickup-position edit accepted by original Java reader", "status": "pass"})

        java_saved = temporary / "java-resaved.vwm"
        run([*JAVA, edited, java_saved])
        assert compare(java_saved) == summary
        report.append({"check": "Rust rereads original Java reserialization of edited preset", "status": "pass"})

        before = edited.read_bytes()
        overwrite = subprocess.run([*RUST, "roundtrip", str(originals[0]), str(edited)], capture_output=True)
        assert overwrite.returncode != 0 and edited.read_bytes() == before
        report.append({"check": "existing output is refused and preserved", "status": "pass"})

    output = ROOT / "analysis/reports/preset-compatibility.json"
    output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"PASS: {len(report)} compatibility checks; report: {output}")


if __name__ == "__main__":
    main()
