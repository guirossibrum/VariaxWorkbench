//! SIGINT only sets a flag. Device cleanup stays on the probe's main thread.

use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_sigint(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::Relaxed);
}

pub(super) struct Interrupt(libc::sigaction);

impl Interrupt {
    pub(super) fn install() -> std::io::Result<Self> {
        INTERRUPTED.store(false, Ordering::Relaxed);
        // SAFETY: All fields are initialized before installing the action. The
        // C handler only stores to a lock-free atomic, and old receives the
        // prior SIGINT action on success.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_sigint as *const () as libc::sighandler_t;
            let mut old = std::mem::MaybeUninit::uninit();
            if libc::sigemptyset(&mut action.sa_mask) != 0
                || libc::sigaction(libc::SIGINT, &action, old.as_mut_ptr()) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self(old.assume_init()))
        }
    }

    pub(super) fn requested(&self) -> bool {
        INTERRUPTED.load(Ordering::Relaxed)
    }
}

impl Drop for Interrupt {
    fn drop(&mut self) {
        // SAFETY: This reinstalls the action returned by sigaction for SIGINT.
        unsafe { libc::sigaction(libc::SIGINT, &self.0, std::ptr::null_mut()) };
    }
}
