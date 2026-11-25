//! Steps 1.1 and 1.2 (#253): see the PODxt Live on USB, then open and give
//! back the guitar interface, verifying the driver binding (#239).

use super::computer::{UsbDevice, linux};
use super::record::Mode;
use super::runner::Ctx;
use crate::device_probe::{EXPECTED_POD_DRIVER, EXPECTED_VARIAX_DRIVER};

pub const ABSENT: &str = "Device absent: no PODxt Live (USB 0e41:4650) is connected. Connect it by USB, switch it on, wait 10 seconds and run step 0.2 again.";
pub const MORE_THAN_ONE: &str = "More than one PODxt Live is connected. Rig Check tests one rig at a time: disconnect the others.";
pub const PERMISSION: &str = "Permission denied: this user may not open the PODxt Live. Step 0.2 shows the device node permissions.";
pub const BUSY: &str = "Interface busy: another program (Variax Workbench, Line 6 Edit, a DAW or a second Rig Check) is using the guitar interface. Close it and try again.";
pub const NOT_RETURNED: &str = "Driver not returned: after release, the guitar interface has no snd-usb-variax driver. Unplug and replug the PODxt Live USB cable.";

/// A plain-language text for an error from opening the guitar interface.
pub fn classify_open_error(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    let text = if lower.contains("found 0")
        || lower.contains("no pod")
        || lower.contains("no such device")
    {
        ABSENT
    } else if lower.contains("access") || lower.contains("permission") {
        PERMISSION
    } else if lower.contains("busy") {
        BUSY
    } else {
        return format!("Could not open the guitar interface: {error}");
    };
    format!("{text} (system: {error})")
}

fn the_podxt(ctx: &Ctx<'_>) -> Result<UsbDevice, String> {
    let view = ctx.data.usb.as_ref().ok_or("No USB list (step 0.2).")?;
    match view.podxt_lives()[..] {
        [] => Err(ABSENT.into()),
        [device] => Ok(device.clone()),
        _ => Err(MORE_THAN_ONE.into()),
    }
}

pub fn see_podxt(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let device = the_podxt(ctx)?;
    ctx.rec
        .note(format!("PODxt Live found on port {}.", device.port));
    ctx.rec.note(device.describe());
    for (interface, expected) in [(0, EXPECTED_POD_DRIVER), (1, EXPECTED_VARIAX_DRIVER)] {
        match device.driver(interface) {
            Some(driver) if driver.replace('_', "-") == expected => ctx
                .rec
                .note(format!("Interface {interface}: {driver} (expected).")),
            Some(driver) => ctx.rec.note(format!(
                "Interface {interface}: {driver}, expected {expected}. Another driver holds it."
            )),
            None => ctx.rec.note(format!(
                "Interface {interface}: no driver, expected {expected}."
            )),
        }
    }
    if let Some(node) = &device.node {
        ctx.rec.note(format!("Device node: {node}."));
        if !node.contains("rw-rw-rw") && ctx.mode == Mode::Real {
            ctx.rec.note("The device node is not open to every user; if a later step says permission denied, check your group membership for this node.");
        }
    }
    ctx.data
        .devices
        .push(("PODxt USB".into(), format!("port {}", device.port)));
    Ok(())
}

fn guitar_driver(ctx: &Ctx<'_>, port: &str) -> Option<String> {
    let system = ctx.port.system();
    let view = linux::usb(&*system, "");
    view.devices
        .into_iter()
        .find(|d| d.port == port)
        .and_then(|d| d.driver(1).map(str::to_owned))
}

pub fn open_and_release(ctx: &mut Ctx<'_>) -> Result<(), String> {
    let device = the_podxt(ctx)?;
    let linux_view = cfg!(target_os = "linux") || ctx.mode == Mode::Practice;
    if linux_view {
        let before = guitar_driver(ctx, &device.port);
        ctx.rec.note(format!(
            "Before: interface 1 driver {}.",
            before.as_deref().unwrap_or("(none)")
        ));
    }
    let step = ctx.step();
    ctx.port
        .variax(step)
        .map(|_| ())
        .map_err(|error| classify_open_error(&error))?;
    ctx.rec
        .note("Guitar interface opened (interface 1 claimed). No message was sent.");
    let released = ctx.port.release_variax();
    let lines = released.map_err(|error| {
        if error.to_ascii_lowercase().contains("driver") {
            format!("{NOT_RETURNED} ({error})")
        } else {
            format!("Release failed: {error}")
        }
    })?;
    for line in lines {
        ctx.rec.note(line);
    }
    if linux_view {
        let after = guitar_driver(ctx, &device.port);
        ctx.rec.note(format!(
            "After: interface 1 driver {}.",
            after.as_deref().unwrap_or("(none)")
        ));
        if after.as_deref().map(|d| d.replace('_', "-")) != Some(EXPECTED_VARIAX_DRIVER.into()) {
            return Err(NOT_RETURNED.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::StepId;
    use crate::rig_check::practice::Faults;
    use crate::rig_check::record::Outcome;
    use crate::rig_check::run_practice;
    use crate::rig_check::runner::NoReport;
    use crate::rig_check::steps::RunData;
    use crate::rig_check::testing::Script;

    fn failure(faults: Faults, step: StepId) -> String {
        let mut person = Script::run_all();
        let (run, _, _) = run_practice(faults, &mut person, &mut NoReport, RunData::default());
        match &run.step(step).outcome {
            Outcome::Failed(reason) => reason.clone(),
            other => panic!("{step}: {other:?}"),
        }
    }

    #[test]
    fn clear_failure_texts() {
        assert!(
            failure(
                Faults {
                    podxt_absent: true,
                    ..Faults::default()
                },
                StepId(1, 1)
            )
            .starts_with("Device absent")
        );
        assert!(
            failure(
                Faults {
                    interface_busy: true,
                    ..Faults::default()
                },
                StepId(1, 2)
            )
            .starts_with("Interface busy")
        );
        assert!(
            failure(
                Faults {
                    permission_denied: true,
                    ..Faults::default()
                },
                StepId(1, 2)
            )
            .starts_with("Permission denied")
        );
        assert!(
            failure(
                Faults {
                    driver_not_returned: true,
                    ..Faults::default()
                },
                StepId(1, 2)
            )
            .starts_with("Driver not returned")
        );
        assert!(
            classify_open_error("Expected one USB device 0e41:4650, found 0.")
                .starts_with("Device absent")
        );
        assert!(
            classify_open_error("claim interface 1: Resource busy").starts_with("Interface busy")
        );
        assert!(
            classify_open_error("open: Access denied (insufficient permissions)")
                .starts_with("Permission denied")
        );
    }

    #[test]
    fn the_report_records_port_speed_and_drivers() {
        let mut person = Script::run_all();
        let (run, _, rig) = run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        let see = run.step(StepId(1, 1));
        assert!(
            see.evidence
                .iter()
                .any(|l| l == "PODxt Live found on port 1-2.")
        );
        assert!(see.evidence.iter().any(|l| l.contains("speed 12 Mbit/s")));
        let open = run.step(StepId(1, 2));
        assert!(
            open.evidence
                .iter()
                .any(|l| l == "After: interface 1 driver snd-usb-variax.")
        );
        assert!(open.messages.is_empty(), "1.2 sends nothing");
        assert!(rig.borrow().driver_bound);
    }
}
