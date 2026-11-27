# Product objective: one application for the connected guitar rig

The final objective is **one Rust application** for guitar editing, POD editing,
preset/library management, USB audio recording and device maintenance. The present
VariaxWorkbench repository and GUI are the starting point, not a commitment to a
guitar-only application. This is a target architecture and roadmap, not a claim
that these features already work.

## First complete rig

Target the user's Variax 600 → VDI → PODxt Live → USB → Linux setup:

- Guitar models, pickups, wiring, tuning and preset libraries.
- PODxt Live amp/cab/effect controls, presets and librarian functions.
- USB audio routing, recording into the app and interoperability with external
  recording software; processed/clean capture and monitoring where supported.
- Shared device information, applicable firmware versions, offline image
  inspection, backups and evidence-based recovery/maintenance workflows.
- A coordinated view of both devices without confusing the guitar and POD
  endpoints, model identities, data formats or firmware images.

Line 6's [GearBox recording guide](https://line6.com/media/support/toneporthelp/GearBox2_OnlineSupport_Recording_Rev03.pdf)
explicitly describes PODxt Live processing feeding Record Send 1-2 digitally over
USB (printed page 10), and processed versus unprocessed recording and re-amping
routing for the PODxt family (printed pages 21–22). The original USB driver
therefore supported audio use, not only preset/control traffic.

This confirms the historical hardware capability. It does **not** establish
current Linux capture, playback, VDI-source routing, rates, bit depths, buffering
or latency. Those must be recovered and validated. Investigate the existing Linux
audio driver/stack first; the downloaded Windows/macOS drivers are reference
material and cannot simply be loaded into Linux. Audio and control-interface
ownership must coexist without reset, unexpected routing changes or interference
with a DAW. Opening a real recording/playback stream is live hardware access and
is not authorized in the present phase. The offline GearBox and Linux-driver
record is [POD USB audio](pod-usb-audio.md).

The offline test recorder in [recording files](recording.md) provides explicit
input and route selection, integer levels, start/feed/stop/cancel,
non-overwriting saves, file integrity, and bounded buffers for synthetic and
file-backed streams. It does not open a live stream. Monitoring, processed/clean,
and re-amping behavior on a connected device remain unrecovered.

## Shared application, independent device implementations

Provide common Guitar, POD, Recording, Library and Maintenance workflows with a
shared device/session and capability registry. Each backend declares what it can
actually do, with exact supported model/firmware/transport combinations.

Keep codecs, coefficient engines, controls, artwork mappings, firmware containers,
update protocols and audio capabilities specific to their device families. A
shared screen must never imply preset interchangeability, firmware compatibility
or identical audio behavior. Unsupported operations should be explicit, with no
automatic fallback to another model or unsafe generic device operation.

Preserve the current source-level physical-access locks. Device discovery,
control, recording/playback and maintenance need independent operation policies
and simulation coverage. Firmware availability must not trigger an update;
entering a recording page must not open a stream automatically.

## Platform and device boundaries

The app and each family implementation build from the same Rust source on
Linux, macOS and Windows. The shared app owns editor and library workflows,
connection state, operation permissions and reporting. Family code owns model
data and capabilities, preset codecs, protocol messages, retry decisions and
the sequence of device operations. A family can reuse common operations when
the device evidence supports them.

Native connection adapters implement the common transport contract. They
discover and select endpoints, verify and open the intended attachment, move
bytes, expose native errors and release the endpoint. The application checks
whether the selected route can support an operation before offering it. A
missing native route can make an operation unavailable on a platform even
when all packages were built from the same commit.

When adding a device, register its model and capabilities with the shared app.
Reuse its family module if the verified protocol and formats fit. Add a family
module for a genuinely different protocol or data format and a native adapter
only where the available OS transport needs one. Test the same family workflow
with fake links across platforms and verify each native route separately on an
approved rig.

## Future families under the same roof

Future targets explicitly include:

- Workbench HD functionality and James Tyler Variax **JTV-69**.
- **POD HD500X**, including its editing, library and audio capabilities.
- Additional Variax, POD and related models through independently verified
  backend adapters and a published capability/support matrix.

Original Variax 600/Workbench and HD-generation behavior must not be presumed
compatible. First acquire the appropriate artifacts and product/version evidence,
then recover and test each family's formats, algorithms and protocols offline.
No future model is currently claimed as implemented or hardware-tested.

## Work tracking and safety

GitHub issues and the linked project remain the work-tracking source of truth.
New scope is recorded as distinct issues with native dependencies and project
ordering. Existing implementation continues sequentially; adding these targets
does not start multiple implementation issues or authorize any device operation.
The README summarizes current capabilities. The [status ledger](status.md)
records verified work, and the [roadmap](roadmap.md) records pending features
and unavailable behavior.

All current tests remain offline, with equipment disconnected. Recording, playback,
POD edits and firmware updates share the same risk boundary as guitar control.
Full device/audio emulation and successful offline tests cannot guarantee safety
on real equipment; any later connection or operation requires separate explicit
authorization after the relevant evidence, backups and recovery prerequisites.

## Longevity and self-contained operation

The user prioritizes long-term reliability as operating systems, external
libraries and vendor services change. Build a self-contained Rust core with
versioned coefficient mathematics, preset formats and protocol logic. Avoid
relying on host libm behavior for consistent cross-platform outputs. Evaluate
maintenance and portability of dependencies; pin the selected versions and keep
them replaceable. Research runtimes are comparison tools, not automatic product
dependencies. Normal use should work offline without Line 6 servers or external
applications. Keep OS USB/audio/desktop interfaces at narrow adapter boundaries;
those unavoidable platform interfaces must not own the coefficient algorithms.
Preserve independent synthetic/offline regressions and documented formats so a
future maintainer can rebuild and validate the core. This requirement does not
authorize publishing original assets or contacting hardware.
