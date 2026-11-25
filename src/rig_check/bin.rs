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

const USAGE: &str = "Usage: rig-check [--practice | --real] [--text] [--out DIR] [--evidence] [--engine-folder DIR]
       rig-check --catalogue
       rig-check --mockups DIR   (draw the wizard screens to PNG files, headless)
  Without --text, a build with the gui feature opens the wizard window.
  --practice  rehearse every step with simulated devices (nothing is sent to real hardware)
  --real      use the connected PODxt Live and Variax (device steps need the owner's approval)
  --out DIR   folder for the report (default: rig-check-output next to this program)
  --evidence  also save backups and captures in a separate evidence folder
  --engine-folder DIR  your own reference models for step 0.3 (models.tsv, NAME.vwm, NAME.packed or NAME.sha256)";

struct Options {
    /// `None`: the person chooses in the window.
    practice: Option<bool>,
    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    text: bool,
    out: PathBuf,
    evidence: bool,
    engine_folder: Option<PathBuf>,
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
    let mut text = false;
    let mut engine_folder = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--practice" => practice = Some(true),
            "--real" => practice = Some(false),
            "--evidence" => evidence = true,
            "--text" => text = true,
            #[cfg(feature = "gui")]
            "--mockups" => {
                let dir = PathBuf::from(args.next().ok_or(USAGE)?);
                for path in rig_check::gui::write_mockups(&dir)? {
                    println!("{}", path.display());
                }
                std::process::exit(0);
            }
            "--out" => out = Some(PathBuf::from(args.next().ok_or(USAGE)?)),
            "--engine-folder" => engine_folder = Some(PathBuf::from(args.next().ok_or(USAGE)?)),
            "--catalogue" => {
                for step in rig_check::catalogue::STEPS {
                    println!("{} {} [{}]", step.id, step.title, step.risk.label());
                }
                std::process::exit(0);
            }
            _ => return Err(USAGE.into()),
        }
    }
    if practice.is_none() && (text || !cfg!(feature = "gui")) {
        return Err(USAGE.into());
    }
    Ok(Options {
        practice,
        text,
        out: out.unwrap_or_else(default_out),
        evidence,
        engine_folder,
    })
}

/// The wizard window. The runner works on its own thread.
#[cfg(feature = "gui")]
fn window(options: Options) {
    use eframe::egui;
    use rig_check::gui::{App, ChannelOperator, Prompt, Wizard};
    use rig_check::record::Mode;
    let out = options.out.clone();
    let evidence = options.evidence;
    let engine_folder = options.engine_folder.clone();
    let start = move |mode: Mode, mut op: ChannelOperator| {
        let mut sink = match report::ReportFile::create(&out, Redactor::from_environment()) {
            Ok(sink) => sink,
            Err(error) => {
                let _ = op.to_ui.send(Prompt::Progress(format!(
                    "Cannot create the report: {error}"
                )));
                return;
            }
        };
        let mut data = RunData {
            engine_folder,
            ..RunData::default()
        };
        if evidence {
            let stem = sink
                .path
                .file_stem()
                .map_or("rig-check".into(), |s| s.to_string_lossy().into_owned());
            data.evidence_dir = Some(out.join(format!("{stem}-evidence")));
        }
        let run = if mode == Mode::Practice {
            rig_check::run_practice(Faults::default(), &mut op, &mut sink, data).0
        } else {
            let mut port = RealPort::default();
            for (path, state) in rig_check::recover::pending(&out).into_iter().take(1) {
                if let Ok(mut restore_sink) =
                    report::ReportFile::create(&out, Redactor::from_environment())
                {
                    rig_check::recover::offer_restore(
                        &mut port,
                        &mut op,
                        &mut restore_sink,
                        &path,
                        &state,
                    );
                }
            }
            let state_file = sink.path.with_extension("state");
            rig_check::run_with_state(&mut port, &mut op, &mut sink, data, state_file).0
        };
        let _ = op.to_ui.send(Prompt::Done {
            run: Box::new(run),
            report: sink.last.clone(),
            path: sink.path.display().to_string(),
        });
    };
    let mut wizard = Wizard::default();
    wizard.mode = options
        .practice
        .map(|p| if p { Mode::Practice } else { Mode::Real });
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 800.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Rig Check",
        native,
        Box::new(move |cc| {
            rig_check::gui::apply_theme(&cc.egui_ctx);
            Ok(Box::new(App {
                wizard,
                start: Some(Box::new(start)),
            }))
        }),
    );
    if let Err(error) = result {
        eprintln!("Cannot open the window: {error}. Use --text for the terminal version.");
        std::process::exit(1);
    }
}

fn main() {
    let options = match options() {
        Ok(options) => options,
        Err(usage) => {
            eprintln!("{usage}");
            std::process::exit(2);
        }
    };
    #[cfg(feature = "gui")]
    if !options.text {
        return window(options);
    }
    let redactor = Redactor::from_environment();
    let mut sink = match report::ReportFile::create(&options.out, redactor) {
        Ok(sink) => sink,
        Err(error) => {
            eprintln!("Cannot create the report: {error}");
            std::process::exit(1);
        }
    };
    let mut data = RunData {
        engine_folder: options.engine_folder.clone(),
        ..RunData::default()
    };
    if options.evidence {
        let stem = sink
            .path
            .file_stem()
            .map_or("rig-check".into(), |s| s.to_string_lossy().into_owned());
        data.evidence_dir = Some(options.out.join(format!("{stem}-evidence")));
    }
    let stdin = io::stdin();
    let mut person = Terminal::new(BufReader::new(stdin.lock()), io::stdout());
    let run = if options.practice == Some(true) {
        rig_check::run_practice(Faults::default(), &mut person, &mut sink, data).0
    } else {
        let mut port = RealPort::default();
        // An earlier run that did not finish: offer to put the rig back first.
        for (path, state) in rig_check::recover::pending(&options.out)
            .into_iter()
            .take(1)
        {
            let mut restore_sink =
                match report::ReportFile::create(&options.out, Redactor::from_environment()) {
                    Ok(sink) => sink,
                    Err(error) => {
                        eprintln!("Cannot create the restore report: {error}");
                        break;
                    }
                };
            rig_check::recover::offer_restore(
                &mut port,
                &mut person,
                &mut restore_sink,
                &path,
                &state,
            );
        }
        let state_file = sink.path.with_extension("state");
        rig_check::run_with_state(&mut port, &mut person, &mut sink, data, state_file).0
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
