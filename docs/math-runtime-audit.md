# Math runtime compatibility

This is an offline sensitivity audit for issue [#5](https://github.com/guirossibrum/VariaxWorkbench/issues/5).
It does not establish audio equivalence or device safety. Production Rust math
still depends on platform implementations of transcendental functions; a controlled,
portable production backend remains pending. No Linux math library was added to
the application's dependencies.

## What the originals use

The original i386 Mac coefficient library imports `atan2f`, `cos`, `cosf`, `exp`,
`expf`, `log`, `log10f`, `pow`, `powf`, `sin`, `sinf`, `tan` and `tanf` from
`libSystem`. Its recorded libSystem link version is 88.3.6. That identifies the
system-library interface, not the exact implementation installed on every Mac.
An original historical Mac runtime binary has not been authenticated or tested.

The corresponding Windows coefficient library imports only KERNEL32 services;
its CRT math routines are embedded in the module. Its linker version is 8 and
its PE timestamp is 2009-11-05. The audit runs identified `_CI` math instructions
in private Unicorn memory, without loading the DLL into Windows or Wine.
Identification uses disassembly and coefficient call sites as well as numerical
checks. This is stronger evidence than substituting a modern Windows math DLL.

## Linux, independent arithmetic and Apple source comparison

Tracing 110 initialized Mac-generator cases leaves all canonical output hashes
unchanged. The trace contains **684,564 math calls and 206,228 distinct argument
tuples**, across ten functions. The other three imports are not covered by that
corpus. MPFR comparisons at 256 and 512 working bits agree after conversion to
the target float/double width for every observed tuple. This is a precision
cross-check, not proof of historical system-library behavior.

| Function | Distinct inputs | glibc versus MPFR differing results | Maximum finite difference |
| --- | ---: | ---: | ---: |
| `cosf` | 96,808 | 1,195 | 1 ULP |
| `sinf` | 96,808 | 1,349 | 1 ULP |
| `powf` | 3,898 | 3 | 1 ULP |
| `tan` | 848 | 2 | 1 ULP |
| Other six observed functions | 7,866 | 0 | 0 |

Replacing the Mac generator's math imports with MPFR-256 changes **one of 110
packed outputs**: the two-pickup series case. Four coefficient words (zero-based
indices 139, 141, 143 and 145) and the additive checksum at index 1091 change.
Small floating-point differences can therefore survive quantization. A full
MPFR-512 generator replay has not been performed; the precision agreement above
covers the original trace's arguments.

A separate comparison builds Apple's published
[Libm-92 source](https://github.com/apple-oss-distributions/Libm/tree/Libm-92),
pinned to commit `0501897231e5e00031dac38fe44309a3881fe296`. It is a Linux x86_64
recompilation using the source's assembly ABI and C `pow`/`powf`, with remaining
host `fabs`, `scalbn` and `sqrt` dependencies. It is **not an original Mac runtime**.
This candidate changes 92 of 110 packed outputs (200 words including checksums)
and returns NaN for `exp(-infinity)` where glibc/MPFR return zero. These results
demonstrate why a historical source tag cannot be silently adopted as canonical.
Apple sources, license, helpers and original-derived outputs remain local and
ignored; no vendor source or binary is included in this repository.

## Original Windows math comparison

All 206,228 observed tuples complete in each of three explicitly selected modes:
original CPU dispatch initialization with 53-bit and 64-bit x87 precision, and
the image's uninitialized dispatch flag with 53-bit x87 precision. These modes
are comparison conditions, not observations from a running historical JVM.

| Mode | Differing result counts versus glibc |
| --- | --- |
| Initialized SSE dispatch, 53-bit | `tan`: 5; `powf`: 3; `cosf`: 1,195; `sinf`: 1,349 |
| Initialized SSE dispatch, 64-bit | Same counts as 53-bit |
| Image-default x87 dispatch, 53-bit | `pow`: 194; `powf`: 3; `exp`: 36; `log`: 298; `cosf`: 1,195; `sinf`: 1,349 |

With the initialized 53-bit Windows math adapter, all 110 Mac-generator cases
complete. One packed output differs from glibc at the same four coefficient words
and checksum listed above; all 110 packed outputs match the MPFR-256 candidate.
This is agreement for this corpus, not a mathematical or cross-platform guarantee.
Float aliases use the Mac import's float32 input/output width around the Windows
double intrinsic; complete Windows compiler arithmetic is not represented.

The image-default x87 adapter produces startup constants
`0.0009999999999999987` and `0.009999999999999997`, rather than the canonical
`0.001` and `0.01`. The unchanged startup guard rejects this state. Its individual
math comparisons are complete; **its whole-generator comparison did not run**.
The failed first audit attempt and completed second report remain locally saved.

## Reproduction and boundaries

The optional analysis helpers require Linux x86_64, a C compiler, LLVM tools,
MPFR development headers, the existing Python/Unicorn environment and locally
extracted original modules. The build helper obtains the pinned Apple source
over HTTPS if absent. These are research tools, not application runtime needs.

```sh
.tools/python/bin/python scripts/build_math_reference.py
.tools/python/bin/python scripts/audit_math_runtime.py
.tools/python/bin/python scripts/audit_windows_math.py
.tools/python/bin/python scripts/verify_math_runtime_guards.py
```

Reports retain source/helper/library hashes, compiler commands, precision and
CPU controls. The canonical emulated Mac uses x87 control `0x037f`, empty x87
tags and MXCSR `0x1f80`. The measured host used glibc 2.44 and MPFR 4.2.2 with
round-to-nearest. Its MXCSR `0x1fa0` includes a sticky precision flag; the rounding
control matches `0x1f80`. This is an observed environment, not a portable contract.

Local reports are `analysis/reports/math-runtime-audit-v1.json` and
`analysis/reports/windows-math-audit-v1.json`. Private traces and packed candidates
are saved under corresponding `analysis/extracted/` directories without replacing
the canonical `initialized-host-libm-v2` fixtures. Alternate math adapters must
use a distinct runtime namespace. Startup and per-call knob checks remain strict;
an alternate backend failing them stops rather than changing expected constants.

The Windows harness maps only the hash-bound original image and private buffers.
All OS imports lead to rejection traps; unmapped access and instruction/time
limits stop execution. A private errno pointer replaces only the original CRT
pointer getter. A private FS:0 exception-list sentinel permits the original CPU
selection probe to run; it does not provide host OS services. The wrapper clears
Unicorn's translated-code cache whenever its call instructions change. Regression
checks cover function/result-width switching, errno domains, rejection traps,
precision modes and alternate-reference namespace separation.

## Remaining work

Historical Mac binary parity, complete Windows coefficient/JVM execution,
arithmetic outside math calls, wider boundary coverage and a versioned portable
Rust production math implementation remain unresolved. In particular, substituting
Windows math results into the Mac generator does not reproduce the Windows
compiler's x87 expressions. Do not bless either host OS's results, or the Apple
source candidate, as universally correct. Keep issue #5 open until the remaining
compatibility limits are resolved or explicitly accepted. No result here permits
live USB/MIDI/audio traffic or a firmware update.
