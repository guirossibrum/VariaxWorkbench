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
