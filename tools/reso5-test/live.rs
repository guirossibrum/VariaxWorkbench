mod slot_read;

use super::procedure::{self, Stage, StorePolicy, VariaxPolicy};
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

const USAGE: &str = "Usage: variax-reso5-test --card | --store-card | --slot-read-card
       variax-reso5-test [--check] stage-1|stage-2 BACKUP_DIR ARCHIVE.vxf MODELS.tsv LOG_ROOT
       variax-reso5-test [--check] store BACKUP_DIR ARCHIVE.vxf LOG_ROOT
       variax-reso5-test [--check] slot-read VARIAX_BACKUP PODXT_BACKUP ARCHIVE.vxf LOG_ROOT";

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
    if let [stage, backup_dir, archive, log_root] = args
        && stage == "store"
    {
        return run_store(
            offline,
            Path::new(backup_dir),
            Path::new(archive),
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
        let today = &started[..10];
        let prepared = procedure::prepare(
            stage,
            backup_dir,
            today,
            Path::new(archive),
            Path::new(models),
        )?;
        trace.record(
            "Verified the same-day backup, original program SHA-256 and provisional reverb bounds."
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
        if &now()?.1[..10] != today {
            return Err("The UTC day changed during preflight. Select a current backup.".into());
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

fn run_store(
    offline: bool,
    backup_dir: &Path,
    archive: &Path,
    log_root: &Path,
) -> Result<(), String> {
    let trace = Trace::create(log_root, "store")?;
    let result = (|| {
        let (_, started) = now()?;
        let today = &started[..10];
        trace.record(format!(
            "Reso-5 slot 50 store at {started}. Backup: {}",
            backup_dir.display()
        ));
        let prepared = procedure::prepare_store(backup_dir, today, archive)?;
        let verified = "Verified the archived slot 50 program fingerprint, name and checksum, and the same-day complete backup.";
        trace.record(verified.into());
        if offline {
            println!("{verified} Offline check passed.");
            return Ok(());
        }

        print!("{}", super::STORE_CARD);
        install_cancellation()?;
        run_if_unlocked(Gate::StoreRestore, || {
            run_if_unlocked(Gate::CurrentSettingsRead, || {
                run_if_unlocked(Gate::IdentityRequest, || {
                    run_if_unlocked(Gate::OsDiscovery, || {
                        run_device(DeviceTask::Store(&prepared, today), &trace)
                    })
                })
            })
        })
    })();
    trace.record(match &result {
        Err(error) => format!("STOPPED: {error}"),
        Ok(()) if offline => "PASS: offline preflight. No device was opened.".into(),
        Ok(()) => {
            "PASS: store confirmation and settings read-back. Continue with the test card.".into()
        }
    });
    println!("Log folder: {}", trace.path.display());
    combine(result, trace.finish())?;
    if !offline {
        println!(
            "Received 0x71 and verified all 192 Reso-5 settings bytes. Continue with the test card."
        );
    }
    Ok(())
}

enum DeviceTask<'a> {
    Edit(&'a procedure::Prepared),
    Store(&'a procedure::PreparedStore, &'a str),
}

fn prompt_store(today: &str, stopping: impl Fn() -> bool) -> Result<(), String> {
    print!("Type {} and press Enter: ", procedure::STORE_CONFIRMATION);
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut answer = Vec::new();
    let mut stdin = libc::pollfd {
        fd: libc::STDIN_FILENO,
        events: libc::POLLIN,
        revents: 0,
    };
    // Poll stdin so Ctrl+C can release the already-open interface even when
    // no confirmation line has been entered.
    while !stopping() {
        let ready = unsafe { libc::poll(&mut stdin, 1, 100) };
        if ready == 0 {
            continue;
        }
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("Cannot read slot confirmation: {error}"));
        }
        let mut byte = 0u8;
        let count = unsafe { libc::read(stdin.fd, (&mut byte as *mut u8).cast(), 1) };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("Cannot read slot confirmation: {error}"));
        }
        if count == 0 || answer.len() > procedure::STORE_CONFIRMATION.len() + 1 {
            break;
        }
        answer.push(byte);
        if byte == b'\n' {
            let answer = std::str::from_utf8(&answer).map_err(|error| error.to_string())?;
            procedure::confirm_store(answer)?;
            if &now()?.1[..10] != today {
                return Err(
                    "The UTC day changed during preflight. Select a current backup.".into(),
                );
            }
            return Ok(());
        }
    }
    Err("Slot 50 store confirmation was not given.".into())
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
            DeviceTask::Edit(prepared) => {
                let mut session = Session::new(link, &VariaxPolicy, &mut log);
                procedure::attempt(&mut session, prepared, stopping)
            }
            DeviceTask::Store(prepared, today) => {
                let mut session = Session::new(link, &StorePolicy, &mut log);
                procedure::attempt_store(
                    &mut session,
                    prepared,
                    || prompt_store(today, stopping),
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
