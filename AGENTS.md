# Project instructions

## Hardware boundary — user requirement

PODxt Live and Variax 600 are irreplaceable. **No live device traffic is authorized
during the current phase**, including identity queries and read requests, because
they transmit USB/MIDI data and may change session state. Do not run `amidi`
send/receive probes, direct USB scripts, updaters, reset commands, driver unbinds,
or any other operation that contacts either device. USB may be disconnected.

`src/usb.rs::LIVE_ACCESS_LOCKED` and the Python probe guard must remain locked.
Do not add an environment, command-line, GUI, or test override. Use captured data,
fake transports and CPU emulation. Future live operations require explicit user
authorization after reviewing the offline validation ledger; general permission
to work on this project is not authorization to unlock devices.

No preset upload, flash write, firmware install, reset, or bank switching has been
authorized. Firmware downloads, version comparison, static extraction and offline
reverse engineering are authorized. Latest firmware availability must not cause
an automatic update. GitHub issues and the linked project contain the work plan.

## Implementation

The product is written in Rust. Python and Java are offline analysis/reference
helpers. Keep original vendor artifacts, captures, and user settings local and
ignored. Do not publish them or contact third parties without explicit user
instructions.

Preserve originals. New copies must use atomic non-overwriting saves. Validate
against original Java/native behavior where practical; label inference, emulated
behavior, real captures, and unverified behavior distinctly. Report partial
captures as partial; current-preset metadata is not a full backup.

Run `cargo test --all-features`, `cargo clippy --all-features --all-targets -- -D
warnings`, and appropriate offline compatibility/reference tests. Tests must not
open USB/MIDI devices or require the guitar to be connected.

## README status ledger

Keep README.md's checked offline passes, pending implementation, and currently
unavailable/not-working checklist current whenever work changes those states.
Attach precise test scope and counts where supported. Distinguish disabled
hardware features, unimplemented features, observed failures, original-library
oracle results and completed Rust behavior. Do not mark simulation success as
physical safety, audio equivalence, recovery capability or permission to unlock
hardware. Keep issue links and status consistent with actual completion.

## Work tracking

GitHub issues and the VariaxWorkbench project are the work-tracking source of
truth. Do not recreate or use TODO.md. README.md contains the requested concise
validation/implementation/failure summary; it is not a second task queue. Keep
only one implementation issue In Progress and respect native dependencies.

## Product scope

The final objective is one Rust app for guitar editing, POD editing, USB audio
recording/DAW integration, libraries and device-specific maintenance/update
workflows. Initial rig: Variax 600 + PODxt Live on Linux. Future targets include
Workbench HD/JTV-69, POD HD500X and additional independently verified models.
See docs/product-vision.md. Do not assume cross-family format/firmware or audio
compatibility. Roadmap scope does not authorize hardware access: opening audio
capture/playback streams also requires separate explicit user authorization.
