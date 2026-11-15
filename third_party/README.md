The optional `portable-math` feature compiles pinned `libm` 0.2.16 Rust algorithms
into the coefficient engine. It does not load a system math library. The crate
has no production transitive dependencies; Cargo.lock records its archive hash.
Its published license and embedded source notices are preserved under `notices/`.
Include these notices with a distribution that uses this dependency.

The dependency source is obtained through Cargo, separately from the notices.
The research verifier checks its archive hash and all 155 archived files before
building reference helpers. `scripts/collect_math_notices.py` reproduces the
notice collection from that checked source. Other dependency notices and a
complete vendored/offline build bundle remain packaging work in issues #10/#11.

The implementation boundary is `src/math.rs`; replacing the dependency requires
a new profile identifier and independent scalar/component/whole-engine checks.
Version changes must be deliberate. Source preservation, licensing and the
documented replacement boundary support future maintenance; they do not promise
continued upstream maintenance or OS compatibility. See `docs/math-profiles.md`.

## Linux Variax prefix reference bytes

`src/linux_variax_prefix.rs` embeds two compiled excerpts of Linux
`line6_variax_process_message`, virtual addresses `[0x160, 0x240)`. Upstream
identifies that source as `SPDX-License-Identifier: GPL-2.0-only`, Copyright
(C) 2004-2010 Markus Grabner. The pinned file is `sound/usb/line6/variax.c` at
commit `497791a0a96b866d90add64041c47acdff49cae6`. The license text is
`notices/GPL-2.0-only.txt`. Provenance, the compiler, and the detached build
recipe are in `notices/linux-variax-prefix-NOTICE.txt`.

These bytes are test reference data. They are not a Cargo dependency and they
are not linked into the application. Message buffers passed to the interpreter
are separate synthetic inputs.
