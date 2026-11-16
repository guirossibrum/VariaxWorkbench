# Firmware-improvement feasibility assessment

The archived files support further offline research and several concrete
improvement hypotheses. They do **not** yet support a deployable firmware change
for the Variax 600 or PODxt Live. This assessment closes issue #19's research
scope; it does not approve a patch, firmware writer, installation or hardware
experiment. No modified firmware, deployable image, update package or new device
operation was generated.

The most useful near-term improvements belong in the Rust host application:
precise file/target validation, preservation of unknown data, explicit uncertainty
in update results, and reliable offline comparison. Those are independently
testable without changing irreplaceable hardware. Device-side changes remain
conditional on gaps described below.

## Evidence boundary

This assessment uses the source-bound static and emulator results already
recorded in the repository. The primary archives remain private and unchanged:

| Artifact | Source SHA-256 | Established interpretation |
| --- | --- | --- |
| Variax 300/600 3.10 | `e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a` | Four regions; DSP56300-compatible program/loader structure; 50 templates and 60 complete archived slot programs |
| PODxt Live 3.01 | `738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5` | Nineteen regions; SHARC-compatible sections, internal additive checksums and a word-indexed named resource directory |
| Central host updater engine | `6786a97458a5a6e5d225e5c2d2804281a8e0a082a0d992f558ce041c144ae078` | Selected host methods, applicability helpers and response callbacks statically mapped and exercised with intercepted calls in private x86 emulation |

The [firmware map](firmware-map.md), [factory extraction](factory-extraction.md),
[update source map](update-recovery.md) and [bounded simulation](receiver-simulation.md)
contain method/word addresses, integrity coverage and source fingerprints.
They distinguish host code execution from static device-firmware analysis. Device
firmware has not been executed. Exact installed chips, peripheral behavior,
physical flash geometry and an independently usable recovery path remain unknown.

## Candidate improvements and what would establish feasibility

| Candidate | Grounding | Assessment and missing proof |
| --- | --- | --- |
| Earlier validation of user/factory transfers | Application handlers P:0x1474A and P:0x14798 call programming-like helpers before checking the final terminator; unlike the code transfer, the modeled outer stream has no additive payload checksum | A staged receive/verify/commit design might reduce some partial-transfer effects. Available RAM, transfer pacing, flash bank semantics, compatibility with existing host tools and interrupted-commit recovery are unestablished. It is not a drop-in checksum change. No device patch is justified yet. |
| More reliable update completion reporting | Original erase method 0x100D4B10 returns zero on the modeled timeout and boot-guard paths as well as its normal path; selected POD parent waits log missing acknowledgments without necessarily aborting | Improve the replacement host's result model: distinguish transport delivery, protocol reply and unknown durability. This is a host-side improvement, not evidence that a firmware modification is necessary. A modeled success must never become a claim of completed physical erase or write. |
| Additional/custom default presets | Exact 3.10 slots expose complete 3276-byte programs and metadata; 54 of 60 agree with compact-template expansion, six differ | Offline libraries and alternate/custom tuning settings can be preserved and compared now. Changing firmware defaults additionally requires verified generation, correct layout/integrity, installed-model compatibility and preservation of user/calibration data. The archive does not identify this owner's shipping state. No replacement factory image was made. |
| Model, pickup, tuning or sustain changes | The host coefficient generator and parameter formats are partly recovered; archived slots include alternate tunings | Host settings are a practical research surface; pickup settings refer to virtual models, not physical magnetic pickups. The guitar uses piezo string sensing. Device DSP behavior, state, timing, pitch shifting, bend tracking and body-dependent decay are not independently emulated. Metadata agreement or an additive checksum cannot demonstrate improved sound. Device-side DSP changes remain unassessed, rather than declared feasible. |
| POD model/effect changes | Region 4 has coherent SHARC-compatible code; the resource directory has 486 named records; the offline tone reader has separate host-format evidence | Resource names do not establish implementation, routing, performance budgets or safe editing. Region 5 remains opaque and firmware region 17 is not the individual-tone codec. No new amp/effect algorithm or parameter domain can be inferred from a name alone. |
| USB/audio timing changes | Driver-source PCM timing records a rational source clock of 78125/2; main flash and USB component are distinct investigations | Host file/export handling can preserve the exact clock. The installed USB-component version and firmware artifact remain unknown (#20); neither the main-flash image nor a rounded integer rate establishes a USB firmware fix. No clock, descriptor or stream change is proposed. |

The first candidate addresses transfer ordering, not a demonstrated exploitable
fault or proof that altered content will be accepted by the DSP. Internal
preset/program checksums and later loading semantics remain separate from outer
packet framing. Firmware compatibility cannot be established by making a sender
and a newly written receiver agree with each other.

## Architecture and space constraints

The Variax load records use three-byte words and separate numeric memory spaces.
Nominal load destinations and entry words are not physical flash offsets.
Instructions can cross record boundaries; contiguous program ranges must be
assembled before static decoding. DSP56300 compatibility does not identify the
exact IC, external buses, available free RAM or cycle budget.

The boot region contains two distinct candidate loader streams, padding and a
reset path. A jump toward ROM address 0xFF0000 does not supply the missing ROM
implementation or establish a usable external recovery mode. The existing
simulation assumes a selected application/boot receive mode and never enters it
on a device. It explicitly rejects recovery.

The POD image mixes program/data sections, named resources, opaque payloads and
FF regions. FF padding and all-FF regions are not evidence of free executable
space, spare physical sectors or permission to repurpose them. Load addresses,
word-indexed resource pointers and file offsets are different namespaces. A
resource edit may affect alignment, counts, pointers and multiple integrity
layers; a working host tone file does not establish that firmware editing works.

The software model does not emulate peripheral polling, flash erase/program,
power transitions, timing, DSP audio or the full host-update sequence. These are
material gaps for changes in either firmware, not details a checksum can resolve.

## Model and calibration compatibility

The exact original Variax 300/600 product identity is `0x00070004`, distinct from
the 500/700 family identity. A same-numbered release for another product is not
an interchangeable image. HD/JTV, bass and acoustic families require independent
format, hardware and model evidence. POD main flash `0x0003000A` is likewise
not a USB-component image or proof of compatibility with other POD products.

Known global Variax string levels occupy a separate six-value structure. Commands
0x54/0x56/0x57 and the static persistence branch are mapped in
[factory extraction](factory-extraction.md). The archived fallback table is generic,
not the owner's calibration, and the known values are not proven to exhaust
unit-specific data. Bus address 0xD04000 is not an approved physical flash offset.

A modified preset program must not be confused with global levels or a complete
bank. Firmware defaults, host model definitions and current-unit contents are
separate references. No complete current bank/DSP/calibration backup is available.
A future preservation assessment must identify every affected persistent region,
its unit-specific content, erase overlap and restore semantics before any write
proposal. Nothing in this assessment supplies substitute calibration or a reset.

## Integrity and recovery implications

The L6FF embedded MD5 and target/version declarations detect selected corruption
and mismatches; they do not establish authenticity, hardware compatibility or
preservation. SHA-256 pins the exact archived bytes used in research. Recomputing
an integrity value would establish neither safety nor authenticity, so this task
adds no repacker or modified firmware output.

Additional observed checks are the Variax slot's modulo-24-bit sum, the code
transfer's additive 24-bit checksum, the boot transfer's additive byte checksum,
and POD regions 4/5's additive 32-bit sums of big-endian 16-bit values. Coverage
and byte/word order matter. Compensating changes can preserve additive checksums;
the simulator tests one such collision. Correct checksums are necessary only for
the specific checks identified, never a general approval to execute new bytes.

A simulated timeout or power loss retains prior programming/erase intent. It
does not roll back flash. Main/boot receive buffering does not make their later
programming helpers atomic, and malformed preset termination need not precede
all effects. A successful protocol reply does not demonstrate durable contents.
No known ROM path, second bank, fallback image or boot guard has been validated
as a recovery mechanism on this hardware.

## Decision and remaining gates

Proceed with host-side correctness, file comparison and bounded offline research.
Keep firmware modification as a research target. A deployable device change is
not ready to implement: full CPU/peripheral and flash maps, original receiver
semantics, model/calibration compatibility, complete backups and demonstrable
recovery are missing. No estimated improvement in audio or reliability is justified
without an independent reference and appropriate behavior tests.

Further offline research can narrow a specific candidate, trace its dependencies
and test it in a suitable independently checked emulator when available. The
late synthetic matrix (#33) still needs sustain/decay, continuous bends/vibrato,
bends during decay and alternate/custom tuning combinations with meaningful
independent audio evidence. A protocol-only model cannot satisfy those audio gates.

The readiness review (#22), backup scope (#23), any future authorized read-only
integration (#24), and persistent-operation design (#25) retain these unknowns.
They may conclude that a physical operation remains unjustified. Neither issue
closure nor passing tests authorizes connection, queries, installation or an
unlock. The shared maintenance UI (#30) must present unsupported operations as
unsupported, rather than turning this feasibility report into an updater.

## Validation of this assessment

This is a documentation/research change. It introduces no firmware modifications
or executable feature. Findings were checked against the exact source-bound maps,
recovered host/receiver branches, archived slot extraction and the simulator's
explicit unsupported behavior. The existing 46-case host oracle replay and
16-test simulation set establish their recorded bounded scopes, not this report's
hypotheses. Primary archive fingerprints were rechecked locally. No hardware
was enumerated, opened, read, written, probed, reset or flashed.
