#![cfg(target_os = "linux")]

use line6_devices::device_lock::{DeviceLock, LockError};
use std::path::Path;
use std::process::Command;

const ROLE: &str = "VARIAX_DEVICE_LOCK_INHERITANCE_ROLE";
const TEST: &str = "the_owner_child_inherits_the_locked_descriptor";

fn child(role: &str, runtime: &Path) -> Command {
    let mut command = Command::new(std::env::args_os().next().unwrap());
    command
        .args(["--exact", TEST, "--nocapture"])
        .env(ROLE, role)
        .env("XDG_RUNTIME_DIR", runtime);
    command
}

fn assert_child_passed(command: &mut Command) {
    let output = command.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{stdout}\n{stderr}"
    );
}

// Keep this in its own test binary. Before exec, a subprocess can briefly
// retain open file descriptions from unrelated tests in a parallel harness.
#[test]
fn the_owner_child_inherits_the_locked_descriptor() {
    match std::env::var(ROLE).as_deref() {
        Ok("owner") => {
            let runtime = std::env::var_os("XDG_RUNTIME_DIR").unwrap();
            let rig = DeviceLock::acquire().unwrap();
            assert_child_passed(
                child("probe", Path::new(&runtime)).stdin(rig.child_stdin().unwrap()),
            );
        }
        Ok("probe") => {
            DeviceLock::verify_inherited_stdin().unwrap();
            assert!(matches!(DeviceLock::acquire(), Err(LockError::Busy)));
        }
        _ => {
            let runtime = std::env::temp_dir()
                .join(format!("variax-inherited-lock-test-{}", std::process::id()));
            std::fs::create_dir(&runtime).unwrap();
            assert_child_passed(&mut child("owner", &runtime));
            std::fs::remove_dir_all(runtime).unwrap();
        }
    }
}
