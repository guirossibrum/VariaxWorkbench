# Observed native word map

The [machine-readable ranges](dsp-word-map.json) index all 1,091 native data words
using CPU memory-write hooks during the initialized 110-case host-generator sweep.
Constructor zeroing and the separate checksum word are excluded. All 110 packed
outputs remained identical to the versioned v2 reference candidates.

748 words had observed instruction writes; 343 did not. The latter stayed zero
in all 110 candidate outputs. That is **not proof that they are unused/reserved**:
other model/settings paths or adapter memory operations could write them. Their
semantics remain unknown. No unknown word is assigned a new meaning or reused.

Ranges list all observed writer procedures; overlapping ownership is retained.
For example, optional effect/microphone blocks can write ranges also used by
pickup fields. Whole-engine call order and enable conditions must be reproduced
rather than relocating the data based on its names. The observed map is evidence
for implementation, not exhaustive firmware/DSP coverage.

Reproduce locally with `scripts/verify_dsp_word_map.py`. Detailed per-word write
counts and original input/output candidates remain ignored under `analysis/`.
