# Workbench HD static asset-loader evidence

Partial offline source-recovery note for [#31](https://github.com/guirossibrum/VariaxWorkbench/issues/31).
The evidence comes from static inspection of the supplied Workbench HD 2.15
macOS app and its resource files. The vendor app was not run, and no device,
USB, MIDI, or audio interface was contacted. The extracted originals and
analysis output remain private under ignored `.tools/issue-31-analysis/`.
Mac executable SHA-256:
`531bbcf1a187afe4243c9e48af491acb64841f3986383bb099c69fd9a0be1fe8`.

## Recovered asset path

The Mac app's `WB2App::InitInstance` calls
`WB2AssetManager::LoadAllAssets`, then separately loads
`FactoryPresets.bun` and `AltTunings.dat`. `LoadAllAssets` enumerates two app
asset directories. Its `AssetFolderEnumCallback` passes each file to
`WB2AssetManager::LoadAssetFile`.

Static decompilation of `LoadAssetFile` shows it:

1. Opens a candidate as IFF and requires a `FORM` with type `WB2A`.
2. Reads the `AAINF` chunk into asset name/version/ID fields.
3. Iterates `LIST/MDEF` chunks, constructs a `WB2ModelInfo`, associates the
   file path and asset metadata, and invokes a virtual method on the definition
   stream.
4. Obtains a model ID from the loaded definition and indexes the
   `WB2ModelInfo` tree by that ID, dropping a duplicate ID already in the tree.

This establishes a static asset ingestion and model-ID indexing path. The
loader's `GetModelID()` result is the key used in the `WB2ModelInfo` tree. It
does not establish which attached guitar reports a given ID, how discovery
chooses an asset, or whether an editor can successfully communicate with a
guitar. The virtual MDEF reader's complete field semantics and the source of
its model-ID association remain unresolved.

## ALTT definition and value consumers

The ALTT definition is a separate static table in the application, rather than
a record in `WB2Base.ast`. At Mac executable VM addresses, `GetParamDefs`
(`0x1000079b8`) dispatches group ID `0x414c5454` (`ALTT`) to
`AltTuningParamDefs` (`0x10014b0c0`). `WB2AltTuning`'s constructor
(`0x100007a7e`) passes that pointer to `WB2ParamGroup`'s definition
constructor (`0x10000e514`).

`WB2ParamGroup::Reset` (`0x10000e548`) walks 0x20-byte entries until type 2.
For this table, entry 0 has type 0 and entries 1–6 have type 1; the table's
numeric seed slots are zero. The constructor calls `Reset`, so these are
source-backed construction/reset seeds. `WB2AltTuning` then explicitly writes
integer zero to entry 0. This does not establish a guitar's factory tuning,
the displayed tuning after a device read, or a physical default.

`WB2ParamGroup::SetParam` (`0x10000e6d8`) checks that the index is below the
group's count and that the supplied type tag equals the table's tag; it copies
the 32-bit value when both checks pass. It performs no numeric min/max,
scaling, or units check. `WB2AltTuning::SetTune` (`0x100007c5a`) maps its six
slot arguments to ALTT entries 1–6 and supplies float type 1. The standalone
tuning stream reader (`WB2AltTuning::FromStream`, `0x100007ece`) resets the
group, reads `LIST/TUNE` metadata and `STRG` records, then routes record slots
0–5 to entries 1–6 with type 1. The separate SysEx decode path converts
fractional VXWORD values to float before setting entries; this establishes a
conversion call, but not the units or physical string routing of the result.

`wb2.xml` tuning controls declare UI limits −12 through +12 and label slots
String 1/high E through String 6/Low E. Those are UI declarations only. This
trace did not find stored-value acceptance limits, mathematical units, a
range/scaling consumer for the ALTT values, or a physical routing observation.
The table's remaining 0x20-byte record metadata fields have not been assigned
semantics. A bounded 236-byte static-table slice hash is
`d4509e4f83be743a499745ad0bd681ef83fe13e6d8ddfea755681e4839486391`; the
table bytes are not reproduced here.

Function slices are identified by the executable SHA above and these Mac VM
addresses; hashes cover each function's Ghidra-defined instruction body, not
decompiler text:

| Function | VM address | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `GetParamDefs` | `0x1000079b8` | 188 | `4d1fde2221eb9be49b8ebe20f6a011dacaeaf03893aa9d3637c039a51e3e5049` |
| `WB2AltTuning` constructor | `0x100007a7e` | 99 | `b0b39d210b054306799326d485dcc6a7c623d38a563bf1dd709d374a1a5c122d` |
| `SetTune` | `0x100007c5a` | 56 | `ea0459fe86f987699232262e1374cdb29d5a4ddd7243a36424d86a230187e8e5` |
| `FromStream` | `0x100007ece` | 416 | `fc08da7ca8d83b660797208c2202e2c5be3afb808599965f5b22df505aebff2b` |
| `WB2ParamGroup` definition constructor | `0x10000e514` | 25 | `bff7f211caead1e739c8ee0c124b07a3386c58327a2e587319658558b4eb018b` |
| `Reset` | `0x10000e548` | 238 | `81b3f4e800eed72deb2bdc540191b2a35e0699c5b023f5576617551a4708c236` |
| `SetParam` | `0x10000e6d8` | 40 | `8999864a03a6cb0d658678467473f22ff6ad28e0c2a6621c23103f26ec897e15` |
| `LoadAssetFile` | `0x10000f5ce` | 1445 | `77a1d28f5d395bbbbeacfef13d033c120b820ce00710da90d0420536412d69a5` |

## Supplied `WB2Base.ast`

The Mac and Windows Workbench HD 2.15 `WB2Base.ast` files are byte-identical
(12,800,737 bytes; SHA-256
`80286d1101f879affe3ebacc629d229e9690d9f0febd829778ca5a9baab9cfd3`). The
file is an IFF `FORM/WB2A` containing `AAINF` and repeated `LIST/MDEF` asset
definitions. A bounded scan of `MINF` records found 29 names, including
`Neutral Body`, `T-Model Custom`, `Lester Flametop`, `Jazz Super Four`,
`T-Model Neck`, and `Jazz 90 Bridge`. These are asset-definition names, not a
verified instrument compatibility table.

The exact string `ALTT` does not occur in this AST; the application maps that
group ID to its compiled `AltTuningParamDefs` table instead. The UI XML contains
`JTV-59`, `JTV-69`, `JTV-89`, and `JTV-89F` strings, but those strings alone do
not establish support. In particular, this pass found no static mapping from
JTV-69 identity/version to a `WB2ModelInfo` ID or supported model definition.
The `WB2Base.ast` catalog and the editor's generic tuning resources therefore
cannot yet be applied as JTV-69-specific defaults, domains, or protocol values.

The package's English `WB2Strings.lang` resources include a generic
compatibility message requiring a James Tyler Variax version 2.00 or newer and
the Variax USB Interface. This is package-local product/version guidance; this
trace did not connect the localized message to a particular identity-check
branch, and it does not name JTV-69 specifically. It is not evidence of a
validated identity, installed firmware version, or successful connection. The
Mac English resource SHA-256 is
`4a934e577587f61e64b0034ad00682eb400b99b825b1fcd42af230f1f35548e4`.

## #31 admission and prerequisite matrix

| Capability or claim | Status from this trace | Evidence or prerequisite |
| --- | --- | --- |
| Workbench HD 2.15 IFF asset loader and model-ID index | Static path recovered | `FORM/WB2A` → `AAINF` → `LIST/MDEF` → virtual definition reader → `GetModelID()` key; complete MDEF semantics remain open. |
| Generic ALTT parameter-group shape | Static source behavior recovered | Compiled `AltTuningParamDefs`, seven entries, type tags, reset seeds, and index/type acceptance documented above. |
| ALTT accepted numeric domain, units, and remaining metadata | Unverified | Need a source-backed numeric consumer or complete metadata consumer; UI range is insufficient. |
| JTV-69 support and identity/version-to-asset mapping | Unverified; backend admission held | Need a traced compatibility-selection branch or provenance-labelled evidence linking JTV-69 identity/version to an asset model ID. |
| Physical string routing, device behavior, and firmware/update compatibility | Locked / unverified | Requires separate authorization and independent evidence; this static package pass provides none. |

## Boundary and next evidence needed

No JTV-specific value edits, backend admission, protocol implementation, or
simulated device integration is justified by this trace. The precise remaining
source edge is from the identity/version result accepted by the Workbench HD
compatibility check to the `GetModelID()` key used for its `WB2ModelInfo` tree;
for tuning, the table's nonnumeric metadata consumers and any stored-value
range/scaling path are also unresolved. Useful next offline evidence is source
or static call-flow completing those edges, or an authenticated,
provenance-labelled capture linking a JTV-69 identity/version to relevant
asset IDs. Until then, #31 remains pending. Keep `LIVE_ACCESS_LOCKED` and the
Python probe guard locked.
