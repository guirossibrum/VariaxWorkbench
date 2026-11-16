# Historical Apple math candidate

Issue [#5](https://github.com/guirossibrum/VariaxWorkbench/issues/5) remains open.
The original Mac coefficient library imports math through libSystem. Its link
interface is 88.3.6. We statically extracted Apple's later 88.3.9 runtime from
the Intel Mac OS X 10.4.11 combined update and ran selected i386 math code in
Unicorn. This is a dated publisher-origin candidate, not proof of the OS,
processor, JVM or floating-point environment used by every Workbench installation.
The application's default math profile is unchanged.

## Source and preservation

The [Apple update](https://download.info.apple.com/Mac_OS_X/061-4048.20071114.6Gn4g/MacOSXUpdCombo10.4.11Intel.dmg)
is 337,080,169 bytes, SHA-256
`ba29bcf6f3a0a1ac657eeb712bd7e5d53bcd4dbcadb00f8bac68299db1fd5ebb`.
Its MD5 agrees with the CDN Content-MD5. This establishes the acquired byte
identity and publisher URL, not a verified digital signature.

The selected package payload is old ASCII CPIO inside gzip, despite its
`Archive.pax.gz` name. The extractor streams it under explicit size limits,
selects only two regular library files, checks gzip integrity, verifies their
hashes, and carves the i386 slices. It refuses an existing output directory.
It does not mount the image, run a package, or load vendor code into the host.

| Library | Universal SHA-256 | i386 SHA-256 |
| --- | --- | --- |
| libSystem.B.dylib 88.3.9 | `d8fa0d166c4492dda68a7fd92a07a20c03233b46b8def96781f55711628729f1` | `36f2a3fbaa04d2ddc7a7ad600c083c637ff28fcae5aa86252509742b16325358` |
| libmathCommon.A.dylib 220.0.0 | `16565737f8c75f1b088be5470cd421730baee5cd1c4f0663fff0c2dd4985ac35` | `7445e66b1cb7b49407cc9641d71bd090b5706a0999936834d9bd36a56bfaa6a0` |

The thirteen selected functions are in libSystem; mathCommon was inventoried
but was not needed to service them. The oracle maps the unmodified pinned i386
libSystem bytes. LLVM rejected an obsolete load command; a separate display-only
header copy was used for disassembly, never as oracle input. Original binaries,
disassembly, observed inputs, generated presets and detailed outputs stay private.

## Results and limits

- All thirteen basic finite-function checks return in the CPU sandbox.
- The existing observed trace has **206,228 distinct typed argument tuples**
  across ten functions. The Apple candidate differs from recorded glibc bits
  on **2,551** tuples: pow 4, powf 3, cosf 1,195 and sinf 1,349. These are internal
  calls from the existing corpus, not new guitar settings or audio tests.
- Both the pure guest candidate and the separate current-CPU x87 candidate
  complete the same **110 whole-generator cases**. Each matches **109/110**
  recorded glibc packed outputs. Both match all **110 existing MPFR-256 packed
  candidates** byte for byte. This comparison is to saved MPFR results; the
  separate fresh product comparison below covers Linux x86-64 only.
- The changed case is `two-pickups/series`, zero-based index 102. Four coefficient
  words and the checksum change. The glibc output SHA-256 is
  `2ae6c0bc84a01c442380c99335beb469f4890ee0391f1d891c636dd280917aeb`;
  both historical candidates produce
  `fc62774dc83405c8b303c27ad3616fb6ea20e4d0f39794eeb5b10926db19c03e`.

Unicorn's [i386 floating-point helpers](https://raw.githubusercontent.com/unicorn-engine/unicorn/2.1.4/qemu/target/i386/fpu_helper.c)
implement x87 transcendental instructions using host double math conversions.
Executing original Apple control flow therefore does not authenticate physical
historical x87 results. This qualification also applies to earlier x87 emulator
comparisons; an SSE branch and an x87 branch do not have identical evidence scope.

An independently compiled, owned C helper uses only FSIN, FCOS and FPTAN under a
saved/restored x87 control word. It imports no external functions, contains no
vendor instructions, and is a research dependency only. On this current x86-64
CPU, **194,460 float trig results agree** with the Apple guest. Of **848 double
tangent results**, one differs. The alternate generator adapter using these
current-CPU trig results still produces the same 110 packed candidates. This
is neither every historical Intel CPU nor physical audio validation.

The runtime's two startup powers produce `(0.0010000000000000002,
0.010000000000000002)`, each one ULP above the existing glibc startup contract.
The ordinary oracle correctly rejects these constants. A separate pinned Apple
profile explicitly requires them; the default profile still requires exactly
`(0.001, 0.01)`. Both reject uninitialized or wrong-profile state. The original
26 initializers and 53 captured destructor registrations complete. Destructors,
a complete JVM, return floating-point exception flags, all ABI behavior and
historical processor environments remain unverified.

Unknown imports, OS calls, unsupported controls, unmapped accesses and exhausted
execution budgets stop execution. The first uncached whole-generator attempt
stopped after 68 cases at the existing wall limit. Successful math values were
then memoized under exact typed input bits and floating-point controls with a
bounded cache; no budget or import allowance was relaxed. The complete runs use
that cache and explicitly do not model return exception flags. Failed attempts
and subsequent reports are retained separately.

## Optional reproduction

With the original privately archived Workbench inputs and existing research
Python environment available, obtain the update separately and supply a local
7-Zip executable. Output must be a new ignored directory:

```sh
python scripts/extract_apple_tiger_runtime.py \
  vendor/installers/apple-runtime/MacOSXUpdCombo10.4.11Intel.dmg \
  .tools/apple-runtime-new --sevenzip /absolute/path/to/7zz
```

The extraction manifest names the carved i386 library. Pass that path as `LIB`
in these separate research runs:

```sh
python scripts/verify_apple_tiger_runtime.py "$LIB"
python scripts/audit_apple_tiger_math.py "$LIB" "$TRACE"
python scripts/audit_apple_tiger_generator.py "$LIB"
python scripts/audit_apple_tiger_generator.py "$LIB" --current-host-x87
python scripts/audit_x87_instruction_reference.py "$TRACE" "$APPLE_REPORT"
```

`TRACE` is the private observed-math-inputs JSON, pinned SHA-256
`13dcfbf977114421578f4430f411eb64167a513af6ed774105b3ddb5f0a882ec`;
`APPLE_REPORT` is the complete math-audit report produced above. The native x87
comparison requires current x86-64 Linux research tools gcc and nm, builds only
owned arithmetic C, and records source/compiler/flags/binary hashes. None of these
tools or libraries becomes an application dependency. CI checks Python syntax
without originals or native helper execution.

The extractor reproduced both library and slice hashes. Eight profile/CPU guard
checks passed, including altered-source rejection, unsupported controls and FTZ,
wrong startup states, namespace isolation, unknown-import rejection, a blocked
interrupt and an execution-limited loop. The ordinary initialized-host startup
regression produced its unchanged reference report; six existing runtime guard
checks also passed. These are bounded offline research results. They establish
neither audio equivalence, recovery capability, physical safety nor permission
to contact equipment. Default-profile rollout and wider runtime coverage remain
separate work and owner decisions.

## Fresh Rust product comparison

A further direct check builds the actual CLI twice per profile, in debug and
release, using an isolated source/compiler-addressed Cargo target directory.
It verifies the pinned libm archive, all 155 source files and Cargo's resolved
source; records every product-source hash, compiler, command and binary hash;
and rejects source changes during the run. The ordinary initialized oracle
reconstructs the existing 110 inputs and validates the private model resources.
The two saved Apple reports must have the expected runtime/source identities,
completed initialization and the exact input order, and every saved output is
checked against its recorded hash before comparison.

Both fresh `portable-math` binaries match **110/110** outputs from **each**
Apple candidate. Both default host-math binaries retain **110/110** recorded
glibc outputs and match **109/110** Apple outputs, with the same series case
and four coefficient words plus checksum differing. Debug and release outputs
agree within each product profile. This tests actual current Rust CLI outputs
against the saved original-instruction candidates, rather than relying solely
on their earlier agreement with MPFR. It does not rerun the historical generator
or establish a physical Mac/Windows process, audio or wider-input equivalence.

Reproduce with the two complete generator reports from the earlier commands:

```sh
python scripts/audit_apple_tiger_product.py "$GUEST_GENERATOR_REPORT" "$CURRENT_X87_GENERATOR_REPORT"
```

This optional command is currently scoped to Linux x86-64 and uses offline,
locked Cargo builds with no GUI/USB features. It writes a private content-addressed
report under `.tools/apple-product-audit/`; failures retain their report. The
110 settings are unchanged existing cases and are not additional sample coverage
for #33. The experiment neither selects a new default nor changes any runtime
contract or production dependency.

## Single-case factor diagnostic

Eight fresh-state diagnostic runs isolate the observed series-case difference.
The complete host and Apple controls first reproduce their saved stateful-sweep
hashes. Substituting only the Apple `sinf` produces the complete Apple result
(four coefficient words and checksum); `pow`, `powf` and `cosf` alone each
retain the complete glibc result. The `sinf`+`cosf` and `pow`+`powf` pairs retain
the corresponding single-factor outcome. Each run enforces the independently
pinned startup tuple appropriate to its selected `pow`; no profile accepts an
arbitrary observed constructor state.

```sh
python scripts/isolate_apple_tiger_case.py "$LIB"
```

The original library remains CPU-emulated, while the diagnostic host control
uses system libm as the existing research adapter does. These eight executions
reuse one existing settings case. They are not new distinct settings for #33,
not additional product compatibility profiles and not a general assertion that
other functions cannot matter on other inputs. Their private report records the
source/input/auditor hashes and output hashes; no vendor output is published.
