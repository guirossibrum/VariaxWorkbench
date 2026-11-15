# Work plan and hardware safety boundary

## Critical requirement

The PODxt Live and Variax 600 are irreplaceable legacy equipment. The user has
instructed us not to attempt device writes until practical offline analysis,
simulation, and validation avenues have been exhausted. Future hardware work
requires explicit user authorization. Firmware availability is not permission
to install it, and confirming a newer version is not permission to update.

**Live traffic is currently locked, including read probes.** Read queries still
transmit USB messages and session-enable commands. The Rust transport rejects
before opening a device; the Python probe is locked; the GUI's live action is
disabled. There is no runtime override. Do not remove these locks merely to run
tests. Continue from the already captured data while USB is disconnected.

Before this boundary was requested, the completed probes sent only identity,
current-preset metadata queries, and temporary MIDI-session enable/disable
messages. They did not switch presets, install firmware, write flash, or upload
preset changes. The current preset “Reso-5” was captured and decoded independently
with Rust and original Java; the resulting model/settings summaries match.
This is **one current-preset settings capture, not a complete guitar backup**.
An experimental broader DSP read timed out and did not produce a DSP backup.

## Firmware acquisition and factory reference

- [x] Download and preserve the exact official Variax 300/600 firmware **3.10**
  artifact, with source URL, product applicability, size and SHA-256.
  Official release listing: release ID **1062**, January 6, 2006, estimated 121 KB.
  User supplied `Variax300_3.10.vxf`; 124,208 bytes and MD5 match the public
  300/600 catalog. SHA-256:
  `e9f471e987b04cf31321e8a0da068f44abcdff2edbb32368385c08f92b999c1a`.
  Archived locally without execution. Original download URL was not recorded.
- [x] Download and preserve the latest official **PODxt Live** firmware artifact
  and record the same provenance. Verify product specificity; exclude PODxt,
  PODxt Pro, Bass PODxt Live, and later POD models unless documentation explicitly
  establishes a shared compatible image.
  User supplied `PODxtLive_3_01.xtf`; declared product `0x0003000a`, revision
  3.01; bounded zlib expansion and embedded image MD5 validated offline.
  SHA-256: `738f72e95f72e0bf4e52c4973ca86f098fae54a5d68e1c7b46112ad261ff07f5`.
- [ ] Investigate offline firmware acquisition through legacy Monkey and newer
  Line 6 Central, without connecting either updater to the hardware.
- [ ] Inspect updater packages, metadata/catalogs and downloaded firmware files
  offline; identify containers, signatures, checksums, compression, encoding and
  version/product metadata before interpreting payloads.
- [ ] Determine whether firmware contains factory presets and extract a
  non-mutating reference where possible. Alternatively locate an authenticated
  original Variax 300/600 Workbench factory bundle with reliable provenance.
- [ ] Distinguish factory defaults for a particular firmware version from the
  guitar's exact original shipping state, which may have used another version.
- [ ] Compare any trustworthy factory reference against captured settings and,
  only after a separately authorized future capture, the complete stored bank.
  Report unknowns; the installer’s “Default Patch” is not a full factory bank.

## Version status — verification only

- [x] Observe connected guitar identity: Variax 300/600 product ID `0x00070004`,
  ASCII firmware revision `0310` = **3.10**.
- [x] Observe PODxt Live identity: product ID `0x0003000a`, firmware **3.01**.
- [x] Compare these observed versions with the latest applicable official
  Variax 300/600 and PODxt Live firmware releases and their release notes.
- [x] Record whether each device is current, outdated, or unverified. Do not
  convert this task into a firmware-update action.
  Both observed versions match the newest applicable official listings:
  [PODxt Live 3.01](https://line6.com/software/index.html?hardware=PODxt%20Live&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show)
  (April 20, 2006, release 1224), and
  [Variax 300/600 3.10](https://line6.com/software/index.html?hardware=Variax%20300%2F600&name=Flash%20Memory&os=All&submit_form=set&submit_query=Show)
  (January 6, 2006, release 1062). No update is indicated by those listings.

## Firmware reverse engineering and improvement research

- [ ] Map the firmware CPU/DSP architecture, executable/data regions, vectors,
  loader/update protocol and integrity checks using static evidence.
- [ ] Recover factory model/preset and calibration data layouts where possible.
- [ ] Build offline parsers and simulated update/protocol fixtures that cannot
  contact hardware; include corrupt/truncated/wrong-model inputs.
- [ ] Assess whether firmware improvements are technically feasible. Record
  candidate improvements, constraints, and evidence; do not generate or flash
  experimental firmware as part of this research phase.
- [ ] Identify recovery/bootloader behavior from documentation and static code,
  preserving uncertainty. Do not deliberately put either device into recovery
  mode to investigate it.

## Rust application and offline verification

- [x] Extract and inventory Workbench/Monkey packages and original UI artwork.
- [x] Build REA and establish Ghidra native analysis with saved evidence.
- [x] Parse/edit individual `.vwm` files losslessly and compare with original
  Java reader/writer.
- [x] Implement offline GUI with recovered images and preset controls.
- [x] Verify native DSP packing and resonator conversion with original machine
  code in an isolated CPU emulator.
- [x] Verify all 43 pickup-definition layouts with original native serialization.
- [x] Finish body-definition parsing and compare all 28 assets against original
  native serialization.
- [ ] Complete and validate the coefficient engine without hardware, using
  native reference outputs, varied models, boundary values and rounding tests.
- [x] Parse L6BA/L6CO bank/collection files, extract presets, preserve bytes and
  validate both formats against original Java. Embedded replacement has unit
  coverage; GUI bank browsing and exhaustive bank variants remain future work.
- [x] Improve GUI behavior and test undo/redo, interrupted operations, copy saves,
  missing assets and dirty-document handling through offline interaction tests.
- [x] Build a fake transport covering fragmentation, realtime MIDI bytes,
  timeouts, cancellation, disconnects, wrong identity/model and cleanup failures.
- [x] Review every output opcode and prohibit preset/flash/firmware writes from
  the offline build. A future write-capable implementation needs its own review.
- [x] Keep a concrete validation ledger of what passed, what remains unknown,
  and which future hardware operations (if any) would still be necessary.

## Future hardware phase — blocked by design

- [ ] Finish the offline validation ledger and review it with the user.
- [ ] Obtain explicit authorization for a precisely defined hardware operation.
- [ ] Establish comprehensive verified backups and a documented recovery path
  before considering persistent changes. Current captures are insufficient.
- [ ] Consider staged read-only integration first. Treat firmware flashing as
  a separate decision, never an automatic consequence of version checking.

Current offline ledger: [consolidated report](docs/report.md). The full coefficient
engine, authenticated firmware artifacts, factory bank and recovery simulation
remain incomplete; this phase does not authorize live access.

Coefficient-engine implementation sequence and acceptance criteria:
[coefficient-engine-plan.md](docs/coefficient-engine-plan.md). Additional live
settings captures are not needed for its next offline steps.

New offline milestone: [generator and POD download results](docs/offline-generator-results.md).
Both firmware containers unpack and validate; exact region meanings/factory bank
recovery remain pending. Original complete native generation and JNI mapping now
execute offline with explicit runtime adapters; this is reference generation,
not a completed Rust coefficient port. Windows/macOS drivers were statically
extracted and inventoried, not installed.

- [x] Run the complete original native coefficient generator offline with all
  model definitions: 110 cases, 98 distinct complete outputs, checksums and
  fresh/warm repeatability passed. Four JNI mapping comparisons cover 53 fields.
  Host libm/Java-wrapper limitations remain documented; Rust parity is pending.
- [x] Unpack both L6FF firmware containers with bounded decompression, validate
  embedded MD5 and complete region coverage; retain all regions locally.
