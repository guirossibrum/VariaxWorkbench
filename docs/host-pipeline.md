# Legacy host pipeline

Issue #4 is in progress. All work is offline; physical access remains locked.

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

Pending for #4: special combination-response resource loading and validation,
combined original Java/native final-byte checks, synthetic complete-host fixtures,
input hardening and complete fresh-clone regressions. The special native tables
are external local resources, not bundled vendor data. Unknown fields and failed
paths are not silently replaced with fabricated coefficients.

Reference helpers: `VerifyHostPipeline.java`, `verify_host_pipeline.py`,
`verify_host_engine.py`, and `verify_host_edge_cases.py`. No upload, live query,
reset, audio stream or firmware-writing path is provided.
