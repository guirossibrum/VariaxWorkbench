# Variax editor working-buffer test (#204)

The editor has separate closed compile-time switches for automatic live
editing and manual Send Edit Buffer. This card is for an owner-run build
after approval of the specific switch used for the test. Offline tests
check the shared #245 procedures against scripted owner-tool wire bytes.

Use the standard host math build and a verified recorded library backup.
Place the private model manifest in `variax-live-reference/models.tsv`
beside the editor preferences, and the approved Mac Workbench program
digests in `variax-live-reference/workbench.sha256`. Use one SHA-256
digest per line for each exact 3,276-byte candidate program, including
the muted intermediate image for each tuning transition tested.

1. Connect the supported Variax 600 through PODxt Live, let identity finish,
   and select a downloaded library position. Record the starting position
   and sound. The editor opens with Live editing off.
2. Enable **Live editing**. Confirm the working-buffer upload completes and
   the selected model plays. Move one pickup control several times and then
   stop. Confirm the edited sound and the **Modified · not stored on
   guitar** marker. Change the alternate-tuning Enable flag and verify the
   resulting tuning. Expect an acknowledged difference to the muted transition image followed by the final full upload.
3. Select another library position. Confirm a full working-buffer upload.
   Return to the first position and confirm the editor still holds the
   local edits. **Get Edit Buffer** opens the current patch while keeping
   the library accessible. Test **Send Edit Buffer** only in the build
   approved for its separate switch.
4. Turn Live editing off during a local edit. Confirm it stops sending
   updates. After a disconnect and verified reconnect, confirm Live
   editing is off and local edits remain in the editor.
5. Return to the starting position using the guitar selector. Confirm the
   stored preset still loads as it did before the test. Stop on a
   verification failure or a busy rig. The editor stops its live session
   on error and retains the local edits.

This path addresses the guitar working buffer. Stored-bank operations are
covered separately by #205.
