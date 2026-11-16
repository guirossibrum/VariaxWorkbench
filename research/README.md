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

The opt-in `scripts/native_updater_oracle.py` additionally checks
`updater-oracle-reference.json`: 43 source-fingerprinted synthetic host-method
case results, with lengths, derived hashes and statuses. No vendor code, captured
packets, preset data or emitted buffers are included. The baseline may support
later independent Rust simulation tests; it does not approve receiver inputs,
provide an updater or establish recovery safety. The helper compares the baseline
without rewriting it and keeps the full execution reports private.
