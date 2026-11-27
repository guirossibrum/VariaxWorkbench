use super::*;
use podxt::EditorBuffer;

#[cfg(any(test, target_os = "macos"))]
const RETRY: Duration = Duration::from_millis(200);
pub(super) const CHANNEL_SETTLE: Duration = Duration::from_millis(100);

enum Outcome {
    #[cfg(any(test, target_os = "macos"))]
    Changed(Update),
    #[cfg(any(test, target_os = "macos"))]
    Yielded,
    Stopped,
}

pub(super) struct Update {
    pub(super) buffer: EditorBuffer,
    pub(super) program_change: bool,
}

struct Worker {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Result<Outcome, String>>>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            // A tab can close with passive Follow still running. Return its
            // interface before the application exits.
            let _ = thread.join();
        }
    }
}

#[derive(Default)]
pub(super) struct Follow {
    worker: Option<Worker>,
    retry_at: Option<Instant>,
    reconcile: bool,
    failed: bool,
}

impl Follow {
    pub(super) fn pause(&mut self) {
        if let Some(worker) = &self.worker
            && !worker.stop.swap(true, Ordering::AcqRel)
        {
            self.reconcile = true;
        }
    }

    pub(super) fn reset(&mut self) {
        self.pause();
        self.failed = false;
        self.retry_at = None;
        self.reconcile = false;
    }

    pub(super) fn poll(
        &mut self,
        device: Option<&detection::DeviceKey>,
        reply_pending: &Arc<AtomicBool>,
        ctx: &egui::Context,
    ) -> Result<Option<Update>, String> {
        if let Some(worker) = &self.worker {
            if !worker.thread.as_ref().expect("Follow worker").is_finished() {
                ctx.request_repaint_after(REPAINT_WHILE_READING);
                return Ok(None);
            }
            let mut worker = self.worker.take().expect("finished Follow worker");
            let stopped = worker.stop.load(Ordering::Acquire);
            let result = worker
                .thread
                .take()
                .expect("Follow worker")
                .join()
                .unwrap_or_else(|_| Err("PODxt Follow stopped unexpectedly.".into()));
            if stopped {
                if device.is_some() {
                    ctx.request_repaint();
                }
                return Ok(None);
            }
            match result {
                #[cfg(any(test, target_os = "macos"))]
                Ok(Outcome::Changed(update)) => {
                    self.reconcile = false;
                    self.retry_at = None;
                    ctx.request_repaint();
                    return Ok(Some(update));
                }
                #[cfg(any(test, target_os = "macos"))]
                Ok(Outcome::Yielded) => {
                    self.reconcile = true;
                    self.retry_at = Some(Instant::now() + RETRY);
                    ctx.request_repaint_after(RETRY);
                }
                Ok(Outcome::Stopped) => {}
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            }
            return Ok(None);
        }

        if let Some(device) =
            device.filter(|_| !self.failed && cfg!(target_os = "macos") && platform_available())
        {
            if let Some(at) = self.retry_at {
                let wait = at.saturating_duration_since(Instant::now());
                if !wait.is_zero() {
                    ctx.request_repaint_after(wait);
                    return Ok(None);
                }
            }
            let endpoint = device.clone();
            let stop = Arc::new(AtomicBool::new(false));
            let worker_stop = Arc::clone(&stop);
            let pending = Arc::clone(reply_pending);
            let repaint = ctx.clone();
            let reconcile = self.reconcile;
            let thread = std::thread::Builder::new()
                .name("podxt-follow".into())
                .spawn(move || {
                    let result = platform_follow(&endpoint, &worker_stop, &pending, reconcile);
                    repaint.request_repaint();
                    result
                })
                .map_err(|error| {
                    self.failed = true;
                    format!("Could not start PODxt Follow: {error}")
                })?;
            self.worker = Some(Worker {
                stop,
                thread: Some(thread),
            });
        }
        Ok(None)
    }
}

#[cfg(any(test, target_os = "macos"))]
fn listen(
    session: &mut MidiSession<'_, &mut dyn Link>,
    stop: impl Fn() -> bool,
    reply_pending: &AtomicBool,
    reconcile: bool,
) -> Result<Outcome, String> {
    use line6_devices::{midi::Event, session::POLL};
    let mut changed_at = None;
    while !stop() {
        let now = session.link.elapsed();
        let settled = changed_at.is_some_and(|at| now >= at);
        if settled || reconcile && changed_at.is_none() {
            // Finish a read already in flight before yielding the shared link.
            // Its result stays private until release, and a stopped worker's
            // result is discarded. Never start a read after a stop is observed.
            return track_reply(reply_pending, |outstanding| {
                podxt::get_editor_buffer_tracking_reply(session, || false, outstanding)
            })
            .map(|buffer| {
                Outcome::Changed(Update {
                    buffer,
                    program_change: settled,
                })
            });
        }
        let wait = changed_at
            .filter(|&at| at > now)
            .map_or(POLL, |at| (at - now).min(POLL));
        for event in session.poll(wait)?.into_iter().flatten() {
            match event {
                Event::Short(message)
                    if matches!(message.as_slice(), [status, program]
                        if status & 0xf0 == 0xc0
                            && Location::new(BankKind::Tones, u16::from(*program)).is_some()) =>
                {
                    changed_at = Some(session.link.elapsed() + CHANNEL_SETTLE);
                }
                Event::Sysex(frame)
                    if frame.starts_with(&[0xf0, 0, 1, 0x0c, 3, 0x74])
                        || frame == [0xf0, 0, 1, 0x0c, 3, 0x72, 0xf7] =>
                {
                    reply_pending.store(true, Ordering::Release);
                    return Err("An unsolicited PODxt tone reply arrived while following.".into());
                }
                _ => {}
            }
        }
    }
    Ok(Outcome::Stopped)
}

#[cfg(any(test, target_os = "macos"))]
fn follow_on_link(
    stop: impl Fn() -> bool,
    reply_pending: &AtomicBool,
    reconcile: bool,
    with_link: impl FnOnce(
        RunOnLink<'_, Result<Outcome, String>>,
    ) -> Result<Result<Outcome, String>, String>,
    cleanup_failed: impl FnOnce(),
) -> Result<Outcome, String> {
    let opened = std::cell::Cell::new(false);
    let result = with_link(Box::new(|link| {
        opened.set(true);
        let mut quiet = Quiet;
        let policy = edit::ReadBufferPolicy;
        let mut session = MidiSession::new(link, &policy, &mut quiet);
        session.log_packets = false;
        // Keep operation and release results separate until the link has closed.
        Ok(listen(&mut session, stop, reply_pending, reconcile))
    }));
    if result.is_err() && opened.get() {
        reply_pending.store(true, Ordering::Release);
        cleanup_failed();
    }
    result?
}

#[cfg(all(not(test), target_os = "macos"))]
fn platform_follow(
    device: &detection::DeviceKey,
    stop: &AtomicBool,
    reply_pending: &AtomicBool,
    reconcile: bool,
) -> Result<Outcome, String> {
    use line6_devices::device_lock::{FollowDeviceLock, LockError};
    use std::cell::Cell;

    let run = || {
        if stop.load(Ordering::Acquire) || reply_pending.load(Ordering::Acquire) {
            return Ok(Outcome::Stopped);
        }
        let lease = match FollowDeviceLock::acquire() {
            Ok(lease) => lease,
            Err(LockError::Busy | LockError::BusyHere) => return Ok(Outcome::Yielded),
            Err(error) => return Err(error.to_string()),
        };
        let yielded = Cell::new(false);
        let cancelled = || {
            if !yielded.get() && lease.yield_requested() {
                yielded.set(true);
            }
            yielded.get() || stop.load(Ordering::Acquire)
        };
        if cancelled() || reply_pending.load(Ordering::Acquire) {
            return Ok(if yielded.get() {
                Outcome::Yielded
            } else {
                Outcome::Stopped
            });
        }

        let outcome = follow_on_link(
            cancelled,
            reply_pending,
            reconcile,
            |run| with_platform_link(device, lease.rig(), run),
            || lease.cleanup_failed(),
        )?;
        Ok(if yielded.get() || lease.yield_requested() {
            Outcome::Yielded
        } else {
            outcome
        })
    };
    hardware_gates::run_if_unlocked(Gate::OsDiscovery, || {
        hardware_gates::run_if_unlocked(Gate::CurrentSettingsRead, run)
    })
}

#[cfg(any(test, not(target_os = "macos")))]
fn platform_follow(
    _device: &detection::DeviceKey,
    _stop: &AtomicBool,
    _reply_pending: &AtomicBool,
    _reconcile: bool,
) -> Result<Outcome, String> {
    Ok(Outcome::Stopped)
}

#[cfg(test)]
mod tests;
