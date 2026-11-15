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

## Service returned on the version path

The returned pointer is the service object whose version method runs. The
userspace constructor that installs vptr `0x1c1878` has no PC-relative call in
this image; its address appears in a data table. The kernel constructor at
`0x8adc0` installs vptr `0x1c1838`. On a cache miss, the XT control's
`AcquireService(0)` calls that kernel constructor. The userspace constructor
stays off this path. The section below records the hit, the miss, and the
pointer the base call returns.

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

Slot `+0x0c` of `L6DeviceControlBase` is `0x85150`. The body matches Line 6
License Manager `L6DeviceControlBase::AcquireService` at `0x969c0`. It calls
slot `+0x08`. When that call's low bit is set and the index is below 12, it
returns the pointer at `this+8+index*4` and increments the refcount at
`this+0x38+index*4`. A null pointer returns null and status `0x80007301`.
The function does not construct a service. `L6BrandiControlMac` overrides
the slot at `0x9b770`; that override's create path is index `1`, and index
`0` falls through to `0x85150`.

## Description setter slot

The description vtable at vptr `0x1c1598` stores `0x85650` at slot `+0x0c`.
That dword is the only absolute reference to `0x85650` in the Monkey image.
The function adds `0x21c` to `this` and assigns the argument through
`L6DevSmartPtr<L6DeviceControlBase>::operator=`. A search for PC-relative
`call` and `jmp` instructions found none whose displacement names `0x85650`.
The Monkey image has no symbol-table entry for this function, so no
indirect-symbol or lazy stub binds that address. The PODxt Live assignment
is the virtual call in the factory section below.

The five `GetDeviceDesc` call sites use the returned description for its
fields. The one direct caller of the description finder at `0x1482cc` stores
the pointer into another interface object and does not call slot `+0x0c`.
The image also contains 154 `call *0x0c(reg)` instructions. Those sites were
not each attributed to a class. `traced_virtual_call_found` stays false for
those earlier producers. The PID `0x4650` path is a separate producer.

`L6TWX_u.dylib` contains the same class names and its own description vtable.
Its slot `+0x0c` is `0x54240`, and that function also assigns `this+0x21c`.
A PC-relative search in that image found no call that names `0x54240`.
That image also has no symbol-table entry for the setter, so no stub binds
`0x54240`.

## Cache index 0 on one control class

`L6DeviceControlBase`'s constructor at `0x85020` writes zero through all 12
cache slots at `+8` and the matching refcounts at `+0x38`.

`L6XTAudioControlMac` (vptr `0x1c18b8`) overrides slot `+0x0c` at `0x8dd20`.
When the index is below 12 and the refcount at that index is zero, a jump
table handles indexes 0 through 11. Index 0 allocates `0x10` bytes, calls
thunk `0x8ad80`, and stores the allocated pointer at cache index 0. The thunk
calls `0x8adc0`, which installs kernel vptr `0x1c1838`. The only PC-relative
call found for `0x8ad80` is this index-0 arm. The allocation uses the
`__Znwm` stub at `0x169162`. The PID `0x4650` setter path constructs this
control and does not itself run the fill. The return of that cache slot
is [AcquireService index 0](#acquireservice-index-0-and-kernel-getversion).

Two PIC computations store vptr `0x1c18b8`. The PID `0x4650` constructor
body at `0x8c740` pops `0x8c74c` and adds `0x13516c`, and the store is at
`0x8c774`. The other computation is the `lea` at `0x8c893`
(`0x8c88c + 0x13502c`), reached from vtable slot `+0x04` at `0x8c810`.
The PID path reaches the first constructor through `0x8c700`.

The kernel version slot at `0x8b810` does not embed the userspace builder
fields `0xC0`, `0x67`, and `0x0011`. `0x8c040` stays the userspace builder.
It is not the closed PODxt Live query, and the five recorded fields are not
an approved live read.

## AcquireService index 0 and kernel GetVersion

The reviewed PID `0x4650` branch treats the constructed `L6XTAudioControlMac`
as the description control on that static success path. This is not an
observed hardware state.

Monkey `0x8dd20` matches Line 6 License Manager
`L6XTAudioControlMac::AcquireService(unsigned long, long*)` at `0x9f590`.
The License addresses name the bodies. They are not Monkey addresses.

The method locks the word at `this+4`. An index of 12 or more skips the fill.
A nonzero refcount at `this+0x38+index*4` is the hit, at `0x8e67b`, and the
fill is skipped. A zero refcount is the miss. The miss calls
`FindDriverService` at `0x8e720`. The value it returns is overwritten before
the jump table. The store that call performs is
[FindDriverService and selector 6](#finddriverservice-and-sharedxt-selector-6).

Index 0 of the table is `0x8ddd0`. The arm clears `this+8` and calls
`FindServiceNamed` at `0x8e980` with `com_line6_driver_sharedxt`. A non-null
result calls the `__Znwm` stub at `0x169162` with size `0x10`, then thunk
`0x8ad80`. The allocated pointer is stored at `this+8`. The named service is
released through the `_IOObjectRelease` stub at `0x167a3a`. A null name leaves
the slot null. This arm stores the kernel object. It does not return the
pointer, and it does not call `GetVersion`.

The hit and the miss both continue at `0x8e680`, which calls `0x85150`.
That is the base `AcquireService` body. It calls slot `+0x08`. On this
control that slot is `0x8cf10`. Index 0 of its table, at `0x8cf5e`, sets the
low bit. That arm has no product-id compare. When the bit is set and the
cached pointer is non-null, `0x85150` increments the refcount and returns
`this+8+index*4`. A null slot returns null and status `0x80007301`.
Description `AcquireService` returns that pointer. Brandi slot `+0x14` keeps
it at Brandi `+0x10` when the status is non-negative. Slot `0x28` then calls
slot `+0x14` of the kept pointer.

A miss that stored the kernel object therefore hands that object to the
version call. Its vptr is `0x1c1838`. A hit returns the pointer already in
the slot and does not run the constructor again. The userspace vptr
`0x1c1878` is the other Brandi class. The index-0 arm does not construct it.

Kernel slot `+0x14` at `0x8b810` calls `0x8b750`. That body matches
`L6KernelBrandiServiceMac::GetVersionAndStatus`. The License copy is
`0x9d080` for `GetVersion` and `0x9cfc0` for `GetVersionAndStatus`. `0x8b750`
calls the local semaphore helper at `0x8b360` with timeout `0x5dc`. The
non-negative arm of that helper uses the `_sem_trywait` stub at `0x169366`,
the `___error` stub at `0x1691bc`, and the `_usleep$UNIX2003` stub at
`0x16941a`. After a clear failure bit, `0x8b750` calls `0x88350`, which
matches `L6KIOBrandi::GetVersionStatus`. `0x88350` calls the
`_IOConnectCallScalarMethod` stub at `0x1679b0` with selector `6`, no scalar
inputs, and an output count initialized to `2`. A zero return stores a
uint16 from the first output scalar and a byte from the second. `0x8b750`
then calls the `_sem_post` stub at `0x169360`. Each of those stubs is a
six-byte indirect jump named by the Mach-O indirect-symbol table. A null
connection word at the kernel service returns status `0x80007000`.

The kernel slot does not contain `0xC0`, `0x67`, or `0x0011`. Those fields
remain on userspace builder `0x8c040`. They are not an approved live read.
No device reply was captured. The kernel handler that selector 6 reaches is
the next section.

## FindDriverService and sharedxt selector 6

`FindDriverService` at Monkey `0x8e720` matches License
`L6XTAudioControlMac::FindDriverService` at `0x9ff90`. The only direct
caller in each image is the AcquireService miss: Monkey `0x8dd9d`, License
`0x9f60d`. Both calls are `e8 7e 09 00 00`. AcquireService overwrites the
return register. The function's effect is a store.

The Monkey stubs, named by the indirect-symbol table, are `_IOMasterPort`
at `0x167a1c`, `_IOServiceMatching` at `0x167a88`,
`_IOServiceGetMatchingServices` at `0x167a82`, `_IOIteratorNext` at
`0x167a16`, `_IORegistryEntrySearchCFProperty` at `0x167a64`,
`_CFGetTypeID` at `0x16753c`, `_CFStringGetTypeID` at `0x1675cc`,
`_CFStringGetCString` at `0x1675b4`, `_CFRelease` at `0x167560`, `_strcmp`
at `0x1693c0`, and `_IOObjectRelease` at `0x167a3a`. The stack check is
`___stack_chk_fail` at `0x1691ce`.

The match dictionary is the C string `com_line6_audio`. That is the audio
class, not `com_line6_driver_sharedxt`. The search walks that iterator. For
each entry it reads the `UniqueID` property in the `IOService` plane with
options immediate `3`. When the property is a CFString, it copies at most
`0xff` bytes with encoding immediate `0` and compares the copy with the
bytes at `this+0x16b`. A match stores that entry at `this+0x278` and stops.
The store does not release the previous word. A non-match releases the
entry and continues. If nothing matches, `this+0x278` is left as it was.
The iterator is released on the way out.

The XT constructor body at `0x8c740` stores its first integer argument at
`0x8c799` (`this+0x278`) and writes a zero byte at `0x8c7c7`
(`this+0x16b`). License `C2(unsigned int, unsigned long)` does the same at
`0x9e009` and `0x9e037`. License `DriverAdded` calls that constructor at
`0xa6f8a`. The first argument is the `_IOIteratorNext` result taken from
the notification iterator. The second argument is stored at `this+0x294`.
Those License addresses are not Monkey addresses.

`Init` reads `this+0x278`. It copies the `USB Product Name` property into
`this+0x6c`, then `GetUniqueIDfromDevice` copies `UniqueID` into
`this+0x16b`. `Init` does not call `FindDriverService`. On the PID path,
slot `+0x18` is that `Init`, and the version query's miss runs later. The
miss can therefore compare `this+0x16b` with a UniqueID that `Init` already
copied, then replace `this+0x278`. This is a static order. It is not an
observed registry walk.

`FindServiceNamed` at `0x8e980` loads `this+0x278` at `0x8e9bd`. It creates
an iterator in the `IOService` plane with options immediate `1`, using
`_IORegistryEntryCreateIterator` at `0x167a52`. Each child is named with
`_IORegistryEntryGetName` at `0x167a5e` into the buffer at `this+0x298`.
`_strcmp` at `0x1693c0` compares that name with the caller argument. A
match calls `_IOObjectRetain` at `0x167a40`. The loop then releases the
iterator's reference and continues, so the returned pointer is the last
match. No match returns null. The index-0 arm passes
`com_line6_driver_sharedxt`. The object it keeps is the child with that
registry name, or null.

Selector 6 is the other side of the userspace
`IOConnectCallScalarMethod` call. In the signed Line6Audio kext,
`com_line6_driver_sharedxt_UserClient::externalMethod` at `0x272e` indexes
`sMethods` at `0xa2100` while the selector is below 9. Each entry is 24
bytes. Entry 6 points at `sVersionStatusCommand` (`0x2818`), with scalar
input count 0, structure input size 0, scalar output count 2, and structure
output size 0. That action loads the service from the user client at
`+0xd8` and calls `com_line6_driver_sharedxt::VersionStatusCommand` at
`0x13892`. Entry 7 is `sGetIOPVersion`, and `GetIOPVersion` at `0x1370e`
calls `com_line6_driver_kev::GetIOPVersion`. Selector 6 is not that method,
and it is not `L6SP_GetVersion`.

`VersionStatusCommand` reads the USB interface at `this+0xa8`. A null
interface returns `0xe00002cd`. Otherwise it builds an `IOUSBDevRequest` on
the stack: `bmRequestType` `0xC0`, `bRequest` `0x67`, `wValue` `0x0011`,
`wIndex` `0`, `wLength` `3`, and a three-byte buffer. It calls
`com_line6_kusbinterface::DeviceRequest` at `0x1412e` with both timeouts
`0x3e8` and a null completion. `DeviceRequest` does not write those fields.
It forwards the same request pointer through a virtual call. On a zero
return, `VersionStatusCommand` stores byte 1 shifted left by 8, plus byte
0, as the uint16, and stores byte 2 as the status byte. The unsigned kext
has the same bytes at `0x272e`, `0xa2100`, and `0x13892`. No kext was
loaded or installed.

Those five immediates are the fields already recorded for the userspace
builder. The kernel handler is the static site that places them in a device
request. The handler does not name a firmware file, and it does not say
what the device puts in the three bytes. The userspace string "USB Firmware
Version:" remains display text. The fields stay unapproved. No device reply
was captured.

The signed kext personality `PODxtLive` matches vendor 3649, product 18000,
and interface 0 with class `com_line6_audio`. That is the class name
`FindDriverService` iterates. The personality does not by itself limit
selector 6 to PODxt Live, and it still does not give a firmware version.

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
| Line 6 License Manager | 1895712 | `15517a66e0306d9f54920b6d9b53165c9078415d083cdd3bdff9f767c0c49e4b` | named DriverAdded path; setter symbol has no stub entry |
| L6MIDIJava i386 slice | 511468 | `902713ed5e4819021a73ff8a4f2d6a35521a99a96af7253dc53ef153e3789a8f` | named DriverAdded path; `cmpl $0x4650`; setter symbol has no stub entry |

No image was executed, loaded, or installed. The Windows executable is not
confirmation of the macOS setter. The signed and unsigned audio kexts, the
MIDI plugin, and the PODxt userspace libraries contain no matching class-name
strings for these types. That observation does not exclude a stripped binary
or a different-ABI implementation of the same behavior.

## Stub bindings and the PODxt Live control association

The bounded stub search covered `SetDeviceControl` and the description and
XT-audio constructors in the Monkey executable, `L6TWX_u.dylib`, Line 6
License Manager, and the L6MIDIJava i386 slice. Monkey and `L6TWX_u.dylib`
have no symbol-table entry for the setter, so no indirect-symbol or lazy
stub binds those addresses. License Manager defines
`L6DeviceDescription::SetDeviceControl` at `0x96ee0`. The L6MIDIJava i386
slice defines it at `0x1d8da`. Each of those symbols has no direct
PC-relative caller and no indirect-symbol-table entry. The calls inside the
setter bodies are the smart-pointer copy, assignment, and destructor.
Those instructions are calls made by the setter. The setter symbol has no
stub entry.

In the Monkey image, USB PID `0x4650` is an equality at `0x953a6`
(`subl $0x4650` / `je 0x9541d`). That branch allocates `0x418` bytes and
calls `0x8c700` at `0x9544a`. `0x8c700` calls `0x8c740`, which stores vptr
`0x1c18b8` (`L6XTAudioControlMac`). The object is saved at stack slot
`-0x34`. Instruction `0x9555d` calls control slot `+0x18`. When the returned
status has bit `0x80000000` clear, instruction `0x955f1` calls helper
`0x95e10` with the manager and that control. The helper allocates `0x26c`
bytes and calls `0x852f0` at `0x960e9`. That wrapper's only direct caller is
`0x960e9`, and its call at `0x85302` enters `0x85310`, which stores
description vptr `0x1c1598`. The helper writes description offsets including
`+0x108` from manager slot `+0x14`, `+0x4` from slot `+0x2c`, a pointer at
`+0x214`, the word at `+0x104` from slot `+0x34`, and the word at `+0x106`
from slot `+0x38`. The caller then loads the returned object's slot `+0x0c`
and calls it at `0x95616` with the XT-audio control. Slot `+0x0c` of the
description vtable is setter `0x85650`, which assigns the smart pointer at
`+0x21c`. After that call the same function stores a pointer at description
`+0x210` and a pointer at `+0x218`.

The helper has two other direct callers, `0x95065` and `0x967e3`. This pass
did not trace them.

Line 6 License Manager repeats the shape under symbols. `DriverAdded` at
`0xa6cf0` compares PID `0x4650` at `0xa6ee6` with the same `subl` / `je`
encoding and allocates `0x418` bytes for `L6XTAudioControlMac::C1` at
`0x9df70`. Slot `+0x18` is called at `0xa709d`. When bit `0x80000000` is
clear, `GetDescription` at `0xa7950` runs and the virtual call at `0xa7156`
loads description slot `+0x0c`. The setter at `0x96ee0` adds `0x21c` and
assigns the smart pointer. License Manager names XT slot `+0x18` `Init` and
XT slot `+0x0c` `AcquireService` at `0x9f590`. That body matches Monkey
`0x8dd20`. The License address is not a Monkey address.

The L6MIDIJava i386 slice repeats the named shape. `DriverAdded` at
`0x2af7a` compares PID `0x4650` at `0x2b0e6` with `cmpl` / `je`, allocates
`0x418` bytes, and calls `L6XTAudioControlMac::C1` at `0x22ebc`. On a
non-negative result from control slot `+0x18`, it calls `GetDescription` at
`0x2a58a` and then description slot `+0x0c` at `0x2b28a`. The setter symbol
at `0x1d8da` has no direct caller and no indirect-symbol entry. These
addresses belong to the L6MIDIJava slice.

The setter path still does not execute the index-0 fill. The fill and the
returned pointer are recorded in
[AcquireService index 0 and kernel GetVersion](#acquireservice-index-0-and-kernel-getversion).
The five recorded userspace fields stay unapproved for any live read. The
transfer must not be sent.

Edges that remain:

- Helper `0x95e10` callers `0x95065` and `0x967e3` were not traced.
- Symbolicated description consumers `L6BrandiHW`, `CreateUSBMonkeyDevices`,
  `L6MnkyDevice`, and `L6MIDIHW` were located and were not disassembled for
  a setter call in this pass.
- `FindDriverService` at `0x8e720` runs on a cache miss. Its return value is
  overwritten before the jump table. The match store at `this+0x278` is
  recorded in
  [FindDriverService and selector 6](#finddriverservice-and-sharedxt-selector-6).
- A cache hit returns the resident pointer and does not identify a class
  again. A null `FindServiceNamed` result leaves the slot null.
- Missing class-name strings remain a search observation. That observation
  leaves a stripped binary or a different-ABI implementation possible.

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
  was not sent. USB PID `0x4650` constructs an `L6XTAudioControlMac` and,
  when control slot `+0x18` returns without the failure bit, virtually calls
  description slot `+0x0c` at `0x85650` with that control. No symbol stub
  binds the setter. A later `AcquireService(0)` miss stores an
  `L6KernelBrandiServiceMac`, and the base acquire call returns that cache
  slot. Kernel `GetVersion` calls `IOConnectCallScalarMethod` selector `6`.
  Sharedxt selector 6 builds the same five fields in an `IOUSBDevRequest`
  and forwards that pointer. That handler is not `GetIOPVersion` and does
  not identify a firmware image. That is a static branch. The five fields
  stay unapproved.
- `bcdDevice` 0.01: unexplained, and it was not read again.

## Future check

1. Offline: the setter has no indirect-symbol or lazy stub. USB PID `0x4650`
   passes an `L6XTAudioControlMac` to description slot `+0x0c` when control
   slot `+0x18` returns without the failure bit. `AcquireService(0)` on that
   control stores the kernel service on a cache miss, and the base call
   returns that slot. Kernel `GetVersion` reaches `IOConnectCallScalarMethod`
   selector `6` through the stubs named above. Sharedxt selector 6 builds
   the same five fields and forwards that request pointer. It does not
   identify a firmware image. The userspace builder stays unselected. Leave
   the transfer unsent, including the ruled-out frame above. The missing
   USB-firmware image and unknown installed version remain open. A cache hit
   and a null service name are also still open.
2. A live read still needs a separate explicit authorization. No reply has
   been captured. The five recorded fields are not an approved live read.
3. The kernel version slot is identified on the static branch. A separate
   authorization is still required before any read. Do not update, reset,
   switch banks, or install a driver.

Until that authorization exists, there is no justified live query. A version
learned later would still not authorize an update.

## Decisions left for review

- Issue #20 stays open. This document is a partial milestone.
- The combined acceptance line stays open because the USB-firmware artifact
  and the installed version are still unknown.
- `bcdDevice` 0.01 stays an unexplained descriptor field.
- The recorded transfer stays unsent. Its five fields are not an approved
  live read. The crate keeps a portable data record and does not take IOKit,
  Monkey, or a driver as a dependency.
- No USB-firmware image is claimed to be inside the main `.xtf` container.
- The reviewed decision stands: on the static success branch,
  `L6XTAudioControlMac` is the description control. That is not an observed
  hardware state. A successful `AcquireService(0)` miss returns the kernel
  service, and kernel `GetVersion` is the slot the version call uses.
  Sharedxt selector 6 builds the recorded device request. The five fields
  stay unapproved.
- This pass found that kernel request, so reserved issue #21 was not started.
