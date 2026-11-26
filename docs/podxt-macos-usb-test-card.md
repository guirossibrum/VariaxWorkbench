# First PODxt Live direct USB test on macOS

**Status: awaiting the owner's approval.** The Mac USB switch is closed.

## Verify native linkage

The normal package still uses `gui,portable-math` and opens offline. On a Mac
runner with Xcode command-line tools, Rust 1.99.1, both Darwin Rust targets and a
populated Cargo cache, run the verifier from the checkout with the switch closed:

```sh
python3 scripts/verify_podxt_macos_linkage.py /path/to/new-linkage-evidence
```

The verifier builds the USB profile (`gui,portable-math,usb`) for both Darwin
targets without launching either app. It records the exact static archive
reported by Cargo and checks the archive architecture and defined libusb
symbols. It uses the production package checks for the four executable slices:
macOS 11.0 on arm64 and 10.15 on x86_64, with dynamic dependencies limited to
Apple system paths. It also rejects a dynamic libusb dependency and Catalina's
unsupported `UniformTypeIdentifiers.framework`. Keep the native verifier
logs. [Package verification](../packaging/macos/package.py)

## Owner-run sequence on M1

After the owner approves the test, use a dedicated Mac owner worktree. Set
only `DIRECT_USB_APPROVED` in
`crates/line6-devices/src/transport/usb/macos.rs` to `true`. Keep audio closed
and leave the Linux owner-test worktree as it is. Build and verify the owner
binaries using a **new** evidence directory:

```sh
python3 scripts/verify_podxt_macos_linkage.py --expect-owner-link /path/to/new-owner-linkage-evidence
```

The additional check requires the USB functions to be defined in each
Rig Check executable. Close Line 6 Edit, Workbench, DAWs and other clients of
the rig before the owner run. Open the arm64 Rig Check in real mode. It
offers exactly these five cards in order:

1. **0.2** checks the USB attachment list, and **1.1** requires exactly one
   PODxt Live attachment.
2. **2.1** asks the PODxt Live for its identity over interface 0. Expect
   **PODxt Live, firmware 3.01**.
3. **1.2** claims and releases interface 1 without sending MIDI. On macOS
   there is no Linux driver binding to restore.
4. **3.1** asks the Variax for its identity over interface 1. Expect the
   guitar's actual model family and firmware. Rig Check releases USB and ends.
5. Keep the report, exact build revision and native linkage logs locally.

A skipped or failed card or an unverified PODxt identity ends the run. An
identity exchange can retry once under its existing timeout rules. Both links
permit only the universal MIDI identity request. The owner build has no
recovery prompt or MIDI cleanup, and the editor stays offline. If macOS
reports that a driver or another application holds the device, record the
error and stop. The transport refuses the claim without unloading the driver.

The M1 test covers claiming, identity and release on that Mac. Behavior with
an Intel Line 6 kext still needs separate verification.
[Protocol research](podxt-usb-direct.md)
