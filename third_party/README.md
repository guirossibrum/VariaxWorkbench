The optional `portable-math` feature compiles pinned `libm` 0.2.16 Rust algorithms
into the coefficient engine. It does not load a system math library. The crate
has no production transitive dependencies; Cargo.lock records its archive hash.
Its published license and embedded source notices are preserved under `notices/`.
Include these notices with a distribution that uses this dependency.

The dependency source is obtained through Cargo, separately from the notices.
The research verifier checks its archive hash and all 155 archived files before
building reference helpers. `scripts/collect_math_notices.py` reproduces the
notice collection from that checked source. The Linux builder also copies
dependency license/notice files found in cached crate sources and records
versions, SPDX declarations and source identifiers. Rebuilding offline requires
the prepared public Cargo cache; the package does not include a complete
dependency source/build-tool bundle.

The implementation boundary is `src/math.rs`; replacing the dependency requires
a new profile identifier and independent scalar/component/whole-engine checks.
Version changes must be deliberate. Source preservation, licensing and the
documented replacement boundary support future maintenance; they do not promise
continued upstream maintenance or OS compatibility. See `docs/math-profiles.md`.
