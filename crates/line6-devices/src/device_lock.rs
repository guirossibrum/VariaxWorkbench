//! Cooperative exclusion for the PODxt Live and its guitar interface.
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
    Busy,
    BusyHere,
    Unavailable(String),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy | Self::BusyHere => {
                f.write_str("PODxt Live is busy with another operation.")
            }
            Self::Unavailable(error) => write!(f, "Cannot lock the PODxt Live: {error}."),
        }
    }
}

const LOCK_NAME: &str = "variax-workbench-podxt-live.lock";
static ACTIVE_HERE: AtomicBool = AtomicBool::new(false);
static PASSIVE_HERE: AtomicBool = AtomicBool::new(false);

struct LocalLease(&'static AtomicBool);

impl LocalLease {
    fn acquire(held: &'static AtomicBool) -> Result<Self, LockError> {
        held.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self(held))
            .map_err(|_| LockError::BusyHere)
    }
}

impl Drop for LocalLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub struct DeviceLock {
    // The OS releases the lock when the last clone of this handle closes.
    _file: File,
    _active: Option<File>,
    _local: Option<LocalLease>,
}

impl DeviceLock {
    pub fn acquire() -> Result<Self, LockError> {
        acquire_tracked(&lock_path(std::env::var_os("XDG_RUNTIME_DIR")))
    }

    #[cfg(all(test, feature = "usb", target_os = "linux"))]
    pub(crate) fn acquire_test_lock(path: &Path) -> Result<Self, LockError> {
        acquire_at(path)
    }

    /// Pass the same locked file description to a multi-stage owner procedure.
    #[cfg(unix)]
    pub fn child_stdin(&self) -> std::io::Result<std::process::Stdio> {
        self._file.try_clone().map(std::process::Stdio::from)
    }

    /// A child command may reuse the lock only through the inherited descriptor.
    #[cfg(unix)]
    pub fn verify_inherited_stdin() -> Result<(), LockError> {
        verify(
            std::io::stdin(),
            &lock_path(std::env::var_os("XDG_RUNTIME_DIR")),
        )
    }
}

fn lock_path(runtime: Option<std::ffi::OsString>) -> PathBuf {
    runtime
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(fallback_directory)
        .join(LOCK_NAME)
}

fn fallback_directory() -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from("/tmp")
    }
    #[cfg(not(unix))]
    {
        std::env::temp_dir()
    }
}

fn unavailable(error: impl fmt::Display) -> LockError {
    LockError::Unavailable(error.to_string())
}

fn open_lock_file(path: &Path) -> Result<File, LockError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .mode(0o644);
    }
    // Windows needs write access to take an exclusive file lock.
    #[cfg(not(unix))]
    options.write(true);

    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let result = options.clone().write(true).create_new(true).open(path);
            match result {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    options.open(path).map_err(unavailable)?
                }
                Err(error) => return Err(unavailable(error)),
            }
        }
        Err(error) => return Err(unavailable(error)),
    };
    if !file.metadata().map_err(unavailable)?.is_file() {
        return Err(unavailable("lock path is not a regular file"));
    }
    Ok(file)
}

fn lock(file: &File) -> Result<(), LockError> {
    file.try_lock().map_err(|error| match error {
        TryLockError::WouldBlock => LockError::Busy,
        TryLockError::Error(error) => unavailable(error),
    })
}

// Active work signals intent before competing for the original rig lock.
// Passive listeners yield within a read poll and cannot reacquire over it.
fn active_file(path: &Path) -> Result<File, LockError> {
    open_lock_file(&path.with_extension("active.lock"))
}

fn acquire_tracked(path: &Path) -> Result<DeviceLock, LockError> {
    let local = LocalLease::acquire(&ACTIVE_HERE)?;
    let passive_was_local = PASSIVE_HERE.load(Ordering::Acquire);
    let mut guard =
        acquire_at_with_handoff(path, Some(&FOLLOW_HANDOFF)).map_err(|error| match error {
            LockError::Busy
                if passive_was_local
                    || PASSIVE_HERE.load(Ordering::Acquire)
                    || FOLLOW_HANDOFF.held.load(Ordering::Acquire) =>
            {
                LockError::BusyHere
            }
            other => other,
        })?;
    guard._local = Some(local);
    Ok(guard)
}

#[cfg(all(test, target_os = "linux"))]
fn acquire_at(path: &Path) -> Result<DeviceLock, LockError> {
    acquire_at_with_handoff(path, None)
}

fn acquire_at_with_handoff(
    path: &Path,
    handoff: Option<&FollowHandoff>,
) -> Result<DeviceLock, LockError> {
    let failures_before = handoff.map(|state| state.failures.load(Ordering::Acquire));
    let active = active_file(path)?;
    // The monitor may be in its brief try/unlock check when active work arrives.
    match lock(&active) {
        Err(LockError::Busy) => {
            std::thread::sleep(Duration::from_millis(4));
            lock(&active)?;
        }
        result => result?,
    }
    let file = open_lock_file(path)?;
    // Follow may be finishing an identity exchange or returning the driver.
    let wait = if handoff.is_some_and(|state| state.held.load(Ordering::Acquire)) {
        Duration::from_secs(10)
    } else {
        Duration::from_millis(100)
    };
    let deadline = Instant::now() + wait;
    loop {
        match lock(&file) {
            Ok(()) => {
                if handoff.map(|state| state.failures.load(Ordering::Acquire)) != failures_before {
                    return Err(unavailable(
                        "Variax Follow could not cleanly release the rig",
                    ));
                }
                return Ok(DeviceLock {
                    _file: file,
                    _active: Some(active),
                    _local: None,
                });
            }
            Err(LockError::Busy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(4));
            }
            Err(error) => return Err(error),
        }
    }
}

struct FollowHandoff {
    held: AtomicBool,
    failures: AtomicUsize,
}

impl FollowHandoff {
    const fn new() -> Self {
        Self {
            held: AtomicBool::new(false),
            failures: AtomicUsize::new(0),
        }
    }
}

static FOLLOW_HANDOFF: FollowHandoff = FollowHandoff::new();

/// A yieldable rig lease for Follow. Hold it through protocol cleanup and
/// driver return. A foreground device lock takes precedence.
pub struct FollowDeviceLock<'a> {
    rig: DeviceLock,
    active: File,
    handoff: &'a FollowHandoff,
}

impl FollowDeviceLock<'static> {
    pub fn acquire() -> Result<Self, LockError> {
        if ACTIVE_HERE.load(Ordering::Acquire) {
            return Err(LockError::BusyHere);
        }
        Self::acquire_at(
            &lock_path(std::env::var_os("XDG_RUNTIME_DIR")),
            &FOLLOW_HANDOFF,
        )
    }
}

impl<'a> FollowDeviceLock<'a> {
    fn acquire_at(path: &Path, handoff: &'a FollowHandoff) -> Result<Self, LockError> {
        let active = active_file(path)?;
        lock(&active)?;
        let file = open_lock_file(path)?;
        lock(&file)?;
        handoff.held.store(true, Ordering::Release);
        let lease = Self {
            rig: DeviceLock {
                _file: file,
                _active: None,
                _local: None,
            },
            active,
            handoff,
        };
        lease.active.unlock().map_err(unavailable)?;
        Ok(lease)
    }

    pub fn rig(&self) -> &DeviceLock {
        &self.rig
    }

    /// Once a handoff is requested, finish the current session before yielding.
    pub fn yield_requested(&self) -> bool {
        active_requested(&self.active)
    }

    /// Reject a concurrent foreground acquisition if the handoff failed.
    pub fn cleanup_failed(&self) {
        self.handoff.failures.fetch_add(1, Ordering::Release);
    }
}

impl Drop for FollowDeviceLock<'_> {
    fn drop(&mut self) {
        self.handoff.held.store(false, Ordering::Release);
    }
}

fn active_requested(active: &File) -> bool {
    lock(active).is_err() || active.unlock().is_err()
}

/// An interruptible lease on the normal rig lock for passive input only.
pub struct PassiveDeviceLock {
    _file: File,
    active: File,
    _local: Option<LocalLease>,
}

impl PassiveDeviceLock {
    pub fn acquire() -> Result<Self, LockError> {
        let local = LocalLease::acquire(&PASSIVE_HERE)?;
        let mut guard = Self::acquire_at(&lock_path(std::env::var_os("XDG_RUNTIME_DIR")))?;
        guard._local = Some(local);
        Ok(guard)
    }

    fn acquire_at(path: &Path) -> Result<Self, LockError> {
        let file = open_lock_file(path)?;
        lock(&file)?;
        // A passive attempt must not look like active intent to a Follow
        // lease that already owns the rig.
        let active = active_file(path)?;
        lock(&active)?;
        active.unlock().map_err(unavailable)?;
        Ok(Self {
            _file: file,
            active,
            _local: None,
        })
    }

    /// Call before each bounded read. Any failure also ends the lease.
    pub fn yield_requested(&self) -> bool {
        active_requested(&self.active)
    }
}

#[cfg(unix)]
fn verify(inherited: impl std::os::fd::AsFd, path: &Path) -> Result<(), LockError> {
    use std::os::unix::fs::MetadataExt;

    let file = File::from(
        inherited
            .as_fd()
            .try_clone_to_owned()
            .map_err(unavailable)?,
    );
    let expected = open_lock_file(path)?.metadata().map_err(unavailable)?;
    let actual = file.metadata().map_err(unavailable)?;
    if (actual.dev(), actual.ino()) != (expected.dev(), expected.ino()) {
        return Err(unavailable(
            "the owner procedure did not inherit the rig lock",
        ));
    }
    lock(&file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_path_uses_absolute_runtime_directory_with_a_temp_fallback() {
        let absolute = if cfg!(windows) {
            r"C:\Runtime"
        } else {
            "/run/user/fixture"
        };
        assert_eq!(
            lock_path(Some(absolute.into())),
            Path::new(absolute).join(LOCK_NAME)
        );
        assert_eq!(lock_path(None), fallback_directory().join(LOCK_NAME));
        assert_eq!(lock_path(Some("relative".into())), lock_path(None));
        assert_eq!(lock_path(Some("".into())), lock_path(None));
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux {
    use super::*;

    #[test]
    fn tracked_lock_distinguishes_this_process_from_untracked_contenders() {
        let folder =
            std::env::temp_dir().join(format!("variax-lock-origin-test-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("lock");
        let held = acquire_tracked(&path).unwrap();
        assert!(matches!(acquire_tracked(&path), Err(LockError::BusyHere)));
        drop(held);
        let external = open_lock_file(&path).unwrap();
        lock(&external).unwrap();
        assert!(matches!(acquire_tracked(&path), Err(LockError::Busy)));
        drop(external);
        assert!(acquire_tracked(&path).is_ok());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn passive_listener_yields_to_active_work_and_cannot_reenter() {
        let folder =
            std::env::temp_dir().join(format!("variax-passive-lock-test-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("lock");
        let passive = PassiveDeviceLock::acquire_at(&path).unwrap();
        assert!(!passive.yield_requested());
        let pending = active_file(&path).unwrap();
        lock(&pending).unwrap();
        assert!(passive.yield_requested());
        assert!(matches!(
            PassiveDeviceLock::acquire_at(&path),
            Err(LockError::Busy)
        ));
        drop(passive);
        drop(pending);
        let active = acquire_at(&path).unwrap();
        assert!(matches!(
            PassiveDeviceLock::acquire_at(&path),
            Err(LockError::Busy)
        ));
        drop(active);
        assert!(PassiveDeviceLock::acquire_at(&path).is_ok());
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn follow_releases_the_session_before_foreground_work_and_reverifies_after_it() {
        use crate::family::variax::{FollowUpdate, follow};
        use crate::identity::{REQUEST as IDENTITY, tests::VARIAX_600};
        use crate::session::{
            Path as LinkPath, Quiet, Session,
            testing::{AllowAll, Fake, packets},
        };
        use std::sync::mpsc;

        const BRIDGE_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 1, 0xf7];
        const SESSION_ON: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 1, 0xf7];
        const SESSION_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 7, 0, 0x2a, 0, 0xf7];
        const BRIDGE_OFF: &[u8] = &[0xf0, 0, 1, 0x0c, 3, 0x23, 0, 0xf7];
        let mut reply = VARIAX_600;
        reply[12..16].copy_from_slice(b"0310");
        let fake_link = || {
            Fake::from_bytes(
                LinkPath::VariaxViaPodxt,
                vec![
                    (IDENTITY.into(), packets(&reply)),
                    (BRIDGE_ON.into(), vec![]),
                    (SESSION_ON.into(), vec![vec![0xb0, 7, 42]]),
                    (SESSION_OFF.into(), vec![]),
                    (BRIDGE_OFF.into(), vec![]),
                ],
            )
        };
        let folder =
            std::env::temp_dir().join(format!("variax-follow-handoff-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();

        for (operation, fails) in [
            ("device-check", false),
            ("live-edit", false),
            ("store", false),
            ("foreground-error", true),
        ] {
            let path = folder.join(operation);
            let handoff = FollowHandoff::new();
            let lease = FollowDeviceLock::acquire_at(&path, &handoff).unwrap();
            let (start, ready) = mpsc::channel();
            let (returned, driver_return) = mpsc::channel();
            let (finished, finish) = mpsc::channel();
            std::thread::scope(|scope| {
                let path_ref = &path;
                let handoff_ref = &handoff;
                let worker = scope.spawn(move || {
                    ready.recv_timeout(Duration::from_secs(1)).unwrap();
                    let _foreground = acquire_at_with_handoff(path_ref, Some(handoff_ref)).unwrap();
                    assert!(
                        driver_return.try_recv().is_ok(),
                        "{operation} ran before cleanup"
                    );
                    assert!(matches!(
                        FollowDeviceLock::acquire_at(path_ref, handoff_ref),
                        Err(LockError::Busy)
                    ));
                    finished.send(()).unwrap();
                    if fails {
                        Err("injected operation failure")
                    } else {
                        Ok(())
                    }
                });
                let mut log = Quiet;
                let mut session = Session::new(fake_link(), &AllowAll, &mut log);
                follow(
                    lease.rig(),
                    &mut session,
                    || lease.yield_requested(),
                    |event| {
                        assert_eq!(event, FollowUpdate::Volume(42));
                        start.send(()).unwrap();
                        let deadline = Instant::now() + Duration::from_secs(2);
                        while !lease.yield_requested() {
                            assert!(Instant::now() < deadline);
                            std::thread::sleep(Duration::from_millis(1));
                        }
                    },
                )
                .unwrap();
                session.link.assert_finished();
                assert_eq!(
                    session.link.writes,
                    [
                        &IDENTITY[..],
                        BRIDGE_ON,
                        SESSION_ON,
                        SESSION_OFF,
                        BRIDGE_OFF
                    ]
                );
                assert!(finish.try_recv().is_err(), "lease must cover driver return");
                returned.send(()).unwrap();
                drop(lease);
                finish.recv_timeout(Duration::from_secs(1)).unwrap();
                assert_eq!(worker.join().unwrap().is_err(), fails);
            });

            let resumed = FollowDeviceLock::acquire_at(&path, &handoff).unwrap();
            let stop = std::cell::Cell::new(false);
            let mut log = Quiet;
            let mut session = Session::new(fake_link(), &AllowAll, &mut log);
            follow(
                resumed.rig(),
                &mut session,
                || stop.get(),
                |_| stop.set(true),
            )
            .unwrap();
            assert_eq!(session.link.writes.first().unwrap(), &IDENTITY);
            session.link.assert_finished();
        }
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn failed_follow_cleanup_refuses_the_waiting_foreground_acquisition() {
        let folder =
            std::env::temp_dir().join(format!("variax-follow-failed-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("rig");
        let handoff = FollowHandoff::new();
        let lease = FollowDeviceLock::acquire_at(&path, &handoff).unwrap();
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| acquire_at_with_handoff(&path, Some(&handoff)));
            let deadline = Instant::now() + Duration::from_secs(2);
            while !lease.yield_requested() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            lease.cleanup_failed();
            drop(lease);
            assert!(matches!(
                worker.join().unwrap(),
                Err(LockError::Unavailable(message)) if message.contains("could not cleanly release")
            ));
        });
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn exclusive_until_the_last_inherited_handle_closes() {
        const CHILD_PATH: &str = "VARIAX_DEVICE_LOCK_TEST_PATH";
        if let Some(path) = std::env::var_os(CHILD_PATH) {
            let path = std::path::PathBuf::from(path);
            verify(std::io::stdin(), &path).unwrap();
            assert!(matches!(acquire_tracked(&path), Err(LockError::Busy)));
            return;
        }
        let folder =
            std::env::temp_dir().join(format!("variax-device-lock-test-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("lock");
        let first = acquire_at(&path).unwrap();
        assert!(matches!(acquire_at(&path), Err(LockError::Busy)));
        assert!(matches!(
            verify(open_lock_file(&path).unwrap(), &path),
            Err(LockError::Busy)
        ));
        let inherited = first._file.try_clone().unwrap();
        verify(&inherited, &path).unwrap();
        let output = std::process::Command::new(std::env::args_os().next().unwrap())
            .args([
                "--exact",
                "device_lock::linux::exclusive_until_the_last_inherited_handle_closes",
            ])
            .env(CHILD_PATH, &path)
            .stdin(first.child_stdin().unwrap())
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{stdout}"
        );
        drop(first);
        assert!(matches!(acquire_at(&path), Err(LockError::Busy)));
        drop(inherited);
        assert!(acquire_at(&path).is_ok());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
