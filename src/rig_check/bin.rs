//! `rig-check`: the Rig Check app (#243).
//!
//! `rig-check --practice` rehearses every step against simulated devices.
//! `rig-check --real` uses the real rig; in this build every device step is
//! refused until the owner approves it (#267). There is no default mode, so
//! the person always chooses.

use std::io::{self, BufReader};
use variax_workbench::rig_check::{
    self, cli::Terminal, port::RealPort, practice::Faults, runner::NoReport, steps::RunData,
};

const USAGE: &str = "Usage: rig-check --practice | --real
       rig-check --catalogue
  --practice  rehearse every step with simulated devices (nothing is sent to real hardware)
  --real      use the connected PODxt Live and Variax (device steps need the owner's approval)";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match args.first().map(String::as_str) {
        Some("--practice") => "practice",
        Some("--real") => "real",
        Some("--catalogue") => {
            for step in rig_check::catalogue::STEPS {
                println!("{} {} [{}]", step.id, step.title, step.risk.label());
            }
            return;
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    let stdin = io::stdin();
    let mut person = Terminal::new(BufReader::new(stdin.lock()), io::stdout());
    let (run, _) = if mode == "practice" {
        let (run, data, _) = rig_check::run_practice(
            Faults::default(),
            &mut person,
            &mut NoReport,
            RunData::default(),
        );
        (run, data)
    } else {
        let mut port = RealPort::default();
        rig_check::run_on(&mut port, &mut person, &mut NoReport, RunData::default())
    };
    println!(
        "\nRig Check finished: {} passed, {} failed, {} skipped. {}",
        run.count("passed"),
        run.count("failed"),
        run.count("skipped") + run.count("skipped_needs"),
        run.rig_state
    );
}
