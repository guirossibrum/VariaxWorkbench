# Unified rig session

Issue #26 defines one offline Rust session for the initial rig and names the
later profiles without treating them as supported. This is not connectivity,
a live audio stream, effect-parameter editing, or a firmware update. Record-route planning
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
| POD preset open/edit | Unsupported | Offline, bounded tone file, #29 | Unverified |
| Record-route selection | Unsupported | Offline, in-memory plan only | Unverified |
| Enumerate, MIDI, audio, send, flash, reset, bank switch, module install, bind, USB-component read | Locked | Locked | Locked |

Offline guitar open accepts file identity `0x00070004` only. Identity
`0x00070000` is the 500/700 model and is rejected. A PODxt Live firmware
product id is a cross-family rejection, not a conversion. The math gate does
not execute the coefficient engine. Inspection calls the existing FORM/L6FF
inspector and then requires that role's archived product and version. Embedded
MD5 agreement is not authentication. Saving a guitar copy uses the existing
non-overwriting file save and does not upload.

PODxt Live tone files are parsed by the separate librarian in
[POD presets](pod-presets.md), not by the guitar codec. Open and PINF-name edit
are offline for that role. Known asset names and 12-byte parameter records
are reported. No PODxt Live parameter unit or range was recovered, so values
are not edited. Device transmission stays unimplemented.
PODxt Live record-route
selection is the offline planner in [POD USB audio](pod-usb-audio.md): one
documented Record Send 1-2 mode and a synthetic buffer, with capture still
locked. Offline test recording is the recorder in [recording files](recording.md)
and does not open that capture path. Maintenance
beyond offline inspection stays on #30.

The GUI also has a separate offline HD Edit library for statically associated
H5EP/H5ES/H5EB files, including child extraction and 265 selected source-joined
type-1 and numeric controls across 80 descriptor model IDs. This includes 184
primary delay, reverb, and distortion rows; one numeric row remains opaque
because source ESI and EDI widget bounds differ. Type-1 controls use the
editor's normalized integer-percent grid. Numeric controls use the selected
descriptor's integer widget bounds and exact normalized float grid; these
bounds are not physical units. Auxiliary widgets and remaining model/control
rows remain opaque. These values have no established device availability. This does not
change the shared HD500X session capability
from `Unverified`, because the file group and preserved header context do not
establish physical target compatibility.

## Versions

| Profile | Archived container | Installed unit | USB-firmware component |
| --- | --- | --- | --- |
| Variax 600 | `0x03100000` (`3.10`), catalog HEAD only | Unknown | Not applicable to this role |
| PODxt Live | `0x03010000` (`3.01`), catalog HEAD only | Unknown | Unknown. Main flash 3.01 and bcdDevice 0.01 are not this component. Tracked on #20. |
| Workbench HD, JTV-69 | Not established | Not established | Not established |
| POD HD500X | Public name Flash Memory v2.62, release 6260. Public download routes did not supply that image. Separately supplied private firmware-image material has been examined statically but is not linked to release 6260 or admitted as applicable; no container word is registered. | Not established. No live read. | The same notes name USB Firmware 1.04. Private image material is not attributed to that label. |

No product id is invented for an extension profile. Workbench HD, JTV-69, and
POD HD500X are separate families as well as separate profiles. None of them
accepts an original Variax or PODxt file id, and none of them treats another
unverified profile as support.

POD HD500X has a separate driver-text USB match, vendor `0x0E41` product
`0x4159`, recorded in [POD HD500X](pod-hd500x.md). That match is not a firmware
product id and does not move preset, record-route, or firmware operations off
`Unverified`. The Windows INF default of 48000 Hz and 24-bit has
`EngineFormatLock` 0. It is not a PODxt Live record route.

## What stays prohibited

This module has no USB, MIDI, or audio opener and no driver install, load, or
bind. A fake attempt passed to `reject_physical` is not called. `src/usb.rs`
is unchanged. Passing these tests does not authorize a later connection.
