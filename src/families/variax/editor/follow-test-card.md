# Variax Follow owner test card

Owner acceptance for [#202](https://github.com/guirossibrum/VariaxWorkbench/issues/202)
is pending. Use the verified Variax 600 3.10 tab through the supported PODxt Live.

**Refresh prerequisite.** Before checking slot selection, use **Download from
guitar…** on the Variax tab (the library Refresh) and wait for the full 12-bank
download. If the tab already shows **Downloaded on this connection**, use that
download. A **Saved guitar download** or imported backup is a historical copy
that you can browse and edit offline. Save any edits you want to keep before
replacing it with a fresh download. Make a local edit in the current download
to verify that Follow preserves it.

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
4. Turn **Follow** off and check that further knob movements leave the editor
   view alone. Turn Follow back on before the disconnect check.
5. Note the selected slot, then unplug the VDI cable while the PODxt stays
   connected. Record whether the editor reports the disconnect and stops
   Follow. If the PODxt sends no notice, turn Follow off and use **Refresh
   guitar** on the PODxt tab to detect the disconnect. Reconnect and let
   identity verification finish. Follow should resume if its checkbox was
   left on. If you turned it off, turn it back on after verification.
   Confirm that the earlier library is labelled **Saved guitar download**,
   with the same selected slot and local edit. Move the selector and then the
   tone knob after the preset settles. The guitar preset indicator and
   **Live tone** should report the new guitar values while the historical
   library keeps its offline selection and saved tone settings. Save any
   edits you want to keep, then use **Download from guitar…** again. Confirm
   **Downloaded on this connection** and repeat steps 1 and 2 with this download.
6. With Follow on, run Device Check. Check that it can use the rig and
   that Follow resumes after the operation gives the interface back. In
   separate builds approved for live editing and stores, check the same
   handoff for those operations. An unknown store outcome must leave
   Follow stopped.
7. With Follow on, close the Variax tab. **Refresh guitar** on the PODxt tab
   should be able to use the rig.

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
