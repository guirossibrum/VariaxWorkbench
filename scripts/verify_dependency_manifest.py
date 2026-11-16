"""Check tracked dependency declarations without installing or executing tools."""
from __future__ import annotations

import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
LIBM_SHA256 = "b6d2cec3eae94f9f509c767b45932f1ada8350c4bdb85af2fcab4a3c14807981"
UNICORN_SHA256 = "9d6e6dea140560de4ebd8446661f7ef84a357d428c14a3ef09dacd306ec8c239"


def _requirements(path: Path) -> list[str]:
    return [line.strip() for line in path.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("#")]


def validate(root: Path = ROOT) -> list[str]:
    """Return declaration errors; never imports pip or starts a subprocess."""
    errors: list[str] = []
    try:
        root_req = _requirements(root / "requirements.txt")
        research_req = _requirements(root / "research/requirements-linux-x86_64.txt")
        manifest = json.loads((root / "requirements-tools.json").read_text(encoding="utf-8"))
        cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
        lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
        workflow = (root / ".github/workflows/offline-core.yml").read_text(encoding="utf-8")
        setup_source = (root / "scripts/research_setup.py").read_text(encoding="utf-8")
        reproduction = (root / "docs/reproducing-references.md").read_text(encoding="utf-8")
        native_analysis = (root / "docs/native-analysis.md").read_text(encoding="utf-8")
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        return [f"cannot parse a dependency source: {error}"]

    if root_req != ["--require-hashes", "-r research/requirements-linux-x86_64.txt"]:
        errors.append("requirements.txt must contain only hash mode and the authoritative Linux x86-64 include")
    if any(line.startswith(("libm", "llvm-nm")) for line in root_req + research_req):
        errors.append("Rust crates and system executables must not be declared as pip packages")

    unicorn = [line for line in research_req if line.startswith("unicorn")]
    expected_unicorn = f"unicorn==2.1.4 --hash=sha256:{UNICORN_SHA256}"
    if unicorn != [expected_unicorn] or research_req != [expected_unicorn]:
        errors.append("the Unicorn requirement must keep its exact version and wheel hash")

    libm = [package for package in lock.get("package", []) if package.get("name") == "libm"]
    if len(libm) != 1 or libm[0].get("version") != "0.2.16" or libm[0].get("checksum") != LIBM_SHA256:
        errors.append("Cargo.lock must remain authoritative for libm 0.2.16 and its archive checksum")
    dependency = cargo.get("dependencies", {}).get("libm", {})
    if (dependency.get("version") != "=0.2.16" or dependency.get("default-features") is not False
            or dependency.get("features") != ["force-soft-floats"]):
        errors.append("Cargo.toml must preserve the libm version/features contract")
    if cargo.get("features", {}).get("portable-math") != ["dep:libm"]:
        errors.append("the optional portable-math feature must remain the only direct libm activation")

    toolchain = re.search(r'^\s*toolchain:\s*["\']?([^"\'\s]+)', workflow, re.MULTILINE)
    if not toolchain or toolchain.group(1) != "1.99.1":
        errors.append("offline-core workflow must keep the documented Rust toolchain pin")
    research = manifest.get("profiles", {}).get("linux_x86_64_research", {}).get("required", {})
    if research.get("python", {}).get("minimum") != "3.11" or research.get("python", {}).get("tested_baseline") != "3.14.8":
        errors.append("Python minimum and tested baseline must match the research workflow documentation")
    if research.get("jdk17", {}).get("version") != "17.0.21" or "17.0.21" not in reproduction or "17.0.21" not in setup_source:
        errors.append("JDK 17 pin must agree with the tested doctor and reference profile")
    if research.get("7zip", {}).get("version") != "26.06" or "26.06" not in reproduction or "26.06" not in setup_source:
        errors.append("7-Zip pin must agree with the tested doctor and reference profile")
    optional = manifest.get("profiles", {}).get("linux_x86_64_research", {}).get("optional", {})
    reanalysis = optional.get("ghidra_rea", {})
    for key, source, label in [
        ("ghidra", reproduction, "Ghidra"),
        ("rea", reproduction, "REA"),
        ("node", reproduction, "Node"),
        ("npm", reproduction, "npm"),
        ("jdk", native_analysis, "JDK 25")
    ]:
        if not reanalysis.get(key) or str(reanalysis[key]) not in source:
            errors.append(f"{label} optional tool pin must match its provenance note")
    if manifest.get("schema_version") != 1:
        errors.append("unsupported requirements-tools.json schema_version")
    if manifest.get("profiles", {}).get("linux_x86_64_research", {}).get("required", {}).get("unicorn", {}).get("wheel_sha256") != UNICORN_SHA256:
        errors.append("tool manifest Unicorn hash must match the pip lock")
    math = manifest.get("profiles", {}).get("linux_x86_64_research", {}).get("required", {}).get("libm", {})
    if (math.get("version") != "0.2.16" or math.get("crate_sha256") != LIBM_SHA256
            or math.get("source_files_verified") != 155 or math.get("features") != ["force-soft-floats"]
            or math.get("default_features") is not False or math.get("transitive_dependencies") != 0):
        errors.append("tool manifest must preserve the exact libm research contract")
    required_categories = {"rust_ci", "linux_x86_64_research"}
    if not required_categories.issubset(manifest.get("profiles", {})):
        errors.append("tool manifest is missing a required contributor profile")
    if not any("llvm-nm" in gap and "absent" in gap for gap in manifest.get("known_gaps", [])):
        errors.append("tool manifest must disclose the unprovisioned, unpinned llvm-nm gap")
    return errors


def main() -> int:
    errors = validate()
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print("PASS: pip, Cargo, CI, and tool-manifest declarations agree; no tools executed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
