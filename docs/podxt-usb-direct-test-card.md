# First PODxt Live direct USB identity on Linux

**Status: awaiting the owner's approval.** The direct interface-0 switch in
`crates/line6-devices/src/transport/usb/podxt.rs` is closed in the reviewed
build. This procedure is for the owner after a separate decision to run it.
It covers the PODxt identity and return of `snd-usb-pod`. The [protocol
note](podxt-usb-direct.md) describes what the offline work established.

## Prepare

- Close Variax Workbench, Rig Check, DAWs, and other clients of the PODxt MIDI
  or audio device. Keep the rig powered and leave the selected channel alone.
- In the Linux device inventory find the one `0e41:4650` PODxt Live. Record
  its Linux bus/port path and USB address. For example, path `8-2.1` and
  address `11` are distinct values.
- Check that the target has `snd-usb-pod` (hyphens or underscores) bound to
  interface 0. Record interface 1's binding for comparison.

  ```sh
  pod_port=8-2.1  # Replace with the verified port of your rig.
  cat "/sys/bus/usb/devices/$pod_port/devnum"
  readlink "/sys/bus/usb/devices/$pod_port:1.0/driver"
  readlink "/sys/bus/usb/devices/$pod_port:1.1/driver"
  ```

## Run once

In an owner test worktree, change only `PODXT_DIRECT_USB_APPROVED` to
`true` to build this test. Create a scratch Cargo binary whose
`Cargo.toml` depends on that worktree's device crate. Use the same user
and runtime directory as Workbench so the rig lock is shared.

```toml
[package]
name = "podxt-direct-identity-owner"
version = "0.1.0"
edition = "2024"

[dependencies]
line6-devices = { path = "/absolute/path/to/owner/worktree/crates/line6-devices", features = ["usb"] }
```

Put this identity-only client in the scratch project's `src/main.rs`.

```rust
use line6_devices::{
    device_lock::DeviceLock,
    family::podxt,
    gate::{GatePolicy, Operation},
    session::{Quiet, Session},
    transport::usb::UsbInterfaceLink,
};

struct IdentityOnly;
impl GatePolicy for IdentityOnly {
    fn allows(&self, operation: Operation) -> bool {
        operation == Operation::PodxtIdentity
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("Give the USB bus/port path and address.")?;
    let address = args.next().and_then(|value| value.parse::<u8>().ok())
        .filter(|&value| value != 0).ok_or("Give a valid USB address.")?;
    if args.next().is_some() {
        return Err("Give only the USB bus/port path and address.".into());
    }
    let rig = DeviceLock::acquire().map_err(|error| error.to_string())?;
    let mut link = UsbInterfaceLink::open_podxt_midi(&path, address, rig)?;
    let reply = podxt::identify_podxt_live(
        &mut Session::new(&mut link, &IdentityOnly, &mut Quiet),
    );
    match (reply, link.finish()) {
        (Ok(_), Ok(())) => {
            println!("PODxt Live firmware 3.01. Interface 0 snd-usb-pod binding verified.");
            Ok(())
        }
        (Err(error), Ok(())) => Err(format!("{error} Interface 0 driver returned.")),
        (Ok(_), Err(error)) => Err(format!("PODxt Live firmware 3.01. Driver return failed: {error}")),
        (Err(identity), Err(cleanup)) => Err(format!("{identity} Driver return failed: {cleanup}")),
    }
}
```

With the port and address recorded above, run `cargo run --offline -- 8-2.1
11` from the scratch project. The session asks only for identity. The
shared identity exchange permits a single resend after an observed orphan
reply tail. The interface claim ends before the program verifies the driver
binding and releases the rig lock.

## Check and record

- Record the program output and exit status. A pass requires PODxt Live
  firmware 3.01 and a verified return of `snd-usb-pod`.
- Repeat the read-only driver-link checks from Prepare. Interface 0 should
  show `snd-usb-pod` on the same USB address. Interface 1's binding should
  match its starting binding.
- On any identity, claim, or cleanup failure, stop and keep the output.
  If interface 0 remains unbound, leave the card failed and reconnect the
  USB cable before using the normal driver again.
