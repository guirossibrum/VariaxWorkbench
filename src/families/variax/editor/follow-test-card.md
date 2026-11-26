# Variax Follow owner test card

Use the verified Variax 600 3.10 tab through the supported PODxt Live. Open a
12-bank guitar library in the tab and make a local edit so you can verify
that Follow preserves it.

1. After identity verification, confirm **Follow** starts automatically.
   Move the model knob and pickup selector. Compare the
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
   the tab still contains the local edits. Follow should resume if its
   checkbox was left on.
6. With Follow on, run Device Check. Check that it can use the rig and
   that Follow resumes after the operation gives the interface back. In
   separate builds approved for live editing and stores, check the same
   handoff for those operations. An unknown store outcome must leave
   Follow stopped.

Follow opens the verified attachment for a G3 read session. It sends no
preset changes, preset edits or store commands.

## Tuning override evidence

- **Static fact.** Workbench 1.75 `L6VariaxMIDITranscoder.DecodeCCParamChange`
  passes CC 84 to `HandleTuningOverride(value != 0)`. The
  `L6VariaxPatchManager.OnSysex` handler for `03 07 0D 02` defines subtype 0
  for program and subtype 1 for tone. It provides no tuning-override subtype.
- **Observed.** The existing Variax 600 knob captures contain CC 7 and CC 79.
  They contain no confirmed guitar-side CC 84. The PODxt Live's own CC 84
  is assigned to AMP/COMP in the PODxt controller map.
- **Static fact.** The traced callers of the 3.10 controller-send helper
  at `P:0x14A1D` pass 7 or 79. The legacy image returns without acting on
  the override query, and its supported tuning path is per-preset.
- **Inference.** No ordinary Variax 600 control is expected to emit tuning
  override over this route. **Unknown.** The physical source, if any, of a
  tuning-override CC 84 on the Variax 600/PODxt Live rig remains unidentified.
- **Offline check.** Fake Follow input with CC 84 value 127 and then 0
  displays “Guitar: alternate tuning on” and then “…off.” The saved
  tuning, file bytes and undo history stay unchanged.

See the [Variax 300/600 capability audit](../../../../docs/variax-300-600-capabilities.md)
for the traced legacy tuning path.
