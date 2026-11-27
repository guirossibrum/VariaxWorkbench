# macOS PODxt Live USB readiness (#298)

## Owner evidence

The November 27 instrumented Workbench build was based on `99a62f37`. The
owner saw the PODxt Live in the Mac registry and Workbench showed it at
`1-1` with identity unverified and interfaces unavailable. After replug,
Workbench briefly displayed presence unknown with a registry error at
`0x0`. Device Check then recognized the PODxt at `1-1` and stopped at
identification. The trace records one interface-1 opener invocation. Its
active configuration descriptor lookup returned `Entity not found` before
the application opened the device or sent a MIDI identity question. The
trace supplied no fresh USB configuration value. The displayed route alone
does not link the observations to one registry generation across replug or
establish a direct-versus-hub comparison.
[Owner report](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7217046654)

An earlier owner report from the same base recorded 35 passing Rig Check
steps across the run and continuation, two firmware-related skips, four
unavailable audio checks, and verified starting buffers and release in
6.2. Workbench independently identified PODxt Live firmware 3.01 and read
128 tones. Its Variax identity timed out after the interface opened.
The November 27 failure precedes that stage. The Variax identity/reconnect
problem after a successful USB open belongs to #201.
[Earlier owner report](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7208605901)

In #274, the first Rig Check run also saw USB presence and later failed at
the active descriptor lookup. The separate read-only diagnostic observed
advertised configuration 1 and a fresh GET_CONFIGURATION value of 0 on its
then-current attachment. A separately approved later test confirmed a
fresh 0 and published interfaces 0 and 1 after activating configuration 1
once. The owner accepted the milestone after a run on `f13eaf7` verified
both identities and full backups (299 PODxt entries and 60 Variax programs).
Final rig state was not verified in that run. The measurements apply to
their own attachments and times.
[#274 first failure](https://github.com/guirossibrum/VariaxWorkbench/issues/274) ·
[Read-only result](https://github.com/guirossibrum/VariaxWorkbench/issues/274#issuecomment-7192957862) ·
[Activation result](https://github.com/guirossibrum/VariaxWorkbench/issues/274#issuecomment-7193758024) ·
[Owner acceptance](https://github.com/guirossibrum/VariaxWorkbench/issues/274#issuecomment-7196696377)

## Where the paths diverge

| Entry point | USB observation and attachment selection | First readiness check |
| --- | --- | --- |
| Workbench automatic detection | `MacUsbPresence` lists IOKit device services through `macos_inventory::inventory`. A PODxt row stays visible without an open candidate. A candidate needs an address, a unique location/route, the same registry generation and publication of both MIDI interfaces. | The identification worker rechecks the candidate under the rig lock before each open. Initial discovery uses interface 0 for PODxt identity and then interface 1 for Variax identity. The shared opener checks the descriptor before each device open. |
| Workbench Device Check | G1 calls `macos::attachments` under the rig lock. This uses libusb and records the route and address of the sole matching PODxt; it does not require an IOKit generation or published interface pair. | G2 obtains the lock and matches the saved path and address in libusb again for interface 1. Descriptor rejection stops the exchange. The UI includes the raw open detail. |
| Rig Check | 0.2 reads the passive IOKit inventory. 1.1 uses that view for presence. On the first open, Rig Check re-enumerates under the lock, compares the generation with the 0.2 snapshot when available, and correlates the candidate with libusb. | 1.2 opens and releases interface 1 without MIDI, and 2.1 uses interface 0 for identity. Each actual open rechecks the selected candidate, then uses the same descriptor gate. Probe reporting can therefore pass presence while a later interface step fails. |

[Workbench inventory and detection](../../src/app/detection/mod.rs) ·
[Workbench opener](../../src/app/detection/identity.rs) ·
[Device Check](../../src/device_probe.rs) ·
[Device Check exchange](../../src/usb.rs) ·
[Rig Check steps](../../src/rig_check/connection.rs) ·
[Rig Check attachment selection](../../src/rig_check/port.rs) ·
[Shared candidate rules](../../crates/line6-devices/src/transport/usb/macos_inventory.rs)

Two independent checks explain the displayed symptoms.

- **Registry query.** The adapter combines the IOKit return status and
  iterator handle into one error. The message containing `0x0` identifies
  its success-status/null-handle branch. That supplies no completed scan.
  A valid iterator which yields no matching devices supplies a successful
  empty scan. Workbench preserves the existing tab as presence unknown
  when enumeration fails. Interfaces unavailable is a different message
  used when a seen device lacks an eligible candidate. Missing route or
  address and an ambiguous attachment can also make that candidate
  unavailable. Publication of both children alone cannot establish access.
  [Registry adapter](../../crates/macos-usb-inventory/src/lib.rs) ·
  [Workbench detection](../../src/app/detection/mod.rs)
- **Active descriptor.** The shared USB opener first selects exactly one
  libusb device by product, physical route and enumeration address. On
  macOS it requires active descriptor configuration 1 with the requested
  vendor-class interface, altsetting and interrupt endpoints, including
  packet size and interval, before calling `device.open()`. In Darwin
  libusb, the lookup can return `NotFound` when its cached active value is
  0 or when the cached value has no matching configuration index. Descriptor
  retrieval can also propagate a lookup error. The cache is initially
  inferred from interface enumeration for single-configuration devices.
  Even libusb's Darwin `get_configuration` returns the cached value.
  The November 27 failure establishes unavailable descriptor data for the
  matched device at that moment. The live configuration value, driver
  ownership, and cause of the readiness failure remain unknown.
  [USB opener](../../crates/line6-devices/src/transport/usb.rs) ·
  [Descriptor guard](../../crates/line6-devices/src/transport/usb/podxt.rs) ·
  [Vendored Darwin backend](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c)

## Proposed change and offline cases

Keep every transport guard in place. The smallest fix is to split the
iterator outcome in `crates/macos-usb-inventory/src/lib.rs` and report
“macOS USB registry returned no iterator” for success with a null handle.
Keep the unavailable scan result and existing Workbench state handling.
A real nonzero IOKit failure keeps its status, and a valid empty iterator
keeps its completed-empty result. This improves the inventory diagnosis
without touching the descriptor or open path.

The active-descriptor stop already prevents MIDI. If the Gatekeeper wants
consistent wording in the two apps, scope a separate diagnostic change to
`crates/line6-devices/src/transport/usb/podxt.rs` and
`crates/line6-devices/src/transport/usb/podxt/tests.rs`, showing that the active
USB descriptor is unavailable and retaining the OS error. A regression
that counts calls before the device-open boundary would also touch
`crates/line6-devices/src/transport/usb.rs`. Coordinate all those paths with
Lane 1 before editing. No detection or Device Check change is needed for
the iterator fix. Aligning Device Check with IOKit generation matching
would be a separate change in `src/device_probe.rs` and `src/usb.rs`.

| Offline case | Expected observation and stopping point | Existing or proposed coverage |
| --- | --- | --- |
| Success with null iterator; nonzero status; success with valid iterator and no matches | Unavailable with missing-iterator detail; unavailable with status; completed empty result, respectively. None opens a device. | Add adapter fixtures with an injected query outcome when implementing the iterator split. Workbench's unreadable/ambiguous snapshot test and the Rig Check computer report test already exercise failed-versus-valid application state. |
| Direct route and populated hub, with missing and published interfaces | Both PODxt rows stay visible. The incomplete row has no candidate. An unrelated peripheral or its address change never redirects selection. | Existing `macos_inventory` direct/hub and neighbor-hotplug fixtures and Workbench's passive-inventory test. |
| Same route/address after replacement, or ambiguous location | Passive recheck refuses the stale or ambiguous candidate; exact libusb matching refuses missing/ambiguous devices. Device Check's path/address alone cannot distinguish route/address reuse. | Existing `macos_inventory`, `transport::usb`, Workbench replacement and Rig Check attachment selection tests. |
| Matched attachment with active descriptor `NotFound`, other read error, or wrong profile | Report the descriptor failure, preserve presence, and stop before open, claim or send. Require configuration 1 and the requested interface profile on the success case. | The existing Device Check open-refusal fake asserts no exchange. Add descriptor-specific injected results and open-call counts when changing that shared path. An open-refusal fake alone does not exercise Darwin descriptor lookup. |
| Valid descriptor followed by a denied/busy open; successful open followed by no Variax reply | Report the stage that failed. Count no exchange on open refusal; preserve a separate post-open identity outcome. | Existing Device Check and Rig Check fake refusal paths, and separate Workbench silent-guitar fixture. |

Three temporary Linux fixtures exercised the current registry adapter with
substituted IOKit calls. They reproduced the zero-status error, kept a
nonzero status, and confirmed that a valid empty iterator is consumed and
released. These fixtures establish the adapter's behavior for injected
inputs. Four more Linux fixtures compiled the descriptor guard and open
boundary with the Mac branch selected and fake libusb calls. Missing or
other descriptor errors and profile mismatches stopped with zero open
calls. The matching profile reached the injected open refusal once.
These fixtures do not exercise native IOKit or Darwin libusb. The Mac
runtime cause of either symptom remains unknown.

## Decision for Owner Inbox

If fresh configuration evidence is required, request approval for one
owner-run diagnostic on a still-reproducible failure. Record whether the rig
is direct or on a populated hub and correlate registry generation, route,
address, published interfaces and cached descriptor result on the same
attachment. A fresh GET_CONFIGURATION would require its own explicit
approval and reviewed procedure. The prior one-request diagnostic used an
exact selection under the rig lock, a non-seizing open, one read and close.
A refused open or changed selection ends that attempt. Configuration
activation, recovery policy and repeat runs each remain separate decisions.
No setting, driver change, probe run or retry is part of this checkpoint.
Direct-versus-hub native comparison and Intel/Catalina runtime coverage
remain open. [Prior connection design](../podxt-macos-connection-design.md)
