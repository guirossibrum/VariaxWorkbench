# Initialized native reference runtime

Updated 2026-11-15. All execution uses Unicorn private memory, with no device,
file/network APIs inside the emulated library or host callback registration.

The prior whole-generator baseline omitted Mach-O module initialization. In
particular, volume/tone taper globals remained zero. Those results remain
invalidated and preserved locally; they must not be reused as initialized-library
references.

`CoefficientOracle` now reads the original artifact's module-initializer table
and executes all **26 entries in table order** before loading models. Entries must
belong to executable image segments; arbitrary non-table addresses are rejected.
The library SHA-256 remains bound to the REA index, and the adapter for the local
C++ runtime wrapper is additionally bound to the known artifact hash and bytes.
No initializer values are assigned in place of running the original instructions.

The only added startup adapter captures **53 `__cxa_atexit` registrations** in
private memory, returning success without registering or executing a callback on
the host. Native constructors execute with the existing bounded private allocator
and disclosed host-libm adapters. Destructor/process-teardown behavior is not
validated. Unrecognized imports still stop execution.

`scripts/verify_native_startup.py` independently checks the table against the
original symbol table, all **52 translation-unit complex constants**, initialized
tone/volume values, repeatable fresh startup state and rejection of non-table
entries or damaged knob constants. The latter values are checked again before
every whole-generator call. This is an offline reference harness, not a hardware
unlock or complete device emulator.

The versioned `initialized-host-libm-v2` candidates pass **110 whole-generator
cases**, including a fresh-instance/warm-instance baseline comparison and packed
checksums, plus **four original-JNI/direct-generation comparisons**. There are
98 distinct output hashes, and **102 outputs differ from the invalidated baseline**.
This executes the original host library; it is not completion of the Rust engine,
Java model adjustments/metadata, historical macOS math parity, or audio/DSP parity.

The first corrected run reached its 20-million-instruction limit at the Chime
Toaster Bridge case (48 passes before stopping). That failed attempt is retained.
A targeted fresh-instance replay reproduced the instruction-budget stop in
response fitting. Full generation now has a **200-million-instruction / 10-second
per-call limit**; startup calls retain the 1-million / 2-second limit. The complete
110-case sweep passes within these bounds. No partial output from the failed
attempt counts as a passing reference.

Versioned local reports:

- `analysis/reports/native-startup-initialized-host-libm-v2.json`
- `analysis/reports/coefficient-generator-tests-initialized-host-libm-v2-budget200m.json`
- `analysis/reports/coefficient-jni-agreement-initialized-host-libm-v2.json`

Reference input/output files use a separate `initialized-host-libm-v2` subdirectory
beneath the source artifact hash. Non-overwriting saves preserve earlier results.
Vendor files and outputs stay ignored and local. Reproduce with:

```sh
.tools/python/bin/python scripts/verify_native_startup.py
.tools/python/bin/python scripts/test_coefficient_generator.py
.tools/python/bin/python scripts/verify_coefficient_jni.py
```

After the startup change, the body component was checked again: **252 vendor-body
outputs and 84 synthetic outputs**, and **600 routing-plan comparisons**, still
pass. The independent raw pickup response/primary-fit fixtures also pass Rust
regression tests. Historical math/runtime parity remains open in issue #5; the
remaining pickup tables/geometry stay open in issue #2.
