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
2. Checks for the four-byte `AINF` chunk and reads its asset name/version/ID
   fields. The on-disk bytes at offset `0x0c` are `AINF`; “AAINF” was an
   earlier mistaken label formed by conflating the preceding `WB2A` form type
   with the chunk tag.
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

## Guitar compatibility predicate and model catalog

The English `WB2Strings.lang` has four relevant result keys: `EICV` is the
JTV Variax version requirement, `EICG` is the James Tyler compatibility
message, `EHDU` describes the higher JTV version requirement when connected
through POD HD, and `EVUI` names the Variax USB Interface requirement. These
are not just strings in a resource file: `WB2AppController::OnL6Event`
(`0x100037356`) handles event `0x2a` and maps result values `0x80009011`,
`0x8000900d`, `0x80009012`, and `0x80000005` to those keys, respectively
(the last value maps to `EVUI`).

The producer-side port scan is `WB2GuitarComm::OnDetectGuitars`
(`0x1000159fa`) → `GuitarDetectPortScanCallback` (`0x100015f94`). For a
detected MIDI-device entry whose product ID masked by `0x7f0000` equals
`0x70000`, the callback:

1. Requires exact internal product ID `0x70005`; a different ID in that
   family produces detection status 3, later mapped to `EICG`.
2. Compares the detected entry's `L6Version` field at offset `+8` with
   `0x01990070`; a lower value produces status 2, later mapped to `EICV`.
3. If the MIDI port-pair name contains `POD HD`, compares that same detected
   entry version field with `0x0200006a`; a lower value produces status 4,
   later mapped to `EHDU`.
4. Otherwise it creates a guitar connection record (status 1).

`OnDetectGuitars` maps statuses 1–4 through the four-word table at
`0x1000e83a0` (`0`, `0x80009011`, `0x8000900d`, `0x80009012`) and passes the
result to its observer's vtable slot `+0x30`. `WB2GuitarData`'s vtable base is
`0x10013a8d0`; its slot at `0x10013a900` points to
`WB2GuitarData::GtrNotify_GuitarDetectCompleted` (`0x10000a30c`), which posts
event `0x2a`. The localized wording calls the checked field a JTV Variax
upgrade/version requirement and sets the minimum to 2.00 generally and 2.01
when the port-pair name indicates POD HD. The code compares packed
`L6Version` values; this pass did not recover their complete human-readable
component encoding. The value being compared is on the detected Variax MIDI
entry; `POD HD` is a separate port-pair name test. It checks a generic
JTV-family internal ID, not a JTV-69 model number. It does not compare a
separate POD HD firmware/version field in this predicate.

`EVUI` is selected when event `0x2a` receives `0x80000005`; that error value
has other uses in the executable, and its specific producer for the interface
message was not found in this bounded pass. The text identifies the intended
interface requirement, while the concrete interface-absence predicate remains
unresolved.

The catalog uses a separate namespace. `WB2AssetManager::RebuildAvailableModelCache`
(`0x10000f314`) iterates loaded asset model IDs and calls `GetModelData`
(`0x10000fea2`). For body-category IDs (`id & 0xff0000 == 0x100000`) it keys
the body map by decoded-data word index `0x42d` (byte offset `0x10b4`) and
stores the asset model ID as the value. For pickup-category IDs
(`id & 0xff0000 == 0x110000`) it keys the pickup map by the first word of
decoded model data and stores that asset model ID. `BodyModelIDForVariaxID`
(`0x100010022`) and `PickupModelIDForVariaxID` (`0x100010072`) look up those
maps. Thus the app has a source-backed VariaxID-to-body/pickup-asset lookup,
but the detected-device product/version predicate and these model-data
VariaxID keys are distinct inputs. The examined cache builder visits all
loaded model assets; no current-device-specific asset filtering is visible in
this path. This does not establish a JTV-69-to-model-data mapping or that every
loaded asset is valid for every connected device.

Function slices below use the Mac executable SHA listed above. The result
table hash covers 16 bytes at `0x1000e83a0`; proprietary bytes are not included.

| Function / data | VM address | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `GetModelData` | `0x10000fea2` | 296 | `0b044e53bb52b55322e4c49bd9d96110ceaad95d79620bd70822c725f7a14b0f` |
| `RebuildAvailableModelCache` | `0x10000f314` | 336 | `a50d72d727c2e1c9f8343ec7871989a2f6ce7600b8db553bfbe713206feef74f` |
| `BodyModelIDForVariaxID` | `0x100010022` | 80 | `4315773dfaee71c77ef53381ef3c68afeca8cabcd28ed02e13e2734f9068613f` |
| `PickupModelIDForVariaxID` | `0x100010072` | 83 | `96d14e673cf8bfccacf0ffd5107eb87b3cbf51114e7be05249241a217ae4ae66` |
| `OnDetectGuitars` | `0x1000159fa` | 226 | `23a92f01aabdac48f4ae18c98d00fda0997ea4b444c20eb7bd6923d024ca5855` |
| `GuitarDetectPortScanCallback` | `0x100015f94` | 550 | `af70b23124c95b7440bd964c36d99ee76753ddcbeee91f4fb8714aa0b0868575` |
| `OnL6Event` | `0x100037356` | 1313 | `528a27d7a74d3a5bd3aeb27a279f7e4c79acb51bcc0916fa3ba54deb5b86cbd4` |
| Result map | `0x1000e83a0` | 16 | `8f78c049bf1a80165dd17b2c6d4bc9d7c83513e14a13f30cd158b0b7808728c8` |

## Supplied `WB2Base.ast`

The Mac and Windows Workbench HD 2.15 `WB2Base.ast` files are byte-identical
(12,800,737 bytes; SHA-256
`80286d1101f879affe3ebacc629d229e9690d9f0febd829778ca5a9baab9cfd3`). The
file is an IFF `FORM/WB2A` containing `AINF` and repeated `LIST/MDEF` asset
definitions. A bounded scan of `MINF` records found 29 names, including
`Neutral Body`, `T-Model Custom`, `Lester Flametop`, `Jazz Super Four`,
`T-Model Neck`, and `Jazz 90 Bridge`. These are asset-definition names, not a
verified instrument compatibility table.

The exact string `ALTT` does not occur in this AST; the application maps that
group ID to its compiled `AltTuningParamDefs` table instead. The UI XML contains
`JTV-59`, `JTV-69`, `JTV-89`, and `JTV-89F` strings, but those strings alone do
not establish support. This pass recovered a generic JTV-family internal
device ID and a separate catalog VariaxID-to-body/pickup-asset mapping, but did
not link a JTV-69-specific model variant to particular catalog entries. The
AST catalog and editor's generic tuning resources therefore cannot yet be
applied as JTV-69-specific defaults, domains, or protocol values.

The Mac English resource SHA-256 is
`4a934e577587f61e64b0034ad00682eb400b99b825b1fcd42af230f1f35548e4`.

## #31 admission and prerequisite matrix

| Capability or claim | Status from this trace | Evidence or prerequisite |
| --- | --- | --- |
| Workbench HD 2.15 IFF asset loader and model-ID index | Static path recovered | `FORM/WB2A` → `AINF` → `LIST/MDEF` → virtual definition reader → `GetModelID()` key; complete MDEF semantics remain open. |
| Generic ALTT parameter-group shape | Static source behavior recovered | Compiled `AltTuningParamDefs`, seven entries, type tags, reset seeds, and index/type acceptance documented above. |
| ALTT accepted numeric domain, units, and remaining metadata | Unverified | Need a source-backed numeric consumer or complete metadata consumer; UI range is insufficient. |
| Generic JTV compatibility gate | Static predicate recovered; model-specific support remains unverified | Internal product ID `0x70005`, detected-entry `L6Version`, and host port-pair name produce distinct JTV/POD HD results. `JTV-69` is not named by this predicate. |
| Asset availability mapping | Static catalog path recovered; device applicability unverified | Decoded body/pickup model data supplies VariaxID keys mapped to asset model IDs; no device-specific filtering is visible in the cache builder. |
| JTV-69-specific model availability | Unverified; backend admission held | Need a source-backed link from JTV-69-specific model variant to relevant decoded body/pickup catalog entries, without assuming equal identifier namespaces. |
| Physical string routing, device behavior, and firmware/update compatibility | Locked / unverified | Requires separate authorization and independent evidence; this static package pass provides none. |

## Boundary and next evidence needed

No JTV-69-specific value edits, backend admission, protocol implementation, or
simulated device integration is justified by this trace. The connected path
establishes a generic JTV-family/product/version gate and a separate
VariaxID-to-body/pickup-asset cache. It does not identify JTV-69-specific body
or pickup keys, show device-specific asset filtering, or resolve all loaded
model definitions. The actual producer of the `EVUI` result and ALTT's
nonnumeric metadata consumers and stored-value range/scaling path also remain
unresolved. No device input is requested. Until these source edges are
recovered, #31 remains pending. Keep `LIVE_ACCESS_LOCKED` and the Python probe
guard locked.
