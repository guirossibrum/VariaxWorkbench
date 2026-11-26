//! `rig-check`: the Rig Check app (#243).
//!
//! `rig-check --practice` rehearses every step against simulated devices.
//! `rig-check --real` uses the connected rig under the released step approvals.
//! The person confirms each step.

use std::io;
use std::path::PathBuf;
use variax_workbench::rig_check::{
    self, cli::Terminal, port::RealPort, practice::Faults, redact::Redactor, report, steps::RunData,
};

const USAGE: &str = "Usage: rig-check [--practice | --real] [--text] [--out DIR] [--evidence] [--engine-folder DIR]
       rig-check --catalogue
       rig-check --compare A.wav B.wav [BASE1.wav BASE2.wav] [--reports A.txt B.txt]
                 (offline: compare two clips; two takes from one computer as the baseline)
       rig-check --mockups DIR   (draw the wizard screens to PNG files, headless)
  Without --text, a build with the gui feature opens the wizard window.
  --practice  rehearse every step with simulated devices (nothing is sent to real hardware)
  --real      use the connected PODxt Live and Variax (device steps need the owner's approval)
  --out DIR   folder for the report (default: rig-check-output next to this program)
  --evidence  save raw evidence in practice (always saved for real runs)
  --engine-folder DIR  use local Variax models and reference programs instead of the bundle (models.tsv, NAME.vwm, NAME.packed or NAME.sha256)";

struct Options {
    /// `None`: the person chooses in the window.
    practice: Option<bool>,
    #[cfg_attr(not(feature = "gui"), allow(dead_code))]
    text: bool,
    out: PathBuf,
    evidence: bool,
    engine_folder: Option<PathBuf>,
}

fn compare_clips(args: &[String]) -> Result<(), String> {
    use rig_check::compare;
    let (clips, reports) = match args.iter().position(|a| a == "--reports") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (args, &args[args.len()..]),
    };
    let read = |path: &String| std::fs::read(path).map_err(|e| format!("{path}: {e}"));
    let analyse =
        |path: &String| compare::analyze_wav(&read(path)?).map_err(|e| format!("{path}: {e}"));
    let (a, b) = match clips {
        [a, b] | [a, b, _, _] => (analyse(a)?, analyse(b)?),
        _ => return Err(USAGE.into()),
    };
    let baseline = match clips {
        [_, _, x, y] => Some((analyse(x)?, analyse(y)?)),
        _ => None,
    };
    let engine = |path: Option<&String>| -> Result<Vec<String>, String> {
        path.map_or(Ok(Vec::new()), |p| {
            Ok(compare::engine_lines(&String::from_utf8_lossy(&read(p)?)))
        })
    };
    let result = compare::compare(
        &a,
        &b,
        baseline.as_ref().map(|(x, y)| (x, y)),
        &engine(reports.first())?,
        &engine(reports.get(1))?,
    );
    for line in result.lines {
        println!("{line}");
    }
    println!(
        "{}",
        if result.exceeded.is_empty() {
            "No difference exceeds the tolerance.".to_string()
        } else {
            format!("Exceeds the tolerance: {}.", result.exceeded.join(", "))
        }
    );
    Ok(())
}

fn default_out() -> PathBuf {
    if cfg!(target_os = "windows") {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(std::env::temp_dir)
            .join("Rig Check")
            .join("reports");
    }
    if cfg!(target_os = "macos") {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support/Rig Check/reports"))
            .unwrap_or_else(|| std::env::temp_dir().join("rig-check-output"));
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("rig-check-output")))
        .unwrap_or_else(|| PathBuf::from("rig-check-output"))
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args()
        .skip(1)
        .filter(|arg| !cfg!(target_os = "macos") || !arg.starts_with("-psn_"));
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
            "--compare" => {
                let rest: Vec<String> = args.by_ref().collect();
                std::process::exit(match compare_clips(&rest) {
                    Ok(()) => 0,
                    Err(error) => {
                        eprintln!("{error}");
                        1
                    }
                });
            }
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
                op.send(Prompt::Progress(rig_check::runner::Progress::action(
                    format!("Cannot create the report: {error}"),
                )));
                return;
            }
        };
        let mut data = RunData {
            engine_folder,
            ..RunData::default()
        };
        if evidence || mode == Mode::Real {
            let stem = sink
                .path
                .file_stem()
                .map_or("rig-check".into(), |s| s.to_string_lossy().into_owned());
            data.evidence_dir = Some(out.join(format!("{stem}-evidence")));
        }
        let run = if mode == Mode::Practice {
            rig_check::run_practice(Faults::default(), &mut op, &mut sink, data).0
        } else if variax_workbench::offline_device_notice().is_some() {
            rig_check::run_on(&mut RealPort::default(), &mut op, &mut sink, data).0
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
        op.send(Prompt::Done {
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
        #[cfg(target_os = "macos")]
        renderer: eframe::Renderer::Glow,
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
    if options.evidence || options.practice != Some(true) {
        let stem = sink
            .path
            .file_stem()
            .map_or("rig-check".into(), |s| s.to_string_lossy().into_owned());
        data.evidence_dir = Some(options.out.join(format!("{stem}-evidence")));
    }
    let mut person = Terminal::from_stdin(io::stdout());
    let run = if options.practice == Some(true) {
        rig_check::run_practice(Faults::default(), &mut person, &mut sink, data).0
    } else if variax_workbench::offline_device_notice().is_some() {
        rig_check::run_on(&mut RealPort::default(), &mut person, &mut sink, data).0
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
    println!("{}", run.rig_description);
    for difference in run.report_differences() {
        println!("{difference}");
    }
    for mismatch in run.look_mismatches() {
        println!("{mismatch}");
    }
    println!(
        "\nRig Check finished: {} passed, {} failed, {} skipped. {}\nWorking report: {}",
        run.count("passed"),
        run.count("failed"),
        run.count("skipped")
            + run.count("skipped_verified_firmware")
            + run.count("skipped_not_applicable")
            + run.count("skipped_needs"),
        rig_check::record::plain_result(&run.rig_state),
        sink.path.display()
    );
    let target = person.prompt_for_report_copy();
    if !target.is_empty() {
        match report::save_copy(&sink.last, std::path::Path::new(&target)) {
            Ok(()) => println!("Saved {target}."),
            Err(error) => println!("Not saved: {error}"),
        }
    }
}
