# PODxt Live direct USB MIDI

Research for [#274](https://github.com/guirossibrum/VariaxWorkbench/issues/274).
**Static fact** denotes what a saved descriptor, capture, source path or
disassembly directly establishes. **Inference** denotes an implementation
choice supported by those records. **Unknown** denotes an open hardware or OS
question. This pass used existing files and public source only.

## Interface and endpoints

**Static fact.** The saved PODxt Live descriptor (`0e41:4650`, `bcdDevice
0.01`) advertises configuration 1 with two vendor-specific interfaces.
The table records endpoint addresses from the host's perspective. Both
interrupt pairs have `wMaxPacketSize = 16` and `bInterval = 5`. The
`bcdDevice` field is the USB release field. The MIDI identity reports the
PODxt firmware separately. [Linux transport study](linux-transport.md)
[macOS driver study](driver-macos-podxt.md)

| Interface and alternate | OUT | IN | Type and use |
| --- | --- | --- | --- |
| 0, alt 0 | `01` | `82` | Isochronous, zero-byte packet limits. No MIDI endpoints. |
| 0, alt 1 | `03` | `84` | Interrupt, PODxt MIDI. |
| 0, alt 1 | `01` | `82` | Isochronous, audio. Limits are 240 bytes OUT and 252 bytes IN. |
| 1, alt 0 | — | — | No endpoints. |
| 1, alt 1 | `05` | `86` | Interrupt, Variax through the PODxt Live. |

**Static fact.** Linux binds `snd-usb-pod` to interface 0 for this product.
Its Live-specific row selects alt 1, identifies `03`/`84` as control MIDI,
and gives the separate audio pair to PCM. The generic probe selects the
alternate on the particular interface it matched. `snd-usb-variax` matches
interface 1 for the same USB product and selects alt 1 with `05`/`86`.
[Linux POD probe and model table](https://codebrowser.dev/linux/linux/sound/usb/line6/pod.c.html#418)
[Linux POD endpoint table](https://codebrowser.dev/linux/linux/sound/usb/line6/pod.c.html#488)
[Linux common probe](https://codebrowser.dev/linux/linux/sound/usb/line6/driver.c.html#783)
[Linux Variax table](https://codebrowser.dev/linux/linux/sound/usb/line6/variax.c.html#184)

**Static fact.** The legacy macOS kext matches interface 0, selects alt 1
for its MIDI service, then finds interrupt pipes by direction. The Windows
driver's Live profile registers both pairs and the separate isochronous
pair. These static traces agree with the descriptor map. They describe
different host ownership and setup paths. [macOS driver study](driver-macos-podxt.md)
[Windows driver study](driver-windows-podxt.md)

## MIDI framing

**Static fact.** The Linux sender passes MIDI stream bytes directly as
interrupt OUT data. The receive path feeds the actual byte count from the
interrupt IN completion into the driver's MIDI parser. Raw MIDI is delivered
to ALSA after parsing. The interrupt payload has no USB-MIDI class event
header, cable/CIN nibble or four-byte event padding in this path. The 16-byte
descriptor limit is for a USB packet. SysEx spans packets and uses MIDI
message boundaries. The Linux driver's 64-byte receive buffer is a host
read-buffer size. [Linux MIDI send path](https://codebrowser.dev/linux/linux/sound/usb/line6/midi.c.html#103)
[Linux receive and requeue path](https://codebrowser.dev/linux/linux/sound/usb/line6/driver.c.html#272)
[Linux receive-buffer declaration](https://codebrowser.dev/linux/linux/sound/usb/line6/driver.h.html#26)

**Static fact.** The Linux receive parser maps MIDI status `F2 → F0`,
`B2 → B0` and `C2 → C0` before interpreting Line 6 traffic. This correction
is in receive parsing, and the Linux send path uses normal outgoing MIDI
status bytes. The existing Device Check raw USB decoder already applies
these substitutions on its receive path. [Linux receive status correction](https://codebrowser.dev/linux/linux/sound/usb/line6/midibuf.c.html#143)
[Existing decoder](../src/usb.rs)

**Static fact.** The ignored owner artifacts include a standalone
`podxt-identity.syx` with a 17-byte assembled reply starting `F0 7E 7F 06 02`.
In the saved `probe-podxt-read-1795406765` session, the ALSA-side log shows
`F0 7E 7F 06 01 F7` followed by `F0 00 01 0C 03 75 F7` and a 168-byte
edit-buffer reply in `editbuffer.syx`. These files describe the MIDI
messages after the host transport. [Owner read scope](podxt-live-reads-test-card.md)

**Inference.** The direct link can retain the current `PodxtMidi` path:
send the existing request bytes as ordered interrupt OUT pieces of at most
16 bytes, receive each transfer's actual bytes, and use the existing
assembler across arbitrary fragment boundaries. Apply only the known receive
status correction at the POD USB boundary so the generic parser keeps its
ordinary MIDI semantics. Treat short writes as failed sends. The identity
request fits one OUT packet. A complete 17-byte identity reply spans at
least two USB packets.

**Unknown.** The saved POD MIDI captures lack physical interface-0 packet
boundaries and raw pre-normalization status values. They cannot establish
whether firmware 3.01 uses each alternate status, which transfers use short
packet boundaries, or whether a newly claimed interface has the same
first-reply loss seen on interface 1. The existing one-resend rule has an
observable orphan-tail condition.

## Startup and control requests

| Finding | Classification and evidence |
| --- | --- |
| Linux selects the interface alternate, initializes the MIDI buffers and queues interrupt IN before scheduling the first POD identity request. That request is the six-byte universal MIDI identity request, scheduled one second after the driver's POD initialization schedules startup. | **Static fact.** The delay is the driver's scheduling choice. [Control initialization](https://codebrowser.dev/linux/linux/sound/usb/line6/driver.c.html#686) [Version request bytes](https://codebrowser.dev/linux/linux/sound/usb/line6/driver.c.html#34) [POD initialization](https://codebrowser.dev/linux/linux/sound/usb/line6/pod.c.html#397) [Delay constant](https://codebrowser.dev/linux/linux/sound/usb/line6/pod.c.html#32) |
| Handling the first identity reply schedules a device serial read and then ALSA card registration. The serial read uses a vendor control OUT/IN handshake on endpoint 0, request `67`. | **Static fact.** The identity arrives before that handshake in the source path. For the four-byte serial read, the driver submits value `0421` and index `80D0`, polls length with value `0012`, then fetches the data with value `0013`. [POD identity handler](https://codebrowser.dev/linux/linux/sound/usb/line6/pod.c.html#177) [POD startup stages](https://codebrowser.dev/linux/linux/sound/usb/line6/pod.c.html#274) [Linux control exchange](https://codebrowser.dev/linux/linux/sound/usb/line6/driver.c.html#340) |
| Intel Line 6 MIDI setup has an interrupt-IN halt clear and a vendor control OUT `40/67`, value `0101`, index 0, zero length. Windows has that same first-reader request and a separate attach request, value `0201`, index 2. | **Static fact.** These values come from host-driver disassembly. [macOS MIDI setup](driver-macos-podxt.md#coremidi-plug-in-and-kernel-midi-transport) [Windows startup](driver-windows-podxt.md#attach-and-start-traffic) |
| For the first direct identity test, select alt 1 on interface 0, receive on `84` and send only the identity message on `03`. Omit serial queries, audio transfers, vendor enable writes and resets. | **Inference.** Linux gets its MIDI identity without a preceding vendor enable in the inspected path. The first direct run follows that evidence. |
| Minimum settling time, the need for vendor enable or a halt clear from a cold macOS attachment, and any effect of prior Linux binding before a claim. | **Unknown.** Linux driver source and ALSA-side captures cannot establish cold user-space startup behavior. |

## The guitar relay

**Static fact.** Workbench's PODxt Live mode brackets its guitar session
with family-`03` SysEx `F0 00 01 0C 03 23 01 F7` and
`F0 00 01 0C 03 23 00 F7`. Its separate family-`07` command `2A 01`
starts the Variax MIDI session, with `2A 00` at close. Monkey's traces
describe the `03 23` pair as enabling and disabling large buffers. The
ignored `podxt-variax-session.syx` starts with the `03 23 01` frame and
then records the identity request and family-`07` traffic. That file
preserves message order, without USB endpoint metadata. [Relay study and owner observations](podxt-variax-relay.md)

**Static fact.** The existing direct guitar link uses interface 1 and has
its own identity transaction. The current guitar procedure uses the relay
pair where called for by the guitar session. The Linux kernel's interface-1
ALSA startup has a separate activation sequence. A PODxt identity on
interface 0 needs none of those guitar-session actions in the Linux source
path. [Existing guitar transport](../crates/line6-devices/src/family/variax/transport.rs)
[Linux guitar startup](https://codebrowser.dev/linux/linux/sound/usb/line6/variax.c.html#67)

**Inference.** Keep the relay pair in the existing Variax procedure. An
interface-0 link only supplies MIDI bytes to and from the PODxt. The relay
state belongs to a guitar transaction, separate from USB interface selection
and vendor endpoint setup. The driverless build can use the existing
interface-1 route for the guitar and reserve interface 0 for the PODxt
identity and tone traffic.

**Unknown.** The captures do not establish an interface-0 route for all
guitar opcodes, the internal meaning or persistence of the relay state, or
an inbound reply-size limit. One silent guitar request leaves those
questions open. [Relay study](podxt-variax-relay.md)

## Boundary for the link and the owner run

**Inference.** The initial Linux run should check the target attachment and
endpoint descriptors, hold the rig lock through driver return, detach only
`snd-usb-pod` for interface 0, and give it back after the bounded identity
exchange. Verify the driver name on the same attachment on success and
failure. Detaching interface 0 disconnects the associated ALSA card and PCM state.
The owner run assumes no client is using that card.
The reported outcome is identity plus driver return. [Linux disconnect path](https://codebrowser.dev/linux/linux/sound/usb/line6/driver.c.html#829)
[Existing driver-return card](variax-driver-rebind-test-card.md)

**Inference.** The macOS stage can look for the same endpoints on arm64 and
x86_64 and use normal libusb claims on the matched device. If a Line 6 kext
owns an interface, report the conflict and stop. The macOS distribution
decision calls for vendored, statically linked libusb. The owner runs both
OS checks under the separate compile-time switches.

**Unknown.** The direct interface-0 path and driver return have not been
run against the owner rig. A Mac test will also need to establish ownership
and cold startup with no Line 6 kext.

The public kernel source above is protocol documentation. The reviewed
`pod.c`, `driver.c`, `midi.c` and `variax.c` pages identify Linux revision
`v6.19-rc8-185-g2687c848e578`. The `midibuf.c` page identifies
`v6.16-rc1`. The source files are GPL-2.0-only. This note records behavior
and protocol values in original prose. The later Rust link will use the
workspace's own transport and parser.
