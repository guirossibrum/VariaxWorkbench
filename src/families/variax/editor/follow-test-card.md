# Variax Follow owner test card

Use the verified Variax 600 3.10 tab through the supported PODxt Live. Open a
12-bank guitar library in the tab and make a local edit so you can verify
that Follow preserves it.

1. Turn on **Follow**. Move the model knob and pickup selector. Compare the
   guitar preset indicator and the selected bank and slot. The indicator waits
   for the first preset message, and the editor selects a clean slot after about
   300 ms or an edited slot after about one second.
2. Move the volume and tone knobs after the slot settles. Compare the knob
   movements and the live percentages. Check that the saved tone position,
   **Save tone-knob position** setting, and local edit remain as before.
3. Record any **Clip**, **DSP overload**, or **MIDI queue overflow** notice
   that occurs during normal playing.
4. Turn **Follow** off. Further knob movement should leave the editor view
   alone. Start Follow again and close the tab. **Refresh guitar** on the
   PODxt tab should be able to use the rig.
5. Unplug the VDI cable while the PODxt stays connected. Record whether the
   editor reports the disconnect and switches Follow off. If the PODxt sends
   no notice, turn Follow off and use **Refresh guitar** on the PODxt tab to
   detect the disconnect. After reconnecting and verifying identity, check that
   the tab still contains the local edits before you restart Follow.

Follow opens the verified attachment for a G3 read session. It sends no
preset changes, preset edits or store commands.
