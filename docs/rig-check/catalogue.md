# Rig Check test catalogue

**Catalogue version: 1.6 (2026-11-25).** Every report quotes this version.
Epic [#243](https://github.com/guirossibrum/VariaxWorkbench/issues/243) ·
catalogue [#244](https://github.com/guirossibrum/VariaxWorkbench/issues/244).

**Status: released for the owner's first real run (2026-11-25, #267).**
The device and guided steps, including the Variax firmware evidence branch,
have separate open approval switches and still require their hardware
gates and Run confirmations. Audio streams and steps 8.2–8.5 remain closed.
Practice runs against simulated devices. A passing owner run authorizes
testing the app's matching writes. Distribution to others needs a separate
owner decision.

The code keeps the same list in `src/rig_check/catalogue.rs`. A test checks
that every step ID and the version here match the code.

## Rules every step follows

These are the owner's requirements from #243 (2026-11-25).

1. **Every step explains what it does and asks before it runs.** No
   exceptions: computer checks, reads, guided actions, live changes and
   saves all show their card and wait for **Run**. The default answer is
   **No** (do not run). The person may choose **Run**, **Skip** or **Stop**.
2. **Nothing moves into a new step by itself.** When a guided action is
   detected (for example the selector was turned), that completes the
   current step. The next step still shows its own card and asks.
3. **A failure closes later live edits and saves.** Every applicable earlier
   step must pass before a save card is offered. The report names the step
   that stopped a later save.
4. **The rig ends where it started.** Each device step that changes
   anything attempts a verified return. The final step (6.2) checks both
   devices. An unknown store outcome or the evidence branch ends with USB
   release only.
5. **Saves are never retried automatically.** If a save confirmation does
   not arrive, Rig Check stops sending, keeps the rig untouched for
   2 minutes, and tells the person what to check (the silent-timeout
   procedure from #177).
6. **Guided steps use the editor's Follow listeners.** After setup they
   listen for the action. PODxt Follow re-reads the tone after a channel
   change, and the Drive check reads back the decoded knob value.
7. **Listening questions are answerable by ear** (owner, 2026-11-25, #260).
   They ask only about obvious differences (body family, 12-string doubling,
   tuning, sitar), always as an A/B against a sound just heard, changing one
   obvious thing. Subtle settings (one pickup off, tone capacitor, pot
   values) are proven by byte checks, and the step says so instead of asking.
8. **The report is written after every step** so a crash still leaves
   evidence. No network access. Personal data is redacted. Backups go in a
   separate, optional evidence folder.

## Risk levels

| Level | Meaning |
| --- | --- |
| **None** | Nothing is sent to any device. |
| **Read** | Rig Check asks the device questions. Nothing on the device changes. Some reads switch presets or channels and switch back (stated on the card). |
| **Live** | The sound you are playing changes for a moment and is put back. Nothing is saved to the device. |
| **Save** | A stored slot on the device is written. It is written with its own bytes, so nothing should change, but a power loss or fault during the write can leave the slot unusable. |

## Plain-language risk notices

### Start notice (shown once, before any step; Accept / Quit, Quit by default)

> **Before you start.** Rig Check tests your PODxt Live and Variax with the
> same code the editor uses. Each step tells you what it will do and waits
> for you. Nothing runs until you choose **Run**.
>
> Most steps only ask the devices questions. Some steps briefly change the
> sound you hear and put it back. Saving steps come later and have their
> own warning.
>
> Your PODxt Live and Variax are old and cannot be replaced. Keep both
> powered and connected until Rig Check says it has finished. Do not touch
> the selector, knobs or footswitches unless a step asks you to.
>
> Rig Check writes a text report on this computer. It does not use the
> internet. Your user name, computer name and serial numbers are removed
> from the report.

### Save notice (shown before each save step; names the slot and the backup)

> **This step saves to your {device}.** It writes slot **{slot}** with the
> bytes it already holds, so the sound should not change. The backup taken
> in step {backup_step} protects it: **{backup_file}** (checksum
> {backup_sha256}).
>
> If the power fails or a cable is pulled during the save, that slot can
> stop working until it is saved again from the backup. Rig Check never
> repeats a save by itself. If the device does not confirm the save, keep
> everything powered and untouched for 2 minutes and follow the steps on
> screen.
>
> Type **{confirmation}** to save, or choose **Skip**.

## Step list

Columns: **Needs** = steps that must have passed first. **Gate** = project
hardware gate ([hardware-gates.md](../hardware-gates.md)). **Proof** = the
owner-rig issue or card that proved the procedure.

| ID | Step | Needs | Gate | Risk | Proof |
| --- | --- | --- | --- | --- | --- |
| 0.1 | Computer and operating system | – | – | None | – |
| 0.2 | USB devices and drivers on this computer | – | G1 | None | #239, g1.md |
| 0.3 | Engine check (no device) | – | – | None | #238 sweep, host fixtures |
| 1.1 | See the PODxt Live on USB | 0.2 | G1 | None | #213 |
| 1.2 | Open and give back the guitar interface | 1.1, 2.1 | G1, G2 | Read | #239, [rebind card](../variax-driver-rebind-test-card.md) |
| 2.1 | PODxt Live identity and firmware | 1.1 | G2 | Read | #213, [identity card](../automatic-identity-owner-test-card.md) |
| 2.2 | PODxt Live current tone | 2.1 | G3 | Read | #220, [reads card](../podxt-live-reads-test-card.md) |
| 2.3 | PODxt Live one stored channel | 2.2 | G5 | Read | #220 |
| 2.5 | PODxt Live full library backup | 2.2 | G5 | Read | #220, #222 |
| 2.7 | Compatibility check for other firmware versions (PODxt Live) | 2.1 | None, uses captured identity | Read | #277 |
| 3.1 | Variax identity, model and firmware | 1.2, 2.1 | G2 | Read | #213 |
| 3.2 | Variax current settings | 3.1 | G3 | Read | #176, #225 |
| 3.3 | Variax full current program (35 reads) | 3.1 | G3 | Read | #234, [full-read card](../reso5-full-read-test-card.md) |
| 3.4 | Variax all 60 presets (backup), and back | 3.3 | G5 | Read (switches preset and back) | #236, [backup card](../variax-backup-test-card.md) |
| 3.5 | Editor: Read unmodified from device | 3.4 | G5 | Read (switches preset and back) | #238, [selection card](../variax-any-selection-test-card.md) |
| 3.7 | Compatibility check for other firmware versions (Variax) | 2.1 | G1, G2, G3 | Read | #271 |
| 4.1 | PODxt Live: change one control, read back, undo | 2.2 | G6 | Live | #221, [edit card](../podxt-live-edit-test-card.md) |
| 4.2 | Variax: send the current preset's own bytes to the working buffer | 3.3 | G6 | Live (no audible change) | #176, [Reso-5 card](../reso5-test-card.md) |
| 4.3 | Variax: listen to a test model, then return | 4.2, 3.4 | G6 | Live | #238 listen-list, [listen card](../variax-listen-list-test-card.md) |
| 5.1 | Variax: save a preset's own bytes back to its slot | 3.4, 4.2 | G7 | Save | #177, #238 4c, [store card](../reso5-store-test-card.md) |
| 5.2 | PODxt Live: save a channel's own bytes back to it | 2.5, 4.1 | G7 | Save | #222 stage B (owner run pending), [store card](../podxt-store-test-card.md) |
| 5.3 | Variax: save a temporary output trim and restore it | 5.1 | G7 | Save | #238 4b/4c |
| 6.1 | VDI cable unplug and replug noticed | 3.1 | G2 | Read | #235 |
| 6.2 | Rig back to its starting state, driver given back | – | G1–G3, G6 | Read (switches back if needed) | #239, #238 |
| 7.1 | Turn the Variax selector to a named preset | 3.4 | G3 | Read | #202 Follow, #238 |
| 7.2 | Sweep the Variax volume and tone knobs | 3.1 | G3 | Read | live-control-midi (CC 7, CC 79) |
| 7.3 | Switch the PODxt Live to a named channel | 2.5 | G3 | Read | #220 |
| 7.4 | Move the PODxt Live pedal | 2.1 | G3 | Read | #220 (CC 7 / CC 4) |
| 8.1 | Audio device and formats (no sound) | 0.1 | – | None | #264 |
| 8.2 | Play a test tone (not in this version) | 8.1 | G4 | Live | #265 |
| 8.3 | Record 10 seconds while you strum (not in this version) | 8.1 | G4 | None | #265 |
| 8.4 | Measure latency (not in this version) | 8.2 | G4 | Live | #265 |
| 8.5 | Record each test model (not in this version) | 4.3, 8.3 | G4 | Live | #265 |

**Run order.** Computer, identity, reads and guided checks come first.
Step 2.1 precedes the guitar interface check and every Variax step. These
are followed by live steps 4.1–4.8, saves 5.1–5.4, then the closing guard 6.2.
The conditional evidence branch 3.7 can end the normal sequence after 3.1.
Every applicable earlier step must pass before a save is offered. A failure
also closes any remaining live edits. See [coverage](coverage.md).

**Unverified devices.** A Variax other than a 300/600 on firmware 3.10, or a
PODxt other than a PODxt Live on firmware 3.01, is flagged **unverified
device**. For a Variax 300/600 with unverified firmware, Rig Check offers
step 3.7 after 3.1 and ends the normal sequence. If 2.1 fails, all Variax
steps are skipped. If a PODxt Live reports unverified firmware, the only
Variax step offered is 3.7 because the relay was verified on PODxt 3.01.
On an unverified PODxt Live, Rig Check records its identity result and offers the read-only Variax evidence branch; regular PODxt checks are skipped. When the
guitar and relay report the tested firmware, the compatibility check shows
SKIP. This automatic skip permits live edits and saves to continue.

## Additional coverage (1.2)

| Step | Action | Player cue |
| --- | --- | --- |
| 2.6 | PODxt editor Get, packs and globals | Leave the controls alone. See the current channel, tone and available globals. |
| 4.4 | Variax trim difference and return | Play the same passage. The card names the louder or quieter test level. Hear the original again afterwards. |
| 4.5 | Variax alternate-tuning transition and return | Play the low E string. Hear a lower pitch and then the original tuning. |
| 4.6 | PODxt whole-tone send | The current sound stays loaded. See a verified whole-tone read-back. |
| 4.7 | PODxt recall and return | Hear the next saved channel, then the starting tone. |
| 4.8 | PODxt tuner | Pluck an open string. See tuner readings and the tuner turn off. |
| 5.3 | Variax temporary save and restore | Confirm the named slot. See both stores verified. |
| 5.4 | Variax two-slot Sync | Confirm both named slots. See each verified, then the original preset. |
| 7.5 | Six PODxt Live tone knobs | Turn Drive, Bass, Mid, Treble, Presence and Channel Volume all the way up and down in one listening window. |

The Follow checks use the editor's listeners. 6.1 follows disconnect and
reconnect. 7.1 and 7.2 check the named preset and both full knob sweeps.
7.3 waits for a matching channel update and checks the tone Follow reads back.
7.4 checks pedal movement. 7.5 reports each measured range, whether the endpoints 0 and 127 arrived, and whether the sweep reached ≤20 and ≥107. Run 7.5 before 7.3 so the footswitch check can return the PODxt to its starting tone before live and save steps. The final rig guard remains a separate check.

## Step cards

Each card shows **What it does**, **What you'll see / hear**, **Risk**, and
what the report records. **HEAR** and **SEE** lines are shown in large text
so they can be read from where the guitar is.

### 0.1 Computer and operating system
- **What it does:** Reads the operating system name and version, kernel,
  processor model and features (for example FMA), how this copy of Rig Check
  was built (target, compiler, maths profile) and the app version.
- **You do:** Nothing. **SEE:** a short summary. **HEAR:** nothing.
- **Risk:** None. Nothing is sent to any device.
- **Evidence:** OS, kernel, architecture, CPU model and flags, build target,
  rustc version (if recorded at build time), maths profile.

### 0.2 USB devices and drivers on this computer
- **What it does:** Reads what the operating system already knows about
  connected USB devices: IDs, port, speed, which driver holds each PODxt
  interface (`snd-usb-pod`, `snd-usb-variax`) and the device permissions.
  Also the libusb version.
- **You do:** Nothing. **SEE:** the list. **HEAR:** nothing.
- **Risk:** None. No device is opened.
- **Evidence:** device list (serial numbers hashed), drivers, permissions.

### 0.3 Engine check (no device)
- **What it does:** Builds reference guitar models on this computer and
  compares every one of the 1,092 program words with the expected result.
  Differences are listed by word, bit and sound part (pickups, body, EQ,
  pitch/tuning, settings, checksum).
- **You do:** Nothing, or choose a folder with your own reference models.
- **SEE:** Results for the reference models and a count of the synthetic
  cases.
- **Risk:** None.
- **Evidence:** one line per model, full word/bit detail when different.
- **Built-in references** cover 48 synthetic native cases, a synthetic
  preset, and a generated drop-D audition preset. The audition uses the
  built-in model library and is offered if its rendered SHA-256 matches
  the Workbench oracle. No factory preset or owner file is needed.
- **Your own references** (optional, `--engine-folder DIR`) override the
  bundled audition and the definitions used in the trim and tuning edits.
  Supply a private folder with `models.tsv` (your local model manifest, as for
  `variax-workbench render-preset`) and `NAME.vwm` files, each with
  `NAME.packed` (expected program) or `NAME.sha256` (its hash only). This is
  how the owner's Stratocaster and test models A-D are checked without
  putting them in the repository.

### 1.1 See the PODxt Live on USB
- **What it does:** Looks for one PODxt Live in the USB list from 0.2 and
  notes its port and speed.
- **You do:** Make sure the PODxt Live is on and connected. **SEE:** "PODxt
  Live found on port …".
- **Risk:** None. Failure texts: device absent, more than one PODxt Live,
  permission denied.
- **Evidence:** vendor/product ID, port path, speed, address.

The USB list cannot show firmware. Step 2.1 checks the PODxt Live
firmware, and step 3.1 checks the Variax firmware.

### 1.2 Open and give back the guitar interface
- **What it does:** Opens the PODxt Live's guitar interface (interface 1),
  closes it again, and checks that Linux gave it back to its driver
  (`snd-usb-variax`). If it was not given back it waits and retries once
  (#239).
- **You do:** Nothing. **HEAR:** nothing.
- **Risk:** Read. No message is sent to the guitar.
- **Failure texts:** device absent, permission denied, interface busy,
  driver not returned.
- **Evidence:** open/release result, driver binding before and after.

### 2.1 PODxt Live identity and firmware
- **What it does:** Asks the PODxt Live who it is. A lost first reply packet
  is retried once.
- **HEAR:** nothing.
- **SEE:** The reported PODxt model and firmware, with verification status.
- **Risk:** Read.
- **Evidence:** identity reply in hex, model, firmware, verified/unverified.

### 2.2 PODxt Live current tone
- **What it does:** Reads the tone you are playing (168 bytes).
- **SEE:** the tone name. **HEAR:** nothing changes.
- **Risk:** Read. This is also the rig's starting tone for step 6.2.
- **Evidence:** dump in hex, SHA-256, tone name, amp and effects.

### 2.3 PODxt Live one stored channel
- **What it does:** Reads one saved channel by memory location. The sound
  you are playing does not change.
- **Risk:** Read. **Evidence:** dump, SHA-256, shape check.

### 2.5 PODxt Live full library backup
- **What it does:** Reads all 128 tones, 107 amp setups and 64 FX setups
  (299 entries) and saves them to the evidence folder. Rig Check compares
  channel 1A with the independent read in 2.3 and reports a content difference
  separately from a lost reply. It reads the playing tone again at the end
  to verify it stayed the same. The verified backup is required for step 5.2.
- **SEE:** The number of entries read out of 299.
- **Risk:** Read. **Evidence:** entry count, SHA-256 per entry and of the
  manifest, before/after current tone.

### 2.7 Compatibility check for other firmware versions (PODxt Live)

Compares the identity already captured in 2.1 with firmware 3.01 and lists
each differing firmware byte. It sends no additional request. The normal
PODxt Live checks require 3.01.

### 3.1 Variax identity, model and firmware
- **What it does:** Asks the guitar who it is, through the PODxt Live. A
  lost first packet or a silent first try is retried once.
- **HEAR:** nothing.
- **SEE:** The reported Variax model family and firmware, with verification
  status.
- **Risk:** Read.
- **Evidence:** identity reply, model (300/500/600/700), firmware, VDI link
  answering, verified/unverified.

### 3.2 Variax current settings
- **What it does:** Reads the 192 bytes of settings for the preset you are
  playing (name, pickups, tuning, controls).
- **Risk:** Read. **Evidence:** two replies in hex, decoded settings.

### 3.3 Variax full current program (35 reads)
- **What it does:** Reads all 1,092 words of the sound the guitar is playing
  in 35 small reads of 32 words (the PODxt Live drops replies larger than
  about 256 bytes). A damaged reply is retried up to twice, 100 ms apart.
- **SEE:** The number of chunks read out of 35 and the checksum result.
- **Risk:** Read. This is also the rig's starting program for step 6.2.
- **Evidence:** per read: request, reply, retries, lost bytes; SHA-256 of
  the full program and its checksum word.

### 3.4 Variax all 60 presets (backup), and back
- **What it does:** Reads all 60 stored presets (1,092 words each) and saves
  them to the evidence folder. It finds the preset you started on (the one
  matching 3.3), switches back to it and checks all words.
- **You do:** Leave the physical selector in its starting position during the
  sweep. One exact byte match identifies the starting slot. For no match or
  multiple matches, Rig Check asks you to move the physical selector away and
  back. The selected slot comes from the guitar's program-change report.
  The saved sounds are used during the checks. The captured starting buffer,
  including unsaved edits, is restored and verified in 6.2.
- **HEAR:** the sound changes 60 times, then comes back. About 4 minutes.
- **SEE:** The number of presets read out of 60 and whether the guitar
  returned to its starting preset.
- **Risk:** Read. **Evidence:** 60 SHA-256 values, manifest, the starting
  preset, return check.
- The full backup comes first (before 3.5) because a single preset
  read can only switch back once the starting preset is known.

### 3.5 Editor: Read unmodified from device
- **What it does:** Uses the same shared read as the Variax editor's Read
  unmodified from device and Sync revert. It selects one other stored
  preset, compares all 1,092 words with the 3.4 backup, and checks the return
  to the starting preset.
- **HEAR:** the sound changes for a few seconds and comes back.
- **SEE:** Which preset was read, whether it matches the backup, and whether
  the guitar returned to its starting preset.
- **Risk:** Read (preset switch and back). **Evidence:** program, backup
  comparison, return check.

### 3.7 Compatibility check for other firmware versions

After 3.1 identifies a Variax 300/600 on firmware other than 3.10, or
2.1 identifies a PODxt Live on firmware other than 3.01, an extra risk
notice explains the diagnostic read. Accepting it opens the
regular step card and its Run confirmation. Rig Check captures the
identity, two settings replies, and 35 current-program replies. The
report includes raw replies and available comparisons to the 3.10
message sizes, version bytes and checksum word. The guitar remains on
the current preset. An unverified Variax skips the other Variax steps. A tested PODxt Live
continues through its regular checks.
The Rig Check and device-crate Variax evidence approvals are open for the owner.

### 4.1 PODxt Live: change one control, read back, undo
- **What it does:** Changes one control of the tone you are playing (the
  stomp on/off), reads the tone back, puts the control back and checks that
  the whole tone matches the start. Nothing is saved.
- **HEAR:** the stomp switches for a moment.
- **SEE:** The stomp setting read back and whether the starting tone was
  restored.
- **Risk:** Live. **Evidence:** before, after and restored dumps, differences.

### 4.2 Variax: send the current preset's own bytes to the working buffer
- **What it does:** Sends the 1,092 words read in 3.3 back to the guitar's
  working sound and reads them back. The sound should not change. Nothing
  is saved and no save command is sent.
- **HEAR:** no change (a very short drop-out is possible).
- **Risk:** Live. **Evidence:** upload hash, read-back hash, word differences.

### 4.3 Variax: listen to a test model, then return
- **What it does:** Plays a test model in the working buffer, asks what you
  hear, then switches back to the starting preset and checks all words.
- **Listening:** "Play the original first." Then **Yes / No / Not sure**
  plus an optional note, only about obvious differences (body family,
  12-string, tuning, sitar), A/B against the sound just heard.
- **SEE:** Your answers for each test model and whether the guitar returned
  to its starting preset.
- **Risk:** Live. Nothing is saved; no save command is ever sent.
- **Evidence:** upload and read-back hashes, answers next to byte checks.

### 5.1 Variax: save a preset's own bytes back to its slot
- **Shown after the save notice.** Uses the backup from 3.4.
- **What it does:** Checks the slot still matches the backup, sends its own
  program, waits 100 ms, sends one save command, waits up to 15 s for one
  confirmation, then reads the slot back (35 reads) and compares it with the
  backup. Never repeats the save. Slots in flash 40–59 are not offered (the
  spare sector cannot be checked through the PODxt Live).
- **HEAR:** a short drop-out, then the same sound.
- **Risk:** Save. **Evidence:** pre-check, store messages, confirmation
  time, read-back comparison; on no confirmation, the 2-minute procedure.

### 5.2 PODxt Live: save a channel's own bytes back to it
- **Shown after the save notice.** Uses the backup from 2.5 and the typed
  confirmation (for example **STORE PODXT 17A**).
- **What it does:** Reads the slot and requires all 160 tone bytes to match
  the backup, stores the same bytes, waits for the PODxt's confirmation,
  then reads the slot back.
- **Risk:** Save. **Evidence:** pre-check, store messages, receipt, read-back.

### 5.3 Variax: save a temporary output trim and restore it

After the exact save confirmation, store the temporary trim variant and verify the
slot. Restore the original verified backup and confirm it reads back.

### 5.4 Variax: Sync two presets' own bytes

Confirm the two named slots. Each is prechecked against the backup, stored
once using its own bytes and verified. See each result, then the verified
return to your starting preset.

### 6.1 VDI cable unplug and replug noticed
- **What it does:** Asks you to unplug the VDI cable from the guitar, waits
  until the guitar stops answering, asks you to plug it back, and reads the
  identity again.
- **You do:** unplug, wait for the screen, plug back in.
- **HEAR:** the guitar goes silent while unplugged.
- **SEE:** The observed disconnect and reconnect, and the guitar firmware
  after reconnecting.
- **Risk:** Read. **Evidence:** times of loss and return, identity after.

### 6.2 Rig back to its starting state, driver given back
- **What it does:** Reads the PODxt Live tone and the Variax program again
  and compares them with the start. If the channel or preset differs, it
  switches back and checks again. Then it gives the guitar interface back
  to the operating system and checks the driver.
- **SEE:** What matched the starting state, and the guitar driver status if
  checked.
- **Risk:** Read (switches back if needed). Always offered last, also after
  **Stop**.
- **Evidence:** start and end hashes, any switch-back, driver binding.

### 7.1 Turn the Variax selector to a named preset
- **What it does:** Opens a listening window. Nothing is sent while you
  turn the selector. When the guitar reports the new preset, Rig Check reads
  it and checks it matches the backup.
- **You do:** turn the selector to the named preset.
- **Timeout:** "Didn't see it: Try again / Skip".
- **SEE:** The preset the guitar reported and the return to the starting
  preset.
- **Risk:** Read. **Evidence:** program change seen, matching read.

### 7.2 Sweep the Variax volume and tone knobs
- **You do:** turn the volume knob all the way down and up, then the tone
  knob. Rig Check watches for CC 7 and CC 79.
- **Risk:** Read. **Evidence:** min/max seen per knob.

### 7.3 Switch the PODxt Live to a named channel
- **You do:** press the footswitch for the named channel. Rig Check checks
  the new tone against the library backup, then asks you to switch back.
- **SEE:** The channel and tone the PODxt Live reported and the return to
  the starting channel.
- **Risk:** Read. **Evidence:** program change seen, tone match.

### 7.4 Move the PODxt Live pedal
- **You do:** rock the pedal heel to toe. Rig Check watches CC 7 (or CC 4
  in wah mode).
- **Risk:** Read. **Evidence:** range seen.

### 8.1 Audio device and formats (no sound)
- **What it does:** Lists what the operating system says about the PODxt
  Live's audio device: channels, sample rates, formats, the audio system
  (ALSA/PipeWire/JACK, WASAPI/ASIO, Core Audio) and the default devices.
  No audio stream is opened.
- **Risk:** None. **Evidence:** the listing.

### 8.2–8.5 Audio play and record (not in this version)
- Designed in [audio-steps.md](audio-steps.md). They need the owner's
  separate approval to open audio streams (AGENTS.md) and show "not
  available in this version" until then.

## Evidence mapping

| Step | Owner-rig evidence |
| --- | --- |
| 0.2, 1.1, 1.2, 6.2 | #239 driver return (card: variax-driver-rebind), #213 automatic identity |
| 2.1–2.5, 7.3, 7.4 | #220 PODxt live reads (owner runs pending per card) |
| 3.1 | #213 identity, G2 owner run (build 12) |
| 3.2 | #176 / #225 current-buffer reads |
| 3.3 | #234 35-chunk full read |
| 3.4–3.5 | #236 full backup, #238 program-change verification and switch-back |
| 4.1 | #221 PODxt live edit with undo, first G6 write (stomp CC 25) |
| 4.2 | #176 Reso-5 own program to the edit buffer |
| 4.3 | #238 listen-list A–D (passed 2026-11-25) |
| 5.1 | #177 Reso-5 store, #238 rung 4c restore |
| 5.2 | #222 stage B (approval pending) |
| 6.1 | #235 reconnect re-attaches |

## Changes

- 1.5 (2026-11-25): removed the standalone bank read (2.4). The full
  library backup (2.5) reads all banks. Other step numbers stay the same.
- 1.4 (2026-11-25): named the editor read in 3.5 and removed the repeated bank sweep (3.6).
- 1.3 (2026-11-25): clarified where firmware is checked and added result navigation (#276).
- 1.2 (2026-11-25): added app-message and Follow coverage and strict save prerequisites.
- 1.1 (2026-11-25): added the conditional Variax firmware evidence step (#271).
- 1.0 (2026-11-25): first catalogue.
