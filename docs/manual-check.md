# Short guided guitar check

The future commissioning routine targets about five minutes. It checks the guitar's
physical model selector and pickup switch, volume/tone sweeps, an 18-note spot
sample, bends/vibrato, and optionally an existing alternate tuning. A full neck
sweep is optional. No preset must be created or uploaded for this routine.

The initial Rust terminal app is an **offline workflow rehearsal**. It displays
instructions and records whether you understood them, had a problem or skipped.
It does not currently receive guitar controls, record audio, recognize pitch or
verify switches/knobs. Keep the equipment disconnected during rehearsal. The
instructions to move controls and play notes describe the future authorized session.

Preview the plan:

```sh
cargo run --bin variax-check -- plan
```

Try the prompts and save a report locally:

```sh
mkdir -p backups
cargo run --bin variax-check -- rehearse backups/check-rehearsal.txt
```

Enter `ok`, `problem`, `skip` or `quit`. EOF also ends the session. Every step after
cancellation remains NOT RUN. Reports include version/time and explicitly mark
hardware recognition and audio behavior UNVERIFIED. Reports are created atomically
and never overwrite existing files. Choose a new filename for another rehearsal.
`backups/` is ignored by Git. No live mode or runtime unlock exists.

The future integration is tracked by [issue #34](https://github.com/guirossibrum/VariaxWorkbench/issues/34),
after the authorized integration and audio prerequisites. It must distinguish:

- Timestamped decoded device observations, compared with declared physical actions.
- Audio measurements, compared with expected string/fret/tuning and bend trajectories.
- Your own observations of sound and control behavior.

A confirmation click cannot substitute for either of the first two. It is not yet
established whether every physical control has usable telemetry; unsupported or
unreported controls must be labeled rather than fabricated. A physical selector
position alone does not establish body identity because presets can have changed.
Mixed USB audio also does not establish that per-string data is available.

The brief note/bend/tuning routine is a commissioning spot check. It does not
replace the larger offline test matrix in #33 or prove that every fret/model,
firmware recovery, or any future hardware operation is safe. Returning knobs to
their starting positions is not a verified restoration of device state.
