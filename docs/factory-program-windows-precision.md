# Windows Workbench and Variax 3.10 factory precision

**Wider x87 arithmetic explains part of the factory gap.** In a controlled
comparison of the original 32-bit Windows coefficient engine, widening only
its Q23 converter from 24 to 53 significand bits made 927 previously differing
slot/word positions match the firmware and made 134 other positions differ.
The original Windows engine with 53-bit x87 precision elsewhere as well
still differed from all 60 factory presets. [Counts, indices and hashes](factory-program-windows-precision.json)

## Historical scope

The recovered Workbench 1.75 Windows coefficient library identifies as PE32
Intel i386. The COFF timestamp is 2009-11-05, after the October 2009 Windows 7
launch. The Variax 300/600 firmware 3.10 release is dated 2006-01-06. The
recovered DLL is relevant to the 32-bit Windows 7-era editor. The factory
authoring environment for the earlier firmware remains unidentified.
[Local artifact provenance](artifact-provenance.md) · [Microsoft's Windows 7 launch notice](https://blogs.windows.com/windowsexperience/2009/10/22/windows-7-arrives-today-with-new-offers-new-pcs-and-more/)

The comparison runs original PE32 instructions in Unicorn. That covers 55 C++
initializers, the native readers for 71 body and pickup resources, the JNI field
reader, coefficient generation and packing. It also uses the original Java
preset reader and final metadata/checksum overlay from the previous comparison.
Each case starts with the initialized coefficient object. Private adapters
provide synthetic JNI calls, allocation, default C locale thread state and
capture of 53 destructor registrations. All original OS imports are trapped.
This test covers the coefficient pipeline under specified emulator conditions.
The settings and controls that a live Windows 7 JVM would supply remain
unverified. Unicorn's x87 transcendental implementation also limits historical
runtime claims. [Original comparison](factory-program-parity.md) · [x87 emulator scope](apple-tiger-math.md)

## Controlled test

The fingerprinted Windows routine at `0x10008d00` writes the nine volume
curve points. Its converter at `0x10008370` loads binary32 inputs and keeps the
multiplication and addition in x87 registers until truncating to an integer.
Running it on the default nine-point linear curve gives these exact results
across the 60 archive slots.

| x87 significand bits in the converter | Complete nine-word curves matching firmware | Differing curve indices |
| ---: | ---: | --- |
| 24 | 0/60 | 551 in every slot |
| 53 | 60/60 | None |
| 64 | 60/60 | None |

In the full pipeline, a second run kept the same DLL, preset bytes, model
resources, Java and 53-bit x87 precision, changing the x87 control word only
during the Q23 converter and restoring it at return. Widening the converter
back from 24 to 53 bits reduced non-checksum mismatches from 2,137 to 1,344.
That isolates the final conversion as a cause of real firmware differences.
The 927 exact resolutions include word 551 in all 60 slots.

## Full-program comparison

| Generator and precision | Exact slots | Different bytes | Different program words excluding checksum | Different checksum words |
| --- | ---: | ---: | ---: | ---: |
| Linux x86-64 Rust standard and the original Mac host reference | 0/60 | 3,188 | 2,954 | 59 |
| Windows PE32, 53-bit x87, 24-bit Q23 converter | 0/60 | 2,357 | 2,137 | 58 |
| Windows PE32, 53-bit x87 throughout | 0/60 | 1,437 | 1,344 | 59 |
| Windows PE32, 64-bit x87 throughout | 0/60 | 1,439 | 1,347 | 58 |

The switch from the standard Rust reference to the 53-bit Windows pipeline
removed 2,019 differing positions, including 1,958 of the previous 2,893
unknown discrepancies, and introduced 409 differing positions. That change
includes Windows generator/compiler differences outside the controlled
converter experiment. A switch from 53 to 64 x87 bits in the Windows engine
changed seven complete outputs. For each of those precision settings, the
initialized SSE2 dispatcher and image-default x87 dispatcher produced identical
outputs across the 60 slots.

Setting x87 to 24-bit precision throughout the Windows pipeline stopped at slot
8 under both dispatch settings after exhausting the 200-million-instruction
budget. The 24-bit converter-only and volume-curve results completed within
the same bounds.

The 53-bit Windows comparison still has **1,344 unexplained program-word
differences**. Both host families ran from the settings decoded out of the
archive, which preserve only the upper 24 bits of the stored binary32 fields.
Factory authoring inputs remain an independent source of uncertainty.
Issue #174 remains open. The later owner decision permits offline encoder
work in #175. The [#179 measurements](factory-program-parity.md#device-output-reference-and-firmware-bounds-179)
govern the offline prerequisite for #176.

## Reproduction

Use the same ignored artifact root and firmware extractor described in the
[first comparison](factory-program-parity.md), with the original Windows DLL
present in the extracted Workbench classes. Each run needs a new report path.

```sh
"$artifact_root/.tools/python/bin/python" scripts/verify_windows_factory_programs.py \
  --artifact-root "$artifact_root" \
  --archive "$artifact_root/vendor/installers/Variax300_3.10.vxf" \
  --extractor target/issue-174/std/debug/variax-firmware-inspect \
  --baseline docs/factory-program-parity.json \
  --precision 53 --dispatch initialized-sse2 \
  --report target/issue-174/windows-replay.json
```

Repeat with `--quantizer-f32` for the controlled conversion test, with
`--precision 64` for the extended-precision comparison and with
`--precision 24 --volume-only` for the single-precision curve check.
`--dispatch image-default-x87` tests the alternate CRT dispatch. The report
contains hashes, counts and zero-based indices. Raw programs and model data
stay in temporary private storage.
