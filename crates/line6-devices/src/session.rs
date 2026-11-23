//! A gated, logged conversation with one device over one link.

use crate::gate::{GatePolicy, Operation};
use crate::midi::{Assembler, ControllerTally, Event, describe_short, hex};
use std::fmt::Debug;
use std::time::Duration;

/// The path a link reaches. A request is only ever sent on its own path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Path {
    /// A Variax behind PODxt Live USB interface 1 (VDI bridge).
    VariaxViaPodxt,
    /// A PODxt-family device's own MIDI port.
    PodxtMidi,
}

/// A message a family allows. Families define these as closed enums whose
/// parameters are validated types, so no caller can hand a session arbitrary
/// bytes.
pub trait Request: Copy + Debug {
    fn bytes(self) -> Vec<u8>;
    fn path(self) -> Path;
    fn operation(self) -> Operation;
}

/// Raw byte transport. Only [`Session::send`] calls `write`.
pub trait Link {
    fn path(&self) -> Path;
    fn write(&mut self, bytes: &[u8]) -> Result<(), String>;
    /// One packet or chunk, or `None` when `wait` passes with nothing.
    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String>;
    /// Time since the link opened (a test link uses a virtual clock).
    fn elapsed(&self) -> Duration;
    fn pause(&mut self, duration: Duration);
}

/// Lets a caller lend a link to a session and keep using it afterwards
/// (for example to release a USB interface).
impl<L: Link + ?Sized> Link for &mut L {
    fn path(&self) -> Path {
        (**self).path()
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        (**self).write(bytes)
    }
    fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
        (**self).read(wait)
    }
    fn elapsed(&self) -> Duration {
        (**self).elapsed()
    }
    fn pause(&mut self, duration: Duration) {
        (**self).pause(duration)
    }
}

pub trait Log {
    fn line(&mut self, text: String);
}

/// Discards everything.
pub struct Quiet;

impl Log for Quiet {
    fn line(&mut self, _: String) {}
}

pub enum Wait {
    Found(Vec<u8>),
    /// Orphan bytes arrived and then the link went quiet.
    OrphanQuiet,
    Timeout,
}

pub const POLL: Duration = Duration::from_millis(100);
const ORPHAN_QUIET: Duration = Duration::from_millis(300);

pub struct Session<'a, L: Link> {
    pub link: L,
    policy: &'a dyn GatePolicy,
    log: &'a mut dyn Log,
    pub assembler: Assembler,
    /// Bytes of every request sent, in order.
    pub sent: Vec<Vec<u8>>,
    /// When false, received packets are tallied but not logged line by line
    /// (long library reads). Sent requests are always logged.
    pub log_packets: bool,
    pub packets_in: usize,
}

impl<'a, L: Link> Session<'a, L> {
    pub fn new(link: L, policy: &'a dyn GatePolicy, log: &'a mut dyn Log) -> Self {
        Self {
            link,
            policy,
            log,
            assembler: Assembler::default(),
            sent: Vec::new(),
            log_packets: true,
            packets_in: 0,
        }
    }

    pub fn note(&mut self, text: impl Into<String>) {
        let line = format!("{:8.3}s {}", self.link.elapsed().as_secs_f64(), text.into());
        self.log.line(line);
    }

    pub(crate) fn allows(&self, operation: Operation) -> bool {
        self.policy.allows(operation)
    }

    /// The only way bytes reach a device.
    pub fn send<R: Request>(&mut self, request: R) -> Result<(), String> {
        if request.path() != self.link.path() {
            return Err(format!("{request:?} is not sent on {:?}", self.link.path()));
        }
        if !self.policy.allows(request.operation()) {
            return Err(format!(
                "{request:?} needs {:?}, which this build does not allow",
                request.operation()
            ));
        }
        let bytes = request.bytes();
        self.note(format!("OUT {}", hex(&bytes)));
        self.link.write(&bytes)?;
        self.sent.push(bytes);
        Ok(())
    }

    /// Reads at most one packet, logs it, and returns the events it completed.
    /// `None` means nothing arrived within `wait`.
    pub fn poll(&mut self, wait: Duration) -> Result<Option<Vec<Event>>, String> {
        let Some(packet) = self.link.read(wait.max(Duration::from_millis(1)))? else {
            return Ok(None);
        };
        self.packets_in += 1;
        let shown: Vec<u8> = packet.iter().copied().filter(|b| *b < 0xf8).collect();
        let events = self.assembler.feed(&packet)?;
        if self.log_packets && !shown.is_empty() {
            // One line per packet; channel messages are decoded on the same line.
            let decoded: Vec<String> = events
                .iter()
                .filter_map(|event| match event {
                    Event::Short(message) => Some(describe_short(message)),
                    Event::Sysex(_) => None,
                })
                .collect();
            let suffix = if decoded.is_empty() {
                String::new()
            } else {
                format!("   [{}]", decoded.join("; "))
            };
            self.note(format!("IN  {}{suffix}", hex(&shown)));
        }
        Ok(Some(events))
    }

    /// Waits up to `window` for a SysEx message that `wanted` accepts.
    pub fn wait_for(
        &mut self,
        window: Duration,
        wanted: impl Fn(&[u8]) -> bool,
        stop_on_orphan_quiet: bool,
    ) -> Result<Wait, String> {
        let start = self.link.elapsed();
        let orphans_before = self.assembler.orphans;
        let mut last_packet = start;
        loop {
            let now = self.link.elapsed();
            let used = now.saturating_sub(start);
            if used >= window {
                return Ok(Wait::Timeout);
            }
            if stop_on_orphan_quiet
                && self.assembler.orphans > orphans_before
                && now.saturating_sub(last_packet) >= ORPHAN_QUIET
            {
                return Ok(Wait::OrphanQuiet);
            }
            let Some(events) = self.poll((window - used).min(POLL))? else {
                continue;
            };
            last_packet = self.link.elapsed();
            for event in events {
                if let Event::Sysex(message) = event {
                    if wanted(&message) {
                        return Ok(Wait::Found(message));
                    }
                    self.note(format!(
                        "    (other SysEx message, {} bytes)",
                        message.len()
                    ));
                }
            }
        }
    }

    /// Logs everything that arrives for `window`. Sends nothing.
    /// Returns a tally of every controller message seen.
    pub fn listen(&mut self, window: Duration) -> Result<ControllerTally, String> {
        let mut tally = ControllerTally::default();
        let end = self.link.elapsed() + window;
        while self.link.elapsed() < end {
            let left = end.saturating_sub(self.link.elapsed());
            if let Some(events) = self.poll(left.min(POLL))? {
                for event in events {
                    if let Event::Sysex(message) = &event {
                        self.note(format!("    (SysEx message, {} bytes)", message.len()));
                    }
                    tally.add(&event);
                }
            }
        }
        Ok(tally)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use crate::gate::Operation;
    use std::collections::VecDeque;

    pub struct AllowAll;
    impl GatePolicy for AllowAll {
        fn allows(&self, _: Operation) -> bool {
            true
        }
    }

    pub fn packets(message: &[u8]) -> Vec<Vec<u8>> {
        message.chunks(16).map(<[u8]>::to_vec).collect()
    }

    /// Scripted device: each expected write releases its reply packets.
    pub struct Fake {
        path: Path,
        clock: Duration,
        script: VecDeque<(Vec<u8>, Vec<Vec<u8>>)>,
        inbox: VecDeque<Vec<u8>>,
        pub writes: Vec<Vec<u8>>,
    }

    impl Fake {
        pub fn new<R: Request>(path: Path, script: Vec<(R, Vec<Vec<u8>>)>) -> Self {
            let script = script
                .into_iter()
                .map(|(r, replies)| (r.bytes(), replies))
                .collect();
            Self {
                path,
                clock: Duration::ZERO,
                script,
                inbox: VecDeque::new(),
                writes: Vec::new(),
            }
        }
    }

    impl Link for Fake {
        fn path(&self) -> Path {
            self.path
        }
        fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.writes.push(bytes.to_vec());
            let (expected, replies) = self.script.pop_front().expect("unexpected write");
            assert_eq!(bytes, expected, "write out of script order");
            self.inbox.extend(replies);
            Ok(())
        }
        fn read(&mut self, wait: Duration) -> Result<Option<Vec<u8>>, String> {
            match self.inbox.pop_front() {
                Some(packet) => {
                    self.clock += Duration::from_millis(4);
                    Ok(Some(packet))
                }
                None => {
                    self.clock += wait;
                    Ok(None)
                }
            }
        }
        fn elapsed(&self) -> Duration {
            self.clock
        }
        fn pause(&mut self, duration: Duration) {
            self.clock += duration;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;
    use crate::family::{podxt::PodxtRequest, variax::VariaxRequest};
    use crate::gate::AllClosed;

    #[test]
    fn requests_are_refused_on_the_wrong_path() {
        let mut log = Quiet;
        let mut s = Session::new(
            Fake::new::<VariaxRequest>(Path::VariaxViaPodxt, vec![]),
            &AllowAll,
            &mut log,
        );
        assert!(s.send(PodxtRequest::EditBuffer).is_err());
        assert!(s.link.writes.is_empty());
    }

    #[test]
    fn a_closed_policy_sends_nothing() {
        let mut log = Quiet;
        let mut s = Session::new(
            Fake::new::<VariaxRequest>(Path::VariaxViaPodxt, vec![]),
            &AllClosed,
            &mut log,
        );
        assert!(s.send(VariaxRequest::Identity).is_err());
        assert!(s.link.writes.is_empty() && s.sent.is_empty());
    }
}
