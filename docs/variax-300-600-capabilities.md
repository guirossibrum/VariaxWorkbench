# Variax 300/600 capability audit

App baseline: `c29ebcd` (after #213). The Variax 300/600 share product ID
`0x00070004` in the fingerprinted 3.10 archive and the Variax 600 identity
capture. The 300 handbook describes the shared controls and model set.
Mechanical details are specific to the instrument described.

- **VH** means the [Variax 300 Pilot's Handbook, Rev A](https://line6.com/data/6/0a060b316ac34f05943d62560/application/pdf/Variax%20300%20User%20Manual%20-%20English%20.pdf) (40-00-0064).
- **PH** means the [PODxt Live Pilot's Handbook, Rev C](https://l6c-acdn2.line6.net/data/6/0a060b316ac34f0593d2ca219/application/pdf/PODxt%20Live%20Manual%20-%20Version%203%20%28%20Rev%20C%20%29.pdf). Pages below use the printed numbering. Page 3•2 refers forward to 4•10, and the actual model instructions are on 4•11–12.
- **WB** uses the existing Workbench 1.75 b121 static comparison behind #188–#206. **L6E** uses the Line 6 Edit 3.06 [parameter inventory](../research/podxt-live-parameters.json).
- **FW** means the fingerprinted Variax 3.10 firmware. Sources include the [dispatcher map](../research/variax-310-dispatch.json), [merged traces](firmware-variax-dispatch.md) and the #181 local research commits through `4e86e4b`. See [factory extraction](factory-extraction.md) for archive layout and original Java agreement.
- **Device evidence** labels prior owner checks and captures explicitly. Static or offline results carry no device verification. All checks for this audit used files.
- Status measures the listed app workflow at the baseline. **Done** includes completed offline workflows. **Partial** means a subset or a backend/probe exists. **Missing** includes intentionally excluded firmware functions. **Does not match** denotes a demonstrated discrepancy in the listed workflow. Physical-only rows are outside the software tally. Scope exclusions and unattributed opcodes call for no new implementation issue.

The tally covers 91 software capabilities: **10 done, 31 partial, 47 missing
and 3 mismatched**. Seven physical-only rows are outside the tally. The
firmware rows include unsupported maintenance and unclassified commands, so
the missing count is larger than the product backlog.

## Guitar, bridge and editor

| Capability | Source | Use | App status and code | Device evidence | Issue |
| --- | --- | --- | --- | --- | --- |
| Model knob and five-way selector, 50 model-bank positions | VH pp. 5–6, 10–20 | Guitar / computer | **Done** for representing and addressing all slots, `crates/line6-devices/src/family/variax.rs:91` | Owner-run backup captured all 50 via program changes | #203 for editor refresh |
| Ten writable positions in the two Custom banks | VH pp. 5–8 | Guitar / computer | **Partial**. Slot representation and backup work. Editor Sync is pending, `crates/line6-devices/src/family/variax.rs:99`, `src/families/variax/editor/guitar.rs:423` | Owner backup covers ten positions. Guitar-side save gesture unverified here | #178, #205 |
| Select target Custom slot during the hold-and-release save without auditioning target | VH p. 7 | Guitar | Physical only. Computer store is a separate transaction, `crates/line6-devices/src/family/variax/write.rs:322` | Handbook, no owner verification | #178 covers computer store |
| Pickup selector numbering and bank addressing | VH p. 10, WB `LocationToProgramNumber` | Guitar / computer | **Done** for program and position mapping, `crates/line6-devices/src/family/variax.rs:128`, `crates/line6-devices/src/backup.rs:241` | Backup index has Custom 1 position 1 = program 4 | #203 |
| Recall the saved tone position until first knob movement | VH p. 8, WB `L6VariaxEditor` | Guitar / computer | **Partial**. Saved flag and position are editable offline, `src/families/variax/editor/mod.rs:976`, `src/families/variax/file/preset.rs:514` | Both flag states occur in owner captures. Recall transition unverified | #202, #204 |
| Move the tone knob during Custom save to opt into physical tone on recall | VH p. 8, WB `L6VariaxEditor` | Guitar / computer | **Partial**. File supports the opt-out flag. Its off state disables editing the stored position, `src/families/variax/file/preset.rs:514` | Gesture and recall response unverified | #178, #202, #204 |
| Compare saved tone mode in semantic backup/restore checks | VH p. 8, FW metadata | Computer | **Does not match**. `same_preset` ignores the mode flag as well as position, `crates/line6-devices/src/family/variax.rs:431`. Raw folder compare includes both, `crates/line6-devices/src/backup.rs:456` | Acoustic 4 differs from the archive only in the position bytes; three complete backups contain both flag states | Proposed gap A |
| Electric volume taper and effect on tone, level differences across models | VH pp. 5, 10 | Guitar / computer | **Partial**. Wiring controls, trim and coefficient inputs exist, `src/families/variax/editor/mod.rs:533`, `src/host/engine.rs:112` | Knob positions captured. Audio response unverified | #204, #33 |
| Acoustic volume acts as a level control | VH pp. 5, 10 | Guitar / computer | **Partial**. Acoustic body and wiring are preserved. The editor presents shared wiring controls, `assets/model-ui.json:113`, `src/families/variax/editor/mod.rs:950` | Handbook. Level-only response unverified | Proposed gap B |
| Acoustic and applicable Reso tone control shapes modeled mic position | VH pp. 5, 10, 20 | Guitar / computer | **Partial**. Body and tone fields survive. The GUI labels the field generically, `src/families/variax/editor/mod.rs:976` | Handbook. Reso 5 owner test changed trim, so mic behavior remains unverified | Proposed gap B, #204 |
| Reso 2 tone controls drone level | VH p. 20 | Guitar / computer | **Partial**. Sitar-specific DSP generation exists, `src/host.rs:87`, `src/dsp/resonance.rs:137`. Tone UI remains generic, `src/families/variax/editor/mod.rs:976` | Handbook. Drone response unverified | Proposed gap B |
| T-Model 4 neck tone bypass | VH p. 11, FW slot 6 | Guitar / computer | **Partial**. Sign-sensitive coefficients preserve the factory bypass, `src/pickup/calculated.rs:82`. Generic positive capacitance editing and auto-values can replace the signed value, `src/families/variax/file/edit.rs:69`, `src/families/variax/file/preset.rs:442` | Static archive and handbook agree. Device audio unverified | Proposed gap B |
| Spank bridge tone stays active | VH p. 12, FW slot 14 | Guitar / computer | **Partial**. Archive has positive tone capacitance and the editor preserves wiring on import, `src/families/variax/file/preset.rs:155` | Static archive; device audio unverified | Proposed gap B |
| Shipping Custom assignment lookup, comparison and eventual offline reconstruction | VH p. 8, FW compact and expanded regions | Computer | **Partial**. This branch adds a tested [shipping assignment table](../research/variax-300-handbook-custom.tsv). Existing comparator targets archive slots, `src/factory_comparison.rs:52`, `src/factory_reference.rs:59` | Handbook and offline archive are distinct baselines. Owner captures reflect the current stored settings | Proposed gap C |
| Six individual piezo signals and modeled guitar audio | VH pp. 1–2 | Guitar | Physical only. The host generates coefficients, `src/host/engine.rs:112` | Handbook. Full audio equivalence unverified | #33, #34 |
| Mono ¼-inch analog output | VH p. 3 | Guitar | Physical only. No device status derives from this jack, `src/app/detection/mod.rs:250` | Handbook | Outside app scope |
| Six AA cells or emergency 9 V, battery circuit with mono plug | VH p. 3 | Guitar | Physical only. No battery telemetry in Variax identity/read path, `crates/line6-devices/src/family/variax.rs:153` | Handbook | #201 for connection guidance |
| XPS Mini external power over TRS and mono out | VH p. 4 | Guitar | Physical only. Bridge identity is the app's visibility boundary, `src/app/detection/mod.rs:302` | Handbook | #201 for connection guidance |
| Neck relief, intonation and action adjustments | VH pp. 21–22 | Guitar | Physical only, outside coefficient and preset workflow, `src/families/variax/editor/mod.rs:950` | Handbook | Outside app scope |
| Digital jack via supported PODxt Live | VH p. 3, PH p. 3•2 | PODxt / computer | **Done** for Linux identity and current metadata over the supported bridge, `src/app/registry.rs:19`, `crates/line6-devices/src/family/variax.rs:247` | Owner-run Variax 600 3.10 / PODxt Live 3.01 | #114 |
| Automatic identification, explicit Refresh guitar and detach status | WB `L6VariaxDevice.CheckConnectionStatus` | Computer | **Partial**. #213 verifies the route and creates the tab. Unsolicited Workbench-style reconnect/follow remains open, `src/app/detection/mod.rs:302`, `src/app/shell/detect.rs:53` | Explicit Device check owner-verified. Automatic path has fake-transport coverage | #201, #224 |
| PODxt global Variax control ON/OFF | PH p. 3•2 | PODxt / computer | **Partial**. Generic POD global read exists. The GUI has no equivalent global control, `crates/line6-devices/src/family/podxt/mod.rs:1118` | Owner saw ON on the POD; USB connection affected display/control ownership | #220 for read, proposed gap D for verified control |
| Analog guitar and Variax inputs can coexist | PH p. 3•2 | PODxt | Physical only. Separate app audio route validation, `docs/pod-usb-audio.md` | Handbook, simultaneous input unverified | #99, #100 |
| Save/recall linked F- model in PODxt channel | PH pp. 4•11–12, L6E family model field | PODxt / computer | **Partial**. Channel switching is implemented, linked editor selector pending, `crates/line6-devices/src/family/podxt/mod.rs:41`, `src/families/podxt/file/controls.rs:937` | Owner verified F-Chime 1 then F-T-Model 1 with USB disconnected during selection. USB-triggered linked recall unverified | #218, #222, proposed gap D |
| Linked U- and Custom model recall | PH pp. 4•11–12 | PODxt / computer | **Missing** as an app workflow, `crates/line6-devices/src/family/podxt/mod.rs:41` | Handbook. Owner verified F- recall only | #218, #222, proposed gap D |
| Linked setting to keep the guitar's current model on recall | PH p. 4•11 | PODxt / computer | **Missing** as an app workflow, `crates/line6-devices/src/family/podxt/mod.rs:41` | Handbook; unverified on rig | #218, #222, proposed gap D |
| POD Edit page tracks physical model changes and can audition a linked model | PH pp. 4•11–12, L6E `WrapVariaxMessage` | PODxt / computer | **Does not match** for wrapped audition on tested USB route, `crates/line6-devices/src/family/podxt/mod.rs:1536` | Both tone and guitar dumps unchanged after the owner sent F-Spank 1 and attempted undo | Proposed gap D, #221 |
| Per-family linked fields and additional model settings in POD channel | PH pp. 4•11–12, L6E family fields | PODxt / computer | **Partial**. Static parameter/offset inventory exists, `research/podxt-live-parameters.json:4067`. App UI and verified controls are pending | Owner F- electric selection only. Other families remain separate | #218, #220–#222 |
| Return Variax control to the PODxt | WB `SetXTLiveVariaxMessagesEnable` | Computer / PODxt | **Does not match** the expected display handoff on tested USB route. Release sender exists, `crates/line6-devices/src/family/podxt/mod.rs:1523` | Owner standalone release left the display unchanged. Internal ownership state unverified | Proposed gap D, #201 |
| POD amp voicing for Variax acoustic models | PH p. 5•4 | PODxt / computer | **Partial**. Static model inventory includes Variax Acoustic, `research/podxt-live-parameters.json:3699` | Handbook; app audition of this specific amp unverified | #218, #221 |
| POD tuner with connected guitar | PH pp. 2•12–13 | PODxt / computer | **Partial**. Family tuner backend, GUI follow pending, `crates/line6-devices/src/family/podxt/mod.rs:1168` | POD operation owner-tested; Variax input-specific tuner behavior unverified | #220 |
| Body, pickup model, placement, angle, enable, phase and series/parallel editing | WB `L6VariaxEditor`, `L6PickupControl` | Computer | **Partial**. Core edits work offline. Workbench placement presets, gestures and artwork are open, `src/families/variax/editor/mod.rs:581` | Offline fixtures and native reference. Physical response unverified | #188–#191, #224 |
| Per-preset alternate tuning and 12-string offset/mix | WB `TuningPresetDialog`, VH pp. 16, 19 | Computer / guitar / PODxt | **Done** for offline edits, `src/families/variax/editor/guitar.rs:741`. Persistent managed user presets and live sends are separate work | Factory metadata/oracle. Owner tuning/audio outcome unverified | #193, #204 |
| Open and save named model, bundle and collection files | WB `L6Librarian`, `L6VariaxEditor` | Computer | **Done** for existing root files and non-overwriting copies, `src/families/variax/file/document.rs:59`, `src/families/variax/file/document.rs:218` | Offline original-file compatibility | #194–#196 for remaining library workflows |
| Import or export one bank position as a model file | WB `L6VariaxEditor` | Computer | **Partial**. Offline CLI handles slot extraction and replacement, `src/main.rs:793`. App slot actions pending | Offline compatibility only | #194 |
| Copy, paste or swap models and banks | WB `L6Librarian` | Computer | **Missing** in the editor bank strip, `src/families/variax/editor/guitar.rs:319` | Offline UI only | #195 |
| Rename banks and create models, bundles or collections | WB `L6Librarian` | Computer | **Missing** in the GUI. Existing files can be loaded and edited, `src/families/variax/file/document.rs:59` | Offline UI only | #196 |
| Export the model list as HTML | WB `L6Librarian` | Computer | **Missing** in the editor bank strip, `src/families/variax/editor/guitar.rs:319` | Offline UI only | #198 |
| Manage named tuning presets across sessions | WB `TuningPresetDialog` | Computer | **Partial**. Built-in presets are selectable, `src/families/variax/editor/guitar.rs:658`. Saved user presets pending | Offline UI only | #193 |
| Remember artwork path and editor defaults | WB `L6EditorAppPreferencesDialog` | Computer | **Partial**. Artwork control exists, `src/families/variax/editor/mod.rs:333`. Full preferences workflow is tracked separately | Offline UI only | #199 |
| Current metadata converted to model/settings file | WB `RequestDevicePatch`, FW `0x3C` | Computer | **Done** for supported current-preset settings, `src/families/variax/editor/mod.rs:261`, `src/families/variax/file/preset.rs:75` | Owner verified stage 3 and Reso 5 reads | #176, #204 for live Get/Edit Buffer |
| Read and save all 60 preset settings | WB `RefreshBundle`, FW program change plus `0x3C` | Computer | **Done** for Device check backup, `crates/line6-devices/src/family/variax.rs:487`, `crates/line6-devices/src/backup.rs:268` | Three verified owner backup manifests, 60 preset settings per backup | #197 and #203 for editor reuse |
| Convert backup to editable bank and refresh in place | WB `RefreshBundle`, `L6Librarian` | Computer | **Missing**. Loader decodes backup, GUI keeps it separate from bank document, `crates/line6-devices/src/backup.rs:358`, `src/families/variax/file/document.rs:59` | Owner backup data available; workflow unverified | #197, #203 |
| Follow model knob and selector program changes | WB `L6VariaxPatchManager.OnSysex`, FW PC/session | Computer | **Partial**. Read/listen path and backup addressing exist. Editor follow missing, `crates/line6-devices/src/family/variax.rs:268` | Host-issued PC supported by owner backup; physical follow behavior unverified | #202 |
| Monitor live volume position | FW CC 7 | Computer | **Partial**. Family probe recognizes CC 7, editor has no live indication, `crates/line6-devices/src/family/variax.rs:557` | Owner full 0–127 sweep | #202 |
| Monitor live tone position and send from editor | WB `EncodeParamChange`, FW CC 79 | Computer | **Partial**. Probe recognizes CC 79, GUI control is offline, `crates/line6-devices/src/family/variax.rs:557`, `src/families/variax/editor/mod.rs:976` | Owner full 0–127 sweep. App sends unverified | #202, #204 |
| Show clip, DSP overload and MIDI queue overflow | WB `OnSysex`, FW notices `0x28/0x4D/0x74` | Computer | **Partial**. Write path accounts for asynchronous replies. Editor indicators missing, `crates/line6-devices/src/family/variax/write.rs:354` | Static/Workbench evidence, owner UI behavior unverified | #202, #205 |
| Track modified slots, Sync, revert and warn on exit | WB `L6VariaxPatchManager.Synchronize` | Computer | **Missing** as device Sync. File dirty state and disconnected controls exist, `src/families/variax/file/document.rs:175`, `src/families/variax/editor/guitar.rs:423` | #176 was edit-buffer only. Owner G7 tests pending | #205, #177, #178 |
| Inspect archived firmware offline | FW FORM/L6FF, updater research | Computer | **Done** for bounded offline inspection and exact archive extraction, `src/factory_reference.rs:82` | Original Java and Rust decoded 60/60 archive slots in prior validation | #30, #181 |
| Resolve each fixed model-bank position to its source instrument and pickup setup | VH pp. 11–20, FW templates | Guitar / computer | **Missing** as a position reference in the app. The app displays the saved preset name and independent body/pickup labels, `src/families/variax/editor/guitar.rs:459`, `assets/model-ui.json:1` | Archive body IDs checked against all 50 handbook positions below | #203, #224 |

## Source models by selector position

All 50 positions below match the **body types** of the 3.10 factory templates.
Every enabled pickup ID and every body ID from those slots resolves in the app
catalog. The archive stores Lester Custom position 4 using the library labels
Lester Custom Bridge and Lester Custom Neck, while the handbook describes the
bridge and middle setup. That is a label difference. Physical placement or
acoustic equivalence cannot be inferred from those names.
The app uses Workbench-style asset labels such as T-Model F-Hole, Special Bird,
and Masonite Plank. These body choices identify the underlying models. Individual
pickup choices and enabled states are separate fields. A slot label or body
choice alone cannot identify the pickup combination, since a saved preset can
replace either. The model sources below are lookup information for #203/#224.
The number in each heading is the physical selector position.

| Bank / VH page | 1 (bridge end) | 2 | 3 | 4 | 5 (neck end) |
| --- | --- | --- | --- | --- | --- |
| T-Model / 11 | 1960 Telecaster Custom, bridge | Modified 1968 Telecaster, series wiring | 1968 Telecaster Thinline, bridge + neck | 1960 Telecaster Custom, neck, tone bypass | 1968 Telecaster Thinline, neck |
| Spank / 12 | 1959 Stratocaster, bridge | Stratocaster, bridge + middle | Stratocaster, middle | Stratocaster, neck + middle | Stratocaster, neck |
| Lester / 13 | 1958 Les Paul Standard, bridge | 1952 Les Paul Goldtop, P-90 bridge | 1958 Les Paul Standard, bridge + neck | 1961 Les Paul Custom, bridge + middle | 1958 Les Paul Standard, neck |
| Special / 14 | 1956 Les Paul Junior, P-90 bridge | 1976 Firebird V, bridge | 1955 Les Paul Special, bridge + neck | 1976 Firebird V, neck | 1955 Les Paul Special, neck |
| R-Billy / 15 | 1959 Gretsch 6120, bridge | 1956 Gretsch Silver Jet, bridge | 1959 Gretsch 6120, bridge + neck | 1956 Gretsch Silver Jet, neck | 1959 Gretsch 6120, neck |
| Chime / 16 | 1968 Rickenbacker 360, bridge | 1966 Rickenbacker 360-12, bridge | 1968 Rickenbacker 360, bridge + neck | 1966 Rickenbacker 360-12, neck | 1968 Rickenbacker 360, neck |
| Semi / 17 | 1961 Gibson ES-335, bridge | 1967 Epiphone Casino, bridge | 1961 Gibson ES-335, bridge + neck | 1967 Epiphone Casino, neck | 1961 Gibson ES-335, neck |
| Jazzbox / 18 | 1957 Gibson ES-175, bridge | 1953 Gibson Super 400, bridge | 1957 Gibson ES-175, bridge + neck | 1953 Gibson Super 400, neck | 1957 Gibson ES-175, neck |
| Acoustic / 19 | 1959 Martin D-28 | 1970 Martin D12-28 | 1967 Martin O-18 | 1966 Guild F212 | 1995 Gibson J-200 |
| Reso / 20 | 1935 Dobro Model 32 | Coral Sitar | 1965 Danelectro 3021, both pickups | Gibson Mastertone-inspired banjo | 1928 National Tricone |

## Firmware command capabilities

The 3.10 map contains 110 ordinary dispatch-table entries, including 64 direct
returns and 46 command-specific handlers. Each of the 46 appears once below.
The two universal-header paths are separate from that table. A handler with an
unattributed purpose is described by its observed action, with #181/#226
tracking attribution. `0x45` has different meanings as a request and a reply.

Code references distinguish the family receive surface
(`crates/line6-devices/src/family/variax.rs:38`) and the family write surface
(`crates/line6-devices/src/family/variax/write.rs:131`). These point to the
request definitions when an opcode is absent. "Static" below describes firmware
evidence with no device verification.

| Capability | Source | Use | App status and code | Device evidence | Issue |
| --- | --- | --- | --- | --- | --- |
| Load a RAM block and run its follow-up handler | FW `0x10`, `P:0x1451D` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Load a 60-word X-memory block | FW `0x16`, `P:0x144FA` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Load a 12-word preset-buffer region and run its follow-up handler | FW `0x17`, `P:0x14506` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Load six word pairs and a byte | FW `0x1C`, `P:0x144F6` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Set the six-word RAM region also exposed by 0x1E | FW `0x1D`, `P:0x145FE` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Read the six-word RAM region shared with 0x1D | FW `0x1E`, `P:0x14618` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Load a 52-word preset-buffer region and run its follow-up handler | FW `0x23`, `P:0x1465B` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Read DSP usage (Workbench debug) | FW `0x29`, `P:0x1469D` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Enable or disable program/CC session reports | FW `0x2A`, `P:0x144DD` | Computer | **Done**. Session wrapper and request, `crates/line6-devices/src/family/variax.rs:38` | Owner metadata sweep and CC capture | #202 for editor follow |
| Load a nine-word preset-buffer region and run its follow-up handler | FW `0x35`, `P:0x14569` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Set an X-memory word and use the shared 0x35 trailer | FW `0x36`, `P:0x14513` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Set a preset-buffer word and use the shared 0x35 trailer | FW `0x37`, `P:0x14518` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Send changed edit-buffer words | FW `0x3B`, `P:0x14433` | Computer | **Partial**. Bounded difference writer, editor controls offline, `crates/line6-devices/src/family/variax/write.rs:285` | Fake transport. #176 used full upload | #204 |
| Read a range from the active preset buffer | FW `0x3C`, `P:0x144A9` | Computer | **Partial**. Two fixed metadata ranges, `crates/line6-devices/src/family/variax.rs:223` | Owner metadata read. Wider reads static | #119, #181, #204 |
| Write through the X:0x39E pointer and call an indirect target | FW `0x3E`, `P:0x143D7` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Read a bounded range through the X:0x39E pointer | FW `0x3F`, `P:0x1446C` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Replace the preset/DSP flag word | FW `0x40`, `P:0x14545` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Read a DSP state word from Y:0xD1 | FW `0x41`, `P:0x14554` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Replace the full edit buffer and reload DSP | FW `0x43`, `P:0x14668` | Computer | **Partial**. Full writer and owner probe. Editor send pending, `crates/line6-devices/src/family/variax/write.rs:241` | Owner #176: same program and −6 dB trim, with settings readback and listening | #176, #204 |
| Read a complete stored slot or the current buffer without program change | FW `0x44`, `P:0x147F2` | Computer | **Missing**. Absent on audited base, `crates/line6-devices/src/family/variax.rs:38`. Bounded owner probe on lane 5 at `8cdfcdc` | Static; #225 owner run pending in available record | #225 |
| Receive a full program and conditionally store it | FW `0x45`, `P:0x1480A` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Drain a fixed raw-byte stream and send a status reply | FW `0x47`, `P:0x147FE` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Update the preset/DSP flag word while clearing bit 20 | FW `0x49`, `P:0x1452A` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Toggle DSP modeling bypass during transfers | FW `0x4C`, `P:0x14561` | Computer | **Partial**. Write transaction toggles bypass. Current metadata read uses session only, `crates/line6-devices/src/family/variax/write.rs:175`, `crates/line6-devices/src/family/variax.rs:247` | Owner current read without bypass. Failure recovery unverified | #181, #203 |
| Request the session-gated tone controller report | FW `0x4E`, `P:0x143B5` | Computer | **Missing**. The listener handles incoming CC 79, `crates/line6-devices/src/family/variax.rs:557` | Static query; owner observed knob-originated CC 79 | #181, #202, #226 |
| Set six global string levels in RAM | FW `0x54`, `P:0x144E3` | Computer | **Missing**. No global-level API, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static, also WB String Volume Setup | #206, #181 |
| Persist six global string levels | FW `0x56`, `P:0x14583` | Computer | **Missing**. No global-level API, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static, also WB String Volume Setup | #206, #181 |
| Read six global string levels | FW `0x57`, `P:0x1458E` | Computer | **Missing**. No global-level API, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static, also WB String Volume Setup | #206, #181 |
| Copy factory templates into one or all 50 model-bank slots | FW `0x58`, `P:0x145BD` | Computer | **Missing**. No bulk factory-to-user store request, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226; proposed gap C covers offline reference |
| Copy stored model-bank data towards the template region | FW `0x59`, `P:0x147D8` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Load the first 240-word X-memory block | FW `0x5E`, `P:0x144FE` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Load the second 240-word X-memory block | FW `0x5F`, `P:0x14502` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Receive a code block for firmware maintenance | FW `0x60`, `P:0x14687` | Computer | **Missing**. Firmware installation/reset outside product scope, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #30, #181, #226 |
| Persist the active buffer in a regular preset slot | FW `0x61`, `P:0x14731` | Computer | **Partial**. Bounded store API and probe. GUI Sync pending, `crates/line6-devices/src/family/variax/write.rs:322` | Static and fake transport. Owner store pending | #177, #178, #205 |
| Program the expanded user-slot region | FW `0x62`, `P:0x1474A` | Computer | **Missing**. Firmware installation/reset outside product scope, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #30, #181, #226 |
| Bulk erase with the flash window selected | FW `0x63`, `P:0x146BD` | Computer | **Missing**. Firmware installation/reset outside product scope, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #30, #181, #226 |
| Program the compact factory-template region | FW `0x67`, `P:0x14798` | Computer | **Missing**. Firmware installation/reset outside product scope, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #30, #181, #226 |
| Bulk erase in the other flash window | FW `0x68`, `P:0x146C0` | Computer | **Missing**. Firmware installation/reset outside product scope, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #30, #181, #226 |
| Query an unattributed diagnostic reply | FW `0x69`, `P:0x14394` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Query a second unattributed diagnostic reply | FW `0x6A`, `P:0x143A4` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Query a third unattributed diagnostic reply | FW `0x6B`, `P:0x143C6` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Store a bank name | FW `0x6C`, `P:0x14623` | Computer | **Partial**. Rename API exists. GUI Sync pending, `crates/line6-devices/src/family/variax/write.rs:422` | Static and fake transport. Owner write unverified | #181, #205 |
| Read a bank name | FW `0x6D`, `P:0x14650` | Computer | **Partial**. Bank-name request and optional backup replies, `crates/line6-devices/src/family/variax.rs:380` | The three complete owner backup folders have no bank-name replies | #203 |
| Dispatch maintenance subcommands, including boot transfer | FW `0x6F`, `P:0x1468B` | Computer | **Missing**. Firmware installation/reset outside product scope, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #30, #181, #226 |
| Read an indexed X-memory diagnostic reply | FW `0x75`, `P:0x146CF` | Computer | **Missing**. Absent from request surface, `crates/line6-devices/src/family/variax.rs:38`, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #181, #226 |
| Reset the Variax processor | FW `0x7D`, `P:0x1469B` | Computer | **Missing**. Firmware installation/reset outside product scope, `crates/line6-devices/src/family/variax/write.rs:131` | Static | #30, #181, #226 |
| Universal identity response | FW universal `0x7E` header, `P:0x14840` | Computer / PODxt | **Done** for the bounded identity request, `crates/line6-devices/src/family/variax.rs:153` | Owner identity verified on Variax 600 3.10 | #114, #201 |
| Universal `0x6E` maintenance response | FW universal `0x7E` header, `P:0x14869` | Computer / updater | **Missing**. The app uses universal identity, `crates/line6-devices/src/family/variax.rs:153` | Static | #181, #226 |

The Tyler-only Workbench commands `0x1B`, `0x3D` and `0x48` are direct
returns on this image. WB's CC 84 tuning-override decoder has no Variax 600
capture. The supported legacy tuning path is per-preset.

## Mismatches and verified distinctions

1. **Selector position, MIDI program and flash slot are different numbers.**
   Position 1 is the bridge end, and 5 is the neck end. The backup and the slot
   picker show the same one-based position. The family API accepts zero-based
   slots. For position `p` in zero-based bank `b`, the flash index is
   `5*b + (5-p)`. The MIDI number adds 55 for banks 1–10. Thus Custom 1 / 1 is
   program 4 and flash index 4. Reso / 5 is program 105 and flash index 50.
   Imported bundle captions and built-in sample content describe their files.
   They provide no physical preset identification. The picker has no fixed
   source-instrument lookup. [Bank strip](../src/families/variax/editor/guitar.rs)
2. **The Tele tone bypass is position 4.** VH p. 11 assigns the bypass to the
   neck sound of the 1960 Telecaster Custom, which is position 4. Position 5
   uses the 1968 Thinline neck. The archived position 4 has negative tone
   capacitance, position 5 has positive, and the calculated response uses the
   sign to freeze tone gain. The editor has generic tone wiring controls and an
   auto-values operation that replaces capacitance. The audit also found a
   source-label discrepancy for Lester Custom / 4 in the table above.
   [Coefficient code](../src/pickup/calculated.rs) · [Auto-values code](../src/families/variax/file/preset.rs)
3. **The two tone fields have different meanings.** The file editor stores
   both. The raw folder comparator compares both. The restore matcher used to
   locate the starting preset and verify its return ignores bytes 61–64,
   including the saved/live flag. Two otherwise equal settings with different
   recall behavior can therefore pass that check. Capture provenance matters
   for the position itself, since live settings can follow the knob. The exact
   factory comparator checks the represented position and mode as separate
   values. [Restore matcher](../crates/line6-devices/src/family/variax.rs) · [Raw backup comparison](../crates/line6-devices/src/backup.rs) · [Factory comparison](../src/factory_comparison.rs)
4. **The shipping Custom table and firmware-expanded Custom slots differ.**
   The compact region has 50 model-bank templates and no Custom templates.
   The expanded region has 60 programs, including ten Custom positions.
   Compared to the handbook-indicated source slot in that same archive,
   only one expanded Custom assignment matches on name and all decoded
   settings other than live tone position (Custom 1 / 4, Reso 2).
   In particular, all five archived Custom 2 positions are alternate-tuning
   presets. Keep shipping assignments, firmware archive defaults, built-in
   examples and captured user slots as distinct data sources. The handbook
   specifies source-slot identities and provides no per-slot calibrated DSP
   or tone-knob value. [Archive layout](factory-extraction.md) · [Handbook assignments](../research/variax-300-handbook-custom.tsv)
5. **The 30/50 result measures complete captured metadata.** All three
   complete owner backup manifests verified. The audit paired the two
   32-word replies per position, decoded low-first nibbles to canonical
   words, and compared 192 bytes to the matching expanded archive slot
   starting at word 851. The three backups give identical counts. There
   are 30/50 exact raw matches, 31/50 when omitting only the position bytes,
   and 41/50 on decoded name plus body ID. Differences occur in T-Model (5),
   Spank (5), Lester (4), Special (4), R-Billy (1) and Acoustic (1).
   This includes body and pickup changes, renames and one Acoustic 4
   difference confined to position bytes. The save flag agrees in all 50
   model-bank comparisons. All three 60-position backups contain both flag
   values. None of these comparisons measures the full stored DSP program
   or audible differences. [Current decoder](../crates/line6-devices/src/family/variax.rs) · [Archive extraction](factory-extraction.md)
6. **PODxt standalone recall and computer control have different evidence.**
   Standalone F- recall worked with USB disconnected during channel
   selection. The first USB linked-recall test stopped at baseline before
   sending a channel change. The standalone release request left the
   display unchanged. The separately completed wrapped model change left
   both dumps unchanged. F- and U- are distinct program categories, and
   the latter plus Custom and keep-current still need verified workflow
   coverage. [Owner ledger](../README.md)
7. **Bank names and coefficients have different coverage from preset
   settings.** The three verified owner backups cover all 60 metadata
   positions and have no bank-name reply files. The audited Rust backup
   now requests bank names, and can stop the sweep on a missing reply.
   The older Workbench review inferred a metadata-only firmware read.
   The newer firmware trace shows that `0x3C` addresses the active buffer
   more broadly and `0x44` has a full-slot path. A successful 60-slot
   metadata sweep alone establishes neither names nor full DSP readback.
   [Dispatch notes](firmware-variax-dispatch.md) · [Backup implementation](../crates/line6-devices/src/family/variax.rs)

## Reviewer handoff for GitHub

The [worktree delivery rules](parallel-work.md) reserve GitHub updates for the
reviewer. These are deduplicated issue and comment drafts for that handoff.
Gap D tracks hardware verification and routing for the existing PODxt editor
work in #221 and #222.

### A. Gap: preserve saved-tone semantics in Variax backup restore checks

Source: VH p. 8 and audit #227, mismatch 3.

**Original behavior.** Recall may start from the stored tone position or the
physical knob, based on the saved mode. The first knob movement can override
a saved setting.

**Rust today.** `same_preset` skips the mode byte during semantic restore
matching. Raw file comparison already retains it.

- [ ] Distinguish toggle-only changes in restoration and backup comparison.
- [ ] Document when a captured position reflects a live knob and when a source
  supplies a stored position.
- [ ] Cover both flag states and ambiguity between otherwise equal slots with
  synthetic data. Preserve original capture bytes.

Hardware boundary: offline comparison and fake transports. Physical recall
requires a separately approved owner check.

### B. Gap: preserve and explain model-specific Variax knob behavior

Source: VH pp. 5, 10–12, 20 and audit #227, mismatch 2.

**Original behavior.** Tone covers mic position, Sitar drone level, normal
electric tone and a bypassed Tele neck preset. Electric volume may change tone
and acoustic volume controls level.

**Rust today.** The coefficient and file layers can represent the relevant
settings. The GUI shows generic tone and wiring controls, and pickup
auto-values can replace the factory bypass marker.

- [ ] Surface documented behavior when the current model and wiring establish
  it. Account for edited presets and the Spank bridge tone path.
- [ ] Keep bypass intact in unrelated edits and provide a deliberate edit path.
- [ ] Use synthetic file cases or the existing offline reference oracle.

Hardware boundary: offline UI and coefficient tests. Owner-only audio and
knob comparison if separately approved.

### C. Gap: compare Variax Custom assignments against the handbook offline

Source: VH p. 8, archive 3.10 layout and audit #227, mismatch 4.

**Original behavior.** The handbook lists ten Custom source assignments for
shipping guitars. Firmware 3.10 also carries a version-specific expanded bank.

**Rust today.** The archive comparison compares physical slot indices.
The #227 reference data maps shipping positions to source models.

- [ ] Let an offline comparator name the selected baseline.
- [ ] Expose differences between handbook assignments and the 3.10 expanded
  defaults. Label reconstruction from factory model data as derived and flag
  unknown shipping-specific parameters.
- [ ] Export any derived reference only as a new file. Cover all ten positions.

Hardware boundary: no reset, firmware install or device store. #177/#178
continue to cover owner-run writes.

### D. Gap: verify PODxt Live USB Variax control ownership and linked audition

Source: PH pp. 3•2, 4•11–12, WB release and the two failed owner observations
in #227, mismatch 6.

**Original behavior.** PODxt can follow or change the attached guitar on its last
Edit page and recall a linked choice per channel.

**Rust today.** A standalone release sender and a two-value wrapped setter
exist. Owner runs observed no display handoff and no edit effect under USB.
The standalone F- recall has separate passing evidence.

- [ ] Attribute control ownership, relay setup and release using offline
  traces first. Record the input route and what each status observation proves.
- [ ] Design a bounded owner check with independent POD tone and Variax
  readback, together with an explicit baseline and restoration criterion.
- [ ] Feed verified choices into #221/#222. Preserve the keep-current, F-, U-
  and Custom meanings from #218.

Hardware boundary: offline research, followed only by individually approved
owner procedures. Existing approvals cover their original limited tests.

### Existing issue comments for the reviewer

| Issue | Comment to add |
| --- | --- |
| #201 | #227 confirmed VDI identity and current metadata through the supported bridge. #213 covers identity admission and explicit Refresh guitar. Keep unsolicited reconnect and device status in scope. Guitar battery/analog power is distinct from USB presence, and release acceptance alone did not clear the POD display. |
| #202 | Owner CC sweeps confirmed CC 7 and CC 79. Include current vs saved tone display and show that a live update does not alter the stored mode flag. The session gates unsolicited program and control reports. |
| #203, #224 | Use bridge-to-neck one-based slot positions. The #227 source table maps all 50 documented factory choices to physical positions. Preserve current preset names and present the instrument mapping as a factory reference. The three owner backups lack bank-name replies, so test name reads separately. |
| #204 | #176 demonstrated a full `0x43` send and a trim change on Reso 5. Incremental sends and CC 79 transmission are distinct. Wide buffer reads are static findings, with no owner evidence from the existing two metadata blocks. |
| #205, #178 | Use archive and shipping defaults as separate concepts. Verify stored/live tone mode during comparisons. #181 reports that bank rename persistence differs from preset-slot storage. Retain the owner-test boundary. |
| #206 | `0x54`, `0x56` and `0x57` are separate set, persist and read paths for six global values. A 60-position metadata backup does not cover these values. |
| #218, #221, #222 | Preserve family-specific channel fields and keep-current/F-/U-/Custom model choices. Owner evidence supports standalone USB-disconnected F- recall. The wrapped live setter needs independent verification before enabling it. |
| #181, #226 | #227 accounts for all 46 non-return ordinary handlers and the separate universal replies. Continue attribution and product-scope review for unknown commands. #225 covers bounded stored-slot readback. |

## Owner test coverage

The new handbook table test exercises all ten assignments through the existing
Rust slot-to-program mapping. The archive was extracted again from the verified
source hash. Its 50 compact templates, 60 expanded slot files and 60 Rust
settings files match the earlier extraction byte for byte. Original Workbench
Java and Rust agree on the preset name and represented model settings for all
60 archived slots. Java and Rust serialize those settings into different file
bytes.

The [owner test card](owner-tests/variax-capabilities.md) describes the
remaining physical comparisons. It references the existing dedicated cards
for current-settings reads, stored-slot reads and stored-bank writes. There
were no live runs for #227.
