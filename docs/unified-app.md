# Unified rig session

Issue #26 defines one offline Rust session for the initial rig and names the
later profiles without treating them as supported. This is not connectivity,
a live audio stream, POD editing, or a firmware update. Record-route planning
is offline and is described in [POD USB audio](pod-usb-audio.md).

## Initial rig

One physical PODxt Live USB device hosts two logical roles:

| Role | Profile | Kernel match | Interface | `driver_info` |
| --- | --- | --- | --- | --- |
| Guitar | Variax 600 | `snd-usb-variax` row 0 | 1 | 0 |
| POD | PODxt Live | `snd-usb-pod` row for `0x4650` | 0 | 5 |

Both rows use vendor `0x0e41` and product `0x4650`. Interface 1 is the Variax
match. The properties field `ctrl_if` 0 is not that interface. USB product
`0x534D` remains a separate kernel match and is not a role of this rig.

`Session::initial_rig` copies that static description. It does not enumerate
USB. `LIVE_BACKEND_INITIALIZER` is `None`.

## Capability matrix

`resolve` ignores any caller or session claim. A physical operation is `Locked`
for every profile, including Workbench HD, JTV-69, and POD HD500X.

| Operation | Variax 600 | PODxt Live | HD, JTV-69, POD HD500X |
| --- | --- | --- | --- |
| Open, browse, or save a guitar file | Offline | Unsupported | Unverified |
| Family-math identity gate | Offline, original id only | Unsupported | Unverified |
| FORM/L6FF inspection | Offline, archived 300/600 target | Offline, archived PODxt Live target | Unverified |
| POD preset open/edit | Unsupported | Planned, #29 | Unverified |
| Record-route selection | Unsupported | Offline, in-memory plan only | Unverified |
| Enumerate, MIDI, audio, send, flash, reset, bank switch, module install, bind, USB-component read | Locked | Locked | Locked |

Offline guitar open accepts file identity `0x00070004` only. Identity
`0x00070000` is the 500/700 model and is rejected. A PODxt Live firmware
product id is a cross-family rejection, not a conversion. The math gate does
not execute the coefficient engine. Inspection calls the existing FORM/L6FF
inspector and then requires that role's archived product and version. Embedded
MD5 agreement is not authentication. Saving a guitar copy uses the existing
non-overwriting file save and does not upload.

POD preset bytes are not parsed with the guitar codec. PODxt Live record-route
selection is the offline planner in [POD USB audio](pod-usb-audio.md): one
documented Record Send 1-2 mode and a synthetic buffer, with capture still
locked. Offline test recording is the recorder in [recording files](recording.md)
and does not open that capture path. POD editing stays on #29. Maintenance
beyond offline inspection stays on #30.

## Versions

| Profile | Archived container | Installed unit | USB-firmware component |
| --- | --- | --- | --- |
| Variax 600 | `0x03100000` (`3.10`), catalog HEAD only | Unknown | Not applicable to this role |
| PODxt Live | `0x03010000` (`3.01`), catalog HEAD only | Unknown | Unknown. Main flash 3.01 and bcdDevice 0.01 are not this component. Tracked on #20. |
| Workbench HD, JTV-69, POD HD500X | Not established | Not established | Not established |

No product id is invented for an extension profile. Workbench HD, JTV-69, and
POD HD500X are separate families as well as separate profiles. None of them
accepts an original Variax or PODxt file id, and none of them treats another
unverified profile as support.

## What stays prohibited

This module has no USB, MIDI, or audio opener and no driver install, load, or
bind. A fake attempt passed to `reject_physical` is not called. `src/usb.rs`
is unchanged. Passing these tests does not authorize a later connection.
