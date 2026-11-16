# PODxt Live USB audio routing

Issue #27 records two evidence classes and an in-memory route planner.
Opening a capture or playback stream remains unauthorized. `SelectRecordRoute`
on the PODxt Live role is offline. `StartAudioCapture`, `StopAudioCapture`,
and `OpenAudioStream` stay locked. `LIVE_BACKEND_INITIALIZER` stays `None`.

The offline criteria on #27 were accepted after revised PR #65 merged as
`6318a2e055393d582cbe593aa6d11b4e26eb8bdc`. In-app recording, levels, file saves,
and DAW routing remain #28. This page does not map firmware regions (#13) and
does not identify the USB firmware component or `bcdDevice` (#20).

## Evidence classes

| Class | What was read | What it can support |
| --- | --- | --- |
| GearBox guide | Line 6 GearBox 2.0 Recording Setup Guide, revision 03 | Documented product routing for PODxt, PODxt Pro, and PODxt Live |
| Linux driver source | `sound/usb/line6` at tag `v7.2.15` | Declarations in that source |
| Installed module file | Already-present `snd-usb-pod.ko.zst`, decompressed to a private file and not loaded | Constants stored in that file |
| Connected device | No capture, playback, enumeration, or probe was run | No rate, latency, speed, packet size, or routing measurement |

The guide file is local and uncommitted. SHA-256
`0299bbb1d8c1c485adf33179deef2b2f424080ac8adcd83d687c1076efd029fa`.
Source URL:
<https://line6.com/media/support/toneporthelp/GearBox2_OnlineSupport_Recording_Rev03.pdf>.

Tagged source files, also uncommitted:

| File | SHA-256 |
| --- | --- |
| `sound/usb/line6/pod.c` | `7b07e2e44d983b03abbf78351b09f7c9cdb3a6968d455c8fb14feaf1b73fa7b6` |
| `sound/usb/line6/pcm.c` | `059b24421408f7efb4fe0e73862edcc549193540d82da0c5628bc2734695c233` |
| `sound/usb/line6/driver.c` | `73123eb8ddef699d836ae27269b7f716bbcd3fbf8917c12c658160d123b01d11` |
| `sound/usb/line6/driver.h` | `6bdbf42547971ee457d67a02b742ef342ffc7ca9a675d5f298ef21f6e1c3f8cf` |
| `sound/usb/line6/pcm.h` | `642dc31a8dc7e1aa53d86ee08652731d79ae7bd4f785ab2aa00f929de68cae50` |

The source is GPL-2.0-only, copyright Markus Grabner. Those files are research
inputs. They are not a new runtime dependency, and this crate is not relicensed.

## Documented PODxt Live connections

Printed pages 10–11 describe the PODxt Live jacks:

- Instrument input is processed in the POD. That signal is fed to the analog
  line outs and, over USB, to stereo Record Send 1-2.
- Analog outs and the phones jack carry host playback plus the instrument and
  Aux inputs. Phones follow the analog outs.
- Aux In is a stereo 1/8-inch monitor input. It is heard on the line outs and
  phones. The guide says it does not go to Record Send 1-2.
- USB is the connection the guide uses for GearBox and for the POD as a
  sound-card device. The same pages recommend a separate USB controller and a
  port on the computer. That is guide advice. It was not tested here.

GuitarPort and PODxt devices are described, on printed page 23, as exposing
one stereo Record Send 1-2. TonePort adds Send 3-4. The guide also says audio
software can pick one side of a stereo send for a mono track. The Linux source
below still declares a two-channel PCM device. This planner accepts two
channels and rejects a one-channel stream.

S/PDIF in the guide is TonePort UX2 and PODxt Pro. AES/EBU is PODxt Pro.
The 44100 Hz and 48000 Hz sentences in that digital-clock section are not the
Linux `pod_ratden` value. PODxt Live does not gain those routes here.

## Four Record Send modes

Printed pages 21–22 list four USB signal-routing modes for PODxt, PODxt Pro,
and PODxt Live. The guide names modes 2, 3, and 4. Mode 1 is the first listed
mode. `src/pod_audio.rs` keeps that table:

| Mode | Name | Record Send 1-2 | Processing input | Monitor, as that bullet states it |
| --- | --- | --- | --- | --- |
| 1 | Send Processed Guitar | Processed guitar | Instrument | Processed guitar and host playback |
| 2 | Send Clean Guitar | Unprocessed guitar | Instrument | Processed guitar and software playback |
| 3 | Send Clean Guitar Re-Amp Playback | Unprocessed guitar | Host feeds processing while the clean guitar is still the record send | Processed host audio at the POD outputs |
| 4 | Send Re-Amp Playback | Processed guitar returned to the host | Unprocessed guitar from the host | The bullet does not restate the monitor mix |

Mode 2 is the clean capture the guide says can later be played through mode 3
or 4. Modes 3 and 4 are the re-amp passes. Mode 3's bullet also says the
processed result can be heard or recorded from the analog outputs, and that
the recording program adds latency. No latency number is given. The GearBox
Record knob, +18 dB switch, mono switch, and pan slider appear later in the
guide as GearBox send controls. They are not implemented here, and #28 owns
recording levels.

The Linux `pod.c` source does not name these four modes. Selecting one in
`Session::select_record_route` stores the documented choice and a synthetic
buffer plan. The plan's outbound buffer is empty, including the existing
`PODXT_ROUTING_ENABLE` constant. Nothing is sent.

## Linux driver source

The initial rig still has one USB product and two roles, from the registry in
`src/linux_transport.rs`:

| Role | Match | Interface | `driver_info` | Capabilities in the property row |
| --- | --- | --- | --- | --- |
| Guitar | `snd-usb-variax` row 0 | 1 | 0 | `0x11` = control and control-MIDI |
| POD | `snd-usb-pod` `LINE6_PODXTLIVE_POD` | 0 | 5 | `0x17` = control, control-MIDI, PCM, and hwmon |

Vendor `0x0e41`, product `0x4650`. USB product `0x534D` remains a separate
match and is not a record route. Pocket POD's source row has no PCM
capability and a comment that it has no audio channel. That row is a different
product.

`line6_init_pcm` returns before creating a PCM device when `LINE6_CAP_PCM` is
clear. The guitar row does not set that bit, so the source does not initialize
PCM for it. The POD row sets it, and `pod_init` calls `line6_init_pcm`. The
POD row also sets `LINE6_CAP_HWMON` and does not set `LINE6_CAP_IN_NEEDS_OUT`.

Both property rows list audio endpoints `0x82` and `0x01` and altsetting 1.
Control endpoints differ: guitar `0x86` / `0x05`, POD `0x84` / `0x03`. The
guitar properties field `ctrl_if` 0 is still not interface 1. Listing the same
audio endpoint numbers on the guitar row does not assign that row a PCM device.

`pod_ratden` is the offline timing evidence: numerator 78125, denominator 2.
The integer fields `rate_min` 39062 and `rate_max` 39063 sit beside that knot.
They are not proven OS or device-open parameters. `accept_rate` accepts only
`78125/2` and rejects 39062, 39063, 44100, and 48000. A later recorder must
keep this rational and its source provenance. Live rate negotiation stays
unverified. Format is `SNDRV_PCM_FMTBIT_S24_3LE`,
`bytes_per_channel` 3, and both channel counts are 2, so one interleaved frame
is 6 bytes. The declared byte window is period 64..=8192, buffer at most
60000, and period count 1..=1024. Playback info includes pause. Capture info
does not. The planner additionally requires the period to be a multiple of 6.
The source minimum 64 and maximum 8192 are not multiples of 6, so those two
declared endpoints are rejected.

`pcm.h` sets `LINE6_ISO_PACKETS` to 1 and `LINE6_ISO_INTERVAL` to 1. Its
comment says the Windows driver transmits two frames per packet, and it names
PODxt Pro as an example of a full-speed device where interval 1 means 1 ms.
That parenthetical is not a PODxt Live speed measurement. `driver.c` selects
1000 intervals per second and 2 ISO buffers when `usbdev->speed == USB_SPEED_LOW`,
and otherwise selects 8000 and 16. PODxt Live's speed is not in this source,
so the planner leaves the buffer count unselected. When the control endpoint
descriptor is missing, `driver.c` falls back to interval 10 and max packet 16.
The planner does not adopt that fallback. Max packet size stays undeclared.

`pod_init` contains a comment that registering the sound card at that point
makes PODxt Live display "Invalid Code Error 07", with registration deferred.
That comment is source text. This issue does not record it as an observed
device message. The same file defines a mixer control named "Monitor Playback
Volume". That name is a driver control. It is not the GearBox Record knob, and
this planner does not write it.

`pcm.h` says PCM, software monitor, impulse, and capture-helper traffic share
one capture stream and one playback stream from the device's point of view.
The planner does not acquire any of those stream bits.

## Installed module file

`/lib/modules/7.2.15-1-omarchy/kernel/sound/usb/line6/snd-usb-pod.ko.zst` was
already on this machine. It was decompressed to a private file and not
installed, loaded, bound, or probed. SHA-256
`056d5df834aad3dd9d1a56b6c338705f9ea2cb609ccb28bf0ff67163e66cbb2e`.
The file contains `vermagic=7.2.15-1-omarchy SMP preempt mod_unload`, the
strings `PODxtLive`, `PODxt Live`, and `Monitor Playback Volume`.

Against the `v7.2.15` `struct snd_pcm_hardware` layout (`info`, `formats`,
`subformats`, `rates`, then the rate and channel fields), the file contains
two 80-byte images:

| Field | Playback image at file offset 1944 | Capture image at file offset 2024 |
| --- | --- | --- |
| `info` | `4784387` (`MMAP`, `MMAP_VALID`, `INTERLEAVED`, `BLOCK_TRANSFER`, `PAUSE`, `SYNC_START`) | `4260099` (the same bits without `PAUSE`) |
| `formats` | `1 << 32` (`S24_3LE`) | `1 << 32` |
| `rates` | `1 << 31` (`SNDRV_PCM_RATE_KNOT`) | `1 << 31` |
| `rate_min`, `rate_max` | 39062, 39063 | 39062, 39063 |
| channels | 2, 2 | 2, 2 |
| `buffer_bytes_max` | 60000 | 60000 |
| `period_bytes_min`, `period_bytes_max` | 64, 8192 | 64, 8192 |
| `periods_min`, `periods_max` | 1, 1024 | 1, 1024 |

File offset 2992 holds the little-endian `u32` sequence 78125, 78125, 1, 2,
which is the `pod_ratden` initializer. This is file content. It is not a
listing of ALSA cards and it is not a measurement from a connected POD.
The whole module was not rebuilt and compared instruction by instruction.

## Audio stack conclusion

`snd-usb-pod` in the pinned source already declares PCM for the PODxt Live POD
role. This issue does not propose a custom driver. ALSA, PipeWire, and
PulseAudio devices were not opened, so host routing and latency are still
unknown. The Windows hardware id `USB\VID_0E41&PID_4650` and the macOS audio
personality (`vendor` 3649, `product` 18000, configuration 1, interface 0)
remain the static references already stored in `src/linux_transport.rs`.
Those packages were not loaded. The macOS interface number matches the POD
role's interface in that static record. The #21 Variax interpreter still stubs
ALSA and is not this audio path.

## Synthetic planner

`src/pod_audio.rs` plans a route, a 6-byte S24_3LE frame, and a bounded
in-memory buffer. `BufferPlan` keeps its fields private. `SyntheticBuffer::try_new`
runs the same `plan_buffer` checks and rejects a struct whose stored capacity
or frame count disagrees with that result, including a capacity above 60000.
A period is accepted only when it is a multiple of 6, inside
64..=8192, the period count is inside 1..=1024, and the product is at most
60000. The smallest accepted period is 66 bytes. `66 * 909` fits. `66 * 910`
and `66 * 1024` do not. A full buffer rejects another period. Cancel keeps the
bytes already stored and rejects further writes. The device-open flag stays
false, and the open callback passed to `plan` is not called.

Guitar control and POD audio are returned together. The guitar role still has
no PCM capability. Extension profiles stay unverified, and the guitar role's
`SelectRecordRoute` capability stays unsupported. Workbench HD, JTV-69, and
POD HD500X do not receive a PODxt Live record route.

## Limits

- No USB, MIDI, or audio device was enumerated, opened, or streamed.
- No driver was installed, loaded, bound, or probed.
- Device speed, max packet size, latency, and host-server routing are unknown.
- The four mode names come from the GearBox guide. The PCM window comes from
  the Linux source and the installed module file.
- Passing these tests does not authorize a later connection.
