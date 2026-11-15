# Emulating the PODxt Live and Variax 600 before hardware tests

## Feasibility and present limits

A stateful Rust simulation of the computer → PODxt Live → Variax 600 control path
is feasible and is the intended pre-hardware test environment. A faithful emulator
of both complete devices is **not currently available or established as feasible**.
The downloaded firmware helps, but firmware files alone do not describe every
CPU/DSP, peripheral, memory map, bootloader, calibration or physical behavior.

The existing CPU oracle runs the original **computer-side Workbench library**.
It does not run the POD firmware or the guitar's DSP firmware. Current fake
transport tests cover selected read-session outcomes; they are not a complete
POD or guitar simulator. No existing result establishes complete audio equivalence,
bootloader recovery or physical safety.

## Layers to build and verify

| Layer | Current evidence | Required simulation/emulation |
| --- | --- | --- |
| Editor and coefficient generation | Original Java/native code, model resources, golden outputs and synthetic fixtures | Complete Rust/native differential checks, including final metadata/assembly |
| POD USB/control routing | Saved identities/packets, Linux source, extracted Windows/macOS drivers | A virtual POD with explicit interface/endpoint routing, framing, buffering and failure states |
| Variax control/preset behavior | One saved current-preset metadata capture and original host protocol code | A separate virtual guitar with session state, modeled preset/DSP memory and independently verified command handling |
| Firmware update/recovery | Both L6FF images unpack with matching embedded MD5; updater code is available | Establish actual state transitions first, then model rejection, interrupted transfers, corruption, virtual power loss and recovery |
| Actual firmware execution | Image bytes and region tables; architectures and memory/peripheral maps remain unestablished | Identify instruction sets, loaders, vectors and peripheral contracts; assess whether original firmware can boot and execute in an isolated emulator |
| DSP/audio | Host coefficient generator executes offline; device DSP execution is not emulated | Identify the DSP architecture and processing path, then assess instruction/audio emulation and reference comparisons |
| Physical effects | No complete model of electrical/analog/aged components | Remain explicit limits: power, component condition, flash behavior, physical timing and analog sound cannot be certified by the present software model |

The POD image expands to 1 MiB in 19 regions; the Variax image to 442,402 bytes
in four regions. Those region counts and integrity checks are established. Numeric
region-type meanings, executable ISAs and boot/recovery behavior are not.

## Proposed offline test environment

Use separate in-memory components for the virtual POD and virtual guitar behind
the existing `ControlTransport` abstraction. The simulator must not use `rusb`,
ALSA, USB passthrough or a real device handle. Keep its capabilities separate from
the source-locked physical backend. Do not add an unlock or live fallback.

Start with statically evidenced identity, session and metadata behavior. Then
extend only as protocol evidence supports it. Unknown commands, unsupported
peripheral accesses and unmodeled states must fail explicitly, rather than
inventing a successful response that makes tests pass.

The simulator should cover:

- Correct target identification and POD-to-guitar routing; wrong model/version.
- Fragmented/coalesced MIDI, realtime bytes, noise, malformed messages and limits.
- Session activation/deactivation, ordering, cancellation and cleanup failures.
- Delayed/dropped/duplicated responses, disconnects and bounded retries.
- Virtual preset/DSP storage, valid/invalid lengths, checksums and typed fields.
- Future virtual write transactions and rejection without any physical writer.
- Update/recovery faults, including incomplete images and virtual power loss,
  once original state-machine evidence is available.
- Audio/coefficient comparisons only to the extent a DSP model is independently
  established; no invented claim that successful packet handling proves sound.

Use original Java/native routines, captured golden packets and independent
reference decoders to validate the simulator. Reusing the same new Rust codec
on both sides is not sufficient evidence: the same bug could make sender and
receiver agree. Mark each modeled behavior as observed, recovered from original
code, inferred, or not implemented.

## Test gate before any real connection

Require the relevant automated suites to pass offline, with the equipment
physically disconnected from the computer. Record exact coverage and remaining
unknowns. Passing all implemented tests means **all implemented tests passed**;
it does not mean every possible device state or failure has been covered.

Even a firmware emulator cannot prove zero risk on the real equipment. Keep the
current hardware locks until the user reviews the offline evidence and explicitly
authorizes a narrowly defined live operation. Full backups and a documented
recovery path remain separate prerequisites before considering persistent changes.
No simulator result automatically authorizes connecting hardware, reading it,
switching a preset, resetting a device or flashing firmware.

## Tracking and sequence

This plan refines issue [#18](https://github.com/guirossibrum/VariaxWorkbench/issues/18).
Its existing prerequisites are static update/recovery work (#16) and hardened
firmware inspection (#17), with architecture mapping (#13) upstream. Full
coefficient parity (#6) supplies the host-side reference for later DSP/audio work.
The readiness review (#22), backups (#23) and any live read-only integration (#24)
remain gated separately.

Follow the project execution order and keep only one implementation issue In
Progress. Issue #1 is now complete; documenting this plan does not start #18 or
change the sequential order.
No additional guitar capture is required to start the offline protocol model or
continue coefficient work; some device-specific unknowns may remain unresolved
without further independent evidence.
