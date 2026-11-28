# Workbench session log

Workbench writes one local JSON Lines file per desktop session under the configured application data folder. Each line carries the schema version, Unix timestamp in milliseconds, application version, build, revision, severity, endpoint and event category. Related events share a process-local operation ID. Package builds use the pinned source revision. Local builds record the Git checkout's commit when available.

| Platform | Log folder |
| --- | --- |
| Linux with an absolute `XDG_DATA_HOME` | `$XDG_DATA_HOME/variax-workbench/logs/` |
| Linux otherwise | `$HOME/.local/share/variax-workbench/logs/` |
| macOS | `$HOME/Library/Application Support/VariaxWorkbench/logs/` |
| Windows | `%LOCALAPPDATA%\VariaxWorkbench\logs\` |

The file name is `workbench-session-<20-digit Unix milliseconds>-<unique suffix>.jsonl`. A supplied `--data-dir` takes precedence over the absolute `VARIAX_WORKBENCH_DATA_DIR` override and the platform defaults. The log folder is `logs/` under that root. If Workbench cannot resolve or write the folder, logging is unavailable and Workbench can continue.

Development and local owner builds record TRACE, DEBUG, INFO, WARN, ERROR and CRITICAL, including optimized owner builds. The public package profile records WARN, ERROR and CRITICAL. Local owner delivery selects TRACE on Linux and macOS USB and on Windows GNU offline. A combined delivery that reuses a hosted Windows distribution package inherits that package's WARN level.

The worker discards events when the queue is full and records the drop count when it can write again. Each file is capped at 4 MiB. Its final 64 KiB are reserved for WARN and above when ordinary records reach the limit. Workbench keeps eight recent sessions and protects files that active writers still hold open. More than eight concurrent processes can temporarily exceed the history count. An export is an explicit, non-overwriting copy to a user-selected destination. Records contain typed facts such as device family, action, disposition, duration and bounded control comparisons. The schema excludes free-form device errors, paths, preset names, serial numbers and raw preset or transport data. Workbench does not upload logs.

## Coverage

| Area | Recorded now | Coverage pending |
| --- | --- | --- |
| Buttons | HD500X Open HD Edit file, including cancelled open and load outcome | Other editor actions, dialogs, Home, connection controls and Rig Check |
| Switches | HD500X effect enable offline and PODxt Live stage switches | Other switches and toggles in the family editors, shell and tools |
| Dials and sliders | HD500X Effect slot slider and normal PODxt Live stage controls. PODxt batches correlate changed queue values, coalescing, dispatch and checked outcomes | Other editor dials and sliders, individual PODxt control identifiers, and staged or file-only PODxt edits |
| Device | Variax portion of Device check step 3 on Linux and macOS. Logs logical sends, packet polls, identity reply or timeout, orphan-tail outcomes, and separate read and checked interface release outcomes | UI-to-worker correlation for Device check, connection/open/verification stages, individual native USB transfers, reply subtypes and final metadata timeout classification, explicit retry reasons, other device operations and release sites, and Rig Check |
| PODxt recall | Live sidebar clicks, approval and cancellation, addressed comparison, device-buffer draft retention, Program Change, checked readback and release with one operation ID | Native owner verification and separate logging for draft replay, manual Send, Undo, store, Follow and library operations |

PODxt recall records zero-based Program Change indices and uses the saved-slot reply read on the checked link as evidence. A mismatch can include a name-change flag, bounded CC numbers and values with their classification, and opaque offsets with XOR masks. The editor records device-buffer draft provenance after checked release and before attempting continuation. Preset names and draft bytes stay in the editor. Live control values use the existing PODxt encoding and report the verified value after release. Repeated input for an identical pending value adds no queue event.

The Variax step 3 fixture exercises the existing identity retry and expected-reply logic with a scripted link. PODxt headless editor tests exercise the actual sidebar, confirmation controls, Drive knob and amp switch through the scripted device transaction and inspect the JSON Lines records.

Lane 1 owns the Help/Diagnostics placement. The shell can use `session_log::status()` for the effective level, current file, folder and sink state, and `session_log::export_current(destination)` to queue a copy and poll its result without blocking the UI. Rig Check's report files and exports use their existing locations.
