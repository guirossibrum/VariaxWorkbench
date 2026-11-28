use crate::pump::poll_pair;
use line6_devices::{
    device_lock::{DeviceLock, FollowDeviceLock},
    family::variax::{FollowUpdate, VariaxRequest, follow_window},
    gate::{GatePolicy, Operation},
    midi::{Assembler, Event},
    session::{Link, Path, Quiet, Request, Session},
};
use std::{
    cell::{Cell, RefCell},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const WINDOW: Duration = Duration::from_secs(30);

struct FollowReads;

impl GatePolicy for FollowReads {
    fn allows(&self, operation: Operation) -> bool {
        matches!(operation, Operation::VariaxIdentity | Operation::VariaxRead)
    }
}

trait PodxtInput {
    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String>;
    fn finish(self) -> Result<(), String>;
}

trait GuitarInput: Link {
    fn finish(&mut self) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stop {
    Running,
    Cancelled,
    Foreground,
}

#[derive(Default)]
struct Observed {
    channel: bool,
    selector: bool,
    volume: bool,
    tone: bool,
    disconnected: bool,
}

struct PairedInput<'a, P, G> {
    podxt: &'a mut P,
    guitar: &'a mut G,
    decoder: Assembler,
    observed: &'a RefCell<Observed>,
    emit: &'a dyn Fn(&str),
    listening: &'a Cell<bool>,
    clock: Instant,
    session_on: Vec<u8>,
    armed: bool,
}

impl<P: PodxtInput, G: GuitarInput> Link for PairedInput<'_, P, G> {
    fn path(&self) -> Path {
        self.guitar.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.guitar.write(bytes)?;
        if bytes == self.session_on {
            self.armed = true;
        }
        Ok(())
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        // Keep identity outside the measurement window. The first read
        // after SessionOn is after the shared family settling delay.
        if !self.armed {
            return self.guitar.read(wait);
        }
        if !self.listening.get() {
            // Drop data buffered before the overlap marker. Bound the drain
            // so a constantly busy MIDI stream cannot prevent cancellation.
            let mut drained = false;
            for _ in 0..8 {
                if self.podxt.read(Duration::ZERO)?.is_none() {
                    drained = true;
                    break;
                }
            }
            if !drained {
                return Err("PODxt input did not drain before the overlap marker.".into());
            }
            self.listening.set(true);
            (self.emit)(
                "OVERLAP: up to 30 seconds. Move PODxt channel, Variax selector, volume and tone. Then Ctrl+C.",
            );
        }
        let at = self.clock.elapsed().as_secs_f64();
        poll_pair(
            |wait| {
                if let Some(bytes) = self.podxt.read(wait)? {
                    for event in self.decoder.feed(&bytes)? {
                        if let Event::Short(frame) = event
                            && let [status, channel] = frame.as_slice()
                            && status & 0xf0 == 0xc0
                        {
                            self.observed.borrow_mut().channel = true;
                            (self.emit)(&format!("{at:.3}s PODxt ALSA channel program {channel}"));
                        }
                    }
                }
                Ok(())
            },
            |wait| self.guitar.read(wait),
            wait,
        )
    }

    fn elapsed(&self) -> Duration {
        self.guitar.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.guitar.pause(duration);
    }
}

fn exercise<P: PodxtInput, G: GuitarInput>(
    mut podxt: P,
    open_guitar: impl FnOnce() -> Result<G, String>,
    verify_attachment: impl Fn() -> Result<(), String>,
    stop: impl Fn() -> Stop,
    start_waiter: impl Fn() -> Result<(), String>,
    emit: impl Fn(&str),
) -> Result<(), String> {
    let outcome = (|| {
        if stop() != Stop::Running {
            return Err("Stopped before claiming the guitar interface.".into());
        }
        verify_attachment()?;
        let mut guitar = open_guitar()?;
        let outcome = verify_attachment()
            .and_then(|()| listen(&mut podxt, &mut guitar, &stop, &start_waiter, &emit));
        let checked_attachment = verify_attachment();
        let usb_release = guitar.finish();
        if usb_release.is_ok() {
            emit("Variax USB interface released and kernel-driver return verified.");
        }
        combine(
            combine(outcome, checked_attachment, "Final attachment check"),
            usb_release,
            "Variax USB release",
        )
    })();
    let alsa_release = podxt.finish();
    if alsa_release.is_ok() {
        emit("PODxt read-only ALSA descriptor checked close succeeded.");
    }
    combine(outcome, alsa_release, "PODxt ALSA close")
}

fn listen(
    podxt: &mut impl PodxtInput,
    guitar: &mut impl GuitarInput,
    stop: &dyn Fn() -> Stop,
    start_waiter: &dyn Fn() -> Result<(), String>,
    emit: &dyn Fn(&str),
) -> Result<(), String> {
    let observed = RefCell::new(Observed::default());
    let listening = Cell::new(false);
    let stopped = Cell::new(Stop::Running);
    let waiter = RefCell::new(None);
    let cancelled = || {
        if stopped.get() != Stop::Running {
            return true;
        }
        let reason = stop();
        stopped.set(reason);
        if reason == Stop::Cancelled && listening.get() {
            // Start the lock-only claimant and observe its active intent
            // before Follow exits its poll loop and sends either off command.
            *waiter.borrow_mut() = Some(start_waiter());
        }
        reason != Stop::Running
    };
    let clock = Instant::now();
    let input = PairedInput {
        podxt,
        guitar,
        decoder: Assembler::default(),
        observed: &observed,
        emit,
        listening: &listening,
        clock,
        session_on: VariaxRequest::SessionOn.bytes(),
        armed: false,
    };
    let mut log = Quiet;
    let mut session = Session::new(input, &FollowReads, &mut log);
    let result = follow_window(&mut session, WINDOW, false, cancelled, |update| {
        let mut state = observed.borrow_mut();
        match update {
            FollowUpdate::ProgramChange(_) => state.selector = true,
            FollowUpdate::Volume(_) => state.volume = true,
            FollowUpdate::TonePosition(_) => state.tone = true,
            FollowUpdate::Disconnected => state.disconnected = true,
            _ => {}
        }
        let at = clock.elapsed().as_secs_f64();
        match update {
            FollowUpdate::ProgramChange(preset) => emit(&format!(
                "{at:.3}s Variax USB selector {} {}",
                preset.bank_name(),
                preset.slot() + 1
            )),
            _ => emit(&format!("{at:.3}s Variax USB {update:?}")),
        }
    });
    // The family API returns success only after both off sends succeed.
    if result.is_ok() && session.link.armed {
        emit("Variax session off and relay off succeeded.");
    }
    combine(
        result,
        waiter.into_inner().unwrap_or(Ok(())),
        "Foreground waiter",
    )?;
    let state = observed.into_inner();
    if state.disconnected {
        return Err("The guitar disconnected during the overlap window.".into());
    }
    if !listening.get() || stopped.get() != Stop::Cancelled {
        return Err("The overlap must end with Ctrl+C while both inputs are open.".into());
    }
    let missing: Vec<_> = [
        (state.channel, "PODxt channel"),
        (state.selector, "guitar selector"),
        (state.volume, "guitar volume"),
        (state.tone, "guitar tone"),
    ]
    .into_iter()
    .filter_map(|(seen, name)| (!seen).then_some(name))
    .collect();
    if !missing.is_empty() {
        return Err(format!("Missing physical input: {}.", missing.join(", ")));
    }
    Ok(())
}

fn combine(
    first: Result<(), String>,
    second: Result<(), String>,
    stage: &str,
) -> Result<(), String> {
    match (first, second) {
        (result, Ok(())) => result,
        (Ok(()), Err(error)) => Err(format!("{stage}: {error}")),
        (Err(first), Err(error)) => Err(format!("{first}\n{stage}: {error}")),
    }
}

fn begin_waiter(
    lease: &FollowDeviceLock<'_>,
    waiter: &RefCell<Option<JoinHandle<Result<(), String>>>>,
) -> Result<(), String> {
    let thread = thread::Builder::new()
        .name("ingress-lock-only".into())
        .spawn(|| {
            // This mimics foreground intent. It never opens either device
            // and never issues an identity, read, or send.
            let guard = DeviceLock::acquire().map_err(|error| error.to_string())?;
            drop(guard);
            Ok(())
        })
        .map_err(|error| error.to_string())?;
    *waiter.borrow_mut() = Some(thread);
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        if lease.yield_requested() {
            return Ok(());
        }
        if Instant::now() >= deadline
            || waiter
                .borrow()
                .as_ref()
                .is_some_and(JoinHandle::is_finished)
        {
            return Err("Could not verify foreground intent before cleanup.".into());
        }
        thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(not(test))]
pub(crate) use native::run;

#[cfg(not(test))]
mod native {
    use super::*;
    use crate::interrupt::Interrupt;
    use line6_devices::transport::{
        rawmidi::{RawMidiAttachment, ScopedRawMidiInput},
        usb::{UsbInterfaceLink, VariaxDriverBinding, variax_driver_binding},
    };
    use std::io::Write;
    use variax_workbench::hardware_gates::{Gate, run_if_unlocked};

    impl PodxtInput for ScopedRawMidiInput<'_> {
        fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
            self.read(wait)
        }

        fn finish(self) -> Result<(), String> {
            self.finish().map_err(|error| error.to_string())
        }
    }

    impl GuitarInput for UsbInterfaceLink<&DeviceLock> {
        fn finish(&mut self) -> Result<(), String> {
            self.finish()
        }
    }

    fn say(line: &str) {
        let _ = writeln!(std::io::stdout(), "{line}");
    }

    pub(crate) fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
        let args: Vec<_> = args.collect();
        let [observe, path, address] = args.as_slice() else {
            return Err("Usage: passive-ingress-probe observe USB_PATH USB_ADDRESS".into());
        };
        if observe != "observe" {
            return Err("Usage: passive-ingress-probe observe USB_PATH USB_ADDRESS".into());
        }
        let address = address
            .parse::<u8>()
            .ok()
            .filter(|value| (1..=127).contains(value))
            .ok_or("Supply the current numeric USB address (1–127).")?;
        run_if_unlocked(Gate::OsDiscovery, || {
            run_if_unlocked(Gate::IdentityRequest, || {
                run_if_unlocked(Gate::DeviceCheckCurrentSettingsRead, || {
                    run_if_unlocked(Gate::CurrentSettingsRead, || observe_pair(path, address))
                })
            })
        })
    }

    fn observe_pair(path: &str, address: u8) -> Result<(), String> {
        let interrupt = Interrupt::install().map_err(|error| error.to_string())?;
        let lease = FollowDeviceLock::acquire().map_err(|error| error.to_string())?;
        let selected = RawMidiAttachment::select_podxt_live(lease.rig(), path, address)?;
        let stop = || {
            if lease.yield_requested() {
                Stop::Foreground
            } else if interrupt.requested() {
                Stop::Cancelled
            } else {
                Stop::Running
            }
        };
        let podxt =
            match ScopedRawMidiInput::open_attachment_borrowed(&selected, lease.rig(), || {
                stop() != Stop::Running
            }) {
                Ok(input) => input,
                Err(error) => {
                    if error.cleanup.is_some() {
                        lease.cleanup_failed();
                    }
                    return Err(error.to_string());
                }
            };
        say(&format!(
            "Verified read-only PODxt ALSA input on USB {path} address {address}."
        ));

        let waiter = RefCell::new(None);
        let result = exercise(
            podxt,
            || {
                UsbInterfaceLink::open_variax_attachment_borrowed_checked(
                    path,
                    address,
                    lease.rig(),
                    || {
                        selected.revalidate(lease.rig())?;
                        if !matches!(
                            variax_driver_binding(path, address)?,
                            VariaxDriverBinding::Expected
                        ) {
                            return Err("Expected the Variax kernel driver on interface 1.".into());
                        }
                        if stop() != Stop::Running {
                            return Err("Stopped before claiming the guitar interface.".into());
                        }
                        Ok(())
                    },
                )
            },
            || selected.revalidate(lease.rig()),
            stop,
            || {
                begin_waiter(&lease, &waiter)?;
                say("Foreground lock-only waiter requested the rig before cleanup.");
                Ok(())
            },
            say,
        );
        let waiter = waiter.into_inner();
        if result.is_err() && (waiter.is_some() || lease.yield_requested()) {
            // Conservatively reject the pending handoff on any incomplete run.
            // This records only the existing process-local failure evidence.
            lease.cleanup_failed();
        }
        drop(lease);

        let handoff = waiter.map_or_else(
            || Err("No lock-only waiter completed the handoff check.".into()),
            |waiter| {
                waiter
                    .join()
                    .map_err(|_| "Lock-only waiter stopped unexpectedly.".to_owned())?
            },
        );
        if result.is_ok() && handoff.is_ok() {
            say("Foreground lock-only waiter acquired after both checked releases.");
        }
        combine(result, handoff, "Foreground acquisition")?;
        say("PASS: both input routes and checked handoff observed.");
        Ok(())
    }
}

#[cfg(test)]
#[path = "passive_ingress_tests.rs"]
mod tests;
