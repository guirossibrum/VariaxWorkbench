# Spider V30 offline audit · input inventory and discovery

[Issue #311](https://github.com/guirossibrum/VariaxWorkbench/issues/311) · Phase 1 · Repository baseline `0c1b2b4969d43e2c4f266a9e83d9bcabb9d73f1e`.

The owner-pinned Line 6 download set is dated 2026-11-27. All eight inputs passed the supplied SHA-256 check. The archive basenames below also identify the directories and IDs in the private file manifest.

| ID | Pinned input | SHA-256 | Container and observed payload |
| --- | --- | --- | --- |
| 1 | Line 6 Central 1.06 (1).dmg | `34c77824799f044f7e94631e2b1b31cad2535faea5687f8932f9194fce477457` | UDIF/zlib → GPT/HFS+ → XAR → gzip/cpio. Central 1.06, universal x86_64/arm64, macOS minimum 11.0 in Info.plist. Maintenance and UI. |
| 2 | Line 6 Central 1.06 Installer (1).exe | `e9a7269e4ff670b5421c79f24196323cfcb0d663159857b3e7de85aad818ebfd` | PE32 x86 NSIS/Deflate → managed PE32 frontend, native engine and libraries, maintenance scripts, prerequisites. Version designation from supplied release. |
| 3 | Line 6 Driver2_5 Spider V v2.00.0.0 Installer.exe | `906705bf47a2e4d0ae33ca4d28efaf000d36d899653139a63e920af61cde35c5` | PE32 x86 NSIS/Deflate → AMD64 INF and two AMD64 SYS files, CAT, mixed x86/x64 Interposer and backends, ASIO/control panel and installer support. Version 2.00.0.0 in INF. |
| 4 | Line 6 Updater 1.28.dmg | `e5b2bed1daf8717f9442688bbd8b1781c145c3466e78a122bf9cd4716ce7defe` | UDIF/bzip2 → APM/HFS+ → XAR → gzip/cpio. Updater 1.28, Mach-O x86_64, Qt/QtWebEngine, libusb, Cypress USB library, two inert package hook files. |
| 5 | Line 6 Updater v1.28 Installer.exe | `13245d8201cf2fdf356ede2de90e79c74d2e1a0e33d6185a11bf8b77c12fd54d` | PE32 x86 NSIS/Deflate → native PE32 x86 Qt Updater, WebEngine helper/runtimes, vendor and USB libraries, translations and installer support. Version designation from supplied release. |
| 6 | Spider V Remote 2.01 Installer.exe | `1f8291548abbd72ba73322e8d8916fdc8083badc2dc15d49933a87c4dc17a613` | PE32 x86 NSIS/Deflate → managed PE32 UI, mixed-mode PE32 EngineCLIWrapper, native PE32 x86 APIV3 engine and Interposer, libusb, model resources and prerequisites. Version designation from supplied release. |
| 7 | Spider V Remote 2.01.dmg | `d6ec785dc16d27805b1e3d2bebcd1bbaa145869b72a19c8e9b59b90535fdcf73` | UDIF/zlib → APM/HFS+ → XAR → gzip/cpio. Remote 2.01, Mach-O x86_64, macOS minimum 10.12 in Info.plist. Swift/AppKit and native engine in the same main image, bundled Swift and libusb. |
| 8 | SpiderV_30_60_2.05.0.svu | `5301e9c582fab5bf1ddcbe321cf642e293dbaf62fc90a70de923b8b4fec6c00b` | Big-endian IFF `FORM/L6FF` → HEAD, dinf, data → zlib → opaque body. CPU and flash regions unresolved. |

## Reproducing the private inventory

Preserve `backups/spider-v30-inputs.ndK8uuQS/` and `backups/spider-v30-audit-311.OIUQg5qW/` in the main checkout. The second directory has the extracted trees, `manifest-v3.jsonl`, `duplicates-v3.json`, `inventory_v3.py`, source listings and private Ghidra material. Manifest rows bind each pathname and classification to its input ID and extraction layer. The manifest has 1,874 entries, including 52 symlinks, and 72 groups of duplicate file contents. Regular files have byte size, SHA-256 and `file` metadata, including binary architecture where applicable. Layout symlinks record their literal targets. The private duplicate index groups identical file contents by SHA-256. Every extracted entry has a role classification. Embedded prerequisites, installer metadata, runtime resources and the opaque firmware body are indexed at their appropriate layers.

- From the input directory, run `sha256sum -c SHA256SUMS`. Confirm archive order by sorting the eight basenames in the C locale.
- Use local 7-Zip 26.06, `l -slt` then `x -tNsis -aos` on PE installers. For DMGs, extract partitions with `x -t* -aos`, then extract the HFS member with `x -tHFS -aos`. Use separate empty output directories for each layer. HFS output includes metadata streams and journals. Restore owner read access on extracted journal copies if required for hashing.
- Use `bsdtar -tf` and `bsdtar -xf` for the XAR, then list and unpack each gzip/cpio payload or Scripts archive with `--no-same-owner --no-same-permissions`. Keep package BOM, Distribution, PackageInfo, scripts and volume layout as separate evidence.
- The Windows .NET and Visual C++ prerequisites are third-party installation support. Their first nested 7z/CAB layers are indexed. Inner OS servicing packages and MSI database streams are treated as opaque and outside the Spider protocol trace. Installer stubs and PE runtime resources need only file-level classification except where an application workflow references an embedded resource.
- Run the private `inventory_v3.py INPUT_DIR EVIDENCE_DIR` with the observed directory layout, into a fresh evidence directory or without existing output manifests. `inventory-v3.summary` gives per-layer counts. Extracted resource and executable bytes remain private.

## Desktop comparison

Nine parsed resource pairs have the same contents: eight model catalogues (amp, cab, delay, fixed, modulation, reverb, stomp, wah) and the Parker seed preset. The Parker symbol table is byte-identical. The AssetDefs resource also matches after line-ending normalization. The Windows package carries additional HD and other-family seed assets. Their presence alone does not extend Spider capabilities. The macOS bundle has a firmware-update definition plist, compiled AppKit storyboards and Swift support. Windows has a managed WPF shell, CLI bridge, WebView and separate engine/Interposer DLLs.

Both Central builds ship identical parsed Spider maintenance steps. The Windows Central and Windows Remote Interposer files are byte-identical. The two Updater builds share Qt/QtWebEngine and USB library roles with OS-specific binary formats. The Updater binary also contains Parker device handling. The separate driver package adds Windows kernel/audio/MIDI and user-mode transport layers. The pinned macOS Remote, Central and Updater payloads contain no kext bundle.

## Discovery checkpoint

Addresses are Ghidra virtual addresses in the named image. Direct references and decompiler observations are static evidence. Whether a given path is selected on a particular host remains a runtime question.

| Image and SHA-256 | Static observations |
| --- | --- |
| Windows Remote `APIV3EngineDLL.dll`, PE32 x86, `a46c6161d7e2a8ef4f73ecb5ad277ad1a87d7c82ca13ea4a36b210f9d34f5edf` | `0x10063690` calls SetupAPI enumeration and inserts USB candidates through `0x100653c0`. A separate branch in the same function enumerates Interposer descriptors. At `0x10066030`, the arrival refresh checks vendor `0x0e41` and includes `0x5068` in the Spider-range branch before device-list refresh. `0x10068890` enumerates WinMM MIDI endpoints. `0x10052e10` is a generic libusb VID/PID match, open and claim wrapper. `0x10052fb0` references the control-transfer import. |
| macOS Remote executable, Mach-O x86_64, `c99aaa95cb09eb20553b698e00729e38b112e8f1c064b99b61d620b5669f0e21` | `0x1000f6e19` enumerates libusb descriptors and compares supplied VID/PID, then inspects configuration descriptors through `0x1000f6cbc`. `0x10007577e` opens the matching device and claims interface 0. `0x10007591e` calls the generic control-transfer stub at `0x1002e458e`. `0x100001966` enumerates CoreMIDI sources and destinations. Direct cross-reference `0x1000fb940` → V30 name at `0x10031a9e7` anchors the `0x1000fb6a8` model-selection function. Its PID branch maps `0x5068` to product `0x220000`. `0x1000fd086` independently handles product identity values when setting device metadata. |

USB binding for V30 is `0e41:5068` in the driver INF. The same file identifies V60 at `0e41:5069`, V120 at `506a`, V240 at `506b`, 240 head at `506c` and V20 at `506f`. Central assigns logical product IDs `0x220000` to `0x220005` in that order. The macOS selection branch agrees. These tables establish V30 selection at the static level and family membership for the other entries. Vendor USB helper calls, driver bindings, and MIDI entry points alone do not establish which path carries normal editor traffic. The first request/reply and any descriptor-open-to-MIDI transition require further tracing.

## Firmware extent

`HEAD` at file offset `0x0c` has 52 bytes, `dinf` at `0x48` has 16, and `data` at `0x60` has 393,318. The declared FORM length consumes all 393,422 source bytes. The zlib stream inflates to exactly 393,192 bytes with SHA-256 `58b058a7310a8f397d5c029f6dc0c0b07e5128dbd027548c060eba797647a0b4`. The length matches a big-endian field in each metadata chunk. The 16-byte value at absolute offset `0x28` equals MD5 of the inflated body. This is a byte-level match and provides no supported security or signature claim.

The version word is `0x02050030`. The product word is `0x00220000`, which independently matches the V30 desktop identity. The filename declares shared V30/V60 coverage. The separately enumerated V60 identity is `0x00220001`. Shared-image applicability and eligible hardware revisions remain open. The inflated body has high byte diversity in all 96 measured spans of up to 4 KiB. The internal encoding and any region boundaries remain unresolved.

## Bounded follow-up phases

1. Trace platform discovery through the actual Parker device factory, message-port selection and first exchange. Locate discovery, matching and reconnect branches in each build and split normal editor mode from update mode.
2. Reconcile the shared Parker seed and catalogue with call sites. Identify preset formats and metadata, range and unit handling, current-buffer versus store branches, synchronization and failures. Use synthetic fixtures for schema, frame and error contracts.
3. Trace driver, Updater, Central and SVU dependencies to the depth needed to distinguish editing, audio, accessory and maintenance paths. Resolve model and revision scope only where static evidence allows.
4. Complete the feature-to-evidence/reuse/gap/test matrix and independently reviewable Workbench integration slices. Keep unresolved evidence open and propose a separately authorized first read-only owner test.

The [protocol and asset report](spider-v30-static-trace.md) records the follow-up results, including the first exchange. The [feature matrix and integration plan](spider-v30-tab-plan.md) tracks the remaining evidence by workflow. No vendor payload or installer hook was run.
