# Rig Check guided prompts

[#248](https://github.com/guirossibrum/VariaxWorkbench/issues/248).
Design checkpoint for the owner. The common guided chrome cited here
is the text on `dac532129971e18571bc2b4065ffcbbe76caf985`. That chrome
is unchanged through `c5cdc3f323f801d5ac90dabc1c5994f668912975`, the
branch base, except the 6.1 cable sentences, which Lane 1 replaced
before this base. #249 on this base changes where Mac reports are
stored. It does not change these prompts, the resume target, or the
6.1 countdown. It maps that wizard and terminal, and proposes four changes
for a later implementation: when the act-now sentence appears, how
step 7.3 names a footswitch and step 7.4 names the expression pedal,
how an accidental Skip is retried from review, and how a continuation
preview and the progress list separate the resume target, a
prerequisite reread, and a kept expected firmware skip. This file does
not change Rust sources, gates, resume selection, or the fourteen
headless pictures. It is not owner approval.

The screenshot checklist remains
[rig-check-wizard-review.md](rig-check-wizard-review.md). Screen index:
[docs/rig-check/design.md](../rig-check/design.md).

## Lane split

| Issue | Owns in this round | Stays out of this file's proposal |
| --- | --- | --- |
| #248, this checkpoint | Common guided chrome: prepare, Get ready, and NOW. Footswitch and expression-pedal wording. Review-only Back, plus the explicit Skip recovery described for #276. Continuation preview and progress lines that name the resume target, a prerequisite reread, and a kept expected firmware skip. | 6.1 detection, identity fallback, and cable-state sentences. How a continuation chooses its target, which steps it keeps, which it reads again, live identity revalidation, and the #255 checksum refusal. Application code. |
| #260, Lane 1 | Offline 6.1 state machine: unplug, detected, reconnect, and restoration cues. Automatic advance on supported evidence. Bounded identity checks when the Mac relay gives no notice. No-response distinct from a failed transport. The same verified identity on return. Tests for notice and no-notice, no prompt during a read, timing, timeout, and Stop. | The common Ready / NOW chrome, footswitch and expression-pedal copy, and Skip recovery. Those stay here and with #276. |
| #276 | After this design is marked, the implementation of Back, the explicit retry, dependency recomputation, and the Practice path Skip → Back → explicit retry → current card. | A second way for Back or Return to start a device action. |

#260's acceptance still holds for every guided step: never ask for a
physical action while a read is running, and let detection advance so
the player does not walk back to the computer. This proposal keeps
Continue as an optional check. It does not add a prompt during a read.

#248's acceptance is unchanged. Mockups are not approved by this file.
The new screens below need a mark before any picture or code. Keyboard
access stays: Tab moves between buttons, and Enter or Space presses the
focused button.

## Evidence

Private report, not committed:

`backups/verification/298-owner-runtime.9AEclz63/rig-check-20261126T164933Z.txt`

Rig Check 0.9.0 at `dac5321`, catalogue 1.6, real devices, finished
2026-11-26. The protocol trace inside that file stays private. Plain
outcomes used here:

- 6.1 passed on attempt 1. The result lines say the guitar stopped
  answering and later answered with its starting identity, and that no
  connection notice was decoded. Those cues belong to Lane 1 on #260.
- 7.3 passed on attempt 2. Attempt 1 failed with "No 168-byte
  edit-buffer dump arrived." The passing attempt records the change to
  the named channel as already in the expected state, and records the
  return to the starting channel during the get-ready countdown.
- 7.4 passed on attempt 1. It records an expression-pedal sweep during
  the action window. The catalogue has one such sweep.
- The summary counts skipped-by-you as 0. This run does not contain an
  accidental Skip. The keyboard default that causes one is still present.
- 3.2 passed. 3.3 failed the checksum check. The summary counts two
  expected firmware skips. 2.7: the PODxt Live reports firmware 3.01,
  the tested version. 3.7: the Variax reports firmware 3.10, the tested
  version.
- The owner reported the continuation of that run through #297. Resume
  looked as if it had stopped on 3.7. The continuation selected 3.3,
  carried 2.7 and 3.7 forward as those expected skips, read 3.2 again,
  and failed 3.3 again. No second report is quoted here.

7.3 and 7.4 ran as two separate pedalboard steps. 7.2 is the Variax
volume and tone knobs. 7.5 is the six PODxt Live tone knobs. Those two
are not pedalboard steps.

## Prompts and keys on this head

Source: `src/rig_check/gui/mod.rs`, `src/rig_check/cli.rs`,
`src/rig_check/guided.rs`, `src/rig_check/catalogue.rs`,
`src/rig_check/recover/vdi.rs`, `src/rig_check/resume.rs`. The window
and the terminal share the runner.

A guided step has four spoken moments today.

1. **Step card**, before Start. Focus is **Skip (S)**. The card shows
   What it does, "You do", and SEE / HEAR at 34 points. The muted line
   is "Nothing runs until you choose Run. Skip is the default."
2. **Prepare**, after Start. `prepare_action` shows "Step {id}: get
   ready", the same SEE and HEAR at 34 points, and "Press OK when you're
   ready. A short countdown tells you when to start." Focus is **OK**.
3. **Countdown.** `begin_action` calls `instruct` immediately after OK,
   so the 34-point SEE and HEAR stay on screen. The large line is
   "GET READY {n} s" for 10 seconds (`READY_DELAY`), with **+10 seconds**.
   The terminal prints "Get ready. Start in 10 s. Watching for {what}."
4. **Action window.** After 10 seconds the large line becomes
   "NOW {see}" and a 60-second bar (`ACTION_WINDOW`). On the `guided()`
   path used by 7.1–7.5, the listener is already open during the
   countdown. A match then is stored as "Seen: {what} during the
   get-ready countdown" and completes the phase. That is what 7.3's
   return did, and what a pedal sweep at 2 seconds does in
   `a_pedal_sweep_during_ready_or_late_in_the_action_window_is_detected`.

6.1 uses `begin_action`, then its own wait. The countdown body is the
wait sentence, not the cable cue. Unplug waits on "Wait for the
countdown before unplugging the VDI cable from the guitar." Reconnect
waits on "Disconnect detected. Wait for the countdown before plugging
the VDI cable back into the guitar." `wait_until_ready` holds through
that countdown and does not follow the cable. When the countdown ends,
`instruct` shows the cable cue, and listening starts. The cues are
"Unplug the VDI cable from the guitar. Leave the PODxt Live USB cable
connected." and "Plug the VDI cable back into the guitar." They live in
`src/rig_check/recover/vdi.rs`. An identity check can finish the phase
before the countdown: already unplugged, or already answering with the
starting identity. This checkpoint does not change that order.

### Window keys

| Screen | Focus | Enter or Space | Other keys |
| --- | --- | --- | --- |
| Choose mode | Practice | Practice | The other button is Real rig, This Mac, or This computer, from the compile-time title |
| Start notice | Quit | Quit | Accept and continue is the other button |
| Step card | Skip (S) | Skip | R starts or runs, S skips, Q stops. Backspace reviews when a prior result exists. Back is a button. |
| Save notice | Skip | Skip | Save stays disabled until the confirmation matches |
| Prepare | OK | OK, which starts the countdown | Skip and Stop are buttons |
| Get ready and NOW | The screen does not move focus | A click on Continue ends the wait during the countdown and during NOW. Tab can reach that button. | +10 seconds during Get ready. Stop asks the step to finish. |
| Didn't see it | Skip | Skip | Try again and Stop are buttons |
| Listen | Not sure | Not sure | Yes and No are buttons |
| Current result | Continue | Continue | Different…, Repeat this step when `can_repeat`, Stop, Back. Backspace reviews. |
| Review | Return to current step | Return | C returns. Back and Backspace move only through recorded screens. |

Closed window on a card skips. Closed window on prepare or "didn't see
it" stops.

### Terminal keys

| Screen | Enter | Other input |
| --- | --- | --- |
| Start | Quits | `accept` or `a` starts |
| Step card | Skip | `[r]` starts or runs, `[s]` skips, `[q]` stops, `[b]` reviews. Any other line also skips. |
| Prepare | OK | `[s]` skips. `[q]`, Stop, or a closed input stops. Any other line skips. |
| Countdown | Continue, which ends the wait | `[+10]` adds ten seconds. `[q]` stops. The line already names what is being watched. |
| Didn't see it | Skip | `[t]` tries again. `[q]` or a closed input stops. |
| Listen | Not sure | `[y]`, `[n]`, `[?]` |
| Current result | Continue | `[b]` reviews, `[c]` continues, `[d]` records a display difference when offered, `[r]` repeats when `can_repeat`, `[q]` stops. |

### What Back does now

Back, Backspace, sidebar selection, and terminal `[b]` open a recorded
result or the accepted start notice. Return and C leave that view.
`history::back` does not call the runner. Review renders the result with
`awaiting` false, so the only controls are Return and Back. A card Skip
is stored as skipped-by-you with no start time, the run moves to the
next card, and Back from that card can show the skipped result. That
view cannot start the step. Repeat this step is only on the current
result, and `can_repeat` requires a start time, so a card Skip never
offers it.

## Proposed sequence: Get ready, then NOW

Use one countdown chrome for 7.1–7.5, in the window and the terminal.
6.1 keeps the countdown order in `recover/vdi.rs`, described below.

**Prepare**, at the computer or once the player has chosen Start. Large
line: "Get in place." Name where to stand and which family of control
the next NOW will use, without the act itself. Example for 7.3: "Go to
the PODxt Live. This step uses a channel footswitch. Leave every
footswitch and the expression pedal as they are." Then: "Press OK when
you are there. A countdown will tell you when to act." Focus stays
**OK**, because Start was already a deliberate choice on a card whose
focus was Skip. Skip and Stop remain on the screen. Enter means OK.

**Get ready.** For 7.1–7.5. Large line: "GET READY" and the seconds
left, default 10, with +10 seconds. One body line: "Do not act yet.
The instruction appears at zero." No 34-point SEE or HEAR. The terminal
does not name the action in this line: "Get ready. 10 s. Do not act
yet. [+10] More time   [q] Stop". Enter does nothing. Continue is not
shown. Stop still stops the run. 6.1's countdown body stays the wait
sentence in `recover/vdi.rs`.

**NOW.** Large line: "NOW" plus the one act sentence for this phase.
The 60-second bar and the live "Seen" line stay. Continue checks what
was seen. Stop stops the run. The next phase, including the return half
of 7.3, starts again at Prepare.

Already-there uses the state preflight a step already has. It runs
before Prepare. It is not added at NOW, and it is not given to a step
that returns no state.

- 7.1 compares the current program with the requested backup
  (`already_on_variax`), including the firmware checksum. A checksum
  mismatch still fails that step.
- 7.3 compares the playing channel and tone with the stored tone
  (`already_on_podxt`).
- 6.1's identity preflight stays where Lane 1 put it, before the
  countdown.

7.2 calls identify and 7.4 reads the current tone, and both closures
then return no state, so neither call is a state preflight. 7.5 asks
for nothing before its window (`|_| Ok(None)`). Those three stay
passive sweeps. The proposal does not turn them into state checks.

For 7.2, 7.4, and 7.5, motion during Get ready is shown as "Seen during
Get ready. It does not count." The one listening window opens at NOW
and needs new motion after that. 7.2 is one window for volume and tone.
7.4 is one expression-pedal sweep. 7.5 is one passive poll of the six
tone knobs and sends nothing, as
`the_six_knob_window_records_exact_and_near_end_sweeps_without_sends`
already requires. No readback and no second request is added.

For 7.1 and 7.3, a match during Get ready does not complete the phase.
At NOW the listener the step already uses opens
(`variax_program_window`, `channel_window`). The state preflight is
not run again at the start of NOW. 7.1's existing full-program compare
stays after a detected turn. It is not copied onto 7.2, 7.4, or 7.5.
An early selector turn or footswitch has to happen again after NOW.
That is the rule for 7.3's return, which on `dac5321` completed during
the countdown.

6.1 does not listen during its countdown. The countdown body stays
Lane 1's wait sentence. When the countdown ends, the cable cue is
shown and listening starts. A cable change during the countdown is not
the completion path. The identity preflight before the countdown still
finishes a phase that is already unplugged, or already back with the
starting identity, and then no countdown opens. This checkpoint does
not change 6.1's listener, identity checks, sentences, gates, or
traffic. Lane 1's tests cover notice and no-notice, no prompt during
a read, timing, timeout, and Stop.

## Footswitch and expression pedal

The PODxt Live pedalboard has channel footswitches and a separate
expression pedal. On this head the two steps are easy to hear as the
same request.

| | 7.3 today | 7.4 today |
| --- | --- | --- |
| Card title | Switch the PODxt Live to a named channel | Move the PODxt Live pedal |
| You do | Press the footswitch for the named channel, then switch back. | Rock the pedal from heel to toe and back, then leave it where it was. |
| Act sentence, each phase | "On the PODxt Live, press the footswitch for channel {label}." then "press the footswitch for channel {label} again." | "Rock the PODxt Live pedal from heel to toe and back, then leave it where it was." |
| Watched event | channel change to {label}, then channel change back | pedal sweep (CC 7 or CC 4) |
| Live label for CC 4 and CC 7 | "pedal", on every PODxt listen, including 7.3 | "pedal" |

Proposed card titles, for a later catalogue edit:

- 7.3: "PODxt Live footswitches: change channel and return"
- 7.4: "PODxt Live expression pedal: heel to toe"

Proposed NOW sentences:

- 7.3 go: "NOW: press the channel footswitch for {label}. Leave the expression pedal still."
- 7.3 return: "NOW: press the channel footswitch for {start} to come back. Leave the expression pedal still."
- 7.4: "NOW: rock the expression pedal from heel to toe and back, then leave it where it was. Do not press a channel footswitch."

Prepare names the same control in one sentence and does not include
"press" or "rock". The card may still preview both halves of 7.3 under
You do, because that card is read before Start.

Seen lines use the control name. A channel change is "channel {label}".
CC 4 or CC 7 is "expression pedal {value}" on every PODxt listen. On
7.3 that line is information. It is not progress toward the channel
change. On 7.4 it is the watched event. 7.2 keeps "guitar volume" and
"guitar tone". 7.5 keeps the six knob names.

7.3 stays two phases. Each phase has its own Prepare, Get ready, and
NOW. The return is not printed during the first countdown.

## Accidental Skip, for #276

Enter on a step card skips, in the window and the terminal. Any other
terminal line on that card also skips. That is the safe default and it
stays. A person who presses Enter while reaching for R loses the step
and cannot start it from Back.

Add one control on the review of a step whose outcome is skipped-by-you.
The label is **Run this skipped step**. It is absent for an automatic
firmware skip, a needs-skip, not applicable, a failed step, a stopped
step, a save, a live edit or store that already sent, and any attempt
whose record contains an uncertain write. Those no-replay protections
stay. Listening that only received, and a live step skipped from its
card before any send, are the only candidates. Gates and the
catalogue's traffic for that step are unchanged.

The control is not focused. Focus on review stays **Return to current
step**. Back, Backspace, Enter, C, and terminal `[b]` and `[c]` do not
activate it.

Choosing it revalidates the step against the current run before any
card is shown. The checks are the ones the runner already has:

- **Rig.** `precheck` still accepts the step for the connected rig in
  this run. A missing device, a different verification result, or an
  offline-device refusal leaves the skip in place.
- **Gate.** `real_device_check` for that step still passes. A closed
  gate leaves the skip in place. No gate constant changes.
- **Order.** Catalogue order still allows the step. A Needs entry that
  did not pass, an earlier failure that makes the step read-only, or a
  save whose earlier applicable steps did not pass, leaves the skip in
  place.
- **Step prerequisites.** The data that step already requires is still
  present. 7.3 still needs its starting channel and backup. 7.1 still
  needs its starting preset and program. If that data cannot be
  established, the skip stays.

Any check that cannot be established leaves the recorded skip as it
is. The review names the first reason. The card stays closed.

When every check passes, a confirm opens. Large line: "Run the skipped
step {id}?" Body: "The current rig, this step's gate, catalogue order,
and this step's prerequisites were rechecked. Later steps that were
waiting only on this skip will be offered again. A step that already
ran is left as it is. Nothing already saved is written again." Focus
is **Keep it skipped**. **Run it** is the other button. Terminal: the
review lines stay as they are, and the confirm asks for the words
`run skipped` or `keep`. Enter means keep.

**Run it** does not send. It shows that step's normal card, which still
focuses Skip. Start or R is required. The attempt already stored stays
in the report. The new attempt is appended. The latest outcome counts.
Showing that card is allowed only after the revalidation above. An
older skip, after later steps have run, is not offered on the strength
of the old card alone.

After that result, Continue offers, in catalogue order, each later step
that has not executed and was held only by this skip. That includes a
step whose Needs list was waiting on it, and a save that was held
because an earlier step had not passed, once every earlier applicable
step passes. Each of those cards focuses Skip. Steps that already
executed stay recorded. A save or live edit that already sent is left
recorded. The card that was current when Back was pressed is then shown
again. If the retry is skipped or it fails, those dependents stay
blocked and the current card returns.

Practice covers Skip, Back, the confirm left on Keep it skipped, and the
same pending card, with the simulated message count unchanged. A second
case revalidates a skipped step whose rig, gate, order, and
prerequisites still hold, presses Run it, then Start, records both
attempts, and returns to that same card. A third case leaves the skip
in place when a prerequisite can no longer be established. Back during
any case sends nothing.

A person-skip still blocks later saves, because a save requires every
earlier applicable step to pass. It also blocks any later step that
lists it in Needs. A live step is held back by an earlier failure. A
person-skip alone does not set that failure. An automatic skip of a
tested-firmware check still does not block saves. Those are the
runner's rules on this head, in `precheck`.

## Resume target, prerequisite, and kept skip

[#297](https://github.com/guirossibrum/VariaxWorkbench/issues/297).
Catalogue order places 2.7 after 2.1, and 3.1, then 3.7, then 3.2,
then 3.3. On the saved run both firmware checks are already complete.
2.7 is the expected skip for PODxt Live firmware 3.01, the tested
version. 3.7 is the expected skip for Variax firmware 3.10, the tested
version. The continuation's target is 3.3. 3.2 is read again because
that passed read is the input to 3.3's full-program cross-check. 2.7
and 3.7 are carried forward with the other kept results.

The window preview is the continuation card. Its strong line is
"Continue at {id} {title}" for the plan target. The notes under it
include "First failed or unfinished check: {target}." and, for this
target, "The Variax settings will be read again for the full-program
cross-check." "Passed checks eligible for reuse" lists only reused
steps that passed, so 2.7 and 3.7 are absent from that line. The sidebar tag is
40 by 18 points. A reused pass is REUSED. An expected firmware skip and
a skip the person chose both use the tag SKIP. The terminal does not
print this card. After Resume…, the window and the terminal both ask
the existing continuation confirmation before any reused result is
committed.

The card and the progress list each show three role lines. The plan's
target, rebuild set, and reused set stay as `resume.rs` builds them.

**Resume retries.** "Resume retries {id} {title}." In this continuation
that is 3.3, Variax full current program, and the line keeps the last
result: the checksum did not match. A mismatch fails 3.3 again. That
#255 refusal stays. The target's progress row says "Resume retries this
step." When the first open applicable step is a skip the person chose,
this line names that step, and the row adds "Skipped by you. Can need
attention."

**Read again first.** One line for each step the plan will run again
before the target. "Read again first: {id} {title}." For 3.3 the body
stays "The Variax settings will be read again for the full-program
cross-check." and the step is 3.2, Variax current settings. Until 3.2
is the running step, its progress row says "Read again before 3.3."

**Expected skip, kept.** One line for each reused expected firmware
skip. "Expected skip, kept: {id} {title}. Firmware {version} is the
tested version, so this step is already complete." In this continuation
the lines are:

- 2.7, Compatibility check for other firmware versions. PODxt Live
  firmware 3.01 is the tested version.
- 3.7, Compatibility check for other firmware versions. Variax
  firmware 3.10 is the tested version.

The progress tag stays SKIP. A second line under the title repeats
"Expected skip, kept." A skip the person chose keeps "Skipped by you.
Can need attention." and stays off this heading. The #276 control
"Run this skipped step" remains the review path for that skip, and
only after the revalidation in the Skip section.

Passed steps that the continuation reuses stay on "Passed checks
eligible for reuse," and their tag stays REUSED.

The continuation starts only when Resume… is chosen. Start new stays
the first button. The confirmation stays the one in `Plan::revalidate`:
Rig Check checks the connected rig and the starting state, including
live identity, before reusing results. A refusal ends the continuation
during that read-only check. Selection, retain, and rebuild stay.

## Live and write authorization

This checkpoint changes no constant in `src/hardware_gates.rs` and no
approval switch. Read, live, and save risk on each catalogue step stay
as they are. Saves still need every earlier applicable step to pass,
still require the typed confirmation, and still have no repeat. The
proposed retry never calls a store and never repeats a live send. Audio
steps 8.2–8.5 stay unavailable. #267 remains the owner's release for
real guided steps. A mark on this file is design approval to implement
the chrome, the retry, and the resume role lines. It is not a new
device permission, and it leaves continuation selection, retain,
rebuild, live identity revalidation, and the checksum refusal as they
are.

## Owner marks

Write **Approve** or **Change** on each row.

| # | Proposal | Mark |
| --- | --- | --- |
| 1 | For 7.1–7.5, Get ready shows the countdown and "Do not act yet." The act sentence appears at NOW, at 34 points. | |
| 2 | For 7.1–7.5, Enter during Get ready does nothing. Continue is absent until NOW. OK stays the focused prepare button. 6.1's countdown keys stay as Lane 1 wrote them. | |
| 3 | Passive sweeps 7.2, 7.4, and 7.5 count only new motion after NOW, in the one listening window they already use. No readback or second request is added. 7.5 sends nothing. 7.1 and 7.3 keep the state preflight they already run before Prepare. That preflight is not repeated at NOW and is not copied onto the sweeps. | |
| 4 | 6.1 shows Lane 1's wait sentence during the countdown. The unplug or replug cue is shown after the countdown, and listening starts then. A cable change during the countdown is not the completion path. The identity preflight stays before the countdown. | |
| 5 | 7.3 says channel footswitch. 7.4 says expression pedal. CC 4 and CC 7 read "expression pedal" on every PODxt listen. | |
| 6 | Back and Return stay review-only. "Run this skipped step" is a separate, unfocused control, confirmed by "Keep it skipped" as the focused choice. | |
| 7 | The retry keeps both attempts, recomputes only unexecuted dependents, and does not replay a live send or a store. An earlier skipped step is offered only after the current rig, its gate, catalogue order, and its prerequisites are revalidated. Otherwise it stays skipped. | |
| 8 | The continuation card and the progress list name three roles. Resume retries is 3.3 in this report. Read again first is 3.2. Expected skip, kept is both 2.7 (PODxt Live firmware 3.01) and 3.7 (Variax firmware 3.10). A skip the person chose can need attention. Resume… stays its own button. | |

## Later implementation

No code in this checkpoint. After the marks:

- The common chrome and the 7.3 / 7.4 sentences are a #248 implementation.
  Practice covers a sweep during Get ready that does not count, the same
  sweep after NOW in the one window, and 7.5 still sending nothing.
  7.1 and 7.3 still use only the preflight they already have.
- The Skip control, confirm, revalidation, dependency recomputation, and
  the Practice path are #276, against rows 6 and 7.
- The 6.1 state machine stays #260, against row 4. This checkpoint
  records Lane 1's countdown, then the cable cue, then listening. It
  does not change that order.
- The three continuation role lines, on the window card and the
  progress rows, are a #248 implementation against row 8. They label
  the plan that `resume.rs` already builds.

`docs/rig-check/design.md`, the catalogue, and the fourteen pictures
stay as they are until that implementation. The roadmap bullet for #248
still points at the screenshot checklist.
