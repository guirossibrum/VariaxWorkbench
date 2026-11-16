"""Pinned research-tool checks; only version commands are executed."""
from __future__ import annotations

import subprocess

LLVM_NM_VERSION = "23.1.6"


def verify_llvm_nm(run=None) -> str:
    """Return the version output or fail closed when the pinned tool is absent/wrong."""
    if run is None:
        run = subprocess.run
    command = ["llvm-nm", "--version"]
    try:
        result = run(command, capture_output=True, text=True, timeout=30, check=True)
    except (OSError, subprocess.SubprocessError) as error:
        raise RuntimeError("required llvm-nm 23.1.6 is missing or unusable") from error
    output = result.stdout + result.stderr
    if LLVM_NM_VERSION not in output:
        raise RuntimeError(f"expected llvm-nm {LLVM_NM_VERSION}; version command returned {output.strip()!r}")
    return output
