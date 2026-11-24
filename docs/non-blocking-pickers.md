# Non-blocking file and folder pickers

[#233](https://github.com/guirossibrum/VariaxWorkbench/issues/233)

The shell's **Open file**, Device check's report and backup-folder actions,
and recording's load and save actions use the same picker in
`src/shared/picker.rs`. The app polls the native future from the GUI update.
The future wakes egui on completion. One picker can be open at a time.
Closing Device check or recording discards that window's pending result.

## Editor lane handoff

Use `EditorServices::request_picker::<Self>(ctx, request, complete)` for
family-editor actions. Choose a `PickRequest` for the file filters and
suggested name. The method returns whether the request was accepted. A
duplicate request returns `false` and never runs its completion.

The completion is a one-shot callback with the originating editor, the
selected `Option<PathBuf>`, and fresh `EditorServices`. `None` means the
user cancelled. The shell dispatches to the originating tab ID even after
the user switches tabs, and drops results for a tab that is gone. Capture
the requested target, such as a slot index or edit operation, if it is
needed when the result comes back. Keep each editor's existing validation,
cancellation messages and atomic save functions in the callback. For a
save requested by a close prompt, call `services.finish_close_if(saved)` with the result of the save.

The family lanes still own the callers for Save Copy (including the shell
button and shortcut that delegate to the editor), artwork, model and
tuning files, tone files, and editor backup folders. Their legacy
`FileDialogs` methods remain during conversion.

## Owner check on Hyprland

1. Launch the GUI normally and open the shell's **Open file…** picker.
2. Leave it open for at least 30 seconds. The compositor should keep the
   Workbench window responsive and show no **Application Not Responding**
   prompt.
3. Cancel the picker. The current document and tabs should remain as they
   were. Repeat with **Save steps 1–2 report** in Device check if you want
   to check the save picker. Cancel before selecting a destination.
