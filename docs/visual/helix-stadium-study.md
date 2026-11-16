# Visual modernization study — issue #41

Research milestone, not an implemented GUI or closed acceptance checklist.
Reference: the owner-supplied “Helix Stadium Native v1.4.1 Installer.exe”.
No vendor program was executed. No device, audio, network or authorization
service was accessed for this investigation. All proposed artwork is original.

## Provenance and reproducible private evidence

The supplied artifact is 558,857,808 bytes, SHA-256
`db091b7ba465bc76f8e69240efc2101b611fbd272d82c148d2a0badc6b2991df`.
The owner supplied it from Downloads; the assigned checkout received the same
bytes through the coordinator's read-only private input inventory. Source mode
was 0444. This establishes identity with the supplied reference, not publisher
authenticity, signature validity or latest-release status.

Inventory SHA-256:
`ae6f747b2dcf4d52244e6106f11465eadfdb4654983467310695bd9e31c59184`.
Policy SHA-256:
`e34ecaafc47dc2385eb9fa281d6bd431151c671b03fbb7e1704ce0715eb7d34c`.
Private source relative to this task checkout:
`.tools/coordinator-inputs/installers/Helix Stadium Native v1.4.1 Installer.exe`.

Static `file` identification reports an x86 PE/Nullsoft installer. Trusted
7-Zip 26.06 identifies its embedded archive as NSIS-3, Deflate, non-solid,
with 2,929 entries. Listing and extraction used 7-Zip, never installer switches:

```sh
.tools/7zip/7zz l "$SOURCE"
.tools/7zip/7zz x -aos -oanalysis/issue-41/extracted "$SOURCE"
```

`SOURCE` denotes the exact private source above. Before extraction, independently
stat/hash the source against the inventory. Use a new private output directory;
`-aos` refuses overwrites. The original is unchanged. Extracted executables are
data only; never launch or load them. The complete container was unpacked but
only visual XML, font filenames and UI/version-related binary strings were
inspected. DSP, model definitions, license and activation resources were not
investigated. Extracted resources and logs remain ignored under `analysis/issue-41/`:
`installer-list.txt`, `extract-log.txt`, `visual-source-index.json`, `extracted/`.
Do not commit these files, font files, icon atlases or vendor screenshots.

The VST3's static UTF-16 version-resource strings identify “Helix Stadium Native
VST Plugin”, internal name `com.line6.helixstadiumnative.vst`, and file/product
version `1.4.1.1`. This is internal product evidence, not independent signature
authentication. XML uses the `xg` root and XG widget vocabulary, with P35/C63
application widget classes; the binary contains `XGWidget` RTTI names. Together
these support an XG widget system. The exact framework release and ownership
are **unresolved**. No JUCE/Qt identification was established; absence of searched
strings does not prove absence of a dependency. No framework version is invented.

## Source citations and evidence classification

All source paths below are relative to the private extracted
`$APPDATA/Line 6/Helix Stadium Native/res/xg_scripts/` directory. IDs identify XML
elements rather than runtime screenshots. These are resource observations (O).
Runtime composition and behavior remain unobserved. Proposed adaptations (P)
and reconstruction (R) must not be read as vendor behavior.

| Source / SHA-256 | Observed evidence |
| --- | --- |
| `window_main.xml` / `b53c1a7a007c6f1c1e541cec20a5110fdc2e3780c3cfb9488e3e692f72b4522d` | O: `main` 1024×768; `tpbc` top 0 height 60; `inbr` top 62 height 48; `edit` top 112 right anchor 250; `stat` bottom height 20; right `scvc` width 250. |
| `view_home_edit.xml` / `b4be9ae4f207b1d5cd77a5c106f8771e7aa365bcd1361b20753b4a3c43789a73` | O: proportional split, signal-flow minimum 240 and inspector minimum 128; 44-unit meter gutters; model pane minimum 400 and parameter pane minimum 200; category list 150 wide, 30-unit rows; rounded upper inspector corners 8. |
| `view_topbar.xml` / `7d40a70a2cfe85c333b898eb64953796e53afa98c73b946e4c23732ddc24e729` | O: 60-unit top-bar container, `C63TopBarView`; product/account/trial widgets exist. P: replace account/trial area with local file actions and offline status. |
| `view_instancebar.xml` / `791efc416585f50487ec9e481b5b3bd84c1e0be6874d00a4e66e4c988f850f7e` | O: 48-unit context strip, instance name, A/B and tempo controls. P: context strip shows family/document/slot; no inherited tempo or A/B capability. |
| `widget_file_browser.xml` / `6d1ede9043daab84546a4d3b9719470633eb964f85a44a5f426a32c1cfeeb6ae` | O: search panel 28 high; rounded 6-unit corners; tree and column browser structures; 30-unit rows and explicit selection colors. |
| `widget_param_knob.xml` / `e336356073ae8216504929282cb69ea18a94b683f771a9952feb1f2635b9cfdf` | O: 115×129 cell, 83×83 knob at top 26, numeric entry above, centered label below, Inter Display 14; hidden sync control and assignment markers. P: omit unsupported assignment markers. |
| `widget_param_slider.xml` / `c973d757495f6aa99fa59dc61088f6a224acf2b7550fb69877561b8c1b2bb3af` | O: separate slider parameter widget exists. Actual drag response unobserved. |
| `widget_param_segmented.xml` / `3e03a28d20c6cd82fd571e484cbbe7a7752bc9e30db2dcd92cf0102e06ad7dd0` | O: separate segmented parameter widget exists. Runtime state binding unobserved. |
| `view_statusbar.xml` / `373fdb60738c2a22ee82d29bd8e66fab66e2329cd17b26643c1246bed22f92ac` | O: 20-unit status strip with text and progress widgets. |

Resource dimensions are authored coordinates, not measured physical pixels.
Some child layouts have nominal dimensions exceeding the parent's default;
anchors, splitters and custom classes resolve this at runtime. The mockups are
reconstructed compositions, not pixel-accurate screenshots. The right scheduler
resource is not evidence for a preset browser at that location: repurposing that
space as a local library is an explicit proposal.

## Visual language and workflow mapping

Retain the stacked global/context bars, large upper task canvas, divided lower
inspector, narrow right utility panel and bottom status strip. Preserve this
hierarchy across guitar and POD pages; avoid a palette-only repaint. The original
mockups use 1280×800 logical coordinates with 60/48/24-unit chrome and a 250-unit
right library. The lower inspector occupies approximately 40% of the workspace;
its left side selects a category/model, its right side exposes named values.
These proportions are P, informed by O above.

| Reference structure | Guitar / original Variax 600 | PODxt Live / librarian |
| --- | --- | --- |
| Global bar | Guitar, POD, Recording, Library, Maintenance navigation; explicit offline badge | Same shell and file actions; family always visible |
| Context strip | Local document, bank/slot, unsaved marker | Local tone name, source file, assigned bank/slot metadata |
| Upper signal/task canvas | Original guitar silhouette and two **virtual modeled** pickup markers; never label piezo sensors as virtual pickups | Fixed stage cards for independently supported processing roles; illustrative cards do not assert validated parameter domains or arbitrary routing |
| Lower model/category selector | Body, Pickups, Wiring, Tuning, Controls; separate semantic pages | Library selection and recovered metadata; unknown effect/model IDs remain raw IDs with “unverified” |
| Parameter inspector | Existing validated document fields/units, numeric edit and undo grouping | Unsupported amp/cab/effect parameter editing shows reason, never plausible invented values |
| Right utility | Local bank rows with slot/name/dirty state | Family-scoped tone library; file identity, source and bank/slot positions |
| Bottom status | Non-overwriting save outcome, validation/error feedback | Same persistence feedback; never “synced” without hardware evidence |

Settings and maintenance remain separate destinations with capability summaries.
Entering any page never enumerates hardware or opens an audio stream. Library
filters never imply cross-family preset conversion. Recording navigation must
retain the existing offline/file-based scope. No Helix model, format, assignment,
snapshot, scheduler or account workflow becomes a Variax/POD capability.

## Original review mockups

- [Guitar editor](guitar.svg): selected virtual pickup, local editing, library and
  unsaved state, permanently disabled live actions.
- [POD library](pod-library.svg): local selection, bank/slot metadata, unsupported
  parameter editing and successful local copy feedback.
- [Empty/error/save states](states.svg): empty library, malformed-file error,
  existing-destination save refusal and keyboard focus state.

All are original SVG geometry and text using system sans-serif. They embed no
vendor image/font/icon and contain no external URL or script. Synthetic preset
names and values are examples, not compatibility fixtures. A highlighted card
means local selection, never observed device state. These static mockups do not
exercise interaction, accessibility APIs, file saves or GUI state machines.

## States and interaction contract (proposal)

Normal rows use a dark flat surface; hover lightens it; selection adds a bright
left marker and the word/shape of selection. Keyboard focus adds a separate
2-unit cyan outline, so focus and selection remain distinguishable. Pressed
buttons inset visually without moving their labels. Dirty documents use a text
“Unsaved” marker. Never rely on color alone. Disabled live actions retain readable
text and show “Hardware access locked — offline phase” beside them, with the same
explanation available by keyboard. Unsupported parameter controls instead say
“Parameter definition unverified”; a lock and missing implementation are distinct.

Local save feedback must use the existing atomic, non-overwriting persistence:
“Saved new copy: …” only after success; cancel retains the pending action;
existing path refusal offers a new destination and keeps edits. Errors stay
visible until dismissed or resolved. Save/discard/cancel prompts block background
edits, slot changes, undo and shortcuts, preserving existing GUI semantics.
Empty libraries offer Open local file; parse errors preserve prior selection and
document. Operations use progress plus textual status; cancellation does not
claim rollback or recovery. No animated meters appear without actual authorized
data: use “No live audio” instead of simulated live levels.

## Theme and reusable Rust components (proposal)

The existing GUI uses pinned `eframe = 0.33.3` / egui with glow, default fonts,
Wayland/X11, and `rfd` behind the file-dialog adapter. Keep that stack; no new
runtime dependency, webview, vendor renderer or bundled vendor font is proposed.
Use egui's existing fonts first and measure labels before changing typography.

| Token | Proposed value | Evidence / purpose |
| --- | --- | --- |
| canvas / inspector | `#000000` / `#0f0f0f` | O: home-edit XML backgrounds |
| raised / selected / hover | `#222222` / `#424242` / `#5e5e5e` | O: browser/button/list treatments; exact blending unobserved |
| primary / secondary text | `#ebebeb` / `#c0c0c0` | O-inspired; P: readable labels |
| focus / warning / error | `#78dce8` / `#ffd580` / `#ff9c9c` | P: explicit accessible status palette |
| spacing | 4, 8, 12, 16, 24 logical units | P: coherent spacing scale |
| corner / focus border | 6–8 / 2 logical units | O-inspired corners; P focus visibility |
| body / title | 14 / 18 logical units | P: increase small reference labels |
| target / list row | minimum 32 / 32 logical units | P: larger than observed 28/30 controls |

Reusable pieces: `RigShell` (navigation/context/status), `CapabilityNotice`
(reason + disabled action), `DocumentStrip` (family/source/dirty),
`FamilyLibraryRow` (slot/name/selection), `TaskCanvas`, `SplitInspector`,
`ParameterCell` (label/value/unit/domain), `SaveFeedback`, `BlockingDirtyPrompt`.
These are design names, not claimed implemented Rust types. Bind them to existing
Session/backend capability results and EditorDocument history, not duplicated
visual state. Keep file dialogs and OS behavior behind current adapters. Theme
configuration must not alter coefficient profiles, codecs or protocol policy.

Keyboard order: navigation → context/file actions → canvas selection → inspector
categories → values → library → status actions. Tab/Shift-Tab traverses controls;
arrows move list selection; Enter activates; Escape cancels dialogs and returns
focus to the invoker. Ctrl+S uses existing save semantics; text entry must retain
undo/edit keys. All knobs also have named numeric/keyboard entry; sliders expose
units, bounds and current value. Announce errors and save result via accessible
text; actual screen-reader support must be validated separately on each platform.

At wide sizes retain the right 250-unit library; below the space needed for
400+200 inspector columns and library, make Library a toggled drawer and stack
inspector sections. At narrow widths allow vertical scrolling, never shrink
labels below readable size. Persist user split positions with bounds checks.
Use egui logical units and pixels-per-point; validate 100%, 150%, 200% scaling,
text expansion, long filenames and 1024×768 through 1920×1080 layouts. XML anchor
rules suggest intended resizing; actual vendor high-DPI behavior is unobserved.
Text contrast target is 4.5:1; focus/control boundaries target 3:1 against adjacent
surfaces. Selected text and disabled explanations must meet the text target;
low-contrast decorative dividers are not control boundaries. Verify actual
rendered alpha/composition rather than assuming resource hex values suffice.

## Migration and focused follow-up proposals

These are proposed issue scopes for Gatekeeper triage, not new assignments or a
second task queue. Link their eventual GitHub issues to #41 and the named parents.

1. Under #26: introduce theme tokens and shell composition using current egui;
   preserve operation policies, document lifetime and dirty-close semantics.
   Validate focus order, resize/scaling and headless navigation before rollout.
2. Under #26: family-scoped library/inspector components with capability reasons;
   reuse current file/history APIs and existing offline interaction tests.
3. Under #29, informed by #40: apply the shell to POD librarian metadata only;
   enable parameter widgets only after independent domain/source validation.
4. Guitar presentation follow-up: original vector guitar/pickup visuals with
   explicit virtual-pickup meaning; preserve tuning/wiring units and grouped undo.
5. Desktop/accessibility follow-up: native windows, screen readers, contrast,
   keyboard focus, OS pickers, scaling and long-label review on each platform.

No broad production GUI change is included here. #40 retains POD Farm behavior
research; this study supplies visual direction only. #41 remains open until
reviewed evidence, follow-up tracking and all acceptance gaps are reconciled.

## Validation and unresolved gaps

Completed research: source identity/size/mode verification; static container
inventory/extraction; nine source XML hashes and parse counts; 51 UI XML files
identified; internal product/version strings and XG type-name evidence; original
vector mockups and this migration specification. Source was hashed again after
research. XML/SVG parsing and color-pair contrast checks are focused structural
checks, not native GUI tests or product compatibility evidence. Three SVGs parsed
and rendered with rsvg-convert for visual inspection; no external assets/scripts
were referenced. Four solid-color text/background pairs passed 4.5:1: primary
on selected 8.43:1, secondary on selected 5.52:1, focus on raised 10.00:1,
error on raised 7.95:1. These checks do not validate every rendered state.

Unresolved: publisher signature authentication; exact XG framework version;
custom-class runtime composition; actual icon/knob rendering, animation, tooltips,
focus/error states, font metrics, high-DPI behavior and runtime resizing. No
reference screenshots were captured and no vendor renderer was executed. The
mockups reconstruct hierarchy, not a verified runtime screenshot match. Follow-up
issues are proposed above; they have not been created or accepted by this study.
No Rust GUI behavior, hardware support, safety, recovery or audio equivalence is
established by this work. Existing hardware locks remain unchanged.
