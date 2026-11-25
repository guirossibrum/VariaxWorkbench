# Rig Check wizard design

[#248](https://github.com/guirossibrum/VariaxWorkbench/issues/248) ·
**Owner approval of this design is pending.** The screens below were drawn
headless from the running code (`rig-check --mockups DIR`, a software
renderer: no window, display server, GPU or device). They use the shared UI
theme (`src/shared/ui/theme.rs`) and `section_panel`. No Line 6 art is used.

| Screen | File |
| --- | --- |
| Choose Practice or Real rig (no default mode) | [01-choose-mode.png](mockups/01-choose-mode.png) |
| Start risk notice: Quit (focused) / Accept and continue | [02-start-notice.png](mockups/02-start-notice.png) |
| Step card, read step: What it does / What you'll see and hear / Risk | [03-step-card-read.png](mockups/03-step-card-read.png) |
| Step card, live step (real rig) | [04-step-card-live.png](mockups/04-step-card-live.png) |
| Running, with Stop | [05-running.png](mockups/05-running.png) |
| Listening window: large SEE / HEAR | [06-listening.png](mockups/06-listening.png) |
| Didn't see it: Skip (focused) / Try again | [07-not-seen.png](mockups/07-not-seen.png) |
| Listening question: Yes / No / Not sure (focused) and a note, with what the byte checks proved | [08-listening-question.png](mockups/08-listening-question.png) |
| Save notice naming the slot and the backup; Save stays disabled until the confirmation is typed exactly | [09-save-notice.png](mockups/09-save-notice.png) |
| Summary with rig state, report preview and Save report | [10-summary-preview.png](mockups/10-summary-preview.png) |

## Rules the design follows

- Every step shows its card and waits. Focus starts on **Skip**, so Enter
  never runs a step by accident. Nothing advances into a new step by itself.
- **Keyboard:** Tab moves between buttons, Enter or Space presses the
  focused one. On a step card, R = Run, S = Skip, Q = Stop the run. The
  start notice focuses Quit; the save notice focuses Skip and its Save button
  is disabled until the typed confirmation matches.
- **HEAR / SEE** are 34 px, readable from where the guitar is.
- Status is never colour alone: the progress list shows PASS / FAIL / SKIP /
  STOP / N/A / RUN as words.
- Risk is coloured by level (read green, live amber, save red) and always
  spelled out with the gate and the steps it needs.
- The listening window says that nothing is sent while it is open.
- The runner works on a background thread; the window only answers its
  questions (`gui::ChannelOperator`), so the window, the terminal (`--text`)
  and the tests run the same runner code.

## Open questions for the owner

1. Is a fixed progress list on the left the right place, or should it be a
   compact bar so the card can be larger?
2. Should the step card show the evidence ("Records: ...") or leave it to
   the report?
3. Should the summary offer "Save report..." only, or also "Open folder"?
