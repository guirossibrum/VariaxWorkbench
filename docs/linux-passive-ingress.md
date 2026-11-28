# Shared Linux passive ingress for PODxt Live and Variax Follow

Static review of `405022afbf2f62d9090b59026a5facb79b35c12f` for the
[#275 follow-up](https://github.com/guirossibrum/VariaxWorkbench/issues/275#issuecomment-7231444392).
The app keeps its existing listeners. The exact read-only ALSA descriptor and
borrowed Variax USB interface have no joint native acceptance evidence.
The existing ALSA input also lacks the checked ownership and close API a
combined worker would need before allowing foreground device access.

## Existing ownership and handoff

| Boundary | Source evidence |
| --- | --- |
| PODxt input | Linux detection chooses the unique PODxt ALSA card, takes `PassiveDeviceLock`, checks the USB path/address and opens `RawMidiInput` read-only. The input owns a `File` without a lease borrow or a checked finish. Complete channel notices are carried by the detection worker under its parent attachment key. [Listener](../src/app/detection/identity.rs#L260) · [Raw input](../crates/line6-devices/src/transport/rawmidi.rs#L120) · [Notice decoding](../src/app/detection/notice.rs#L53) · [Application](../src/app/detection/mod.rs#L847) |
| Checked ALSA capture | The existing scoped capture verifies the USB, interface 0, ALSA card and MIDI node identities, checks the opened descriptor, and closes that descriptor exactly once under a borrow of `DeviceLock`. It opens with read and write access for backup capture. Reusing the verifier and checked close for the same ALSA route requires a read-only descriptor API. [Capture adapter](../crates/line6-devices/src/transport/rawmidi/attachment.rs#L151) · [Open flags](../crates/line6-devices/src/transport/rawmidi.rs#L73) · [Attachment verification](../crates/line6-devices/src/transport/rawmidi/attachment/inventory.rs#L82) |
| Variax input | The editor's verified attachment includes the full device key. Its source borrows `FollowDeviceLock::rig()` to open the selected USB path and address on interface 1, then rechecks the guitar identity and listens. Release attempts session off, relay off, USB release and checked kernel-driver return. The worker keeps the lease throughout. [Editor attachment](../src/families/variax/editor/live_attachment.rs#L3) · [Follow source](../src/families/variax/editor/follow.rs#L608) · [USB interface specification](../crates/line6-devices/src/family/variax/transport.rs#L12) · [Protocol cleanup](../crates/line6-devices/src/family/variax.rs#L298) · [Relay cleanup](../crates/line6-devices/src/family/variax.rs#L447) · [Driver return](../crates/line6-devices/src/transport/usb.rs#L225) |
| Contention and failed release | Both leases compete for the same rig file. A foreground request takes the active-intent lock first. The `FollowDeviceLock` failure counter can reject a concurrent foreground request in the same process after the rig file becomes available. A later request snapshots the updated count, and another process has a separate counter. The existing counter therefore provides neither a persistent failure quarantine nor cross-process checked-release evidence. [Foreground acquisition](../crates/line6-devices/src/device_lock.rs#L237) · [Follow lease](../crates/line6-devices/src/device_lock.rs#L319) · [Passive lease](../crates/line6-devices/src/device_lock.rs#L400) |

The Linux interface split is statically established. Rig Check can retain
its ALSA link while claiming the guitar interface, but its Linux opens select
a card alias and a matching VID/PID independently and its ALSA link is cached
without checked finalization. The #201 notice probe also polls both input
sources under one lock and checks Variax release. It uses the read/write ALSA
link and its owner card records the combined native run as pending. Those
paths establish useful precedent for polling and separate endpoint
ownership. The exact editor pair and checked two-input handoff still need
evidence. [Linux driver split](linux-transport.md#usb-matching) ·
[Rig Check transport audit](rig-check/linux-macos-audit.md#selection-transfer-retry-and-release) ·
[Probe ownership and pump](../tools/variax-probe/src/notice_probe.rs#L135) ·
[#201 owner card](variax-notice-owner-test-card.md)

## Offline evidence and implementation boundary

- The linked-device test at this revision exercises the real lock in an
  isolated runtime directory and fake MIDI. Variax selector and knob input
  arrives while the lease is held. The second listener is refused, then a
  fresh PODxt channel notice is admitted under the passive lease. A separate
  detection fixture rejects a stale generation. [Linked and stale fixtures](../src/app/detection/tests.rs#L635)
- The #201 fake probe exercises two input sources in one bounded window.
  Device-crate tests exercise foreground intent, cancellation while waiting,
  Variax release before foreground acquisition, rejection on a failed
  Follow handoff, and ALSA descriptor replacement and checked-close errors.
  These tests exercise their current paths. They do not establish checked
  release of both editor inputs in a combined worker. [Probe fixtures](../tools/variax-probe/src/notice_probe/tests.rs) ·
  [Lease fixtures](../crates/line6-devices/src/device_lock.rs#L482) ·
  [ALSA fixtures](../crates/line6-devices/src/transport/rawmidi/attachment/tests.rs#L146)

A combined candidate can borrow one `FollowDeviceLock` and reuse the existing
transport and decoder for each route. It needs a descriptor-verified, checked
read-only ALSA adapter, a bounded polling loop, and the existing parent and
linked-guitar generation keys on their respective outputs. Revalidate the
parent before opening either input. Reject stale events after a replacement,
including a new guitar generation on the same PODxt. Route Variax selection
through the no-send editor path coordinated with #315 and PODxt programs
through the existing detection notice. A foreground request, cancellation,
replacement or failure must end polling, attempt both finalizers even if
one fails, and report that outcome before releasing the lease. Establish
the failure behavior beyond the current concurrent in-process handoff
before claiming that every foreground caller is covered.

## Smallest owner-run prerequisite

Review one bounded read-only probe on the verified Linux PODxt Live 3.01
and Variax 600 3.10. Use the #201 pump as a reference and bind the actual
read-only ALSA descriptor and borrowed interface 1 to the same expected
USB attachment under one existing rig lease. Use only the existing Follow
identity and relay/session setup and cleanup operations. During a single
overlap window, physically change a PODxt channel, move the Variax selector,
then move the guitar volume and tone knobs after settling. Record the source
and time for each event. Cancel with both inputs open. Record the checked
ALSA close, session and relay off results, and verified interface 1 driver
return before a foreground lock-only waiter reports acquisition. A failed
or missing cleanup result leaves this prerequisite open. Keep raw captures
local and report only event and cleanup summaries.

This is the proposed acceptance scope for a separately reviewed owner
procedure. No paired owner probe or production worker is delivered by this
checkpoint.
