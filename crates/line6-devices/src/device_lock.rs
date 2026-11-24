//! Cooperative exclusion for the PODxt Live and its guitar interface.
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum LockError {
    Busy,
    Unavailable(String),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("PODxt Live is busy with another operation."),
            Self::Unavailable(error) => write!(f, "Cannot lock the PODxt Live: {error}."),
        }
    }
}

pub struct DeviceLock {
    #[cfg(target_os = "linux")]
    file: std::fs::File,
}

impl DeviceLock {
    pub fn acquire() -> Result<Self, LockError> {
        #[cfg(target_os = "linux")]
        {
            linux::acquire(std::path::Path::new(linux::LOCK_PATH))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(LockError::Unavailable(
                "device locking requires Linux".into(),
            ))
        }
    }

    /// Pass the same locked file description to a multi-stage owner procedure.
    #[cfg(target_os = "linux")]
    pub fn child_stdin(&self) -> std::io::Result<std::process::Stdio> {
        self.file.try_clone().map(std::process::Stdio::from)
    }

    /// A child command may reuse the lock only through the inherited descriptor.
    #[cfg(target_os = "linux")]
    pub fn verify_inherited_stdin() -> Result<(), LockError> {
        linux::verify(std::io::stdin(), std::path::Path::new(linux::LOCK_PATH))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use rustix::fd::AsFd;
    use rustix::fs::{self, FlockOperation, OFlags};
    use std::fs::{File, OpenOptions};
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;

    pub(super) const LOCK_PATH: &str = "/tmp/variax-workbench-podxt-live.lock";

    fn unavailable(error: impl fmt::Display) -> LockError {
        LockError::Unavailable(error.to_string())
    }

    fn open(path: &Path) -> Result<File, LockError> {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags((OFlags::NOFOLLOW | OFlags::NONBLOCK).bits() as i32);
        let file = match options.open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let result = options
                    .clone()
                    .write(true)
                    .create_new(true)
                    .mode(0o644)
                    .open(path);
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

    fn lock(file: impl AsFd) -> Result<(), LockError> {
        fs::flock(file, FlockOperation::NonBlockingLockExclusive).map_err(|error| {
            if error == rustix::io::Errno::WOULDBLOCK {
                LockError::Busy
            } else {
                unavailable(error)
            }
        })
    }

    pub(super) fn acquire(path: &Path) -> Result<DeviceLock, LockError> {
        let file = open(path)?;
        lock(&file)?;
        Ok(DeviceLock { file })
    }

    pub(super) fn verify(inherited: impl AsFd, path: &Path) -> Result<(), LockError> {
        let expected = fs::fstat(open(path)?).map_err(unavailable)?;
        let actual = fs::fstat(&inherited).map_err(unavailable)?;
        if (actual.st_dev, actual.st_ino) != (expected.st_dev, expected.st_ino) {
            return Err(unavailable(
                "the owner procedure did not inherit the rig lock",
            ));
        }
        lock(inherited)
    }

    #[cfg(test)]
    #[test]
    fn exclusive_until_the_last_inherited_handle_closes() {
        const CHILD_PATH: &str = "VARIAX_DEVICE_LOCK_TEST_PATH";
        if let Some(path) = std::env::var_os(CHILD_PATH) {
            let path = std::path::PathBuf::from(path);
            verify(std::io::stdin(), &path).unwrap();
            assert!(matches!(acquire(&path), Err(LockError::Busy)));
            return;
        }
        let folder =
            std::env::temp_dir().join(format!("variax-device-lock-test-{}", std::process::id()));
        std::fs::create_dir(&folder).unwrap();
        let path = folder.join("lock");
        let first = acquire(&path).unwrap();
        assert!(matches!(acquire(&path), Err(LockError::Busy)));
        assert!(matches!(
            verify(open(&path).unwrap(), &path),
            Err(LockError::Busy)
        ));
        let inherited = first.file.try_clone().unwrap();
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
        assert!(matches!(acquire(&path), Err(LockError::Busy)));
        drop(inherited);
        assert!(acquire(&path).is_ok());
        std::fs::remove_dir_all(folder).unwrap();
    }
}
