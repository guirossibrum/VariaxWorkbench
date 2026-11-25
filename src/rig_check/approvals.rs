//! Per-step owner approvals for real devices.
//!
//! The owner released device and guided steps for the first real run
//! (2026-11-25, #267). Audio stays closed. Each step also requires open
//! hardware gates and the person's Run choice. Saves require every applicable
//! earlier step to pass and a separate typed confirmation. Practice uses only
//! simulated devices.

use super::catalogue::{self, StepId};
use crate::hardware_gates::{self, Gate};

pub const APPROVED_STEP_1_1: bool = true;
pub const APPROVED_STEP_1_2: bool = true;
pub const APPROVED_STEP_2_1: bool = true;
pub const APPROVED_STEP_2_2: bool = true;
pub const APPROVED_STEP_2_3: bool = true;
pub const APPROVED_STEP_2_4: bool = true;
pub const APPROVED_STEP_2_5: bool = true;
pub const APPROVED_STEP_2_6: bool = true;
pub const APPROVED_STEP_3_1: bool = true;
pub const APPROVED_STEP_3_2: bool = true;
pub const APPROVED_STEP_3_3: bool = true;
pub const APPROVED_STEP_3_4: bool = true;
pub const APPROVED_STEP_3_5: bool = true;
pub const APPROVED_STEP_3_6: bool = true;
pub const APPROVED_VARIAX_EVIDENCE: bool = true;
pub const APPROVED_STEP_4_1: bool = true;
pub const APPROVED_STEP_4_2: bool = true;
pub const APPROVED_STEP_4_3: bool = true;
pub const APPROVED_STEP_4_4: bool = true;
pub const APPROVED_STEP_4_5: bool = true;
pub const APPROVED_STEP_4_6: bool = true;
pub const APPROVED_STEP_4_7: bool = true;
pub const APPROVED_STEP_4_8: bool = true;
pub const APPROVED_STEP_5_1: bool = true;
pub const APPROVED_STEP_5_2: bool = true;
pub const APPROVED_STEP_5_3: bool = true;
pub const APPROVED_STEP_5_4: bool = true;
pub const APPROVED_STEP_6_1: bool = true;
pub const APPROVED_STEP_6_2: bool = true;
pub const APPROVED_STEP_7_1: bool = true;
pub const APPROVED_STEP_7_2: bool = true;
pub const APPROVED_STEP_7_3: bool = true;
pub const APPROVED_STEP_7_4: bool = true;
pub const APPROVED_STEP_7_5: bool = true;
pub const APPROVED_STEP_8_2: bool = false;
pub const APPROVED_STEP_8_3: bool = false;
pub const APPROVED_STEP_8_4: bool = false;
pub const APPROVED_STEP_8_5: bool = false;
/// Opening any audio stream needs the owner's separate approval (AGENTS.md).
pub const AUDIO_STREAMS_APPROVED: bool = false;

pub const NOT_AUTHORIZED: &str = "not yet authorized by the owner (#267)";

/// The owner's approval for one step. Computer steps need none.
pub const fn approved(id: StepId) -> bool {
    match (id.0, id.1) {
        (1, 1) => APPROVED_STEP_1_1,
        (1, 2) => APPROVED_STEP_1_2,
        (2, 1) => APPROVED_STEP_2_1,
        (2, 2) => APPROVED_STEP_2_2,
        (2, 3) => APPROVED_STEP_2_3,
        (2, 4) => APPROVED_STEP_2_4,
        (2, 5) => APPROVED_STEP_2_5,
        (2, 6) => APPROVED_STEP_2_6,
        (3, 1) => APPROVED_STEP_3_1,
        (3, 2) => APPROVED_STEP_3_2,
        (3, 3) => APPROVED_STEP_3_3,
        (3, 4) => APPROVED_STEP_3_4,
        (3, 5) => APPROVED_STEP_3_5,
        (3, 6) => APPROVED_STEP_3_6,
        (3, 7) => APPROVED_VARIAX_EVIDENCE,
        (4, 1) => APPROVED_STEP_4_1,
        (4, 2) => APPROVED_STEP_4_2,
        (4, 3) => APPROVED_STEP_4_3,
        (4, 4) => APPROVED_STEP_4_4,
        (4, 5) => APPROVED_STEP_4_5,
        (4, 6) => APPROVED_STEP_4_6,
        (4, 7) => APPROVED_STEP_4_7,
        (4, 8) => APPROVED_STEP_4_8,
        (5, 1) => APPROVED_STEP_5_1,
        (5, 2) => APPROVED_STEP_5_2,
        (5, 3) => APPROVED_STEP_5_3,
        (5, 4) => APPROVED_STEP_5_4,
        (6, 1) => APPROVED_STEP_6_1,
        (6, 2) => APPROVED_STEP_6_2,
        (7, 1) => APPROVED_STEP_7_1,
        (7, 2) => APPROVED_STEP_7_2,
        (7, 3) => APPROVED_STEP_7_3,
        (7, 4) => APPROVED_STEP_7_4,
        (7, 5) => APPROVED_STEP_7_5,
        (8, 2) => APPROVED_STEP_8_2 && AUDIO_STREAMS_APPROVED,
        (8, 3) => APPROVED_STEP_8_3 && AUDIO_STREAMS_APPROVED,
        (8, 4) => APPROVED_STEP_8_4 && AUDIO_STREAMS_APPROVED,
        (8, 5) => APPROVED_STEP_8_5 && AUDIO_STREAMS_APPROVED,
        _ => false,
    }
}

/// Whether a step may touch real devices on this build. `Ok` for computer
/// steps; otherwise the step's approval and every gate it names must be open.
pub fn real_device_check(id: StepId) -> Result<(), String> {
    let Some(step) = catalogue::step(id) else {
        return Err(format!("Step {id} is not in the catalogue."));
    };
    if !step.needs_device_approval() {
        return Ok(());
    }
    if cfg!(target_os = "macos") {
        return Err(crate::MAC_CONNECTION_UNAVAILABLE.into());
    }
    if !approved(id) {
        return Err(format!(
            "Step {id} is {NOT_AUTHORIZED}. Nothing was sent to any device."
        ));
    }
    if id == catalogue::VARIAX_EVIDENCE
        && !line6_devices::family::variax::firmware_read::APPROVED_FIRMWARE_READ
    {
        return Err(
            "The device crate's Variax firmware compatibility read is closed. Nothing was sent."
                .into(),
        );
    }
    check_gates(id, step.gates)
}

fn check_gates(id: StepId, gates: &[Gate]) -> Result<(), String> {
    for gate in gates {
        hardware_gates::run_if_unlocked(*gate, || Ok(()))
            .map_err(|error| format!("Step {id}: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig_check::catalogue::{Kind, STEPS};

    #[test]
    fn device_and_guided_steps_are_released() {
        for step in STEPS
            .iter()
            .filter(|step| matches!(step.kind, Kind::Device | Kind::Guided))
        {
            assert!(approved(step.id), "{}", step.id);
            let result = real_device_check(step.id);
            if cfg!(target_os = "macos") {
                assert_eq!(result, Err(crate::MAC_CONNECTION_UNAVAILABLE.into()));
            } else {
                assert!(result.is_ok(), "{}", step.id);
            }
        }
    }

    #[test]
    fn audio_streams_and_audio_steps_are_refused() {
        for step in STEPS.iter().filter(|step| step.kind == Kind::Audio) {
            assert!(!approved(step.id), "{}", step.id);
            let error = real_device_check(step.id).unwrap_err();
            if cfg!(target_os = "macos") {
                assert_eq!(error, crate::MAC_CONNECTION_UNAVAILABLE);
            } else {
                assert!(error.contains("not yet authorized by the owner (#267)"));
                assert!(error.contains("Nothing was sent"));
            }
        }
    }
}
