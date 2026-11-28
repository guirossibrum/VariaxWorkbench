use super::{DEFAULT_LEVEL, Endpoint, Event, Record, Severity, unix_ms};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};

use serde::Serialize;

const QUEUE_CAPACITY: usize = 1024;
const FILE_LIMIT: usize = 4 * 1024 * 1024;
const FAILURE_RESERVE: usize = 64 * 1024;
const RETAINED_SESSIONS: usize = 8;
const LOG_PREFIX: &str = "workbench-session-";
const LOG_SUFFIX: &str = ".jsonl";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinkState {
    Starting,
    Recording,
    SizeLimit,
    Unavailable,
    Closed,
}

#[derive(Clone, Debug)]
pub struct LogStatus {
    pub level: Severity,
    pub folder: Option<PathBuf>,
    pub path: Option<PathBuf>,
    pub state: SinkState,
}

enum Message {
    Record(Record),
    Export(PathBuf, SyncSender<io::Result<()>>),
    Shutdown,
}

pub type Export = Receiver<io::Result<()>>;

struct Inner {
    level: Severity,
    status: Mutex<LogStatus>,
    sender: SyncSender<Message>,
    dropped: AtomicU64,
    accepting: AtomicBool,
}

#[derive(Clone)]
pub struct SessionLog(Arc<Inner>);

#[must_use = "the guard keeps the log worker running for this session"]
pub struct SessionGuard {
    log: SessionLog,
    worker: Option<JoinHandle<()>>,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.log.0.accepting.store(false, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = self.log.0.sender.send(Message::Shutdown);
            let _ = worker.join();
        }
    }
}

static CURRENT: OnceLock<SessionLog> = OnceLock::new();

pub fn current() -> Option<&'static SessionLog> {
    CURRENT.get()
}

pub fn status() -> Option<LogStatus> {
    current().map(SessionLog::status)
}

/// Queue export of the Workbench session to a user-selected path.
pub fn export_current(destination: PathBuf) -> io::Result<Export> {
    current()
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))?
        .export_current(destination)
}

/// Starts logging for a single GUI process. File creation is asynchronous.
pub fn start(data_root: Option<PathBuf>) -> Option<SessionGuard> {
    if current().is_some() {
        return None;
    }
    let folder = data_root.map(|root| root.join("logs"));
    let (log, guard) = SessionLog::open(folder, DEFAULT_LEVEL);
    CURRENT.set(log).ok()?;
    Some(guard)
}

impl SessionLog {
    /// Construct an independent sink in an explicitly supplied log folder.
    /// The desktop uses [start] to select and publish its process-wide sink.
    pub fn open(folder: Option<PathBuf>, level: Severity) -> (Self, SessionGuard) {
        let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
        let log = Self(Arc::new(Inner {
            level,
            status: Mutex::new(LogStatus {
                level,
                folder: folder.clone(),
                path: None,
                state: SinkState::Starting,
            }),
            sender,
            dropped: AtomicU64::new(0),
            accepting: AtomicBool::new(true),
        }));
        let mut guard = SessionGuard {
            log: log.clone(),
            worker: None,
        };
        if let Some(folder) = folder {
            let worker_log = log.clone();
            guard.worker = thread::Builder::new()
                .name("workbench-session-log".into())
                .spawn(move || worker_log.run(&folder, receiver))
                .ok();
        }
        if guard.worker.is_none() {
            log.set_unavailable();
        }
        (log, guard)
    }

    pub fn status(&self) -> LogStatus {
        self.0.status.lock().unwrap().clone()
    }

    pub(crate) fn accepting(&self) -> bool {
        self.0.accepting.load(Ordering::Relaxed)
    }

    /// Queue a point-in-time copy to a user-chosen destination. The worker
    /// serializes it after earlier events. Existing files are preserved.
    /// The UI can poll the returned receiver with `try_recv`.
    pub fn export_current(&self, destination: PathBuf) -> io::Result<Export> {
        let (reply, response) = mpsc::sync_channel(1);
        self.0
            .sender
            .try_send(Message::Export(destination, reply))
            .map_err(|error| {
                let kind = match error {
                    TrySendError::Full(_) => io::ErrorKind::WouldBlock,
                    TrySendError::Disconnected(_) => io::ErrorKind::NotConnected,
                };
                io::Error::from(kind)
            })?;
        Ok(response)
    }

    pub fn gui_startup_failed(&self) {
        self.record(
            Severity::Critical,
            Endpoint::Workbench,
            None,
            Event::GuiStartupFailed,
        );
    }

    pub(super) fn record(
        &self,
        severity: Severity,
        endpoint: Endpoint,
        operation: Option<u64>,
        event: Event,
    ) {
        if severity < self.0.level || !self.accepting() {
            return;
        }
        let record = Record {
            unix_ms: unix_ms(),
            severity,
            endpoint,
            operation,
            event,
        };
        if let Err(TrySendError::Full(_)) = self.0.sender.try_send(Message::Record(record)) {
            self.0.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn set_unavailable(&self) {
        self.0.accepting.store(false, Ordering::Relaxed);
        self.0.status.lock().unwrap().state = SinkState::Unavailable;
    }

    fn run(&self, folder: &Path, receiver: Receiver<Message>) {
        let Ok(mut writer) = Writer::create(folder) else {
            self.set_unavailable();
            return;
        };
        {
            let mut status = self.0.status.lock().unwrap();
            status.path = Some(writer.path.clone());
            status.state = SinkState::Recording;
        }
        prune(folder, &writer.path);
        if self
            .write_milestone(&mut writer, Event::SessionStarted)
            .is_err()
        {
            self.set_unavailable();
            return;
        }
        while let Ok(message) = receiver.recv() {
            let dropped = self.0.dropped.swap(0, Ordering::Relaxed);
            if dropped > 0
                && writer
                    .write(system_record(
                        Severity::Warn,
                        Event::QueueOverflow { dropped },
                    ))
                    .is_err()
            {
                self.set_unavailable();
                return;
            }
            match message {
                Message::Record(record) => {
                    if writer.write(record).is_err() {
                        self.set_unavailable();
                        return;
                    }
                    if writer.limited {
                        self.0.status.lock().unwrap().state = SinkState::SizeLimit;
                    }
                }
                Message::Export(destination, reply) => {
                    let _ = reply.send(writer.export(&destination));
                }
                Message::Shutdown => {
                    let result = self.write_milestone(&mut writer, Event::SessionFinished);
                    if result.is_err() {
                        self.set_unavailable();
                    } else {
                        prune(folder, &writer.path);
                        self.0.status.lock().unwrap().state = SinkState::Closed;
                    }
                    return;
                }
            }
        }
    }

    fn write_milestone(&self, writer: &mut Writer, event: Event) -> io::Result<()> {
        if self.0.level <= Severity::Info {
            writer.write(system_record(Severity::Info, event))?;
        }
        Ok(())
    }
}

fn system_record(severity: Severity, event: Event) -> Record {
    Record {
        unix_ms: unix_ms(),
        severity,
        endpoint: Endpoint::Workbench,
        operation: None,
        event,
    }
}

struct Writer {
    file: File,
    path: PathBuf,
    buffer: Vec<u8>,
    bytes: usize,
    limited: bool,
}

impl Writer {
    fn create(folder: &Path) -> io::Result<Self> {
        fs::create_dir_all(folder)?;
        let file = tempfile::Builder::new()
            .prefix(&format!("{LOG_PREFIX}{:020}-", unix_ms()))
            .suffix(LOG_SUFFIX)
            .tempfile_in(folder)?;
        file.as_file().try_lock()?;
        let (file, path) = file.keep().map_err(|error| error.error)?;
        Ok(Self {
            file,
            path,
            buffer: Vec::with_capacity(512),
            bytes: 0,
            limited: false,
        })
    }

    fn write(&mut self, record: Record) -> io::Result<()> {
        if self.limited && record.severity < Severity::Warn {
            return Ok(());
        }
        self.encode(record)?;
        let limit = if record.severity >= Severity::Warn {
            FILE_LIMIT
        } else {
            FILE_LIMIT - FAILURE_RESERVE
        };
        if self.bytes + self.buffer.len() <= limit {
            self.append()?;
        } else if !self.limited {
            self.limited = true;
            self.encode(system_record(Severity::Warn, Event::SizeLimit))?;
            if self.bytes + self.buffer.len() <= FILE_LIMIT {
                self.append()?;
            }
        }
        Ok(())
    }

    fn encode(&mut self, record: Record) -> io::Result<()> {
        #[derive(Serialize)]
        struct Entry {
            schema: u8,
            version: &'static str,
            build: &'static str,
            revision: &'static str,
            #[serde(flatten)]
            record: Record,
        }
        let build = crate::BUILD_NUMBER;
        let revision = env!("VARIAX_LOG_REVISION");
        self.buffer.clear();
        serde_json::to_writer(
            &mut self.buffer,
            &Entry {
                schema: 1,
                version: env!("CARGO_PKG_VERSION"),
                build: if !build.is_empty()
                    && build.len() <= 16
                    && build.bytes().all(|byte| byte.is_ascii_digit())
                {
                    build
                } else {
                    "dev"
                },
                revision,
                record,
            },
        )?;
        self.buffer.push(b'\n');
        Ok(())
    }

    fn append(&mut self) -> io::Result<()> {
        if let Err(error) = self.file.write_all(&self.buffer) {
            let _ = self.file.set_len(self.bytes as u64);
            return Err(error);
        }
        self.bytes += self.buffer.len();
        Ok(())
    }

    fn export(&mut self, destination: &Path) -> io::Result<()> {
        let parent = destination
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut copy = tempfile::NamedTempFile::new_in(parent)?;
        self.file.seek(SeekFrom::Start(0))?;
        let copied = io::copy(
            &mut (&mut self.file).take(self.bytes as u64),
            copy.as_file_mut(),
        );
        self.file.seek(SeekFrom::Start(self.bytes as u64))?;
        copied?;
        copy.as_file().sync_all()?;
        copy.persist_noclobber(destination)
            .map_err(|error| error.error)?;
        Ok(())
    }
}

fn prune(folder: &Path, current: &Path) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    let mut sessions: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let stamp_and_id = name
                .to_str()?
                .strip_prefix(LOG_PREFIX)?
                .strip_suffix(LOG_SUFFIX)?;
            let (stamp, id) = stamp_and_id.split_once('-')?;
            (stamp.len() == 20
                && stamp.bytes().all(|byte| byte.is_ascii_digit())
                && id.len() == 6
                && id.bytes().all(|byte| byte.is_ascii_alphanumeric())
                && entry.file_type().ok()?.is_file())
            .then(|| entry.path())
        })
        .collect();
    sessions.sort_unstable();
    let mut excess = sessions.len().saturating_sub(RETAINED_SESSIONS);
    // Active writers keep their own files locked. If more than eight
    // sessions are running, the cap is re-applied as active sessions close.
    for old in sessions {
        if excess == 0 {
            break;
        }
        if old != current
            && let Ok(file) = File::options().read(true).write(true).open(&old)
            && file.try_lock().is_ok()
            && fs::remove_file(&old).is_ok()
        {
            excess -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_bound_reserves_space_for_severe_events() {
        let directory = tempfile::tempdir().unwrap();
        let mut writer = Writer::create(directory.path()).unwrap();
        let info = system_record(Severity::Info, Event::SessionStarted);
        for _ in 0..50_000 {
            writer.write(info).unwrap();
        }
        assert!(writer.limited);
        let high = system_record(Severity::Critical, Event::GuiStartupFailed);
        writer.write(high).unwrap();
        let first_contents = fs::read_to_string(&writer.path).unwrap();
        assert_eq!(
            first_contents
                .matches("\"category\":\"size_limit\"")
                .count(),
            1
        );
        assert!(first_contents.ends_with("\"category\":\"gui_startup_failed\"}\n"));
        for _ in 0..1000 {
            writer.write(high).unwrap();
        }
        let bytes = fs::read(&writer.path).unwrap();
        assert!(bytes.len() <= FILE_LIMIT);
        assert_eq!(bytes.last(), Some(&b'\n'));
    }

    #[test]
    fn retention_preserves_an_active_session_and_unrelated_files() {
        let directory = tempfile::tempdir().unwrap();
        let old = directory
            .path()
            .join(format!("{LOG_PREFIX}{:020}-abcdef{LOG_SUFFIX}", 1));
        let active = File::create(&old).unwrap();
        active.lock().unwrap();
        for stamp in 2..=16 {
            let retired = directory
                .path()
                .join(format!("{LOG_PREFIX}{stamp:020}-abcdef{LOG_SUFFIX}"));
            File::create(retired).unwrap();
        }
        let other = directory.path().join("backup.jsonl");
        fs::write(&other, "retained").unwrap();
        let (log, guard) = SessionLog::open(Some(directory.path().into()), Severity::Trace);
        drop(guard);
        let current = log.status().path.unwrap();
        assert!(old.exists());
        assert!(current.exists());
        assert_eq!(fs::read_to_string(other).unwrap(), "retained");
        assert_eq!(
            fs::read_dir(directory.path()).unwrap().count(),
            RETAINED_SESSIONS + 1
        );
    }
}
