# PODxt Live owner test card for #220

Use the Linux USB-enabled Workbench build and the supported PODxt Live on
firmware 3.01. Close other PODxt tools. On the PODxt, note the playing
channel, tone name and a few knob settings before starting.

1. Open the automatically detected PODxt tab. Confirm that reads are disabled
   until automatic identity verifies firmware 3.01. Verify the tone on the
   PODxt remains as it was before connection.
2. Choose **Read current tone**. Compare the name, amp, cab, Drive and tempo
   against the PODxt display. Expand **Show control values** to inspect other
   settings.
3. Choose **Read installed packs** and compare the reported packs with your
   installed set. Choose **Read globals** and compare the current channel and
   other settings you can confirm on the device. A missing reply appears as
   **No reply**.
4. Choose **Read library** and cancel partway through. Check that the partial
   list remains browseable and that the unit stayed on its playing channel.
   Repeat and wait for the full 299 entries (128 tones, 107 amp setups, and
   64 FX setups). Click several **Device library** rows and verify only the
   stored preview changes.
5. Choose **Start tuner**, pluck a string, and compare the note and deviation
   with the PODxt. Choose **Stop tuner** and verify the tuner exits. The app
   also attempts tuner cleanup when the read finishes or is cancelled. If
   cleanup reports an error, check the tuner state on the PODxt.
6. Choose **Follow the POD**. Move a knob, check the live control value, then
   put the knob back. Use the PODxt TAP switch and look for the Tap indicator
   and updated tempo. The Clip indicator appears when the PODxt reports
   clipping during normal use. Choose **Stop following** to release the rig.
7. Confirm the original channel is still playing. For a normal unplug and
   reconnect, verify the old tab disables live reads and the new attachment
   verifies its identity before accepting requests.

The first run leaves stored slots untouched. Save-notice slot refresh, incoming
program changes, mid-read disconnect, and gate denial also have fake-transport
coverage. Report the firmware, completed steps, library counts, tuner cleanup,
and any changes observed on the unit.
