# Exact source-clock AIFF export

The offline recorder can explicitly save a stopped native-clock synthetic take
as uncompressed stereo 24-bit AIFF. It preserves the source clock `78125/2`
(39062.5 frames/second) without rounding or resampling. The original in-memory
S24_3LE bytes stay unchanged. The file reverses each sample's three bytes for
AIFF storage; decoding restores the original bytes exactly.

This is a file feature, not device recording, measured live negotiation or a DAW
compatibility claim. Integer PCM WAV remains a separate option. Native-clock WAV
export and the stand-ins 39062, 39063 and 78125 remain rejected.

## Format and implementation

The Apple-authored [AIFF specification](https://ich.music.mcgill.ca/classes/synth/AudioIFF1.2.1/AudioIFF1.2.1.html)
defines an 18-byte COMM chunk with an 80-bit extended sample rate, and SSND
sample data with explicit offset/block-size fields. Its signed samples use
big-endian byte order and interleaved channels. Apple's current
[AIFF format description](https://support.apple.com/guide/final-cut-pro/aiff-audio-interchange-file-format-ver65f484c7/mac)
also identifies it as a cross-platform audio format. This implementation writes
FORM/AIFF, COMM and SSND only, with offset/block size zero and two 24-bit channels.

The rate bytes are `400e9896800000000000`: exponent `16383 + 15`, explicit
significand `78125 << 47`. Integer construction preserves the exact rational on
every supported platform. No host floating-point conversion, OS math or audio
library is involved. The Rust codec has no new production dependency.

`recording::aiff::encode` accepts only whole six-byte stereo frames under the
existing 32 MiB take ceiling. The 54-byte file header is outside that PCM cap.
`decode` validates this canonical exact-clock export profile only; it rejects
other rates, compression, extra chunks, inconsistent sizes/frames and truncation.
It is a bounded profile checker, not a general AIFF importer. The GUI still loads
integer-rate WAV input only; it does not import arbitrary AIFF files.

## Save and quit behavior

The native-clock recording panel offers **Save exact-clock AIFF**. Its file dialog
and unsaved-take quit prompt name that format explicitly. The existing integer
path writes WAV. Neither choice silently converts a clock or sample values.

Saving writes a new AIFF and `.aiff.recording.json` sidecar, using the existing
atomic, non-overwriting file helper. Both destinations are checked first. A
failure between the two saves can leave the audio file without its sidecar;
the take stays unsaved and in memory. Existing files are never replaced. A
complete successful pair marks only the current take saved. Cancelling or
failing the save retains audio and prevents the quit continuation from closing.
An explicit later discard does not delete previously saved copies.

The sidecar records the rational numerator/denominator and driver-source
provenance, original PCM format/hash, stored sample format, AIFF hash, frames,
route label, peaks and gaps. It says no resampling and no device recording.
The source clock is Linux v7.2.15 driver evidence, not observed live negotiation.
There is no misleading integer `sample_rate_hz` field for this export.

## Independent validation and reader limits

Eight Rust-generated files with 0, 1, 11, 32, 64, 256, 1025 and 8192 frames were
inspected and decoded with **SoX_ng 14.8.0.1**. Its file-info reader reported
39062.5, two channels, 24 bits and the exact frame counts for all eight. Explicit
raw-file decoding with dithering disabled restored every original PCM byte,
including positive/negative full scale and deterministic varied samples.
The derived results are `research/aiff-export-reference.json`; default Rust tests
check those fingerprints without requiring SoX or opening an audio device.

The research-only Arch package was extracted privately, not installed. Package
SHA-256: `e19ec87e62195f72efc0256e1175c3ea10bc41ed0385cf69168b1e63c11880f8`.
The decoder executable hash is in the reference. SoX and its shared libraries
are independent research tools; the app does not link to or invoke them.

A separate local FFmpeg file-info check reported **39063** for the exact same
rate field. This is a reader limitation, not a changed file clock. No DAW was
opened and arbitrary software must not be assumed to preserve this unusual rate.
The native save UI therefore asks the user to verify imports. This task adds no
resampler, playback or monitoring path.

Reproduce into a new directory using a separately obtained SoX binary:

```sh
cargo run --example synthetic_aiff -- /ABS/NEW_FIXTURE_DIRECTORY
python scripts/verify_aiff_export.py /ABS/NEW_FIXTURE_DIRECTORY --sox /ABS/sox
```

The helper inspects files and decodes to raw stdout only; it never selects a
sound device. It writes a content-addressed private report, retaining prior
results. Source PCM accompanies each synthetic fixture. Tests also cover
truncated/malformed profiles, caps, wrong format/clock, cancellation, refusal to
replace audio or sidecar, saved-take status, original PCM retention and actual
headless quit/save actions. This is not the late-project audio/DSP matrix in #33.
