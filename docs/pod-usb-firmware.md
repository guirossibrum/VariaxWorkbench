# PODxt Live USB firmware component

Partial research milestone for [#20](https://github.com/guirossibrum/VariaxWorkbench/issues/20).
The installed USB-component version is unknown. No USB-firmware image was found.
macOS Monkey 1.78 has a userspace service method that builds a USB vendor IN
control transfer. PODxt Live's USB product id selects device-manager
construction. The class of the service object installed for that device is
still open. The transfer was not sent. No device was opened, and no update
was run.

The main flash revision and the USB firmware component are different things.

## Shipped catalog placeholder

Monkey 1.78 `default.cfg` and the Windows Monkey `RELEASES` resource both
contain a software entry distinct from Flash Memory:

| Field | USB Firmware entry |
| --- | --- |
| Name / type | `USB Firmware` / `usb` |
| Release id | 0 |
| Version | `0.00` |
| URL and filename | empty |
| Date | 2010-09-30 |
| Note | Updating this component requires a connection to the internet. |
| PODxt Live | listed by MIDI id `0x0003000a`, among many other devices |

Version `0.00` with an empty filename is a placeholder. It is an empty catalog
row, and it is a different revision from the archived main-flash container
**3.01**. No file in the local archive tree has a USB-firmware image name. The
`.xtf` file remains the main flash container.

Monkey and License Manager strings keep the component separate from device
flash: "Uses Server USB Firmware updates", a distinct label "USB Firmware
Version:", and the License Manager text that a USB firmware update may be
required before licenses can be installed. Those strings do not include a
PODxt Live version number. The macOS driver payload lists the names "PODxt
Flash" and "PODxt USB Firmware" next to each other, again without a version.

## What a universal identity reply contains

`IDENTITY_REQUEST` in `src/read_session.rs` is the universal inquiry
`F0 7E 7F 06 01 F7`. The reply parser in `src/midi.rs` reads one product id and
one four-byte revision. PODxt Live's product id is `0x0003000a`. The archived
main-flash container `PODxtLive_3_01.xtf` displays revision **3.01**, and the
previously decoded identity reply for this product displays the same revision.
That reply layout has no second version field, so the main-flash revision is
the only version it carries.

Saved Variax identity from the same period is guitar product `0x00070004`,
displayed **3.10**, plus metadata reads. That is a guitar identity, and it does
not supply a POD USB-component version.

## Static print site

The Monkey 1.78 macOS executable was disassembled offline. The binary was not
executed. Addresses below are virtual addresses in that Mach-O i386 image.
Symbol names from the disassembler are unreliable in this region, so the
findings are tied to addresses.

The diagnostic printer at `0x3233a` has one reference to the string "USB
Firmware Version:" (`0x14e494`), at instruction `0x3295b`. It prints the string
already stored at diagnostic-object offset `0x2c`. "Device Version:" is a
different label and prints the string at offset `0x18`.

The fill routine at `0x32bae` runs before that printer. It copies the driver
session's cached string at offset `0xc` into offset `0x2c` through the getter
at `0x678a`. When that cache is empty, `0x678a` calls `0x65aa`. `0x65aa` reads
a 16-bit value from virtual slot `0x28` on the session's driver object and
formats that word into the cache. On failure it logs "Failed to get firmware
version."

## Recovered request shape

Interface type `0x100` on `L6USBDevice_DevMgr` constructs `L6BrandiImpl_DevMgr`.
For USB vendor `0x0e41` and USB product `0x4650`, the static
`IsInterfaceSupported` compare at `0x1482aa` reaches the return-true path.
Virtual slot `0x28` of `L6BrandiImpl_DevMgr` is `0x145466`. That method holds
no message bytes. It tail-calls slot `+0x14` of the service pointer stored on
the Brandi object.

`L6BrandiService::GetVersion` at `0x9bcd0` returns `0x80000005` and builds no
request. `L6BrandiServiceMac::C2` at `0x9bb70` installs vptr `0x1c1878`. Slot
`+0x14` of that vtable is `0x8c150`. When the product word read there is
`0x534D`, that function takes another branch. Any other product word, including
a word equal to USB PID `0x4650`, takes `0x8c040`. Taking that branch requires
the object to be this userspace service.

`0x8c040` fills one control transfer and calls function offset `0x68` on the
device interface stored in the service. That offset matches `DeviceRequest` on
the public `IOUSBDeviceStruct182` layout, which submits on the default control
pipe. This record does not link IOKit, Monkey, or a driver. The fields are:

| Field | Value |
| --- | --- |
| bmRequestType | `0xC0` (device-to-host, vendor, device) |
| bRequest | `0x67` |
| wValue | `0x0011` |
| wIndex | `0` |
| wLength | `3` |

The builder leaves the transfer unissued when a readiness flag at the service
is clear or the interface pointer is null. After the call, the first two bytes
of the three-byte buffer become a little-endian word, and the third byte is
stored separately. The formatter turns that word into the "USB Firmware
Version:" string. A display rewrite for some product ids can run after that
word returns; the rewrite is display post-processing. No device returned a
buffer, so the word is not an installed version, not placeholder `0.00`, and
not `bcdDevice` 0.01.

The kernel service vptr is `0x1c1838`. Its slot `+0x14` is `0x8b810`, a
different function. Its `PreLoadKey` relocation sits at vtable offset `+0x24`,
as does the userspace `PreLoadKey`. Those slots are not the version slot.

## PODxt Live device-manager construction

Before the device manager exists, one matcher requires USB vendor `0x0e41`
(`0x1425d6`). USB PID `0x4650` at `0x142805` sets the expected product id to
`0x0003000a` and jumps to `0x142976`. When that value equals the id at the
device object `+0x208`, the only call to the `L6USBDevice_DevMgr` constructor
runs at `0x1429a4`. The constructor at `0x148790` stores that device object at
`DevMgr+0x110`.

`QueryInterface` type `0x100` then constructs `L6BrandiImpl_DevMgr` with
`this+0xc` equal to `DevMgr+0x110`. Slot `+0x14` of the Brandi object
(`0x145356`) calls slot `+0x14` of that stored device object and keeps the
returned pointer at Brandi `+0x10` when the out-status is non-negative. Slot
`0x28` tail-calls slot `+0x14` of the returned pointer.

## Service class still open

The returned pointer is the service object whose version method runs. The
userspace constructor that installs vptr `0x1c1878` has no PC-relative call in
this image; its address appears in a data table. This pass does not show that
the device object's slot `+0x14` returns an `L6BrandiServiceMac` rather than
the kernel service. Until that return is identified, `0x8c040` is the
userspace builder and is not yet the closed PODxt Live query.

A byte scan of the Windows Monkey executable did not establish this instruction
sequence. The finding is the macOS Monkey 1.78 image only.

The transfer must not be sent. A recovered shape is not a device reply, and it
does not authorize a read or an update. `src/pod_usb_firmware.rs` stores the
fields as plain data and exposes no submit function.

## Candidate that does not fill the version field

An 8-byte manufacturer frame is stored at `0x15c52b`:

`F0 00 01 0C 03 0E 00 F7`

Its only code reference is instruction `0x33105`, in a refresh routine that
also walks several product ids, including `0x0003000a`. The callback at
`0x322b6` accepts a 14-byte message and stores one status byte. That path does
not write the string printed as "USB Firmware Version:". The frame is a
ruled-out candidate. It must not be sent.

## Saved descriptor, still unexplained

An earlier descriptor listing reports USB vendor `0x0e41`, product `0x4650`,
`bcdUSB` 1.00, `bcdDevice` **0.01**, one configuration and two interfaces. It
was not collected again. `bcdDevice` 0.01 is a different number from main-flash
**3.01** and from the placeholder **0.00**. Nothing in the saved evidence says
which, if any, of those numbers is the USB firmware component.

The macOS driver personality matching vendor 3649, product 18000, configuration
1, interface 0 identifies the audio interface. It is a device match, and it
does not give a firmware version. No driver was installed.

## Gaps

- Applicable USB-firmware image: none identified in the local archive.
- Installed USB-component version: unknown. No device reply was captured.
- Version-query shape: the userspace service builder above is recovered and
  was not sent. The PODxt Live service-class link is still open. The Windows
  image was not shown to match the builder.
- `bcdDevice` 0.01: unexplained, and it was not read again.

## Future check

1. Offline: the request shape is recorded. Leave it unsent, including the
   ruled-out frame above. The open offline gap is the missing USB-firmware
   image and the unknown installed version.
2. A live read still needs a separate explicit authorization. No reply has
   been captured.
3. Only after that authorization, send the reviewed read once. Do not update,
   reset, switch banks, or install a driver.

Until that authorization exists, there is no justified live query. A version
learned later would still not authorize an update.

## Decisions left for review

- Issue #20 stays open. This document is a partial milestone.
- The combined acceptance line stays open because the artifact, the installed
  version, and the PODxt Live service class are still unknown.
- `bcdDevice` 0.01 stays an unexplained descriptor field.
- The recorded transfer stays unsent. The crate keeps a portable data record
  and does not take IOKit, Monkey, or a driver as a dependency.
- No USB-firmware image is claimed to be inside the main `.xtf` container.
