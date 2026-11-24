# Element mapping

Each current screen element, and where it goes in the design in
[app-design.md](app-design.md). "Remove" means it leaves the primary surface.
The behavior stays until the issue that implements that surface changes it.
This map does not change code.

## Shell

| Current | Source | New treatment |
| --- | --- | --- |
| Heading `Variax Workbench` | `src/app/shell/mod.rs` | Top bar title next to the Workbench mark |
| `Open file…` | same, also repeated on the empty start page | One toolbar button. The start page keeps a single `Open file…`, not a second copy of every command |
| `New offline tab` menu, and start-page `New offline tab · {family}` | same | One toolbar menu. The start page uses three family cards instead of repeating the menu as buttons |
| `Detect devices` | `src/app/shell/detect.rs` | Toolbar button. Enabled when detection exists. Disabled hover stays `USB presence detection is unavailable in this build.` |
| `Save copy…` | shell, enabled when a tab can save | Toolbar button. Still writes a new copy |
| Family actions, including `Replace file in this tab…` | each family's `actions` | Stay as tab actions in the top bar, after Save copy |
| `Live access locked` | Variax `actions` | Remove. Guitar reads and writes are Device check |
| `Device check…` | `src/gui.rs` | Toolbar button. Opens the restyled window |
| `Test recording…` | shell | Toolbar button. Its panel is unchanged in this proposal |
| `Milestone & Help` | shell | Becomes `Help`. Milestone text already lives in Help and stays there |
| Tab label plus ` *` and `×` | `tab_strip` | Tab with family accent, the word `Unsaved` when dirty, and a 32px close control |
| Status: build number, version, editor status, device status, detection error | bottom panel | Status chip and editor status only. Build number and version move to Help |
| Empty heading `Open a file or start an offline tab` | central panel | Start page title `Open a file or start an offline tab`, then the three cards |
| Help window and its shortcut list | shell | Same window, same six shortcuts. Family help stays inside it |

## Variax editor

| Current | New treatment |
| --- | --- |
| `Editing bank: Custom 1 (example)` and the bank menu | Wood header, same words |
| Five slots, thumbnail, name, dirty mark | Five 32px slots. Slot 2 is Spank example. Dirty is the word `Unsaved` |
| `Get from guitar`, disabled, hover `Reading the guitar is locked during offline editing` | Remove from the header |
| `Send to guitar`, disabled, hover `Writing to the guitar is locked during offline editing` | Remove from the header |
| `Unsaved changes` in the header | Keep, as the word `Unsaved` |
| Model, Author, Notes | Keep, one metadata row. 18-character rule stays in Help and on the field |
| Undo, Redo | Keep |
| Body photo and `Artwork folder…` | Body well. Artwork folder stays a secondary button on Body type |
| Body type / Pickups / Controls | Inspector sections. Body photo stays up on every section |
| Guitar body selector | Body type section |
| Pickup 1 and Pickup 2, Enabled, Front, model, Position, Angle, Level | Pickups section, with Workbench unit strings |
| `Reverse pickup 2 phase`, `Wire pickups in series` | Keep on Pickups |
| Volume knob resistance and taper | Controls section. Example shows `500k Ω` and Audio |
| Tone knob resistance, capacitance, Tone position, Save tone-knob position | Controls section. Example shows `470k Ω`, `11 nF`, `100%`, unchecked |
| Auto-select checkbox and its hover sentence | Keep |
| Output trim | Footer readout `0.00dB` |
| Fretboard | Footer band using `assets/ui/variax/fretboard.png`, six stored offsets |
| Positions 1–5 | Not in the current editor. Added as visible disabled buttons, caption `Not in this editor yet` |
| Spread, and `Workbench 1.75 had no Spread control.` | Stays off the standard-guitar page |
| Milestone essay (`guitar::milestones`) | Already in Help via `Family::help`. It does not return to the guitar page |
| Status `Opened the built-in example banks.` and `Edit preset files offline. Your guitar is unchanged.` | Status chip. The second sentence is the offline chip `No device` |

## PODxt Live editor

| Current | New treatment |
| --- | --- |
| `Offline PODxt Live tone file. This does not edit a connected POD.` | Remove as a banner. The status chip says `nothing is sent` |
| `This is not the Variax guitar editor. Guitar model slots are not applied.` | Remove as a banner. The link readout says `No linked model` and does not apply a guitar slot |
| `Nothing is sent to a device. No preset is switched.` | Status chip: `nothing is sent` |
| `PAYLOADS_NOT_DECODED` paragraph about raw ids | Remove from the page. Raw ids move to the closed Details disclosure |
| `Line6Edit host scale 0–1` labels on Amp, Stomp, Mod, Delay, Reverb | Remove. Knobs use the parameter-table units |
| Pedalboard bank and channel A–D | Metadata strip. An example with no address says `No channel · file only` |
| Amp, Stomp, Mod, Delay as the only toggle buttons | All Edit blocks, each with an LED: Amp, Mic, Cab, Comp, Stomp, Mod, Gate, Delay, Reverb, EQ, Wah, Vol |
| Apply / Discard for a staged edit | Keep as the block's commit action when a control stages a value. The mockup has nothing staged |
| Model names and the four practice/example paths | Selectors show the name. Empty selectors say `No amp model` and the same for cab and mic |
| `Submit This Tone To Online Library` | Not in this app, and not added |

## POD HD500X editor

| Current | New treatment |
| --- | --- |
| Heading `POD HD Edit files · offline` | Page title `POD HD500X` |
| `HD_EDITOR_OFFLINE_NOTICE` | Kept, in the page body |
| The paragraph on fixed H5EP framing and opaque rows | Kept, as the second notice, when a file is open. The empty mockup does not invent rows |
| `No device, audio, firmware or driver operation is available here. Copy saves never replace an existing file.` | Kept |
| `Live connection not supported yet` | Kept as the connection chip |
| Effect reset buttons (Vintage Pre, Screamer, and the others) | Stay with the file editor. They are absent from the empty mockup because no file is open |

## Device check

| Current | New treatment |
| --- | --- |
| Window title `Device check` | Keep |
| Availability banner that names gates G1–G3 and locked steps | Move into Details |
| Mode radios Simulation and This computer | Keep. Labels lose the gate parentheses on the primary line. The full `this_computer_label` sentence stays in Details |
| `SIMULATION_LABEL` | Keep when Simulation is selected |
| `Each step asks first…` | Keep |
| `STEP_TWO_PREFLIGHT` | Keep, under the steps, not as a gate code |
| `CANCEL_LIMIT` | Keep |
| Pretend setup combo and `Start over` | Keep, in Simulation |
| `Step N · Title (G1|G2|G3)` plus `real device: locked` or `real device: open` | Primary row becomes `Step N · Title` and the status word (`not run`, `waiting for your answer`, `passed`, `stopped`). Gate letter and lock move to Details |
| Step action and risk paragraphs | Keep, in full, for the step that is waiting |
| `No, stop` focused, then the Yes label | Keep. This computer step 1 Yes stays `Yes, read this computer's USB list` |
| Backup and PODxt Live tone sections | Keep behind their own questions. Not redrawn in the #215 mockups |
| Outcome lines (`Stopped at step N…`, simulation finished, this-computer finished) | Keep |

## Dialogs

| Current | New treatment |
| --- | --- |
| Detect title, tested-on sentence, scan question | Keep, verbatim |
| `Yes, scan`, `No` with `No` focused | Keep |
| `No supported devices found.` / `Detected N supported device(s).` | Status line after a scan |
| `Unsaved changes` / `Save a copy to keep your changes before continuing.` | Keep |
| `Save copy and continue…`, `Keep editing`, `Discard changes` | Keep this order. `Keep editing` stays focused |

## Removed from the primary surface

- A second copy of Open, New, and Detect on the empty page.
- The four PODxt banners and the host-scale captions.
- Raw hex parameter ids, except inside Details.
- Gate letters and `real device: locked` / `real device: open` on the device-check row.
- `Get from guitar`, `Send to guitar`, and `Live access locked` on the guitar.
- Build number in the status bar.
- Online-library submit, live meters, a firmware button, and an invented HD500X icon.
- Any status that would say synced before a real identity and read.

## Mockup index

The prototype hash is the view. `?shot=1` hides the prototype navigation.
Screenshots:

| File | Hash |
| --- | --- |
| `mockups/01-start.png` | `#start` |
| `mockups/02-tabs.png` | `#variax-body` |
| `mockups/03-variax-pickups.png` | `#variax` |
| `mockups/04-variax-controls.png` | `#variax-controls` |
| `mockups/05-podxt.png` | `#podxt` |
| `mockups/06-podxt-offline.png` | `#podxt-offline` |
| `mockups/07-podxt-disconnected.png` | `#podxt-disconnected` |
| `mockups/08-podxt-unknown.png` | `#podxt-unknown` |
| `mockups/09-hd500x.png` | `#hd` |
| `mockups/10-device-check.png` | `#device-check` |
| `mockups/11-detect-devices.png` | `#detect` |
| `mockups/12-save-prompt.png` | `#save` |
| `mockups/13-connection-states.png` | `#states` |

`02-tabs.png` is the shell with Variax, PODxt Live, and POD HD500X open, on
the Body type section. `03-variax-pickups.png` is the section the example
opens on. The body photo stays visible in both.
