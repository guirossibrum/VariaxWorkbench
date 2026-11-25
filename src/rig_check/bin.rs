//! `rig-check`: the Rig Check app (#243).
//!
//! `rig-check --practice` rehearses every step against simulated devices.
//! `rig-check --real` uses the real rig; in this build every device step is
//! refused until the owner approves it (#267). There is no default mode, so
//! the person always chooses.

use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use variax_workbench::rig_check::{
    self, cli::Terminal, port::RealPort, practice::Faults, redact::Redactor, report, steps::RunData,
};

const USAGE: &str = "Usage: rig-check --practice | --real [--out DIR] [--evidence]
       rig-check --catalogue
  --practice  rehearse every step with simulated devices (nothing is sent to real hardware)
  --real      use the connected PODxt Live and Variax (device steps need the owner's approval)
  --out DIR   folder for the report (default: rig-check-output next to this program)
  --evidence  also save backups and captures in a separate evidence folder";

struct Options {
    practice: bool,
    out: PathBuf,
    evidence: bool,
}

fn default_out() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("rig-check-output")))
        .unwrap_or_else(|| PathBuf::from("rig-check-output"))
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mut practice = None;
    let mut out = None;
    let mut evidence = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--practice" => practice = Some(true),
            "--real" => practice = Some(false),
            "--evidence" => evidence = true,
            "--out" => out = Some(PathBuf::from(args.next().ok_or(USAGE)?)),
            "--catalogue" => {
                for step in rig_check::catalogue::STEPS {
                    println!("{} {} [{}]", step.id, step.title, step.risk.label());
                }
                std::process::exit(0);
            }
            _ => return Err(USAGE.into()),
        }
    }
    Ok(Options {
        practice: practice.ok_or(USAGE)?,
        out: out.unwrap_or_else(default_out),
        evidence,
    })
}

fn main() {
    let options = match options() {
        Ok(options) => options,
        Err(usage) => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    let redactor = Redactor::from_environment();
    let mut sink = match report::ReportFile::create(&options.out, redactor) {
        Ok(sink) => sink,
        Err(error) => {
            eprintln!("Cannot create the report: {error}");
            std::process::exit(1);
        }
    };
    let mut data = RunData::default();
    if options.evidence {
        let stem = sink
            .path
            .file_stem()
            .map_or("rig-check".into(), |s| s.to_string_lossy().into_owned());
        data.evidence_dir = Some(options.out.join(format!("{stem}-evidence")));
    }
    let stdin = io::stdin();
    let mut person = Terminal::new(BufReader::new(stdin.lock()), io::stdout());
    let run = if options.practice {
        rig_check::run_practice(Faults::default(), &mut person, &mut sink, data).0
    } else {
        let mut port = RealPort::default();
        rig_check::run_on(&mut port, &mut person, &mut sink, data).0
    };
    println!(
        "\nRig Check finished: {} passed, {} failed, {} skipped. {}\nWorking report: {}",
        run.count("passed"),
        run.count("failed"),
        run.count("skipped") + run.count("skipped_needs"),
        run.rig_state,
        sink.path.display()
    );
    let mut input = BufReader::new(io::stdin().lock());
    let mut ask = |question: &str| -> String {
        print!("{question} ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        let _ = input.read_line(&mut line);
        line.trim().to_owned()
    };
    if ask("Show the report now? [y/N]").eq_ignore_ascii_case("y") {
        println!("{}", sink.last);
    }
    let target = ask("Save a copy of the report to (file path, Enter to skip):");
    if !target.is_empty() {
        match report::save_copy(&sink.last, std::path::Path::new(&target)) {
            Ok(()) => println!("Saved {target}."),
            Err(error) => println!("Not saved: {error}"),
        }
    }
}
