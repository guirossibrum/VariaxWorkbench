# POD Farm 1.12: PODxt control applicability investigation

This research milestone explains why the examined descriptor and conversion
paths do not yet justify numeric PODxt Live preset editing. It continues the
[1.12 source recovery](pod-farm-112-source-recovery.md) and concerns one candidate:
PINF product `0x3000a`, model `0x2001b`, amp slot `0x30000`, tagged control
`0x3f100003` / raw PARM identifier `0x100003`. Those identifiers describe the
investigation, not a supported control or an admitted value domain.
[#29](https://github.com/guirossibrum/VariaxWorkbench/issues/29) remains open;
the existing [offline librarian](pod-presets.md) is unchanged.

## Source and method

The original is the same supplied 137,347,009-byte POD Farm 1.12 DMG, SHA-256
`086416c6c6b7e4471871bee4d44f4d8949362e57155677fd06d8288a344e399a`.
The examined TWXY framework has SHA-256
`81fdb247a010a23f5a0139abf490f93bf6c15d5a306eca9171110111e12ae703`;
its 4,013,896-byte i386 slice at fat offset `0x1000` has SHA-256
`db9253497fb715ae71a3c3c31adeabdb81878594fab486d8e621bbdf04e6fd30`.
These identify supplied bytes, without establishing publisher authenticity.

All coordinates below belong to that slice. Ghidra 12.2 static cross-references
and decompilation were checked against GNU objdump instruction decoding, concrete
vtable entries and Mach-O pointer/relocation metadata. Relevant `__TEXT` and
`__DATA` addresses equal slice offsets; that shortcut does not apply to
`__IMPORT` or `__LINKEDIT`. Recovered signatures are imperfect, so a decompiler's
guessed argument or return type alone is not a contract.

The investigation used static readers only. It did not execute or emulate vendor
code, obtain an installed entitlement, query licensing storage, discover hardware
or open a USB/MIDI/audio device. Original files, extracted resources, instructions,
resource rows and detailed original-derived reports remain local and ignored.
This document records independently authored findings, not vendor code or tables.

## Descriptor producers were located, but applicability was not established

The first continuation connected instance initialization at `0x4a9c0` to the
descriptor root at instance offset `0x28c`. The amp builder reaches `0x47600`
and the selected model's parameter builder `0x48640`. That builder obtains a
model definition through cache lookup `0x29460`, filters parameter definitions,
checks the current value's type and projects metadata through `0x48150`.
Its later model-specific override is `0x47f20`.

A separate setter path, `0x3e500` → `0x4cb60`, writes a descriptor cache word
through `0x61ed0` before the ordinary typed assignment at `0x31490` consults a
registry row and conditionally clamps the patch value. A successful early cache
write therefore does not prove that the patch accepted the proposed value.
The registry is bound through cache offset `0xc0`; its availability and identity
depend on the selected cache and eligible resources.

One candidate resource associated with the model/control contains scalar-looking
metadata. Neither that metadata, endpoint interpolation, nor the earlier
byte/127 conversion establishes its applicability to an untouched PODxt PARM.
No bounds, unit, default, quantization step or continuity rule is admitted.
The final descriptor override and effective registry domain were not promoted
past the product-applicability gate.

## Four different identities must stay separate

| Identity | Examined producer or consumer | What it establishes |
| --- | --- | --- |
| Device/reference identity | Runtime backend identity `0x4650`, translated to product `0x3000a` | A device-to-product mapping, not a control definition |
| Cache selection | `0x2ea60` queries interface slots `+0x50`, `+0x48`, `+0x4c` | Instance kind, reference product key and runtime context key |
| Processing destination | File load `0x4fbb0` queries interface `+0x44` and version `+0x40` | Inputs to the conversion path `0xf2e24` → `0x104c60` |
| On-disk PINF identity | Reader `0xf7806` through `0x106f7c` | Parsed format, product and device-version words in the patch object |

The PINF words are stored at patch offsets `0x74`, `0x78`, `0x7c`. They do not
populate the three cache keys by themselves. In particular, cache product
`0x3000a` can coexist with a different processing destination. Conversion
compatibility is not unchanged-file preservation or an editing-domain contract.

The runtime context key at context offset `0x8` can come from a caller selector
or a cached firmware ESN obtained through `0x06370` / `0x061d0`. The latter's
uncached path uses a backend request. Static tracing followed manager construction,
registry insertion and callback registration to IOKit discovery code, including
the `0x4650` interface matcher at `0x22cc54`. No callback state or request result
was obtained. A constructor's initial device list alone would miss this separate
registration and cannot establish device exclusion.

## Concrete receiver alternatives give a bounded negative result

Interface constructor `0xfe60` installs vtable `0x3612a8` and initializes its
optional receiver to null. Initializer `0x100b0` sets the selection byte at
interface offset `0x15` before obtaining a context; it marks the interface active
only after the context factory succeeds. Selector `0x10860` returns either the
interface's `+0x34` receiver or the context's `+0x24` receiver. Inactive or missing
context paths do not establish a usable receiver.

The concrete stores resolve three types, with different product behavior:

- The interface receiver is constructed by `0x22d70` and stored by `0x15d30`.
  Its vtable is `0x3616a8`. Product-key method `0x21980` can translate a backend
  identity to `0x3000a`, but destination method `0x21b70` selects `0x130001` or
  `0x80006`. A matching nested core or reference key does not change that outward
  destination. Failed initialization clears/releases the receiver.
- Context initializer `0x15060` can construct a derived core through `0x75090`,
  installing vtable `0x361ea8`. Its destination method `0x75350` returns
  `0x130001`. This is a different-product branch if constructed.
- The same initializer can construct a base core through `0x20e90` → `0x208f0`,
  installing vtable `0x361488`. Destination method `0x21590` forwards the product
  method `0x17540`. Its initializer `0x1f1a0` contains a `0x4650` → `0x3000a`
  mapping. That conditional leaf initially looked relevant, but its callers do
  not establish an applicable PODxt path.

Two earlier guards explain the last distinction. The flag-selected context
factory `0x16c40` ranks candidates through `0x13b50`; its identity filter gives
`0x4650` a score below the selection threshold, before feature/ranking logic.
Separately, context dispatcher `0x14460` routes identity `0x4650` through context
slot `+0x18`, whose concrete target `0x11f30` returns unsupported status
`0x8000700a`. Its class-eight branch, rather than the PODxt branch, invokes the
core-building method `0x15060`. Thus the generic product-forwarding leaf cannot
be used as a PODxt control contract by ignoring the preceding matcher/dispatcher.

Null optional receivers are also significant: an active interface can use a
reference key derived from `0x4650` while its destination fallback is `0x130001`
or `0x80006`. Context construction initially clears `+0x24`, and later processing
initialization can store a core even when initialization reports an error.
Neither a null fallback nor a transient nonnull pointer proves an initialized,
target-preserving receiver. Cache selection after interface initialization does
not by itself prove later cache rebinding.

The examined alternatives therefore classify as different-product conversion,
rejection/unavailability, or runtime state that was not observed. Conversion
classification describes the requested destination, not a successful conversion,
lossless serialization or model/audio equivalence. No applicable target-preserving
branch was established for this candidate. This does not establish that every
entry point has been exhausted, that POD Farm can never support a target, or that
another original is required.

## Validation scope and remaining contract

Three private continuation rounds recorded **12**, **14** and **21** static
consistency checks respectively. These check selected source/data relationships,
raw instructions, concrete vtable entries and guard arithmetic. They are not
independent native-oracle cases, Rust parser tests, successful editing roundtrips
or physical-compatibility tests. The last round retains 22 instruction excerpts
privately. An initial assertion used the wrong instruction byte offset; the
failed result and the correction against saved instructions remain preserved.

The final bounded report hashes to
`581826feaaf46a571c3d3c03fe1d4893dc6b7a2b81f83fd5d608898a20cb3a65`;
its final consistency record hashes to
`05e7bd89793e91f21268bb3fb51ed8ba803bd92d10d3038f3cde99323b9ee85e`.
These pins identify private evidence, without publishing it or making its checks
an implementation acceptance result. Earlier extraction-test counts in the
[source-recovery report](pod-farm-112-source-recovery.md) retain their own scope.

Before numeric editing, an independently evidenced contract must connect:

1. A target-preserving entry for product `0x3000a` to a specified container and
   format/device-version combination, model, slot and control.
2. The eligible resource and final effective descriptor/registry domain to that
   same receiver, including encoding, scaling, units, defaults and a justified
   quantization step or continuity rule.
3. Rejection/error behavior, including mismatched targets/types, invalid values,
   nonfinite inputs, missing definitions and partial mutation.
4. Serialization and preservation behavior, including unchanged unknown bytes,
   save/reopen, independent compatibility evidence and atomic non-overwriting
   new-copy saves.

Those gates remain unresolved; the issue's implementation acceptance is still
incomplete. This milestone adds documentation only, with no editing API, runtime
dependency, default-math change or device operation. Hardware access remains
locked independently of any future offline result.
