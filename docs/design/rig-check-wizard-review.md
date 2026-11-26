# Rig Check wizard — owner review checklist

[#248](https://github.com/guirossibrum/VariaxWorkbench/issues/248).
Redrawn on 2026-11-26 from source
`e2a0e1a303dc7e1502247cb70105fb4a5a304ba8`. This checkpoint does not change
Rust sources. It records the redraw. It is not owner approval.

Screen index and the three open layout questions:
[docs/rig-check/design.md](../rig-check/design.md).
Headless pictures: [docs/rig-check/mockups](../rig-check/mockups/).

## Approval on record

No owner approval of this wizard design is on record at this head.
[design.md](../rig-check/design.md), [the roadmap](../roadmap.md) and
`src/rig_check/gui/mod.rs` each say approval is pending. The 2026-11-24
approval in [architecture-families.md](../architecture-families.md) is the
app-shell delivery. The 2026-11-25 #267 decision releases device and guided
steps for the owner's first real run. Mark each row below. A merge of the
wizard code leaves this issue open until those marks exist.

## Provenance

| Item | Value |
| --- | --- |
| Source revision | `e2a0e1a303dc7e1502247cb70105fb4a5a304ba8`. Rust files are unchanged in this checkpoint. |
| Date | 2026-11-26 |
| Host | Linux x86_64 |
| Compiler | rustc 1.99.1 (58cf8f9c3 2026-10-13) |
| Build | `cargo build --offline --no-default-features --features gui --bin rig-check` |
| Shared build path | `/home/grbrum/Documents/Code/VariaxWorkbench/target/shared-builds/lane2` |
| Enabled features | `gui`, `podxt-table`. The `usb` feature was off for the draw. Recorded in `debug/.fingerprint/variax-workbench-9b142a1e1738d4a5/bin-rig-check.json`. |
| Binary | `target/shared-builds/lane2/debug/rig-check` at the draw. `ldd` listed no libusb. Dep-info named this worktree (197 paths) and no file from the main checkout. The later `--all-features` test build replaced that executable, so the path links libusb after the suite. The draw fingerprint above still records `gui` and `podxt-table`. |
| Command | `rig-check --mockups` into an empty directory. Exit 0. About 7.0 seconds. A second empty directory produced the same 14 SHA-256 values. |
| Inputs | `mockup_states()` in `src/rig_check/gui/mod.rs`. Synthetic step records, the fixed save-notice strings, and the summary time `2026-11-25 22:05:00 UTC`. No device, capture, or private report. |
| Previous bytes | All 14 files were last committed in `c7ede9f`. Thirteen of this redraw match those bytes. `01-choose-mode.png` changed (126,069 bytes to 117,625; 18,255 pixels differ). |

All 14 PNGs are 1180×800, 8-bit RGBA. `raster::render` draws them at one
pixel per point (`src/rig_check/gui/raster.rs`). The window asks for the
same logical size (`src/rig_check/bin.rs`).

SHA-256 of this redraw, identical across the two runs:

| File | SHA-256 |
| --- | --- |
| `01-choose-mode.png` | `c5d7cfeb4122419f12bfd878dd090f79f1bc9674df57d7b6e49fdb780f955e47` |
| `02-start-notice.png` | `8c4815357e40399b7b1676eb01d0707a60c37893df433370045d235d3b6f417f` |
| `03-step-card-read.png` | `d283f8f1eb7a480356863b0ab5f1ac593f356895f38d3a78e87a62d9c6ae223d` |
| `04-step-card-live.png` | `4c55d375a5c5545e3e963cee73b34b8c01c14b7c062a2ac40c34b4f5d27974c0` |
| `05-running.png` | `d9d1b26c580fc8bf62043c6f20f5c192979961235fbb00f9d7df8d31a68174bf` |
| `06-listening.png` | `c603ea3c2bedf12b513c7c9d6c36f53360b46cae718423c29d5bd818b44db367` |
| `07-not-seen.png` | `55baea72d885af7292deed273aa213db4961c406b779d00a4a077d09ebda80de` |
| `08-listening-question.png` | `1e6f24831850635dee9ab6f1509a525df93c09578b584df72971a69ac4e1a7ec` |
| `09-save-notice.png` | `6d3f883ceb5121d5b1708a2d035b554b4274f6be051bbeb19147f3049a0b7fb7` |
| `10-summary-preview.png` | `86c1cd8c4e3bc13d43d91a9cb5265418c04a198f28fbc1a2bea34fa3fed2609b` |
| `11-step-result.png` | `43eaf1a36737f921de86d9722d4477fbca06a1a79dd84aa116164f794f81f597` |
| `12-review-result.png` | `e016db56a05c8e57e64f849e001759d7b087990d2cd7b3743ab7b3128f95d998` |
| `13-review-start.png` | `131800ed040f218ba5c69efd9fa5f7391f5dcec018713aaabfe1dd192a3da15e` |
| `14-display-difference.png` | `ab0960bb034227ce2ef47c10d99959f5bda85f283f08839980533b03ecf7d48e` |

`01-choose-mode.png` now reads: "Practice uses simulated devices. Real rig
uses the connected PODxt Live and Variax. Each device step waits for you to
choose Run." The `c7ede9f` file said Practice "sends nothing to your rig"
and that each device step "needs the owner's approval in this version."
The buttons remain Practice and Real rig. `mockup_states` sets
`focus_armed` false, so the picture has no focus ring.

This Linux binary contains that Linux sentence. It does not contain the
Windows sentence "This computer runs computer-only checks." or the Mac
sentence "This Mac runs computer-only checks." Those are `cfg!` branches in
`Wizard::central`.

`05-running.png` shows Elapsed 00:02. `06-listening.png` shows Elapsed 00:02
and GET READY 8 s. Both runs matched those digits and matched the `c7ede9f`
bytes. The digits come from `Instant::now()` at state construction, then
whole seconds when that screen is rastered (`src/rig_check/gui/mod.rs`).
A draw that reaches those screens on a different whole second would change
those two files.

## Exit before port construction

`main` calls `options()` first (`src/rig_check/bin.rs`). With `feature =
"gui"`, the `--mockups` arm inside `options()` calls `write_mockups` and
then `std::process::exit(0)`:

```rust
"--mockups" => {
    let dir = PathBuf::from(args.next().ok_or(USAGE)?);
    for path in rig_check::gui::write_mockups(&dir)? {
        println!("{}", path.display());
    }
    std::process::exit(0);
}
```

That exit is the end of the process. `RealPort::new` runs only on the paths
that continue after `options()` returns: `window()` (the native window,
when `--text` is absent) and the text path in `main`. Those are the only
`RealPort::new` call sites in `bin.rs` (the `window` closure and the text
path). `eframe::run_native` is in `window()` only.

`write_mockups` (`src/rig_check/gui/mod.rs`) calls `mockup_states`,
`raster::render`, and `save_new`. The files under `src/rig_check/gui/` do
not name `RealPort`, `FileSource`, `rusb`, or a USB open. `raster::render`
builds an `egui::Context` and tessellates at 1.0. `RealPort::new`
(`src/rig_check/port.rs`) is the constructor that, outside tests, installs
`RootFs { root: "/" }`. The mockup path does not call it. With the `usb`
feature off, the `UsbInterfaceLink` field on `RealPort` is not compiled
(`cfg(all(feature = "usb", not(target_os = "macos")))`).

## How to mark a row

Write **Approve** or **Change**, and a short note when the row needs a
change. Screenshot rows stay empty until the owner keeps a private
screenshot. Do not commit those screenshots.

## Checklist

| # | Criterion | Evidence on this head | Still needed | Mark |
| --- | --- | --- | --- | --- |
| 1 | Choose Practice or Real rig, with no mode stored until a choice | `01-choose-mode.png` from this redraw shows the Linux sentence in Provenance. Source focuses **Practice**, so Enter selects Practice. The mode field stays empty until that press. | Private screenshot of the focus ring. The Mac and Windows sentences have no picture: they are compile-time branches, and this binary is Linux. Decide whether Enter-on-Practice is an acceptable keyboard default. | |
| 2 | Start notice: Quit focused, Accept and continue as the other choice. Enter quits. | `02-start-notice.png` shows Quit and Accept and continue. Byte-identical to `c7ede9f`. Test `the_start_notice_defaults_to_quit_and_save_needs_the_exact_text` presses Enter and receives `Reply::Accept(false)`. A closed window also refuses the start. | Private screenshot of the focus ring on Quit. The committed PNG hides it (`focus_armed` is false). | |
| 3 | Every step shows its card and waits. Nothing starts the next step by itself. | Card buttons are Skip (focused), Run or Start, Stop the run, and Back when a prior result exists. Enter skips (`the_step_card_shows_its_sections_and_enter_skips`). R runs, Q stops (`keyboard_shortcuts_run_and_stop`). A closed window skips. Practice (`the_channel_operator_runs_practice_mode_from_another_thread`) runs 35 cards and 35 results, and the runner waits at each result. | Private screenshot of a step card with Skip focused. | |
| 4 | Step card sections: What it does, What you'll see and hear, Risk, with the gate and the steps it needs | `03-step-card-read.png` (step 1.1) and `04-step-card-live.png`. Both match `c7ede9f` and the current `mockup_states` output. The same test looks for those three headings plus SEE and HEAR. Risk colour follows the level: read and none use the pass green, live uses amber, save uses the danger colour. The word is also on the card. | Owner look at 03 and 04. Cards for 0.2, 1.2 and 6.2 are not among the 14 states, so those bodies are still unpictured. | |
| 5 | HEAR and SEE at 34 logical points, readable from the guitar | `pub const BIG: f32 = 34.0` in `src/rig_check/gui/mod.rs`. The theme scales logical points by `pixels_per_point`. The pictures are 1×, so 34 points is 34 pixels there. | Private laptop screenshot and private Retina screenshot, from the playing position. No wizard test sets `pixels_per_point` to 2. | |
| 6 | Running screen: current action, count, elapsed time, Stop, and the "leave the controls alone" notice | `05-running.png` shows step 3.3, "12 of 35 reads", Elapsed 00:02 and Stop. Byte-identical to `c7ede9f`. The sidebar word is RUN. Test `running_shows_latest_action_count_time_and_playing_notice` checks the count, elapsed time, the controls notice, and 15 s / 6 s / 0 s left. | Private screenshot while a count is moving, if the owner wants motion as well as the still picture. The still clock is whole seconds from the draw, as recorded above. | |
| 7 | Get-ready screen before a physical action. The next step still has its own card. | `06-listening.png` shows GET READY 8 s and Elapsed 00:02. Byte-identical to `c7ede9f`. Design notes in [design.md](../rig-check/design.md). `prepare_action` focuses **OK**. Enter starts the countdown. Skip and Stop are the other buttons. A closed window returns Stop. Detecting the movement can finish the current action. The following step still arrives as its own card. | Owner decision on OK-as-Enter. The design notes say focus starts on Skip. This screen focuses OK. A change belongs in `src/rig_check/gui/mod.rs`. This checkpoint does not make it. | |
| 8 | Didn't see it: Skip focused, Try again, Stop | `07-not-seen.png`. Byte-identical to `c7ede9f`. `not_seen` focuses Skip. A closed window returns Stop. | Private screenshot with Skip focused. | |
| 9 | Listening question: Yes, No, Not sure focused, optional note, and the byte-check lines | `08-listening-question.png` is the A/B drop-D question. Byte-identical to `c7ede9f`. `question` focuses Not sure. A closed window answers Not sure. | Private screenshot with Not sure focused. | |
| 10 | Save notice names the exact slot and the backup that protects it. Save stays off until the confirmation matches. Skip is the default. | `09-save-notice.png`: slot "Spank 1 (flash slot 14)", backup step 3.4, file `variax-library/14-Spank-1.bin`, first 12 hex characters of the SHA-256. The type-in line repeats `STORE VARIAX Spank 1`. Byte-identical to `c7ede9f`. Enter sends `Reply::Typed(None)` in the start/save test. Source enables Save only when the field equals the confirmation. | Private screenshot of Skip focused, of Save disabled, and of Save enabled after the exact text. The headless test does not press a wrong string and then the exact string. | |
| 11 | After a step, the result shows the outcome and the SEE values. Continue is focused and returns to the next card, which still waits for Run. Different… records the displayed text. | `11-step-result.png` and `14-display-difference.png`. Byte-identical to `c7ede9f`. Practice asserts the SEE lines, Continue and Stop, and that Enter continues. `different_opens_a_field_and_enter_records_exact_text` checks the field. A reported difference fails the step. | Private screenshot of Continue focused, and of the difference field. | |
| 12 | Progress uses the words PASS, FAIL, SKIP, STOP, N/A and RUN | Visible on `03`, `05`, `10` and `11`. `outcome_tag` in `src/rig_check/gui/mod.rs`. Colour repeats the word. | Owner look at one PASS, one SKIP and the RUN row together. | |
| 13 | Summary: rig state, counts, result preview, Save report… | `10-summary-preview.png`: "Rig Check finished", the rig paragraph, "10 passed · 0 failed · 1 skipped", the report path, Hide results, Save report…, and one preview line for step 2.3. Byte-identical to `c7ede9f`. The preview lists only that line because only the 2.3 fixture has a start time. The Practice test is the full-run evidence: 35 passed. Summary focus starts on the preview button. Save report… is the other button. In tests the save dialog returns "Saving is not available in tests." | Private screenshot of a finished Practice summary whose preview lists the run. The Practice test does not draw `Prompt::Done`. | |
| 14 | Back reviews an earlier result and the accepted start notice. Return resumes the pending card and sends nothing. | `12-review-result.png`, `13-review-start.png`. Byte-identical to `c7ede9f`. Tests `backspace_reviews_previous_results_and_c_resumes_the_same_card`, `first_card_back_shows_the_already_accepted_start_notice`, `review_shows_the_start_notice_accepted_for_this_mac`, and `clicking_a_finished_step_replays_it_without_continuing_the_active_result`. | Private screenshot of Back and of Return to current step. | |
| 15 | Laptop presentation | The wizard's headless path draws one logical size, 1180×800, at 1 pixel per point. The app shell has a separate headless matrix at 1120×760 and 720×640 (`src/app/shell/tests.rs`). That matrix does not open Rig Check. Sidebar titles already truncate at 1180×800. | Owner screenshots of the 14 states in the real window on the laptop. A 1120×760 or 720×640 wizard check belongs in `src/rig_check/gui/tests.rs`. | |
| 16 | Retina presentation | Shared theme sizes are logical points. Body-canvas and tuning tests in `src/shared/ui/tests.rs` run at `pixels_per_point` 1 and 2. The shell footer test does the same. `raster::render` tessellates at 1.0 and never sets `pixels_per_point`. | Owner screenshots on a Retina display. A wizard test at `pixels_per_point` 2 belongs in `src/rig_check/gui/tests.rs`. | |
| 17 | Screenshots of each state, kept for this review | The 14 repository PNGs are this redraw. Thirteen match `c7ede9f`. `01-choose-mode.png` is the current Linux sentence. | Private owner screenshots of each state, including the focus ring. Mac and Windows sentences, cards 0.2, 1.2 and 6.2, and the #297 continuation screens are outside this set. | |

## Remaining gaps

- Owner design approval is still pending. This redraw does not supply it.
- The focus ring is hidden on 13 of the 14 pictures. Only `11-step-result.png` sets `focus_armed`.
- The Mac and Windows choose-mode sentences have no picture.
- Step cards 0.2 ("USB devices and drivers on this computer"), 1.2 ("Open and give back the guitar interface") and 6.2 ("Rig back to its starting state and connections released") are not among the 14 states.
- Laptop logical sizes and a Retina `pixels_per_point` of 2 are not drawn.
- Choose-mode still focuses Practice. The get-ready screen still focuses OK, while the design notes say the safe answer starts with focus. Both stay undecided. This checkpoint does not change them.
- Native-window screenshots stay private owner copies.
- #297 continuation screens (Resume, Start new, Open last report) are outside this set. `write_mockups` still emits the same 14 names.
- The clocks on `05-running.png` and `06-listening.png` are whole seconds from the draw. This draw matched the previous bytes. They are not a frozen time fixture.
- [design.md](../rig-check/design.md) does not name revision `e2a0e1a`. The provenance table above is the record for this redraw.

The three open questions already in design.md (progress list, evidence line,
Open folder) stay unanswered here.

## Headless tests read for this comparison

In `src/rig_check/gui/tests.rs`:

- `the_step_card_shows_its_sections_and_enter_skips`
- `keyboard_shortcuts_run_and_stop`
- `the_start_notice_defaults_to_quit_and_save_needs_the_exact_text`
- `the_channel_operator_runs_practice_mode_from_another_thread`
- `running_shows_latest_action_count_time_and_playing_notice`
- `mockups_render_headless` (14 state names, then one 400×300 frame of the third state, colour count only)
- `different_opens_a_field_and_enter_records_exact_text`
- `stop_at_a_result_offers_the_guard_without_offering_another_step`
- `backspace_reviews_previous_results_and_c_resumes_the_same_card`
- `first_card_back_shows_the_already_accepted_start_notice`
- `review_shows_the_start_notice_accepted_for_this_mac`
- `clicking_a_finished_step_replays_it_without_continuing_the_active_result`
- `technical_details_are_hidden_until_opened`

Re-run on source `e2a0e1a` with Rust unchanged:
`cargo test --offline --all-features` passed 1,152 tests, 0 failed, 5 ignored
(library 755 passed and 2 ignored, `variax-reso5-test` 73 passed and 3 ignored,
`variax-workbench-gui` 297 passed, integration 27 passed).
`cargo fmt --all -- --check` and
`cargo clippy --offline --all-features --all-targets -- -D warnings` passed.
`crates/line6-devices` and `tools/variax-probe` were not edited, so their
separate suites were not run.

## Outside this checkpoint

Lane 2 owns `docs/design/`, `src/shared/ui/`, `assets/ui/`, and the visuals
of `src/app/shell/` after #213. The wizard implementation is
`src/rig_check/`. This checkpoint does not edit it. Still with that owner:

- Pictures for the Mac and Windows choose-mode sentences, and for cards 0.2, 1.2 and 6.2.
- The #297 continuation screens.
- The OK and Practice focus decisions above.
- Extending `mockups_render_headless`, or a neighbour test, to compare a fresh 1180×800 render with the committed PNG text, to require the exact save confirmation before Save enables, to draw the summary and look for "Save report…", and to run the wizard at laptop logical sizes and at `pixels_per_point` 2.
