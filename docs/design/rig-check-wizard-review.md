# Rig Check wizard owner review checklist

Owner design approval is pending for [#248](https://github.com/guirossibrum/VariaxWorkbench/issues/248).
The device and guided step release in #267 covers the owner's rig run.
Use the [screen index and open design questions](../rig-check/design.md)
and the [guided prompts proposal](rig-check-guided-prompts.md) during review.

## Generated evidence

The mockups were redrawn on 2026-11-26 on Linux x86_64 with rustc 1.99.1
(58cf8f9c3 2026-10-13). They draw the wizard from master
`8988335d342fe961c60f8815090d1bb867deebbd` with this checkpoint's renderer
and fixture changes. That base contains the run-position merge
`c2b9caf` and the VDI replug merge `35102eb`. The fixtures use generated
Practice records and demo strings, including the Real rig card example.

Reproduce with the USB feature off, using an empty output directory:

```sh
cargo build --locked --offline --no-default-features --features gui --bin rig-check
"${CARGO_TARGET_DIR:-target}/debug/rig-check" --mockups /path/to/empty/output
```

`--mockups` renders through egui on the CPU and exits during option parsing,
before the program constructs a real port or opens a native window. The
software renderer lays out logical points at each scale and rasterizes the
resulting meshes in physical pixels. All 22 PNGs are 8-bit RGBA.

| Set | Count | Logical size | Scale | PNG size |
| --- | --- | --- | --- | --- |
| Numbered screens 01–14 | 14 | 1180×800 | 1× | 1180×800 |
| Laptop examples | 4 | 1120×760 | 1× | 1120×760 |
| Retina examples | 4 | 1120×760 | 2× | 2240×1520 |

Two draws produced identical PNGs. Ten numbered images changed from the
previous set and four matched byte for byte. The SHA-256 of the
filename-sorted `sha256sum *.png` output from the image directory is
`096aa0c9f73b2d34342abdc22b39161b5f4c74b6fe991a7da831582984a85bf7`.

The clocks in 05 and 06 start immediately before rendering. The stills
show the start of the read and the selector countdown, respectively.
Laptop and Retina fixtures show the VDI card and its first Get ready
prompt, and two later cards in the same demo visit order. The run heading
shows both progress and the catalogue reference on all eight images.

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
| 15 | Narrow laptop view | Four samples at 1120×760 show the existing headings and controls. | Review native window layout and any states outside the sample. | |
| 16 | Retina view | The same four samples render at 2 pixels per point. | Review native scaling and focus on a Retina display. | |
| 17 | Screenshots of wizard states | The 14 numbered images cover the existing review set. Eight additional images cover the four narrower fixtures. | Keep native screenshots privately, including focus states. | |

The Mac and Windows choose-mode copy, individual cards 0.2, 1.2 and 6.2,
and the #297 continuation screens are outside this image set. The
revised VDI Get ready example covers the unplug phase. Reconnect and
identity-check states remain for native review. The owner questions about
progress-list placement, the evidence line, Open folder, Practice focus
and Get ready focus remain open.

## Rendering checks

`mockups_render_headless` covers the numbered fixture set and a software
raster frame. `laptop_mockups_show_run_position_and_catalogue_id_at_both_scales`
checks the heading, stable ID and primary action at 1× and 2× for all four
narrow fixtures and checks the 2× raster dimensions. Existing headless
tests cover skip, run and stop shortcuts, notice defaults, history and
run ordinals across retries.
