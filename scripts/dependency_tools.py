"""Pinned research-tool checks; only version commands are executed."""
from __future__ import annotations

import re
import subprocess

LLVM_NM_VERSIONS = frozenset({"23.1.5", "23.1.6"})
_VERSION_LINE = re.compile(r"^[ \t]*(?:LLVM|Debian LLVM|Apple LLVM) version (\d+\.\d+\.\d+)(?:\s|$)")


def verify_llvm_nm(run=None) -> str:
    """Return the version output or fail closed when the pinned tool is absent/wrong."""
    if run is None:
        run = subprocess.run
    command = ["llvm-nm", "--version"]
    try:
        result = run(command, capture_output=True, text=True, timeout=30, check=True)
    except (OSError, subprocess.SubprocessError) as error:
        expected = ", ".join(sorted(LLVM_NM_VERSIONS))
        raise RuntimeError(f"required llvm-nm ({expected}) is missing or unusable") from error
    output = result.stdout + result.stderr
    versions = [match.group(1) for line in output.splitlines() if (match := _VERSION_LINE.match(line))]
    if len(versions) != 1 or versions[0] not in LLVM_NM_VERSIONS:
        expected = ", ".join(sorted(LLVM_NM_VERSIONS))
        raise RuntimeError(f"expected llvm-nm version line to match one of [{expected}]; version command returned {output.strip()!r}")
    return output
