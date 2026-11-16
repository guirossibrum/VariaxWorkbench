# POD Farm 2.58 XFP6 section ledger

This finite static continuation of [the provider trace](pod-farm-plugin-provider.md)
uses the same full AU x86-64 slice, SHA-256
`715d30b8fed77d4ea18150050131db759b8e8a27eeaf4e3d9b3872b6bca6fbe0`.
Addresses below are AU virtual addresses. These are derived static contracts,
not observed plugin execution, a native oracle or a complete import/export codec.
Original-derived reports remain ignored and private. #40 remains partial/open;
#29, publication, full command-v1 checks and exact-head independent review remain
held/required under the existing coordinator instructions.

## Prefix producers and section boundaries

The 244-byte header's words at +`0x10`/+`0x14` have a concrete AU producer:
`0x29014` calls `0x3b94a`, which calls `0xc7780` to store registration globals.
The first argument is `0x7d010217`, a caller product/application identifier;
it is not a guitar/device identity. The second comes from `0xf9d50`, which reads
`CFBundleShortVersionString` and passes its bytes to `0x19633e`. The exact version
packing remains unresolved; an on-disk zero in the mutable globals is not an
observed runtime version. Other unnamed header words retain neutral labels.
For example +`0x5c` follows `0x84784`/`0x84726` through a cached word or command
getter/setter; neither units nor user-facing meaning follow from its bit pattern.

The prefix reader still does not validate total length, overlap or all spans.
`0xc78b8` requests a subview of each tone offset/length, resets its parser and
reads/validates a child patch (`0x190b98`/`0x18f480`). The underlying view arithmetic
is the previously documented unsigned/signed implementation, not a general
bounds-validation barrier. A failed first tone read propagates; a failed second
can leave the first eligible for application. `0xcaa44` performs category-dependent
conversion for `0x400002`/`0x400003`, including clone/translate paths; `0xc9ae8`
serializes a temporary patch before core application. This establishes conversion
boundaries, not value/audio equivalence or applicability to PODxt.

Mapping/settings loaders (`0xc783c`/`0xc77cc`) require both corresponding offset
and length to be nonzero, then seek the original stream to the offset. They do
**not** create length-limited subviews: the header length is a presence condition,
not an enforced read ceiling in these adapters. A section reader may therefore
read beyond its advertised span if more stream bytes exist. Restore changes
context first, then tones, settings and mappings. Tone application errors can be
written to an optional detail output while `0xc7d64` returns success after updating
the name. Later failures do not roll back earlier mutations.

## Mapping section: L6ME

Writer `0x1a674e` and reader `0x1a6fe8` are connected through `0xc9e02`/
`0xca744`. The writer emits this big-endian framing:

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | 4 | ASCII L6ME |
| 4 | 4 | Writer version 2 |
| 8 | 4 | Record count |
| 12 | 4 | Reserved zero |
| 16 | count × 104 | Mapping records |

Each record starts with four uninterpreted event bytes, followed by 25 big-endian
32-bit words. Record +`0x10` is the conversion tag; +`0x24` is emitted as zero
and ignored on read. Other words include source object fields +`0x14`/+`0x18`/
+`0x1c`, +`0x28` through +`0x44`, four scalar words at +`0x48`, and eight raw
numeric words at +`0x58`. These offsets describe the 120-byte in-memory record,
not identical offsets within its 104-byte serialization. The tag 1 conversion
uses `0x7af88`; tags 0, 2 and 10 use `0x7af4c`. Other tags still append a record
with the initialized typed branch; this is not strict unknown-tag rejection.

The reader clears its destination array **before** validating the stream/header.
It checks the 16-byte read and signature, but version 1 returns success immediately
with the cleared array. Every other version follows the record loop; acceptance
is not limited to writer version 2. There is no up-front count × 104 remaining-byte
check. A short record seeks back to the initial stream position and returns
`0x80001008`, retaining earlier appended records. Wrong magic returns
`0x80001019`; writer short writes return `0x80001009` and seek back without proving
truncation/rollback of already written bytes.

Array append (`0x1a6fd6` → `0x1a7994`) copies each record without deduplication;
order and duplicates survive that local stage. Export `0xc9d4c` filters records
through `0x874b0`: eligibility is object +`0x44` == 1, or +`0x3c` == 2/3,
or +`0x3c` == 1 with +`0x40` in 1..6. Import `0xca6d8` appends into the context
array, then `0x89e2a` clears the engine mapping target before converting/applying
records. A later conversion failure leaves partial applied state; it does not
establish atomic restoration. The reader neither requires EOF nor preserves
uninterpreted trailing bytes in its rebuilt mapping output.

## Settings section: 1MA6

This receiver is now concrete: core initialization `0x86f02` calls `0x86976`.
The provider's callback object (+`0x4b8`, constructor `0x9cdd0`, vtable `0x24cdf0`)
uses slot +`0x60` (`0x8ee2a`) to test whether its two counts sum to nonzero. If so,
`0x86976` creates a 48-byte settings object via `0x9c346`/`0x9c2f0`, stores it
at context +`0x38`, and populates it through callback slot +`0x68` (`0x9d32c`).
Its vtable `0x24cb10` resolves save +`0x10` to `0x9c3c8` and restore +`0x18` to
`0x9c082`. A null receiver makes the outer settings adapter succeed without work.
Actual runtime counts/receiver availability have not been observed.

| Offset | Size | Writer meaning |
| --- | --- | --- |
| 0 | 4 | Little-endian word `0x36414d31`, bytes 1MA6 |
| 4 | 4 | Header size 20 |
| 8 | 4 | Version 2 |
| 12 | 4 | Total size = 20 + count × 24 |
| 16 | 4 | Record count |
| 20 | count × 24 | Six 32-bit words per record |

The writer selects enabled in-memory entries, refreshes their final three words
through `0x9bd5e`, then writes header and records. Short writes return
`0x80001009`; there is no demonstrated rollback. The reader accepts native or
swapped magic, swapping numeric fields/records in the latter case. Version <=1
returns success without applying entries; every version >1 enters the record
branch. For nonzero count it compares unsigned 32-bit `(total - header)` with
64-bit `count × 24`, allocates that declared byte count and requires a full read
before changing entries. Zero count skips this arithmetic check. A non-20 header
invokes a relative seek with the recovered expression `20 - header`; this is
not evidence of safe extension skipping. No outer section-length cap is added.

After the read, only min(serialized count, existing count) entries are applied.
For each, words 3..5 are copied and passed to `0x9bfc8` to reconstruct the first
in-memory word. Remaining existing entries reset their first word to their stored
fallback. This is positional restoration, not preservation of arbitrary record
identity or extra records. Truncated header/data returns the stream's slot +`0x38`
result; do not substitute the mapping reader's fixed error code or assume that
result is always negative. Neither reader requires EOF. Unknown versions, header
extensions, reserved words and trailing bytes lack a generic retention contract.

## Owned verification and limits

`scripts/pod_farm_section_audit.py` is a strict framing audit, deliberately separate
from native reader behavior: writer version 2 only, settings header exactly 20,
complete records required before returning anything, unbounded Python arithmetic.
It retains uninterpreted records/tail for inspection and does not apply them.
Eight synthetic tests pass, including every truncation of a one-record frame,
count/length disagreement, huge counts, both settings byte orders, empty settings,
and explicit rejection of the native mapping version-1 shortcut. The previous
eight prefix/status tests also pass. These checks validate owned audit behavior;
there are no original state fixtures or native roundtrip/oracle claims.

```sh
python3 -m unittest discover -s scripts -p test_pod_farm_section_audit.py -v
python3 -m unittest discover -s scripts -p test_pod_farm_state_prefix.py -v
```

Reproduce static selections with the earlier hash-pinned AU project and owned
`PodFarmEdge.java --exact` selector: `000c7780 0003b94a 000f9d50 000c78b8
000c7d64 000c77cc 000c783c 001a674e 001a6fe8 000c9d4c 000ca6d8 00089e2a
00086f02 00086976 0009c2f0 0009c3c8 0009c082 0008ee2a 0009d32c`.
Raw reports, including rejected selections, remain private. No vendor code or
serialized vendor fixture is committed. The finite pass stops here: exact version
packing, all scalar meanings/conversions, full nested patch preservation, active
runtime state, AU deferred-cache consumption and RTAS outer id producers remain
unverified. No new reachable AU deferred consumer was established and no further
wrapper archaeology or missing-input request is substituted for those gaps.
No vendor initialization/import/DSP, emulator, USB/MIDI/audio, authorization or
service path was executed; guards and production Rust/dependencies are unchanged.

Private evidence manifest: `sections-evidence-58294b49cebd9bc91d80f546ca8a5896315246fd45f98cb24fca33cc155a7392.json`
(18 new reports, including rejected selections and the audit test log). Audit
test log SHA-256: `2a9382a66726fc7d622e74b589e52c196af7ca9fbb5729d3a71967f5c4142daa`.

The [tone/coverage consolidation](pod-farm-tone-coverage.md) follows the concrete
child parser and conversion matrix, records nested preservation losses and
runtime input boundaries, and distinguishes evidence-file counts from test counts.
