# Rig 1 hardware gates

This page describes the G1–G8 operation boundaries for the PODxt Live and
Variax 600. **G1 is authorized by the owner's direct permission (2026-11-22)**,
for the procedure recorded in [the G1 ledger](hardware-gates/g1.md) only. That
permission takes effect only in the [#158](https://github.com/guirossibrum/VariaxWorkbench/issues/158)
build, which opens only the G1 lock. Until that build exists,
`hardware_gates::G1_OS_DISCOVERY_LOCKED` stays true. **G2–G8 are not
authorized.** Each gate needs its own direct permission. Permission for one
gate does not authorize another. The owner does not authorize a gate by signing
a GitHub issue.

## Gate matrix

### G1 — OS-level discovery

- **Operation:** Read the USB IDs, kernel-cached descriptors, and ALSA card list
  already exposed by Linux, as scoped by [issue #125](https://github.com/guirossibrum/VariaxWorkbench/issues/125).
- **Application traffic:** None from the app. This stage sends no app-originated
  request. Kernel startup traffic is recorded in the G1 ledger from the public
  Linux v7.2.15 source. It was not observed live.
- **System changes:** None by the app. The stock kernel drivers remain as
  installed; no bind, unbind, detach, module patch, or install is part of G1.
  The G1 lock stays true until the #158 build.
- **Prerequisites and tracking:** Authorization record:
  [G1 ledger](hardware-gates/g1.md) ([issue #131](https://github.com/guirossibrum/VariaxWorkbench/issues/131)).
  Gate issue: [#125](https://github.com/guirossibrum/VariaxWorkbench/issues/125).
  The build that opens only this lock: [#158](https://github.com/guirossibrum/VariaxWorkbench/issues/158).
  Supporting ledger and driver decision: [issue #112](https://github.com/guirossibrum/VariaxWorkbench/issues/112),
  [issue #113](https://github.com/guirossibrum/VariaxWorkbench/issues/113),
  and [issue #24](https://github.com/guirossibrum/VariaxWorkbench/issues/24).

### G2 — Device identity request

- **Operation:** One identity request for the Variax 600 through the PODxt
  Live, as proposed for review in [the G2 ledger](hardware-gates/g2.md).
  [#126](https://github.com/guirossibrum/VariaxWorkbench/issues/126) should be
  narrowed to that single request. PODxt Live identity stays separate.
  The ledger does not authorize G2.
- **Application traffic:** The captured exchange is one universal identity
  request and one Variax 600 3.10 reply. The following settings read is G3
  and is not part of this procedure. The published PODxt Live 3.01 reply has
  no recorded request in the 2026-11-14 files. See
  [the G2 ledger](hardware-gates/g2.md). This page does not authorize sending
  the request.
- **System changes:** The proposed procedure detaches the stock driver from
  PODxt Live interface 1 before it claims that interface. Release runs only
  after a successful claim. Reattach runs whenever detach succeeded, including
  after a failed claim, a failed alternate setting, or a cancel before the
  send. Interface 0 is untouched. This is future procedure detail, not
  permission to detach now. The live form is an identity-only path: it needs
  G2 and does not need G3. Device check lists that path, and still hides it
  while G2 is locked. The consent screen names the gates the build can
  run. Step 2's Yes button says it sends one Variax identity request. No
  stays the default. Step 2 is offered only after a fresh step 1 pass for
  one PODxt Live with `snd-usb-pod` on interface 0 and `snd-usb-variax` on
  interface 1. Close wins before that exchange starts. Once Yes has started,
  the window cannot cancel it. After the driver is given back, Linux's own
  `snd-usb-variax` startup traffic is not a second request from the app.
  The step 2 report is saved before Device check starts over for the
  follow-up step 1. The owner test card is section 10 of the G2 ledger.
  #114 stays open.
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
equivalence. G1's permission is the record in the G1 ledger. It does not open
`G1_OS_DISCOVERY_LOCKED` until the #158 build is merged. G2–G8 are not
authorized, and their source locks stay engaged.
