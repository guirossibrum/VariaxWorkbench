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

struct LocalLease<'a>(&'a AtomicBool);

impl<'a> LocalLease<'a> {
    fn acquire(held: &'a AtomicBool) -> Result<Self, LockError> {
        held.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self(held))
            .map_err(|_| LockError::BusyHere)
    }
}

impl Drop for LocalLease<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub struct DeviceLock {
    // The OS releases the lock when the last clone of this handle closes.
    _file: File,
    _active: Option<File>,
    _local: Option<LocalLease<'static>>,
}

impl DeviceLock {
    pub fn acquire() -> Result<Self, LockError> {
        Self::acquire_waiting(wait_for_rig)
    }

    fn acquire_waiting(
        wait: impl FnOnce(&File, Duration) -> Result<(), LockError>,
    ) -> Result<Self, LockError> {
        let (mut guard, local) = acquire_tracked(
            &lock_path(std::env::var_os("XDG_RUNTIME_DIR")),
            &ACTIVE_HERE,
            &PASSIVE_HERE,
            &FOLLOW_HANDOFF,
            wait,
        )?;
        guard._local = Some(local);
        Ok(guard)
    }

    /// Wait for the same foreground lock used by discovery and device tools.
    /// A busy result is safe to retry because the link has not been opened.
    pub fn acquire_for_download(
        cancelled: impl Fn() -> bool,
        acquired: &AtomicBool,
    ) -> Result<Self, String> {
        wait_for_download(
            &cancelled,
            acquired,
            || Self::acquire_waiting(|file, wait| wait_for_rig_until(file, wait, &cancelled)),
            std::thread::sleep,
        )
    }

    #[cfg(all(test, target_os = "linux"))]
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

fn wait_for_download<T>(
    cancelled: impl Fn() -> bool,
    acquired: &AtomicBool,
    mut acquire: impl FnMut() -> Result<T, LockError>,
    mut pause: impl FnMut(Duration),
) -> Result<T, String> {
    while !cancelled() {
        match acquire() {
            Ok(guard) if !cancelled() => {
                acquired.store(true, Ordering::Release);
                return Ok(guard);
            }
            Ok(_) => break,
            Err(LockError::Busy | LockError::BusyHere) => {
                if cancelled() {
                    break;
                }
                pause(Duration::from_millis(60));
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Download cancelled before opening the connection.".into())
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

fn acquire_tracked<'a>(
    path: &Path,
    active_here: &'a AtomicBool,
    passive_here: &AtomicBool,
    handoff: &FollowHandoff,
    wait_for: impl FnOnce(&File, Duration) -> Result<(), LockError>,
) -> Result<(DeviceLock, LocalLease<'a>), LockError> {
    let local = LocalLease::acquire(active_here)?;
    let passive_was_local = passive_here.load(Ordering::Acquire);
    let guard = acquire_with_wait(path, Some(handoff), wait_for).map_err(|error| match error {
        LockError::Busy
            if passive_was_local
                || passive_here.load(Ordering::Acquire)
                || handoff.held.load(Ordering::Acquire) =>
        {
            LockError::BusyHere
        }
        other => other,
    })?;
    Ok((guard, local))
}

#[cfg(all(test, target_os = "linux"))]
fn acquire_at(path: &Path) -> Result<DeviceLock, LockError> {
    acquire_with_wait(path, None, |file, _| lock(file))
}

fn acquire_with_wait(
    path: &Path,
    handoff: Option<&FollowHandoff>,
    wait_for: impl FnOnce(&File, Duration) -> Result<(), LockError>,
) -> Result<DeviceLock, LockError> {
    ForegroundRequest::begin(path, handoff)?.complete(wait_for)
}

struct ForegroundRequest<'a> {
    file: File,
    active: File,
    handoff: Option<&'a FollowHandoff>,
    failures_before: Option<usize>,
}

impl<'a> ForegroundRequest<'a> {
    fn begin(path: &Path, handoff: Option<&'a FollowHandoff>) -> Result<Self, LockError> {
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
        Ok(Self {
            file: open_lock_file(path)?,
            active,
            handoff,
            failures_before,
        })
    }

    fn complete(
        self,
        wait_for: impl FnOnce(&File, Duration) -> Result<(), LockError>,
    ) -> Result<DeviceLock, LockError> {
        // Follow may be finishing an identity exchange or returning the driver.
        let wait = if self
            .handoff
            .is_some_and(|state| state.held.load(Ordering::Acquire))
        {
            Duration::from_secs(10)
        } else {
            Duration::from_millis(100)
        };
        wait_for(&self.file, wait)?;
        if self
            .handoff
            .map(|state| state.failures.load(Ordering::Acquire))
            != self.failures_before
        {
            return Err(unavailable(
                "Variax Follow could not cleanly release the rig",
            ));
        }
        Ok(DeviceLock {
            _file: self.file,
            _active: Some(self.active),
            _local: None,
        })
    }
}

fn wait_for_rig(file: &File, wait: Duration) -> Result<(), LockError> {
    wait_for_rig_until(file, wait, || false)
}

fn wait_for_rig_until(
    file: &File,
    wait: Duration,
    cancelled: impl Fn() -> bool,
) -> Result<(), LockError> {
    let deadline = Instant::now() + wait;
    loop {
        if cancelled() {
            // Match the outer admission's contention path so it can report cancellation.
            return Err(LockError::Busy);
        }
        match lock(file) {
            Err(LockError::Busy) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(4));
            }
            result => return result,
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
    _local: Option<LocalLease<'static>>,
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
    fn queued_download_uses_the_existing_lock_and_checks_for_cancellation() {
        use std::cell::{Cell, RefCell};

        let folder =
            std::env::temp_dir().join(format!("variax-download-lock-test-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let shared = folder.join("shared.lock");
        let independent = folder.join("independent.lock");
        let acquired = AtomicBool::new(false);
        let holder = RefCell::new(Some(acquire_at(&shared).unwrap()));
        let released = Cell::new(false);
        let independent_guard = wait_for_download(
            || false,
            &AtomicBool::new(false),
            || acquire_at(&independent),
            |_| unreachable!(),
        )
        .unwrap();

        let next = wait_for_download(
            || false,
            &acquired,
            || acquire_at(&shared),
            |_| {
                assert!(!acquired.load(Ordering::Acquire));
                released.set(true);
                drop(holder.borrow_mut().take());
            },
        )
        .unwrap();
        assert!(released.get() && acquired.load(Ordering::Acquire));
        assert!(matches!(acquire_at(&shared), Err(LockError::Busy)));

        let cancelled = Cell::new(false);
        let acquired = AtomicBool::new(false);
        let error = wait_for_download(
            || cancelled.get(),
            &acquired,
            || acquire_at(&shared),
            |_| cancelled.set(true),
        )
        .err()
        .unwrap();
        assert!(error.contains("cancelled"));
        assert!(!acquired.load(Ordering::Acquire));
        drop((next, independent_guard));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn cancelling_inner_follow_wait_releases_active_intent_before_the_rig() {
        use std::cell::Cell;

        let folder =
            std::env::temp_dir().join(format!("variax-inner-download-wait-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("rig");
        let active = AtomicBool::new(false);
        let passive = AtomicBool::new(false);
        let handoff = FollowHandoff::new();
        let follow = FollowDeviceLock::acquire_at(&path, &handoff).unwrap();
        let polls = Cell::new(0);
        let cancelled = Cell::new(false);
        let acquired = AtomicBool::new(false);
        let result = wait_for_download(
            || cancelled.get(),
            &acquired,
            || {
                acquire_tracked(&path, &active, &passive, &handoff, |file, wait| {
                    assert_eq!(wait, Duration::from_secs(10));
                    assert!(matches!(
                        lock(&active_file(&path).unwrap()),
                        Err(LockError::Busy)
                    ));
                    wait_for_rig_until(file, wait, || {
                        polls.set(polls.get() + 1);
                        if polls.get() == 2 {
                            cancelled.set(true);
                        }
                        cancelled.get()
                    })
                })
            },
            |_| panic!("inner cancellation must bypass the outer retry"),
        );

        assert!(result.err().unwrap().contains("cancelled"));
        assert_eq!(polls.get(), 2);
        assert!(!acquired.load(Ordering::Acquire));
        assert!(!active.load(Ordering::Acquire));
        lock(&active_file(&path).unwrap()).unwrap();
        assert!(matches!(
            lock(&open_lock_file(&path).unwrap()),
            Err(LockError::Busy)
        ));
        drop(follow);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn tracked_lock_distinguishes_this_process_from_untracked_contenders() {
        let folder =
            std::env::temp_dir().join(format!("variax-lock-origin-test-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("lock");
        let active = AtomicBool::new(false);
        let passive = AtomicBool::new(false);
        let handoff = FollowHandoff::new();
        let acquire = || acquire_tracked(&path, &active, &passive, &handoff, |file, _| lock(file));
        let held = acquire().unwrap();
        assert!(matches!(acquire(), Err(LockError::BusyHere)));
        drop(held);
        let listener = LocalLease::acquire(&passive).unwrap();
        let externally_locked = open_lock_file(&path).unwrap();
        lock(&externally_locked).unwrap();
        assert!(matches!(acquire(), Err(LockError::BusyHere)));
        drop(listener);
        assert!(matches!(acquire(), Err(LockError::Busy)));
        drop(externally_locked);
        assert!(acquire().is_ok());
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
            let mut pending = None;
            let mut log = Quiet;
            let mut session = Session::new(fake_link(), &AllowAll, &mut log);
            follow(
                lease.rig(),
                &mut session,
                || lease.yield_requested(),
                |event| {
                    assert_eq!(event, FollowUpdate::Volume(42));
                    pending = Some(ForegroundRequest::begin(&path, Some(&handoff)).unwrap());
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

            // The foreground still owns its original intent and cannot enter
            // while the Follow lease covers driver return.
            let pending = pending.unwrap();
            let foreground = pending
                .complete(|file, wait| {
                    assert_eq!(wait, Duration::from_secs(10));
                    assert!(lease.yield_requested());
                    assert!(matches!(lock(file), Err(LockError::Busy)));
                    drop(lease);
                    lock(file)
                })
                .unwrap();
            assert!(matches!(
                FollowDeviceLock::acquire_at(&path, &handoff),
                Err(LockError::Busy)
            ));
            let result = if fails {
                Err("injected operation failure")
            } else {
                Ok(())
            };
            drop(foreground);
            assert_eq!(result.is_err(), fails);

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
        let result = acquire_with_wait(&path, Some(&handoff), |file, _| {
            assert!(lease.yield_requested());
            assert!(matches!(lock(file), Err(LockError::Busy)));
            lease.cleanup_failed();
            drop(lease);
            lock(file)
        });
        assert!(matches!(
            result,
            Err(LockError::Unavailable(message)) if message.contains("could not cleanly release")
        ));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn exclusive_until_the_last_cloned_handle_closes() {
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
        drop(first);
        assert!(matches!(acquire_at(&path), Err(LockError::Busy)));
        drop(inherited);
        assert!(acquire_at(&path).is_ok());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
