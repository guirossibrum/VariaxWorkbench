# Spider V30 preset format checkpoint

[Issue #311](https://github.com/guirossibrum/VariaxWorkbench/issues/311) · [Pinned inputs and extraction](spider-v30-inventory.md) · [Librarian request and completion trace](spider-v30-static-trace.md#librarian-transactions-after-the-sysex-envelope)

Ghidra 12.2 headless analysis used the pinned Spider V Remote 2.01 Windows `APIV3EngineDLL.dll` (PE32 x86, base `0x10000000`) and macOS main executable (Mach-O x86_64, base `0x100000000`). The hashes are in the [audit trace](spider-v30-static-trace.md#reproducible-binary-references). The Windows predicate at `0x100697b0` includes product `0x220000` (V30) in the Parker family. The format evidence below covers the desktop family code, with the V30 branch identified where applicable.

## Windows version-2 PSRP writer and reader

The writer at `0x1008e850` uses `0x10095bd0` to emit two typed records. The first is key `0x80000000`, type 2, value 2. The second is key `0x80000001`, type 2, value `0x50525350`. The first 20 bytes are therefore `00 00 00 80 02 00 02 00 00 00 01 00 00 80 02 00 50 53 52 50`. The ASCII marker `PSRP` starts at offset 16. These are little-endian records with a 32-bit key and a 16-bit type. The cursor advances by the exact encoded size without alignment padding.

| Type | Bytes following the 6-byte key/type prefix | Evidence |
| --- | --- | --- |
| 0 | None | Structural markers emitted by the writer. |
| 1, 2, 3, 7 | Four bytes | Fixed-width representation. Type 2 carries the version and container words. Type 7 carries symbol-table indices in the group and model records. |
| 4 | 32-bit byte count, then that many bytes | String writer includes its trailing NUL in the byte count. Applies to the preset name and label. |
| 6 | 32-bit word count, then four bytes per word | Length is an element count. |
| Other | Unestablished | The checked writer and reader do not establish a usable payload layout for these types. |

The value writer is at `0x100954a0`, with string/array lengths at `0x100953a0`. The token reader at `0x10095660` uses a scratch buffer for strings and arrays, limits the copied portion to its capacity and skips excess declared units when available. Its malformed-input behavior is uneven. For example, some short string or array paths return from the token fetch without a clean error, and the outer dispatcher exits on an input tail under four bytes. Those returns do not prove that a complete preset was recognized.

- The PSRP keys end with `0x80000002` of type 0. The `NamePSKey` writer ends there. The full `Preset` branch then writes a second section headed by the same two control keys, with version 1 and container word `0x47545254` (serialized as `TRTG`). The first group is resolved as `GuitarProc` through the supplied symbol table.
- In the version-1 section, `0x80000003` type 7 opens an indexed group and `0x80000004` type 0 closes it. `0x80000005` type 7 selects the model symbol and `0x80000006` type 2 gives the number of following parameter records. The group writer is at `0x1008e710`, close at `0x10095c60`, and the model/count and parameter sequence at `0x10095ca0`. These counts are separate from the string and array lengths.
- The entry at `0x10089b20` receives a pointer, byte extent and product/symbol context, then passes the bytes to `0x100959d0`. That dispatcher reads the version and container records, tracks the active container through virtual slots `+0x18` and `+0x1c`, and sends ordinary keys through `+0x0c`. The secondary table at `0x10176188` resolves these to `0x1008d040`, `0x1008d060` and `0x1008b570`. The last checks the active PSRP word. The version-1 path descends through `0x10095850` and resolves the indexed group and model through the supplied table. No total-length field was found in the two-record PSRP prefix. The parser is given the extent by its caller.
- The serializer's scratch buffer is 4,096 bytes. The librarian wrappers separately use a 32,768-byte receive capacity and reject sends over 32,768 bytes. Neither bound establishes an authentic tone size. The librarian reply's status and the write request's index and payload size are transport fields outside the preset bodies.

### Recognized keys and field scope

| PSRP keys | Evidence in the Windows family codec |
| --- | --- |
| `0` and `0x4b` | Name and label. Writer `0x1008d090` emits type 4 for each string when present and limits each source to 63 bytes before adding the NUL. The label writer is reached through the Parker predicate. The reader maps each key into metadata. |
| `1` and `2` | Full preset writer selector 1000 reads `tnid` and splits it into two 32-bit type-2 records. The reader accumulates the two parts separately. |
| `3`, `7`, `8`, `9`, `0x4a` | The table at `0x1018ebc0` maps these to `global.@tempo`, `@tweakmin`, `@tweakmax`, `@pedal2assign` and `@speakermode`, with types 3, 3, 3, 1 and 1 respectively. Tempo is requested by the full preset branch. The Parker branch requests speaker mode and goes through the tweak-range and pedal branches. |
| `4` and `5` | Writer selector 1001 can emit two type-7 symbol indices from `global.@tweakgroup` and `@tweakparam`. The reader uses symbol and product-specific mapping. The selector value is internal to the writer. |
| `0x19`–`0x21` | Variax model/magnetic mode, tone knob and six tuning fields elsewhere in the shared codec. The writer and reader guard these paths with `0x10069790`, which accepts `0x200004` and `0x200006` and excludes V30. |

The full preset tree requests Gate, VolumePedal, Wah, FX1–FX4 or their selected stereo variants, Amp, Air, StudioCab, PostComp and PostEQ. The Windows writer excludes the separate LiveCab group for Parker and excludes its Looper branch for this family. These are requests in the desktop serializer. The map from each symbol index, model and parameter ID to an actual V30 field and scale remains open.

### Current and numbered tone callers

| Route | Static join in Windows Remote 2.01 |
| --- | --- |
| Read the current edit buffer | `0x100729d0` (read branch) → librarian read wrapper `0x1009c6a0` with `0xffffffff` → `0x10085d50`. The wrapper passes the returned pointer/length through the buffer object into `0x10089b20`. |
| Select a numbered tone, then read current | `0x1007d610` → Parker branch of `0x1006c040` → slot selection `0x1009c8d0` with a numbered index → read wrapper with `0xffffffff` → `0x10089b20`. This host route activates the slot before reading the buffer. |
| Read a numbered tone directly | The Parker numbered-preset rename branch at `0x1007e760` uses the numbered index at the same read wrapper and parser, before changing the name and writing back. `0x10085d50` can serve a numbered cache hit or send the numbered request. The read itself has no preceding select within this branch. Device-side effects remain unobserved. |
| Write the full current buffer | The write branch of `0x100729d0` calls the same serializer slot as a numbered save and sends its result with `0xffffffff`, mode 0. In `0x1006f540` the guarded name/label routes call `0x1008e850` with `NamePSKey` or `LabelPSKey` and also use `0xffffffff`. Label mode follows the serializer's non-name path. |
| Store a numbered tone | `0x1007d080` → `0x1006c3c0` → serializer slot `+0x34` of the preset table at `0x10176138` → `0x10089ac0` → `0x1008e850` with `Preset`. The buffer goes to `0x1009c740` with the numbered index and mode 0. The rename branch also reaches this serializer and numbered writer. |

## macOS .svl6p reader, writer and conversion

- `0x10010865a` supplies `.svl6p` to the app's file-type setup. The path-based read at `0x10004d400` loads bytes via `0x100112b26`, then invokes the family preset reader `0x1000a293a` → `0x100027f34`. The latter takes a buffer and its length, parses it through `0x100031e18` and `NSJSONSerialization` at `0x1000d927e`, checks schema `L6Preset`, and compares the parsed version to the preset object's expected version. The family preset constructor at `0x1000a0046` initializes version 5.
- The standard preset writer `0x1000277f4` uses `0x10002fef9` with schema `L6Preset`, the object's version, and compression disabled. `0x10002dee6` inserts plain `data` for the ordinary case, and `0x1000d8b66` returns the bytes and length generated by `NSJSONSerialization`. `0x10004bf4c` is the path write call from the conversion/export handler `0x1000485e8`. This path uses the JSON document as its file payload.
- Payload reader `0x10002a26c` reads `device` and `device_version` and passes them to compatibility methods. It processes `data.meta` when it is an object and expects `data.tone` to be an object on the full decode. `0x100028dde` builds the corresponding fields on write. The optional envelope-level `meta` is distinct from `data.meta`. The shared envelope has encoded-data and compression paths, and the bundle writer `0x1000c2bf2` uses `L6PresetBundle` version 1 with optional zlib. Archive round trips and slot preservation remain open.
- The macOS engine also has the binary device codec. The export handler at `0x1000485e8` can pass binary bytes into `0x1000a7f3a` → `0x100106820` and write the resulting preset object through the JSON writer. Its reverse conversion branch parses that JSON object and calls `0x1000a6dd4` → `0x1000a6e3c`. The latter passes the PSRP word to the version-2 writer `0x100106b6c` and the TRTG word to the version-1 writer `0x100106bb4`. This joins the two representations at the preset object, including name, label and the traced global fields. It does not establish preservation of opaque or unknown fields across conversion.

## Sample coverage and offline checks

The local inventory looked at 977,485 regular file entries in the checkout's `backups/`, `capture/`, `vendor/`, `reference/`, `attachments/`, `analysis/`, and `research/`, plus local `Downloads/`. Eighteen unreadable entries were filesystem journal artifacts in unrelated installer extractions. The scan checked the three preset-related filename extensions, the exact PSRP v2 prefix at byte zero, and object-shaped `L6Preset` or `L6PresetBundle` JSON candidates with a schema string in the first 4 KiB. The scan excluded the new evidence directory, followed no directory symlinks, and did not unpack further archives or search for embedded bytes in arbitrary files.

The scan found two authentic Parker installer seeds (3,768 Windows bytes and 3,646 macOS bytes), identical as parsed JSON, plus one other-family preset seed. The Parker pair covers version 5, product `0x220002` (V120), integer `device_version`, `data.meta` including name, label and `tnid`, and 12 `data.tone` blocks including seven global keys listed in the table above. It supplies one unique family document and zero V30 examples. There were zero standalone PSRP v2 bodies, zero named `.svl6p` or `.svl6a` files and zero V30 readback captures identified in this inventory. Authentic current-buffer, numbered-tone and saved-file coverage is therefore zero each.

Fourteen offline synthetic checks cover a literal 20-byte prefix, a 46-byte name-only body, a 184-byte nested example, an unencoded JSON document, type and extent checks, truncation boundaries and cross-format rejection in the fixture model. The fixtures use invented parameters. The model checks a deliberately bounded writer subset and does not emulate the vendor reader's permissive malformed-input handling. These results establish no vendor execution, device behavior, lossless round trip or complete V30 parameter map. The project databases, Ghidra scripts and address-tagged outputs, inventory and fixtures are retained privately under the main checkout's ignored `backups/311-spider-v30-presets/`.
