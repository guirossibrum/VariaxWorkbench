# Reproducing the offline reference workspace

Normal Rust builds/tests use committed synthetic fixtures and do not require
vendor installers, REA, Java or an emulator. The following separate research
workflow reconstructs private inputs and regenerates their reference results.
It is currently verified on Linux x86-64. Product core CI covers Linux, Windows
and macOS; that does not certify these Linux research tools on other systems.

Original installers, extracted code/artwork, reports and personal captures stay
ignored. Never execute a vendor installer, original native library or updater.
The extraction recipe treats installer EXEs, DMGs, XAR packages, gzip/CPIO and
JARs as archives. It does not install the USB-interface package, run package
scripts, register drivers, mount a device or open USB/MIDI/audio hardware.

## Inputs and tools

`research/artifacts.json` identifies the archived installer/firmware files by
size and SHA-256. The Workbench recipe requires the two exact 1.75 installers:

| Archive | Bytes | SHA-256 |
| --- | --- | --- |
| Variax Workbench 1.75.dmg | 18085510 | `d70b73a1b0e441e7a640c387f13e42f44284388063b155fa162fcc771a614fa1` |
| Variax Workbench v1.75 Installer.exe | 19871616 | `a6870ad2792fb0ee70683be0402cbc828ac316dcd1aa66abb2813cdc1df40f99` |

Hashes identify the archived bytes; they are not publisher signatures. Other
listed archives are inventories for their device-specific research workflows,
not inputs to the coefficient-reference extraction recipe.

The tested research tools are CPython 3.14.8, Temurin JDK 17.0.21, 7-Zip 26.06,
Unicorn 2.1.4 and Rust 1.99.1, with a Linux C linker/compiler available as `cc`.
Python 3.11+ is needed by the setup helper. `doctor` checks tool availability,
the tested JDK/extractor/compiler versions and both original archive identities,
and records the host/Python/libc versions. A missing/mismatched prerequisite
stops with a setup error. The default reference adapter uses the research
host's libm; changed runtime rounding is a test failure to investigate, not
permission to replace expectations. See [math profiles](math-profiles.md).

Supply trusted portable JDK and 7-Zip distributions under a tool directory as
`jdk/bin/java`, `jdk/bin/javap`, `jdk/bin/javac` and `7zip/7zz`. Existing local
tools can be attached without copying them. They are development tools, separate
from the application and from the original vendor archives.

The research Python environment is created locally with `venv`. Its only
required package is the hash-locked Linux x86-64 Unicorn wheel named by
`research/requirements-linux-x86_64.txt`. Prepare that wheel cache explicitly:

```sh
python -m pip download --only-binary=:all: --no-deps unicorn==2.1.4 --dest WHEELS
```

Use an existing development Python/pip if the system Python has no pip. The
subsequent bootstrap uses `--no-index --require-hashes`; it has no network
fallback. A different platform wheel is deliberately not accepted by this
Linux research profile. No package is added to application runtime dependencies.

## Fresh disposable checkout

Start from a committed revision, with no pre-existing extracted artifacts,
schemas, model catalog or Python environment. These example paths are placeholders
for the user's archived installers, tool directory and cached wheel directory:

```sh
git worktree add --detach .worktrees/reference-replay master
cd .worktrees/reference-replay
python scripts/research_setup.py attach-tools /ABS/TOOLS
python scripts/research_setup.py bootstrap-python --wheels /ABS/WHEELS
python scripts/research_setup.py --installers /ABS/INSTALLERS doctor
python scripts/research_setup.py --installers /ABS/INSTALLERS extract
python scripts/research_setup.py catalog
python scripts/research_setup.py index
python scripts/research_setup.py references
```

`attach-tools` creates links for required JDK/7-Zip and for Node/JDK25/Ghidra only
when those optional tool directories exist. It does not replace tools. Use a
fresh checkout if any extraction destination already exists. Installer identity
checks precede archive-tool execution. Existing different files are refused;
partially completed research output is retained for diagnosis.

The recipe extracts the Windows NSIS archive, then the ZIP embedded in its
installed application EXE. It extracts the Mac DMG's package as XAR and reads
only the Workbench gzip/old-ASCII-CPIO payload. Regular files are created as data
without restoring executable modes; symbolic links are recorded without being
materialized. Archive paths, sizes and bounds are checked. The JAR and native
coefficient-slice digests are checked after extraction.

All eight validated i386/PowerPC slices are regenerated. The original raster
assets are copied unchanged into ignored `assets/original-ui/`, with hashes in
the extraction receipt. This path needs no Pillow. The older optional contact-
sheet helper `catalog_ui_assets.py` additionally needs Pillow; contact sheets
are not product or reference prerequisites.

The model catalog comes from the extracted definitions XML and `javap`'s static
constant inspection of `L6AssetDefs_const`. No original Java class is executed
to create the catalog. It contains 28 bodies and 43 native pickup definitions,
including internal combinations; those are not the 32 visible UI pickup choices.

## Procedure metadata and REA

`research/native-functions.json` contains 1,069 derived procedure names,
addresses, status values and body ranges from the existing digest-bound REA
export. It contains no instructions, decompiled source, vendor resource bytes
or original executables. The original export digest, REA checkout commit and
provider version are recorded. `index` prepares the exact metadata expected by
the reference helpers, or checks an existing complete private REA index without
replacing it. The native library digest is still checked by the emulator.

That saved address metadata removes the full decompiler export from the minimum
reference prerequisites. Independent static re-analysis remains possible with
REA 6.9.0 at commit `79cc07ecf191e44ff4e280fa843e11a574e20f77`, Node 24.22.1,
npm 11.16.0, JDK 25.0.5 and Ghidra 12.2. Ghidra 12.2 was accepted by the bridge;
its recorded verified baseline was 12.1.4. See [native analysis](native-analysis.md).

`scripts/rea` resolves the sibling checkout from Git's common directory, so it
also works in disposable worktrees. `VARIAX_REA_CHECKOUT` and
`VARIAX_GHIDRA_DIR` can select private alternatives. Build the provided sibling
REA checkout with its pinned `npm ci`/`npm run build:cached` workflow. No `rea
setup`, client registration or desktop configuration change is needed. Only
static analysis tools are appropriate here; the hardware boundary still applies
to any optional REA job. Original target execution is restricted to the explicit
private Unicorn reference harnesses, with their fail-closed adapters and limits.

## Reference regeneration and receipts

`references` accepts only its fixed allowlist of file/emulator verifiers. It
first regenerates serialization metadata/schema, then body, pickup, DSP and host
references and original-Java file checks. Specific steps can be selected by name:

```sh
python scripts/research_setup.py references verify_native_pack verify_native_resonators
```

Prepare the public Rust dependency cache with `cargo fetch --locked` before an
offline replay (or use the future packaged dependency bundle). The reference
runner sets `CARGO_NET_OFFLINE=true`, with no dependency-network fallback.
Steps run sequentially, each with a separate new log and timeout. A failed step
stops the replay. Existing logs are refused to preserve earlier results. Use
another disposable checkout for another complete replay; do not delete a failed
run to conceal it. The before/after digest comparison covers every committed
fixture. A changed, added or removed fixture is an error requiring review, even
if another test passed. Never promote generated expectations automatically.

Content-addressed receipts under ignored `analysis/reports/` record extraction
hashes and reference log hashes. The fresh-workspace replay is distinct from
historical Mac authentication, full Windows generator/JVM equivalence, audio
equivalence or a hardware recovery test. Nothing here authorizes live traffic.

`python scripts/test_research_setup.py` runs nine synthetic guard tests without
vendor archives, emulation or external tools. They check truncated CPIO prefixes,
path traversal, symlink redirection, retained data/executable modes, archive
identity mismatches, file replacement refusal, a competing-save race and conflicting
procedure indexes.
The core CI runs these guards on all three product platforms.


## Validation history and limits

Fresh detached worktrees were prepared from the two archived installers, trusted
local tools and the hash-locked wheel cache. Each got its own Python environment,
extracted files, model catalog, derived index and generated reports. The catalog
matches the earlier private catalog, and the eight native-slice records match
regardless of list order. No existing extracted directory was attached.

The first replay exposed a missing prerequisite: `verify_dsp_word_map` needs the
whole-generator outputs. The recipe now runs `test_coefficient_generator` before
that step. All 39 distinct reference stages passed across the original run and
its documented continuation, including 110 whole-generator cases and 144 complete
original-Java/native preset comparisons. The continuation receipt is
`reference-replay-859a1ef72fb82168.json`; fixture digests remained unchanged.
The initial failed log remains private with that workspace.

A second fresh replay passed the first 29 stages, then the generator stopped on
case 69 (pickup ID `fffffff7`) within its existing 200-million-instruction / ten-
second call limits. Its earlier error did not distinguish the two stop reasons;
this is not classified as a numerical mismatch or a proven wall timeout. The
failure is retained in `reference-replay-2f02d0fe401fb70c.json`. A fresh emulator
retry under the same limits passed all 110 cases byte-for-byte against the first
workspace's native outputs; its slowest call was 7.600205 seconds, at case 69.
That retry is `generator-limits-97b8ac763dd25af3.json`. Prior native outputs are
regression evidence, not an independent historical-math or audio oracle.

`native_oracle.py` now includes elapsed time and Unicorn's timeout flag in a
non-return error. Neither execution limit nor any numerical expectation was
relaxed. An optional diagnostic replay keeps the same bounds and stops at its
first failure:

```sh
.tools/python/bin/python scripts/profile_generator_limits.py --expected-dir /ABS/PREVIOUS/NATIVE/OUTPUTS
```

Use the prior workspace's coefficient-cache directory containing the generated
`.packed.bin` files. The diagnostic writes a new content-addressed report; it does
not replace a failed replay receipt or promote fixtures. Slow or constrained
hosts can still stop at the bounds and should retain the resulting diagnostics.

The setup guards also exposed a Windows-specific ZIP-name normalization issue in
CI. Validation now checks the original archive member name before normalization,
and synthetic backslash/NUL cases run on all three platforms. The legitimate
Workbench archives have no such names. Linux, Windows and macOS CI passed all
nine setup guards plus 114 software-profile Rust core tests in debug and release.
The private extraction/emulation workflow itself remains Linux x86-64 only.
