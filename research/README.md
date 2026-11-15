# Research inputs

These files support optional offline reference regeneration. The Rust application
does not load them during normal operation.

- `artifacts.json`: archived file identity metadata, with no original bytes.
- `native-functions.json`: procedure names/addresses/ranges derived from private
  REA/Ghidra analysis, bound to the original coefficient-library SHA-256. No
  instructions or decompiler output are included.
- `requirements-linux-x86_64.txt`: hash-locked research-only Unicorn wheel.

Keep installers, native libraries, original resources/artwork, firmware, captures,
full private decompiler exports and tool distributions outside Git. The setup
recipe reconstructs a separate ignored workspace from explicitly supplied inputs.
See [the reproduction guide](../docs/reproducing-references.md) and the fixture
provenance in [tests/fixtures](../tests/fixtures/README.md).
