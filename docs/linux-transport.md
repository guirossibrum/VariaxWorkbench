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
inside the authorized offline work. Installation, loading, binding, and
probing still require explicit authorization and remain prohibited. The
detached build below was not installed.

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

## Integration review

PR #57 was squash merged as `002471c` after reviewing rebased head
`0385e0107d50cf9beb7cc3ec3048ddd823896b26`. The exact upstream
[source file](https://github.com/gregkh/linux/blob/497791a0a96b866d90add64041c47acdff49cae6/sound/usb/line6/variax.c)
matched the recorded SHA-256, and the patch passed a dry run against a detached
copy. The installed compressed module matched its recorded digest, and its
identity/state branches matched the documented disassembly. Local checks passed
128 all-feature tests, 115 core tests, nine setup guards, Clippy and formatting;
the exact-head three-platform core CI also passed. The saved frame is absent
from the rewritten branch and was not reintroduced into master history.
Issue #21 remains open; this review is no proof of live connectivity or safety.

## Detached rebuild

The installed headers omit every Line 6 C file except `Kconfig`. They do
contain the generic kernel headers, `sound/core.h`, `.config`, `Module.symvers`,
and `vmlinux`. `variax.c` includes `driver.h`, and that header includes
`midi.h`, which includes `midibuf.h`. Those three headers from commit
`497791a0a96b866d90add64041c47acdff49cae6` were enough. Their SHA-256 values
are `6bdbf42547971ee457d67a02b742ef342ffc7ca9a675d5f298ef21f6e1c3f8cf`,
`8928524fc89e772aec5cfabf6eb568d33f43cd0216a195bdaf44cd9a9209e060`, and
`c56dfe4fe244119b98dbcae55197d60267533be0d512e4c22d6ec9d613599f80`. The full
kernel tarball, `driver.c`, and the PCM objects were not required. The upstream
Makefile already builds `snd-usb-variax` from `variax.o` alone.

`make modules` used `/lib/modules/7.2.15-1-omarchy/build` and wrote only under
a temporary directory. The host compiler is the same `gcc (GCC) 16.2.1 20260810`
recorded in that tree's `include/generated/compile.h`. Both objects report
vermagic `7.2.15-1-omarchy SMP preempt mod_unload`. Their `srcversion` values
are `D89A1DD411DBFCF34E2F6C2` and `8D7F8A31C939F5CEAB036CE`, not the installed
`1885465675CE02E68674BB5`, so the rebuilt files are not drop-in copies of the
packaged module. The whole `.ko` files also differ because the detached link
keeps the build's debug information.

The unpatched `.text` is 793 bytes (`0x319`) and its SHA-256 is
`3eae27ccbea3a6ca067a86faf4ade9c713702234b369df20e1391fbe4ff9849d`. That is
the installed module's `.text` as well. The patched `.text` has the same size
and SHA-256
`3d3aa8f53ab2dc4a97bfffa37e73cffea55d4354ab31bbffdb4186b75c058454`. Bytes
from `variax_startup` onward match the unpatched object. The difference is
inside `line6_variax_process_message`.

The patched family compare is still `cmpq` of `7E 7F 06 02 00 01 0C 07` at
offset 1. A local `je` then reaches three byte checks: `cmpb $0` at offset 9
(family high), `testb $0xfb` at offset 10 (model low), and `cmpb $0` at offset
11 (model high). The mask `0xfb` leaves only bit 2 clear, so the model-low
byte passes only for `0x00` and `0x04`. The
progress store is unchanged: load the dword at `0x2128`, `testl` / `jg`, then
`movl $1` and delay immediate `0x64`. The init-done arm still compares with
`1` and skips when the value is greater than 1. Relocated `e8` and `e9`
slots are zero in the object file; their displayed targets are not callees.
The local `74`, `75`, and `7f` displacements are the branch evidence.

Neither object was copied into `/lib/modules`, and neither was passed to
`modules_install`, `depmod`, or a module loader. This rebuild is not a live
driver trace and it is not connectivity. Issue #21 stays open.

## Compiled prefix harness

`src/linux_variax_prefix.rs` is a Rust test. It interprets three half-open
windows from the detached objects:

| Window | Symbol | Bytes | SHA-256 |
| --- | --- | --- | --- |
| `[0x160, 0x240)` | `line6_variax_process_message`, unpatched | 224 | `68dea14f9600a5c5cb820c35f2a9ab3a65e71f112b0dbc5017d8e0146ad3d7a1` |
| `[0x160, 0x240)` | same function, patched | 224 | `e9ef1f53b7e6ce3c52ac39581dd4fe1bdccd634d56b4358c505916b301cd1a6b` |
| `[0x250, 0x309)` | `variax_startup`, both objects | 185 | `82e84447105d6a8db79880fa7f340b969f7efa8ff14713a419ddaf3238a65db2` |
| `[0x309, 0x319)` | `line6_variax_process_message.cold`, both objects | 16 | `e2134e1ab91cd5c4c2735fb7f5030aa3b8b351bfe574ff083a671501ac3c6401` |

The model-byte patch changes only bytes `0x196` through `0x23f`, so the
startup and cold windows are identical in both objects. The 16-byte nop
prefix at `[0x240, 0x250)` is not part of either function. All four slices
sit inside the 793-byte `.text` images recorded above.

Those instruction arrays are compiled Linux reference bytes. Upstream
`sound/usb/line6/variax.c` declares `SPDX-License-Identifier: GPL-2.0-only`
and Copyright (C) 2004-2010 Markus Grabner. The pinned commit is
`497791a0a96b866d90add64041c47acdff49cae6`. The patched prefix is that file
plus `docs/linux-variax-model-bytes.patch`, built with
`gcc (GCC) 16.2.1 20260810` by the detached `make modules` recipe above.
The license and the full recipe are in
[third_party](../third_party/README.md). The bytes are test reference data.
They add no runtime dependency. Message buffers in the sweep are synthetic
inputs, separate from the instruction arrays. The activate buffer is a copy
of the committed `VARIAX_ACTIVATE` constant.

The interpreter is safe Rust. It decodes the local compares, stores, and
branches. It does not map a module and it does not follow a relocated
immediate into kernel code. A `call` is recorded and execution continues. A
tail `jmp` stops.

These sites stay inside the harness:

- `__fentry__` is recorded, then execution continues.
- `cancel_delayed_work` is recorded when `rdi` is the device plus `0x20b0`,
  then the prefix continues.
- The `system_percpu_wq` load sets `rsi` to zero. The harness does not
  dereference that pointer.
- `__x86_return_thunk` stops. The startup return at `0x304` is this thunk.
  Its zero immediate is not a branch into the cold body.
- A tail `jmp queue_delayed_work_on` records `ecx` as the delay, `edi` as
  cpu `0x2000`, and `rdx` as the work pointer at device plus `0x20b0`, then
  stops. The identity delay is `0x64`. The init-done delay is `0`. The
  activate-arm delay is `0x64`.
- The version-request arm's `call queue_delayed_work_on` records delay
  `0x3e8` (source `VARIAX_STARTUP_DELAY1`, 1000 ms), the same cpu, and the
  same work pointer, then continues. The following tail jump is
  `line6_version_request_async`. That jump stops. Progress stays unchanged
  and the request is not sent.
- `line6_send_raw_message_async` is recorded and execution continues. The
  recorded arguments are the device, the activate buffer, and length 9.
  Byte 7 of that buffer is already `1` at the call. The message is not sent.
- `snd_card_register` stops with `rdi` equal to the pointer loaded from
  device plus `0x28` (`struct usb_line6.card` in the recovered header).
  ALSA is not called. Progress stays 2.
- `_dev_info` stops. The cold body has loaded the pointer at device plus
  `0x20` (`ifcdev`) into `rdi`. The format immediate is the unresolved
  relocation `R_X86_64_32S .rodata+0x158`. The source literal is
  `VARIAX reset\n`. The harness keeps the object's zero immediate and does
  not print it.

The taken cold `je` in `line6_variax_process_message` uses relocation
`0x17A R_X86_64_PC32 .text+0x305`. That resolves to VA `0x309`, so the
harness enters the cold body. The zero displacement stored in the object is
not added.

`variax_startup` follows the compiled tests on the dword at `0x2128`:

- Progress 0 falls through to the version-request arm above.
- Progress 1 loads the activate pointer from device plus `0x2120`, stores
  `1` at byte 7, stubs the 9-byte send, stores progress 2, then tail-jumps
  to the 100 ms queue.
- Progress 2 takes the ALSA arm above.
- Every other value, including `i32::MIN`, `-1`, `3`, and `i32::MAX`,
  returns through the thunk and leaves the dword unchanged.

The prefix check runs both images. Every model-low byte from `0x00` through
`0xff` is crossed with progress values `i32::MIN`, `-1`, `0`, `1`, `2`, and
`i32::MAX`. Model-low `0x00` and `0x04` are also crossed with every
family-low, family-high, and model-high byte at progress 0. The dword stored
at `0x2128` matches the Rust gate for that image. The unpatched compare still
requires model `0x0000`. The patched `testb $0xfb` still accepts only
model-low `0x00` and `0x04`, and only when family high and model high are
zero. A match stores `1` and queues delay `0x64` only while the signed
progress is not greater than 0.

Init-done uses the same progress boundaries. It queues delay `0` when the
signed progress is not greater than 1, and it leaves the stored dword
unchanged. A non-SysEx first byte returns through the thunk. A first byte
`0xff` enters the cold body, leaves progress unchanged, and stops at
`_dev_info`.

The delay immediates are register arguments. This harness does not run a
timer. `variax_init`, probe, and disconnect stay outside this interpreter.

## Offline acceptance

The written #21 criteria are an offline investigation:

1. Compare kernel matching and routing with saved replies and the Windows
   and macOS static driver evidence. The installed module's USB `0x4650`
   interface-1 row, the property endpoints, and the compiled identity compare
   are recorded against saved product `0x00070004` and version `0x03100000`,
   which remain derived scalars. The Windows INF match and the macOS
   personalities are the static routing evidence above. The startup switch
   and the reset cold body are now interpreted from the same objects.
2. Add offline identity and routing fixtures and establish the smallest
   justified Linux transport change. The synthetic comparator packets, the
   prefix sweep, and the startup and cold oracles are those fixtures. The
   model-byte patch is the change: it accepts model low `0x00` and `0x04`
   and leaves the progress store as it was.
3. Document interface ownership, framing, and cleanup, install no driver
   patch, and make no live probe. Ownership, framing, and cleanup are
   recorded above. `DRIVER_PATCH_INSTALLED` and `LIVE_PROBE_PERFORMED` stay
   false. `src/usb.rs` is unchanged and `LIVE_ACCESS_LOCKED` stays on.

Review accepted all three offline criteria after PR #62 merged as `9c8cceac7bc6e4981eb9477707b8d29d871f47a0`.
Independent checks compared the startup/cold slices and relocation targets with
both linked modules, and passed 132 all-feature tests, Clippy and formatting.
The exact PR head passed three-platform core CI. Issue #21 is complete as an
offline investigation. This does not claim live connectivity, authorize a driver
installation or unlock equipment. Earlier open-status milestones above are
historical. The activate byte starts at zero in the fake buffer to make the
compiled store of one observable; the real init template already contains one.

`bcdDevice` 0.01 and the USB component version remain on issue #20. USB
product `0x534D` remains a separate Workbench match from this guitar's USB
`0x4650` and MIDI product `0x00070004`. Connectivity belongs to a later
phase with its own authorization. The patched object keeps a different
`srcversion`, and the detached link keeps its debug information. The
rebuild was not installed.
