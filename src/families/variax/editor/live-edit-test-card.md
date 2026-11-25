# Variax editor working-buffer test (#204)

The editor has separate closed compile-time switches for automatic live
editing and manual Send Edit Buffer. This card is for an owner-run build
after review of the #245 adapter, program reference checks, and the
specific switch used for the test. Offline tests drive a fake adapter.

1. Connect the supported Variax 600 through PODxt Live, let identity finish,
   and select a downloaded library position. Record the starting position
   and sound. The editor opens with Live editing off.
2. Enable **Live editing**. Confirm the working-buffer upload completes and
   the selected model plays. Move one pickup control several times and then
   stop. Confirm the edited sound and the **Modified · not stored on
   guitar** marker. Change the alternate-tuning Enable flag and verify the
   resulting tuning. The adapter should use a full upload for that toggle.
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
