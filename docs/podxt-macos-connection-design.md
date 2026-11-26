# Shared PODxt Live connection on macOS

Design for [#274](https://github.com/guirossibrum/VariaxWorkbench/issues/274),
reviewed against production commit `a420df4`. Direct macOS PODxt Live USB
support for Workbench and Rig Check was merged in
[d2a2229](https://github.com/guirossibrum/VariaxWorkbench/commit/d2a22293fa11be764aa4b0bab52abbc46a9d8ec4).
The recovery proposed here requires separate implementation and device
review. The scope includes the
[design checkpoint](https://github.com/guirossibrum/VariaxWorkbench/issues/274#issuecomment-7193813576),
[non-conflicting clients](https://github.com/guirossibrum/VariaxWorkbench/issues/274#issuecomment-7193850150),
and [populated hubs](https://github.com/guirossibrum/VariaxWorkbench/issues/274#issuecomment-7193877940).

## Evidence and its limits

Rig Check previously listed the PODxt Live in inventory and failed at active
configuration descriptor validation before sending MIDI. The owner reports
that the first activation preflight refused on a foreign IOKit user-client
entry. After Chrome quit, a saved registry snapshot still showed the matched
device with an advertised configuration and no interface children. The
subsequent approved Gatekeeper run confirmed the same attachment, read active
configuration 0, and issued one macOS configuration-1 activation. macOS
accepted it and published interfaces 0 and 1. The run ended without an
interface claim or MIDI.

The earlier preflight refusal records client presence. An OS exclusive-open
refusal was never established in that run. The sequence leaves the cause of
configuration 0 open. The successful activation establishes interface
publication for that attachment on that run. Reopen, firmware identity,
persistence across sleep or reconnect, Intel runtime, and application parity
remain to be verified. The raw evidence stays private.

## Discovery and attachment identity

Give both apps a shared macOS inventory sourced from IOKit device services.
Select PODxt Live by VID/PID `0e41:4650` at device level, independent of its
configuration, published interfaces, descriptor access, or ability to open.
Workbench shows a detected-device card even when live actions are unavailable.
Rig Check inventory reports the same detected device and leaves the actual
access outcome to the connection step. Enumerate direct ports and every
populated hub branch. Filter unrelated peripherals out before evaluating
configuration, ownership or access. The discovery adapter should require
registry metadata only. The current libusb scanner filters entries it could
cache and may fetch a device descriptor to populate that cache, so its results
alone cannot satisfy the inventory requirement.

Correlate the chosen IOKit service with the libusb device by exact VID/PID,
full location ID and bus/port chain, enumeration address, and service entry
ID or equivalent backend session identity tied to that service. Expose the
correlation through the shared Mac transport. The existing portable
`path,address` pair alone cannot rule out an address reused after a quick
replug. Treat missing correlation and ambiguous results as detected but
unavailable for opening. When no attachment has been selected, require a
single exact candidate under the lock or an explicit selection. When there is
a selection, reject changes and never substitute a neighbor, another PODxt,
or a later attachment at the old address. Invalidate live tokens and pending
results on target removal, replacement, wake, or inconsistent metadata.
Unrelated neighbor changes leave a still-matching token usable.

Current source: [Mac attachment scan](../crates/line6-devices/src/transport/usb/macos.rs),
[Darwin device descriptor cache and scan](../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1072),
[Workbench attachment reconciliation](../src/app/detection/mod.rs),
[Rig Check inventory and direct port](../src/rig_check/port.rs).

## Direct connection when configured

1. Run each action through its existing operation gate. Obtain the existing
   rig lock before opening and keep it through the operation, interface
   transitions and cleanup. Revalidate the selected attachment against
   fresh metadata under the lock before every open or reopen.
2. Verify active configuration 1 and the published interfaces required by
   the requested operation. Keep the descriptor, alternate-setting and
   interrupt-endpoint checks (interface 0 for PODxt MIDI and interface 1 for
   the Variax relay). An indexed descriptor establishes what the device
   advertises. A safe claim requires an active configuration and the matching
   live interface. A missing active descriptor is an unknown active state
   until independently verified.
3. Use the ordinary non-seizing device open and interface claim. Keep driver
   protection on the required interfaces of the matched device. Browser
   processes and visible user clients are advisory observations. With no
   actual OS refusal or verified conflicting binding, the connection must
   succeed with Chrome running normally or a visible non-conflicting client.
   The current Darwin kernel-driver heuristic treats any interface child as a
   driver; distinguish a verified conflicting binding from child presence
   before using that result to refuse. Preserve native error and phase
   information. Stop for an actual exclusive-access/busy refusal or a verified
   conflicting binding. Treat other access errors according to their own codes.
   Darwin libusb can reuse an in-process device open. The rig lock and
   finish-before-reopen rule must keep that reuse from masking a refusal.
4. Open one interface at a time, explicitly finish and drop the handle
   before reopening the other under the same lock. Stop on failed cleanup.
   Verify the PODxt Live through the existing identity exchange (tested
   firmware 3.01). Verify the Variax through the existing relay identity
   exchange when requested (tested firmware 3.10), with only the resend already
   allowed by that procedure. Keep offline editing available and apply the
   existing firmware admission policy when identities differ. No Variax live
   tab on silence.

Share the attachment and admission logic in the device crate. Workbench calls
it from automatic identity and Device Check's staged operations. Rig Check
calls it from the existing locked direct port, preserving consent and step
order. Both use the same disposition for inventory, access refusal and
configuration unavailable. The Linux and no-USB paths retain their existing
behavior.

Current source: [Workbench identity workers](../src/app/detection/identity.rs),
[Device Check](../src/device_probe.rs), [Rig Check direct port](../src/rig_check/port.rs),
[rig lock](../crates/line6-devices/src/device_lock.rs),
[USB opener](../crates/line6-devices/src/transport/usb.rs),
[PODxt descriptor verification](../crates/line6-devices/src/transport/usb/podxt.rs),
[Darwin device open](../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1411),
[interface claim](../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1649),
[driver heuristic](../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L2045),
[error mapping](../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L328).

## Recovery when unconfigured

Recommend an explicit in-app action such as **Configure PODxt Live on this
Mac…**. Offer it in both apps when the exact device is detected and the
interfaces or active descriptor are unavailable. Explain that macOS will
attempt to activate the USB configuration and leave the outcome in place.
Default to No. No separate Terminal setup or instruction to quit Chrome.
Generic Detect, Refresh guitar, Run, reconnect and wake do not give this
consent.

- After a separately approved operation policy and consent, hold the existing
  rig lock and match the exact attachment. Validate advertised configuration
  1 and the expected interface profile using the indexed descriptor. Check
  the matched device and relevant interface services for conflicting
  ownership. A visible user client alone cannot refuse this path. Require
  the ordinary non-seizing device open; stop for conflict, refused access,
  changed attachment, or an inconsistent interface view.
- On that handle make one bounded device-recipient GET_CONFIGURATION and
  require an exact one-byte result. Fresh 0 and absence of the expected
  interface services permit the next check. On fresh 1, close the
  maintenance handle and proceed to normal active validation with no write.
  An unsupported value, contradictory evidence,
  or read failure stops without a configuration attempt.
- Recheck the attachment before the write. Use the host-mediated
  `IOUSBDeviceInterface650::SetConfigurationV2(1, true, false)` once, with
  interface matching enabled and remote wakeup disabled. If unsupported,
  refused, or busy, stop. No retry loop, raw fallback, reset, driver detach,
  seize, interface claim, MIDI or automatic restoration in this action.
- Close the device handle, then observe same-service interface publication
  with the rig lock held. Proposal: six registry-only observations spaced
  200 ms apart, stopping sooner on mismatch, failure or both interfaces.
  Report configuration acceptance and interface publication separately.
  Leave the result in place even if publication is inconclusive.
- Validate active configuration and the expected services again on the same
  attachment before the regular opener claims anything. Darwin libusb
  caches the active configuration and the separate host activation does
  not update that cache. Design a same-service cache refresh or a verified
  native active-metadata path before enabling reopen. Simply reopening a
  libusb handle may reuse stale state. Fail closed if it cannot be refreshed.
  Once validated, run normal identity through the existing gates. Rig Check
  starts a fresh connection attempt; a failed or interrupted run does not
  silently resume previously consented steps.

Apple SDK reference: `IOKit/usb/IOUSBLib.h`, `IOUSBDeviceInterface650`
(`USBDeviceOpen` and `SetConfigurationV2`). Repository references:
[sanitized activation evidence](https://github.com/guirossibrum/VariaxWorkbench/issues/274#issuecomment-7193758024),
[vendored active descriptor lookup](../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L965),
[cached configuration and legacy setter](../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1497).
The prior diagnostic's client-presence preflight was specific to that
approved test.

A conditional recovery inside a foreground Connect action would save a
second interaction after reconnect, if the user had explicitly consented
to configuration as part of that action. An automatic background variant
would also change USB state during polling or wake. Either variant would
need separate authorization and an attempt bound shared across both apps
and keyed to the verified attachment generation. Without that bound the apps
can alternate attempts or repeat on each wake. The explicit in-app action
keeps detection and reconnect predictable while providing a recovery
available on each supported Mac.

## Diagnostics

| Observed outcome | Proposed presentation |
| --- | --- |
| Device service found, active configuration or interfaces unavailable | “PODxt Live detected. Its USB interfaces are unavailable.” Offer the explicit action where eligible. |
| Visible non-conflicting client with successful open and required claims | Continue normally. Client presence is optional local diagnostic detail. |
| Cooperative rig lock busy | “Another rig operation is in progress.” Defer or allow retry when it finishes. |
| Actual exclusive-access refusal during device open or interface claim | “macOS reports the PODxt Live is already in use” with the affected stage or interface. Ask the user to release the conflicting access if known. |
| Verified conflicting binding or other access denial | Name the verified binding where available or report the specific denied stage. |
| Attachment lost or changed | “The USB connection changed. Check again.” Keep the offline tab and inventory refresh available. |
| Activation refused or accepted with missing interfaces | Report the exact outcome. Refusal gives no conclusion about whether activation would publish the interfaces. |
| Identity is unverified | Give reported and tested versions, with offline editing available. |

Record the failed phase, whether configuration evidence was cached or
fresh, whether the selected attachment still matched, the native result,
affected interface, if any, and acceptance and publication as separate
observations. Keep raw registry identifiers and client details local.
Exported summaries need no other peripheral or process details.

## Verification before release

The fake tests below are proposed. Native runs need separate owner
authorization. Run the eventually approved acceptance cases in both apps
on Apple silicon and on the supported Intel/Catalina target. Verify actual
Mach-O slices and deployment before runtime verification.

| Scenario | Required result |
| --- | --- |
| Direct port and populated hub, including an unconfigured or busy PODxt | Both apps list the OS-enumerated PODxt. Inventory performs no interface claim. Access is reported separately. |
| Neighbor disconnect or address change on the hub, with another client active | The selected PODxt stays selected if unchanged. The neighbor is never opened or reported as PODxt contention. |
| Configured PODxt with Chrome running normally or a visible non-conflicting client on the PODxt | Non-seizing open and required interface claims succeed, with normal identity and zero configuration writes. No per-Mac Terminal step. |
| Foreground recovery with Chrome running normally or a visible non-conflicting client on the PODxt | Fresh 0 allows one activation, then verified publication and normal reopen. |
| Actual PODxt device-open refusal, interface-claim refusal, or verified binding | Report the exact phase and keep detection. Stop without seizing, detaching or attempting recovery on a known conflict. |
| Cooperative lock, synthetic timeout and failed cleanup | No overlap between apps or interface handles. Timeout/refusal closes the attempt; cleanup failure stops reopening. |
| Fresh 0, fresh 1, invalid or contradictory evidence, unsupported API | Only the fully verified fresh-0 case calls the host API, once. Refusal, partial publication and stale libusb cache cannot advance to a claim. |
| Unplug/replug, address reuse, or sleep/wake with configuration retained or lost | Invalidate stale results and revalidate the exact attachment. Retained config uses direct connect; lost config offers the in-app action. No automatic configuration attempt. |
| PODxt 3.01 and Variax 3.10, differing reported firmware, or a silent guitar | Existing identity gates and version notices hold. A quiet relay never yields a connected guitar tab. |

In fake coverage assert exact selection, call order, operation gate and
lock lifetime, zero writes on direct/refused paths, one write on the
eligible path, and configured verification before claim. On native runs
record whether each device or interface open was actually refused and what
ownership was observed. A visible client or browser process alone records
neither success nor contention.
