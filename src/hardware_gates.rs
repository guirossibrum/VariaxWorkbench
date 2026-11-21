//! Compile-time safety gates for physical device operations.
//!
//! Every gate remains locked in this offline-validation phase. There is no
//! runtime, build, GUI, command-line, or test mechanism to change these values.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gate {
    OsDiscovery,
    IdentityRequest,
    CurrentSettingsRead,
    AudioCapture,
    FullBackupRead,
    EditBufferWrite,
    StoreRestore,
}

pub const G1_OS_DISCOVERY_LOCKED: bool = true;
pub const G2_IDENTITY_REQUEST_LOCKED: bool = true;
pub const G3_CURRENT_SETTINGS_READ_LOCKED: bool = true;
pub const G4_AUDIO_CAPTURE_LOCKED: bool = true;
pub const G5_FULL_BACKUP_READ_LOCKED: bool = true;
pub const G6_EDIT_BUFFER_WRITE_LOCKED: bool = true;
pub const G7_STORE_RESTORE_LOCKED: bool = true;

pub const fn is_locked(gate: Gate) -> bool {
    match gate {
        Gate::OsDiscovery => G1_OS_DISCOVERY_LOCKED,
        Gate::IdentityRequest => G2_IDENTITY_REQUEST_LOCKED,
        Gate::CurrentSettingsRead => G3_CURRENT_SETTINGS_READ_LOCKED,
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

    const ALL_GATES: [Gate; 7] = [
        Gate::OsDiscovery,
        Gate::IdentityRequest,
        Gate::CurrentSettingsRead,
        Gate::AudioCapture,
        Gate::FullBackupRead,
        Gate::EditBufferWrite,
        Gate::StoreRestore,
    ];

    #[test]
    fn every_gate_is_compile_time_locked_and_blocks_its_operation_closure() {
        for gate in ALL_GATES {
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
