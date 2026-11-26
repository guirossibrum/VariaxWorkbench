# Owner-run PODxt Live USB parity on macOS (#274)

The regular offline DMG still uses `gui,portable-math`. The private owner DMG
builds both Workbench and Rig Check with `gui,portable-math,usb` on the same
reviewed revision. The earlier five-card, identity-only trial was a separate
milestone.

## Build and verify the private DMG

After Gatekeeper review of both consumers, run this on a Mac with Xcode
command-line tools, Rust 1.99.1, both Darwin Rust targets and a populated Cargo
cache:

```sh
packaging/macos/build.sh --owner-usb /path/to/private-owner-output
```

Keep `Variax Workbench - Owner USB.dmg` and `verification.txt` in the
private output directory. The build verifies the exact static libusb archive
reported by Cargo for each target and requires its implementation symbols in
both apps on each mounted slice. It checks macOS 10.15 on Intel and 11.0 on
Apple silicon, Apple system-only dynamic dependencies, ad-hoc signatures
and bundle contents. The USB packaging mode does not launch either binary.
This local build mode is independent of the public offline artifact workflow.

## Owner verification

Record the revision, Mac model and OS, and the results before sharing a build.
Keep Rig Check reports and backup evidence locally. Follow the existing
consent prompts and gate sequence.

- Close other clients of the rig. In Workbench, confirm that the PODxt Live
  3.01 tab opens on its verified attachment. If the supported Variax answers
  through the relay, confirm its identity and tab status.
- Run Device Check in order. Confirm the Mac USB attachment, the guitar
  identity and the current-settings read. Check the PODxt library backup and
  its manifest before any operation that requires that backup.
- Walk the approved PODxt workflows in Workbench using the existing prompts
  and verified backup requirements. Record which workflows passed and any
  recovery or verification the app reported.
- Run Rig Check in real mode through the same applicable non-audio steps as
  on Linux. Confirm the report covers PODxt Live and the supported relay.
  An unexpected identity, failed prerequisite, or cleanup error follows the
  existing stop and recovery behavior.
- If the OS or a competing application holds the interface, retain the
  reported conflict. The app must leave the driver in place. If the device
  re-enumerates during the run, the stored port/address selection must refuse
  the reopen and a new check must select the attachment.

Record these as owner observations separately from the native build report
and simulated tests. [Attachment contract](podxt-macos-usb-integration.md)
