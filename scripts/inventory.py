"""Inventory the locally preserved installers and compare extracted Java classes."""

import hashlib
import json
from pathlib import Path
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]


def inventory():
    installers = []
    for path in sorted((ROOT / "vendor/installers").glob("*")):
        if path.is_file():
            installers.append({
                "path": str(path.relative_to(ROOT)),
                "size": path.stat().st_size,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            })
    jar = ROOT / "analysis/extracted/mac-app/VariaxWorkbench.app/Contents/Resources/Java/VariaxWorkbench.jar"
    windows = ROOT / "analysis/extracted/windows-app"
    with ZipFile(jar) as archive:
        classes = [n for n in archive.namelist() if n.endswith(".class")]
        common = [n for n in classes if (windows / n).is_file()]
        different = [n for n in common if archive.read(n) != (windows / n).read_bytes()]
        return {
            "installers": installers,
            "mac_jar_sha256": hashlib.sha256(jar.read_bytes()).hexdigest(),
            "mac_classes": len(classes),
            "common_classes": len(common),
            "identical_classes": len(common) - len(different),
            "different_classes": different,
            "mac_native_libraries": [n for n in archive.namelist() if ".jnilib." in n],
            "windows_native_libraries": sorted(str(p.relative_to(windows)) for p in windows.rglob("*.jnilib.win")),
        }


if __name__ == "__main__":
    print(json.dumps(inventory(), indent=2))
