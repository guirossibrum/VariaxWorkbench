# PODxt Live automatic draft replay owner test card

Use this card only after the Gatekeeper identifies the exact USB build and
its approved route, the owner authorizes this run, and the build admits
native automatic replay. The offline integration keeps that admission
closed. Before the run, confirm the G6 prerequisites, including the #6/#22
coefficient-parity decision and the separately approved restore procedure.
Use the existing [whole-tone send checkpoint](podxt-store-test-card.md#a-resend-the-playing-tone)
and a complete, manifest-verified Device check backup with all 299 entries.
Use the verified PODxt Live 3.01 attachment and close other device tools.

1. Pick two existing saved tone channels A and B with Variax linking off.
   Record the build, route, firmware, packs, saved names and both addressed
   backup files. Start in Live on A with the saved tone loaded. In **Device
   check → PODxt Live diagnostics**, choose **Verify backup for whole-tone
   resend…** for A and select the baseline backup. Keep the same PODxt tab
   and attachment throughout each trial.
2. Change Drive on A enough to hear it. Wait for checked readback and record
   the value and A's draft marker. Leave B without a draft. Select B from the
   editor. If asked, confirm the switch only after verifying that the prompt
   names A and B and the app shows A's retained draft. Check that B plays its
   saved tone. Select A from the editor and wait for **Session draft restored
   and read back.** Check the channel, audible tone, Drive and **Session draft ·
   playing**. The checked replay compares all 160 payload bytes in A's
   playing-buffer reply with the retained draft.
3. While the draft plays, run the read-only Device check PODxt library backup
   into a new folder. Verify its manifest. Compare the addressed A and B
   tone files byte for byte against the baseline. Record both comparisons.
   Keep the playing-buffer dump separately as evidence of the volatile edit.
4. Repeat A → B → A once with the PODxt footswitches, starting with A playing
   the verified draft. B should play its saved tone again. On return to A,
   verify the same checked readback, Drive and audible tone. Take another
   read-only backup and repeat both addressed comparisons. Keep the physical
   controls steady except for the two planned selections.
5. Use the approved volatile restore procedure to finish on the saved A.
   After approval to discard the test draft, close the PODxt tab, reload
   the saved A via B, and verify the baseline playing tone on A. Preserve
   the baseline and both comparison backups.

Keep **Store**, **Sync** and firmware operations out of both trials. If a
switch, attachment, readback or release is uncertain, or a saved slot differs,
end the trial and record the last verified channel, exact status and backup
paths. Leave recovery to the approved procedure.
