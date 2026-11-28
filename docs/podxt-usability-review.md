# PODxt Live usability review

[#268](https://github.com/guirossibrum/VariaxWorkbench/issues/268) compares the
same synthetic **Studio Signal** `.l6t` in Line 6 Edit 3.06 and Variax Workbench.
Both apps opened the file offline. Line 6 Edit ran in a copy of the local Proton
prefix with a synthetic device tree, a private network namespace, and a
temporary X display. The comparisons and original application screenshots
are local review artifacts.

## What makes the editor slower to use

| Task | Line 6 Edit 3.06 | Workbench | Proposal |
| --- | --- | --- | --- |
| Compare settings across blocks | The 734 × 603 editor puts amp, A.I.R., dynamics, effects and EQ on one screen. | The stage gives one block most of its space. Comparing delay feedback with reverb mix requires switching blocks and remembering the other value. | Keep the stage and selected-block editor. Add an optional compact overview with model, state and key values for each block. |
| Find the amp and its values | Amp, cab, mic, Room and the six amp knobs share one area. Values use percentages for the amp and appropriate units for timing, frequency and gain. | The six large amp knobs are easy to target. Room sits in a disclosure and the signal chain doesn't summarize values. | Bring Room beside cab and mic. Keep percent on amp knobs and use Hz, ms and dB where applicable. The captured original shows 50%, so a 0–10 conversion would change the reference scale. |
| Identify what is being edited | The title names the open tone, and the librarian separates file and device selections. | The status, tone name, and source are on separate lines. A file copy and the current edit buffer can be hard to distinguish at a glance. | Put **Playing 1B**, **File**, or **Library backup** alongside the tone name. Put the unsynchronized mark on both the playing header and its saved position. |
| Browse and transfer | Tone Locker keeps the file browser, selection info and PODxt banks side by side. The transfer buttons label the direction and whether the scope is one item or a bundle. | Tone Locker opens below the editor. Its file and device lists have less vertical room, and transfer actions compete with editing controls. | Give the librarian a workspace of its own, with two persistent lists, a small selection summary, and explicit source and destination for transfers. |
| Select a channel and send an edit | The librarian offers Load and Edit for a selected device entry. The code routes that operation through `L6EditorApp.OnLoadDevicePatch`. The transfer controls in the offline capture are disabled. | Live editing changes what a channel click does. Read, recall, send to the playing buffer and store are distinct actions, but the channel row doesn't explain the destination before a click. | Separate **Preview** from **Recall on POD**. Keep **Send to edit buffer** and **Store to slot** next to the playing context, with the existing gates and confirmations. |

The [stage and librarian mockups](podxt-268-mockups.html) propose these changes
for owner review. The offline captures establish layout and labels. The
channel-selection flow is code-derived, and the comparison did not exercise
a live recall, send, or store. CustomTone was a separate online service.

## Unsaved playing tones on PODxt Live

This is a static trace of the Windows and macOS Line 6 Edit 3.06 Java
implementations for [#242](https://github.com/guirossibrum/VariaxWorkbench/issues/242)
and [#268](https://github.com/guirossibrum/VariaxWorkbench/issues/268).

- On its initial refresh, `L6SharcToneEditor.OnPostRefresh` queries the current
  program and the hardware edited flag. `L6SharcToneDevice.QueryCurrentProgramNumber`
  uses global 17 and `QueryEditedState` uses global 28. If the flag is set, the
  editor reads the playing buffer and uses `L6PatchManager.SetCurrentPatchData`
  to import its model data into the local channel copy and mark that copy
  unsynchronized. The import call does not send the buffer or store a channel.
- Edits in the editor also mark the channel's local copy unsynchronized.
  `L6Editor.SetCurrentPatch` and `L6PatchManager.SendEditBufferOrPC` select clean
  destinations by Program Change and send unsynchronized destinations from the
  local copy into the temporary playing buffer. The incoming program event
  path can reapply that local copy after a physical channel selection.
- `L6EditorApp.Quit` warns about unsynchronized local tones. Explicit
  synchronization controls write saved memory.

On Build 565, the owner verified that Workbench relaunch reads the PODxt's
existing unsaved playing buffer. Restoring saved 2A on the PODxt removed
the unsynchronized marker and restored ordinary app channel switching. The
Workbench fix tracks the hardware playing buffer, saved slot and in-session
draft separately. It targets a verified switch that preserves the outgoing
tone as a draft where possible. Native automatic draft replay remains under
the separate owner acceptance procedure.


### Cached clean recall checkpoint

The Windows and macOS 3.06 decompilations agree. `L6SharcToneDevice.QueryGlobalValue`
sends command 87 (hex `57`) with the global ID and waits up to 100 ms. Global
17 supplies a program index. Global 28 supplies the device's edited flag.
`QueryEditedState` treats a missing reply as false. Saved-slot freshness
requires independent evidence. The synchronized destination path in
`L6PatchManager.SetCurrentPatch` calls
`L6SharcTonePatchManager.ChangeDeviceProgram`, which sends a single Program
Change on the tone bank. PODxt Live uses offset 0. Unsynchronized in-memory
copies follow a separate edit-buffer path in the original application.

Workbench's scripted path starts with a checked addressed download of all 128
tones, while a simulated observer covers the same attachment. The proof binds
each addressed dump to the corresponding save epoch and uninterrupted
observation window. Each accepted sidebar selection sends exactly six
requests in order. It sends identity, global 17, global 28, Program Change,
global 17, and global 28.
The reply waits are bounded. The existing 400 ms channel-load interval is
used to receive MIDI before probing the destination. The app publishes the
cached bytes after validated channel and clean replies and checked release.
Observed relevant saves, coverage gaps, replacement, and a changed source
take the addressed checked path before any switch. After a Program Change,
failed validation requires a checked refresh, and failed checked release
requires reconnecting. Exact outgoing edits and preexisting session drafts
use the existing draft workflow.

Native enablement requires independent evidence for each route.

- **Linux ALSA raw MIDI.** `src/app/detection/identity.rs::listen` opens a
  read-only descriptor per detection batch and closes it at the end. It yields
  the passive rig lock for foreground work, including Variax traffic. The PODxt
  library reader uses `with_verified_platform_link`; Linux checks USB path
  and address there and uses a separate ordinary raw MIDI link. Recall uses
  `RawMidiAttachment::select_podxt_live`, a borrowed `ScopedRawMidiLink`,
  and its checked `finish`. The patch boundary is the passive reader,
  addressed bank reader and checked recall adapter. A shared ordered receiver
  must preserve save observation across those ownership changes or prove
  replay/catch-up for the exact USB generation. The addressed reader must
  collect its slot proof while covered and report checked close. Merely
  reopening the same ALSA endpoint leaves the gap.
- **macOS direct USB MIDI, interface 0.** `Follow::pause` ends the save
  window before foreground work. When Follow starts its own checked tone
  read, `follow::listen` also closes the observation window because the
  reply reader does not forward saves. The checked foreground adapter
  `with_verified_workbench_link` revalidates registry ID, path and address
  under the rig lock and checks interface release. The patch boundary is
  macOS Follow, its handoff to the addressed reader and foreground recall,
  and the interface-0 receive dispatcher. The same generation needs one
  ordered stream of notices and replies across yields or verified replay
  across gaps.

For each platform, verify the actual global reply framing, global 17's
independent post-switch index, and global 28's behavior for clean selection,
physical and app edits, edits restored to their original values, saves, and
concurrent changes. Verify delivery of addressed save notifications and the
behavior of the receiver on loss, yield, and replacement. After a
coverage gap, establish freshness with replay and catch-up or acquire
new addressed proof under a covered link. The scripted tests establish
the app protocol and fallback behavior. Native shortcut admission stays closed
and the physical Linux and macOS runs remain pending.
