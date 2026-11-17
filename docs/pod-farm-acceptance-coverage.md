# POD Farm 2.58 consolidated offline handoff (#40)

This consolidates the supplied original #40 scope after the bounded preset/state/
conversion traces. It adds packaged pack identifiers, their loader roles, a
concrete VST automation path and a capability/adaptation assessment. It is a
**partial research handoff, not issue acceptance or #29 release**.
No product Rust/default/math/dependency/guard changes were made. No device/audio
access, vendor execution, authorization/credential inspection, new CPU emulation
or external service access occurred. Vendor artifacts and original-derived
reports remain private and ignored. Proposed follow-ups below are drafts for the
Gatekeeper; none has been published as a separate follow-up issue or PR.

## Current publication and check status

[PR #84](https://github.com/guirossibrum/VariaxWorkbench/pull/84) is published
against `master`. The publication/access prerequisite is resolved; no further
publication/access approval is requested. All four configured required checks
passed on head `685e75153713041c6483e928ed7650e58c1137be` in coordinator run
`beb6241a-19d1-4569-b362-5059d76dbe97`: `functional:cargo-all-features`,
`functional:cargo-gui-usb`, `lint:cargo-clippy` and `format:cargo-fmt`.

Independent review requested a narrow documentation-status correction. Every
changed head requires fresh configured checks and exact-head review; the pass
above covers only its named head. Independent review and subsequent merge-result
CI remain required. #40 remains open with incomplete acceptance criteria, #29
value editing stays held, and all hardware/private-source boundaries remain.
Older per-pass publication/check hold statements are historical and superseded
by this section. The 69 synthetic methods and historical lookup oracle retain
their separate scopes; product-check success does not establish native/state/DSP
or hardware equivalence. The follow-up issue texts remain unpublished drafts.

## Source coordinates and evidence kinds

| Source | SHA-256 / provenance |
| --- | --- |
| Supplied DMG | `483e4129f2d319665ab7273194818acf570181cac8eb8f75dc5f96581715217e`, 201,113,929 bytes; reverified unchanged in this pass |
| Framework x86-64 slice | `dced9e6bee1ac57ba9135c1b8751d26141be82155122f7aa151b2cb457bd55a1`; full framework `1b6acb3aa012d9b884f00daeacc948aa80884c81f513871be740055cd629a924` |
| Full VST i386 slice | `e68b3d63d0cda33d6d3e3443bfd472fa412ac1fa5ae7636d7d532ed16369a0c4` |
| Full AU x86-64 slice | `715d30b8fed77d4ea18150050131db759b8e8a27eeaf4e3d9b3872b6bca6fbe0`; earlier state/conversion traces |
| Bundled guide | `4ad51a04da44a1fa5d25c8c67563ad0f8030c8522a2675a1aa6859650d1af76b`; printed page citations below, not current OS documentation |
| Resource hashes | All eleven full hashes in owned `scripts/pod_farm_pack_inventory.py`; original resources remain local |

Framework/VST addresses below are slice virtual addresses, not device addresses.
Static Ghidra 12.2 selections and raw GNU objdump checks read pinned copied files;
they do not invoke the code. The prior 3,318-case two-leaf CPU oracle belongs only
to selector/lookup behavior on synthetic memory; it was not expanded or rerun.
No original-library oracle or real host/device session validates these additions.

## Pack definitions, eligibility and runtime state

Resource directory: framework `Versions/A/data/res/`. All eleven resources are
FORM/L6RB with a 56-byte AINF at file payload offset36. They contain these derived
identifiers/words and layout definitions:

| Resource | AINF asset ID | Asset version | Eligibility word | LOUT model layouts |
| --- | --- | --- | --- | --- |
| GP2 | `0x7d000001` | 2 | `0x3fbe` | 85 |
| GP3Std | `0x7d000003` | 2 | `0x3fbe` | 18 |
| HiGainPack | `0x7d008001` | 1 | `0x1` | 18 |
| ClassicAmpsPack | `0x7d008002` | 1 | `0x2` | 18 |
| FXExpansion | `0x7d008003` | 1 | `0x3` | 35 |
| BassPack | `0x7d008004` | 1 | `0x4` | 50 |
| GP2Ext | `0x7d008005` | 2 | `0x101` | 57 |
| Z1Std | `0x7d008007` | 2 | `0x8001` | 17 |
| Z5Std | `0x7d008009` | 2 | `0x135` | 31 |
| ilxstd | `0x7d00800a` | 2 | `0x133` | 101 |
| ilxplt | `0x7d00800b` | 2 | `0x134` | 178 |

These are **packaged asset identifiers and selector words**, not a recovered
complete license-product namespace. Filenames are source labels; inferred marketing
names are unnecessary. Each resource's layouts have distinct model IDs locally;
608 layout records across files are not 608 unique/available/authorized device
models. Cross-resource repeats and host system/UI models prevent that conclusion.
The private survey retains explicit model membership and source offsets/hashes.
A LOUT definition is evidence of a packaged model layout, not transferable DSP.

Framework reader `0x18629c` copies HINF8 and AINF56 into the resource document.
AINF +0 is asset ID, +4 asset version, +8 an unnamed word (zero in these files),
+12 the selector/eligibility word, +16 a short label field. Loader `0x252b0`
passes ID and selector to `0x23ea0` **before** model-cache insertion. It also
checks header-linked binding/expiry conditions (+32/+28 respectively), and retains
opaque context fields. Those fields describe a packaged resource schema, not the
schema of a user's license store. They are not authorization tokens to generate
or modify. No such operation was attempted.

`0x23ea0` branches on selector/product/interface state, consults a resource-ID
lookup (`0x18b07a`) and runtime interface checks, and returns statuses consumed
by `0x252b0`. Some selectors have special branches; a zero/nonnegative result is
not a universal statement that all packs are available. The caller can store a
resource with different status classes before selectively registering models.
That means file present, resource parsed, model registered, model enabled and
model authorized are distinct states. None of the latter runtime states was
observed. Static filters were read, never executed, replaced or bypassed.

The guide pp.1•3–1•7 describes Standard/Platinum, optional packs, computer/device
and iLok authorization roles; p.3•17 distinguishes Edition and Model Pack columns.
The package supplies License Manager1.08, Monkey1.68, framework/driver components.
Their presence establishes companion software roles, not a supported Linux
inventory API. Legitimate independent inventory can report **packaged definitions**
with provenance now; actual installed/authorized state must remain unknown until
a separately authorized, read-only schema/API is established. No user license
storage, credentials, actual entitlement list or activation service was inspected.
No inference transfers a host pack/license to PODxt hardware or equates pack
management with firmware maintenance.

## Concrete host API / automation mapping

The guide pp.3•36–3•38 documents sixteen assignable knob slots, sixteen switch
slots and eight fixed controls, with per-model-instance/Tone A/B assignment.
It explicitly puts automation assignments in the DAW project, not tone presets.
These documented slots are host-facing controls, not PODxt PARM IDs.

The full i386 VST provides a concrete callback path beyond symbol inventory:

| Layer | Recovered connection |
| --- | --- |
| Constructor | `0x29a6e` →`0x979e2` →`0x2aa2c`; initializes AEffect inside object+`0x20` and root vtable`0x21bfc8` |
| Host set/get pointers | AEffect+`0x0c`=`0x2a352`, +`0x10`=`0x2a33e`; recover owner through AEffect+`0x40`, call root slots+`0x2c`/`0x30` |
| Adapter set/get | root`0x97290`/`0x972ca` → provider+`0x48`/`0x4c` (`0x75994`/`0x74e68`) |
| Host-index translation | `0x8b50c`/`0x8b690` use optional map via`0x9f074`, then descriptor interface+`0x10` |
| Index map | 24-byte entries, first internal-ID word or -1 sentinel; alternative name lookup`0x9f238` selects between two entry words using its flag. Bound word has inclusive comparison; allocation/highest-index semantics are not assumed. |
| Value conversion | `0x8a8bc`/`0x8ab02` select association, boolean or numeric conversion using descriptor flags; numeric branches distinguish raw float, signed integer and split-word forms. Replaceable callback interfaces perform the actual scaling. |
| Application dispatch | Raw switch at`0x8b5fc` resolves category0 to false,1→`0x8b090`,2→`0x899e0`,3→`0x89a94`,4/5→`0x89b12`. These use descriptor/current context, setters and notification paths; this is not a single universal linear scale. |
| Name/display | dispatcher opcodes6/7/8 reach bounded label/display/name methods through provider; `0x97a9a` requests up to8 label bytes, `0x9737a` up to24 name bytes; `0x9730e` first gets value then formats through provider+`0x54` |

The category4/5 apply helper calls descriptor slot+`0x24` but returns false, so
false does not prove no mutation. The VST setter discards provider status; the
getter explicitly supplies zero only for a null provider and does not establish
a deterministic fallback for every failed provider read. These are static
error-propagation limits, not observed host failures.

Display `0x8b948` uses descriptor-dependent association names, on/off, fixed decimal
or selected percent/left/right strings. Display strings therefore cannot define
hardware units without a specific producer/consumer link. No new default/step/
unit meaning is proved by generic formatting branches.

The constructor initially supplies 25 in the AEffect parameter-count field; the
guide describes40 controls. The final initialized parameter list, index bindings
and any later count change are not established by this static constructor edge.
This is an unresolved setup/format distinction, not an observed host failure.
A stable40-entry mapping or interchangeability across AU/VST/RTAS must not be
invented from the guide. The concrete descriptor and map interfaces are the
remaining binding boundary; native instance execution was not used to fill it.

State remains a separate mapping: concrete AU/VST/RTAS wrappers reach XFP6,
with tone spans, L6ME mappings and 1MA6 settings. The settings callback serializes
selected positional records, but its complete relationship to all host automation
assignments is not proved. A tone converter cannot therefore promise to retain
DAW automation. The earlier borrowed/cached buffer lifetimes, wrapper status
projection differences, unknown-data loss and AU deferred/RTAS outer-ID gaps
remain in [provider](pod-farm-plugin-provider.md),
[sections](pod-farm-xfp6-sections.md) and [tone](pod-farm-tone-coverage.md) ledgers.
Some exploratory mid-function selections resolve to containing functions; only
connected edges and the decoded raw switch above are accepted evidence.

## PODxt Live capability / consequence matrix

This compares actual #40 findings with the independent existing
[POD tone librarian](pod-presets.md); it does not repeat its Workbench name search.

| Requested capability | POD Farm evidence | Unified Rust consequence / unresolved boundary |
| --- | --- | --- |
| Device identification / pack carrier | Guide historical support for PODxt license/audio role | Not hardware editor support; no active device or entitlement evidence |
| Control editing and amp/cab/effect selection | Host TWXY descriptors, packaged layouts and conversion IDs | Host domains cannot unblock #29; independently applicable PODxt ID/domain/scaling proof still required |
| Global vs preset settings | Host mixer/routing, XFP6 context/settings, generic command adapters | No recovered PODxt global/settings codec; do not send host words to hardware |
| Routing/input/position | Guide pp.2•9,2•18 leaves input selection on device; host tone A/B graph | No physical routing command, ordering or USB-stream behavior validated |
| Tone names/import/export | PINF/L6PA/L6FS/L6AS and lossy L6P2/child reconstruction | Existing Rust librarian retains its target`0x3000a`/opaque-byte policy; this research adds no full codec or lossless plugin conversion |
| Library/setlist order | Guide pp.3•20–3•21 describes bank/collection import as host setlists/shortcuts | Host MIDI ordering is not PODxt memory addressing; bank base, slot index and device ordering remain unknown here |
| Save/recall and bank switching | Host file/state writers and callbacks | No hardware save/recall or bank command inferred; none authorized |
| Required packs/models | Packaged resource membership and conditional loader eligibility | Report packaged definitions, leave installed/available/authorized unknown; no license/model transfer claim |
| Backup/maintenance | Preset metadata and examples, bundled companions | Not full backup/recovery; no device capture, hardware pack write, flash/update/reset or physical safety claim |
| Audio/DAW use | Historical host-DSP/audio-interface distinction; archived plugin ABI | No Linux audio streams or equivalence established; #27/#28 retain their independent boundaries |

Source-established losses remain explicit: PINF extensions/name tails, rebuilt
outer chunk order/duplicates/unknowns, model/parameter conversions and positional
settings are not generally preserved. Native short-copy/search behavior is
permissive; mapping/settings length words do not cap reads. Restore can partially
mutate context/tones/settings/mappings and hide an application error. Strict
owned audits intentionally reject more than the native code.

## Modern DAW / portability feasibility

This is an engineering assessment of the supplied artifact, not a claim about
currently tested DAW/OS versions. No current host compatibility was tested.

| Path | What the evidence supports | Required work / blocker |
| --- | --- | --- |
| Bridge supplied VST/RTAS on Linux | i386 Mach-O and legacy macOS ABI/imports are identified | CPU-width bridging does not supply Carbon/CoreFoundation/CoreMIDI/Line6 runtime or legitimate authorization. No working Linux bridge established. |
| Wrap supplied AU | i386/x86-64 AU, Cocoa view helpers, host callbacks | macOS dependencies and no ARM slice in this package; current macOS/host compatibility and authorization remain untested. Not a portable Rust DSP core. |
| Offline tone/state conversion | Partially recovered headers, model/parameter projections and known losses | Implement explicit supported subsets/targets, preserve originals/opaque data or report losses, map automation separately; unsupported models must not silently substitute. No DSP/audio equivalence follows. |
| Independent Rust core plus VST3 / CLAP / LV2 adapter | Candidate architecture compatible with the product's self-contained-core requirement | Independently define DSP, deterministic math, parameters/stable IDs, routing, state migration and offline audio references; select/pin an appropriately licensed maintained ABI/toolchain later. No dependency adopted or format implemented here. |
| Independent AU adapter | Same independent core could have a thin platform adapter | Separate macOS ABI/UI/host integration and validation; does not make the original AU portable or prove cross-host state equivalence. |

VST3/CLAP/LV2/AU are potential adapter targets, not substitutes for the missing
engine or authoritative definitions. MIDI control assignments, host automation,
preset serialization and audio processing are separate contracts. A future
implementation needs versioned Rust codecs, deterministic coefficient math,
replaceable OS/plugin adapters, atomic non-overwriting output and explicit loss
reports. Original assets/code, research MPFR/historical runtimes and Line6 services
must not become normal app dependencies. Static pack IDs do not grant rights to
redistribute models, DSP code or proprietary resources.

## Acceptance coverage against the supplied eight criteria

| Criterion | Recovered evidence / validation kind | Remaining definition/input and downstream consequence |
| --- | --- | --- |
| 1 Preserve/inventory | Hash/size reverified; prior extraction inventory:16 packages,1,551 regular payloads,62 Mach-O files,1,120 tones; static source facts | Signature trust/authenticated download provenance not established; inventory does not establish execution compatibility |
| 2 Native controls/protocol/resources | Concrete lookup/provider/metadata/clamp/conversion paths; source facts, owned reconstruction, historical two-leaf CPU oracle only | Full PODxt-specific definitions/protocol and active registry/pack inputs absent; #29 editing remains held |
| 3 Capability matrix | Matrix above plus guide and existing independently recovered librarian scope | No live global/routing/save/bank/backup validation; no authorization to attempt it |
| 4 Packs/entitlements | Eleven asset IDs/selectors, packaged model-layout membership, pre-registration loader roles, companions | User storage schema/runtime installed/available/authorized state and device entitlement transfer unresolved; legitimate read-only inventory limited to package facts |
| 5 Formats/preservation | PINF/L6P2/XFP6/L6ME/1MA6 and child reader/writer contracts, documented loss and mutation | No complete lossless Rust conversion/host automation migration or PODxt bank/slot codec; originals remain necessary |
| 6 Modern plugin adaptation | Real architectures/imports plus concrete VST parameter/state paths; feasibility assessment | Initialized host-ID table/count, complete AU/RTAS automation mapping, portable independent DSP and ABI validation missing; no working bridge claim |
| 7 Offline validation | Eight owned suites,69 synthetic test methods pass; hash-pinned private surveys/reports; no devices | Merge-result CI and original-state/audio oracle validation remain outstanding; the required prior-head selection is recorded above. Fake/file-backed audio is an independent existing scope, not validated by these synthetic tests |
| 8 Delivery/follow-ups | Derived ledgers, matrices, owned helpers/tests and unpublished drafts below; PR #84 published and prior-head required checks passed as recorded above | Changed heads require fresh checks and exact-head review; independent review and merge-result CI remain required. Issue acceptance remains incomplete/open; no publication/access decision is pending for this milestone. |

## Reproducible checks and units

All commands below run offline from the owned issue worktree. The new header audit
requires exact AINF56, one header and complete FORM/padding. Those are **research
restrictions**, not proof that the permissive native reader rejects extensions,
duplicates or short reads. It returns unknown installed/authorized state even for
a valid packaged header.

```sh
python3 scripts/pod_farm_pack_inventory.py
python3 -m unittest discover -s scripts -p 'test_pod_farm_*.py' -v
```

The first command hash-checks eleven packaged resources and writes a new private,
content-addressed report with the existing atomic non-overwriting helper. It is
not an installed-license query. The second runs exactly these **69 test methods**:

| File suffix after test_pod_farm_ | Methods | Kind/scope |
| --- | --- | --- |
| inventory.py | 10 | Synthetic file-reader/audit guards |
| metadata_words.py | 6 | Owned descriptor-word reconstruction |
| resource_contract.py | 17 | Mixed strict resource/dual-tone inspection and PINF/clamp reconstruction |
| state_prefix.py | 8 | Fixed-prefix/status reconstruction |
| section_audit.py | 8 | Strict framing audit |
| tone_class.py | 6 | Classifier reconstruction |
| legacy_scalar.py | 8 | Integer/category reconstruction |
| pack_inventory.py | 6 | New strict packaged-header audit |

This consolidates prior30 plus33 earlier methods plus6 new methods. Nested byte/
truncation cases are not counted again. The historical3,318 CPU-emulated cases and
five static seed-call checks are separate historical evidence, not rerun or added
to69. These tests do not prove native acceptance, DSP/audio equivalence or physical
safety. The package survey records608 layout rows; that is another unit, not a test
count. Source reports/evidence files are counted separately in the manifest.

For static reproduction use the existing hash-pinned projects and owned
`PodFarmEdge.java --exact`: framework`0018629c 000252b0 00023ea0`; VST i386
`0002aa2c 0002a352 0002a33e 00097290 000972ca 00075994 00074e68 0008b50c
0008b690 0009f074 0009f238 0008a8bc 0008ab02 0008b948 0008b3a6`.
The private resource survey walks LAYO/LOUT headers and retains their model IDs;
no proprietary model table is tracked. No expanded wrapper/cache/RTAS-ID search,
transcendental emulation or vendor execution was used.

## Proposed follow-up issue text — unpublished drafts

These are suggestions for Gatekeeper disposition, not newly assigned tasks or a
second queue. Dependencies refer to existing native prerequisites.

**Draft: Establish independently applicable PODxt Live parameter definitions.**
Recover target-specific IDs, representation, scaling, domains and actual units/
default/step consumers from independently pinned sources. Separate host-resource
metadata from target firmware/control semantics. Acceptance: source-linked per-
control definitions and offline boundary/model-mismatch tests; preserve unknowns
rather than fill gaps from names. Dependency: #40 partial findings plus applicable
#20/#21 owner evidence; coordinate reserved ownership. Blocks #29 value editing.

**Draft: Define preservation-first tone/plugin-state interchange.**
Specify supported target/product/version subsets and explicit losses for PINF,
L6P2, XFP6, nested models, mapping/settings and automation. Acceptance: bounded
Rust parsers, opaque-byte preservation or explicit rejection/loss reporting,
atomic non-overwriting saves and independently validated malformed/duplicate/
ordering/roundtrip fixtures. Dependencies: reviewed #40 contracts and applicable
parameter/model definitions; no vendor/state/DSP-equivalence assumption. Does not
authorize live import, recall or bank operations.

**Draft: Recover read-only installed/authorized pack inventory contract.**
Document a legitimate, separately authorized schema/API for installed definitions
versus active host/device entitlements, using synthetic records and explicit
unknown state. Acceptance: provenance/version handling, no license transfer or
activation/deactivation, no credentials or entitlement fabrication, no services or
hardware required for normal operation. Dependencies: reviewed package schema,
operator scope for any future private input; coordinate #30 maintenance separately.

**Draft: Specify independent Rust DSP and modern plugin adapter validation.**
Choose one adapter only after evaluating maintenance/licensing/pinning/replacement;
keep DSP/math/state and stable parameter IDs in an independent Rust core.
Acceptance: deterministic offline audio/automation/state reference contracts and
host-specific adapter tests before any compatibility/equivalence claim.
Dependencies: authoritative per-model processing/parameter definitions, #26
architecture and #27/#28 audio boundaries. A bridge experiment with proprietary
binaries requires its own scope; this issue does not authorize vendor execution.

The finite remaining issue-acceptance blockers are the definition/validation gaps
and explicit runtime inputs above. PR #84 is published, and the required checks
passed on the head recorded in the current status section. Changed-head checks,
independent review and merge-result CI remain required; publication/access is
not an unresolved prerequisite. No new missing installer, human decision card,
access poll, cleanup or no-PR acceptance is requested.

Private manifest: `acceptance-evidence-5a5c7efa6295133cefd18d0432a768cf151fdb3e8e9450b215f18ff6f2da6a55.json` (16 new evidence files: twelve
Ghidra logs, two packaged-resource JSON reports, one raw switch range and one
combined test log). Combined69-method log SHA-256:
`3ffa53eaacbc16f9709e5302dbac6eac44b0ef64b74e001c9182348c2d2985fd`.
The manifest retains a superseded private count draft for traceability; it excludes
the earlier provider manifest from the new-file count and uses the computed608
layout-row sum. These corrections do not change source facts or test results.
