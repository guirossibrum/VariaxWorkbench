# Rig Check wizard — owner review checklist

[#248](https://github.com/guirossibrum/VariaxWorkbench/issues/248).
Compared on `c69b903` (2026-11-26). This checklist records the current
offline wizard. It does not change the screens.

Screen index and the three open layout questions:
[docs/rig-check/design.md](../rig-check/design.md).
Headless pictures: [docs/rig-check/mockups](../rig-check/mockups/).

## Approval on record

No owner approval of this wizard design is on record at this head.
[design.md](../rig-check/design.md), [the roadmap](../roadmap.md) and
`src/rig_check/gui/mod.rs` each say approval is pending. The 2026-11-24
approval in [architecture-families.md](../architecture-families.md) is the
app-shell delivery. The 2026-11-25 #267 decision releases device and guided
steps for the owner's first real run. Mark each row below. A merge of the
wizard code leaves this issue open until those marks exist.

## What the pictures are

All 14 PNGs are 1180×800. `raster::render` draws them at one pixel per
point (`src/rig_check/gui/raster.rs`). The window asks for the same logical
size (`src/rig_check/bin.rs`). The last picture commit is `c7ede9f`
(2026-11-25). Later commits `f0d255e` (#274) and `11e529d` (#290) changed
choose-mode and step copy. The PNGs still show the earlier copy.

`01-choose-mode.png` says Practice "sends nothing to your rig" and that each
device step "needs the owner's approval in this version." Current Linux
source says: "Practice uses simulated devices. Real rig uses the connected
PODxt Live and Variax. Each device step waits for you to choose Run."
Windows and a Mac build without USB support use their own computer-check
sentences. Those three sentences have no picture.

Step bodies for 0.2 and 1.2, and the title and SEE line for 6.2, also
changed in `11e529d`. None of the 14 pictures is that card.

Most pictures are drawn with `focus_armed = false`, so the focus ring is
absent. `11-step-result.png` leaves focus armed, and Continue is the
highlighted control. Keyboard defaults below come from the headless tests,
which press keys on a live widget tree.

## How to mark a row

Write **Approve** or **Change**, and a short note when the row needs a
change. Screenshot rows stay empty until the owner keeps a private
screenshot. Do not commit those screenshots.

## Checklist

| # | Criterion | Evidence on this head | Still needed | Mark |
| --- | --- | --- | --- | --- |
| 1 | Choose Practice or Real rig, with no mode stored until a choice | `01-choose-mode.png`. Source focuses **Practice**, so Enter selects Practice. The mode field stays empty until that press. | Private screenshot of the current Linux sentence, and of the Mac and Windows sentences. Decide whether Enter-on-Practice is an acceptable keyboard default. | |
| 2 | Start notice: Quit focused, Accept and continue as the other choice. Enter quits. | `02-start-notice.png` shows Quit and Accept and continue. Test `the_start_notice_defaults_to_quit_and_save_needs_the_exact_text` presses Enter and receives `Reply::Accept(false)`. A closed window also refuses the start. | Private screenshot of the focus ring on Quit. The committed PNG hides it. | |
| 3 | Every step shows its card and waits. Nothing starts the next step by itself. | Card buttons are Skip (focused), Run or Start, Stop the run, and Back when a prior result exists. Enter skips (`the_step_card_shows_its_sections_and_enter_skips`). R runs, Q stops (`keyboard_shortcuts_run_and_stop`). A closed window skips. Practice (`the_channel_operator_runs_practice_mode_from_another_thread`) runs 35 cards and 35 results, and the runner waits at each result. | Private screenshot of a step card with Skip focused. | |
| 4 | Step card sections: What it does, What you'll see and hear, Risk, with the gate and the steps it needs | `03-step-card-read.png` (step 1.1) and `04-step-card-live.png`. The same test looks for those three headings plus SEE and HEAR. Risk colour follows the level: read and none use the pass green, live uses amber, save uses the danger colour. The word is also on the card. | Owner look at 03 and 04. Card copy for 0.2, 1.2 and 6.2 is newer than the pictures. | |
| 5 | HEAR and SEE at 34 logical points, readable from the guitar | `pub const BIG: f32 = 34.0` in `src/rig_check/gui/mod.rs`. The theme scales logical points by `pixels_per_point`. The pictures are 1×, so 34 points is 34 pixels there. | Private laptop screenshot and private Retina screenshot, from the playing position. No wizard test sets `pixels_per_point` to 2. | |
| 6 | Running screen: current action, count, elapsed time, Stop, and the "leave the controls alone" notice | `05-running.png` shows step 3.3, "12 of 35 reads", elapsed time and Stop. The sidebar word is RUN. Test `running_shows_latest_action_count_time_and_playing_notice` checks the count, elapsed time, the controls notice, and 15 s / 6 s / 0 s left. | Private screenshot while a count is moving, if the owner wants motion as well as the still picture. | |
| 7 | Get-ready screen before a physical action. The next step still has its own card. | `06-listening.png`. Design notes in [design.md](../rig-check/design.md). `prepare_action` focuses **OK**. Enter starts the countdown. Skip and Stop are the other buttons. A closed window returns Stop. Detecting the movement can finish the current action. The following step still arrives as its own card. | Owner decision on OK-as-Enter. The design notes say focus starts on Skip. This screen focuses OK. Change belongs in `src/rig_check/gui/mod.rs`. | |
| 8 | Didn't see it: Skip focused, Try again, Stop | `07-not-seen.png`. `not_seen` focuses Skip. A closed window returns Stop. | Private screenshot with Skip focused. | |
| 9 | Listening question: Yes, No, Not sure focused, optional note, and the byte-check lines | `08-listening-question.png` is the A/B drop-D question. `question` focuses Not sure. A closed window answers Not sure. | Private screenshot with Not sure focused. | |
| 10 | Save notice names the exact slot and the backup that protects it. Save stays off until the confirmation matches. Skip is the default. | `09-save-notice.png`: slot "Spank 1 (flash slot 14)", backup step 3.4, file `variax-library/14-Spank-1.bin`, first 12 hex characters of the SHA-256. The type-in line repeats `STORE VARIAX Spank 1`. Enter sends `Reply::Typed(None)` in the start/save test. Source enables Save only when the field equals the confirmation. | Private screenshot of Skip focused, of Save disabled, and of Save enabled after the exact text. The headless test does not press a wrong string and then the exact string. | |
| 11 | After a step, the result shows the outcome and the SEE values. Continue is focused and returns to the next card, which still waits for Run. Different… records the displayed text. | `11-step-result.png` and `14-display-difference.png`. Practice asserts the SEE lines, Continue and Stop, and that Enter continues. `different_opens_a_field_and_enter_records_exact_text` checks the field. A reported difference fails the step. | Private screenshot of Continue focused, and of the difference field. | |
| 12 | Progress uses the words PASS, FAIL, SKIP, STOP, N/A and RUN | Visible on `03`, `05`, `10` and `11`. `outcome_tag` in `src/rig_check/gui/mod.rs`. Colour repeats the word. | Owner look at one PASS, one SKIP and the RUN row together. | |
| 13 | Summary: rig state, counts, result preview, Save report… | `10-summary-preview.png`: "Rig Check finished", the rig paragraph, "10 passed · 0 failed · 1 skipped", the report path, Hide results, Save report…, and one preview line for step 2.3. The preview lists only that line because only the 2.3 fixture has a start time. The Practice test is the full-run evidence: 35 passed. Summary focus starts on the preview button. Save report… is the other button. In tests the save dialog returns "Saving is not available in tests." | Private screenshot of a finished Practice summary whose preview lists the run. The Practice test does not draw `Prompt::Done`. | |
| 14 | Back reviews an earlier result and the accepted start notice. Return resumes the pending card and sends nothing. | `12-review-result.png`, `13-review-start.png`. Tests `backspace_reviews_previous_results_and_c_resumes_the_same_card`, `first_card_back_shows_the_already_accepted_start_notice`, `review_shows_the_start_notice_accepted_for_this_mac`, and `clicking_a_finished_step_replays_it_without_continuing_the_active_result`. | Private screenshot of Back and of Return to current step. | |
| 15 | Laptop presentation | The wizard's headless path draws one logical size, 1180×800, at 1 pixel per point. The app shell has a separate headless matrix at 1120×760 and 720×640 (`src/app/shell/tests.rs`). That matrix does not open Rig Check. Sidebar titles already truncate at 1180×800. | Owner screenshots of the 14 states in the real window on the laptop. A 1120×760 or 720×640 wizard check belongs in `src/rig_check/gui/tests.rs`. | |
| 16 | Retina presentation | Shared theme sizes are logical points. Body-canvas and tuning tests in `src/shared/ui/tests.rs` run at `pixels_per_point` 1 and 2. The shell footer test does the same. `raster::render` tessellates at 1.0 and never sets `pixels_per_point`. | Owner screenshots on a Retina display. A wizard test at `pixels_per_point` 2 belongs in `src/rig_check/gui/tests.rs`. | |
| 17 | Screenshots of each state, kept for this review | The 14 repository PNGs are headless draws through `c7ede9f`. They are the pictures to review, with the copy gaps in rows 1 and 4. | Fresh headless redraw from current master, then private owner screenshots of each state including the focus ring. Redraw and PNG replacement belong with `src/rig_check`. | |

## Headless tests read for this comparison

In `src/rig_check/gui/tests.rs`:

- `the_step_card_shows_its_sections_and_enter_skips`
- `keyboard_shortcuts_run_and_stop`
- `the_start_notice_defaults_to_quit_and_save_needs_the_exact_text`
- `the_channel_operator_runs_practice_mode_from_another_thread`
- `running_shows_latest_action_count_time_and_playing_notice`
- `mockups_render_headless` (14 state names, then one 400×300 frame of the third state, colour count only)
- `different_opens_a_field_and_enter_records_exact_text`
- `stop_at_a_result_offers_the_guard_without_offering_another_step`
- `backspace_reviews_previous_results_and_c_resumes_the_same_card`
- `first_card_back_shows_the_already_accepted_start_notice`
- `review_shows_the_start_notice_accepted_for_this_mac`
- `clicking_a_finished_step_replays_it_without_continuing_the_active_result`
- `technical_details_are_hidden_until_opened`

These tests were read at `c69b903`. This checkpoint did not re-run them.

## Outside Lane 2

Lane 2 owns `docs/design/`, `src/shared/ui/`, `assets/ui/`, and the visuals
of `src/app/shell/` after #213. The wizard implementation is
`src/rig_check/`. Reported for that owner, with no edit here:

- Redraw the 14 PNGs from current master so `01-choose-mode.png` and the
  0.2, 1.2 and 6.2 copy match `11e529d`. Add pictures for the Mac and
  Windows choose-mode sentences.
- `prepare_action` focuses OK. The design notes say the safe answer starts
  with focus.
- Choose-mode focuses Practice.
- Extend `mockups_render_headless`, or a neighbour test, to compare a fresh
  1180×800 render with the committed PNG text, to require the exact save
  confirmation before Save enables, to draw the summary and look for
  "Save report…", and to run the wizard at laptop logical sizes and at
  `pixels_per_point` 2.
- [design.md](../rig-check/design.md) still says the pictures were drawn from
  the running code, which was true at `c7ede9f`.

The three open questions already in design.md (progress list, evidence line,
Open folder) stay unanswered here.
