# Spider V30 static trace for Blackface Lux Drive

[#311 assignment](https://github.com/guirossibrum/VariaxWorkbench/issues/311#issuecomment-7224743104) · [Pinned desktop builds](spider-v30-static-trace.md#reproducible-binary-references) · [Preset codec](spider-v30-preset-format.md)

This trace follows one non-HD amp model, `BlackfaceLux`, and its `Drive` control in Spider V Remote 2.01. Both desktop binary preset writers resolve the same model and control keys for product `0x220000` with the bundled Parker default symbol table. The effective device symbol table, firmware scaling and live-edit protocol remain open.

## Product and catalogue guards

| Evidence scope | Windows PE32 engine | macOS x86-64 main |
| --- | --- | --- |
| **V30 static** | USB PID `0x5068` resolves to product `0x220000` in `0x10055150`. Editor initialization `0x10078e90` includes that product in `0x220000..0x220005` and selects `defaultSymbolTable_Parker.bin` at xref `0x100792bf` → `0x101755dc`. | Product resolution `0x1000fb6a8` makes the same join. Initialization `0x1000422f8` selects the Parker filename for that range at xref `0x100042563` → `0x10031c679`. |
| **Family static** | Catalogue rebuild `0x100a1bc0` uses the supplied product and version to filter rows with `0x100a25a0`, then indexes eligible rows by `symbolicID`. The `devices` xref is `0x100a25ee` → `0x101764a0`. | Rebuild `0x100070570` calls `0x10007078e` and also indexes eligible rows by `symbolicID`. The `devices` xref is `0x1000707bf` → `0x10031cdfc`. |

For bare numeric entries, an absent or empty `devices` array imposes no allowlist, a nonempty array requires the current product, and a matching `exclude_devices` entry rejects the row. The predicates also parse versioned objects. Their version rules are outside this slice.

The Windows and macOS `amps.models` share the parsed `BlackfaceLux` record at zero-based row 1, with numerical catalogue ID `0x20014`. This row has neither product list and passes the observed catalogue predicate for `0x220000`. The Windows-only `amps_hd.models` gives a negative control at row 7. Its distinct `HD_AmpBlackfaceLuxNrm` model (`0x70003`) lists only `0x200004` and `0x200006`, which excludes V30 when the predicate is applied. Neither row alone establishes the final runtime model list.

## Model and control representation

**Family static units.** The `Drive` definition at `amps.models` → row 1 → `params[3]` has `valueType: 1`, bounds `0.0..1.0`, default `0.75` and no unit field. Windows `0x100a1fc0` and macOS `0x1000714a0` handle that catalogue type as a floating-point scalar and apply the configured bounds to numeric values. The desktop representation is dimensionless and normalized. Display scaling, firmware quantization and physical gain units remain open.

**Windows family preset-body path, applicable to the V30 product branch.** The full preset writer `0x1008e850` includes the `Amp` group (xref `0x1008efcd` → `0x10175904`). The product table selected by `0x10083aa0` for `0x220000..0x220005` maps `Amp` to the desktop `amp` object (row at VA `0x1018e4f0`). The group writer `0x1008e710` checks the group symbol before writing it. Its group-body writer `0x1008f7b0` reads `amp.@model` and resolves the model string in the supplied symbol table. A missing model returns an error before model emission. For ordinary control keys such as `Drive`, the catalogue-backed path validates the parameter and passes it to `0x1008e7b0` (call at `0x10090c80`). That function appends `State`, then requires an exact symbol match before producing a control record. Lookup `0x100ac660` uses the name's CRC-32 (`0x1009ddb0`) and then compares the name bytes, returning the zero-based index or `0xffffffff` on a miss.

The pinned, byte-identical Parker defaults yield the following join. The file offsets apply to both copies.

| Role | Name | Index | Record / name offset | CRC-32 |
| --- | --- | ---: | --- | --- |
| Group | `Amp` | 42 | `0x200` / `0x2c7e` | `0x0e4f5a33` |
| Model | `BlackfaceLux` | 280 | `0xd28` / `0x36a6` | `0xa7bb2aa8` |
| Catalogue control spelling | `Drive` | 38 | `0x1d0` / `0x2c55` | `0xa9dcda8b` |
| Serialized control key | `DriveState` | 51 | `0x26c` / `0x2cc4` | `0x16dbee75` |

With this table the `TRTG` group-open record carries index 42, and the model record carries index 280, both with type 7. The model emitter `0x10095ca0` writes the count, then the parameter records. The `DriveState` record uses index 51 as its key. `0x100878c0` converts the internal float variant to record type 3 and copies its four-byte value. `0x100954a0` writes the type and raw value little-endian. Catalogue ID `0x20014`, symbol indices, and CRC-32 values occupy distinct fields or lookup roles. These indices are conditional on the selected table. Both initializers also pass a `cachedSymbolTable.bin` path to their service constructors, so the pinned default indices cannot stand for an attached unit's effective table. No authentic V30 tone or device symbol table was examined.

## macOS binary preset-body path

1. The binary conversion wrapper `0x1000a6dd4` passes the `Preset` selector and supplied symbol table to `0x1000a6e3c`. This writer opens the version-1 `TRTG` section at call site `0x1000a7050` → `0x100106bb4`. The full-preset branch references `Amp` at `0x1000a75ef` → `0x10031c0e0` and calls the group writer `0x1000b6bd0` at `0x1000a7603`. The writer's `amp` subset branch also calls it at `0x1000a7dfc`.
2. The group writer checks `Amp` in the supplied symbol table, opens the group via `0x100106a74`, then calls the body writer `0x1000b6c50`. The body writer classifies `Amp` as local selector `0x0d` using `0x1000b2eb0` and the row at `0x10036d690`. This selects its generic model branch. The product mapper `0x10001cbaf` picks table `0x10036ac10` for `0x220000..0x220005`. Its row at `0x10036acd0` maps `Amp` to `amp` at `0x10031c028`.
3. The body writer reads `amp.@model` and resolves the selected name at `0x1000b7996` → `0x1000d2460`. The lookup computes CRC-32 in `0x100054c8e`, checks the matching name bytes and returns the zero-based index or `0xffffffff`. The body writer also finds the catalogue entry by that model name via `0x100070fae` and validates the ordinary `Drive` field at `0x1000b7d9d` → `0x1000714a0`. The call at `0x1000b8054` → `0x1000b9ad8` appends `State` and requires an exact lookup of `DriveState` before preparing a type-3 record with the validated float's four bytes. A missing model symbol prevents model emission. A missing `DriveState` symbol prevents that parameter from entering the emitted count.
4. The body writer calls `0x100106ad0` at `0x1000b8560` to emit the model, count and prepared parameters. `0x10010656c` writes each 32-bit key. `0x10010624a` writes the 16-bit type and, for the model, count and float, a 32-bit payload. These x86-64 stores are little-endian. `0x100106aa4` closes the group after a successful body write.

The table compares the record fields in macOS with those in the landed Windows trace. The payload indices assume the pinned default table. The count includes every parameter emitted for the model.

| TRTG record | 32-bit key | 16-bit type | Payload |
| --- | --- | --- | --- |
| Open `Amp` | `0x80000003` | 7 | Symbol index 42 |
| Select `BlackfaceLux` | `0x80000005` | 7 | Symbol index 280 |
| Count parameters | `0x80000006` | 2 | 32-bit count |
| Set `DriveState` in the model record | 51 | 3 | Binary32 normalized value |
| Close `Amp` | `0x80000004` | 0 | None |

The static match covers this preset-body record sequence on both platforms. It leaves open the effective symbol table on a V30, the firmware's accepted range and gain mapping, and the bytes or opcodes used by a standalone model change or Drive edit on the wire. This checkpoint used the packaged resources and desktop code. An authentic V30 preset, device table and live capture are still needed to verify those claims.

## Reproduce offline

Use the existing extraction under `backups/spider-v30-audit-311.OIUQg5qW/` in the main checkout. Resource roots are `6/Spider V Remote/res/` and `7.payload/Spider V Remote.app/Contents/Resources/`.

| Resource | Windows SHA-256 | macOS SHA-256 |
| --- | --- | --- |
| `amps.models` | `968ff7d4eaa870a50521f01e7d5950eec8e3dbc379a4ed7da569df04ad40535e` | `b816e5a5e9870971ddf16271f187a25db2ccd0422c3474d33af0e7c06bb09e84` |
| `defaultSymbolTable_Parker.bin` | `cff12f33ba3645bcc842ef117badfca14d6d6929f6d24a3500f726ff7ae109b4` | same |
| `amps_hd.models` | `041dcead7f10f89e2957c3c108545fb30bb6f30058946c28513f26ea36e86a17` | absent |

1. Use the [audit's Ghidra 12.2 procedure and executable SHA-256 pins](spider-v30-static-trace.md#reproducible-binary-references). The images are `APIV3EngineDLL.dll` beside the Windows `res/` directory (PE32 x86, base `0x10000000`) and `Contents/MacOS/Spider V Remote` in the macOS bundle (Mach-O x86-64, base `0x100000000`). Start with the product, catalogue and writer/lookup addresses above in `Focused.java` using the existing read-only projects or a fresh import. Inspect the mapping and selector rows as data, and check the listing at `0x100106a74`, `0x100106ad0` and `0x10010624a` for the type and value stores.
2. Read the JSON rows by exact `symbolicID` and verify the bounds and product predicates. For the Parker table, the count is 910 and total length is 22,074 bytes. Read the three little-endian 32-bit words at `8 + 12 * index` as pool-relative offset, CRC-32 and length. The pool begins at `0x2ab0`. Recompute CRC-32 over the name bytes before the NUL. The parser and lookup in the script below can be reused on the local tables.
3. Run `python3 scripts/verify_spider_amp_symbols.py -v`. Seven synthetic checks cover catalogue filtering, CRC collisions and exact names, malformed symbol records, the state-suffix lookup, and literal group/model/scalar record bytes. The fixture uses invented names and a fabricated scalar.

The follow-on verified both executable hashes and all five resource hashes above. Four local data checks replayed the catalogue join, all 910 Parker symbol records and the macOS CRC table, the two macOS selector rows, and the relative calls on the writer path.
