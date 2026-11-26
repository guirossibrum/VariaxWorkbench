# macOS USB integration (#274)

The normal macOS build that includes Cargo's `usb` feature uses the shared
PODxt Live USB transport. Workbench and Rig Check use the same transport and
the existing family sessions, operation gates, and workflow approvals.
`variax_workbench::macos_usb_available()` is the app admission predicate. It is
false for test builds, other platforms, and the ordinary offline macOS package.
The earlier five-card Rig Check mode is superseded.

## Attachment and cleanup contract

- Workbench identity detection retains the bus/port path and enumeration
  address in `DeviceKey.attachment.usb_location()`. Editors must use that pair.
  A run that has no verified tab key can select exactly one result of
  `line6_devices::transport::usb::macos::attachments()` while holding the rig
  lock. Keep the pair for the whole run and use the existing identity checks
  before running dependent operations.
- Call `UsbInterfaceLink::open_attachment_borrowed(spec, path, address, &rig)`
  with `usb::macos::PODXT_MIDI` for interface 0 or
  `usb::VARIAX_VIA_PODXT_LIVE` for interface 1. Both opens reject a missing,
  changed, or ambiguous attachment before the device is opened. macOS also
  checks the active configuration and expected endpoint descriptors.
- Use separate handles for the two interfaces. Call `finish()`, handle the
  cleanup result, and drop the link before reopening. Keep the same rig lock
  through cleanup and all reopens. Re-enumeration requires a fresh verified
  attachment. A driver on either interface or a busy claim is an error.
  macOS uses a non-seizing claim and never detaches a driver, resets the
  device, or changes the active configuration. The selected interface still
  requires its existing alternate setting.
- Transport writes carry the shared session's gated commands. Discovery
  uses identity-only policies. The application and Rig Check keep their
  existing approval, backup and verification rules.

The transport checkpoint is for integration with the Workbench PODxt and
Rig Check consumers. It stays off master until both consumers are reviewed.
