# Offline test fixtures

These fixtures contain synthetic inputs and reference outputs produced from those
synthetic inputs. They contain no downloaded firmware, vendor model assets,
guitar captures or factory presets.

- `editor-preset.vwm`: synthetic minimal Workbench-format preset named Synthetic.
- `native-body-definition.bin`: generated values across the recovered body schema.
- `native-pickup-definition.bin`: generated electrical/geometry values.
- `native-pack-input.bin`, `native-pack-output.bin`: deterministic synthetic words
  and the original native packer's output for those words.
- `native-resonator-vectors.txt`: finite synthetic boundary values and native outputs.
- `native-body-normalization.txt`: synthetic pickup-position inputs and native float32 results.
- `native-body-interpolation-plans.txt`: synthetic mode/coordinate inputs and traced native routing trees; per-field blending is deliberately bypassed.
- `original-java-word-encoding.txt`: synthetic integer inputs and Java encoder results.

Reference scripts under `scripts/verify_*` document generation and verification.
The ordinary Rust tests use only these fixtures and do not need vendor files or
hardware. Full original-library reference tests require separately obtained local
vendor artifacts and the ignored analysis/tool directories.
