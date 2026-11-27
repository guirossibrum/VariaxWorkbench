# PODxt Live and Chrome coexistence checkpoint (#298)

An owner-approved read found fresh configuration 0, no interface services
and no cached active descriptor on the matching PODxt Live. In the later
activation attempt, the reviewed `7e5a762` tool returned exclusive-access
status from its own registry preflight. It had reached neither
`USBDeviceOpen` nor the fresh configuration read. A direct Chrome
`AppleUSBHostDeviceUserClient` was visible on the same service, then absent
after Chrome quit without a replug. This identifies the old predicate that
stopped the attempt. That attempt left actual OS access unresolved. The
cause of configuration 0 remains unknown.
[Owner configuration read](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7217524214) ·
[Activation stop](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7217561238) ·
[Owner coexistence requirement](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7217625707)

## Owner activation result

On Apple silicon, the standalone probe opened the selected PODxt Live
with ordinary device access and read configuration 0 on that same handle.
macOS accepted one activation to configuration 1. The checked device close
succeeded, then interfaces 0 and 1 appeared on the same attachment
generation. The same shared Chrome device-user-client registration was
visible immediately before and after the activation.
[Owner run](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7218534429)

Active configuration descriptor availability, interface claims, normal
Workbench operation and guitar identity each need separate evidence.
The Gatekeeper matched the owner artifact and native build inputs to the
reviewed source and archived the standalone probe. Offline review passed
13 standalone Rust tests, three portable C fixture groups and all 14
integrated fake coexistence tests.
[Gatekeeper review](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7220765467)

## Later Workbench Build 468 observations

In a separate owner attempt, Build 468 reported a registry scan with no
iterator, then detected the PODxt Live on a subsequent scan.
[Build 468 readiness report](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7220707671)

After the owner physically unplugged and reconnected the PODxt Live USB
cable, Device Check step 1 passed on the reconnected attachment. Step 2
stopped at the active configuration descriptor lookup with “Entity not
found,” before opening the interface or sending a Variax identity request.
The earlier activation verified publication on its own attachment. The
cause of the post-replug failure, the fresh USB configuration value, and
any USB client involvement remain unknown.
[Owner clarification](https://github.com/guirossibrum/VariaxWorkbench/issues/298#issuecomment-7220941889)

The earlier Workbench startup orphan-byte observation is tracked under
[#201](https://github.com/guirossibrum/VariaxWorkbench/issues/201).

## Offline boundary

The [fake-only Rust harness](../../tools/podxt-macos-coexistence.rs) models a
replacement for the old direct-child rule. It keeps the attachment and
advertised-descriptor checks from the reviewed probe. A registered device
user client is recorded as advisory. A verified conflicting binding,
unreadable or inconsistent publication view, mismatched descriptor or changed
selection stops the path. A native adapter would have to validate the
registry sample, reject unknown conflicting services conservatively, and
distinguish direct user clients from actual bindings on the selected device
and its relevant interfaces. The fake tests exercise that decision
boundary. They do not establish how native IOKit classifies a service.

- Under the rig lock, verify the registry generation, location, USB address,
  VID/PID and single advertised configuration. The indexed descriptor must
  advertise both expected vendor-class alt-1 interrupt interfaces and their
  exact endpoints. Check publication and actual bindings again before open,
  immediately after open and after the fresh read.
- Use ordinary `USBDeviceOpen` on the selected service. Preserve an actual
  exclusive/busy refusal and other native failures as separate open outcomes.
  On refusal there is no read, configuration call or handle to close.
- Make one bounded device-recipient GET_CONFIGURATION on that handle and
  require one returned byte, as in the reviewed probe. Fresh 1 gets no
  activation and requires both expected services and an active descriptor
  before reporting readiness. Fresh 0 with published interfaces is
  contradictory. Fresh 0 with neither interface and explicit activation
  approval permits one host-mediated
  `SetConfigurationV2(1, true, false)` on that same handle. Keep unsupported
  API, native refusal or changed evidence as a stop. The harness's approval
  is a test input. It grants no real approval.
- Close once and preserve both errors if the operation and close fail. After
  a successful close, a simulated accepted activation gets up to six
  registry observations 200 ms apart under the same lock. Report acceptance,
  observed interfaces and verified active descriptor separately. Partial
  publication, replacement or stale descriptor cannot advance to readiness.
  The harness has no USB adapter, interface claim, MIDI, retry or reset.

## Darwin active-descriptor refresh

The pinned backend's `darwin_get_active_config_descriptor` uses a cached
active value. The routine that rechecks configuration uses a native
interface iterator, and for a one-configuration device infers the active
value from the advertised configuration when it sees an interface. Its
regular Darwin `get_configuration` also returns the cache. Neither gives
a fresh USB GET_CONFIGURATION value. The separate host-mediated activation
has no path to update libusb's cached value directly.
[Active descriptor and configuration check](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L965) ·
[Cached getter and legacy setter](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1497)

- **Repeat on the global context.** `rusb::devices()` uses the once-initialized
  global context. On Darwin's hotplug path, libusb polls and copies the
  context's existing device list. Ordinary handle reopen calls
  `USBDeviceOpen` when needed and has no configuration recheck. Treat all
  of these as insufficient to prove a refresh on the same service.
  [rusb global context](../../third_party/usb/rusb-0.9.4/src/context.rs#L258) ·
  [libusb device listing](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/core.c#L822) ·
  [Darwin open](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1412)
- **Candidate refresh to review.** A new explicit `rusb::Context` calls
  `darwin_init_context` and then `darwin_scan_devices`. For an enumerable
  device that scan calls `process_new_device` even when the session already
  has a cache entry, which reruns `darwin_check_configuration`. The cache
  is process-wide, so the new context is not isolated. The scan is broader
  than the selected attachment and can touch shared cache state. Any
  candidate would run only in the separately approved native checkpoint,
  after close and interface publication, with no active libusb handle and
  the rig lock held. Match exactly one libusb PODxt by physical route and
  address, bracket the lookup with exact IOKit generation and service
  checks, then require a valid active configuration-1 descriptor in the
  refreshed context. A later product path would also have to verify the
  descriptor in the context it will actually use to claim. Missing data,
  context errors, ambiguity and any service transition stop the attempt.
  A targeted native metadata check could verify publication independently,
  but cannot by itself refresh the ordinary libusb opener. Avoid
  reset, re-enumeration, legacy SetConfiguration and all claim attempts
  as cache workarounds.
  [Darwin context init](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L891) ·
  [Shared cache lookup](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1187) ·
  [Configuration recheck](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1294) ·
  [Darwin scan](../../third_party/usb/libusb1-sys-0.7.0/libusb/libusb/os/darwin_usb.c#L1386)

This is a source-derived refresh candidate. Its locking, scope, and
interaction with other live contexts need separate review and native
evidence. The harness injects the result of active-descriptor revalidation
and checks that failure stops readiness.

## Interpreting observations with Chrome open

| Observation on the matching attachment | What it establishes |
| --- | --- |
| Chrome running, direct client visible, ordinary device open succeeds | The visible client coexisted with that device open. Chrome running with no direct client covers a different case. |
| Ordinary device open refuses access | The native phase and status establish refusal on that attempt. A registry client entry cannot attribute the refusal. |
| Fresh 1, close succeeds, both services and a valid active descriptor observed | Configured device readiness reached without a configuration write. Individual interface claims and normal Workbench identity require their own checks. |
| Fresh 0, separately approved activation accepts, clean close, both services observed | The host activated and published interfaces on that attempt with Chrome open. Matching active descriptor after refresh, recorded separately, establishes whether the intended reopen path has current metadata. |
| Activation accepts but publication or descriptor validation fails | The failed phase still blocks ordinary claims. Keep the activation outcome and stop without another configuration call. |

Record exact attachment continuity, OS and architecture,
direct-versus-hub topology, whether the client was visible, native phase and
status, one fresh read result, whether the host call ran, close result and
the same-service publication and descriptor observations. Retain fine-grained
evidence privately. Configured Workbench and Rig Check runtime parity, direct
versus populated-hub coverage, and Intel/Catalina runtime acceptance remain
open under [#298](https://github.com/guirossibrum/VariaxWorkbench/issues/298).

[Existing recovery design](../podxt-macos-connection-design.md#recovery-when-unconfigured)
