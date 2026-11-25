# Variax Refresh owner test card

Use the verified Variax 600 3.10 device tab through the supported PODxt Live.
Record the selected guitar model before starting. Use a preset whose saved
settings uniquely identify it in the library so the read can verify the
switch back.

1. Open a file in that tab, make an unsaved edit, then choose **Refresh from
   guitar…**. Choose **Keep editing** and verify the original edit remains.
2. Choose **Refresh from guitar…** again, choose **Refresh and replace**, and
   watch the progress count. The existing file view remains in place until all
   60 presets and 12 bank names are available.
3. Check all 12 bank names and five presets per bank against your backup.
   Verify the guitar returned to its original preset. Record any switch-back
   or bridge-release warning and stop the check if one appears.
4. Use **Save As…** with a new .vwb filename. Reopen the copy and compare the
   bank names and model settings. The original open file should remain intact.
5. Start another refresh and choose **Cancel read** partway through. Verify the
   complete open library stays in the editor. Record the reported switch-back
   result. Check the physical guitar's final model.

The editor uses the G5 sweep and preset selection, G3 settings reads and the
G7 bank-name read policy for `0x6D`. DSP stays active. Refresh switches presets
temporarily and attempts a verified switch back when it finds a unique match
among the presets it read. It sends no preset edits or store commands.
