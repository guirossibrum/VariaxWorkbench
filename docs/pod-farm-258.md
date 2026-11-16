# POD Farm 2.58: static inventory milestone (#40)

This is a partial offline research result for
[#40](https://github.com/guirossibrum/VariaxWorkbench/issues/40), not completion of
its acceptance criteria or the parameter-definition prerequisite for
[#29](https://github.com/guirossibrum/VariaxWorkbench/issues/29). No Rust runtime
behavior changes. No vendor executable, installer script or plugin was launched,
and no vendor native library was loaded into the host. No hardware, audio stream
or authorization service was used. The subsequent
[native property edge](pod-farm-parameter-edge.md) adds a separate, bounded
two-leaf CPU oracle and distinguishes its scope from this static inventory pass.

## Sources and reproducibility

The supplied original `POD Farm 2.58.dmg` is 201,113,929 bytes, SHA-256
`483e4129f2d319665ab7273194818acf570181cac8eb8f75dc5f96581715217e`.
The privately provisioned file was mode `0444`; its size/hash matched the
coordinator's owner-host reference before extraction. This is byte identity to
the supplied reference, not independent download provenance or signature trust.
It is a different input from POD Farm 1.12. Preserve both originals.

Run from an assigned research checkout after supported private-input preparation:

```sh
python3 scripts/research_setup.py attach-tools .tools/coordinator-inputs/tools
python3 scripts/pod_farm_inventory.py --output analysis/pod-farm-258-fresh
python3 -m unittest discover -s scripts -p test_pod_farm_inventory.py -v
```

The helper pins the source size/hash and requires a fresh extraction directory
inside ignored `analysis/`. It uses trusted 7-Zip as a file reader with `l` and
`x -so`; it does not mount the image or invoke an installer. XAR members are
written through the existing atomic, non-replacing `write_new` helper. Bounded
gzip/old-ASCII CPIO decoding writes regular files without restoring executable
permissions; symlinks are recorded as data, not materialized. `Scripts` members
remain archive data. Mach-O parsing reads headers, load commands and symbol tables;
IFF parsing surveys bounded chunk structure without interpreting/editing values.
The helper is an offline Python research dependency only.

Local evidence retained under `analysis/pod-farm-258/`:

| Evidence | SHA-256 |
| --- | --- |
| Extracted `installer.pkg` | `291f77ef4602cc47220bcb7197f4e2f93deb324417915ed9b20eae48d81471e6` |
| Bundled `PODFarm_Help.pdf` | `4ad51a04da44a1fa5d25c8c67563ad0f8030c8522a2675a1aa6859650d1af76b` |
| Private `inventory.json` | `3472da5fde6274be22b536923f3dedb5d19f2b28b871bb0df260798851ac9e44` |
| Standalone `Contents/MacOS/POD Farm 2` | `b6c3aad9ac949706aa0479a0e10e0977d165189269dcbbfeeb067478d22a72cc` |
| Framework `L6TWXY.framework/Versions/A/L6TWXY` | `1b6acb3aa012d9b884f00daeacc948aa80884c81f513871be740055cd629a924` |

The inventory includes every extracted regular payload file's path, size and hash,
each component's `PackageInfo`, native slice hashes/imports/symbols and all tone
chunk trees. Listings, binaries, resources, examples and original-derived reports
remain ignored/private. Only this derived description and owned tools/tests are
published. `help.txt` was produced locally with `pdftotext -layout` for inspection;
citations below use the PDF's printed page numbers.

## Container and product identity

7-Zip 26.06 identifies a UDIF DMG with an HFS+ volume, a flat XAR package and a
bundled help PDF. The XAR declares creation `2014-10-13T19:21:01`. These are stored
metadata, not authenticated dates. The package's `PackageInfo` and app/plugin
`Info.plist` independently contain version 2.58; plugin bundle build strings use
`2.5.8`, while the standalone bundle uses `2.58`. The bundled PDF is titled
*POD Farm 2.5 Basic User Guide*, rather than a 2.58-specific implementation spec.

All **16 component packages** were extracted: **1,551 regular payload files**,
**3 unmaterialized symlinks**, plus **16 owned link reports**. All extracted
regular files have no executable permission bits. Package file counts are installer
metadata and need not equal extracted regular-file counts.

| Component group (`xar/<name>/PackageInfo`) | Declared version |
| --- | --- |
| `POD_Farm_2`, `POD_Farm_2_Common` | 2.58 |
| `POD_Farm_2_{AU,VST,RTAS}_Plugin` | 2.58 |
| `POD_Farm_Elements_{AU,VST,RTAS}` | 2.58 |
| `Line_6_TWXY_Framework`, `Line_6_TWX_Framework` | 5.34 |
| `Line_6_Monkey` | 1.68 |
| `Line_6_License_Manager` | 1.08 |
| `Line_6_Audio_Driver`, `Line_6_Midi_Driver`, `Line_6_Driver_Uninstaller` | 7.3.6 |
| `Line_6_Audio-Midi_Prefs` | 3.61 |

The common payload includes 1,120 `.l6t` examples, help, localization XML,
`appMIDI.l6midi`, and `Amp.imb`, `FX.imb`, `App.imb`, `AppMojo.imb` resources.
Those resources have been inventoried, not decoded into parameter definitions.
Installed package contents do not establish an active entitlement or device state.

## Plugin/DAW feasibility from the actual bytes

All **62 Mach-O files** were header/symbol/import surveyed. Main plugins are
under their component payload's `<bundle>/Contents/MacOS/`; there are twelve
main bundles per format: full POD Farm plus eleven Elements categories.

| Bundled component | Actual slices / evidence | Implication and limits |
| --- | --- | --- |
| Full and eleven Elements AU `.component` bundles | i386 + x86_64; also dual-architecture Cocoa view helpers | macOS AU binaries. This does not establish compatibility with current macOS/hosts. No ARM slice was found. |
| Full and eleven Elements VST `.vst` bundles | i386 only; full plugin exports `_VSTPluginMain` | Legacy VST entry point, not a VST3 binary. No native 64-bit Mac VST demonstrated by this package. |
| Full and eleven Elements RTAS `.dpm` bundles | i386 only | Digidesign plugin path/RTAS package identity; not an AAX implementation. |
| Standalone app | x86_64 + i386 | Not a portable DSP core. |
| `L6TWXY.framework` | x86_64 + i386 | Shared native implementation dependency. |
| Separate `L6TWX_u.dylib` component | i386 | Distinct from framework's nested i386/PPC `data/twx/L6TWX_u.dylib`; do not conflate them. |

The guide's page 3•2 specifically associates Mac 64-bit operation with AU; its
Windows x64 VST description is not evidence of a Windows binary in this DMG.
No VST3, CLAP, LV2 or AAX bundle was found in the extracted component inventory.

The full VST i386 slice hash is
`e68b3d63d0cda33d6d3e3443bfd472fa412ac1fa5ae7636d7d532ed16369a0c4`.
Its load commands import Carbon, CoreMIDI, L6TWXY, legacy libstdc++/libgcc,
CoreServices, CoreFoundation and ApplicationServices. The framework x86_64 slice
(`dced9e6bee1ac57ba9135c1b8751d26141be82155122f7aa151b2cb457bd55a1`)
additionally imports Accelerate, CoreAudio, IOKit and `libcrypto.0.9.7.dylib`.
Renaming a bundle or changing an entry point would leave these macOS dependencies.

The standalone slices contain `LC_CODE_SIGNATURE`; the surveyed full AU/VST/RTAS
and framework slices do not. Command presence is recorded only: no certificate
chain, signature contents, revocation or package signature was authenticated.

| Adaptation path | Evidence-based assessment |
| --- | --- |
| Wrap/bridge the supplied Mac binary on Linux | No working path established. Requires a macOS ABI/framework/runtime solution and preserved legitimate licensing; CPU width bridging alone does not supply these dependencies. Vendor execution remains outside this phase. |
| Convert tones/plugin state | A possible independent file workflow, but semantic conversion and plugin state encoding remain unrecovered. Preserve the original and unknown bytes; fail on unsupported targets/models rather than silently substitute. |
| Independent Rust DSP with a new plugin adapter | Would require independently implemented/validated processing, parameter/state mapping and the selected plugin ABI. This inventory supplies neither a portable DSP implementation nor audio equivalence. Keep the product core independent of the research framework. |

The guide pages 3•36–3•37 describe sixteen assignable knob and sixteen switch
automation slots, plus fixed master/Tone A/Tone B controls. These are **host
automation slots**, not PODxt parameter IDs. Their serialized bindings, numeric
domains and modern-plugin mappings are not established here.

## PODxt Live capability/evidence matrix

| Area | Evidence | Result for the unified Rust app |
| --- | --- | --- |
| Hardware audio vs host DSP | Guide pp. 2•1, 4•1 describes PODxt as Core Audio/ASIO interface; POD Farm processes on the host, with hardware models bypassed by the user | Historical documentation only. No Linux stream or hardware behavior tested. Not evidence that POD Farm edits the POD DSP. |
| Device recognition/license carrier | Guide pp. 1•4–1•6, 2•1 supports a license on a PODxt device | Recognition/authorization role is distinct from editor support; no device was inspected. |
| POD knobs as controller input | Guide pp. 3•29–3•30 describes PODxt's fixed MIDI output controlling POD Farm | Input to host software, not recovered host-to-POD control traffic or domains. |
| Amp/cab/effect choice and controls | Framework exports `TWXYGetModelID`, `TWXYGetNumModels`, `TWXYGetParameterProperties`, `TWXYGetParameter`, `TWXYGetParamString` and setters | Static symbol leads only. No ABI, definitions, scaling or PODxt applicability recovered; these functions were not called. #29 stays held. |
| Global/preset settings and routing | Guide pp. 2•9, 2•18 leaves POD input-source setup on hardware; pp. 3•22 onward describes host mixer | No independent PODxt global-setting or routing codec recovered. Do not reuse host settings as device commands. |
| Tone import/export and compatibility | Guide pp. 3•18–3•19 describes upgrade, Save As and target-specific conversion, with dual-tone restrictions and unsupported products | A shared `.l6t` suffix does not establish lossless interchange or PODxt target support for every tone. |
| Libraries/order/bank/slot | Guide pp. 3•20–3•21 describes `.l6b`/`.l6c` imported as setlists; setlist entries are shortcuts and reorder changes host MIDI bank/program selection | Host ordering semantics only. PODxt memory positions, bank bases and physical save/recall remain unverified. |
| Device backup or maintenance | No device backup/restore protocol recovered in this pass | Current examples/metadata are not a complete backup. No update, bank switch, write or recovery operation authorized. |

## Tone survey and lossless boundary

The file-reader structurally validated all **1,120** supplied `.l6t` examples,
including FORM lengths, nested IFF bounds and odd-length padding:

| Outer FORM type | Count | Observed structure, without semantic interpretation |
| --- | --- | --- |
| `L6PA` | 808 | Top-level `PINF`, `LIST/PATC`, `LIST/UNFO`; 806 also contain `MIDM`. All top-level PINFs are 76 bytes. |
| `L6P2` | 312 | Top-level `HEAD`, `LIST/BANK`, `LIST/UNFO`; no top-level PINF. Contains nested `FORM/L6PA` records. |

Using the PINF word positions already described in [POD presets](pod-presets.md),
806 top-level L6PA PINFs have first words `(3, 0x00130001)`; the other two are
`(2, 0x00080002)` and `(2, 0x00080003)`. The nested PINFs add 624 instances of
`(3, 0x00130001)`. None has the existing PODxt Live target `0x0003000A`.
These are observed words; this pass does not register their product identities or
assume that PINF version 3 has every version-1 field meaning. L6P2's `BANK` list
does not by itself identify PODxt hardware banks.

No model/parameter/pack values, domain ranges, unknown-field write rules, native
state chunks or lossless conversion are claimed. The inspector does not offer a
tone serializer. The existing Rust PODxt codec's target checks and preservation
rules remain unchanged. No proprietary preset bytes were turned into fixtures.

## Packs and entitlement boundary

The bundled guide pp. 1•3–1•7 distinguishes Standard/Platinum, individually
purchased packs, factory/device-specific add-ons, authorization and installation.
Page 3•18 describes unavailable models being bypassed by POD Farm. Those are
documented host behaviors, not permission to emulate authorizations or silently
drop unavailable models in Rust. The package includes License Manager 1.08;
its presence does not establish licenses, installed pack state or transferability
to the PODxt DSP. Framework symbols for asset validity/decryption are static leads,
not an approved execution path. No credential store or entitlement was read or
changed. Pack/model ID mappings and legitimate offline inventory storage remain
unrecovered.

## Validation and remaining acceptance

Ten synthetic reader tests pass: 32/64-bit/fat headers, CPU/bounds rejection,
load-command bounds, imports/signature-command presence, symbol bounds, opaque
PINF words, unknown/nested IFF chunks, truncation/trailing data and depth limits.
They test the research reader, not a Rust codec, conversion oracle or vendor
behavior. A fresh full extraction at `analysis/pod-farm-258-replay/` produced a
byte-identical private inventory with the hash above. The original hash remained
unchanged. No native/original-library oracle or CPU execution occurred in this
initial inventory pass; the subsequent two-leaf oracle is scoped separately above.

The required four Rust check identities are submitted separately through the
coordinator for the actual committed head; the preceding research counts are not
a substitute for that result.

This milestone supplies provenance, static extraction, architecture/API leads and
bounded compatibility/feasibility matrices. It leaves #40 open. Focused next
research should trace the framework's parameter-property and patch-data exports
statically to their tables and format dispatch, without calling initialization or
authorization paths. Only source-backed domains with explicit PODxt applicability
can unblock #29. Separately recover L6P2/HEAD/BANK and plugin-state semantics and
add owned malformed/model-mismatch/roundtrip fixtures once there is a real codec
contract. License inventory needs a read-only storage specification, not copied
user authorization data. No new implementation issues were opened in this partial
milestone; those remaining scopes require review before splitting #40.

Later [cache and serialization findings](pod-farm-cache-serialization.md) trace
one actual host metadata definition to three resource records and establish a
PINF-local preservation limit. They do not supply PODxt applicability or complete
format/state roundtrips, and do not complete #40.

The later [setter and outer-state pass](pod-farm-setter-state.md) identifies
conditional signed registry clamping, fixed two-tone L6P2 reconstruction and AU
dictionary-state error limits. Its 17 reconstruction checks include the earlier
nine resource/PINF checks. No plugin-state equivalence or active pack state was
established; #40 remains open and #29 held.
