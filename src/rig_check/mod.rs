//! Rig Check: a standalone test-and-evidence app for the PODxt Live and
//! Variax rig (#243).
//!
//! It walks a person through the catalogue (`docs/rig-check/catalogue.md`)
//! one explained, confirmed step at a time and writes a text report after
//! every step. Device steps use the same device library as the editor
//! (`line6-devices`). Practice mode runs the whole catalogue against
//! simulated devices ([`practice`]); real mode refuses every device step
//! until the owner approves it ([`approvals`], #267).
//!
//! Hardware boundary: tests use the simulated rig and synthetic data only.
//! They never open USB, MIDI or audio devices and never read real `/proc` or
//! `/sys`.

pub mod approvals;
pub mod catalogue;
pub mod cli;
pub mod computer;
pub mod connection;
pub mod engine_check;
pub mod evidence;
#[cfg(feature = "gui")]
pub mod gui;
pub mod guided;
pub mod hints;
pub mod live;
pub mod podxt_reads;
pub mod podxt_save;
pub mod port;
pub mod practice;
pub mod procedures;
pub mod record;
pub mod recover;
pub mod redact;
pub mod report;
pub mod runner;
pub mod steps;
#[cfg(test)]
pub(crate) mod testing;
pub mod variax_live;
pub mod variax_reads;
pub mod variax_save;

use port::{PracticePort, RigPort};
use practice::{Faults, SimRig};
use runner::{Operator, ReportSink, Runner};
use steps::RunData;

/// Run the whole catalogue in practice mode against a fresh simulated rig.
pub fn run_practice(
    faults: Faults,
    op: &mut dyn Operator,
    sink: &mut dyn ReportSink,
    data: RunData,
) -> (
    record::RunRecord,
    RunData,
    std::rc::Rc<std::cell::RefCell<SimRig>>,
) {
    let rig = SimRig::shared(faults);
    let mut port = PracticePort::new(rig.clone());
    let (run, data) = Runner::new(&mut port, op, sink, data).run();
    (run, data, rig)
}

/// Run the whole catalogue, keeping a crash-restore state file (#261).
pub fn run_with_state(
    port: &mut dyn RigPort,
    op: &mut dyn Operator,
    sink: &mut dyn ReportSink,
    data: RunData,
    state_file: std::path::PathBuf,
) -> (record::RunRecord, RunData) {
    let mut runner = Runner::new(port, op, sink, data);
    runner.state_file = Some(state_file);
    runner.run()
}

/// Run the whole catalogue on a caller-supplied port (the real rig).
pub fn run_on(
    port: &mut dyn RigPort,
    op: &mut dyn Operator,
    sink: &mut dyn ReportSink,
    data: RunData,
) -> (record::RunRecord, RunData) {
    Runner::new(port, op, sink, data).run()
}
