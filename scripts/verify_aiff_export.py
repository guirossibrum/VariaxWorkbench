"""Independently inspect/decode synthetic AIFF exports with an explicit SoX binary.

File-only research helper, not an application runtime dependency. Generate the
fixtures with `cargo run --example synthetic_aiff -- NEW_DIRECTORY`. No playback,
audio device, host server or passthrough command is used.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from inspect_firmware import preserve


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exports", type=Path)
    parser.add_argument("--sox", required=True, type=Path)
    args = parser.parse_args()
    sox = str(args.sox.resolve(strict=True))
    version = subprocess.check_output([sox, "--version"], text=True).strip()
    cases = []
    for frames in (0, 1, 11, 32, 64, 256, 1025, 8192):
        audio = args.exports / f"{frames}.aiff"
        original = args.exports / f"{frames}.source.pcm"
        if not audio.is_file() or not original.is_file():
            raise ValueError(f"missing synthetic fixture: {frames}")
        if audio.stat().st_size > 6 * frames + 54 or original.stat().st_size != frames * 6:
            raise ValueError(f"unexpected fixture bounds: {frames}")
        facts = {}
        for key, flag, expected in (("rate", "-r", "39062.5"), ("channels", "-c", "2"),
                                    ("bits", "-b", "24"), ("frames", "-s", str(frames))):
            value = subprocess.check_output([sox, "--i", flag, str(audio)], text=True).strip()
            if value != expected:
                raise ValueError(f"independent decoder {key}: {value}, expected {expected}")
            facts[key] = value
        # Explicit raw stdout output, never the default audio device. Dithering
        # is disabled because this checks exact lossless 24-bit sample storage.
        decoded = subprocess.check_output([sox, "-D", "-t", "aiff", str(audio),
                                          "-t", "raw", "-e", "signed-integer",
                                          "-b", "24", "-L", "-c", "2", "-"])
        if decoded != original.read_bytes():
            raise ValueError(f"independent decoder changed source samples: {frames}")
        cases.append(dict(facts, source_pcm_sha256=hashlib.sha256(decoded).hexdigest(),
                          aiff_sha256=hashlib.sha256(audio.read_bytes()).hexdigest()))
    report = {"schema": 1, "status": "pass", "decoder": version, "decoder_binary_sha256": hashlib.sha256(Path(sox).read_bytes()).hexdigest(), "scope": "file-only synthetic exports; no playback or device access",
              "exact_clock": "78125/2", "resampled": False, "cases": cases,
              "limits": ["Other readers may round the clock; no DAW import or live negotiation tested"]}
    encoded = (json.dumps(report, indent=2) + "\n").encode()
    root = Path(__file__).resolve().parents[1]
    output = root / "analysis/reports" / ("aiff-decoder-" + hashlib.sha256(encoded).hexdigest()[:16] + ".json")
    output.parent.mkdir(parents=True, exist_ok=True)
    preserve(output, encoded)
    print(output)


if __name__ == "__main__":
    main()
