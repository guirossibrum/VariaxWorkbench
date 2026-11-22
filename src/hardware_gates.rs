//! Compile-time safety gates for physical device operations.
//!
//! G1 (OS-level recognition) is open in this build by the owner's direct
//! permission of 2026-11-22, recorded in `docs/hardware-gates/g1.md` section 5
//! and on issue #125. G1 lets the app read what Linux already holds about
//! connected USB devices and sound cards; it sends nothing to a device. Every
//! other gate stays locked. There is no runtime, build, GUI, command-line, or
//! test mechanism to change these values.

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

/// Open: owner's direct permission for G1 only (g1.md section 5, #125).
pub const G1_OS_DISCOVERY_LOCKED: bool = false;
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

    const LOCKED_GATES: [Gate; 6] = [
        Gate::IdentityRequest,
        Gate::CurrentSettingsRead,
        Gate::AudioCapture,
        Gate::FullBackupRead,
        Gate::EditBufferWrite,
        Gate::StoreRestore,
    ];

    #[test]
    fn only_g1_is_open_by_compile_time_constant() {
        const {
            assert!(!G1_OS_DISCOVERY_LOCKED);
            assert!(G2_IDENTITY_REQUEST_LOCKED);
            assert!(G3_CURRENT_SETTINGS_READ_LOCKED);
            assert!(G4_AUDIO_CAPTURE_LOCKED);
            assert!(G5_FULL_BACKUP_READ_LOCKED);
            assert!(G6_EDIT_BUFFER_WRITE_LOCKED);
            assert!(G7_STORE_RESTORE_LOCKED);
        }
        assert!(!is_locked(Gate::OsDiscovery));
        // The closure only flips a flag: no device, file or OS access.
        let mut ran = false;
        let result = run_if_unlocked(Gate::OsDiscovery, || {
            ran = true;
            Ok(())
        });
        assert!(result.is_ok() && ran, "G1 must run its closure");
    }

    #[test]
    fn g2_to_g7_stay_locked_and_block_their_operation_closure() {
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
