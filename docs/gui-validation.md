# Offline GUI interaction validation

The editor keeps file dialogs behind a small `FileDialogs` adapter. Normal
builds use the existing rfd picker. Test builds substitute scripted results and
panic on any unexpected dialog call, so headless tests never open a native
picker. No new production dependency is introduced.

The twelve headless GUI tests cover:

- Actual egui dirty-prompt buttons: save cancellation, existing-file refusal,
  successful retry and continuation, Keep editing, and Discard changes.
- Dirty open cancellation and invalid/truncated-file errors, preserving the
  document, original files, undo/redo and unsaved state.
- A canceled save on a clean document with a pending action: cancellation still
  retains the action and does not close/open a document.
- Actual pickup-slider pointer press/moves/release, restored by exactly one
  undo group and one redo.
- Actual bank browser row clicks for both file formats, retained edits across
  selections and disabled browsing during a simulated operation.
- Active-operation guards for edits, open/save, shortcuts and bank selection.
  Closing requests cancellation and defers closing; a fake disconnected worker
  is joined and its error leaves the current document intact.
- Missing-artwork error caching, canceled folder selection and recovery through
  a new folder without editing the preset.
- Rendering imported NaN/out-of-range values without byte/history mutation,
  invalid startup arguments and the permanent live-access lock.

Dirty prompts now block edits, undo/redo, bank browsing and new open/save
shortcuts behind them. Save-and-continue requires the save operation to return
success; it no longer infers success from the current dirty flag. An existing
pending action is retained when another close request arrives. These fixes came
from the interaction work; they do not relax device access.

`tests/document.rs`, `tests/bank_document.rs` and `tests/edit_domains.rs` separately
exercise atomic non-overwriting persistence, unchanged originals, grouped edits,
whole-container history and validation failures. Native rfd dialogs are not used
in those tests either. Both host/software local configurations pass the full
127-test suite, including all twelve GUI tests, with Clippy and formatting clean.
The three-platform core CI excludes GUI/USB and is not a native-window test.

Remaining manual checks are the OS shell's actual open/save/folder picker,
portal integration and cancellation on supported desktops, window close/focus
behavior, keyboard text-entry focus, display scaling, accessibility, and complete
artwork presentation on native windows. Windows/macOS GUI adapters and their
pickers have not been certified by the core CI. These are distinct from the
automated workflow logic and should be checked when packaging/platform GUI work
is performed. No guitar connection is needed for these manual desktop checks.

All operation interruption tests use fake channels/threads and synthetic files.
No USB/MIDI/audio device is enumerated or opened. Passing GUI/file tests does not
establish audio equivalence, a hardware backup, recovery or permission for live
traffic. The future guided hardware check remains separately gated.
