use std::io;
use std::os::fd::{IntoRawFd, OwnedFd};

#[allow(unsafe_code)]
pub(super) fn close_once(port: OwnedFd) -> io::Result<()> {
    let fd = port.into_raw_fd();
    // SAFETY: the owned descriptor was consumed. Linux releases its number
    // even on EINTR, so retrying could close an unrelated descriptor.
    if unsafe { libc::close(fd) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
