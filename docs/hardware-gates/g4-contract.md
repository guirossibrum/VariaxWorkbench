# PODxt Live Linux capture source contract

[#145](https://github.com/guirossibrum/VariaxWorkbench/issues/145) covers the
driver evidence. [#146](https://github.com/guirossibrum/VariaxWorkbench/issues/146)
uses this contract at the prepared PCM boundary. In-app hardware acceptance
requires the separate owner-approved procedure in
[#128](https://github.com/guirossibrum/VariaxWorkbench/issues/128).

## Evidence and route

The reference is Linux stable `v7.2.15`, commit `497791a`, under
`sound/usb/line6`. The local `pod.c`, `pcm.c`, `driver.c` and headers match
the digests in [PODxt Live USB audio](../pod-usb-audio.md). The linked
[probe source record](g1.md) covers plug-in and startup. The GearBox guide
identifies stereo Record Send 1–2 as the product's recording pair. It gives
the processed and clean instrument-send choices.

The running release string was `7.2.15-4-omarchy` on x86-64. The installed
`snd-usb-pod.ko.zst` and `snd-usb-line6.ko.zst` were inspected as files.
Their decompressed SHA-256 digests were
`820a75715ba00b1202dc5b799768d8f48bda42546ac93cc1a5d138d9342ab90d`
and `aa7b81e59b79213feaafc9a809f3ba05224af115a91ccc606a75f71b6d36c99c`.
The POD module vermagic is `7.2.15-4-omarchy SMP preempt mod_unload`.
Its `srcversion` matches the previously inspected `-1` module
(`A4A2201EC078E5484AE28F9`). Both 80-byte PCM capability records at offsets
1944 and 2024 and the 16-byte rate record at 2992 match byte for byte. The
`-4` common Line 6 module was checked separately through its named PCM
callbacks and call relocations. This confirms the relevant declarations and
the bounded callback behavior below in the installed files. A full
source-to-binary comparison and identification of a loaded module remain open.

| Parameter | Source declaration and adapter requirement |
| --- | --- |
| Attachment | Variax 600 through PODxt Live. The POD driver matches USB `0e41:4650` interface 0. It names capture endpoint `0x82`. The Variax control interface is separate. |
| PCM | `snd_line6_new_pcm` registers device 0 with one capture and one playback substream. Use direct hardware capture on device 0, subdevice 0, for the verified POD attachment. Card number comes from the verified attachment. |
| Format | Two channels, signed packed `S24_3LE`, six bytes per interleaved frame. The adapter requires `RW_INTERLEAVED`. Physical left/right correspondence must be verified for the selected Record Send. |
| Clock | `pod_ratden` declares only `78125/2` Hz. The `39062` and `39063` fields bound the integer rate interval. The adapter requires the exact rational getter result. |
| Buffer | Period 64–8192 bytes, 1–1024 periods, buffer up to 60,000 bytes. The adapter admits whole frames and an exact buffer made up of the reported number of equal periods. |

The POD USB driver uses `KBUILD_MODNAME`. The installed module contains
`snd_usb_pod`, which the adapter maps to the contract spelling `snd-usb-pod`.

## Setup and traffic

| Stage | Statically traced effect |
| --- | --- |
| USB probe | `line6_probe` calls `usb_set_interface` with the POD row's altsetting 1 on interface 0. The existing startup control traffic is documented in [G1](g1.md). |
| Capture open | The `-4` `snd_line6_capture_open` callback calls `snd_pcm_hw_constraint_ratdens` and `line6_pcm_acquire` with its start argument false. It initializes the capture capability record. The acquisition path handles stream buffers and waits for old URBs as needed. |
| Hardware parameters and prepare | Direct libasound `hw_params` uses the PCM HW_PARAMS ioctl and calls prepare. The Line 6 callbacks allocate or acquire buffer ownership, store period bytes, synchronize with prior URBs when needed, and reset internal counters. The traced driver paths have no alternate-setting or USB rate-control call. |
| Start | The direct PCM start ioctl reaches `snd_line6_trigger`. It starts the PCM capture stream and submits isochronous input URBs when required. The capture-helper playback branch is conditional on `LINE6_CAP_IN_NEEDS_OUT`, which the PODxt Live row lacks. Use an unlinked capture handle to avoid grouped stream triggers. |
| Drop and close | The trigger clears PCM stream ownership and unlinks URBs when the underlying stream has no remaining users. Capture close releases its Line 6 acquisition. Libasound close drops a configured handle and frees hardware parameters and the handle. The adapter reports failures from its explicit drop and close calls separately. |

The inspected libasound source and installed headers are version 1.2.16.1.
The direct hardware plugin gets PCM card, device and subdevice through INFO
on the same file descriptor. `snd_pcm_hw_params_current` copies the selected
access, format, channels, rational pair and geometry from the PCM state.
`snd_pcm_hw_params_get_rate_numden` exposes that pair. Libasound's refinement
can supply an integer fallback when a rational pair is absent. The adapter
rejects that fallback along with every other nonmatching pair. It uses
nonblocking mode on its own capture handle. The direct plugin implements
this with the file descriptor's `O_NONBLOCK`; an empty `-EAGAIN` read is
reported as zero frames. PCM status errors terminate the take through the
existing translator.

## Exact-rate request candidate (#118)

The narrowest public libasound rate restriction that retains the POD clock
is `snd_pcm_hw_params_set_rate(pcm, params, 39062, +1)`. The equivalent
request is `39063, -1`. In libasound 1.2.16.1 both calls restrict RATE to
the open, non-integer interval `(39062, 39063)`. A direction of zero
requests the integer itself and excludes the POD clock. A nearest-rate
selection can admit alternatives. The open interval also admits other
fractional rates, so it takes the driver constraint and exact readback to
identify `78125/2`. [Public rate setters][alsa-pcm] ·
[Interval construction and parameter selection][alsa-params].

On the pinned Linux source, POD capture open registers the single
`pod_ratden` knot. `SNDRV_PCM_RATE_KNOT` skips the ordinary integer rate
list. The ratden rule intersects RATE with `(39062, 39063)`, considers
that open one-unit interval a singleton, and supplies `rate_num=78125`
and `rate_den=2`. Libasound's first-rate selection preserves a
refined singleton. During `HW_PARAMS` the kernel requests refinement
again and clears the old pair before the rate rules run. The direct
hardware plugin passes back the ioctl result and libasound saves the
returned pair in the PCM state. The scalar and direction can still
describe a boundary of the interval, and a missing pair can fall back
to an integer over 1. Require the actual pair from the prepared handle.
[Line 6 capture constraint][linux-line6] · [Ratden rule][linux-lib] ·
[Kernel refinement and installation][linux-native] ·
[Libasound interval selection][alsa-interval] · [Direct hardware plugin][alsa-hw].

For a future procedure, the source-derived candidate starts with
`snd_pcm_hw_params_any` on an independently identified direct `hw`
capture handle. Select `RW_INTERLEAVED`, `S24_3LE`, and two channels.
The one-knot driver can already narrow RATE when refining the initial
configuration, so a separate rate setter can be redundant. If spelling
out the restriction, use the single directional setter above and
preserve the returned configuration. Use direct hardware PCM throughout
to avoid plugin conversion. An example geometry for a bounded setup is
625 frames per period via `snd_pcm_hw_params_set_period_size` with
direction zero and a 2,500-frame buffer via
`snd_pcm_hw_params_set_buffer_size`. This gives four equal periods,
3,750 bytes per period and 15,000 bytes total, within the driver and
adapter bounds. The geometry is a fixture choice. The installed handle
must report a valid whole-frame geometry and the exact `78125/2` pair
after `snd_pcm_hw_params` prepares it.

The offline interval fixture exercises the two directional requests and
excludes both adjacent integers and the numerator alone. The fake
prepared-handle fixture admits the exact pair with the example geometry
and verifies close before start on rounded, different fractional, or
missing rational results. These fixtures exercise the rate selection
model and the existing adapter admission. The live refinement and setup
return values remain unobserved.

## Authority and unresolved prerequisites

The inactive native adapter consumes an already configured and prepared
raw hardware capture handle. It requires a separate capture authorization
and an independent evidence record tied to that exact open attachment.
The app has no issuer for either. It rechecks prepared state and ALSA
address on that handle, compares route and driver evidence, and checks the
actual negotiated format, geometry and rational clock before start.

The unresolved negotiation check requires direct approval for a separately
reviewed owner procedure that opens one unlinked hardware capture handle,
refines and installs the candidate, and observes the prepared state and
`snd_pcm_hw_params_current` readback on that same handle. Record the
kernel/libasound versions, actual access, format, channels, geometry and
rational pair, and the close result. Close on any failure or mismatch
and also after successful readback. The negotiation check needs only
open, refine, install/prepare, readback and close, ending before any PCM
start or read. It must establish how discovery ties the verified USB
attachment and driver to the acquired handle. An ALSA card number or
display name alone is insufficient because both can survive or recur
across attachments. The Variax signal, active Record Send and physical
left/right order require separate owner-run signal evidence. In-app
acceptance stays with [#128]. [DAW and app capture](../audio-capture-session.md#daw-route-and-in-app-recording)
has the separate server and project-rate constraints.

[#128]: https://github.com/guirossibrum/VariaxWorkbench/issues/128
[alsa-pcm]: https://github.com/alsa-project/alsa-lib/blob/v1.2.16.1/src/pcm/pcm.c
[alsa-params]: https://github.com/alsa-project/alsa-lib/blob/v1.2.16.1/src/pcm/pcm_params.c
[alsa-interval]: https://github.com/alsa-project/alsa-lib/blob/v1.2.16.1/src/pcm/interval.c
[alsa-hw]: https://github.com/alsa-project/alsa-lib/blob/v1.2.16.1/src/pcm/pcm_hw.c
[linux-line6]: https://github.com/gregkh/linux/blob/v7.2.15/sound/usb/line6/pcm.c
[linux-lib]: https://github.com/gregkh/linux/blob/v7.2.15/sound/core/pcm_lib.c
[linux-native]: https://github.com/gregkh/linux/blob/v7.2.15/sound/core/pcm_native.c
