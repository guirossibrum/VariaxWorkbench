//! Compile-time safety gates for physical device operations.
//!
//! On 2026-11-23 the owner released every gate (G1-G8) by direct permission,
//! after all earlier owner-run tests passed, so that the next owner-run test
//! can write to stored banks on the Variax and the PODxt Live. Every lock
//! below is therefore open. The gate structure stays: each device path still
//! names its gate and goes through [`run_if_unlocked`], so a gate can be closed
//! again by setting its constant back to `true`. There is no runtime,
//! command-line, GUI, or test switch that changes these constants. The user
//! runs device operations. Agents do not. Test builds do not open a USB device.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    OsDiscovery,
    IdentityRequest,
    CurrentSettingsRead,
    DeviceCheckCurrentSettingsRead,
    AudioCapture,
    FullBackupRead,
    /// Device-check library backup only. The editor and every other path stay
    /// behind [`G5_FULL_BACKUP_READ_LOCKED`].
    DeviceCheckBackup,
    EditBufferWrite,
    /// Device-check try: change the active PODxt Live tone, read it back, then
    /// put the original values back. Does not store the tone.
    DeviceCheckLiveTest,
    /// Leave a verified edit in the active PODxt Live tone. Separate from the try.
    KeepLiveEdit,
    /// Device-check saved-tone selection. Does not store a tone.
    DeviceCheckChannel,
    StoreRestore,
}

/// Open: owner's direct permission for G1 (g1.md section 5, #125).
pub const G1_OS_DISCOVERY_LOCKED: bool = false;
/// Open: owner's direct permission on 2026-11-22 for the Variax 600 identity
/// request through the PODxt Live, for any user, following
/// `docs/hardware-gates/g2.md`. No later gate opens until this one has worked.
/// #114 stays open. The user runs the check. Agents do not.
pub const G2_IDENTITY_REQUEST_LOCKED: bool = false;
/// The editor fetch and change check. Released 2026-11-23.
pub const G3_CURRENT_SETTINGS_READ_LOCKED: bool = false;
/// The approved Device check step 3 procedure has its own compile-time lock.
pub const G3_DEVICE_CHECK_READ_LOCKED: bool = false;
/// Released 2026-11-23 with every other gate.
pub const G4_AUDIO_CAPTURE_LOCKED: bool = false;
/// Released 2026-11-23 with every other gate.
pub const G5_FULL_BACKUP_READ_LOCKED: bool = false;
/// Open for the Device check backup choices only. The owner asked for the
/// Variax and PODxt Live library backups from `tools/device-controls` to be
/// in the app. The Variax backup sends program changes so it can read each
/// stored preset, then tries to switch back. It does not store anything.
/// The PODxt Live backup reads stored tones and setups by memory location
/// and does not change the current tone. Step 4 stays off. Agents do not
/// run it.
pub const G5_DEVICE_CHECK_BACKUP_LOCKED: bool = false;
/// Released 2026-11-23 with every other gate.
pub const G6_EDIT_BUFFER_WRITE_LOCKED: bool = false;
/// Open for the Device check tone try only. The try changes a few PODxt Live
/// controls, reads them back, and puts the original values back. It does not
/// store the tone. The stomp is not one of those controls. Agents do not run it.
pub const G6_DEVICE_CHECK_LIVE_TEST_LOCKED: bool = false;
/// Leaving a verified edit in the active tone. Its own gate. Released 2026-11-23.
pub const G6_KEEP_LIVE_EDIT_LOCKED: bool = false;
/// Open for the Device check saved-tone switch only. It selects a stored tone
/// the way the footswitch does, and only when the current tone already matches
/// one saved tone. It does not store anything. Agents do not run it.
pub const G6_DEVICE_CHECK_CHANNEL_LOCKED: bool = false;
/// Stored-bank writes and restore. Released 2026-11-23 for the owner-run
/// bank-write tests; each write path still goes through this gate.
pub const G7_STORE_RESTORE_LOCKED: bool = false;

pub const fn is_locked(gate: Gate) -> bool {
    match gate {
        Gate::OsDiscovery => G1_OS_DISCOVERY_LOCKED,
        Gate::IdentityRequest => G2_IDENTITY_REQUEST_LOCKED,
        Gate::CurrentSettingsRead => G3_CURRENT_SETTINGS_READ_LOCKED,
        Gate::DeviceCheckCurrentSettingsRead => G3_DEVICE_CHECK_READ_LOCKED,
        Gate::AudioCapture => G4_AUDIO_CAPTURE_LOCKED,
        Gate::FullBackupRead => G5_FULL_BACKUP_READ_LOCKED,
        Gate::DeviceCheckBackup => G5_DEVICE_CHECK_BACKUP_LOCKED,
        Gate::EditBufferWrite => G6_EDIT_BUFFER_WRITE_LOCKED,
        Gate::DeviceCheckLiveTest => G6_DEVICE_CHECK_LIVE_TEST_LOCKED,
        Gate::KeepLiveEdit => G6_KEEP_LIVE_EDIT_LOCKED,
        Gate::DeviceCheckChannel => G6_DEVICE_CHECK_CHANNEL_LOCKED,
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
    run_unless_locked(is_locked(gate), gate, operation)
}

fn run_unless_locked<T>(
    locked: bool,
    gate: Gate,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if locked {
        return Err(format!(
            "hardware gate {gate:?} is locked; no device operation was attempted"
        ));
    }
    operation()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_GATES: [Gate; 12] = [
        Gate::OsDiscovery,
        Gate::IdentityRequest,
        Gate::CurrentSettingsRead,
        Gate::DeviceCheckCurrentSettingsRead,
        Gate::AudioCapture,
        Gate::FullBackupRead,
        Gate::DeviceCheckBackup,
        Gate::EditBufferWrite,
        Gate::DeviceCheckLiveTest,
        Gate::KeepLiveEdit,
        Gate::DeviceCheckChannel,
        Gate::StoreRestore,
    ];

    #[test]
    fn every_gate_is_released_by_the_owner() {
        const {
            assert!(!G1_OS_DISCOVERY_LOCKED);
            assert!(!G2_IDENTITY_REQUEST_LOCKED);
            assert!(!G3_CURRENT_SETTINGS_READ_LOCKED);
            assert!(!G3_DEVICE_CHECK_READ_LOCKED);
            assert!(!G4_AUDIO_CAPTURE_LOCKED);
            assert!(!G5_FULL_BACKUP_READ_LOCKED);
            assert!(!G5_DEVICE_CHECK_BACKUP_LOCKED);
            assert!(!G6_EDIT_BUFFER_WRITE_LOCKED);
            assert!(!G6_DEVICE_CHECK_LIVE_TEST_LOCKED);
            assert!(!G6_KEEP_LIVE_EDIT_LOCKED);
            assert!(!G6_DEVICE_CHECK_CHANNEL_LOCKED);
            assert!(!G7_STORE_RESTORE_LOCKED);
        }
        // The closures only flip a flag: no device, file or OS access.
        for gate in ALL_GATES {
            assert!(!is_locked(gate), "{gate:?} is released");
            let mut ran = false;
            let result = run_if_unlocked(gate, || {
                ran = true;
                Ok(())
            });
            assert!(result.is_ok() && ran, "{gate:?} must run its closure");
        }
    }

    #[test]
    fn a_locked_gate_never_runs_its_operation() {
        for gate in ALL_GATES {
            let mut reached = false;
            let result = run_unless_locked(true, gate, || {
                reached = true;
                Ok(())
            });
            assert!(result.is_err(), "{gate:?} must reject the operation");
            assert!(!reached, "{gate:?} reached a device action");
        }
    }
}
