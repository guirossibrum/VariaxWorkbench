# Home and device connections

Home is the first, permanent tab. Open file uses the existing file picker.
The Edit offline cards create editor tabs through the existing family
registry. Returning to Home leaves those editors and their edits in place.
Device arrivals keep the current workspace selection when editor tabs are
open. The existing detector selects the first device editor when it creates
the first tab in an empty workspace.

The Device connections window is a single nonmodal egui window. It opens
centered, can be dragged by its title, and retains its position when its
contents change or it reopens. Rows keep their order, and the list scrolls
when it exceeds the available height. The existing Variax and PODxt artwork
is used when supplied. The POD HD500X has no approved product image in the
committed artwork set, so its connection row uses the neutral device icon.

## Lane 1 integration for #310

The shell owns a `connections::Connections` presentation model. Its mount
has no production event source in this delivery. Feed the model before
`connection_window` renders in each frame.

1. Report startup checking and its outcome through `detection`. Use
   `Complete` after a successful check, including an empty check. Pass the
   backend's reason with `Unavailable` or `Failed` when appropriate.
   Report discoveries from the existing inventory and consent-gated
   discovery paths. Count recognized unsupported devices as rows of their
   own.
2. Assign each logical row a stable identity and a monotonically increasing
   attachment generation. Use `arrived` for each new generation and
   `verified` for its result. When a Variax depends on a PODxt, report the
   upstream arrival first and pass that exact PODxt attachment token in the
   Variax row. A disconnect invalidates that upstream's dependent row.
   A later attachment must verify again.
3. Report `waiting` when an authorized library read is queued behind a busy
   transport. Call `started` when a read begins and retain its returned
   token for `progress` and `finished`. Use the existing readers and
   arbitration, and pass the selected reader's total. The stored PODxt
   tone channels alone total 128. The full 299 stored items include amp
   and effect setups as well as tone channels. The playing tone is
   separate. Choosing the full scope requires the existing authorization
   for that reader. The window accepts either total directly.
4. A Retry click returns an attachment from the shell mount. Validate the
   attachment and existing consent before calling `accept_retry` and
   scheduling work through the same arbitration. Report stalls and read
   failures through `finished`. Keep the existing editor and its last
   complete library until its reader accepts a replacement. Publish only
   a complete library for the current attachment and read. The presentation
   checks tokens for its own state, and the publisher must independently
   check them before replacing editor data.
5. Drain arrivals, disconnects, and read results ahead of drawing the window.
   When all supported reads finish, the window shows completion for 1.4
   seconds and rechecks the state before auto-close. Report a current
   attachment's loss through `disconnected` even after the window closes.
   It reopens at its existing position with the affected rows visible and
   requires manual dismissal, including if the same device returns before
   the user closes it. An unsupported
   row can stand on its own, and does not hold up a successfully synced
   supported row. The shell must keep the editor and unsaved edits through
   attachment changes independently of the window.

The synthetic tests drive these transitions without contacting any device.
Acceptance on the owner's supported rig follows the production integration.
