# Legacy host pipeline

Issue #4 implements the legacy host pipeline. All work is offline; physical access remains locked.

The Rust `host` module maps complete legacy preset documents into Java-shaped
native settings, applies alternate pickup-combination/load-capacitance rules,
and overlays the final name/metadata/checksum after coefficient packing. File
mapping requires complete fields and does not mutate the document or invent
missing model resources. Native settings and batch outputs are analysis formats.

Original Java `GeneratePatch` and `ApplyHacks` run in a reference harness with a
fake coefficient engine. Device constructors and native loading are bypassed;
only the original pure mapping and metadata methods execute. 240 independently
generated synthetic cases match all 192 native-setting bytes, the block mask,
and all 3276 overlay output bytes. Cases include electric/acoustic/twelve-string/
sitar flags, unit/angle conversion, tuning/detune/mix branches, muting, combination
selection, load overrides and ASCII replacement/truncation. Synthetic fixtures
retain the full original outputs; originals stay local.

The native logical parameter IDs differ from model-local file IDs. `HostParameters`
explicitly maps between them. The final Java checksum covers 1089 words, while
the native packer covers 1091; the host overlay deliberately preserves that
original difference rather than replacing it with a generic checksum.

`HostEngine` now joins typed body interpolation, pickup calculations, geometry,
all DSP blocks, legacy enable/mask rules and packing. All 3276 packed bytes match
the existing original-instruction 110-case corpus. This includes all bundled
body/pickup definitions, controls, tuning and a repeated baseline after sweeps.
The initialized v2/host-libm reference remains subject to issue #5's historical
runtime limitations. These comparisons are not rendered audio tests.

The original engine retains unused coil gains between calls, including when
switching into acoustic models. `HostEngine` preserves that context; constructing
a fresh engine resets it. The native local count of newly placed electric coils
is separate from acoustic body geometry when applying tuning-related disables.
Errors do not commit the replacement coil-gain state.

Additional whole-pipeline comparisons exposed two missing component edge cases:
EQ wet products round to float before double dry-mix arithmetic, and logarithmic
interpolation of zero-frequency endpoints can produce NaNs from finite inputs.
300 additional synthetic fractional EQ cases and 48 original sitar snapshots
validate the corrections. Derived sitar NaNs retain logical word 0x1000000, which
Pack masks to zero. Only the crate's internal interpolation path accepts derived
frequency NaNs; public numeric block calls still reject them atomically.

Final-byte comparison now passes 144 complete preset-file cases through the
original Java reader/ParamMap, original native generation and original Java final
overlay versus Rust `render-preset`. This includes 24 targeted -3/-7 special
combination cases varying pickup order, phase and resistance around the original
100-ohm predicate. Original tables stay private; the manifest explicitly supplies
them as 360-byte snapshots with the matching internal rate. Integrity/authenticity
of caller-supplied model resources is not established by these comparisons.

48 complete original-instruction outputs from exclusively synthetic model resources
are committed for ordinary tests, plus one complete synthetic preset/final-byte
fixture. The latter exercises the CLI and non-overwriting output. 103 all-feature
Rust tests pass. Invalid flags/nonfinite inputs, ambiguous manifests, missing
special tables and wrong snapshot rates reject without committing engine state.
The synthetic corpus also exposed the acoustic trim grouping: native adds analog
gain and user trim first, then the body's continuous gain, all in float32.

Historical-runtime parity (#5), broader full-engine coverage (#6), model-specific
domains (#7), rendered audio (#33), device read/write and GUI rendering integration
remain separate. The renderer supports the legacy device family; 0x70005 and HD
models are not enabled by this port. Unknown fields and failed paths are not
silently replaced with fabricated coefficients.

Reference helpers: `VerifyHostPipeline.java`, `verify_host_pipeline.py`,
`verify_host_engine.py`, and `verify_host_edge_cases.py`. No upload, live query,
reset, audio stream or firmware-writing path is provided.


## Local use

```sh
cargo run --bin variax-workbench -- render-preset PRESET.vwm MODELS.tsv NEW.packed
```

Append `muted` to exercise the original muted-trim branch. The destination must
not exist. The 3276-byte result is an offline analysis artifact and has no device
upload command. Single-file rendering uses a fresh engine; batch analysis retains
original engine state between records. Complete documents are required; missing
fields and unsupported families fail without editing the input.

The tab-separated manifest contains `body`, `pickup` or `special`, a hexadecimal
model ID, and a local path (absolute or relative to the manifest). Source bodies
are 17483 bytes, pickups 85 bytes, and special snapshots 360 bytes. Duplicate IDs
fail. Sources must be regular files of the exact required size. Obtain ordinary
model resources from your own installer using the local analysis tools. Generate
private special-table manifest entries with:

```sh
.tools/python/bin/python scripts/extract_host_special_responses.py
```

That helper executes only original host code in CPU emulation and saves snapshots
under ignored `analysis/`, with source hash/runtime provenance. Do not commit the
snapshots or original definitions. Fresh-clone Rust tests need neither the vendor
resources nor Java, Python, Unicorn, network access or equipment.

Additional reference helpers: `verify_host_complete.py`,
`verify_host_synthetic.py`, and `extract_host_special_responses.py`. Their complete
original-derived outputs/reports remain under ignored `analysis/` or temporary
folders; committed fixtures contain only synthetic resources and their results.
