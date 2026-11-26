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

## Authority and unresolved prerequisites

The inactive native adapter consumes an already configured and prepared
raw hardware capture handle. It requires a separate capture authorization
and an independent evidence record tied to that exact open attachment.
The app has no issuer for either. It rechecks prepared state and ALSA
address on that handle, compares route and driver evidence, and checks the
actual negotiated format, geometry and rational clock before start.

A future acquisition procedure must establish the precise ALSA parameter
selection that negotiates `78125/2` on the owner's actual kernel and
libasound. The integer rate interval alone does not identify a request
that succeeds. It also must establish how discovery ties the verified
USB attachment and driver to the acquired handle, and verify the Variax
signal, active Record Send and left/right order. An ALSA card number or
display name alone is insufficient because both can survive or recur
across attachments. [DAW and app capture](../audio-capture-session.md#daw-route-and-in-app-recording)
has the separate server and project-rate constraints.
