# PODxt Live edit-buffer owner test card

Use a USB-enabled Linux Workbench build, a PODxt Live on firmware 3.01, and
the verified library backup. Close other tools using the rig. Choose a tone
you can temporarily alter. For this first run, use a tone with Variax linking
off. Record the playing channel, tone name, and a few front-panel settings.
Keep the hardware controls steady while Workbench sends and reads back edits.

1. Open the PODxt tab and wait for automatic identity. Confirm the playing
   tone stays as it was. Choose **Get edit buffer**. Verify the channel and
   tone match the unit and the undo snapshot is shown.
2. With **Stage changes for Send edit buffer** clear, make each row's edits,
   one row per trial. For each trial, wait for readback, compare the settings
   you can see on the PODxt, then choose **Undo to snapshot**. Record whether
   Workbench verifies all 168 bytes. Pick installed models for the selector
   trials. Amp model selection uses CC 12 with amp defaults off.

   | Block | Edit to try |
   | --- | --- |
   | Amp and drive | Drag Drive, change one tone control, select an amp, then check amp enable. |
   | Cab and mic | Change the cab, mic, and Room. |
   | Gate and compressor | Change threshold or decay and compressor gain. Record the displayed threshold separately from the MIDI readback. |
   | EQ | Change one band's frequency and gain. |
   | Stomp | Change the model and one knob assigned to it. |
   | Modulation | Try a NOTE division, then turn NOTE off and change manual speed. |
   | Delay | Try a NOTE division, then turn NOTE off and change manual time. |
   | Reverb | Change the model and mix. |
   | Wah | Change the model and try a nonzero position before undo. |
   | Volume pedal | Change minimum volume or PRE/POST. |
   | Pedal and Tweak | Change the pedal assignment and a Tweak destination. |
   | Tempo | Enable Delay NOTE and change tempo. Check the device's resulting delay time. |

3. Turn on **Stage changes for Send edit buffer**. Move Drive and confirm the
   PODxt value holds until you click **Send edit buffer**. Compare the
   displayed readback, press **Undo to snapshot**, and verify the original
   tone. The action sends one `03 74` edit-buffer frame based on the fresh
   device tone, with only the staged verified fields changed.
4. If any edit is unconfirmed, stop dragging. Choose **Get edit buffer** to
   refresh the playing tone on the same channel, then choose
   **Undo to snapshot**. Record both results. If undo reports a mismatch,
   record which controls differ and any physical knob or pedal movement.
5. Confirm the original channel is still playing. Click **End edit session**
   after verifying the restoration. For a normal reconnect, verify that
   automatic identity runs again and a fresh **Get edit buffer** is needed.

Keep saved tones untouched throughout the card. Report firmware, model packs,
the completed rows, any readback errors, the explicit transfer result and the
undo result. The GUI path and whole-buffer send still await this owner run.
