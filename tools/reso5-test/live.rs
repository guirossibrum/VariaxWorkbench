mod full_read;
mod slot_read;

use super::procedure::selection::listen_list;
use super::procedure::{
    self, Stage, StorePolicy, VariaxPolicy, selection,
    store::{self, Mode, Prompt},
};
use line6_devices::{
    backup,
    midi::hex,
    session::{Link, Log, Path as DevicePath, Session},
    transport::{
        rawmidi::{PODXT_LIVE_CARD, RawMidiLink},
        usb::{UsbInterfaceLink, VARIAX_VIA_PODXT_LIVE},
    },
};
use std::{
    cell::RefCell,
    ffi::OsString,
    fs::{self, File},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use variax_workbench::hardware_gates::{Gate, run_if_unlocked};

const USAGE: &str =
    "Usage: variax-reso5-test --card | --store-card | --slot-read-card | --full-read-card | --any-selection-card | --listen-list-card
       variax-reso5-test [--check] stage-1|stage-2 BACKUP_DIR ARCHIVE.vxf MODELS.tsv LOG_ROOT
       variax-reso5-test [--check] store rename|restore|factory BACKUP_DIR ARCHIVE.vxf CAPTURE_DIR LOG_ROOT
       variax-reso5-test [--check] slot-read VARIAX_BACKUP PODXT_BACKUP ARCHIVE.vxf LOG_ROOT
       variax-reso5-test [--check] full-read ARCHIVE.vxf LOG_ROOT
       variax-reso5-test [--check] any-selection MODE FLASH_INDEX FULL_BACKUP ARCHIVE.vxf SOURCE MODELS.tsv LOG_ROOT
       variax-reso5-test [--check] listen-list FULL_BACKUP ARCHIVE.vxf MODELS.tsv LOG_ROOT INPUT.vwm [INPUT.vwm ...]";

static CANCELLED: AtomicBool = AtomicBool::new(false);

extern "C" fn cancel(_signal: libc::c_int) {
    CANCELLED.store(true, Ordering::Relaxed);
}

fn install_cancellation() -> Result<(), String> {
    // The handler only sets a lock-free flag. The main thread owns the link,
    // finishes cleanup and closes the log.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = cancel as *const () as libc::sighandler_t;
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(format!(
                    "Cannot install cancellation handler: {}",
                    io::Error::last_os_error()
                ));
            }
        }
    }
    Ok(())
}

pub fn run(args: &[OsString]) -> Result<(), String> {
    let (offline, args) = match args {
        [option, remaining @ ..] if option == "--check" => (true, remaining),
        _ => (false, args),
    };
    if let [stage, backup, archive, models, log, files @ ..] = args
        && stage == "listen-list"
    {
        let files: Vec<_> = files.iter().map(Path::new).collect();
        return run_listen_list(
            offline,
            [backup, archive, models, log].map(Path::new),
            &files,
        );
    }
    if let [stage, mode, slot, backup, archive, source, models, log] = args
        && stage == "any-selection"
    {
        let mode = mode
            .to_str()
            .and_then(selection::Mode::parse)
            .ok_or(USAGE)?;
        let slot = slot
            .to_str()
            .and_then(|text| text.parse::<u8>().ok())
            .ok_or(USAGE)?;
        return run_selection(
            offline,
            mode,
            slot,
            [backup, archive, source, models, log].map(Path::new),
        );
    }
    if let [stage, archive, log_root] = args
        && stage == "full-read"
    {
        return full_read::run(offline, Path::new(archive), Path::new(log_root));
    }
    if let [stage, variax_backup, podxt_backup, archive, log_root] = args
        && stage == "slot-read"
    {
        return slot_read::run(
            offline,
            Path::new(variax_backup),
            Path::new(podxt_backup),
            Path::new(archive),
            Path::new(log_root),
        );
    }
    if let [stage, mode, backup_dir, archive, capture, log_root] = args
        && stage == "store"
    {
        let mode = mode.to_str().and_then(Mode::parse).ok_or(USAGE)?;
        return run_store(
            offline,
            mode,
            Path::new(backup_dir),
            Path::new(archive),
            Path::new(capture),
            Path::new(log_root),
        );
    }
    let [stage, backup_dir, archive, models, log_root] = args else {
        return Err(USAGE.into());
    };
    let stage = match stage.to_str() {
        Some("stage-1") => Stage::Own,
        Some("stage-2") => Stage::Trim,
        _ => return Err(USAGE.into()),
    };
    let backup_dir = Path::new(backup_dir);
    let log_root = Path::new(log_root);
    let trace = Trace::create(log_root, &format!("stage-{}", stage.number()))?;
    let result = (|| {
        let (_, started) = now()?;
        trace.record(format!(
            "Reso-5 stage {} at {started}. Backup: {}",
            stage.number(),
            backup_dir.display()
        ));
        let prepared =
            procedure::prepare(stage, backup_dir, Path::new(archive), Path::new(models))?;
        trace.record(
            "Verified the matching backup, original program SHA-256 and provisional reverb bounds."
                .into(),
        );
        if offline {
            return Ok(());
        }

        print!("{}", super::CARD);
        let confirmation = format!("RUN RESO-5 STAGE {}", stage.number());
        print!("Confirm this stage on the physical Variax 600 by typing {confirmation}: ");
        io::stdout().flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if answer.trim_end_matches(['\r', '\n']) != confirmation {
            return Err("Stage confirmation was not given.".into());
        }
        install_cancellation()?;
        run_if_unlocked(Gate::EditBufferWrite, || {
            run_if_unlocked(Gate::CurrentSettingsRead, || {
                run_if_unlocked(Gate::IdentityRequest, || {
                    run_if_unlocked(Gate::OsDiscovery, || {
                        run_device(DeviceTask::Edit(&prepared), &trace)
                    })
                })
            })
        })
    })();
    trace.record(match &result {
        Err(error) => format!("STOPPED: {error}"),
        Ok(()) if offline => "PASS: offline preflight. No device was opened.".into(),
        Ok(()) => "PASS: settings read-back. Continue with the test card.".into(),
    });
    println!("Log folder: {}", trace.path.display());
    combine(result, trace.finish())?;
    if !offline {
        let trim = match stage {
            Stage::Own => "0.0",
            Stage::Trim => "−6.0",
        };
        println!("Verified Reso-5 settings. Output trim {trim} dB.");
    }
    Ok(())
}

// Each ladder step needs its own direct owner approval on the reviewed build.
const APPROVED_RENAME: bool = false;
const APPROVED_RESTORE: bool = false;
const APPROVED_FACTORY: bool = false;

fn run_store(
    offline: bool,
    mode: Mode,
    backup_dir: &Path,
    archive: &Path,
    capture: &Path,
    log_root: &Path,
) -> Result<(), String> {
    let approved = match mode {
        Mode::Rename => APPROVED_RENAME,
        Mode::Restore => APPROVED_RESTORE,
        Mode::Factory => APPROVED_FACTORY,
    };
    if !offline && !approved {
        return Err(format!(
            "The #177 {} step has no owner approval for a live run.",
            mode.label()
        ));
    }
    let trace = Trace::create(log_root, &format!("store-{}", mode.label()))?;
    let result = (|| {
        let (_, started) = now()?;
        trace.record(format!(
            "Reso-5 slot 50 {} at {started}. Backup: {}. Capture: {}",
            mode.label(),
            backup_dir.display(),
            capture.display()
        ));
        let (prepared, backup_settings) = store::prepare(mode, backup_dir, archive, capture)?;
        let verified = format!(
            "Verified the archive, pinned capture and matching backup. Expected stored-image SHA-256 ({}): {}",
            mode.label(),
            prepared.expected_sha256()
        );
        trace.record(verified.clone());
        println!("{verified}");
        if offline {
            return Ok(());
        }

        print!("{}", super::STORE_CARD);
        install_cancellation()?;
        run_if_unlocked(Gate::StoreRestore, || {
            run_if_unlocked(Gate::CurrentSettingsRead, || {
                run_if_unlocked(Gate::IdentityRequest, || {
                    run_if_unlocked(Gate::OsDiscovery, || {
                        run_device(DeviceTask::Store(&prepared, &backup_settings), &trace)
                    })
                })
            })
        })
    })();
    trace.record(match &result {
        Err(error) => format!("STOPPED: {error}"),
        Ok(()) if offline => "PASS: offline preflight. No device was opened.".into(),
        Ok(()) => "PASS: store confirmation, full reselected image and settings read-back.".into(),
    });
    println!("Log folder: {}", trace.path.display());
    combine(result, trace.finish())?;
    if !offline {
        println!(
            "Received 0x71 and verified the full reselected image and all 192 settings bytes."
        );
    }
    Ok(())
}

const APPROVED_LISTEN_LIST: bool = true;
const APPROVED_LISTEN_4A: bool = false;
const APPROVED_STORE_4B: bool = false;
const APPROVED_RESTORE_4C: bool = false;
const APPROVED_RENAME_3A: bool = false;
const APPROVED_FACTORY_3B: bool = false;

fn run_selection(
    offline: bool,
    mode: selection::Mode,
    slot: u8,
    paths: [&Path; 5],
) -> Result<(), String> {
    let approved = match mode {
        selection::Mode::Listen => APPROVED_LISTEN_4A,
        selection::Mode::Store => APPROVED_STORE_4B,
        selection::Mode::Restore => APPROVED_RESTORE_4C,
        selection::Mode::RenameReso => APPROVED_RENAME_3A,
        selection::Mode::FactoryReso => APPROVED_FACTORY_3B,
    };
    if !offline && !approved {
        return Err(format!(
            "The #238 {} rung has no owner approval for a live run.",
            mode.label()
        ));
    }
    let [backup, archive, source, models, log] = paths;
    let trace = Trace::create(log, &format!("any-selection-{}", mode.label()))?;
    let result = (|| {
        let prepared = selection::prepare(mode, slot, backup, archive, source, models)?;
        for line in prepared.description() {
            println!("{line}");
            trace.record(line.clone());
        }
        if offline {
            return Ok(());
        }
        print!("{}", super::SELECTION_CARD);
        let write_gate = if mode.listens() {
            Gate::EditBufferWrite
        } else {
            Gate::StoreRestore
        };
        run_selection_device(write_gate, DeviceTask::Selection(&prepared), &trace)
    })();
    trace.record(match &result {
        Err(error) => format!("STOPPED: {error}"),
        Ok(()) if offline => "PASS: offline check. No device was opened.".into(),
        Ok(()) => "PASS: read-back and original selection verified.".into(),
    });
    println!("Log folder: {}", trace.path.display());
    combine(result, trace.finish())
}

fn run_listen_list(offline: bool, paths: [&Path; 4], files: &[&Path]) -> Result<(), String> {
    if !offline && !APPROVED_LISTEN_LIST {
        return Err("The listen-list audition has no owner approval for a live run.".into());
    }
    let [backup, archive, models, log] = paths;
    let trace = Trace::create(log, "listen-list")?;
    let result = (|| {
        let prepared = listen_list::prepare(backup, archive, models, files)?;
        for line in prepared.description() {
            println!("{line}");
            trace.record(line);
        }
        if offline {
            return Ok(());
        }
        print!("{}", super::LISTEN_LIST_CARD);
        run_selection_device(
            Gate::EditBufferWrite,
            DeviceTask::ListenList(&prepared),
            &trace,
        )
    })();
    trace.record(match &result {
        Err(error) => format!("STOPPED: {error}"),
        Ok(()) if offline => "PASS: offline check. No device was opened.".into(),
        Ok(()) => {
            "PASS: every audition buffer and the return to the original were verified.".into()
        }
    });
    println!("Log folder: {}", trace.path.display());
    combine(result, trace.finish())
}

fn run_selection_device(
    write_gate: Gate,
    task: DeviceTask<'_>,
    trace: &Trace,
) -> Result<(), String> {
    println!("HEAR: Your starting preset stays selected while the tool checks the rig.");
    println!("SEE: Wait while the tool checks the PODxt Live. Nothing to type.");
    install_cancellation()?;
    run_if_unlocked(write_gate, || {
        run_if_unlocked(Gate::FullBackupRead, || {
            run_if_unlocked(Gate::CurrentSettingsRead, || {
                run_if_unlocked(Gate::IdentityRequest, || {
                    run_if_unlocked(Gate::OsDiscovery, || run_device(task, trace))
                })
            })
        })
    })
}

enum DeviceTask<'a> {
    FullRead(&'a [u8; variax_workbench::factory_reference::PROGRAM_BYTES]),
    Edit(&'a procedure::Prepared),
    Store(&'a store::Prepared, &'a [u8; 192]),
    Selection(&'a selection::Prepared),
    ListenList(&'a listen_list::Prepared),
}

fn prompt_store(prompt: Prompt, stopping: impl Fn() -> bool) -> Result<(), String> {
    let phrase = match prompt {
        Prompt::Confirm(mode) => mode.confirmation(),
        Prompt::Reselect => {
            println!(
                "0x71 received. Select another preset, then return to Reso, position 5 by hand."
            );
            store::RESELECT_CONFIRMATION
        }
    };
    super::confirmation::confirm(phrase, stopping)?;
    store::require_confirmation(prompt, phrase)
}

fn run_device(task: DeviceTask<'_>, trace: &Trace) -> Result<(), String> {
    let stopping = || CANCELLED.load(Ordering::Relaxed) || trace.failed();
    if stopping() {
        return Err("Cancelled or unable to write the log before device preflight.".into());
    }
    let _rig =
        line6_devices::device_lock::DeviceLock::acquire().map_err(|error| error.to_string())?;
    {
        let podxt = RawMidiLink::open(PODXT_LIVE_CARD, DevicePath::PodxtMidi)?;
        let mut log = trace;
        procedure::check_podxt(
            TracedLink {
                inner: podxt,
                trace,
                label: "PODxt",
            },
            &mut log,
        )?;
    }
    if matches!(&task, DeviceTask::Selection(_) | DeviceTask::ListenList(_)) {
        println!("PODxt Live verified.");
    }
    if stopping() {
        return Err(
            "Cancelled or unable to write the log before opening the Variax interface.".into(),
        );
    }
    let mut variax = UsbInterfaceLink::open(VARIAX_VIA_PODXT_LIVE)?;
    let mut log = trace;
    let result = {
        let link = TracedLink {
            inner: &mut variax,
            trace,
            label: "Variax",
        };
        match task {
            DeviceTask::FullRead(factory) => {
                procedure::full_read::capture(link, &mut log, &trace.path, stopping).map(
                    |captured| {
                        for line in procedure::full_read::summarize(factory, &captured) {
                            println!("{line}");
                            trace.record(line);
                        }
                    },
                )
            }
            DeviceTask::Edit(prepared) => {
                let mut session = Session::new(link, &VariaxPolicy, &mut log);
                procedure::attempt(&mut session, prepared, stopping)
            }
            DeviceTask::Selection(prepared) => {
                let policy = selection::Policy(prepared.mode);
                let mut session = Session::new(link, &policy, &mut log);
                selection::attempt(
                    &mut session,
                    prepared,
                    &trace.path,
                    |phrase| super::confirmation::confirm(phrase, stopping),
                    |line| {
                        println!("{line}");
                        trace.record(line.to_owned());
                    },
                    stopping,
                )
            }
            DeviceTask::ListenList(prepared) => {
                let policy = selection::Policy(selection::Mode::Listen);
                let mut session = Session::new(link, &policy, &mut log);
                listen_list::attempt(
                    &mut session,
                    prepared,
                    &trace.path,
                    || super::confirmation::confirm("", stopping),
                    |line| {
                        println!("{line}");
                        trace.record(line.to_owned());
                    },
                    stopping,
                )
            }
            DeviceTask::Store(prepared, backup_settings) => {
                let mut session = Session::new(link, &StorePolicy, &mut log);
                store::attempt(
                    &mut session,
                    prepared,
                    backup_settings,
                    &trace.path,
                    |prompt| {
                        prompt_store(prompt, || {
                            matches!(prompt, Prompt::Confirm(_)) && stopping()
                        })
                    },
                    |notice| eprintln!("{notice}"),
                    stopping,
                )
            }
        }
    };
    let cleanup = variax.finish();
    trace.record(match &cleanup {
        Ok(()) => "Variax USB interface released to its driver.".into(),
        Err(error) => format!("Variax USB interface cleanup failed: {error}"),
    });
    combine(result, cleanup)
}

fn combine(result: Result<(), String>, cleanup: Result<(), String>) -> Result<(), String> {
    match (result, cleanup) {
        (Ok(()), other) | (other, Ok(())) => other,
        (Err(error), Err(cleanup)) => Err(format!("{error} Also: {cleanup}")),
    }
}

fn now() -> Result<(String, String), String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| backup::utc(time.as_secs()))
        .map_err(|error| error.to_string())
}

struct Trace {
    path: std::path::PathBuf,
    file: RefCell<Result<File, io::Error>>,
}

impl Trace {
    fn create(root: &Path, label: &str) -> Result<Self, String> {
        let (stamp, _) = now()?;
        fs::create_dir_all(root).map_err(|error| format!("{}: {error}", root.display()))?;
        let path = tempfile::Builder::new()
            .prefix(&format!("reso5-{label}-{stamp}-"))
            .tempdir_in(root)
            .map_err(|error| format!("{}: {error}", root.display()))?
            .keep();
        let file = File::create_new(path.join("log.txt"))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Self {
            path,
            file: RefCell::new(Ok(file)),
        })
    }

    fn failed(&self) -> bool {
        self.file.borrow().is_err()
    }

    fn record(&self, text: String) {
        let mut file = self.file.borrow_mut();
        if let Ok(destination) = &mut *file
            && let Err(error) = writeln!(destination, "{text}")
        {
            *file = Err(error);
        }
    }

    fn finish(self) -> Result<(), String> {
        self.file
            .into_inner()
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("Capture log is incomplete: {error}"))
    }
}

impl Log for &Trace {
    fn line(&mut self, text: String) {
        self.record(text);
    }
}

struct TracedLink<'a, L> {
    inner: L,
    trace: &'a Trace,
    label: &'static str,
}

impl<L: Link> Link for TracedLink<'_, L> {
    fn path(&self) -> DevicePath {
        self.inner.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.trace.record(format!(
            "{:8.3}s {} OUT transport {} bytes {}",
            self.elapsed().as_secs_f64(),
            self.label,
            bytes.len(),
            hex(bytes)
        ));
        let result = self.inner.write(bytes);
        if let Err(error) = &result {
            self.trace
                .record(format!("{} transport write error: {error}", self.label));
        }
        result
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        let result = self.inner.read(wait);
        if let Ok(Some(bytes)) = &result {
            self.trace.record(format!(
                "{:8.3}s {} IN transport {}",
                self.elapsed().as_secs_f64(),
                self.label,
                hex(bytes)
            ));
        }
        result
    }

    fn elapsed(&self) -> Duration {
        self.inner.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.inner.pause(duration);
    }
}
