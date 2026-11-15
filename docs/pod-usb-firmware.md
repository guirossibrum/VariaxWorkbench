# PODxt Live USB firmware component

Partial research milestone for [#20](https://github.com/guirossibrum/VariaxWorkbench/issues/20).
The installed USB-component version is unknown. No USB-firmware image was found.
The version-query request bytes are still unrecovered. No device was opened, no
version query was sent, and no update was run.

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

The body of virtual slot `0x28` was not located in this executable. The bytes
of the version query are therefore still unknown. No live read is justified
until those bytes have been recovered and reviewed.

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
- Installed USB-component version: unknown.
- Version-query request and reply layout: unrecovered. The remaining offline
  target is the driver method at virtual slot `0x28`.
- `bcdDevice` 0.01: unexplained, and it was not read again.

## Future check

1. Offline: recover the driver method at virtual slot `0x28` and write down the
   request bytes. Do not send any candidate while doing that, including the
   ruled-out frame above.
2. Review those bytes and the expected reply in the offline ledger. A live read
   still needs a separate explicit authorization.
3. Only after that authorization, send the reviewed read once. Do not update,
   reset, switch banks, or install a driver.

Until step 1 exists, there is no justified live query. A version learned later
would still not authorize an update.

## Decisions left for review

- Issue #20 stays open. This document is a partial milestone.
- `bcdDevice` 0.01 stays an unexplained descriptor field.
- The next offline step is the unrecovered virtual method, not a device read.
- No USB-firmware image is claimed to be inside the main `.xtf` container.
