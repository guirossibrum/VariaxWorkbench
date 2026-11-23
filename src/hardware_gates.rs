//! Compile-time safety gates for physical device operations.
//!
//! G1 (OS-level recognition) is open by the owner's direct permission of
//! 2026-11-22, recorded in `docs/hardware-gates/g1.md`. G2 (one Variax 600
//! identity request) is open by the owner's direct permission of 2026-11-22,
//! recorded in `docs/hardware-gates/g2.md`, for any user of the same supported
//! rig. Device-check step 3 reads the Variax and the PODxt Live. Step 4 stays
//! off. The legacy settings lock stays closed so the editor fetch stays off.
//! The user runs those checks. Agents do not. Gates G4–G7 stay locked. There is
//! no runtime, command-line, GUI, or test switch that changes these constants.
//! Test builds do not open a USB device.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    OsDiscovery,
    IdentityRequest,
    CurrentSettingsRead,
    DeviceCheckCurrentSettingsRead,
    AudioCapture,
    FullBackupRead,
    EditBufferWrite,
    StoreRestore,
}

/// Open: owner's direct permission for G1 (g1.md section 5, #125).
pub const G1_OS_DISCOVERY_LOCKED: bool = false;
/// Open: owner's direct permission on 2026-11-22 for the Variax 600 identity
/// request through the PODxt Live, for any user, following
/// `docs/hardware-gates/g2.md`. No later gate opens until this one has worked.
/// #114 stays open. The user runs the check. Agents do not.
pub const G2_IDENTITY_REQUEST_LOCKED: bool = false;
/// The editor fetch and change check retain the original locked setting.
pub const G3_CURRENT_SETTINGS_READ_LOCKED: bool = true;
/// The approved Device check step 3 procedure has its own compile-time lock.
pub const G3_DEVICE_CHECK_READ_LOCKED: bool = false;
pub const G4_AUDIO_CAPTURE_LOCKED: bool = true;
pub const G5_FULL_BACKUP_READ_LOCKED: bool = true;
pub const G6_EDIT_BUFFER_WRITE_LOCKED: bool = true;
pub const G7_STORE_RESTORE_LOCKED: bool = true;

pub const fn is_locked(gate: Gate) -> bool {
    match gate {
        Gate::OsDiscovery => G1_OS_DISCOVERY_LOCKED,
        Gate::IdentityRequest => G2_IDENTITY_REQUEST_LOCKED,
        Gate::CurrentSettingsRead => G3_CURRENT_SETTINGS_READ_LOCKED,
        Gate::DeviceCheckCurrentSettingsRead => G3_DEVICE_CHECK_READ_LOCKED,
        Gate::AudioCapture => G4_AUDIO_CAPTURE_LOCKED,
        Gate::FullBackupRead => G5_FULL_BACKUP_READ_LOCKED,
        Gate::EditBufferWrite => G6_EDIT_BUFFER_WRITE_LOCKED,
        Gate::StoreRestore => G7_STORE_RESTORE_LOCKED,
    }
}

/// Run an operation only when its compile-time gate is open.
///
/// Live adapter call sites use this around discovery/open and device I/O. The
/// closure is never invoked for a locked gate.
pub fn run_if_unlocked<T>(
    gate: Gate,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if is_locked(gate) {
        return Err(format!(
            "hardware gate {gate:?} is locked; no device operation was attempted"
        ));
    }
    operation()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCKED_GATES: [Gate; 5] = [
        Gate::CurrentSettingsRead,
        Gate::AudioCapture,
        Gate::FullBackupRead,
        Gate::EditBufferWrite,
        Gate::StoreRestore,
    ];

    #[test]
    fn g1_g2_and_device_check_read_are_open() {
        const {
            assert!(!G1_OS_DISCOVERY_LOCKED);
            assert!(!G2_IDENTITY_REQUEST_LOCKED);
            assert!(G3_CURRENT_SETTINGS_READ_LOCKED);
            assert!(!G3_DEVICE_CHECK_READ_LOCKED);
            assert!(G4_AUDIO_CAPTURE_LOCKED);
            assert!(G5_FULL_BACKUP_READ_LOCKED);
            assert!(G6_EDIT_BUFFER_WRITE_LOCKED);
            assert!(G7_STORE_RESTORE_LOCKED);
        }
        assert!(!is_locked(Gate::OsDiscovery));
        assert!(!is_locked(Gate::IdentityRequest));
        // The closures only flip a flag: no device, file or OS access.
        for gate in [
            Gate::OsDiscovery,
            Gate::IdentityRequest,
            Gate::DeviceCheckCurrentSettingsRead,
        ] {
            let mut ran = false;
            let result = run_if_unlocked(gate, || {
                ran = true;
                Ok(())
            });
            assert!(result.is_ok() && ran, "{gate:?} must run its closure");
        }
    }

    #[test]
    fn g3_to_g7_stay_locked_and_block_their_operation_closure() {
        for gate in LOCKED_GATES {
            assert!(is_locked(gate), "{gate:?} must remain locked");
            let mut opened_or_enumerated = false;
            let result = run_if_unlocked(gate, || {
                opened_or_enumerated = true;
                Ok(())
            });
            assert!(result.is_err(), "{gate:?} must reject the operation");
            assert!(!opened_or_enumerated, "{gate:?} reached a device action");
        }
    }
}
