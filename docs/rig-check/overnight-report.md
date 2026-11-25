# Rig Check overnight report (2026-11-25)

Branch `worktree-agent-a1cc4b13486c8e4ee`, worktree
`.claude/worktrees/agent-a1cc4b13486c8e4ee`. Local commits only: nothing was
pushed, no GitHub edits, no device opened, no device binary run, every
`APPROVED_STEP_x_y` and `AUDIO_STREAMS_APPROVED` is `false`.

## Commits

| Hash | Issue | What |
| --- | --- | --- |
| f6869d7 | #244 | Catalogue 1.0 (`docs/rig-check/catalogue.md`): steps 0.1-8.5, needs, gates, risk, cards, start and save notices |
| b4d76f6 | #247 | `rig-check` binary, runner (Run/Skip/Stop, default no), dependency skips, approvals, rig guard 6.2, byte-level simulated PODxt Live + Variax 600 |
| 01c0a9a | #249 | Report: atomic rewrite before/after each step, JSON block, redaction, hints, evidence folder |
| c8a1dda | #250 | Linux collectors (injected file tree) |
| 24ae430 | #251 | Windows/macOS collectors (parsers, "not verified on this OS") |
| f3b9bcc | #252 | Engine check 0.3 |
| eb0c532 | #253 | Steps 1.1/1.2 |
| d68ac5c | #254 | PODxt reads 2.1-2.5 |
| d1ea7f4 | #255 | Variax reads 3.1-3.6 |
| 8e8516b | #256 | PODxt live 4.1 |
| be4f6ea | #257 | Variax live 4.2/4.3 |
| b69c9ee | #258 | Variax save 5.1 |
| f9d21ca | #259 | PODxt save 5.2 |
| 02c0007 | #260 | Guided actions 7.1-7.4 |
| e04853d | #261 | VDI replug 6.1, crash restore |
| 741c5dd | #248 | Wizard window, 10 headless mockups |
| 480535d | #264 | Audio 8.1 (no stream) |
| 3d911cf | #265 | Audio 8.2-8.5 design, code path off |
| 41089d7 | #266 | Offline WAV comparison |
| 5734886 | #263 | Linux packaging script, release notes |
| 8ed54e3 | #262 | Windows/macOS transport research |

## What works end to end in practice mode

`rig-check --practice --text` (or the window) runs all 28 available steps
against the simulated rig, every one behind its own card. Tests cover the
full run, Stop before and during every step (rig back at start, 6.2 passes),
skipped dependents, real mode refusing every device step, faults (lost
identity packet, damaged `0x3C` reply, silent stores, busy/absent/denied
interface, driver not returned, missed guided action, unverified firmware),
the window's keyboard paths, and a crash restore. Excerpt of a generated
practice report (the scripted input typed `r` at the save prompt, so the save
was refused):

```text
Passed 26 · Failed 0 · Skipped by you 2 · ... · Not available 5
[0.3] Engine check (no device) — PASSED
    Synthetic preset (full pipeline): all 1,092 words identical
    Synthetic engine cases 01-48: 48 of 48 identical
[4.3] Variax: listen to a test model, then return — PASSED
    Practice test model (drop D): sent SHA-256 dcf07359351ac0ac, read back dcf07359351ac0ac
    Back on Spank 1: all 1,092 words match the start. Nothing was saved.
[5.1] Variax: save a preset's own bytes back to its slot — SKIPPED (by you)
    Pre-check: Spank 1 matches the 3.4 backup (SHA-256 e134ef27e442dd9f).
    The typed confirmation did not match. Nothing was saved.
```

## Stubbed or locked, and why

- Every real device step: owner approval per step (#267).
- Audio 8.2-8.5: owner audio approval (AGENTS.md). 5.3: excluded (#243).
- Windows/macOS device links: need the #262 adapter; collectors unverified
  on those systems. Windows `cargo check --target x86_64-pc-windows-gnu
  --no-default-features` passes; macOS not compiled (no target installed).
- `RealPort` opens links with the existing Linux APIs; it is compiled but
  unreachable while approvals are off.

## Decisions for the owner to confirm

1. Variax backup moved to 3.4 (before one preset 3.5 and one bank 3.6), so
   single reads can switch back to a known start preset.
2. PODxt reads one channel = 1A, one bank = 1A-1D; Variax one preset = next
   slot in the start bank, one bank = the start bank.
3. Save steps write only the **starting** preset/channel back; Variax flash
   40-59 is refused (spare sector 62); 5.2 needs `--evidence` in real mode.
4. Real 4.3 offers only test models built here that matched their reference
   in 0.3 (`--engine-folder`, optional `NAME.listen.txt`); practice uses a
   synthetic drop-D tweak.
5. Engine references: repository synthetic fixtures only; owner models via
   a private folder (expected program or SHA-256).
6. Computer steps (0.1, 0.2, 0.3, 8.1) need no step approval; 0.2 still
   goes through G1.
7. Small internal adapter (`procedures.rs`) instead of waiting for #245/#246;
   it composes public `line6-devices` APIs (`PodxtRequest::Stored` for one
   channel, `PodxtRequest::Program` for crash restore).
8. Mode is never defaulted: the window asks; `--text` needs `--practice` or
   `--real`. Practice crash-state files are not written.
9. **Wizard design approval is pending** (`docs/rig-check/design.md`,
   mockups in `docs/rig-check/mockups/`).

## Test counts (before → after)

- Main crate (`cargo test --all-features`): 950 → 1014 passed, 3 ignored.
- `crates/line6-devices`: 191 → 191 (not modified).
- `tools/variax-probe`: 9 → 9.
- fmt, clippy (all features, all targets, `-D warnings`), no-default build
  and the `variax-reso5-test` build pass at the last commit.

## Failures and leftovers

- Commit b4d76f6 (#247) alone fails three Clippy lints; they were fixed in
  the #249 commit (01c0a9a). Every later commit passed the full list.
- The terminal front end has no Ctrl+C stop (the window has Stop); a killed
  run leaves a valid report and, in real mode, a crash-restore state file.
- Not done: #245, #246 (other lanes), #267 (owner).
