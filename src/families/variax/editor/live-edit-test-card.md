# Variax editor working-buffer test (#204)

The editor has separate closed compile-time switches for automatic live
editing and manual Send Edit Buffer. This card is for an owner-run build
after approval of the specific switch used for the test. Offline tests
check the shared #245 procedures against scripted owner-tool wire bytes.

Use the standard host math build and a verified recorded library backup.
Place the private model manifest in `variax-live-reference/models.tsv`
beside the editor preferences, and the approved Mac Workbench program
digests in `variax-live-reference/workbench.sha256`. Use one SHA-256
digest per line for each exact 3,276-byte candidate program, including
the muted intermediate image for each tuning transition tested.

1. Connect the supported Variax 600 through PODxt Live, let identity finish,
   and select a downloaded library position. Record the starting position
   and sound. The editor opens with Live editing off.
2. Enable **Live editing**. Confirm the working-buffer upload completes and
   the selected model plays. Move one pickup control several times and then
   stop. Confirm the edited sound and the **Modified · not stored on
   guitar** marker. Change the alternate-tuning Enable flag and verify the
   resulting tuning. Expect an acknowledged difference to the muted transition image followed by the final full upload.
3. Select another library position. Confirm a full working-buffer upload.
   Return to the first position and confirm the editor still holds the
   local edits. **Get Edit Buffer** opens the current patch while keeping
   the library accessible. Test **Send Edit Buffer** only in the build
   approved for its separate switch.
4. Turn Live editing off during a local edit. Confirm it stops sending
   updates. After a disconnect and verified reconnect, confirm Live
   editing is off and local edits remain in the editor.
5. Return to the starting position using the guitar selector. Confirm the
   stored preset still loads as it did before the test. Stop on a
   verification failure or a busy rig. The editor stops its live session
   on error and retains the local edits.

This path addresses the guitar working buffer. Stored-bank operations are
covered separately by #205.

## Prepare the private references offline

Use the same Mac coefficient oracle and original Java mapping as
`scripts/verify_factory_programs.py`. The artifact root holds the pinned
Workbench inputs, JDK and Unicorn environment described in
[Reproducing references](../../../../docs/reproducing-references.md).
Use a fresh output directory named `variax-live-reference` beside the
editor preferences. Keep the files local.

Before turning on Live editing, export the exact planned editor states
as `.vwm` copies. Include the initial model, each settled parameter edit,
the final state after each tuning toggle, every model the test will load,
and any manual Send candidate. Use discrete edits with planned final
values so the 120 ms debounce does not produce unlisted states. Mark the
destination of each tuning toggle as also requiring a muted render.
The transition uses the **new** tuning state and the mute flag.

From the repository root, build the offline CLI with
`cargo build --offline --no-default-features --bin variax-workbench`.
Run this preparation using the local Python described above. Replace the
sample file paths and repeat arguments as needed. Set `artifact_root` to
the absolute private artifact root and `reference_dir` to the new
absolute `variax-live-reference` directory. A `transition` argument
generates both the ordinary and the muted program for that file.

```sh
PYTHONPATH="$PWD/scripts" "$artifact_root/.tools/python/bin/python" - \
  "$artifact_root" "$reference_dir" "$PWD/target/debug/variax-workbench" \
  normal:/path/to/initial.vwm normal:/path/to/settled-edit.vwm \
  transition:/path/to/tuning-after-toggle.vwm <<'PY'
from hashlib import sha256
from pathlib import Path
import subprocess
import sys

from verify_factory_programs import CoefficientOracle, load_models, original_programs
from inspect_firmware import preserve

artifact_root, out, host = (Path(arg).resolve() for arg in sys.argv[1:4])
cases = [arg.split(":", 1) for arg in sys.argv[4:]]
assert cases and all(mode in ("normal", "transition") for mode, _ in cases)
assert subprocess.check_output([host, "math-profile"], text=True).strip() == "host-std-v1"
out.mkdir()
oracle = CoefficientOracle(artifact_root=artifact_root)
initial, manifest, _ = load_models(oracle, out)

def generate(settings, _mask):
    oracle.write(oracle.OBJECT, initial)
    return oracle.generate(settings)

hashes = []
for muted in (False, True):
    phase = out / ("muted" if muted else "normal")
    phase.mkdir()
    presets = phase / "presets"
    presets.mkdir()
    inputs = [Path(path) for mode, path in cases if not muted or mode == "transition"]
    if not inputs:
        continue
    for index, path in enumerate(inputs):
        preserve(presets / f"{index}.vwm", path.read_bytes())
    programs, _ = original_programs(
        artifact_root, phase, presets, generate, muted=muted, count=len(inputs)
    )
    for index, (path, program) in enumerate(zip(inputs, programs, strict=True)):
        rendered = phase / f"{index}.packed"
        flags = ["muted"] if muted else []
        subprocess.run(
            [host, "render-preset", presets / f"{index}.vwm", manifest, rendered, *flags],
            check=True,
        )
        if rendered.read_bytes() != program:
            raise SystemExit(f"Workbench mismatch: {path} (muted={muted})")
        digest = sha256(program).hexdigest()
        hashes.append(digest)
        print(f"{digest}  {path} (muted={muted})")
preserve(out / "workbench.sha256", ("\n".join(dict.fromkeys(hashes)) + "\n").encode())
PY
```

The manifest and special model definitions are prepared by `load_models`.
The hashes come from the original Workbench images after exact comparison
with Rust. An ordinary `0x3B` test needs the hash of the new fully rendered
candidate. The read-back separately verifies that the guitar kept the
previous checksum word, which the difference message excludes.
