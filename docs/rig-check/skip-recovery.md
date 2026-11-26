# Rig Check recovery after an accidental Skip

Design checkpoint for owner review. The Mac follow-up asks to recover an
accidental Enter on a step card within the same run. [Owner decision](https://github.com/guirossibrum/VariaxWorkbench/issues/276#issuecomment-7198373640).

## Review and run

| Screen | Action and result |
| --- | --- |
| Current card | Enter on the default focus selects Skip. R explicitly selects Run or Start. Back opens the previous recorded result. Back from the first card opens the accepted notice for review. |
| Earlier result | Back moves to the preceding result. **Return to current step** brings back the pending card or result. The reviewed result and the current position are labeled separately. Both navigation actions use recorded data and leave the rig, attempts and report alone. [Eligible review](mockups/15-review-skipped.png). |
| Eligible earlier skip | **Run skipped check** opens a confirmation naming the step and where the run will return. **Keep it skipped** starts focused and Escape cancels back to review. **Open check card** opens the normal card for that one check, with its procedures and risk visible and Skip focused. Run or Start is a separate, deliberate choice. [Confirmation](mockups/16-confirm-skipped.png). |
| Ineligible earlier skip | Keep **Skipped by you** and display “Run skipped check unavailable” followed by the first concrete reason. Omit the action. If a prerequisite changes after the action was offered, show that reason and keep the original skip. Back and Return remain usable. [Unavailable review](mockups/17-skipped-unavailable.png). |
| Recovered result | Show the new outcome, with the original skip visible in history. Continue returns to the saved pending prompt and its usual safe focus. If the prompt has become unsafe, show that same step with the new prerequisite reason and its ordinary safe choices. [Returned Practice card](mockups/18-return-after-skipped.png). |

On review, Return has default focus. C or Enter on Return goes to the
current step. Backspace works outside text entry. Tab can focus
**Run skipped check** for deliberate activation with Enter or Space.
R and S are card shortcuts only. Consume the activation event when a
screen changes so a keypress or held key cannot carry through to Run.
Pressing Enter on the recovery card's default Skip preserves the skip
and returns to the saved prompt.

In the terminal, **[b] Back** and **[c] Return to current step** remain
navigation. Blank Enter on review returns to the current step. Print
the explicit command **run skipped** only when eligible. At confirmation,
**[o] Open check card** is explicit and Enter keeps the skip. On the card,
**[r] Run/Start** is explicit and Enter keeps the skip. A blank or invalid
line cannot start a check.

## Eligibility and revalidation

- Offer recovery only during an active run at a stable prompt, for an
  earlier record explicitly skipped by the person **before the check
  started**. Inspect the full attempt history, including whether any
  attempt began, sent, or has an unresolved outcome. A skip inside a
  guided action is different from a skip on the card. Expected firmware
  skips, Needs skips, unavailable checks, failures, stops and completed
  results are outside this action. Keep their original status and reason.
- Consider a read or receive-only guided check when its inputs and any
  required starting or return state are still valid. The action can also
  offer a live check skipped before its first send, provided the original
  ordering and all applicable live-write conditions still hold. A
  read-labeled check that changes device position must satisfy its
  position and return checks. Refuse when the effect or the ability to
  restore cannot be established. Stores and final cleanup remain outside
  Skip recovery.
- Compute the offer from current run data without device traffic. At the
  explicit Run or Start, recheck session and rig binding, required device
  identities, verified firmware and relay where applicable, build
  availability, the step's compile-time gates and Real approvals, and
  each catalogue Needs result. Validate inputs like the starting
  channel, preset or edit buffer and verified backup evidence against
  the target and the current connection. An old pass alone cannot
  establish that state after a disconnect, rig change or intervening
  operation. Use only the check's existing authorized preflight after
  explicit activation. If that cannot establish a prerequisite, refuse
  before executing the step. Run ownership must cover revalidation and
  execution so another action cannot invalidate the checks in between.
- Reapply the run's safety history, including failures, read-only mode,
  unresolved restoration or release, persistence errors and uncertain
  writes. An unresolved store blocks recovery traffic and leaves the
  existing stop and release behavior in charge. A later failure can
  veto even an earlier skipped live check. A live check also needs its
  earlier prerequisites and proof that it can still run in order.
  A matching rig and a newly passing read cannot clear these restrictions.
- Executed live checks may have changed the edit buffer or its return
  state. Stores may already have changed flash, even if confirmation
  never arrived. A past write or uncertain outcome therefore cannot
  become eligible through a newer status, Back, or Return. The recovery
  invokes at most the selected skipped check and its normal procedure.
  It never walks forward through later cards to repeat, undo or run
  them. This applies to GUI, terminal and Practice.

Suggested refusal copy is specific to the saved result or current state.
Examples include “Step 2.2 could not be verified for the current PODxt Live
connection,” “This check had already started,” “This check writes a stored
preset,” and “A store outcome is unknown. Further checks are stopped.”

## Dependencies and the report

Recompute the selected step's effective result, direct and transitive
Needs decisions, and the all-earlier-applicable-pass condition for saves
in catalogue order. A successful recovered read satisfies only the facts
it actually established. It can clear a block caused solely by the
original skip for a still-pending check. Expected firmware skips retain
their existing treatment. A failed attempt, an independent missing
prerequisite or a run-wide safety restriction continues to block the
affected checks.

The recovery returns to the exact saved prompt. Earlier visits where a
downstream check was blocked stay in history, even if the reason would
now evaluate differently. Such a check still has no pass evidence, so
it cannot satisfy another dependency or the save rule. This checkpoint
proposes keeping the original run order and avoiding a queue of
intervening cards. The owner can review that choice with the screens.

Keep the original user-skip record, later outcomes and their evidence.
Record each accepted execution as a new attempt under the same catalogue
ID, with its own outcome and evidence. Document a preflight refusal as a
reason, preserving the skip and recording zero executions. If the check
actually ran and failed, retain both records and apply the ordinary
failure policy to its new effective outcome. Preserve any original look
check answers and report both outcomes when a recovered check has its own
display comparison. Keep the same run ID and evidence references in the
saved report. Checkpoint before execution and publish the updated report
through the normal atomic report path. A checkpoint failure prevents
execution. Previously exported report copies remain historical snapshots.

## Practice acceptance checklist

Run with generated fixtures and a simulated message log. These are
acceptance cases for the implementation.

- [ ] Start 2.3 with 2.1 and 2.2 passed, press Enter to Skip, and reach
  2.5. Back shows 2.3 and “Skipped by you”. Back and Return leave both the
  message count and report unchanged. Back from the first card opens
  the accepted notice for review.
- [ ] Go Back again, choose **Run skipped check**, open its card, then
  explicitly Run. Verify that only 2.3's normal preflight and stored
  channel read occur and no edit or store occurs. Continue returns to
  2.5 at Skip focus. No backup starts. Check that the report preserves
  the skip and records the new attempt under 2.3, and that 2.5 can use its
  evidence when the person later runs the backup.
- [ ] In the wizard use Enter at each default (Return, Keep it skipped,
  Skip). Verify no extra simulated messages or run attempts. Test Tab
  and deliberate activation, Backspace outside text fields, Escape at
  confirmation and a held Enter across transitions. In the terminal
  check [b], [c], blank and invalid input, and the explicit command and
  [r] path.
- [ ] Invalidate a connection, a gate or approval, a Needs result, or
  required backup or starting-state evidence. Check both refusal on
  review and a change between confirmation and Run. Verify a specific
  reason, zero step executions, and the original skip in the report.
- [ ] Check refusal for a tested-firmware skip, a Needs skip, a skip
  during a guided action, a failed or stopped check, a store skipped
  before execution, and a live check that previously executed. Check a
  never-started live check after a later failure and a run with an
  uncertain store. No writes or replays occur.
- [ ] Recover a sole prerequisite for pending checks, then repeat with
  an independent unmet prerequisite and with a failing recovered read.
  Verify transitive Needs and save eligibility against the report,
  including expected firmware skips and any historically blocked step
  with no pass evidence. Compare all later attempt records and message
  logs across recovery to confirm none executed or replayed.
- [ ] Inject checkpoint failure before execution and verify zero step
  executions. Inspect the saved report after a successful and a failed
  recovery for the original skip, later outcomes, attempt order and
  intact evidence references.
