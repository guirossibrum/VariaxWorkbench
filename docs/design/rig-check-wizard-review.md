# Rig Check wizard owner review checklist

Owner design approval is pending for [#248](https://github.com/guirossibrum/VariaxWorkbench/issues/248).
The device and guided step release in #267 covers the owner's rig run.
Use the [screen index and open design questions](../rig-check/design.md)
and the [guided prompts proposal](rig-check-guided-prompts.md) during review.

## Generated evidence

The original 34-image set was redrawn on 2026-11-26 on Linux x86_64 with rustc 1.99.1
(58cf8f9c3 2026-10-13), from master
`b3354b46bc8ad683c144f48aa9442e7ec76a85c7` with the #248 fixture additions.
The fixtures use generated Practice records and demo strings, including the
Real rig card example.

Reproduce with the USB feature off, using an empty output directory:

```sh
cargo build --locked --offline --no-default-features --features gui --bin rig-check
"${CARGO_TARGET_DIR:-target}/debug/rig-check" --mockups /path/to/empty/output
```

`--mockups` renders through egui on the CPU and exits during option parsing,
before the program constructs a real port or opens a native window. The
software renderer lays out logical points at each scale and rasterizes the
resulting meshes in physical pixels. The current 38 PNGs are 8-bit RGBA.

| Set | Count | Logical size | Scale | PNG size |
| --- | --- | --- | --- | --- |
| Numbered screens 01–14 | 14 | 1180×800 | 1× | 1180×800 |
| Laptop examples | 12 | 1120×760 | 1× | 1120×760 |
| Retina examples | 12 | 1120×760 | 2× | 2240×1520 |

The four blocked-continuation images were drawn on the same platform and
compiler from master `5eddd174f0d848c1a3692e816d78456ce83afa6d` with the
#248 continuation fixtures. They use the merged #297 wizard and a synthetic
attempt report. The [blocked-continuation index](../rig-check/design.md#blocked-continuation-at-laptop-and-retina-scales)
links both cleanup outcomes at each scale.

Two draws at that checkpoint produced 38 byte-identical PNGs and matched
all 34 previously tracked images. The SHA-256 of
`LC_ALL=C sha256sum *.png` for that checkpoint is
`c4624368930fea7f98ec6809e31a5d8ad11cba3f7610caa3584e3c29506425a9`.

The laptop and Retina verified-release images were redrawn from
`f542d3fa365a08fe89b2d95ff28a39b4d10bcfb7` with the owner-approved
**Open source report** label. These two images also show the build footer
already present at that revision. The other images retain their earlier
renders.

The clocks in 05 and 06 start immediately before rendering. The stills
show the start of the read and the selector countdown, respectively.
Laptop and Retina fixtures show the VDI card and its first Get ready
prompt, two later cards, and paired cards and passing results for 0.2,
1.2 and 6.2. Each screen shows its run position and catalogue ID.

## Checklist

Mark each row **Approve** or **Change**, with a short note for changes.
Keep owner screenshots private.

| # | Criterion | Headless evidence | Owner review | Mark |
| --- | --- | --- | --- | --- |
| 1 | Choose a mode | 01 shows the Linux mode choices. Practice receives focus and Enter selects it. The mode is unset until selection. | Review the Practice focus default and native platform copy. | |
| 2 | Start notice and safe default | 02 shows Quit and Accept and continue. The keyboard test checks that Enter quits. | Check the native focus ring on Quit. | |
| 3 | Each step waits for its own confirmation | The card keyboard tests cover Enter to Skip, R to run and Q to stop. The Practice channel test covers the card and result sequence. | Confirm the flow with the keyboard in Practice. | |
| 4 | Step card explains the action and risk | 03, 04 and the laptop and Retina cards show What it does, What you'll see and hear, Risk, gates and prerequisites. | Review readability and the Records line. | |
| 5 | Large SEE and HEAR | The card and VDI examples render the 34-point labels at 1× and 2×. | Check readability from playing position. | |
| 6 | Running status | 05 shows the current action, count, elapsed time, Stop and the controls notice. | Review a moving progress count if needed. | |
| 7 | Get ready and physical action | 06 shows a selector countdown. Both VDI Get ready examples show the revised wait instruction, with OK, Skip and Stop. | Review OK as the focus default and the countdown-to-action sequence. | |
| 8 | Retry after a missed action | 07 shows Skip, Try again and Stop. The wizard focuses Skip. | Check the native focus ring. | |
| 9 | Listening question | 08 shows Yes, No, Not sure, the optional note and byte-check evidence. The wizard focuses Not sure. | Review the prompt and focus ring. | |
| 10 | Separate save notice | 09 shows the demo Spank 1 slot, backup reference and exact confirmation, with Save disabled. The keyboard test checks that Enter on the initial notice skips. | Check the Skip focus ring plus disabled and enabled Save states. | |
| 11 | Step outcome and display check | 11 and 14 show the result, Continue and the Different… field. The keyboard tests cover Continue and recording the typed difference. | Review the focused Continue and the mismatch flow. | |
| 12 | Progress and stable references | 03, 05, 10 and 11 show the progress list. The paired fixtures show Step 21 of 35 / Catalogue ID 7.4 and Step 29 of 35 / Catalogue ID 4.7 at both scales. | Review progress-list placement and the run heading alongside the report IDs. | |
| 13 | Summary and report | 10 shows the rig state, counts, a demo preview line and Save report…. | Review a completed Practice summary and whether to offer Open folder. | |
| 14 | Back and Return | 12 and 13 show review of an earlier result and the accepted start notice. Keyboard tests cover returning to the pending card. | Review focus and wording in native Practice. | |
| 15 | Narrow laptop view | Twelve samples at 1120×760 show the headings and controls. The sidebar shortens longer titles, and the 6.2 row needs scrolling. | Review the native layout and whether the sidebar should follow the active step. | |
| 16 | Retina view | The same twelve samples render at 2 pixels per point. | Review native scaling and focus on a Retina display. | |
| 17 | Screenshots of wizard states | The 14 numbered images and 24 narrow images cover the generated review set. The paired 0.2, 1.2 and 6.2 cards focus Skip and their results focus Continue. | Keep native screenshots privately, including focus states. | |
| 18 | Blocked continuation | The four screenshots show 3.3 blocked by 2.2 and the expected and observed CC 120 bytes. Verified release focuses Open source report and offers Start fresh. Failed cleanup shows the manual-check instruction and focuses Open report, which opens the attempt inline. | Review the mismatch, cleanup instructions, focus and report destinations. | |

The Mac and Windows choose-mode copy remains for native review.
The VDI Get ready example covers the unplug
phase. Reconnect and identity-check states remain for native review. The
owner questions about progress-list placement, the evidence line, Open
folder, Practice focus and Get ready focus remain open.

The central panels and focused controls in the paired results and blocked
continuation screens fit at both scales.
The 6.2 sidebar starts at the top and omits the active row from the
initial view. The older 01, 02, 07 and 08 fixtures render with focus
disarmed, so their images do not show the keyboard defaults listed in
the checklist. Those defaults remain part of native review.

## Rendering checks

`mockups_render_headless` covers the numbered fixture set and a software
raster frame. `laptop_mockups_show_run_position_and_catalogue_id_at_both_scales`
checks the heading, stable ID and primary action at 1× and 2× for all ten
narrow fixtures and checks the 2× raster dimensions.
`inventory_interface_and_guard_mockups_have_evidence_and_safe_defaults`
checks representative text and confirms that Enter selects Skip on each
new card and Continue on each matching result at both scales. Existing
headless tests cover run and stop shortcuts, notice defaults, history
and run ordinals across retries.

`blocked_resume_mockups_fit_and_focus_open_report_at_both_scales` checks
the full text bounds against the viewport and its clip at 1× and 2×.
Enter activates Open source report after verified release and Open report
after failed cleanup. The first opens the source file and the second
opens the attempt preview inline. The existing
blocked-resume keyboard tests cover deliberate Start fresh without a
continuation plan and verify that failed cleanup cannot restart the run.
