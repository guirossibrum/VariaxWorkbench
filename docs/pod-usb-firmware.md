# PODxt Live USB firmware component

The main flash revision and the USB firmware component are different things.
This note records what the saved packages and captures establish. No device was
opened, no version query was sent, and no update was run.

## What the captured identity establishes

`analysis/captures/podxt-identity.syx` is 17 bytes:

`F0 7E 7F 06 02 00 01 0C 03 00 0A 00 00 03 00 01 F7`

That is the universal identity reply. The request saved beside the later
session traffic is the universal inquiry `F0 7E 7F 06 01 F7`
(`IDENTITY_REQUEST` in `src/read_session.rs`). The reply parser in `src/midi.rs`
reads one product id and one four-byte revision. For this capture those are
product `0x0003000a` and revision bytes `00 03 00 01`, displayed **3.01**.
That is the same product and revision declared by the archived main-flash
container `PODxtLive_3_01.xtf`.

The reply has no second version field. It does not establish the USB firmware
component version, whether an update is required, or which USB interface owns
audio or guitar control. The Variax captures from the same period are guitar
identity `0x00070004` / **3.10** and two metadata reads. They are not a POD USB
firmware version either.

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

Version `0.00` with an empty filename is a placeholder, not an applicable image
and not the observed 3.01 main flash. No file in the local archive tree has a
USB-firmware image name. The `.xtf` file remains the main flash container.

Monkey and License Manager strings keep the component separate from device
flash: "Uses Server USB Firmware updates", a distinct label "USB Firmware
Version:", and the License Manager text that a USB firmware update may be
required before licenses can be installed. Those strings do not include a
PODxt Live version number. The macOS driver payload lists the names "PODxt
Flash" and "PODxt USB Firmware" next to each other, again without a version.

The version-query bytes that would fill "USB Firmware Version:" were not
recovered. The universal identity inquiry is not that query.

## Saved descriptor, not a component version

`analysis/reports/podxt-live-usb-descriptors.txt` is an earlier listing. It was
not collected again. It reports USB vendor `0x0e41`, product `0x4650`,
`bcdUSB` 1.00, `bcdDevice` **0.01**, one configuration and two interfaces.
`bcdDevice` 0.01 is not the identity revision 3.01 and is not the placeholder
version 0.00. Nothing in the saved evidence says which, if any, of those
numbers is the USB firmware component.

The macOS driver personality matching vendor 3649, product 18000, configuration
1, interface 0 is identification of the audio interface. It is not a firmware
version. No driver was installed.

## Future check

1. Offline: find the Monkey code that prints "USB Firmware Version:" and write
   down the request bytes. Do not send them during that work.
2. Review those bytes and the expected reply in the offline ledger. A live read
   still needs a separate explicit authorization.
3. Only after that authorization, send the reviewed read once. Do not update,
   reset, switch banks, or install a driver.

Until step 1 exists, there is no justified live query. Knowing a version later
would still not authorize an update.

## Decisions left for review

- `bcdDevice` 0.01 stays an unexplained descriptor field.
- The next offline step is static recovery of the version-query bytes, not a
  device read.
- No USB-firmware image is claimed to be inside the main `.xtf` container.
