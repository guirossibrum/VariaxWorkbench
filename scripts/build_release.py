#!/usr/bin/env python3
"""Reserve the next application build number and compile an offline release."""

from pathlib import Path
import os
import subprocess
import tempfile


def build_release(root: Path) -> None:
    stamp = root / "app" / "BUILD_NUMBER"
    lock = root / "target" / "release-build.lock"
    lock.parent.mkdir(exist_ok=True)
    lock.mkdir()
    try:
        current = int(stamp.read_text(encoding="ascii"))
        if not 0 <= current < 2**32 - 1:
            raise ValueError("Application build number is outside the incrementable range.")
        number = current + 1
        with tempfile.NamedTemporaryFile(
            dir=stamp.parent, delete=False, mode="w", encoding="ascii"
        ) as output:
            pending = Path(output.name)
            output.write(f"{number}\n")
        try:
            os.chmod(pending, stamp.stat().st_mode)
            pending.replace(stamp)
        finally:
            pending.unlink(missing_ok=True)
        print(f"Building Variax Workbench build {number}", flush=True)
        subprocess.run(
            ["cargo", "build", "--offline", "--locked", "--release", "--bin", "variax-workbench-gui"],
            cwd=root,
            check=True,
        )
    finally:
        lock.rmdir()


if __name__ == "__main__":
    build_release(Path(__file__).resolve().parents[1])
