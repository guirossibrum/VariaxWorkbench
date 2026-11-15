# PODxt Live USB firmware component

Partial research milestone for [#20](https://github.com/guirossibrum/VariaxWorkbench/issues/20).
The installed USB-component version is unknown. No USB-firmware image was found.
macOS Monkey 1.78 has a userspace service method that builds a USB vendor IN
control transfer. PODxt Live's USB product id selects device-manager
construction. The version path then reaches an `L6DeviceDescription`, and
that object's slot returns a pointer already cached on a device-control
object. The description setter is virtual slot `+0x0c`, and the traced
producers of that object do not call it through a PC-relative instruction
that names the setter. `L6XTAudioControlMac` can store a kernel Brandi
service at cache index 0. That fill is useful evidence, and its association
with the PODxt Live description remains unproven. The class of the cached
service on the PODxt Live path is still open. The transfer was not sent. No device was
opened, and no update was run.

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

The reviewed i386 executable has SHA-256
`47417308c9697456eeed59720ec97281ec226139c707347e4cad3ce787491a37`.
The public interface layout used to interpret the function-table offsets is
Apple's [IOUSBDeviceStruct182 declaration](https://github.com/apple-oss-distributions/IOUSBFamily/blob/main/IOUSBFamily/Headers/IOUSBLib.h).
Neither that header nor IOKit is a runtime dependency of this project.

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
`this+0xc` equal to `DevMgr+0x110`. That stored object is the
`L6DeviceDescription` returned by `GetDeviceDesc`. Slot `+0x14` of the Brandi
object (`0x145356`) calls slot `+0x14` of the description and keeps the
returned pointer at Brandi `+0x10` when the out-status is non-negative. Slot
`0x28` tail-calls slot `+0x14` of the returned pointer.

## Service class still open

The returned pointer is the service object whose version method runs. The
userspace constructor that installs vptr `0x1c1878` has no PC-relative call in
this image; its address appears in a data table. The kernel constructor at
`0x8adc0` installs vptr `0x1c1838`. The version-path call chain below does not
call either constructor.

## Description object and cached service

`L6DeviceMgrBase::GetDeviceDesc(unsigned long)` is in this image at `0x142b72`.
The matcher calls it at `0x142615` after the USB vendor compare. The method
walks the list at the manager object `+8` and returns the pointer stored in
each list node. That pointer is an `L6DeviceDescription`.

The description constructor at `0x85310` installs vptr `0x1c1598`. It clears
the word at `+0x104` and the word at `+0x106`, sets the dword at `+0x208` to
`0xffffffff`, and default-constructs two null
`L6DevSmartPtr<L6DeviceControlBase>` values at `+0x21c` and `+0x22c`. The
matcher reads those three fields: `+0x104` against the descriptor dword whose
low half is USB vendor `0x0e41`, `+0x106` as the USB product id, and `+0x208`
as the product id compared with `0x0003000a`.

`L6BrandiImpl_DevMgr` slot `+0x14` at `0x145356` calls slot `+0x14` of the
object at `this+0xc` with argument `0`. For this description, that slot is
`0x85800`. If the smart pointer at `+0x21c` is null, `0x85800` returns a null
pointer and status `0x80007004`. Otherwise it uses the smart pointer at
`+0x22c`, copying `+0x21c` into `+0x22c` when `+0x22c` is null, and calls slot
`+0x0c` of that device-control object with the same argument `0`.

Slot `+0x0c` of `L6DeviceControlBase` and of `L6DeviceControlBaseMac` is
`0x85150`. For index `0` it returns the pointer already stored at the control
object `+8`. It does not construct a service. `L6BrandiControlMac` overrides
that slot at `0x9b770`; the override's create path is index `1`, and index
`0` falls through to `0x85150`.

## Description setter slot

The description vtable at vptr `0x1c1598` stores `0x85650` at slot `+0x0c`.
That dword is the only absolute reference to `0x85650` in the Monkey image.
The function adds `0x21c` to `this` and assigns the argument through
`L6DevSmartPtr<L6DeviceControlBase>::operator=`. A search for PC-relative
`call` and `jmp` instructions found none whose displacement names `0x85650`.
That search does not resolve a Mach-O indirect-symbol or lazy-symbol stub.
A call to a bound stub can reach this function without naming its virtual
address.

The five `GetDeviceDesc` call sites use the returned description for its
fields. The one direct caller of the description finder at `0x1482cc` stores
the pointer into another interface object and does not call slot `+0x0c`.
The image also contains 154 `call *0x0c(reg)` instructions. Those sites were
not each attributed to a class. The traced producers of an
`L6DeviceDescription` do not make that call. Absence of a PC-relative call
is not proof that no assignment exists, including an assignment through a
symbol stub.

`L6TWX_u.dylib` contains the same class names and its own description vtable.
Its slot `+0x0c` is `0x54240`, and that function also assigns `this+0x21c`.
A PC-relative search in that image found no call that names `0x54240`.
Indirect-symbol and lazy stubs in that image were not resolved, so this
does not add a caller for the Monkey setter and does not prove the dylib
setter is uncalled.

## Cache index 0 on one control class

`L6DeviceControlBase`'s constructor at `0x85020` writes zero through all 12
cache slots at `+8` and the matching refcounts at `+0x38`.

`L6XTAudioControlMac` (vptr `0x1c18b8`) overrides slot `+0x0c` at `0x8dd20`.
When the index is below 12 and the refcount at that index is zero, a jump
table handles indexes 0 through 11. Index 0 allocates `0x10` bytes, calls
thunk `0x8ad80`, and stores the allocated pointer at cache index 0. The thunk
calls `0x8adc0`, which installs kernel vptr `0x1c1838`. The only PC-relative
call found for `0x8ad80` is this index-0 arm. Stub calls were not resolved.
This producer is useful evidence. Its association with the PODxt Live
description remains unproven.

The only store of vptr `0x1c18b8` found by address is the `lea` at `0x8c893`,
reached from vtable slot `+0x04` at `0x8c810`. No PC-relative caller of that
function was found. Nothing in the traced PODxt Live path stores an
`L6XTAudioControlMac` into the description smart pointer at `+0x21c`.

The kernel version slot at `0x8b810` does not embed the userspace builder
fields `0xC0`, `0x67`, and `0x0011`. `0x8c040` stays the userspace builder.
It is not the closed PODxt Live query, and the five recorded fields are not
an approved live read.

## Images searched for the setter and the cache fill

| Image | Bytes | SHA-256 | Class names |
| --- | --- | --- | --- |
| macOS Monkey 1.78 executable | 2378528 | `47417308c9697456eeed59720ec97281ec226139c707347e4cad3ce787491a37` | description, both Brandi services, and the control classes |
| `L6TWX_u.dylib` | 691160 | `131780a80db09bb9f49486a0f7dcdb6568877e4033f06b1dc1dee6db6720819a` | same class names; setter duplicated; no PC-relative call to it |
| Windows `Line 6 Monkey.exe` | 4178432 | `eb06a7ffaed9cd13119ca8fd4783fddfd03d4e22e23f2fbb3aa9524e272b2cde` | class-name strings only; Itanium type names absent; slot not recovered |
| Signed `Line6Audio` | 1196736 | `728a52a0a79e7457ed2df733cf4cc50ea0cd5b37cfdb85f838555ed239ac462e` | no matching class-name strings |
| Unsigned `Line6Audio` | 1181576 | `721df4f65fcd28f8724be3e131349becab71da4b63db147811b8d1a7b3610886` | no matching class-name strings |
| `Line6MIDI` | 86600 | `92bccafbff4dec45cd05d1eb027b9ea672b9db596e16094638fd50c788647998` | no matching class-name strings |
| `l6podxt.dll` | 151040 | `c9ef3db97f10935fc4f28aa69a639aa4684bce6697dde506ca83592e2b41c69a` | no matching class-name strings |
| `l6podlv.dll` | 151040 | `91cf569bd191a771f3fa3bd62177128dc3bff7fe680d375b72234a9d11ca03e6` | no matching class-name strings |
| `l6podxtpro.dll` | 151040 | `98b6b0d0fb7c76c4c22ae7faeae256420ac9e0132bded5ba238c7d94b5ee129c` | no matching class-name strings |

No image was executed, loaded, or installed. The Windows executable is not
confirmation of the macOS setter. The signed and unsigned audio kexts, the
MIDI plugin, and the PODxt userspace libraries contain no matching class-name
strings for these types. That observation does not exclude a stripped binary
or a different-ABI implementation of the same behavior.

Unknown edges that remain:

- A virtual `call *0x0c(reg)` whose object is an `L6DeviceDescription` obtained
  through an inlined list walk was not separately attributed.
- The description smart pointer at `+0x21c` has no shown assignment to
  `L6XTAudioControlMac`.
- The kernel version method's other call targets were not identified, and they
  were not recorded as a second request.
- PC-relative searches did not resolve Mach-O indirect-symbol or lazy-symbol
  stubs. A call to a bound stub can reach a constructor or setter without
  naming its virtual address. The next bounded offline edge search resolves
  those stubs for the constructors and setters together with direct
  PC-relative calls.

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
  was not sent. The description setter is virtual slot `+0x0c` at `0x85650`.
  Traced description producers do not call it through a PC-relative
  instruction that names the setter. `L6XTAudioControlMac` stores a kernel
  Brandi service at cache index 0. That producer is useful evidence, and its
  PODxt Live association remains unproven. The class of the PODxt Live cached
  service is still open.
- `bcdDevice` 0.01: unexplained, and it was not read again.

## Future check

1. Offline: resolve whether the description's control is
   `L6XTAudioControlMac` before selecting a PODxt Live request. The setter
   slot is recovered. The traced producers do not call it through a
   PC-relative instruction that names the setter. The next bounded edge
   search resolves Mach-O indirect-symbol and lazy-symbol stubs for the
   constructors and setters as well as those direct PC-relative calls. A
   call to a bound symbol stub can reach the defined function without naming
   its virtual address. Cache index 0 on `L6XTAudioControlMac` stores the
   kernel Brandi service. That producer is useful evidence, and its PODxt
   Live association remains unproven. The recorded userspace builder is still
   unselected. Leave it unsent, including the ruled-out frame above. The
   missing USB-firmware image and unknown installed version also remain open.
2. A live read still needs a separate explicit authorization. No reply has
   been captured. The five recorded fields are not an approved live read.
3. After the service class is resolved, a separate authorization is still
   required before any read. Do not update, reset, switch banks, or install a
   driver.

Until that authorization exists, there is no justified live query. A version
learned later would still not authorize an update.

## Decisions left for review

- Issue #20 stays open. This document is a partial milestone.
- The combined acceptance line stays open because the artifact, the installed
  version, and the PODxt Live service class are still unknown.
- `bcdDevice` 0.01 stays an unexplained descriptor field.
- The recorded transfer stays unsent. Its five fields are not an approved
  live read. The crate keeps a portable data record and does not take IOKit,
  Monkey, or a driver as a dependency.
- No USB-firmware image is claimed to be inside the main `.xtf` container.
