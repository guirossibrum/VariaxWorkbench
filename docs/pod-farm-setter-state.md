# POD Farm 2.58 setter and outer state contracts

This finite static pass continues the [cache/serialization trace](pod-farm-cache-serialization.md).
It resolves a conditional setter clamp, the outer two-tone container and the
full AU's state dictionary entry points. It does not complete #40 or release #29.
All vendor bytes, pseudocode, disassembly and reports remain private/ignored.
No vendor code or additional emulation ran; no device or audio stream was opened.

The subsequent [plugin-provider pass](pod-farm-plugin-provider.md) resolves the
concrete AU receiver and XFP6 payload, and traces the supplied i386 VST/RTAS
state wrappers. Eight new synthetic prefix/status checks supplement the 17 here.
Deferred AU consumption and full cross-product preservation remain unproved.

## Sources and coordinates

| Supplied artifact/slice | SHA-256 | Coordinates below |
| --- | --- | --- |
| L6TWXY full framework | `1b6acb3aa012d9b884f00daeacc948aa80884c81f513871be740055cd629a924` | Original unchanged |
| L6TWXY x86_64 slice | `dced9e6bee1ac57ba9135c1b8751d26141be82155122f7aa151b2cb457bd55a1` | Framework VA; fat offset `0x1000` |
| Standalone full executable | `b6c3aad9ac949706aa0479a0e10e0977d165189269dcbbfeeb067478d22a72cc` | Original unchanged |
| Standalone x86_64 slice | `21404e6d89fc920db3750864d24a52d44b7eae1bd9452d2fb88fd64dcfbbe726` | App VA, image base `0x100000000`; fat offset `0x1000` |
| Full AU x86_64 slice | `715d30b8fed77d4ea18150050131db759b8e8a27eeaf4e3d9b3872b6bca6fbe0` | AU VA; fat offset `0x282000` |

Standalone and AU copies were extracted by reading the fat table and verified
against the earlier private inventory before static import. Ghidra 12.2 and GNU
objdump read these files. This is not evidence for i386, Windows, VST/RTAS,
Elements, another release or active installed state. Recovered standalone method
names provide stronger labels than inference from similar framework code; ABI
and address spaces remain separate.

## The named setter clamps through a separately built registry

Framework export `TWXYSetParameter`, `0x38ea0`, resolves the instance and invokes
slot `+0x1d8` (`0x235368` → `0x45950`). The public value argument is a raw
machine word, not a caller-supplied typed record. The setter validates the selector,
ordinal and descriptor availability, then calls `0x506a0` to store the requested
word. That write is at root + `0x150` + group stride + ordinal stride. Since a
record starts at root + `8 + 0x18`, it is descriptor + `0x130`, the current
field, before registry validation. Omitting the group's extra eight bytes would
incorrectly identify it as the third metadata field + `0x138`.

`0x45950` constructs a typed value using descriptor flags. For the examined
signed integer control it chooses type 0 and the current field's low 32 bits,
which the preceding write has just replaced with the request. A descriptor-class
branch instead uses the argument directly. Other
branches choose float type 1, unsigned type `0x10`, or type 2. The caller cannot
choose an arbitrary tag through this export. A separate `TWXYSetParameterField`
path (`0x391e0` → slot `+0x1e0` → `0x3f310`) copies only descriptor + `0x178`
when mask bit 2 is set; it is not a value setter.

For the named host slot `0x04010000`, the value path calls `0x2c640`. This helper
requires an existing model and a matching parameter in a runtime registry; missing
entries return `0x8000a003` before insertion. It clamps the payload using that
registry's endpoints, then calls `0x175e7c` to update the typed patch record.
The registry's address comes from cache + `0x98`, assigned by `0x2b9b0` at
instance initialization. Its lookup slot `+0x28` resolves to `0x1fdd0` through
vtable `0x234070`.

Producer `0x23740` constructs this registry from the same definition-pointer
array at model + `0xe8` / bounds `0xf4`, `0xf8`. For definitions with + `0x64`
equal to zero and the identifier's top two bits clear, it copies identifier,
type and endpoint/third words into 24-byte
registry entries, keeping the first duplicate identifier. It accepts model
`0x3e700003` explicitly. Registration uses `0x1fa80`; replacement uses `0x1fc90`.
These are reachable cache construction/update paths, not an observed registry
from a running instance. Selection/authorization filters still apply.

Raw instructions `0x2c7bb`–`0x2c7dc` settle an ambiguous decompiler cast: type 1
takes the float branch; every other tag takes **signed 32-bit** comparisons,
first maximum with the lower endpoint, then minimum with the upper endpoint.
There is no tag-equality rejection in this helper. In particular, type `0x10`
does not cause unsigned comparisons here. Existing typed records are updated
with the supplied tag; this is not permission to accept arbitrary types in Rust.
The insertion helper itself copies typed words and has no endpoint check.

Consequently, **if the traced signed 0/1 definition is registered**, this named
export's patch update clamps negative low words to 0 and values above 1 to 1.
That is a conditional software setter contract, stronger than metadata alone.
It does not establish the active registry, a physical guitar/POD domain, units,
reset behavior or Boolean semantics. Importing typed records through the file
reader is a different path and does not automatically call this clamp.

After local insertion, `0x2c640` has optional observer/provider calls and may
return `0xa002`; the descriptor setter also invokes slot `+0x180`. A non-success
return is therefore not proof of rollback. No observer, DSP or device callback
was executed or modeled.

## Third scalar component remains unnamed

The scalar class has an optional clamp: integer setter `0x20d20` invokes slot
`+0x78` (`0x20ff0`) only when its flag is nonzero. Construction `0x20e10` passes
zero, so initialization does not apply that clamp. Float setter `0x20d90` has
the corresponding optional slot `+0x70` (`0x20f90`). These methods consume lower
and upper words, not the third component. The scalar helper returns the lower
endpoint for inverted bounds; the registry's max-then-min branch returns the
upper endpoint. They must not be substituted for one another.

Consumers `0x20ed0` and `0x20ef0` load scalar + `0x14` and pass it back unchanged
when updating the endpoints through the integer/float range setters. `0x20f10`
converts this component when changing scalar representation. It is copied into
the registry above. None of these consumers applies it as a quantization step
or resets the current value to it. The examined paths therefore still do not
prove a default/step/reset meaning. No meaning is assigned from its value 1.

## L6P2 is a two-tone container with a rebuilding writer

The framework's direct patch reader has no `0x410003` dispatch branch. A distinct
implementation exists in the standalone: `L6DocDualTone::LoadFromStream` at
`0x1000fc650`, `SaveToStream` at `0x1000fca5c`, constructor `0x1000fcd58`.
The constructor builds two `L6DocPatch` children at object + `0xc8`, `0x1f8`.
Each child's reader/writer slots resolve to `0x1000f049c` / `0x1000eb2fa`.
The former has the same direct dispatch categories as framework `0x17ac94`;
its native format reader is `0x1000ee29c`. A nested L6P2 is not accepted through
that direct child dispatch. The `L6DocPODPatch` base stubs at `0x1000f2c74` /
`0x1000f2c80` are not these embedded child implementations.

| Examined outer operation | Contract |
| --- | --- |
| Discrimination | IFF category must be `0x410003` (`FORM/L6P2`) |
| `HEAD` | First matching chunk, requested prefix 4 bytes; not exact-length validation |
| `LIST/BANK` | First matching bank is required; searching is independent of its file position relative to HEAD |
| `BINF` | `ReadBank` `0x1000fc818` zeroes and requests 68 bytes; name is terminated at 31 units |
| Embedded tones | First two `FORM` containers in the selected bank, any FORM type at the container-search stage; each child reader validates its own format |
| Child spans | Stored offset includes FORM header; length is declared FORM payload size + 8 |
| Extra/duplicate children | Third FORM is not read; later HEAD/BANK/BINF occurrences are not selected on this path |
| Optional metadata | First `LIST/UNFO`; 14 recognized text tags, first match per tag, no generic opaque-child store |
| Writer | Fresh FORM/L6P2, HEAD(4), LIST/BANK containing BINF(68) and exactly two serialized child forms, then LIST/UNFO |

`WriteBank` at `0x1000fc47c` writes the BINF name as the fixed string `Dual`.
It does not retain an arbitrary original bank name. `ReadUnfo` / `WriteUnfo`
(`0x1000fc246` / `0x1000fc0f8`) use a fixed tag table and rebuild recognized
nonempty values in table order. Unknown outer chunks, a third tone and duplicate
outer records have no replay step in this writer. This establishes losses for
these examined outer structures, not universal handling of every inner chunk.

Container search advances by declared chunk length + 8 (`0x1852c4` in the
framework copy); no odd-length padding increment is present in that helper.
It bounds scanning against stream/container ends and rejects streams at least
4 GiB there. Do not transfer the owned strict IFF inspector's padding policy or
truncation rejection to this native reader. `HEAD` uses an uninitialized local
buffer before bounded copy, unlike zeroed BINF/PINF; a short declared HEAD is
not a defined zero-filled version value.

Failure propagation is partial. Missing BANK or a missing first/second FORM
returns failure. A negative embedded reader result is propagated, but a null
substream can leave that child unchanged without making the overall read fail.
Several open/close/position results are ignored. Missing optional UNFO is allowed;
errors from a found UNFO's text read propagate. Writer child/descend/ascend
failures are generally checked along the two-tone path. No transactional
rollback or malformed-input safety claim follows.

Inner native reads also normalize state. Framework `0x177216` requests 12-byte
MINF/PARM prefixes, maps model `0x3e000000` to `0x3e700000`, inserts models keyed
by slot and returns positive `0x1003` for an existing slot. PARM records are
appended in encountered order, including duplicate IDs; their type words are
not registry-validated here. `0x17745a` stops on any nonzero next-container
search result and returns success, while negative actual model-parser results
propagate. A PARM search/read-loop termination similarly returns success.
Thus malformed trailing search results can be confused with normal exhaustion.
The inner writer rebuilds model/PARM structures rather than replaying the
original child order. This is separate evidence from the earlier PINF limit.

## Full AU dictionary state is another boundary

AU entry `0x2957a` → `0x29800` → `0x7609c` → `0x71f56` dispatches property
get/set. Property 0 reaches `0x6d9aa` / `0x6e59e`, then instance slots `+0xe8`
and `+0xf0`; the AU vtable at `0x244e50` resolves these to `0x9382c` / `0x93a4a`.
Restore requires an 8-byte property payload and global scope before dictionary
handling. These are static call paths only; no component was instantiated.

Save creates a fresh Core Foundation dictionary with identity/name fields,
an instance discriminator (slot `+0x1f0`) and a CFData value keyed by global
`0x2a43e0`. Its opaque state provider
is AU object + `0x218`: readiness uses provider slot `+0x30`, serialization
slot `+0x10`. Restore checks dictionary type, rejects presence of the key at
`0x2a4408`, checks selected identity/discriminator fields and obtains CFData
bytes/length. It then either caches the bytes for later or
invokes provider slot `+0x18` through a memory stream. Unknown dictionary keys
are not replayed by the fresh dictionary writer. Exact dictionary-key strings
and all nested value-type checks are not proved here.

There are concrete error limits: save can return a dictionary with empty data
and success after a negative provider serialization result; restore does not
propagate the provider restore return and can report success. Deferred cached
state is distinct from successfully applied state. At this milestone the
provider binding/payload were unresolved. The subsequent provider pass connects
its constructor and dispatch to an XFP6 state envelope with embedded tone
sections; it is not a whole L6P2 document or a proved complete host session.
The AU also contains a two-tone serializer at `0x1a17fc`, reached by a separate
tone-save path `0xca1fc`; its existence does not connect it to this opaque state
provider at this milestone. Later VST/RTAS entry-point evidence is recorded in
the provider pass; Windows plugin formats remain unproved.

## Reproduction, evidence and remaining definitions

The owned [Ghidra selector](../scripts/PodFarmEdge.java) now supports `name:`,
`scalar:` (including memory displacements), `--list` and qualified callee
coordinates. These select static program data; they do not execute the input.
For an already imported pinned program, use headless `-process`, `-noanalysis`,
`-scriptPath scripts -postScript PodFarmEdge.java --exact` with, for example:

```text
framework: 00045950 0002c640 00023740 0001fdd0 00020ed0 00177216
standalone: 1000fc650 1000fca5c 1000fc818 1000fc47c 1000fc246 1000fc0f8
AU: 0006d9aa 0006e59e 0009382c 00093a4a
```

Raw instructions corrected erroneous decompiler casts in the setter. Failed
mid-function selections remain in private reports and are not accepted function
contracts. All `contracts-<sha>.log/.asm` evidence is retained under ignored
`analysis/pod-farm-258/native-edge/reports/`. The standalone/AU imports used
bounded 60-second auto-analysis; later selections used no analysis. The earlier
two-leaf CPU emulator was not expanded or rerun.

Private manifest (39 reports, including rejected selections and focused test log):
`setter-state-evidence-4e0db6d74ae0374d7aacf084f7f4901d51c75d94cf0396cb837f8bd66760360a.json`.
Focused test-log SHA-256:
`a542989cc66e0d0410818604631fe15ecad3d584922abbea9acfdb375777217f`.

```sh
python3 -m unittest discover -s scripts -p test_pod_farm_resource_contract.py -v
```

**17 synthetic reconstruction checks pass (eight new, nine retained).** New
scope covers signed clamping, low-word interpretation, inverted bounds, first
HEAD/BANK/two-FORM selection, required children and strict truncation. The owned
selection inspector leaves embedded contents uninterpreted, has stricter
length/padding rules, and is not a native compatibility oracle. No full codec,
plugin roundtrip, native malformed-input run or original-library oracle was added.

The reachable contracts above are consolidated, with these precise gaps:

- Scalar + `0x14`/definition + `0x134` still has no proved default/step/reset consumer.
- Actual selected resource/pack state and provider availability remain runtime
  inputs. Cache loader `0x252b0` and registry producer `0x23740` are identified;
  authorization filters were neither run nor bypassed. No PODxt applicability follows.
- The follow-up resolves AU provider + `0x10`/`+0x18` binding and the XFP6 prefix.
  Deferred-state application, complete section semantics and cross-format plugin
  preservation remain unproved.
- Whole inner-document unknown preservation, all compatibility conversions,
  units/scaling and other model domains remain incomplete. The proved outer
  losses prevent a blanket lossless backup claim.

No new missing installer request is made. This pass ends at those unbound state
and semantic definitions rather than treating reconstructed cases as acceptance.
No production code, hardware lock, default math or runtime dependency changed.
Publication and full product checks remain held by the existing coordinator
instruction; exact-head checks and independent review remain required later.
