//! Owner capture of unsolicited traffic on the two PODxt Live inputs (#201).

#[allow(unsafe_code)]
mod interrupt;
mod pump;

use line6_devices::backup::{BackupFolder, utc};
use line6_devices::device_lock::DeviceLock;
use line6_devices::family::{podxt, variax};
use line6_devices::gate::{GatePolicy, Operation};
use line6_devices::identity::Identity;
use line6_devices::midi::{Assembler, Event, hex};
use line6_devices::session::{Link, Path, Quiet, Session};
use line6_devices::transport::{rawmidi::RawMidiLink, usb::UsbInterfaceLink};
use std::cell::RefCell;
use std::io::Write;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use variax_workbench::hardware_gates::{Gate, run_if_unlocked};

pub(super) const DEFAULT_SECONDS: u64 = 60;
const MAX_SECONDS: u64 = 120;
const BRIDGE_SETTLE: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(10);
const CANCELLED: &str = "Cancelled by Ctrl+C.";

pub(super) fn parse_seconds(text: &str) -> Result<u64, String> {
    match text.parse() {
        Ok(seconds @ 1..=MAX_SECONDS) => Ok(seconds),
        _ => Err(format!(
            "listen SECONDS must be a whole number 1..={MAX_SECONDS}"
        )),
    }
}

struct NoticeGrants;

impl GatePolicy for NoticeGrants {
    fn allows(&self, operation: Operation) -> bool {
        matches!(
            operation,
            Operation::PodxtIdentity | Operation::VariaxIdentity | Operation::VariaxRead
        )
    }
}

pub(super) fn run(seconds: u64, out: &std::path::Path) -> Result<(), String> {
    run_if_unlocked(Gate::OsDiscovery, || {
        run_if_unlocked(Gate::IdentityRequest, || {
            run_if_unlocked(Gate::CurrentSettingsRead, || {
                let interrupt = interrupt::Interrupt::install()
                    .map_err(|error| format!("Cannot handle Ctrl+C: {error}"))?;
                with_rig(
                    || DeviceLock::acquire().map_err(|error| error.to_string()),
                    || capture_run(seconds, out, &|| interrupt.requested()),
                )
            })
        })
    })
}

fn with_rig<G>(
    acquire: impl FnOnce() -> Result<G, String>,
    run: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let _rig = acquire()?;
    run()
}

fn capture_run(
    seconds: u64,
    out: &std::path::Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;
    let (stamp, when) = utc(epoch.as_secs());
    let mut capture = BackupFolder::create(
        out,
        &format!("probe-variax-notices-{stamp}-{:09}", epoch.subsec_nanos()),
    )?;
    capture.record(format!("variax-probe variax-notices at {when}"));
    let start = Instant::now();
    let elapsed = || start.elapsed();
    let trace = RefCell::new(Trace::new(&elapsed, &print_line));
    let result = check_cancel(cancelled).and_then(|()| {
        observe(
            RawMidiLink::open_spec(podxt::transport::PODXT_LIVE_MIDI)?,
            || UsbInterfaceLink::open(variax::transport::VARIAX_VIA_PODXT_LIVE),
            Duration::from_secs(seconds),
            cancelled,
            &trace,
        )
    });
    let trace = trace.into_inner();
    for line in trace.lines {
        capture.record(line);
    }
    let summary = format!(
        "SysEx frames seen (PODxt MIDI / Variax link): {} / {}. Marked notices: {}.",
        trace.frames[0],
        trace.frames[1],
        trace.notices.len(),
    );
    print_line(&summary);
    capture.record(summary);
    for notice in trace.notices {
        print_line(&notice);
    }
    if let Err(error) = &result {
        capture.record(format!("STOPPED: {error}"));
    }
    match capture.finish(result.is_ok()) {
        Ok(dir) => {
            print_line(&format!("Capture: {}", dir.display()));
            result
        }
        Err(error) => append_error(result, Err(error), "Save capture"),
    }
}

fn print_line(text: &str) {
    let _ = writeln!(std::io::stdout(), "{text}");
}

trait BridgeLink: Link {
    fn finish(&mut self) -> Result<(), String>;
}

impl BridgeLink for UsbInterfaceLink {
    fn finish(&mut self) -> Result<(), String> {
        UsbInterfaceLink::finish(self)
    }
}

fn observe<M: Link, B: BridgeLink>(
    midi: M,
    open_bridge: impl FnOnce() -> Result<B, String>,
    window: Duration,
    cancelled: &dyn Fn() -> bool,
    trace: &RefCell<Trace<'_>>,
) -> Result<(), String> {
    let mut midi = Observed::new(midi, Source::PodxtMidi, trace, cancelled);
    // Reuse the existing PODxt identity exchange with no post-identity wait.
    let (reply, _) = podxt::listen(
        &mut Session::new(&mut midi, &NoticeGrants, &mut Quiet),
        Duration::ZERO,
    )?;
    let pod = Identity::parse(&reply).expect("PODxt listen verified this reply");
    if pod.member != podxt::PODXT_LIVE_MEMBER || pod.version != [0, 3, 0, 1] {
        return Err(format!(
            "Expected PODxt Live 3.01. Identity: {}",
            hex(&reply)
        ));
    }
    trace
        .borrow_mut()
        .announce("PODxt Live 3.01 identity verified.");
    check_cancel(cancelled)?;

    let mut bridge = Observed::new(open_bridge()?, Source::VariaxLink, trace, cancelled);
    let result = observe_with_bridge(&mut midi, &mut bridge, window, cancelled, trace);
    let finish = bridge.link.finish();
    if finish.is_ok() {
        trace
            .borrow_mut()
            .announce("Variax interface released to the kernel.");
    }
    append_error(result, finish, "Release Variax interface")
}

fn observe_with_bridge<M: Link, B: Link>(
    midi: &mut M,
    bridge: &mut Observed<'_, '_, B>,
    window: Duration,
    cancelled: &dyn Fn() -> bool,
    trace: &RefCell<Trace<'_>>,
) -> Result<(), String> {
    let mut quiet = Quiet;
    let mut session = Session::new(bridge, &NoticeGrants, &mut quiet);
    match variax::identify_if_present(&mut session)? {
        Some(id)
            if id.identity.is_line6()
                && id.identity.family == variax::IDENTITY_FAMILY
                && id.identity.member == 4
                && id.identity.version == *b"0310" =>
        {
            trace.borrow_mut().announce(&format!(
                "Variax 300/600 3.10 identity verified ({} request(s)).",
                id.requests
            ));
        }
        Some(id) => return Err(format!("Expected Variax 300/600 3.10. Identity: {}", hex(&id.reply))),
        None => trace.borrow_mut().announce(
            "No guitar identity answered at startup. Continuing in case the VDI cable is unplugged.",
        ),
    }
    check_cancel(cancelled)?;
    let result = (|| {
        variax::set_bridge_for_read(&mut session, true)?;
        drain_inputs(midi, &mut session.link, BRIDGE_SETTLE, cancelled)?;
        session.send(variax::VariaxRequest::SessionOn)?;
        trace.borrow_mut().announce(&format!(
            "Listening for {} s. Note VDI cable changes relative to this marker.",
            window.as_secs()
        ));
        drain_inputs(midi, &mut session.link, window, cancelled)
    })();
    // On any path that attempted enable, try both off messages in this order.
    // A failed or interrupted enable write may have reached the hardware.
    session.link.honor_cancel = false;
    let midi_off = session.send(variax::VariaxRequest::SessionOff);
    let bridge_off = variax::set_bridge_for_read(&mut session, false);
    append_error(
        append_error(result, midi_off, "Variax MIDI off"),
        bridge_off,
        "PODxt Variax messages off",
    )
}

fn drain_inputs(
    midi: &mut impl Link,
    bridge: &mut impl Link,
    window: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), String> {
    let end = bridge.elapsed() + window;
    loop {
        check_cancel(cancelled)?;
        let remaining = end.saturating_sub(bridge.elapsed());
        if remaining.is_zero() {
            return Ok(());
        }
        pump::poll_pair(
            |wait| midi.read(wait).map(|_| ()),
            |wait| bridge.read(wait),
            remaining.min(POLL),
        )?;
    }
}

fn check_cancel(cancelled: &dyn Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        Err(CANCELLED.into())
    } else {
        Ok(())
    }
}

fn append_error(
    result: Result<(), String>,
    cleanup: Result<(), String>,
    stage: &str,
) -> Result<(), String> {
    match cleanup {
        Ok(()) => result,
        Err(error) => Err(match result {
            Ok(()) => format!("{stage}: {error}"),
            Err(first) => format!("{first}\n{stage}: {error}"),
        }),
    }
}

#[derive(Clone, Copy)]
enum Source {
    PodxtMidi = 0,
    VariaxLink = 1,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::PodxtMidi => "PODxt MIDI",
            Self::VariaxLink => "Variax link",
        }
    }
}

struct Trace<'a> {
    elapsed: &'a dyn Fn() -> Duration,
    display: &'a dyn Fn(&str),
    lines: Vec<String>,
    frames: [usize; 2],
    notices: Vec<String>,
}

impl<'a> Trace<'a> {
    fn new(elapsed: &'a dyn Fn() -> Duration, display: &'a dyn Fn(&str)) -> Self {
        Self {
            elapsed,
            display,
            lines: Vec::new(),
            frames: [0; 2],
            notices: Vec::new(),
        }
    }

    fn line(&mut self, text: &str) -> String {
        let line = format!("{:9.6}s {text}", (self.elapsed)().as_secs_f64());
        self.lines.push(line.clone());
        line
    }

    fn announce(&mut self, text: &str) {
        (self.display)(&self.line(text));
    }

    fn frame(&mut self, source: Source, frame: &[u8]) {
        self.frames[source as usize] += 1;
        let detail = notice(frame);
        let suffix = detail
            .as_ref()
            .map_or_else(String::new, |detail| format!(" [{detail}]"));
        let line = self.line(&format!("{} SysEx {}{suffix}", source.label(), hex(frame)));
        if detail.is_some() {
            self.notices.push(line);
        }
    }
}

fn notice(frame: &[u8]) -> Option<String> {
    let body = frame.strip_prefix(&[0xf0, 0, 1, 0x0c])?;
    match body {
        [0x03, 0x11, ..] if frame.len() == 11 => {
            let state = frame[9];
            let decoded = if state == 0 {
                "disconnected"
            } else {
                "connected"
            };
            Some(format!("03 11: {decoded} (state 0x{state:02X})"))
        }
        [0x7f, 0x03, ..] if frame.len() == 12 => Some("7F 03: disconnected".into()),
        [0x03, 0x11, ..] => Some("03 11: malformed length".into()),
        [0x7f, 0x03, ..] => Some("7F 03: malformed length".into()),
        [0x07, 0x00 | 0x04, op @ (0x28 | 0x4d | 0x74), .., 0xf7] => {
            Some(format!("Variax {op:02X} notice"))
        }
        _ => None,
    }
}

struct Observed<'a, 'trace, L> {
    link: L,
    source: Source,
    assembler: Assembler,
    trace: &'a RefCell<Trace<'trace>>,
    cancelled: &'a dyn Fn() -> bool,
    honor_cancel: bool,
}

impl<'a, 'trace, L> Observed<'a, 'trace, L> {
    fn new(
        link: L,
        source: Source,
        trace: &'a RefCell<Trace<'trace>>,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            link,
            source,
            assembler: Assembler::default(),
            trace,
            cancelled,
            honor_cancel: true,
        }
    }
}

impl<L: Link> Link for Observed<'_, '_, L> {
    fn path(&self) -> Path {
        self.link.path()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        if self.honor_cancel {
            check_cancel(self.cancelled)?;
        }
        self.trace
            .borrow_mut()
            .line(&format!("{} OUT {}", self.source.label(), hex(bytes)));
        self.link.write(bytes)
    }

    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        check_cancel(self.cancelled)?;
        let packet = self.link.read(wait)?;
        if let Some(bytes) = &packet {
            let mut trace = self.trace.borrow_mut();
            trace.line(&format!("{} IN  {}", self.source.label(), hex(bytes)));
            for event in self.assembler.feed(bytes)? {
                if let Event::Sysex(frame) = event {
                    trace.frame(self.source, &frame);
                }
            }
        }
        Ok(packet)
    }

    fn elapsed(&self) -> Duration {
        self.link.elapsed()
    }

    fn pause(&mut self, duration: Duration) {
        self.link.pause(duration);
    }
}

#[cfg(test)]
mod tests;
