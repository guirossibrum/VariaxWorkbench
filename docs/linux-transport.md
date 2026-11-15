# Linux Variax interface split

This is an offline reading of the modules installed with kernel
`7.2.15-1-omarchy`, and of `sound/usb/line6/variax.c` from Linux tag
`v7.2.15` commit `497791a0a96b866d90add64041c47acdff49cae6`. SHA-256 of
that file is `f623cd628440ee3208f857a97a462799260c068633774e4b3ff2cad9107c62af`.
The file is not vendored here. Installed headers contain `Kconfig` only in
that directory. Packaging commit
`326470ed1eca0141d687c701a4faf11694c3358d` builds `linux-omarchy` 7.2.15-1
from that kernel plus 188 patches. None of those patches mention `variax`
or `sound/usb/line6`. The modules were not loaded, unbound, bound, or
installed by this pass. No USB probe was run.

The record is specific to these builds:

| Module | srcversion | SHA-256 of the installed `.ko.zst` |
| --- | --- | --- |
| `snd-usb-variax` | `1885465675CE02E68674BB5` | `198be2ba22408858304d2cbe4b267519516196893ab734848a9e50e5ea3a3c2b` |
| `snd-usb-pod` | `A4A2201EC078E5484AE28F9` | `0f64d61922a6c557430f4f8f0d9290c80698bd7053fc5ee205331c797e124863` |
| `snd-usb-line6` |  | `3ad0267696dff9d609f949b2c4d744e4493e7493cff8d92cc29f1e38f7b8dc29` |

`vermagic` on the variax and pod modules is `7.2.15-1-omarchy SMP preempt mod_unload`.
Another kernel build can differ. These constants do not describe every Linux tree.

## USB matching

`struct usb_device_id` in the installed `include/linux/device-id/usb.h` places
`bInterfaceNumber` at byte 16 and aligns `driver_info` to byte 24. Each table
entry is 32 bytes. `variax_probe` reads `driver_info` from the id at offset
`0x18` and uses it as an index into `variax_properties_table`, stride `0x30`.

| Module row | Flags | Product | Interface constraint | `driver_info` | Property name |
| --- | --- | --- | --- | --- | --- |
| variax row 0 | `0x0403` vendor, product, interface | `0x4650` | interface 1 | 0 | `PODxtLive` / `PODxt Live` |
| variax row 1 | `0x0003` vendor, product | `0x534D` | none | 1 | `Variax` / `Variax Workbench` |
| pod row for `0x4650` | `0x0403` | `0x4650` | interface 0 | 5 | pod table, not this record |

The names come from relocations on those property rows, and the rows are
selected by `driver_info`. The display string is not used by itself to decide
which device matched. USB product `0x534D` is a different match from this
guitar's USB product `0x4650`. Decimal `21325` is `0x534D`. Decimal `18000`
is `0x4650`.

The saved descriptor listing for USB `0x0e41`/`0x4650` has two interfaces.
Interface 0 altsetting 1 carries the isochronous and interrupt endpoints of
the POD side. Interface 1 altsetting 0 has no endpoints. Interface 1
altsetting 1 has interrupt OUT `0x05` and interrupt IN `0x86`. That listing
was not collected again.

## Property row for interface 1

Installed BTF gives this `line6_properties` layout. The `driver_info` 0 row
stores:

| Field | Value |
| --- | --- |
| `capabilities` | `0x11` |
| `altsetting` | `1` |
| `ctrl_if` | `0` |
| `ep_ctrl_r` | `0x86` |
| `ep_ctrl_w` | `0x05` |
| `ep_audio_r` | `0x82` |
| `ep_audio_w` | `0x01` |

`line6_probe` calls `usb_set_interface` with the probed interface
descriptor's `bInterfaceNumber` and `properties.altsetting`. For the
`0x4650` variax match, that is interface 1 and altsetting 1. The call does
not pass `ctrl_if`. The stored `ctrl_if` value `0` is a different field from
the matched interface number.

`ep_ctrl_r` and `ep_ctrl_w` are the interface-1 interrupt endpoints.
`ep_audio_r` and `ep_audio_w` equal the interface-0 isochronous endpoints in
the saved descriptor. This pass does not show `line6_probe` opening those
audio endpoints. The id-table match for the variax module remains interface 1.

The locked Rust transport in `src/usb.rs` already selects vendor `0x0e41`,
product `0x4650`, interface 1, altsetting 1, OUT `0x05`, and IN `0x86`.
`open()` returns before any of that runs while `LIVE_ACCESS_LOCKED` is set.
This record does not edit that file.

## Identity compare

`line6_version_request_async` in `snd-usb-line6` duplicates six bytes at
rodata `0x76c`: `F0 7E 7F 06 01 F7`. That is `IDENTITY_REQUEST`. Variax
startup state 0 tails into that helper. The duplicate is a static reading.
It was not submitted.

`line6_variax_process_message` takes a SysEx path when the first byte is
`0xF0`. A first byte `0xFF` branches to a cold block that calls `_dev_info`.
Any other first byte returns. On the SysEx path the function compares the
eight bytes at offset 1 with `7E 7F 06 02 00 01 0C 07`, then the four bytes
at offset 8 with `07 00 00 00`. Those four bytes are MIDI family `0x0007`
and member `0x0000`, product `0x00070000`. Only that compare reaches the
store of startup progress `1` at object offset `0x2128`. The byte compare
and the progress gate are separate. `matches_startup_identity` is only the
bytes. Linux `v7.2.15` then does
`if (startup_progress >= VARIAX_STARTUP_ACTIVATE) break;` before the
assignment. The enum values are `VARIAX_STARTUP_VERSIONREQ` 0,
`VARIAX_STARTUP_ACTIVATE` 1, and `VARIAX_STARTUP_SETUP` 2. The installed
object code loads the dword at `0x2128`, executes `testl` / `jg`, and stores
`1` only when that signed value is not greater than 0. Installed BTF names
that field `startup_progress` and places it at byte 8488, which is `0x2128`.
A matching message at progress 0 or below assigns 1. The same bytes at
progress 1, or any greater value, leave progress unchanged.

The saved Variax 300/600 reply is kept here as product `0x00070004` and
revision `0x03100000` (displayed 3.10). Its frame stays in the local capture.
The model field in that product is `0x0004`, so the four bytes at offset 8
are `07 00 04 00` rather than `07 00 00 00`. Those scalars fail
`matches_startup_identity`. They also fail the init-done prefix
`F0 00 01 0C 07 00 6B`. The init-done arm has its own gate: the source skips
scheduling when progress is already `>= VARIAX_STARTUP_SETUP`, and the
installed code compares the same dword with `1` and skips when the value is
greater than 1. That arm does not store progress 1.

Comparator packets in the Rust record are synthetic. Both use revision ASCII
`0001`, which parses as version `0x00010000` and is a different revision from
`0x03100000`. The model-4 packet parses as product `0x00070004` and fails the
installed byte compare. The member-zero packet parses as `0x00070000`, passes
the byte compare, and still has to pass the progress gate. A synthetic model
`0x0005` passes neither the installed compare nor the offline patch below.
POD product `0x0003000a` fails the variax compare. None of these packets is a
captured reply.

`variax_init` duplicates the nine-byte `variax_activate` template
`F0 00 01 0C 07 00 2A 01 F7`. Those bytes are `midi_enable()`. Startup state
1 sets template byte 7 to `1`, which the template already contains, and
calls `line6_send_raw_message_async`. This repository does not send that
frame. The existing read allowlist is unchanged.

The rodata symbols `variax_init_version` and `variax_init_done` contain the
compare bytes. No relocation in the variax module's `.text` points at those
two symbols. The compares use immediates.

## Windows and macOS static matches

The Windows PODxt INF `DriverVer` is `03/15/2019,4.2.7.7`. Both the x86 and
amd64 manufacturer sections map PODxt Live to `USB\VID_0E41&PID_4650`.
That hardware id has no interface number.

Both macOS Line 6 audio kext plists have a `PODxtLive` personality:
`IOUSBInterface`, vendor `3649`, product `18000`, configuration `1`,
interface `0`, class `com_line6_audio`. That is the POD-side interface.
`Line6VariaxUSB.kext` matches `IOUSBDevice` vendor `3649` and product
`21325` (`0x534D`) and has no `bInterfaceNumber`. It matches the second
Linux variax row, not USB `0x4650` interface 1.

## Smallest change in this repository

`src/usb.rs` is unchanged. It already selects vendor `0x0e41`, product
`0x4650`, interface 1, altsetting 1, and the interrupt endpoints above.
`open()` still returns immediately while `LIVE_ACCESS_LOCKED` is set.

The identity mismatch is the model field inside `variax_init_version`.
That array hard-codes model `0x0000`. The saved product's model is `0x0004`.
The smallest driver change that covers both keeps the family compare and the
existing progress gate, and accepts model `0x0000` or `0x0004`. The diff is
`docs/linux-variax-model-bytes.patch`. It applies to the recovered `v7.2.15`
file. It is not applied to the installed tree.

Recovering this source, and building or patching a detached module, are
inside the authorized offline work. This pass did not produce a `.ko`.
The installed headers omit `driver.h` and the other line6 objects, so
linking the module needs an unpacked kernel tree. Installation, loading,
binding, and probing still require explicit authorization and remain
prohibited.

Issue #21 stays open.

## Ownership, framing, and cleanup

- Interface 0 of USB `0x4650` is the pod module's match and the macOS audio
  personality. Interface 1 is the variax module's match. Claiming interface 0
  from the guitar-control path would take the POD side.
- Interface 1 altsetting 0 has no endpoints in the saved descriptor.
  Altsetting 1 has the interrupt pair. The kernel sets that altsetting on the
  probed interface. The locked Rust path names the same altsetting and does
  not reach it.
- `ctrl_if` stored as 0 is not the interface number passed to
  `usb_set_interface`.
- If a later authorized open detaches the kernel driver on interface 1, the
  existing cleanup releases that interface and reattaches the driver. A
  failed reattach would leave interface 1 unbound. This pass does not open
  the device, so that path is not exercised.
- The kernel's own activate template is a session command. Matching it to
  `midi_enable()` does not authorize a send. The model-4 result compares the
  derived product `0x00070004` with the module immediates and the recovered
  source. It is a static reading. It is not a live driver trace, and it is
  not connectivity.
- USB product `0x534D` and MIDI product `0x00070000` are different identities
  from this guitar's USB `0x4650` and MIDI `0x00070004`.
- `bcdDevice` 0.01 in the saved descriptor remains a different number from
  MIDI revision 3.10.
