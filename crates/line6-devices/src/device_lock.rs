//! Cooperative exclusion for the PODxt Live and its guitar interface.
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

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

const LOCK_NAME: &str = "variax-workbench-podxt-live.lock";

pub struct DeviceLock {
    // The OS releases the lock when the last clone of this handle closes.
    _file: File,
}

impl DeviceLock {
    pub fn acquire() -> Result<Self, LockError> {
        acquire_at(&lock_path(std::env::var_os("XDG_RUNTIME_DIR")))
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

fn acquire_at(path: &Path) -> Result<DeviceLock, LockError> {
    let file = open_lock_file(path)?;
    lock(&file)?;
    Ok(DeviceLock { _file: file })
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
    fn exclusive_until_the_last_inherited_handle_closes() {
        const CHILD_PATH: &str = "VARIAX_DEVICE_LOCK_TEST_PATH";
        if let Some(path) = std::env::var_os(CHILD_PATH) {
            let path = std::path::PathBuf::from(path);
            verify(std::io::stdin(), &path).unwrap();
            assert!(matches!(acquire_at(&path), Err(LockError::Busy)));
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
