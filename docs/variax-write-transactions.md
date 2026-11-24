# Variax 300/600 write transactions

Offline design and evidence for [#175](https://github.com/guirossibrum/VariaxWorkbench/issues/175)
and [#25](https://github.com/guirossibrum/VariaxWorkbench/issues/25). The
reference is Workbench 1.75 build 121 on the PODxt Live path. The Rust
implementation is in [the Variax family](../crates/line6-devices/src/family/variax/write.rs).
It accepts a caller-provided link and gate policy. The separate
[Reso-5 owner test](reso5-test-card.md) uses only the full edit-buffer
transaction, with its own identity checks, backup verification and G6 policy.

The separate [Reso-5 slot 50 owner test](reso5-store-test-card.md) uses
G7 and its own write policy. It sends only the archived factory program pinned
to slot 50. It verifies the same-day backup and compares the current settings
before and after the store. Its file-only check requires no host model manifest.

## Byte and timing contract

All guitar SysEx commands use the header `F0 00 01 0C 07 00` and end with
`F7`. Workbench's manager sends the six-byte header, payload, and terminator
as separate calls for wrapped commands. The bridge enable/release and
`0x43` full buffer use a single native send call. For each native send call,
the original macOS i386 adapter sends consecutive slices of up to 256 bytes
to its hardware output abstraction. Continuation chunks carry no added bytes.
The lower-level Linux USB packet size and pacing are separate.

| Operation | Outbound bytes and sequence | Response evidence |
| --- | --- | --- |
| Open bridge and MIDI session | PODxt Live `F0 00 01 0C 03 23 01 F7`, then guitar `2A 01`, then wait 100 ms | Original waits for no reply. Device-side acknowledgement or ready indication is unknown. |
| Full edit buffer (G6) | `43`, one 24-bit count (1,092), then all 1,092 words from the 3,276-byte big-endian packed program. Six low-first nibbles per word. The 6,566-byte message is 25 chunks of 256 and one of 166. | Original waits for no reply. Apply completion, rejected programs or interrupted-transfer effects remain unknown. |
| Live difference (G6) | One `3B` containing contiguous runs of changed words 0–1,090. Normal runs start with count then address. The original encodes a run reaching word 1,090 as address then count. Omit the checksum word (1,091) and omit the packet when nothing changed. With an unknown baseline, the original treats all 1,091 eligible words as changed. A tuning toggle sends a diff of the muted transition program followed by a full unmuted program. | Original waits for no reply. Receiver behavior for the terminal-run ordering and for partial differences is unknown. |
| Tone position (G6) | Channel-1 `B0 4F vv`, with `vv` calculated by truncating `127 × position` in single precision. | Owner-run reads observed spontaneous tone-knob CC 79. Echo or application of an incoming CC 79 remains unknown. |
| DSP bypass | `4C 01` at start of the stored patch block, `4C 00` when it ends. The normal real-time edit path leaves DSP processing active and sends DSP on when closing. | Original waits for no reply. Whether audio has resumed after an interrupted transfer is unknown. |
| Store a slot (G7) | Send `43`, wait 100 ms, send `61` and flash index `bank × 5 + (4 − slot)`. Banks and slots are zero-based. | Original checks the first SysEx within its 15 s wait for an eight-byte `0x71` reply. Rust validates the envelope and skips the three known notices within the same deadline. Other messages leave the slot state unknown. The reply provides no slot identifier or DSP data. Timeout, wrong reply, or transport error after the store may leave the slot changed. |
| Rename bank (G7) with readback (G5) | Program change to the bank's switch position zero using the existing preset mapping, wait 200 ms, send `6C` and 18 zero-padded ASCII bytes, wait 700 ms. Send `6D` to read the selected bank. | Original uses a 1 s reply wait for `6D` and reads the 18 name bytes from offset 7 of a reply at least 26 bytes long. It waits for no `6C` acknowledgement. Rust compares the name and bounds the read to one second total. The bank-select Program Change's echo and a name's persistence after power loss remain unknown. |
| Close | DSP on, guitar `2A 00`, then PODxt Live `F0 00 01 0C 03 23 00 F7`. | Original close sends DSP on and bridge release. Rust additionally closes its bounded guitar MIDI session using the existing read-path command. The standalone owner release check observed no display change or acknowledgement on its tested route. |

The original also recognizes asynchronous clip (`0x28`), DSP overload
(`0x4D`) and MIDI queue overflow (`0x74`) notifications. Rust logs and skips these only in a Variax SysEx envelope.
Their occurrence and ordering on the rig during stores remain unknown.

Workbench can truncate names during compilation. Rust rejects a source name
longer than 18 bytes or containing characters outside printable ASCII. It
also checks the compiled name field at offset 2,553, space padding for model
names, and the checksum at word 1,091 before opening a write transaction.
The stored-name and tone domain limits apply to the transaction input. The
library's bank-name read alone emits no Program Change.

## Transaction and failure boundaries

- A live adapter must identify the approved target and its version before
  sending write traffic. Initial validation is scoped to Variax 600 firmware
  3.10 through PODxt Live 3.01. The format, firmware and route of other
  families or versions require independent evidence. The library's path
  and gate check precede its first send.
- The application call site must put discovery and the complete transaction
  behind `run_if_unlocked` and the appropriate gate. A G6 transaction owns
  the bridge, session, edit commands and cleanup. G7 owns those steps when
  they are part of a store or bank rename. The rename also requires the
  G5 bank-name read for verification. Both transactions use a caller-opened
  link. Use one exclusive conversation with a known reply
  stream. The store reply cannot distinguish a delayed acknowledgement
  from the current transaction.
- Before G7, verify the G5 backup and confirm the target and approved
  restoration procedure. A Variax backup from the guitar contains metadata
  words 851–914. A restored DSP program would be regenerated from the
  settings. The original Workbench parity decision and measured-bound work
  continue in [#179](https://github.com/guirossibrum/VariaxWorkbench/issues/179).
- Check cancellation before each edit chunk and before sending `0x61`.
  After sending `0x61`, complete the reply wait and never resend on an
  ambiguous result. After a partial SysEx, attempt `F7` before cleanup.
  Always attempt DSP on, session off and bridge release in order, even if
  a previous step failed. Report cleanup errors separately from the
  original error. Losing the transport can prevent successful cleanup.
- A bank rename selects a stored preset and changes the active model. A
  successful readback verifies the name in the current bank conversation.
  The bounded transaction does not reselect the prior preset or reconcile
  unsaved edits. The caller of a future live rename must account for the
  selection and edit state.
- Metadata readback can confirm name and settings, and a listening check
  can test the result on the rig. Neither verifies the full DSP program
  byte for byte. A store acknowledgement alone cannot verify the result
  or prove recovery.

The [receiver replay](receiver-simulation.md#workbench-application-write-transcript-replay)
checks the #175 host command shapes and rejection cases for #25. Session on/off
runs through virtual control. Application write handlers remain outside the
updater receiver, including store acknowledgement and flash recovery.

## Offline reproduction

From this worktree, use the local artifact root from
[the factory comparison](factory-program-parity.md#reproduction). The
harness uses the same 60 models and renders both normal and muted
programs with standard Rust and the original Java/native coefficient path.

```sh
CARGO_TARGET_DIR=target/reference cargo build --offline --locked \
  --no-default-features --bin variax-workbench --bin variax-firmware-inspect
"$artifact_root/.tools/python/bin/python" scripts/verify_variax_write.py \
  --artifact-root "$artifact_root" \
  --archive "$artifact_root/vendor/installers/Variax300_3.10.vxf" \
  --extractor target/reference/debug/variax-firmware-inspect \
  --host target/reference/debug/variax-workbench
cargo test --offline --all-features --manifest-path crates/line6-devices/Cargo.toml
```

The Java recorder runs original `L6VariaxDevice`,
`L6VariaxMIDITranscoder`, `L6VariaxPatchManager` and `L6MIDIManager`
methods, with hardware constructors bypassed and synthetic acknowledgements.
The macOS i386 native adapter's size setter and send path run under Unicorn
to record actual native chunk boundaries at the hardware-output call.
Explicit wait events are recorded on a virtual clock. No USB, MIDI or
audio interface opens. Linux USB endpoint framing and device-side timing,
application, storage and recovery remain unverified. Real device
confirmation belongs to the separate owner-run steps beginning with
[#176](https://github.com/guirossibrum/VariaxWorkbench/issues/176).
