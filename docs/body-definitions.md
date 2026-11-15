# Original Workbench body definitions

`src/body.rs` decodes the original body-resource header, body resonance and low-cut fields in Rust. This is offline resource decoding. It does not communicate with a guitar, generate a device patch, restore factory presets or claim that body defaults equal the factory contents of a Variax 600.

## Native verification

Run `.tools/python/bin/python scripts/verify_native_bodies.py` from this checkout. It executes the original i386 `L6VWBodyModel::ReadWrite` at `0x99c0`, `ReadWritePDef` at `0x9930`, the nested `VCEVariaxPatch::ReadWrite` at `0xa1f0`, numeric stream readers and array loops in Unicorn. The Mach-O source digest is checked by `NativeOracle`; the reference SHA-256 is `3aa232329d467101637764397b77a7b8ffa6af0ccd72ec380d02f145477ba9fe`.

The emulator supplies bytes from bounded local memory through a synthetic stream's `ReadByte`. Its only patched original function is `StringX::Format` at `0xd500`, replaced by `ret` in private emulator memory because binary `SerialPersistance` ignores field/group labels. Original StringX construction/destruction still executes. This is a deliberate adapter, not a claim of unmodified whole-application execution. No OS service, hardware, original UI or running application is involved.

For all 28 bundled body files, the verifier observes 1,421 serialization calls, checks contiguous complete input coverage, reads every corresponding native destination field, reverses each multibyte value to canonical big endian order, and compares the result with the full input. Each file is 17,483 bytes. All 28 yield identical field layouts. A separately generated synthetic fixture with distinct values also passes the original reader. Its bytes are generated from the observed schema; the committed fixture does not contain an original vendor body resource.

Evidence:

- `analysis/reports/native-body-schema.json`: all field names, types, widths, counts, file offsets and native object offsets.
- `analysis/reports/native-body-serialization.json`: source digest, adapter disclosure, resource hashes and fixture digest.
- `tests/fixtures/native-body-definition.bin`: synthetic test input.

## Verified file layout

Integers and IEEE-754 floating-point values are big endian. Booleans occupy one byte and the native reader treats every nonzero value as true. There is no resource header signature or version field before these fields.

| File offset | Bytes | Field |
| --- | --- | --- |
| 0 | 1 | `has_middle` |
| 1 | 32 | bridge pickup defaults |
| 33 | 32 | middle pickup defaults |
| 65 | 32 | neck pickup defaults |
| 97 | 4 | interpolation mode, unsigned raw representation |
| 101 | 6 × 2,897 | variants named `bb`, `mb`, `nb`, `mm`, `nm`, `nn` by the original reader |

Each pickup default consists of position float32, angle float32, then six pole-offset float32 values. Defaults are serialized even for pickups the model does not contain. Interpolation-mode values 0 through 4 occur in bundled resources; their exact meaning remains unverified and Rust exposes the original numeric value.

The native object's pickup defaults begin at `0x4b50`, `0x4b70`, `0x4b90`, its mode at `0x4b4c`, and `has_middle` at `0x4bb0`. Native variants begin at object offsets `4 + i * 0xc8c`. Native structure size/padding must not be used as serialized size.

Within each serialized variant:

| Relative offset | Bytes | Field |
| --- | --- | --- |
| 0 | 1 | body resonance enabled |
| 1 | 12 | twelve resonator enable flags |
| 13 | 48 | twelve float32 frequencies |
| 61 | 48 | twelve float32 Q values |
| 109 | 48 | twelve float32 gains |
| 157 | 4 | treble frequency |
| 161 | 4 | rolloff frequency |
| 165 | 4 | master |
| 169 | 4 | body mix |
| 173 | 4 | mid mix |
| 177 | 4 | treble mix |
| 181 | 1 | low-cut enabled |
| 182 | 4 | low-cut frequency |
| 186 | 1 | low-cut fourth-order flag |
| 187 | 2,710 | remaining variant fields; preserved, currently opaque in Rust |

## Rust API and limits

`BodyDefinition::parse` requires exactly 17,483 bytes before any field access, returns three pickup defaults and six typed resonance/low-cut variants, and retains all input bytes. `original_bytes()` returns the original resource; `variant_bytes(index)` returns a bounded original variant. Changes to public decoded fields do not alter either byte view. There is intentionally no serialization or device-write API.

The parser follows native boolean behavior and retains floating-point bit patterns, including NaNs. Successful parsing verifies structure/length, not whether a resource is sensible for signal processing. Rust tests exercise distinct synthetic values across every exposed field and variant, every shorter input length, excess bytes, out-of-range variant indices, noncanonical true values and NaN payload preservation.

Standalone test command before library integration:

```sh
rustc --edition=2024 --test src/body.rs -o /tmp/variax-body-tests
/tmp/variax-body-tests
```

Remaining work includes typing the rest of the observed variant schema, verifying interpolation-mode semantics and units, and separately proving the relationship between these modeling resources and shipped guitar presets. None requires hardware probing at this stage.
