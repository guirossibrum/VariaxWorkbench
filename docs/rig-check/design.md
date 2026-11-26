# Rig Check wizard design

[#248](https://github.com/guirossibrum/VariaxWorkbench/issues/248) ·
**Owner approval of this design is pending.** The screens below were drawn
headless from the running code (`rig-check --mockups DIR`, a software
renderer: no window, display server, GPU or device). They use the shared UI
theme (`src/shared/ui/theme.rs`) and `section_panel`. The examples use
generated demo data from the wizard's headless fixtures. The [owner review
checklist](../design/rig-check-wizard-review.md) records the source revision
and the remaining review gaps.

| Screen | File |
| --- | --- |
| Choose Practice or Real rig | [01-choose-mode.png](mockups/01-choose-mode.png) |
| Start risk notice: Quit (focused) / Accept and continue | [02-start-notice.png](mockups/02-start-notice.png) |
| Step card, read step: What it does / What you'll see and hear / Risk | [03-step-card-read.png](mockups/03-step-card-read.png) |
| Step card, live step (real rig) | [04-step-card-live.png](mockups/04-step-card-live.png) |
| Running, with the current read, progress bar, elapsed time and Stop | [05-running.png](mockups/05-running.png) |
| Get-ready countdown before a physical action | [06-listening.png](mockups/06-listening.png) |
| Didn't see it: Skip (focused) / Try again | [07-not-seen.png](mockups/07-not-seen.png) |
| Listening question: Yes / No / Not sure (focused) and a note, with what the byte checks proved | [08-listening-question.png](mockups/08-listening-question.png) |
| Save notice naming the slot and the backup; Save stays disabled until the confirmation is typed exactly | [09-save-notice.png](mockups/09-save-notice.png) |
| Summary with rig state, results preview and Save report | [10-summary-preview.png](mockups/10-summary-preview.png) |
| Step result with the channel name, a display check, Continue and Stop | [11-step-result.png](mockups/11-step-result.png) |
| Reviewing an earlier result | [12-review-result.png](mockups/12-review-result.png) |
| Reviewing the accepted start notice | [13-review-start.png](mockups/13-review-start.png) |
| Recording a display mismatch | [14-display-difference.png](mockups/14-display-difference.png) |

## Laptop and Retina views

The screen index above uses 1180×800 logical points at 1×. These Practice
examples use the same narrower 1120×760 logical viewport at both scales.
The Retina PNGs are rendered at 2 pixels per point (2240×1520 pixels).

| Practice screen | Run heading | Catalogue reference | Laptop 1× | Retina 2× |
| --- | --- | --- | --- | --- |
| VDI disconnect and reconnect card | Step 16 of 35 | Catalogue ID 6.1 | [Laptop](mockups/laptop-vdi-card.png) | [Retina](mockups/retina-vdi-card.png) |
| VDI Get ready before unplugging | Step 16 of 35 | Catalogue ID 6.1 | [Laptop](mockups/laptop-vdi-get-ready.png) | [Retina](mockups/retina-vdi-get-ready.png) |
| PODxt expression-pedal sweep | Step 21 of 35 | Catalogue ID 7.4 | [Laptop](mockups/laptop-expression-pedal.png) | [Retina](mockups/retina-expression-pedal.png) |
| PODxt channel recall and return | Step 29 of 35 | Catalogue ID 4.7 | [Laptop](mockups/laptop-podxt-channel-recall.png) | [Retina](mockups/retina-podxt-channel-recall.png) |

**Step X of Y** follows the runner's visit order. The sidebar follows that
same order and prefixes each row with its catalogue ID. The catalogue ID is
the stable reference used in the report and by Resume. The examples show
catalogue ID 7.4 at Step 21, then catalogue ID 4.7 later at Step 29.
The fixture includes the expected firmware skips for PODxt Live
3.01 (2.7) and Variax 3.10 (3.7), which do not take run positions.

The 6.1 card describes listening for disconnect and reconnect through the
PODxt Live and verifying the returning identity against 3.1. Get ready
asks the player to wait for the countdown before unplugging the guitar end
of the VDI cable. The action cue says to leave the PODxt Live USB cable
connected. The reconnect phase gives its own countdown and cue. Identity
checks may run between listening windows with the VDI cable held still.

## Owner release

The 2026-11-25 decision (#267) releases device and guided steps for the
owner's first real run. The start and save notices remain, with the extra
notice before unverified Variax evidence. A failed step makes the remaining
run read-only. Every applicable earlier step must pass before a save card
appears. Audio streams remain unavailable. A passing owner run approves
testing the app's matching writes.

## Rules the design follows

- Every step shows its card and waits. Focus starts on **Skip**, so Enter
  never runs a step by accident. Nothing advances into a new step by itself.
- **Keyboard:** Tab moves between buttons, Enter or Space presses the
  focused one. On a step card, R = Run, S = Skip, Q = Stop the run. The
  start notice focuses Quit; the save notice focuses Skip and its Save button
  is disabled until the typed confirmation matches.
- After a step runs, the result screen shows its outcome and the values
  named on the **SEE** card. A failure includes the reason and any known
  hint. Display checks use Continue or Enter when the displayed value matches.
  **Different…** opens a field for the exact displayed text. The terminal uses
  **[d]**. A reported difference fails the step and blocks later live edits and
  saves. The summary lists differences first, with the expected and reported
  values together. With no typed difference, the report says “not reported
  different”.
- **Continue** starts with focus on the result screen. It returns to the next
  card, which still requires its own Run choice. **Stop** ends the run and
  offers the closing rig check in a normal run.
- **Repeat this step** on a current read, guided or live result asks for
  Start or Run again. Live repeats perform the complete action and verified
  return. Saves have no repeat control. The report keeps the earlier
  attempts, and the latest outcome governs later steps. Repeated backups get
  their own evidence folders.
- The running screen puts the current action and elapsed time up front.
  Repeated work shows completed and total counts. Confirmation waits, the
  silent-timeout hold and guided windows show time left. During device work,
  leave the guitar selector, volume and tone knobs, and PODxt footswitches and
  knobs alone. Playing is fine.
- Guided cards say **Start**. Rig Check waits for **OK** before an action
  countdown. You have 10 seconds
  to get to the device, with **+10 seconds** available. The action screen shows
  what to do, the remaining time and any decoded movement. Rig Check accepts
  a detected movement early and gives the action 60 seconds after the
  countdown. **Continue** checks the evidence so far. If the requested change
  is missing, **Try again** repeats the action with its existing checks,
  **Skip** records a skip, and **Stop** ends the run. Listening comparisons
  use the same prompts and the player presses Continue after playing.
- **Back** or **Backspace** opens the previous recorded result from a step
  card or result. At the beginning, Back opens the accepted start notice
  for review. The sidebar also opens recorded results.
- **Return to current step** resumes the pending card or result. Press
  **C** to return while reviewing, or Enter when the Return button has focus.
  History navigation preserves the original outcomes and sends no device
  messages. The save prerequisites stay in effect.
- The terminal offers **[b] Back** and **[c] Continue / Return to current
  step**, and uses the same result text. Raw bytes and full hashes stay
  in the saved report. The summary preview shows the plain step results.
- **HEAR / SEE** are 34 logical points. Readability from the playing
  position is part of owner review.
- Status is never colour alone: the progress list shows PASS / FAIL / SKIP /
  STOP / N/A / RUN as words.
- Risk is coloured by level (read green, live amber, save red) and always
  spelled out with the gate and the steps it needs.
- Guided instructions name the physical control and the value or preset the
  player will see verified. The terminal and wizard use the same Follow checks.
- The runner works on a background thread; the window only answers its
  questions (`gui::ChannelOperator`), so the window, the terminal (`--text`)
  and the tests run the same runner code.

## Open questions for the owner

1. Is a fixed progress list on the left the right place, or should it be a
   compact bar so the card can be larger?
2. Should the step card show the evidence ("Records: ...") or leave it to
   the report?
3. Should the summary offer "Save report..." only, or also "Open folder"?
