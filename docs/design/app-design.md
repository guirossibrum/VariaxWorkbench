# App design

Proposal for issue #215. This document, the HTML prototype, and the
screenshots under `docs/design/mockups/` are the design. They do not change
the running app. #216 (shared components), #217 (shell), #219 (PODxt editor)
and #224 (Variax editor) implement it after the owner approves this proposal.

The current editors already edit files offline. The layout does not. One
wrapped row of buttons, a repeated start screen, a hex form for the PODxt,
and a plain two-column guitar page sit in the same dark egui theme. The
replacement is one shell, with two historical instrument layouts inside it.

- The PODxt Live page uses POD Farm 2's dark stage. The signal path is one
  row of block images in Line 6 Edit 3.06's order. The selected block's
  editor is one panel under that row. The amp is that panel's main view.
- The Variax page keeps Variax Workbench 1.75's regions: wood header, guitar
  body, pickup and control inspector, and a bottom row with alternate tuning
  and the neck.
- POD HD500X stays a file editor in the same shell. It does not borrow either
  chassis.

`docs/visual/` remains a Helix study and a component contract. It is not this
app's layout. No vendor font is shipped. The app keeps egui's default font.

The element-by-element map is [element-mapping.md](element-mapping.md).
The clickable prototype is [prototype/index.html](prototype/index.html).
Open it from a local server rooted at this worktree so the image paths
resolve. Add `?shot=1` to hide the prototype's own navigation.

## Window

| | |
| --- | --- |
| Design canvas | 1440×900 logical points |
| Comfortable minimum | 1100×720 |
| Current viewport | 1120×760, minimum 960×680 (`src/gui.rs`) |

Below 1100×720 the page scrolls. Type size and control targets do not shrink.
#217 may raise the viewport toward the design canvas. This issue does not
change `src/gui.rs`.

The shell is three fixed bands plus the page:

| Band | Height |
| --- | ---: |
| Top bar | 56 |
| Tab strip | 40 |
| Status bar | 32 |

Page padding is 16. The PODxt chain and editor, and the Variax middle row, fill the rest.

## Typography

Sizes are logical points. egui scales them once by `pixels_per_point`.

| Role | Size | Line |
| --- | ---: | ---: |
| Page title | 20 | 28 |
| Section title | 15 | 22 |
| Body | 14 | 20 |
| Compact body | 13 | 20 |
| Knob label | 11 | 14 |
| Status | 13 | 20 |

Weight is regular for body and semibold for titles, block names, and the
focused dialog button. Knob values use the same face, tabular if the font
provides it. There is no second UI font and no new font dependency.

## Colour

Tokens are the screen colours. The PODxt page uses these dark surfaces.
Labels are `text` on `panel`, which is the same cream already checked on
`raised` (13.47:1). The earlier pastel block fills are not the layout.

| Token | Hex | Use |
| --- | --- | --- |
| `bg` | `#101114` | Window |
| `bar` | `#171a1f` | Top bar, tab strip, status |
| `raised` | `#22262c` | Cards, dialogs, inputs |
| `line` | `#3a4048` | Hairline, disabled fill |
| `text` | `#f4f1ea` | Primary text |
| `muted` | `#c4bfb4` | Secondary text |
| `dim` | `#9a9488` | Hints only, still 4.5:1 on `bar` |
| `amber` | `#f0a202` | Accent, LED, dirty marker. Not body text |
| `focus` | `#ffc14d` | 2px focus ring |
| `danger` | `#ffb4a8` | Errors |
| `ok` | `#b6e3a2` | Passed, enabled LED on a dark block |
| Variax accent | `#e0b15a` | Family mark and active Variax tab |
| PODxt accent | `#e4572e` | Family mark and active PODxt tab |
| HD accent | `#8aa0b4` | Family mark and active HD tab |
| Ink | `#1c140c` | Text on the light Variax inspector |
| `panel` | `#1a1d22` | PODxt chain tiles and the editor panel |

Cream on the window, bar, and raised surfaces is 16.74, 15.46, and 13.47.
Muted on the bar is 9.52. Dim on the bar is 5.79. Focus amber on the bar is
10.79. Danger on raised is 8.94. OK on raised is 10.49. Amber as text on the
bar is 8.20, so it may label a state, and it is not used for paragraphs.

A bypassed block keeps its image dim and draws its LED off. The name stays
readable. One LED per block is the on/off indicator. There is no second row
of toggles.

## Spacing and targets

The space scale is 4, 8, 12, 16, 24. Corner radius is 6 on controls and 8 on
panels and dialogs. Card grids use one 16-point gap on both axes. A note
that would sit flush under a row is either a card in that grid or the
tooltip of the control it describes. Every click target is at least 32×32,
including tab close, knob hit area, the pickup sliders, and the tuning
♯ / ♭ buttons. The focus ring sits
outside the target and is not the same colour as selection.

## High DPI

Layout uses logical points. Bitmaps (`stage-frame.png`, the guitar photos,
the 64px product icons) are drawn at their intended point size and may render
sharper on a high-DPI screen. They are not pre-scaled a second time. The
Workaround of multiplying both `pixels_per_point` and a manual image scale is
rejected.

## Components

Each component has hover, keyboard focus, disabled, and, where it applies,
error, dirty, and pressed.

| Component | Drawn as | States that matter |
| --- | --- | --- |
| Knob | CSS hardware dial, value under it, unit in the value | Hover brightens the tick. Focus rings the dial. Disabled dial is flat and the value stays visible. Drag is vertical. Double-click or Enter edits the value as text. |
| Switch | LED on the block image | On draws the LED. Off draws an empty lamp. Disabled does not toggle. |
| Selector | Raised menu, current name, 32px row | Empty selection says what is missing (`No amp model`). Disabled keeps the name. |
| Block | Chain image, name, one LED | The selected block's editor is the single panel under the chain. |
| LED | 8px lamp, amber or the block's on-colour | Off is a dark well. On is not the only sign of state: the switch also says On or Off. |
| Display | One-line readout (tempo, channel, trim) | Read-only. No fake meter. |
| Tab | Family accent on the active tab, close control, dirty word | Hover lightens. Focus rings. The active tab is a fill, not colour alone. |
| Toolbar button | Text button on the bar | Hover and focus as above. Disabled explains itself in hover text. |
| Status chip | Short phrase in the status bar | Offline, present, disconnected, and uncertain use the sentences below. |
| Dialog | Raised window, scrim, title, body, actions | Focus starts on the safe button. Escape cancels. Tab cycles the buttons. |

## Keyboard

Tab order is top bar, tab strip, page controls, status bar. A modal dialog
cycles only its own buttons. Escape cancels. Enter activates the focused
button. Arrow keys move inside one group: a bank's slots, one block's knobs,
or one string's ♯ / ♭ buttons.

These shortcuts already exist in Help and stay:

| Key | Action |
| --- | --- |
| Ctrl+O | Open file in a new tab |
| Ctrl+S | Save a new copy |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z | Redo |
| Q | Quit |
| ? | Toggle Help |

The safe button is focused when a dialog opens: `No` on Detect devices,
`Keep editing` on unsaved changes, `No, stop` on a device-check step.

Dirty state is the word `Unsaved` on the tab and in the page header. A colour
may accompany it. Colour is not the only signal.

## Shell

The top bar is the product mark, the name Variax Workbench, and the commands
that apply to the whole window:

`Open file…` · `New offline tab` · `Detect devices` · `Save copy…` · `Device check…` · `Test recording…` · `Help`

`New offline tab` is one menu: `Variax (example banks)`, `PODxt Live`,
`POD HD500X`. The start page does not repeat those labels as a second row of
the same buttons. Build number and version leave the status bar and stay in
Help.

The tab strip shows every open document. A tab title is the family and the
document name. A device tab uses the connection phrases in
[Connection](#connection). Close is a control on the tab. The active tab uses
its family accent.

The status bar is one line: the page status, then the connection chip. Errors
use `danger` and stay on this line. A long error wraps inside the bar rather
than covering the page.

Test recording keeps its current panel. This proposal does not redraw it.
Firmware installation is not a button.

## Start

No devices are connected. The page is the title and three cards. The toolbar
already has Open file and Detect devices, so the page does not repeat them.

| Card | Opens | Icon |
| --- | --- | --- |
| Variax (example banks) | The built-in Custom 1 / Custom 2 example | `assets/ui/shell/variax.png` |
| PODxt Live | An empty offline tone, labeled as an example until a file is open | `assets/ui/shell/podxt-live.png` |
| POD HD500X | The HD file editor, with no file open | Text mark `HD`. There is no HD500X icon in the 64px set, and this design does not invent one. |

On a normal Linux build with USB detection, Detect devices is enabled and
opens the disclaimer. On a build without detection the toolbar button is
disabled, and its tooltip is the current sentence: `USB presence detection
is unavailable in this build.` That sentence is not a card under the grid.

## Variax

The page is the Workbench 1.75 arrangement, with the type, targets, and units
of this app.

| Region | Size | Contents |
| --- | --- | --- |
| Header | about 80, taller when the metadata row needs it | Bank, preset slots 1–5, model, author, notes, Undo, Redo, Unsaved |
| Middle | remaining | Guitar body, large, and the inspector |
| Bottom | 248 | Alternate tuning on the left, vector neck on the right |

The wood header, the body well, the brushed-metal inspector, and the neck are
CSS. The photos on this page are the Spank body and the Spank pickup sprite.
See `assets/ui/SOURCES.md`. The low-resolution `fretboard.png` is not stretched
across the neck. The neck is drawn in the page: nut, frets, six strings, and
inlays. No image generator was used for it.

Pickup sprites are `spank-pickup.png`, placed with the editor's painter.
Horizontal center is `68 + 318 * ((position clamped to 0.1…9.5) - 0.1) / 9.4`
and vertical center is `215`, both in the body image's pixel space, then
scaled with the image. Pickup 1 sits at 1.80" and is rotated 8.7°. Pickup 2
sits at 3.88" and is rotated 0.0°. The selected pickup is drawn last. On the
Pickups section, pickup 1 is selected and drawn with an amber highlight.
Body type and Controls draw both sprites without that highlight. The body
photo stays large on every section.

Position, angle, and level are a slider plus the formatted field. The slider
uses the edit domain: position 0.1–9.5 inches, angle −45–45 degrees, level
−18–6 dB. The field keeps the #192 string (`1.80"`, `8.7°`, `+1.3dB`).

The example the page shows is the built-in Spank example (Custom 1, slot 2).
`example_document` sets these values, and the labels are the Workbench formats
in `value_format.rs`:

| Control | Stored by the example | Shown |
| --- | --- | --- |
| Body | model `0x03100003` | Spank |
| Pickup 1 | `0x03110004`, position 1.8, angle 8.7, level 1.3 | Spank Single Coil Bridge, `1.80"`, `8.7°`, `+1.3dB` |
| Pickup 2 | `0x03110003`, position 3.88, level 1.4, angle left at 0 | Spank Single Coil Middle, `3.88"`, `0.0°`, `+1.4dB` |
| Tone resistance | 470 | `470k Ω` |
| Tone capacitance | 0.011 | `11 nF` |
| Volume taper | integer 1 | Audio |
| Output trim | 0.0 | `0.00dB` |
| Pitch offsets | six zeros | `0 semitones` on each string |

The example builder does not assign volume resistance, tone position, or
save-tone. The synthetic base preset stores volume resistance 500, tone
position 1.0, and save-tone 0, so the editor shows `500k Ω`, `100%`, and an
unchecked save-tone box. Position is `ValueFormat::POSITION` (`1.80"`).
Angle is one decimal and a degree sign. Level is a signed decibel. Trim is
two decimals. Percent truncates. Resistance labels use a space before Ω
(`500k Ω`, `470k Ω`, `1M Ω`).

The inspector has Body type, Pickups, and Controls. The body photo stays
visible on all three. The example currently opens on Pickups; the new page
does the same, and Body type remains one click away.

The only 1–5 controls are the bank's preset slots in the wood header. The
second 1–5 row under the body is not on the page. Workbench's five-position
pickup selector is not in this editor, and the mockup does not draw it.
`Artwork folder…` is not on the page.

The bottom-left pane is alternate tuning, off for the Spank example. Enable
is the on/off control. Each string, high E at the top, shows its note, a
flatten ♭ button, a sharpen ♯ button, and the offset `0 semitones`. Presets
stays as the named-tuning menu from the current editor.

`Get from guitar` and `Send to guitar` leave the header. They are disabled
dead ends (`locked during offline editing`). Reading and writing a guitar
stay in Device check, behind that window's own questions. The toolbar button
`Live access locked` leaves for the same reason. Spread stays off the Spank
page. The editor already says Workbench 1.75 had no Spread control.

Auto-select remains a checkbox with its current meaning: choosing a humbucker
sets 500 kΩ, 500 kΩ and 0.047 µF, and other pickups set 250 kΩ, 250 kΩ and
0.022 µF. The example opens with it on.

## PODxt Live

The page is a dark stage. The frame is `assets/ui/pod-farm/stage-frame.png`
(App.imb image `0x00010003`). The wooden venue image in that container is
not used. `bg_xtlive.png` (734×583, under the Line 6 Edit 3.06 extraction,
not committed) names the blocks. It is not drawn as a grid of open tiles.

The chain is one row, in the order Line 6 Edit's `UpdateBlockToggles` uses
when it lays out the chain buttons. A true pre/post value places that block
before the amp. Guitar EQ is forced post there, and the compressor is always
the block immediately after the amp. Stomp and Wah do not move. The FX loop
is absent on PODxt Live (`FXLOOP_ENABLE` is marked absent on Live), so the
row does not include it. Cab and mic are not chain tiles. They sit with the amp.

All-post, which is wire 127 on the live pre/post controls:

Gate → Wah → Stomp → Amp → Comp → EQ → Vol → Mod → Delay → Reverb

Delay moved pre, wire 0 on `DELAY_PREPOST`:

Gate → Wah → Stomp → Delay → Amp → Comp → EQ → Vol → Mod → Reverb

`MOD_PREPOST`, `DELAY_PREPOST`, `REVERB_PREPOST`, and `VOL_PREPOST` are the
movable guitar controls (`research/podxt-live-parameters.json`). `EQ_PREPOST`
is bass-only and has no Live CC. The offline mockup is the Post → Pre change
for Delay. The other PODxt mockups keep the all-post row so the connection
chip is the difference, except the presence-unknown frame, which selects EQ
so the four bands are visible.

Each block has one LED. There is no second toggle row. Stomp, Mod, Delay, and
Wah show the POD Farm 2 skin for the selected model. Reverb shows Lux Spring.
Vol shows POD Farm's volume pedal. Gate, Comp, EQ, and the chain's amp mark
are drawn in the page. Amp.imb faceplates name other models, including
`64 Lux`, so the amp editor uses one shared dark panel and sets the name in
the page. No generated pedal art was added. The models on this page all have
a POD Farm skin; there is no missing-pedal list for them.

The selected block's editor is the one panel under the chain. When Amp is
selected, that panel is the main section: model name Blackface Lux, cab
`1x12 Blackface`, mic `57 On Axis`, and large Drive, Bass, Mid, Treble,
Presence, and Volume knobs. Those six knobs are percent (`AMP_DRIVE` and the
matching amp definitions). The example shows the 50% midpoint. It is not a
captured performance. When EQ is selected, the same panel shows the four
PODxt Live bands (`EQ_FREQ1`–`EQ_GAIN4`). Bands 5 and 6 stay off this page
because they are bass-only. The pictured frequencies and gains are the source
reset words in `PODXT_EQ_RESET_WORDS`, displayed through the editor's
conversion: 130 Hz, 1000 Hz, 4500 Hz, 7400 Hz, each at 0.0 dB.

The example tone uses catalog names that exist in
`research/podxt-live-parameters.json`:

| Block | Model | LED |
| --- | --- | --- |
| Amp | Blackface Lux, id 131092, cab 1x12 Blackface, mic 57 On Axis | On |
| Stomp | Facial Fuzz, id 33882112 | On |
| Mod | Sine Chorus, id 33751044 | On |
| Delay | Analog Delay, id 33685506 | On |
| Reverb | Lux Spring, id 33816578 | On |
| Wah | Vetta Wah, id 33947648 | Off |
| Gate, Comp | — | Off |
| EQ, Vol | EQ post, volume pedal | On |

Tempo is `120.0 BPM`. There is no tap that sends anything. The link readout
is `(no Variax)` and `No linked model`. It does not send a guitar command.
There is no input meter and no output meter. The readout says `No live audio`.
The metadata strip is the tone name and `No channel · file only` when the
document has no program address. `Submit This Tone To Online Library` is
removed. Host-scale `0–1` labels and raw parameter ids leave the page. Raw
ids sit in a closed Details disclosure.

The status chip for this example is `Example tone · file · nothing is sent`.
A present USB device uses `USB detected · identity pending`. The page does
not say synced.

Three findings stay available from Parameter notes, which is closed on the
mockup: the compressor alias, mod and delay 14-bit rounding versus truncation,
and the volume pedal's 128 versus 127. They are not presented as a finished
live match. #219 binds the knobs to the table. This issue only places them.

## POD HD500X

Same shell. The page says `No HD Edit file open` until a file is chosen.
It shows the current notices, shortened only by wrapping:

- `HD Edit files are handled offline. Internal target and physical compatibility remain unverified.`
- `No device, audio, firmware or driver operation is available here. Copy saves never replace an existing file.`
- `Live connection not supported yet.`

The page does not invent amp, effect, or pedal controls. An open file shows
its file name. A name is not shown for bytes that are not an HD Edit file.

## Device check

The window keeps every safety sentence. It drops gate codes and
`real device: locked` from the primary row. Those move into Details, which
is closed in the mockup.

The window opens on This computer. There is no Simulation mode and no mode
selector. #228 is the lane 1 change that removes Simulation and makes long
content scroll. This proposal matches that window: the consent, the steps,
and a result that is taller than the window scroll inside it. `No, stop` is
focused. Nothing in the picture has run. Steps 2, 3, and 4 are `not run`.
The Yes label is `Yes, read this computer's USB list`.

Step 1's primary text is the current action and risk, without a gate letter
in the heading:

> Look at what Linux already knows about connected USB devices: vendor and
> product numbers, the description Linux saved when the device was plugged in,
> which Linux driver owns each part of it, and the list of sound cards. The
> app sends nothing to the PODxt Live or the guitar.

> Low risk. The app only reads information Linux keeps. When the PODxt Live
> was plugged in, Linux's own drivers already talked to it (for example the
> Variax driver asks for a version); that happens without this app. If no
> device, several devices or unreadable information is found, the check stops
> instead of guessing.

The standing rules stay on the window, in the current words: each step asks
first, the answer is No unless you choose Yes, and a passed step never starts
the next one. Close stops a step before it starts. Backup and tone sections
that the current window already offers stay behind their own Yes, and this
set of mockups does not redraw them.

## Detect devices

The dialog is the current copy:

- Title: `Detect devices`
- `Tested on Linux: PODxt Live 3.01 and Variax 600 3.10.`
- `Scan cached USB information for supported devices? This scan sends no device requests.`
- Buttons: `Yes, scan` and `No`. `No` is focused.

A finished scan still reports `No supported devices found.` or
`Detected N supported device(s).`

## Save and close

The dialog is the current copy, over the dirty document:

- Title: `Unsaved changes`
- `Save a copy to keep your changes before continuing.`
- Order: `Save copy and continue…`, `Keep editing`, `Discard changes`
- `Keep editing` is focused. Escape matches it.

Save copy remains a new file. It does not overwrite.

## Connection

The chip and the tab title use the current sentences. None of them say synced.

| State | Tab | Status |
| --- | --- | --- |
| No device | Family and file name | `No device` |
| Present, identity pending | `PODxt Live · identity pending` | `USB detected · identity pending` |
| Present, linked bridge | the model name plus `identity pending` | `USB bridge detected · guitar identity pending` |
| Uncertain | `PODxt Live · presence unknown` | `USB presence could not be verified` |
| Disconnected | `PODxt Live · disconnected` | `Disconnected · editing the open file` |
| HD, file only | `POD HD500X` | `Live connection not supported yet` |

`No device` is the design phrase for an offline tab. The current status string
for a tab that was connected and then lost is `Disconnected · offline editing`.
The disconnected mockup uses that situation's tab title and the design chip
`Disconnected · editing the open file`, which says the open file is what you
are editing.

## What stays unsupported

Unsupported work stays on screen and is labeled. The mockups do not show a
successful guitar write, a synced preset, a live meter, an HD connection, a
firmware installer, or a five-position edit.

| Item | How it appears |
| --- | --- |
| Five-position pickup selector | Not drawn. The header 1–5 are preset slots |
| HD live connection | `Live connection not supported yet` |
| Identity before a real read | `identity pending`, never synced |
| Amp knob percents on the example | 50% midpoint, not a captured tone |
| Detection missing from a build | Disabled Detect devices, tooltip sentence |
| Step 4 and any later write | Not offered by this picture. Device check lists step 4 as `not run` |
| Simulation mode | Removed. The window is This computer. #228 implements that |

## Art

Committed images and their origins are `assets/ui/SOURCES.md`. The extractor
is `scripts/extract_xgim.py`. It records numeric XGIM ids. Those ids are not
file names. Extracted containers stay in the ignored `analysis/` tree.

Knobs, wood, metal, the shared amp panel, and the neck are drawn in the
prototype. They are not crops of the Workbench chrome sheet or of
`background.png`. POD Farm skins used on the chain are listed in
`assets/ui/SOURCES.md`.

## After approval

#216 builds the components in `src/shared/ui` from this inventory. #217 applies
the shell, start page, tabs, status, and dialogs. The Variax and PODxt lanes
restyle their own editors with those components. This issue does not edit
`src/families/` or device code, and it does not change README.md, because the
design is not an implemented behavior.
