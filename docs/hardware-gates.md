# Rig 1 hardware gates

This page describes the G1–G8 operation boundaries for the PODxt Live and
Variax 600. It is planning information, not authorization. **No gate is
authorized.** Each operation requires the owner's separate, explicit, written
approval of its exact procedure. Approving one gate does not approve another.

## Gate matrix

### G1 — OS-level discovery

- **Operation:** Read the USB IDs, kernel-cached descriptors, and ALSA card list
  already exposed by Linux, as scoped by [issue #125](https://github.com/guirossibrum/VariaxWorkbench/issues/125).
- **Application traffic:** None. This stage sends no app-originated request.
  Kernel startup traffic is separate: the source/object-code trace documented
  in [Linux Variax interface split](linux-transport.md) verifies a
  `snd-usb-variax` startup version-request path. This is static kernel evidence,
  not a live trace or capture. `snd-usb-pod` startup traffic remains
  unverified.
- **System changes:** None by the app. The stock kernel drivers remain as
  installed; no bind, unbind, detach, module patch, or install is part of G1.
- **Prerequisites and tracking:** G1 procedure and evidence: issue #125;
  supporting ledger and driver decision: [issue #112](https://github.com/guirossibrum/VariaxWorkbench/issues/112),
  [issue #113](https://github.com/guirossibrum/VariaxWorkbench/issues/113),
  and [issue #24](https://github.com/guirossibrum/VariaxWorkbench/issues/24).

### G2 — Device identity request

- **Operation:** Request identity from the PODxt Live bridge and attached
  Variax 600, as defined by [issue #126](https://github.com/guirossibrum/VariaxWorkbench/issues/126)
  and the existing ledger in [issue #112](https://github.com/guirossibrum/VariaxWorkbench/issues/112).
- **Application traffic:** One identity request per device through the
  interface-1 control path. The exact request/reply details belong to issue
  #112; this page does not authorize sending them.
- **System changes:** The proposed procedure temporarily detaches the stock
  driver from PODxt Live interface 1, claims that interface, performs the
  bounded operation, releases it, and reattaches the driver. Interface 0 is
  untouched. This is future procedure detail, not permission to detach now.
- **Prerequisites and tracking:** G1 and the owner-approved ledger/procedure
  in issues #112, #114, and #126.

### G3 — Current-preset metadata read

- **Operation:** Read and display current settings only; the result is not a
  backup. See [issue #127](https://github.com/guirossibrum/VariaxWorkbench/issues/127)
  and [issue #115](https://github.com/guirossibrum/VariaxWorkbench/issues/115).
- **Application traffic:** The future sequence is MIDI enable, the two
  current-settings metadata requests, then MIDI disable, as specified by
  issue #112. No other request is specified here.
- **System changes:** The proposed procedure uses the same temporary
  interface-1 detach/claim/release/reattach boundary as G2. It does not include
  preset switching or persistent writes.
- **Prerequisites and tracking:** G2, issue #115's reviewed read path, and the
  owner-approved procedure in issues #112 and #127.

### G4 — Audio capture

- **Operation:** Open an ALSA capture stream for PODxt Live Record Send 1–2 and
  record a take, as planned in [issue #128](https://github.com/guirossibrum/VariaxWorkbench/issues/128).
- **Application traffic:** Opening and reading an audio stream. This requires
  separate explicit authorization for audio capture; no stream has been opened
  by this documentation or its offline tests.
- **System changes:** The planned app behavior does not change routing,
  monitoring, or device settings. The host's actual stream negotiation and
  physical behavior remain unverified; see [PODxt Live USB audio](pod-usb-audio.md)
  and [issue #118](https://github.com/guirossibrum/VariaxWorkbench/issues/118).
- **Prerequisites and tracking:** G1 plus issues #117, #118, and #128, followed
  by its own owner-approved procedure.

### G5 — Full backup read

- **Operation:** Acquire and independently verify a comprehensive backup under
  [issue #23](https://github.com/guirossibrum/VariaxWorkbench/issues/23).
  No verified full backup is currently available. The earlier broader DSP read
  timed out and produced no DSP backup; the captured current-preset settings
  are partial metadata only (see [report](report.md)).
- **Application traffic:** No complete, approved capture procedure is defined
  here. No read is authorized by this gate listing.
- **System changes:** None established for an approved procedure; preservation,
  omissions, and recovery requirements remain for issue #23.
- **Prerequisites and tracking:** Current native prerequisites are the timeout
  diagnosis in [issue #119](https://github.com/guirossibrum/VariaxWorkbench/issues/119),
  the full-backup specification in [issue #120](https://github.com/guirossibrum/VariaxWorkbench/issues/120),
  and the G3 procedure in [issue #127](https://github.com/guirossibrum/VariaxWorkbench/issues/127).
  [Issue #16](https://github.com/guirossibrum/VariaxWorkbench/issues/16) is the
  completed offline source review. Issue #23 tracks backup planning and any
  future capture under its own exact-procedure authorization. Issue #22 remains
  a later G6/G7 write-readiness prerequisite, not a G5 prerequisite.

### G6 — Edit-buffer write

- **Operation:** A future write to device edit-buffer state. This page does not
  specify or authorize a write protocol or implementation.
- **Application traffic:** A device write would be involved; its exact procedure
  must be reviewed and approved separately.
- **System changes:** Device state could change. A verified G5 backup, the
  coefficient-parity decision recorded in [issue #6](https://github.com/guirossibrum/VariaxWorkbench/issues/6)
  and [issue #22](https://github.com/guirossibrum/VariaxWorkbench/issues/22),
  and a separately approved restore procedure are prerequisites.
- **Prerequisites and tracking:** Issues #6, #18, #22, #23, and
  [issue #25](https://github.com/guirossibrum/VariaxWorkbench/issues/25), plus
  separate explicit G6 procedure authorization.

### G7 — Store or restore

- **Operation:** A future persistent preset store or restore. Restore is itself
  a device write; it is not a read-only recovery step.
- **Application traffic:** A device write would be involved. No message or
  procedure is specified or authorized here.
- **System changes:** Persistent device state could change. G7 requires a
  verified G5 backup, the coefficient-parity decision in issues #6/#22, and
  a separately approved restore procedure.
- **Prerequisites and tracking:** Issues #6, #18, #22, #23, and #25, plus
  separate explicit authorization of the exact store/restore procedure.

### G8 — Firmware maintenance

- **Operation:** Future device-specific firmware inspection, update, or
  maintenance planning under [issue #30](https://github.com/guirossibrum/VariaxWorkbench/issues/30).
  G8 is Phase 8 and outside the current phase.
- **Application traffic:** Any live firmware query or transfer would require
  its own exact procedure and authorization. None is approved here.
- **System changes:** Firmware installation would alter persistent device
  software. This page authorizes no download, install, reset, or update.
- **Prerequisites and tracking:** Device-specific validation and backup/recovery
  work in issues #16, #17, #18, #22, and #30, then separate explicit G8
  authorization. No cross-family firmware compatibility is assumed.

## Authorization boundary

These rows do not establish physical safety, recovery capability, or audio
equivalence. Gate-tracking issues #158–#161 cannot be assigned or unlocked
without the owner's sign-off on the corresponding exact procedure. All source
locks remain engaged until that authorization is separately recorded.
