# Device inventory for the staged probe

This is the table gate for issue #93. It is the reviewed name and identity
inventory. It is not the probe application, and it is not permission to open
or write to a device.

`supported_devices.txt` is schema 2. `src/device_inventory.rs` loads it.
A name hit is exact. Two rows with the same name are ambiguous. An alias
carries no id and cannot authorize a settings read.

## What the untracked drafts were

The primary checkout had two untracked drafts. They are not in this commit.

The underscore name list had 216 unique names and no ids or sources. The
hyphenated id list had 176 rows. 172 were marked referenced-only and 4 were
marked partial. Those files are not a union of the recovered material. This
table does not copy an id unless a local source states it.

Of the 176 hyphenated rows, 128 contain at least one hexadecimal id that this
table does not carry. That is 136 distinct absent ids, including Amplifi
`0x5061` and the Helix `0x0021xxxx` block. Those numbers stay out until a
local source is cited.

## Sources

Mac and Windows Workbench `L6ProductConstants` agree. The display-name switch
and the `VARIAX_BASS` constant supply the Workbench product ids. Monkey
`default.cfg`, through `analysis/reports/monkey-hardware-definitions.json`,
supplies nine hardware rows. Committed Rust records supply the PODxt Live USB
id `0x0E41:0x4650`, the separate Variax USB Interface id `0x0E41:0x534D`, and
the POD HD driver-text neighbors in `src/pod_hd500x.rs`. The Workbench HD
static note supplies the JTV name strings and the warning that `0x00070005`
is a family gate, not a JTV-69 model id.

The firmware catalog repeats three of those product ids: PODxt Live
`0x0003000A`, Variax 300/600 `0x00070004`, and Variax 500/700 `0x00070000`.
The inventory test checks the table against those constants and against
`rig::capability::resolve`. A caller claim of `Offline` does not unlock
`EnumerateUsb`.

## Rows that matter for the owner rig

The table has 224 rows: 38 recovered rows and 186 aliases. 182 aliases come
from the 216-name list after names that already had a recovered row were
removed. Four more aliases, `Variax 300`, `Variax 500`, `Variax 600`, and
`Variax 700`, come from the hyphen draft. The engine profile is labeled
"Variax 600", and that label stays an alias.

| Row | Role | Public id | Existing engine | Owner rig |
| --- | --- | --- | --- | --- |
| PODxt Live | bridge | USB `0x0E41:0x4650`, product `0x0003000A` | `PodxtLive` / `EnumerateUsb` / `Locked` | yes |
| Variax 300/600 | guitar | product `0x00070004` | `Variax600` / `EnumerateUsb` / `Locked` | yes |
| Variax 600 | alias | none | none | no |

`ProductProfile::Variax600` is labeled "Variax 600" and its archived product
id is `0x00070004`, the Workbench name "Variax 300/600". The alias "Variax
600" does not carry that id, so a name match on the alias cannot select the
guitar. Monkey calls `0x00070000` "Variax". Workbench calls that same id
"Variax 500/700". The id stays on the Workbench row. "Variax" remains an alias.

`TonePort` is two Workbench ids, `0x00080002` and `0x00080003`, with one
display name. Lookup of that name is ambiguous.

`Tyler Variax` owns `0x00070005` and is family-only. `JTV-69` has no product
id. The engine profile `Jtv69` stays `Unverified`.

`Variax USB Interface` is USB `0x0E41:0x534D` plus Monkey product
`0x00700001`. It is a separate bridge from PODxt Live. The gap is
`bridge-without-guitar`.

PODxt Live and Variax 300/600 are `partial`. Their gaps include
`read-requires-excluded-transfer`, `partial-metadata`, and `not-a-backup`.
The existing live read sends host-to-device interrupt data. This inventory
does not call that path. The synthetic fixture name `in-memory-read-session`
means a later probe may call `read_session::read_current_on` on an in-memory
transport. It does not mean a device was read.

Every other recovered row is `no-settings-codec` and `fixture-missing`, or a
narrower gap such as `family-only`. POD HD500X keeps the committed USB id and
the engine result `Unverified` for `OpenPodPreset`.

## Stage ladder

`src/device_probe.rs` is the consented check. It is not a window and it is not
a package. Live read traffic is part of the design. The check does not send a
preset, a setting change, a bank change, a reset, or firmware.

Each step is shown with what it will do and the risk of doing it. The default
answer is no. Yes covers that step only. The next step appears only when the
previous step is green. The risk rank rises at every step.

| Step | Risk | What it does |
| --- | --- | --- |
| USB identification | 1, low | Match the supplied USB id to the inventory. No interface claim and no message. |
| Device identification | 2 | Parse the identity reply with `midi::Identity::parse`. |
| Inventory check | 3 | Match the product id to one guitar row. The bridge name stays separate. |
| Settings read | 4, high | Run `read_session::read_current_on` and name the guitar, the body model, and `string1_tuning`. |
| Change check | 5, highest | Ask the user to change `string1_tuning` on the instrument, then read again. The report says whether that value changed. |

The change check names the model and the setting from the first read. The user
changes that setting on the instrument. The app reads again. An unchanged
value is still a completed read: the report says the reading stayed the same.

`usb::read_current` is called only while `LIVE_ACCESS_LOCKED` is set. That
call returns before a device is opened and before a message is sent. The named
model and setting in the current tests come from the existing in-memory
engine. The preset name is not copied into the report.

## Pinned: audio capture

Opening the audio interface of the device this run identified is a low-risk
step, and it is still a risk. It needs its own consent. It does not send a
preset, a program change, a routing change, or firmware.

The same capture tool belongs in the application for every device family.
PODxt Live is the only family with an in-memory Record Send planner today.
`StartAudioCapture` and `OpenAudioStream` stay locked, including for that
planner. Workbench, Workbench HD, JTV-69, and POD HD500X do not have a
recovered capture route. A PODxt Live route is not reused for them.

This probe waits for that family work. The audio step is not part of the
stage ladder in this change.

## Device-check window

`variax-device-probe` is the check. `--synthetic-self-check` runs one failure
and one partial success without a window. With the `gui` feature, the window
is 960 by 720 and titled "Device check". The left rail shows the five control
steps. **No, stop** is the first button. **Yes, continue at my own risk**
applies to the current step only. The next step stays without buttons until
the current step is green. The monospace text is the report that Save writes.
A save error is `SaveFailed` and the message does not include the path.

The footer says this build does not open USB, MIDI, or audio. Audio capture
remains the pinned step above.

## What this file does not do

No USB, MIDI, or audio device is opened. `LIVE_ACCESS_LOCKED` is unchanged.
A green in-memory read does not mean a listed product works on hardware.
Passing these tests does not write to an instrument. This session does not
dispatch the workflow.
