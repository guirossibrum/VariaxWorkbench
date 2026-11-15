//! Offline preview of the future short hardware commissioning workflow.
use std::{env, io, path::Path};
use variax_workbench::{document::save_new, manual_check};

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [command] if command == "plan" => {
            println!("{}", manual_check::plan());
            Ok(())
        }
        [command, output] if command == "rehearse" => {
            let report = manual_check::rehearse(&mut io::stdin().lock(), &mut io::stdout().lock())?;
            save_new(Path::new(output), report.as_bytes())?;
            println!("Saved offline rehearsal report to {output}");
            Ok(())
        }
        _ => Err("Usage: variax-check plan | rehearse NEW_REPORT.txt\nOffline rehearsal only; no live mode exists.".into()),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
